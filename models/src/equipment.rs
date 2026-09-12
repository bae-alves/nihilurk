//! Wearing things, in one place.
//!
//! There used to be three of everything here — `Wield` for weapons, `Wear` for
//! armour, `PutOn` for rings, each with its own near-identical forty-line
//! toggle. There is now one [`Equipped`] component with a [`Slot`], and one
//! [`toggle_equipped`]. What makes a sword different from a ring is the
//! *components it carries* — [`crate::effects::PowerDie`], [`ArmorBonus`],
//! [`Grants`] — not a branch in this file.
//!
//! Equipping is also the only thing that lends effects between entities: a ring
//! carrying `Grants(&[Grant::of::<SeesInvisible>()])` puts [`SeesInvisible`] on
//! its bearer's own entity while worn, and takes it back off when removed. Every
//! other system therefore reads a plain component and stays ignorant of gear.
//!
//! [`ArmorBonus`]: crate::effects::ArmorBonus
//! [`Grants`]: crate::effects::Grants
//! [`SeesInvisible`]: crate::effects::SeesInvisible

use bevy_ecs::prelude::*;
use serde::{Deserialize, Serialize};

use crate::components::{Backpack, Curse, GameLog, KnownQuality, Launcher, Position};
use crate::effects::{EFFECTS, EffectSet, GrantedByGear, Grants, effect_set};

/// Where a piece of gear goes. One item per slot at a time, except
/// [`Slot::Finger`] — a hand has room for two rings.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum Slot {
    /// Weapons.
    Hand,
    /// Armour.
    Body,
    /// Rings. Holds up to [`Slot::capacity`], not just one.
    Finger,
}

impl Slot {
    /// How many items this slot holds at once — one, except a pair of rings.
    fn capacity(self) -> usize {
        match self {
            Slot::Finger => 2,
            Slot::Hand | Slot::Body => 1,
        }
    }

    /// "You ___ the dagger." — logged when the item goes on.
    fn donned(self, name: &str) -> String {
        match self {
            Slot::Hand => format!("You wield the {name}."),
            Slot::Body | Slot::Finger => format!("You put on the {name}."),
        }
    }

    /// "You ___ the dagger." — logged when the item comes off.
    fn doffed(self, name: &str) -> String {
        match self {
            Slot::Hand => format!("You stop wielding the {name}."),
            Slot::Body => format!("You take off the {name}."),
            Slot::Finger => format!("You remove the {name}."),
        }
    }

    /// Why this cursed item won't come off — and, by the same token, why it
    /// can't be thrown (see [`crate::items::throw_refusal`]).
    pub(crate) fn stuck(self, name: &str) -> String {
        match self {
            Slot::Hand => format!("You can't — the {name} is welded to your grip!"),
            Slot::Body => format!("You can't — the {name} clings to you and won't come off!"),
            Slot::Finger => format!("You can't — the {name} is fused to your finger!"),
        }
    }

    /// "It is cursed!" — logged the instant a freshly-worn item reveals itself
    /// as one, right after [`Slot::donned`].
    fn cursed_reveal(self, name: &str) -> String {
        match self {
            Slot::Hand => format!("The {name} welds itself to your grip! It is cursed!"),
            Slot::Body => format!("The {name} clings to your body! It is cursed!"),
            Slot::Finger => format!("The {name} welds to your finger! It is cursed!"),
        }
    }

    /// Why the cursed item already in this slot blocks a swap.
    fn blocked(self, name: &str) -> String {
        match self {
            Slot::Hand => format!("You can't switch weapons — the {name} won't leave your hand."),
            Slot::Body => format!("You can't change armour — the {name} won't come off."),
            Slot::Finger => format!("You can't — the {name} won't leave your finger."),
        }
    }
}

/// Gear: which slot it occupies, and who is currently using it (`None` when it's
/// loose in a pack or on the floor). Carried by every weapon, suit of armour and
/// ring — the components *alongside* it are what make those three different.
#[derive(Component, Clone, Copy)]
pub struct Equipped {
    pub by: Option<Entity>,
    pub slot: Slot,
}

