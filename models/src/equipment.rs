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
use bevy_ecs::world::EntityRef;
use serde::{Deserialize, Serialize};

use crate::components::{
    Backpack, Curse, GameLog, KnownQuality, Launcher, Player, Position, Reach,
};
use crate::effects::{
    ArmorBonus, Bided, Effects, Grant, Grants, Held, Lifetime, Momentum, OnWear, SustainsArmor,
    lend, revoke, revoke_matching,
};
use crate::helpers::item_label;
use crate::identify::display_name;

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

/// Everything `wearer` currently has equipped, as a borrowing iterator.
///
/// The item's own [`Equipped`] component is the only thing consulted — not the
/// bearer's pack. That matters: the item system lifts an item out of the pack
/// while it resolves a "use", and a ring must not stop working for those few
/// lines. A monster that caught a thrown dagger is wearing it with no pack to
/// look in at all.
///
/// It is a full-world scan, and that is a constraint rather than a choice:
/// gear points at its wearer, so the only narrow query — `Query<&Equipped>` —
/// needs `&mut World`, and every caller here holds `&World` while it is part
/// way through reading something else off the same world. The scan is cheap
/// (a floor holds tens of entities, not thousands) and allocation-free; the
/// allocating [`equipped_items`] is for callers that go on to mutate.
pub fn equipped(world: &World, wearer: Entity) -> impl Iterator<Item = EntityRef<'_>> {
    world
        .iter_entities()
        .filter(move |e| e.get::<Equipped>().is_some_and(|eq| eq.by == Some(wearer)))
}

/// [`equipped`], collected — for the callers that mutate the world as they go
/// and so cannot hold a borrow of it across the loop.
pub fn equipped_items(world: &World, entity: Entity) -> Vec<Entity> {
    equipped(world, entity).map(|e| e.id()).collect()
}

