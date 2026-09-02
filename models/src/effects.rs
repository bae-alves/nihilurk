//! The effect vocabulary: the one language monsters, gear and systems all speak.
//!
//! A property a creature has — burning doesn't hurt it, it sees invisible
//! things, it hits harder — is a **component on the creature**, never an enum a
//! subsystem has to recognise. A dragon born fire-immune and a player wearing a
//! ring of fire resistance both carry [`FireImmune`], so the wand-of-fire code
//! asks one question and never learns that rings exist.
//!
//! Two kinds of effect live here:
//!
//! * **Markers** ([`FireImmune`], [`SeesInvisible`], …) — a yes/no property,
//!   read as a plain query filter: `Query<&Viewshed, With<SeesInvisible>>`.
//! * **Modifiers** ([`PowerDie`], [`ArmorBonus`], …) — a number that stacks.
//!   Combat folds every equipped source together with [`equipped_total`]
//!   without caring whether it came from a sword, plate mail or a ring.
//!
//! Const tables (the bestiary, the ring catalog) can't hold components
//! directly, so they name them through [`Grant`]: `Grant::of::<FireImmune>()`
//! is a const handle that knows how to attach, detach and probe one effect.
//! That single handle serves all four places an effect moves: a monster born
//! with it, a ring granting it while worn, a wand of cancellation stripping it,
//! and a save file recording it.

use bevy_ecs::prelude::*;
use bevy_ecs::world::EntityWorldMut;
use std::any::TypeId;

// ---------------------------------------------------------------------------
// Marker effects
// ---------------------------------------------------------------------------

/// Fire does nothing to this creature (the dragon; a ring of fire resistance).
#[derive(Component, Default, Clone, Copy)]
pub struct FireImmune;

/// Cold does nothing to this creature (the yeti).
#[derive(Component, Default, Clone, Copy)]
pub struct ColdImmune;

/// Undead: a wand of draining passes straight through, healing its wielder
/// nothing (zombie, phantom, vampire, wraith).
#[derive(Component, Default, Clone, Copy)]
pub struct Undead;

/// Every vorpal weapon slays this creature in one blow, whatever the weapon's
/// rolled bane (the Jabberwock). See [`crate::combat::resolve_attack`].
#[derive(Component, Default, Clone, Copy)]
pub struct VorpalTarget;

/// Everything invisible is visible to this creature: hidden traps, invisible
/// monsters, invisibly-stashed items (a ring of perception). See
/// [`crate::visibility`].
#[derive(Component, Default, Clone, Copy)]
pub struct SeesInvisible;

/// This creature's strength can't be drained by a poisoned dart trap (a ring of
/// strength). See [`crate::traps`].
#[derive(Component, Default, Clone, Copy)]
pub struct SustainsStrength;

/// Every so often, everything on the floor learns where this creature is (a
/// ring of aggravate monster). A *passive* ability that fires on a roll rather
/// than continuously — the odds and the flavour belong to the ability, not to
/// whatever granted it. See [`crate::abilities`].
#[derive(Component, Default, Clone, Copy)]
pub struct AggravatesMonsters;

// ---------------------------------------------------------------------------
// Numeric modifiers
// ---------------------------------------------------------------------------

/// A number that stacks across every equipped source. Implemented by the four
/// combat modifiers below so [`equipped_total`] can fold any of them with one
/// body.
pub trait Modifier: Component + Copy {
    fn amount(self) -> i32;
}

