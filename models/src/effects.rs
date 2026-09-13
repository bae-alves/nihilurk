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
//! * **Caps** ([`MeleeCap`]) — a ceiling the dice cannot beat. Folded with
//!   `min` rather than `+`, since the strictest one wins.
//!
//! Const tables (the bestiary, the ring catalog) can't hold components
//! directly, so they name them through [`Grant`]: `Grant::of::<FireImmune>()`
//! is a const handle that knows how to attach, detach and probe one effect.
//! That single handle serves all four places an effect moves: a monster born
//! with it, a ring granting it while worn, a wand of cancellation stripping it,
//! and a save file recording it.

use bevy_ecs::prelude::*;
use bevy_ecs::world::{EntityRef, EntityWorldMut};
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

/// What this creature is *wearing* can't be eaten away (a ring of maintain
/// armor). [`SustainsStrength`]'s twin, one step further out: that one protects
/// the arm, this one protects the plate on it — an aquator's touch runs off it
/// leaving the armour's plus exactly where it was. See
/// [`crate::equipment::corrode_armor`].
#[derive(Component, Default, Clone, Copy)]
pub struct SustainsArmor;

/// This creature's touch eats armour: every blow it lands takes a point off the
/// plus of whatever its victim is wearing (the aquator). See
/// [`crate::equipment::corrode_armor`].
#[derive(Component, Default, Clone, Copy)]
pub struct RustsArmor;

/// Whatever carries this moves one notch below its own tempo — a ring of slow
/// digestion slows *everything* down, digestion included. Not a [`Speed`] of its
/// own: the notch is folded in when the tempo is read
/// ([`crate::conditions::tempo`]), so taking the ring off gives the notch back
/// and a potion of haste still reads on top of it.
///
/// [`Speed`]: crate::components::Speed
#[derive(Component, Default, Clone, Copy)]
pub struct Sluggish;

/// Nothing notices this creature until it is close enough to touch — two tiles
/// (a ring of stealth). Monsters that already know where it is because somebody
/// shrieked ([`crate::components::MovementType::Aggravated`]) come anyway: the
/// ring hides you, it does not unsay what the floor already heard. See
/// [`crate::ai`].
#[derive(Component, Default, Clone, Copy)]
pub struct Stealthy;

/// This creature knits itself back together as it goes: a roll each turn either
/// lifts one affliction or gives back a point of drained strength (a ring of
/// regeneration). A [`crate::abilities::PassiveAbility`], so the odds and the
/// mechanic live in that table rather than here.
#[derive(Component, Default, Clone, Copy)]
pub struct Regenerates;

/// Space will not hold still around this creature: every so often it is
/// somewhere else (a ring of teleportation). Rolled by
/// [`crate::abilities::passive_ability_system`], which runs at the tail of the
/// turn schedule — so the jump lands at the *start* of the bearer's next turn
/// and it acts from the new tile before anything else moves.
#[derive(Component, Default, Clone, Copy)]
pub struct Teleportitis;

/// This creature knows what items are *for*. It catches gear thrown at it and
/// puts it on — a hobgoblin that fields your dagger will be wielding it next
/// turn — and it can read a scroll that lands on it, out loud, with whatever
/// consequences that brings. Anything with the wits to work a sword has the wits
/// to work a page, so it is one flag rather than two. See
/// [`crate::items::throw_system`].
///
/// Hands and wits, not magic: it lives in the effect registry because that is
/// where a creature's innate properties live — and so a wand of cancellation can
/// knock the sense out of one.
#[derive(Component, Default, Clone, Copy)]
pub struct ItemUser;

/// Every so often, everything on the floor learns where this creature is (a
/// ring of aggravate monster). A *passive* ability that fires on a roll rather
/// than continuously — the odds and the flavour belong to the ability, not to
/// whatever granted it. See [`crate::abilities`].
#[derive(Component, Default, Clone, Copy)]
pub struct AggravatesMonsters;

/// This creature can loose an arrow properly rather than just lobbing it — a
/// drawn bow doubles the die of every arrow it throws. The bow lends it; nothing
/// about the arrow knows a bow exists, only which effect it answers to (see
/// [`crate::components::LaunchedBy`]).
#[derive(Component, Default, Clone, Copy)]
pub struct FireArrow;

/// The crossbow's half of the same bargain, for quarrels.
#[derive(Component, Default, Clone, Copy)]
pub struct FireQuarrel;

