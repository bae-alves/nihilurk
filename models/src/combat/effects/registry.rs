//! The registry: `Grant`, a const handle that attaches, detaches and probes one effect,
//! and `EFFECTS`, the table of every marker with the id a save stores. The `effects!`
//! table here is the one `tests/effects.rs` parses, so keep one row per line.

use super::*;

// ---------------------------------------------------------------------------
// Grant: naming an effect from a const table
// ---------------------------------------------------------------------------

pub(super) fn attach_of<C: Component + Default>(entity: &mut EntityWorldMut) {
    entity.insert(C::default());
}

pub(super) fn detach_of<C: Component>(entity: &mut EntityWorldMut) {
    entity.remove::<C>();
}

pub(super) fn probe_of<C: Component>(world: &World, entity: Entity) -> bool {
    world.get::<C>(entity).is_some()
}

/// A const handle to one effect: how to put it on an entity, take it off
/// again, and ask whether it's there. Lets a `const` table name a component it
/// can't store — `Grant::of::<FireImmune>()`. An effect with a number in it
/// ([`Fencer`]) is named with `Grant::counted::<Fencer>(2)`: the grant says
/// what the number is, the ledger row keeps it, and [`count_of`] reads it.
#[derive(Clone, Copy)]
pub struct Grant {
    id: TypeId,
    count: u8,
    attach: fn(&mut EntityWorldMut),
    detach: fn(&mut EntityWorldMut),
    probe: fn(&World, Entity) -> bool,
}

impl Grant {
    /// The handle for marker component `C`.
    pub const fn of<C: Component + Default>() -> Self {
        Self {
            id: TypeId::of::<C>(),
            count: 0,
            attach: attach_of::<C>,
            detach: detach_of::<C>,
            probe: probe_of::<C>,
        }
    }

    /// The handle for marker `C` lending `count` with it.
    pub const fn counted<C: Component + Default>(count: u8) -> Self {
        Self {
            id: TypeId::of::<C>(),
            count,
            attach: attach_of::<C>,
            detach: detach_of::<C>,
            probe: probe_of::<C>,
        }
    }

    /// The number this grant lends; `0` for an effect that has none.
    pub fn count(&self) -> u8 {
        self.count
    }

    /// Puts the marker on `entity` and does nothing else. No ledger row says
    /// who lent it or for how long, so it is missing from a save and nothing can
    /// revoke it. Go through [`lend`] unless you are the ledger itself.
    pub fn attach(&self, entity: &mut EntityWorldMut) {
        (self.attach)(entity)
    }

    /// Takes the marker off `entity` and does nothing else, so its ledger row
    /// outlives it. Go through [`revoke`] unless you are the ledger itself.
    pub fn detach(&self, entity: &mut EntityWorldMut) {
        (self.detach)(entity)
    }

    /// Whether `entity` currently carries this effect, from any source.
    pub fn probe(&self, world: &World, entity: Entity) -> bool {
        (self.probe)(world, entity)
    }

    /// The stable save-file id for this effect, or `None` when the handle
    /// names a component that is not a registered effect.
    pub fn effect_id(&self) -> Option<&'static str> {
        EFFECTS.iter().find(|e| e.grant.id == self.id).map(|e| e.id)
    }
}

/// One row of [`EFFECTS`]: the id a save file stores, and the handle that
/// attaches, detaches and probes the component it stands for.
///
/// The component itself is unchanged by any of this. `FireImmune` is still an
/// ordinary component the fire code asks about directly, and `SeesInvisible`
/// is still a query filter — this row is the *registry entry* for it, not a
/// replacement for it.
#[derive(Clone, Copy)]
pub struct Effect {
    /// Stable, never renamed. See [`EFFECTS`].
    pub id: &'static str,
    /// The handle that attaches, detaches and probes this effect's marker.
    pub grant: Grant,
    /// What the player reads when this runs out of turns, for the effects
    /// that end on their own. `None` for everything that does not — an
    /// immunity has no moment of wearing off to narrate.
    ///
    /// Written in the second person: only the player is told, because only
    /// the player is reading.
    pub ends: Option<&'static str>,
    /// What `Look` warns about when the reticle lands on a creature carrying
    /// this — named plainly ("venomous bite") rather than as whichever marker
    /// arms it. `None` for anything the player has no business being warned
    /// about: an immunity, and every trick that is theirs alone.
    ///
    /// This lives on the row because it is what the *marker* means to somebody
    /// looking at it, which is true whether or not the marker has an ability
    /// row anywhere. The engine used to keep its own 15-row copy of this list,
    /// in another crate, with nothing holding the two in agreement.
    pub beware: Option<&'static str>,
    /// What the player is told at the start of a run as a creature born with
    /// this (`-am`): "You feel venomous." `None` for everything no bestiary
    /// row is born with — and for anything the HUD already shows as a tag.
    pub feel: Option<&'static str>,
}