/// Macro-free boilerplate would be four near-identical impls; this keeps the
/// four modifier components to one line of intent each.
macro_rules! modifier {
    ($(#[$doc:meta])* $name:ident) => {
        $(#[$doc])*
        #[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
        pub struct $name(pub i32);

        impl Modifier for $name {
            fn amount(self) -> i32 {
                self.0
            }
        }
    };
}

modifier! {
    /// Adds to the bearer's attack **die size**: damage rolls `1d[power]`. This
    /// is what a weapon's class is worth (dagger 4, two-handed sword 10).
    PowerDie
}
modifier! {
    /// Flat modifier added once to the bearer's damage roll — an enchantment, or
    /// a ring of strength.
    PowerBonus
}
modifier! {
    /// Adds to the bearer's defence **die size**: the armour roll is
    /// `1d[armor]`. This is what a suit of armour is worth.
    ArmorDie
}
modifier! {
    /// Flat modifier added once to the bearer's armour roll — an enchantment, or
    /// a ring of protection.
    ArmorBonus
}

// ---------------------------------------------------------------------------
// Grant: naming an effect from a const table
// ---------------------------------------------------------------------------

fn attach_of<C: Component + Default>(entity: &mut EntityWorldMut) {
    entity.insert(C::default());
}

fn detach_of<C: Component>(entity: &mut EntityWorldMut) {
    entity.remove::<C>();
}

fn probe_of<C: Component>(world: &World, entity: Entity) -> bool {
    world.get::<C>(entity).is_some()
}

/// A const handle to one marker effect: how to put it on an entity, take it off
/// again, and ask whether it's there. Lets a `const` table name a component it
/// can't store — `Grant::of::<FireImmune>()`.
#[derive(Clone, Copy)]
pub struct Grant {
    id: TypeId,
    attach: fn(&mut EntityWorldMut),
    detach: fn(&mut EntityWorldMut),
    probe: fn(&World, Entity) -> bool,
}

impl Grant {
    /// The handle for marker component `C`.
    pub const fn of<C: Component + Default>() -> Self {
        Self {
            id: TypeId::of::<C>(),
            attach: attach_of::<C>,
            detach: detach_of::<C>,
            probe: probe_of::<C>,
        }
    }

    pub fn attach(&self, entity: &mut EntityWorldMut) {
        (self.attach)(entity)
    }

    pub fn detach(&self, entity: &mut EntityWorldMut) {
        (self.detach)(entity)
    }

    /// Whether `entity` currently carries this effect, from any source.
    pub fn probe(&self, world: &World, entity: Entity) -> bool {
        (self.probe)(world, entity)
    }

    /// This effect's slot in [`EFFECTS`] — its stable bit in an [`EffectSet`].
    fn bit(&self) -> Option<u32> {
        EFFECTS.iter().position(|g| g.id == self.id).map(|i| 1 << i)
    }
}

/// Every marker effect in the game, in a fixed order: an effect's index here is
/// the bit it occupies in an [`EffectSet`], which is what a save file stores.
/// **Append new effects at the end; never reorder** — that would rewrite the
/// meaning of existing saves.
pub const EFFECTS: &[Grant] = &[
    Grant::of::<FireImmune>(),
    Grant::of::<ColdImmune>(),
    Grant::of::<Undead>(),
    Grant::of::<VorpalTarget>(),
    Grant::of::<SeesInvisible>(),
    Grant::of::<SustainsStrength>(),
    Grant::of::<AggravatesMonsters>(),
];

/// The effects an entity hands out: innate magic on a monster, the effects a
/// piece of gear lends its bearer while equipped. Read by
/// [`crate::equipment::sync_equipment_effects`] (to lend them on) and by the
/// wand of cancellation (to know what a creature was born with).
#[derive(Component, Clone, Copy)]
pub struct Grants(pub &'static [Grant]);

/// A set of marker effects packed into one word, addressed by position in
/// [`EFFECTS`]. Used for bookkeeping that has to survive a save/load round trip
/// (see [`GrantedByGear`]).
pub type EffectSet = u32;

/// Which effects `grants` covers, as a bitmask.
pub fn effect_set(grants: &[Grant]) -> EffectSet {
    grants.iter().filter_map(Grant::bit).fold(0, |acc, bit| acc | bit)
}

/// The effects an entity currently has *on loan from its gear*, as opposed to
/// the ones it was born with. Kept so unequipping strips exactly what equipping
/// added and never touches a creature's innate magic.
#[derive(Component, Clone, Copy, Default)]
pub struct GrantedByGear(pub EffectSet);

/// Attaches every effect in `grants` to `entity` — how a monster is born with
/// its innate magic.
pub fn grant_all(world: &mut World, entity: Entity, grants: &'static [Grant]) {
    let mut e = world.entity_mut(entity);
    for g in grants {
        g.attach(&mut e);
    }
}

/// Strips every marker effect `entity` has, from any source, and forgets what it
/// was born with — the wand of cancellation. Walking [`EFFECTS`] means a new
/// effect is cancellable the moment it joins the registry.
pub fn revoke_all(world: &mut World, entity: Entity) {
    let mut e = world.entity_mut(entity);
    for g in EFFECTS {
        g.detach(&mut e);
    }
    e.remove::<Grants>();
    e.remove::<GrantedByGear>();
}

/// Reads an entity's marker effects back out as a bitmask, for saving.
pub fn effects_of(world: &World, entity: Entity) -> EffectSet {
    EFFECTS
        .iter()
        .enumerate()
        .filter(|(_, g)| g.probe(world, entity))
        .fold(0, |acc, (i, _)| acc | (1 << i))
}

/// Attaches every effect in a saved bitmask.
pub fn attach_effects(entity: &mut EntityWorldMut, set: EffectSet) {
    for (i, g) in EFFECTS.iter().enumerate() {
        if set & (1 << i) != 0 {
            g.attach(entity);
        }
    }
}

/// Restores marker effects from a saved bitmask.
pub fn restore_effects(world: &mut World, entity: Entity, set: EffectSet) {
    let mut e = world.entity_mut(entity);
    attach_effects(&mut e, set);
}

// ---------------------------------------------------------------------------
// Folding modifiers across equipped gear
// ---------------------------------------------------------------------------

/// The total of modifier `C` across everything `entity` has equipped, plus any
/// it carries itself. The one place gear turns into a number: combat and the HUD
/// both call this, and neither knows what kind of item supplied it.
pub fn equipped_total<C: Modifier>(world: &World, entity: Entity) -> i32 {
    let own = world.get::<C>(entity).map(|c| c.amount()).unwrap_or(0);
    let worn: i32 = crate::equipment::equipped_items(world, entity)
        .into_iter()
        .filter_map(|item| world.get::<C>(item).map(|c| c.amount()))
        .sum();
    own + worn
}