/// Airborne: this creature never sets off a floor trap it steps on (a dragon,
/// a griffin, a jabberwock, a kestral). See [`crate::traps::trap_system`].
#[derive(Component, Default, Clone, Copy)]
pub struct Flies;

/// This creature can breathe fire in place of a melee blow — a chance, on any
/// turn it would otherwise land one, of unleashing a wand-of-fire blast
/// instead (the dragon). See [`crate::items::dragon_breath`].
#[derive(Component, Default, Clone, Copy)]
pub struct FireBreath;

/// "Batty": every blow this creature lands, it tries to hop to a random
/// adjacent tile right afterward — landing only if that tile is open and
/// unoccupied (the bat, the phantom). See [`crate::abilities`].
#[derive(Component, Default, Clone, Copy)]
pub struct Batty;

/// Every hit from this creature clamps its victim in a bear trap's jaws (the
/// venus flytrap; a xeroc that has dropped its disguise). See
/// [`crate::abilities`].
#[derive(Component, Default, Clone, Copy)]
pub struct Binds;

/// Petrifies anything that attacks it, fires at it or zaps it — a gaze that
/// lands the instant the player targets this creature, not on the blow that
/// follows (the medusa). See [`crate::abilities::medusa_gaze`].
#[derive(Component, Default, Clone, Copy)]
pub struct Gorgon;

/// Every hit from this creature drinks a point of its victim's *maximum* HP
/// (the vampire). See [`crate::abilities`].
#[derive(Component, Default, Clone, Copy)]
pub struct Vampiric;

/// This creature's bite saps its victim's base power outright — like the dart
/// trap, but with no floor of 1: it can drive a victim's power negative (the
/// rattlesnake). See [`crate::abilities`].
#[derive(Component, Default, Clone, Copy)]
pub struct Venomous;

/// This creature covets coins: hurt and with a red coin somewhere on the
/// floor, it abandons the chase for it, and it scoops up any coin it steps on
/// that would actually help it (an orc). See [`crate::ai`].
#[derive(Component, Default, Clone, Copy)]
pub struct CoinGreedy;

/// This creature freezes what it touches: a chance on every hit of
/// paralysing the victim outright (the ice monster). See [`crate::abilities`].
#[derive(Component, Default, Clone, Copy)]
pub struct Freezing;

/// Every hit from this creature lifts something loose from its victim's pack,
/// uses it on the spot, and vanishes — the leprechaun's whole (dangerous)
/// routine. See [`crate::abilities::leprechaun_theft`].
#[derive(Component, Default, Clone, Copy)]
pub struct StealsAndFlees;

/// Every hit from this creature strips one thing its victim has *equipped*
/// and disappears the instant it does — the nymph. See
/// [`crate::abilities::nymph_theft`].
#[derive(Component, Default, Clone, Copy)]
pub struct StealsAndVanishes;

/// Not merely killable: cut down short of the last point of damage, this
/// creature buds a fresh copy of itself at its current HP, if there is
/// somewhere for it to stand (a slime). See [`crate::monsters::maybe_split`].
#[derive(Component, Default, Clone, Copy)]
pub struct Splits;

// ---------------------------------------------------------------------------
// Numeric modifiers
// ---------------------------------------------------------------------------

/// A number that stacks across every equipped source. Implemented by the
/// modifiers below so [`equipped_total`] can fold any of them with one body.
pub trait Modifier: Component + Copy {
    fn amount(self) -> i32;
}