impl Equipped {
    /// A fresh, unworn piece of gear for `slot`.
    pub const fn loose(slot: Slot) -> Self {
        Self { by: None, slot }
    }
}

/// Everything `entity` currently has equipped.
///
/// The item's own [`Equipped`] component is the only thing consulted — not the
/// bearer's pack. That matters: the item system lifts an item out of the pack
/// while it resolves a "use", and a ring must not stop working for those few
/// lines.
pub fn equipped_items(world: &World, entity: Entity) -> Vec<Entity> {
    world
        .iter_entities()
        .filter(|e| e.get::<Equipped>().is_some_and(|eq| eq.by == Some(entity)))
        .map(|e| e.id())
        .collect()
}

/// Everything `entity` currently has equipped in `slot` — usually zero or one,
/// but up to [`Slot::capacity`] for [`Slot::Finger`].
fn equipped_in_slot(world: &World, entity: Entity, slot: Slot) -> Vec<Entity> {
    equipped_items(world, entity)
        .into_iter()
        .filter(|&i| world.get::<Equipped>(i).is_some_and(|e| e.slot == slot))
        .collect()
}

/// What `entity` has equipped in `slot`, if anything. For [`Slot::Finger`],
/// which can hold two, this is only the first one found — callers that care
/// about both rings want [`equipped_in_slot`] instead.
pub fn equipped_in(world: &World, entity: Entity, slot: Slot) -> Option<Entity> {
    equipped_in_slot(world, entity, slot).into_iter().next()
}

/// The bow or crossbow `entity` currently has in `Slot::Hand`, if any. `f`
/// (fire) and ranged auto-fight both gate on this before reaching for ammo.
pub fn wielded_launcher(world: &World, entity: Entity) -> Option<Entity> {
    equipped_in(world, entity, Slot::Hand).filter(|&w| world.get::<Launcher>(w).is_some())
}

/// Takes `item` off `user`, no questions asked (no curse check, no logging).
/// Used by the curse-lifting scroll, which destroys the gear outright.
pub fn force_unequip(world: &mut World, item: Entity) {
    if let Some(mut e) = world.get_mut::<Equipped>(item) {
        e.by = None;
    }
}

/// Puts `item` on, or takes it off if it's already on — the one path for every
/// slot. Returns `true` if the equipped state actually changed.
///
/// Equipping first frees room in the slot if it's full — both rings, for
/// [`Slot::Finger`] — and a cursed occupant refuses to budge.
pub fn toggle_equipped(world: &mut World, user: Entity, item: Entity) -> bool {
    let Some(slot) = world.get::<Equipped>(item).map(|e| e.slot) else {
        return false;
    };
    let name = crate::identify::display_name(world, item);

    // Already on: take it off, unless it's cursed.
    if world.get::<Equipped>(item).and_then(|e| e.by) == Some(user) {
        if world.get::<Curse>(item).is_some() {
            world.resource_mut::<GameLog>().add(slot.stuck(&name));
            return false;
        }
        force_unequip(world, item);
        world.resource_mut::<GameLog>().add(slot.doffed(&name));
        sync_equipment_effects(world, user);
        return true;
    }

    // Full up: make room, evicting an uncursed occupant over a cursed one. A
    // hand short one ring, say, has room to spare and skips this entirely.
    let occupants = equipped_in_slot(world, user, slot);
    if occupants.len() >= slot.capacity() {
        match occupants.iter().find(|&&e| world.get::<Curse>(e).is_none()) {
            Some(&evictable) => force_unequip(world, evictable),
            None => {
                let stuck_name = crate::identify::display_name(world, occupants[0]);
                world
                    .resource_mut::<GameLog>()
                    .add(slot.blocked(&stuck_name));
                return false;
            }
        }
    }

    if let Some(mut e) = world.get_mut::<Equipped>(item) {
        e.by = Some(user);
    }
    world.resource_mut::<GameLog>().add(slot.donned(&name));
    sync_equipment_effects(world, user);
    crate::identify::learn_by_wearing(world, item);

    // Wearing something is how its plus and curse status come to light — the
    // same moment a ring's effect does. Announce the curse only the first
    // time; after that it's just what the name already says.
    let freshly_known = world.get::<KnownQuality>(item).is_none();
    world.entity_mut(item).insert(KnownQuality);
    if freshly_known && world.get::<Curse>(item).is_some() {
        let true_name = crate::helpers::item_label(world, item);
        world
            .resource_mut::<GameLog>()
            .add(slot.cursed_reveal(&true_name));
    }
    true
}