/// Everything `entity` currently has equipped in `slot` — usually zero or one,
/// but up to [`Slot::capacity`] for [`Slot::Finger`].
fn equipped_in_slot(world: &World, entity: Entity, slot: Slot) -> Vec<Entity> {
    equipped(world, entity)
        .filter(|i| i.get::<Equipped>().is_some_and(|e| e.slot == slot))
        .map(|i| i.id())
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

/// The reach weapon (a bardiche, a whip) `entity` currently has in
/// `Slot::Hand`, if any. `v` (reach attack) gates on this before opening its
/// reticle.
pub fn wielded_reach_weapon(world: &World, entity: Entity) -> Option<Entity> {
    equipped_in(world, entity, Slot::Hand).filter(|&w| world.get::<Reach>(w).is_some())
}

/// Zeroes whatever [`Momentum`] `wearer`'s wielded weapon has built up — the
/// rapier's technique, lost the moment its wielder does anything but keep
/// swinging it (a plain step, a used item) — and, on the same logic, spends
/// the spell Bide the same way: coiled for one blow, lost the moment its
/// caster does anything else with the turn instead of landing it. Both are
/// no-ops when there is nothing to lose.
pub fn reset_momentum(world: &mut World, wearer: Entity) {
    revoke(world, wearer, Grant::of::<Bided>());
    let Some(weapon) = equipped_in(world, wearer, Slot::Hand) else {
        return;
    };
    if world.get::<Momentum>(weapon).is_some() {
        world.entity_mut(weapon).insert(Momentum(0));
    }
}

/// Takes `item` off `user`, no questions asked (no curse check, no logging).
/// Used by the curse-lifting scroll, which destroys the gear outright, and by
/// every other way a piece of gear leaves a hand — which is also where a
/// rapier's built-up [`Momentum`] resets: put down (or thrown, or knocked
/// away), it has nothing left to swing.
pub fn force_unequip(world: &mut World, item: Entity) {
    if let Some(mut e) = world.get_mut::<Equipped>(item) {
        e.by = None;
    }
    if world.get::<Momentum>(item).is_some() {
        world.entity_mut(item).insert(Momentum(0));
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
    let name = display_name(world, item);

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
                let stuck_name = display_name(world, occupants[0]);
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

    // Wearing something is how its plus and curse status come to light — the
    // same moment a ring's effect does. Announce the curse only the first
    // time; after that it's just what the name already says.
    let freshly_known = world.get::<KnownQuality>(item).is_none();
    world.entity_mut(item).insert(KnownQuality);
    if freshly_known && world.get::<Curse>(item).is_some() {
        let true_name = item_label(world, item);
        world
            .resource_mut::<GameLog>()
            .add(slot.cursed_reveal(&true_name));
    }

    // Last of all, anything that happens *because* it went on, rather than
    // while it is on: a ring of adornment spends itself here. It runs after the
    // item is fully worn, named and known, because it is allowed to be the last
    // thing that ever happens to the item.
    if let Some(OnWear(fire)) = world.get::<OnWear>(item).copied() {
        fire(world, user, item);
    }
    true
}

/// Puts `item` on `wearer` with none of the player-facing ceremony: no curse
/// check, no log line, and it refuses rather than swapping if the slot is
/// already taken. This is how a creature that is not the player comes by gear —
/// an orc catching a thrown dagger ([`crate::items::throw_system`]), a monster
/// spawning already equipped, a thief making off with something. Returns
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
    // Wearing it reveals its plus and curse status only when the *player* is
    // the one wearing it — this is how the player's own starting gear (handed
    // over already worn) ends up known from turn one. A monster spawning
    // equipped, catching a thrown weapon or making off with a stolen one
    // learns nothing the player didn't already know: identification is what
    // the player has learned, not what the item has been through.
    if world.get::<Player>(wearer).is_some() {
        world.entity_mut(item).insert(KnownQuality);
    }
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

// ---------------------------------------------------------------------------
// Damage to worn gear
// ---------------------------------------------------------------------------

/// Eats a point off the plus of whatever `victim` is wearing — an aquator's
/// touch ([`crate::effects::RustsArmor`]), which is the only thing in the
/// dungeon that damages gear rather than its owner.
///
/// It bites into the armour's [`ArmorBonus`] and never its [`ArmorDie`]: plate
/// mail corroded to nothing is still plate mail, just ruined plate mail, and a
/// suit can be driven well below zero. A scroll of enchant armour is the cure.
///
/// Three things stop it, and each says so in its own way: nothing worn to eat,
/// a wearer whose gear the magic runs off ([`SustainsArmor`] — a ring of
/// maintain armor), and a victim that isn't the player, whose gear the player
/// never sees a number for anyway. Returns whether anything was actually eaten.
pub fn corrode_armor(world: &mut World, victim: Entity) -> bool {
    let Some(armor) = equipped_in(world, victim, Slot::Body) else {
        return false;
    };
    let is_player = world.get::<Player>(victim).is_some();
    if world.get::<SustainsArmor>(victim).is_some() {
        if is_player {
            world
                .resource_mut::<GameLog>()
                .add("Your armour drinks the corrosion and shrugs it off.".to_string());
        }
        return false;
    }
    let was = world.get::<ArmorBonus>(armor).map_or(0, |b| b.0);
    world.entity_mut(armor).insert(ArmorBonus(was - 1));
    if is_player {
        let name = display_name(world, armor);
        world
            .resource_mut::<GameLog>()
            .add(format!("Your {name} corrodes! It is weaker."));
    }
    true
}

/// Reconciles the effects `bearer` has on loan from its gear with the effects
/// its gear actually grants right now: attaches what was just put on, strips
/// what was just taken off, and never touches what the creature was born with.
pub fn sync_equipment_effects(world: &mut World, bearer: Entity) {
    // What the gear lends right now, as (item, grant) pairs. The item is part
    // of the claim: two rings lending the same effect are two entries, and
    // taking one off must not strip what the other still lends.
    let wanted: Vec<(Entity, Grant)> = equipped(world, bearer)
        .filter_map(|i| i.get::<Grants>().map(|g| (i.id(), g.0)))
        .flat_map(|(item, grants)| grants.iter().map(move |&g| (item, g)))
        .collect();

    let worn: Vec<Entity> = wanted.iter().map(|(item, _)| *item).collect();
    let held: Vec<Held> = world
        .get::<Effects>(bearer)
        .map(|l| l.0.clone())
        .unwrap_or_default();

    // Anything lent by an item that is no longer on goes back. Every other
    // lifetime is somebody else's business — innate magic and a potion's gift
    // for the floor are not the gear's to take.
    revoke_matching(world, bearer, |h| match h.lifetime {
        Lifetime::WhileEquipped(item) => !worn.contains(&item),
        _ => false,
    });

    // And anything newly worn is lent. `held` is the ledger as it was before
    // the sweep, which is what makes this idempotent: an item already lending
    // an effect is not asked to lend it twice every turn.
    for (item, grant) in wanted {
        let already = held.iter().any(|h| {
            h.lifetime == Lifetime::WhileEquipped(item) && Some(h.id) == grant.effect_id()
        });
        if already {
            continue;
        }
        lend(world, bearer, grant, Lifetime::WhileEquipped(item));
    }
}

/// Reconciles every pack-carrying creature. [`toggle_equipped`] already keeps
/// the player in step; this catches the paths that change gear behind its
/// back — a curse-lifting scroll, a stolen item. A loaded save lends gear
/// effects itself (see [`crate::saveload::load_game`]), for wearers with no
/// pack as well, which this query does not reach.
///
/// Assumes nothing upstream: it reconciles unconditionally every turn.
/// `combat_system` and `visibility_system`, downstream, are the ones with the
/// assumption — that this has already folded lent effects into
/// `Loadout`-relevant components before either reads them.
pub fn equipment_effects_system(world: &mut World) {
    let bearers: Vec<Entity> = world
        .query_filtered::<Entity, With<Backpack>>()
        .iter(world)
        .collect();
    for bearer in bearers {
        sync_equipment_effects(world, bearer);
    }
}