/// Declares the modifier vocabulary: one row per modifier, giving the
/// component's type name and the [`Loadout`] field it folds into.
///
/// The rows are the only place a modifier is named. From them this generates
/// the component, its [`Modifier`] impl, the matching `Loadout` field and the
/// line of [`Loadout::absorb`] that sums it — so a sixth modifier is a row,
/// not three edits in three places with nothing to catch the one you forgot.
/// That mattered: the fold is a *concrete* struct rather than a generic pass
/// (see [`Loadout`]), and the price of the speed was exactly this kind of
/// hand-kept parallel list.
macro_rules! modifiers {
    ($($(#[$doc:meta])* $name:ident => $field:ident,)*) => {
        $(
            $(#[$doc])*
            #[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
            pub struct $name(pub i32);

            impl Modifier for $name {
                fn amount(self) -> i32 {
                    self.0
                }
            }
        )*

        /// Every number a creature's gear contributes, folded in **one** pass.
        ///
        /// [`equipped_total`] answers for one modifier, which is the right
        /// shape when a caller wants one — a trap measuring armour plus, a
        /// throw measuring its bonus. It is the wrong shape when a caller
        /// wants all of them, because each call walks the wearer's gear again:
        /// combat used to make four passes per blow and the HUD five per
        /// frame, over the same handful of items, to assemble numbers it
        /// needed together anyway.
        ///
        /// This is the same fold with the loop on the outside. There is no
        /// cache and no second copy of anything — `Equipped` is still the only
        /// record of who is wearing what, and this reads it once instead of
        /// six times.
        ///
        /// # This is a trade, and here is the other side of it
        ///
        /// Concrete fields are what make the single pass possible, and they
        /// cost generality: [`equipped_total::<C>`](equipped_total) takes a new
        /// modifier for free, and this does not. What it no longer costs is
        /// *silence* — the struct and the fold are generated from the row
        /// above, so a modifier cannot be added and quietly left out of the
        /// numbers. What is still by hand is the callers that read the fields
        /// they care about; a new field is simply unread until one of them
        /// asks for it, which is a visible nothing rather than a wrong number.
        ///
        /// Worth it at this table size — it took 5.4% off the game frame and
        /// left the ceiling where it was. It stops being worth it if the
        /// vocabulary ever grows past what one pass can usefully carry.
        #[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
        pub struct Loadout {
            $(
                #[doc = concat!("Summed [`", stringify!($name), "`] across everything equipped.")]
                pub $field: i32,
            )*
            /// The strictest [`MeleeCap`] anything equipped imposes. Folded
            /// with `min` rather than `+`, because ceilings do not add up —
            /// which is why the cap is written out below instead of riding in
            /// the generated rows.
            pub melee_cap: Option<i32>,
        }

        impl Loadout {
            /// Folds one item's contribution in. The entity's own components
            /// count as well as its gear's — a monster's innate `PowerBonus`
            /// is worth exactly what a ring's is, which is the whole point of
            /// the modifier vocabulary.
            fn absorb(&mut self, item: &EntityRef) {
                $( self.$field += item.get::<$name>().map_or(0, |m| m.0); )*
                if let Some(cap) = item.get::<MeleeCap>().map(|c| c.0) {
                    self.melee_cap = Some(self.melee_cap.map_or(cap, |had| had.min(cap)));
                }
            }
        }
    };
}

modifiers! {
    /// Adds to the bearer's attack **die size**: damage rolls `1d[power]`. This
    /// is what a weapon's class is worth (dagger 4, two-handed sword 10).
    PowerDie => power_die,
    /// Flat modifier added once to the bearer's damage roll — an enchantment, or
    /// a ring of strength.
    PowerBonus => power_bonus,
    /// Adds to the bearer's defence **die size**: the armour roll is
    /// `1d[armor]`. This is what a suit of armour is worth.
    ArmorDie => armor_die,
    /// Flat modifier added once to the bearer's armour roll — an enchantment, or
    /// a ring of protection.
    ArmorBonus => armor_bonus,
    /// Flat modifier added once to whatever the bearer *throws* — a ring of
    /// dexterity, or the plus on the bow steadying their aim. Folded from every
    /// equipped source the same way the melee bonus is, so it never matters
    /// which piece of gear supplied it.
    ThrowBonus => throw_bonus,
}

// ---------------------------------------------------------------------------
// One-shots
// ---------------------------------------------------------------------------

/// Something that happens once, the moment this item goes on — the ring of
/// adornment's flourish. Called with `(wearer, item)` by
/// [`crate::equipment::toggle_equipped`] after the item is on and identified,
/// and it owns everything that follows: the log lines, the animation, and
/// tagging the item with [`Consume`](crate::components::Consume) if putting it
/// on is what spends it.
///
/// A [`Grant`] cannot express this — a grant is a property held for as long as
/// the gear is worn, and this is an event — so it rides as its own component,
/// attached by the catalog row the same way grants are.
#[derive(Component, Clone, Copy)]
pub struct OnWear(pub fn(&mut World, Entity, Entity));

// ---------------------------------------------------------------------------
// Caps
// ---------------------------------------------------------------------------

/// A ceiling on what the bearer can do in melee, whatever the dice say.
///
/// Not a [`Modifier`]: caps do not add up. Two of them do not make a smaller
/// ceiling than the tighter one alone, so [`Loadout`] folds them with `min`
/// while it is summing everything else.
///
/// A bow carries `MeleeCap(1)`. Drawn, it is the best thing in the dungeon;
/// swung, it is a stick. That is the price of the hand it occupies.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct MeleeCap(pub i32);

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
    Grant::of::<ItemUser>(),
    Grant::of::<FireArrow>(),
    Grant::of::<FireQuarrel>(),
    Grant::of::<SustainsArmor>(),
    Grant::of::<RustsArmor>(),
    Grant::of::<Sluggish>(),
    Grant::of::<Stealthy>(),
    Grant::of::<Regenerates>(),
    Grant::of::<Teleportitis>(),
    Grant::of::<Flies>(),
    Grant::of::<Batty>(),
    Grant::of::<Binds>(),
    Grant::of::<Gorgon>(),
    Grant::of::<Vampiric>(),
    Grant::of::<Venomous>(),
    Grant::of::<CoinGreedy>(),
    Grant::of::<Splits>(),
    Grant::of::<Freezing>(),
    Grant::of::<StealsAndFlees>(),
    Grant::of::<StealsAndVanishes>(),
    Grant::of::<FireBreath>(),
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
    grants
        .iter()
        .filter_map(Grant::bit)
        .fold(0, |acc, bit| acc | bit)
}

/// The effects an entity currently has *on loan from its gear*, as opposed to
/// the ones it was born with. Kept so unequipping strips exactly what equipping
/// added and never touches a creature's innate magic.
#[derive(Component, Clone, Copy, Default)]
pub struct GrantedByGear(pub EffectSet);

/// The effects an entity has been lent **for the current floor** — a potion of
/// see invisible, as opposed to a ring of perception. Two things follow from
/// keeping them in their own set:
///
/// * [`crate::equipment::sync_equipment_effects`] counts them as innate, so
///   taking off a ring that happened to grant the same effect can never strip
///   the potion's copy of it.
/// * A staircase gives them all back at once
///   ([`crate::conditions::clear_player_conditions`]), which is what "lasts the
///   level" means.
#[derive(Component, Clone, Copy, Default)]
pub struct GrantedForFloor(pub EffectSet);

/// Lends `entity` one effect until it leaves the floor. Attaching twice is
/// harmless — the second dose of the same potion simply re-attaches it.
pub fn grant_for_floor(world: &mut World, entity: Entity, grant: Grant) {
    let had = world
        .get::<GrantedForFloor>(entity)
        .map(|g| g.0)
        .unwrap_or(0);
    let mut e = world.entity_mut(entity);
    grant.attach(&mut e);
    e.insert(GrantedForFloor(had | effect_set(&[grant])));
}

/// Takes back every effect `entity` holds only for this floor. An effect it also
/// owns innately, or has on loan from gear it is still wearing, stays put — the
/// potion's copy is the only thing given up.
pub fn clear_floor_grants(world: &mut World, entity: Entity) {
    let set = world
        .get::<GrantedForFloor>(entity)
        .map(|g| g.0)
        .unwrap_or(0);
    if set == 0 {
        return;
    }
    let innate = world
        .get::<Grants>(entity)
        .map(|g| effect_set(g.0))
        .unwrap_or(0);
    let gear = world.get::<GrantedByGear>(entity).map(|g| g.0).unwrap_or(0);
    let mut e = world.entity_mut(entity);
    for (i, grant) in EFFECTS.iter().enumerate() {
        let bit = 1 << i;
        let only_for_the_floor = set & bit != 0 && innate & bit == 0 && gear & bit == 0;
        if only_for_the_floor {
            grant.detach(&mut e);
        }
    }
    e.remove::<GrantedForFloor>();
}

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
    e.remove::<GrantedForFloor>();
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

// ---------------------------------------------------------------------------
// Folding modifiers across equipped gear
// ---------------------------------------------------------------------------

/// [`Loadout`] for `entity`: what it carries itself, plus everything it wears.
pub fn loadout(world: &World, entity: Entity) -> Loadout {
    let mut total = Loadout::default();
    if let Some(own) = world.get_entity(entity) {
        total.absorb(&own);
    }
    for item in crate::equipment::equipped(world, entity) {
        total.absorb(&item);
    }
    total
}

/// The total of modifier `C` across everything `entity` has equipped, plus any
/// it carries itself. The one place *one* number comes from; [`loadout`] is the
/// one place all of them do.
pub fn equipped_total<C: Modifier>(world: &World, entity: Entity) -> i32 {
    let own = world.get::<C>(entity).map(|c| c.amount()).unwrap_or(0);
    let worn: i32 = crate::equipment::equipped(world, entity)
        .filter_map(|item| item.get::<C>().map(|c| c.amount()))
        .sum();
    own + worn
}