impl Effect {
    /// The row with this id, or `None` when a save names an effect this build
    /// does not have — a retired row, or one from a newer build.
    pub fn by_id(id: &str) -> Option<&'static Effect> {
        EFFECTS.iter().find(|e| e.id == id)
    }
}

/// Declares [`EFFECTS`]: one row per marker effect, `"id" => Type`.
///
/// The id and the type sit on the same line so the two cannot drift apart,
/// the same reason `modifiers!` generates its struct and its fold together.
macro_rules! effects {
    ($($id:literal => $ty:ty $(, ends $ends:expr)? $(, beware $beware:expr)? $(, feel $feel:expr)? ;)*) => {
        /// Every marker effect in the game.
        ///
        /// Each row pairs a **stable string id** with the component it attaches. The
        /// id is what a save file stores, which is the whole reason it exists: an
        /// index would mean the order here could never change, and deleting a row
        /// would silently shift every later effect in every existing save — a saved
        /// `Stealthy` coming back as `Regenerates`, with every index still a valid
        /// index and nothing able to tell. A name that is not in this table is
        /// detectable, and it is one effect rather than all of them.
        ///
        /// So rows may be reordered and retired freely. The one rule is **never
        /// rename an id**, the same rule a bestiary row already lives by
        /// (`docs/how-to/add-a-monster.md`): the name is the identity, on disk and
        /// nowhere else.
        ///
        /// Ids are written out rather than derived from the type name on purpose. A
        /// derived id would rename itself the moment somebody renamed the struct,
        /// which is exactly the silent save break this is here to prevent.
        pub const EFFECTS: &[Effect] = &[
            $(Effect {
                id: $id,
                grant: Grant::of::<$ty>(),
                #[allow(unused_mut, unused_assignments)]
                ends: { let mut e = None; $(e = Some($ends);)? e },
                #[allow(unused_mut, unused_assignments)]
                beware: { let mut b = None; $(b = Some($beware);)? b },
                #[allow(unused_mut, unused_assignments)]
                feel: { let mut f = None; $(f = Some($feel);)? f },
            },)*
        ];
    };
}