/// Puts `item` on `wearer` with none of the player-facing ceremony: no curse
/// check, no log line, and it refuses rather than swapping if the slot is
/// already taken. This is how a creature that is not the player comes by gear —
/// an orc catching a thrown dagger ([`crate::items::throw_system`]). Returns
/// whether it went on.
pub fn equip_silently(world: &mut World, wearer: Entity, item: Entity) -> bool {
    let Some(slot) = world.get::<Equipped>(item).map(|e| e.slot) else {
        return false;
    };
    if equipped_in_slot(world, wearer, slot).len() >= slot.capacity() {
        return false;
    }
    if let Some(mut e) = world.get_mut::<Equipped>(item) {
        e.by = Some(wearer);
    }
    sync_equipment_effects(world, wearer);
    // Wearing it — even without the ceremony — still reveals its plus and
    // curse status. This is also how the player's own starting gear (handed
    // over already worn) ends up known from turn one.
    world.entity_mut(item).insert(KnownQuality);
    true
}

/// Strips everything `wearer` has equipped and lays it out on `at` — the gear a
/// dying creature leaves behind, and the gear a monster abandons when the floor
/// it stands on is torn down. Curses are no obstacle: the wearer is past caring.
pub fn drop_equipment(world: &mut World, wearer: Entity, at: Position) {
    for item in equipped_items(world, wearer) {
        force_unequip(world, item);
        world.entity_mut(item).insert(at);
    }
    sync_equipment_effects(world, wearer);
}

/// Reconciles the effects `bearer` has on loan from its gear with the effects
/// its gear actually grants right now: attaches what was just put on, strips
/// what was just taken off, and never touches what the creature was born with.
pub fn sync_equipment_effects(world: &mut World, bearer: Entity) {
    let wanted: EffectSet = equipped_items(world, bearer)
        .into_iter()
        .filter_map(|i| world.get::<Grants>(i).map(|g| effect_set(g.0)))
        .fold(0, |acc, set| acc | set);

    let had: EffectSet = world.get::<GrantedByGear>(bearer).map(|g| g.0).unwrap_or(0);
    if had == wanted {
        return;
    }

    // Effects the creature has innately are never on loan, so a removed ring can
    // never strip a monster's own magic.
    let innate: EffectSet = world
        .get::<Grants>(bearer)
        .map(|g| effect_set(g.0))
        .unwrap_or(0);

    let mut e = world.entity_mut(bearer);
    for (i, grant) in EFFECTS.iter().enumerate() {
        let bit = 1 << i;
        if wanted & bit != 0 && had & bit == 0 {
            grant.attach(&mut e);
        }
        if had & bit != 0 && wanted & bit == 0 && innate & bit == 0 {
            grant.detach(&mut e);
        }
    }
    e.insert(GrantedByGear(wanted));
}

/// Reconciles every gear-bearing creature. [`toggle_equipped`] already keeps the
/// player in step; this catches the paths that change gear behind its back — a
/// loaded save, a curse-lifting scroll, a stolen item.
pub fn equipment_effects_system(world: &mut World) {
    let bearers: Vec<Entity> = world
        .query_filtered::<Entity, With<Backpack>>()
        .iter(world)
        .collect();
    for bearer in bearers {
        sync_equipment_effects(world, bearer);
    }
}