// The table itself. Its rules are documented on `EFFECTS` below, which is
// what a reader reaches for.
effects! {
    "fire_immune" => FireImmune, beware strings::beware_fireproof(), feel strings::feel_fireproof();
    "cold_immune" => ColdImmune, beware strings::beware_frostproof(), feel strings::feel_frostproof();
    "undead" => Undead, feel strings::feel_undead();
    "vorpal_target" => VorpalTarget, feel strings::feel_vorpal_target();
    "sees_invisible" => SeesInvisible;
    "sustains_strength" => SustainsStrength;
    "aggravates_monsters" => AggravatesMonsters, beware strings::beware_aggravating_shriek();
    "item_user" => ItemUser, feel strings::feel_item_user();
    "fire_arrow" => FireArrow;
    "fire_quarrel" => FireQuarrel;
    "fire_dart" => FireDart;
    "sustains_armor" => SustainsArmor;
    "rusts_armor" => RustsArmor, beware strings::beware_corrosive_touch(), feel strings::feel_rusts_armor();
    "sluggish" => Sluggish;
    "stealthy" => Stealthy;
    "regenerates" => Regenerates, beware strings::beware_regeneration(), feel strings::feel_regenerates();
    "teleportitis" => Teleportitis;
    "flies" => Flies, beware strings::beware_flying(), feel strings::feel_flies();
    "batty" => Batty, beware strings::beware_erratic_strikes(), feel strings::feel_batty();
    "binds" => Binds, beware strings::beware_binding_bite(), feel strings::feel_binds();
    "gorgon" => Gorgon, beware strings::beware_petrifying_gaze(), feel strings::feel_gorgon();
    "vampiric" => Vampiric, beware strings::beware_draining_touch(), feel strings::feel_vampiric();
    "venomous" => Venomous, beware strings::beware_venomous_bite(), feel strings::feel_venomous();
    "score_bounty" => ScoreBounty, feel strings::feel_score_bounty();
    "splits" => Splits, beware strings::beware_splitting_flesh(), feel strings::feel_splits();
    "green_blood" => GreenBlood, feel strings::feel_green_blood();
    "freezing" => Freezing, beware strings::beware_paralysing_touch(), feel strings::feel_freezing();
    "steals_and_flees" => StealsAndFlees, beware strings::beware_thieving_touch(), feel strings::feel_steals_and_flees();
    "steals_and_vanishes" => StealsAndVanishes, beware strings::beware_thieving_touch(), feel strings::feel_steals_and_vanishes();
    "always_tamed" => AlwaysTamed, feel strings::feel_always_tamed();
    "always_helper" => AlwaysHelper, feel strings::feel_always_helper();
    "priority_helper" => PriorityHelper, feel strings::feel_priority_helper();
    "shapeshift_on_kill" => ShapeshiftOnKill, feel strings::feel_shapeshift_on_kill();
    "mirror_on_kill" => MirrorOnKill, feel strings::feel_mirror_on_kill();
    "faerie_on_death" => FaerieOnDeath, feel strings::feel_faerie_on_death();
    "swims" => Swims, feel strings::feel_swims();
    "phasing" => Phasing, beware strings::beware_phasing();
    "cleaves" => Cleaves;
    "heavy_swing" => HeavySwing;
    "fencer" => Fencer;
    "lunges" => Lunges;
    "charges" => Charges;
    "lurk" => Lurk;
    "whirl_on_move" => WhirlOnMove;
    "vorpal_on_condition" => VorpalOnCondition;
    "turbo_magic" => TurboMagic;
    "self_damage_on_hit" => SelfDamageOnHit;
    "builds_momentum" => BuildsMomentum;
    "shatters_stone" => ShattersStone;
    "confusing_touch" => ConfusingTouch, beware strings::beware_confusing_touch();
    "bided" => Bided;
    // The holds. Each ends on its own clock, so a creature can be both asleep
    // and pinned and come out of each when its own turns run out — where the
    // one `Snare` component these replaced could only ever record the most
    // recent of them.
    "asleep" => Asleep, ends strings::ends_asleep();
    // The fourth hold, and the one that is not a `HOLDS` row — see `Petrified`.
    "petrified" => Petrified, ends strings::ends_petrified();
    "pinned" => Pinned, ends strings::ends_pinned();
    "rooted" => Rooted, ends strings::ends_rooted();
    "clamped" => Clamped, ends strings::ends_clamped();
    // The afflictions. Held for `Lifetime::Floor`, so a staircase lifts them
    // through the same machinery a potion of see invisible already used.
    "confused" => Confused;
    "blind" => Blind;
    "paralyzed" => Paralyzed;
    "magic_ward" => MagicWard;
    "detected" => Detected;
    "polymorphed" => Polymorphed;
    "polymorphitis" => Polymorphitis;
    "chimera" => Chimera;
    "typhon" => Typhon;
    "echidna" => Echidna;
    // The deck of cards' ledgers. See `crate::items`'s `decks`.
    "bala" => Bala;
    "bole" => Bole;
    "crit" => Crit;
    "oof" => Oof;
    "vuln" => Vuln;
    "time_stopped" => TimeStopped, ends strings::ends_time_stopped();
    // The runes' two.
    "protected" => Protected, ends strings::ends_protected();
    "explodes_on_death" => ExplodesOnDeath;
    "sustains_form" => SustainsForm;
    "dual_zap" => DualZap;
    "spiked" => Spiked;
}

/// The effects an entity hands out: innate magic on a monster, the effects a
/// piece of gear lends its bearer while equipped. Read by
/// [`crate::equipment::sync_equipment_effects`] (to lend them on) and by the
/// wand of cancellation (to know what a creature was born with).
#[derive(Component, Clone, Copy)]
pub struct Grants(pub &'static [Grant]);
