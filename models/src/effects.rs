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

use crate::components::{Fighter, GameLog, Player};
use crate::constants::combat::CHIP_DAMAGE;

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

/// No polymorph takes hold of this creature, and so no system shock either (a
/// ring of sustain form). See [`crate::items::wands::polymorph_entity_with`].
#[derive(Component, Default, Clone, Copy)]
pub struct SustainsForm;

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

/// Hands charged with a charm (a scroll of monster confusion): the next blow
/// the bearer *lands* confuses what it hits, and the charge is spent doing it.
/// Nothing about the bearer is impaired, which is why it is an effect and not
/// a condition — it is a buff they are holding, the same way [`Sluggish`] is
/// one they are suffering. It does not wear off with time and it survives a
/// staircase; [`crate::combat::resolve_attack`] discharges it. Shown in the
/// HUD as `GLOW`.
///
/// Whoever read the scroll carries it, monster or player alike, and the
/// confusion it delivers goes through [`crate::conditions::confuse`] — so a
/// hobgoblin that reads one you threw can charm *you* with its next punch.
#[derive(Component, Default, Clone, Copy)]
pub struct ConfusingTouch;

/// The spell Bide: coiled for one blow. Adds
/// [`crate::constants::combat::BIDE_ATTACK_BONUS`] to the very next attack
/// [`crate::combat::fold_matchup`] folds for its bearer, then is spent —
/// whether that swing hits, glances or misses. A double-striking estoc or a
/// cleave only ever sees it on the first swing of the turn. Do anything else
/// with the turn instead — walk without attacking, use or throw something,
/// cast another spell — and it is lost the same way, unspent: see
/// [`crate::equipment::reset_momentum`], which clears it on exactly the same
/// occasions it zeroes a rapier's [`Momentum`].
#[derive(Component, Default, Clone, Copy)]
pub struct Bided;

/// Nothing notices this creature until it is within
/// [`STEALTH_RANGE`](crate::constants::rings::STEALTH_RANGE) tiles (a ring of
/// stealth). Monsters that already know where it is because somebody
/// shrieked ([`crate::components::MovementType::Aggravated`]) come anyway: the
/// ring hides you, it does not unsay what the floor already heard. See
/// [`crate::ai`].
#[derive(Component, Default, Clone, Copy)]
pub struct Stealthy;

/// This creature knits itself back together as it goes: a roll each turn either
/// lifts one affliction or gives back a point of drained strength (a ring of
/// regeneration). An [`crate::abilities::Ability`] at [`crate::abilities::Moment::EachTurn`], so the odds and the
/// mechanic live in that table rather than here.
#[derive(Component, Default, Clone, Copy)]
pub struct Regenerates;

/// Space will not hold still around this creature: every so often it is
/// somewhere else (a ring of teleportation). Rolled by
/// [`crate::abilities::ability_system`], which runs at the tail of the
/// turn schedule — so the jump lands at the *start* of the bearer's next turn
/// and it acts from the new tile before anything else moves.
#[derive(Component, Default, Clone, Copy)]
pub struct Teleportitis;

/// This creature is wearing a species' powers for the floor (a potion or wand
/// of polymorph, a ring of polymorph's roll coming due). The badge, and the
/// answer to "may this one polymorph again" — the species itself is not
/// recorded: it is the grants lent beside this marker, each a
/// [`Lifetime::Floor`] entry in the ledger, so a staircase, a save and a wand
/// of cancellation all treat the loan like any other. The creature's own name,
/// glyph and numbers never change. Shown in the HUD as `POLY`.
///
/// A monster that is polymorphed is marked too — permanently, since no floor
/// ends a monster's polymorph, and a Helper keeps it down the stairs — so
/// polymorphing anything that holds this is the coin flip between system shock and a chimeric form
/// ([`Chimera`], [`FORMS`]).
#[derive(Component, Default, Clone, Copy)]
pub struct Polymorphed;

/// Every so often this creature turns into something else (a ring of
/// polymorph), at [`POLYMORPHITIS_CHANCE`](crate::constants::abilities::POLYMORPHITIS_CHANCE) a turn. Like any other polymorph, it is a coin flip
/// between a chimeric form and system shock once the bearer is [`Polymorphed`].
#[derive(Component, Default, Clone, Copy)]
pub struct Polymorphitis;

/// Polymorphed twice and not shocked to pieces: this creature is a chimera.
/// One of the three [`FORMS`], each a ledger effect for the floor like the
/// loan under it — so a staircase, a save and a wand of cancellation end it
/// without anything having been stashed. It changes only what the creature
/// is *called* and *drawn as*, read at the point of use ([`chimeric_form`]);
/// [`Name`] and [`crate::components::Renderable`] are never rewritten.
#[derive(Component, Default, Clone, Copy)]
pub struct Chimera;

/// The second of the three [`FORMS`]. See [`Chimera`].
#[derive(Component, Default, Clone, Copy)]
pub struct Typhon;

/// The third of the three [`FORMS`]. See [`Chimera`].
#[derive(Component, Default, Clone, Copy)]
pub struct Echidna;

/// The Balance (a deck card): [`BALA_POWER`] more on every damage roll this
/// creature makes, for the floor. Shown in the HUD as `BALA`.
///
/// [`BALA_POWER`]: crate::constants::decks::BALA_POWER
#[derive(Component, Default, Clone, Copy)]
pub struct Bala;

/// The Bole (a deck card): [`BOLE_ARMOR`] more on every armour roll this
/// creature makes, for the floor. Shown in the HUD as `BOLE`.
///
/// [`BOLE_ARMOR`]: crate::constants::decks::BOLE_ARMOR
#[derive(Component, Default, Clone, Copy)]
pub struct Bole;

/// VII THE CHARIOT (a deck card): every melee blow this creature lands is an
/// excellent hit, for the floor. Shown in the HUD as `CRT!`.
#[derive(Component, Default, Clone, Copy)]
pub struct Crit;

/// VII THE CHARIOT, reversed: every melee blow this creature takes is an
/// excellent hit, for the floor. Shown in the HUD as `OOF!`.
#[derive(Component, Default, Clone, Copy)]
pub struct Oof;

/// A charge's price: every melee blow this creature takes lands
/// [`VULN_DAMAGE_PERCENT`](crate::constants::combat::VULN_DAMAGE_PERCENT) harder,
/// until the creatures have had their turn. Shown in the HUD as `VULN`.
#[derive(Component, Default, Clone, Copy)]
pub struct Vuln;

/// XXII THE WORLD (a deck card): while the player holds this, nothing else on
/// the floor moves (`crate::ai::ai` stands down) and whatever the player throws
/// hangs in the air until it ends, save a returning weapon
/// ([`crate::components::Returns`]), which flies as ever. Shown in the HUD as
/// `WRLD`.
#[derive(Component, Default, Clone, Copy)]
pub struct TimeStopped;

/// One of the shapes a twice-polymorphed creature settles into.
pub struct FormDef {
    /// The marker that makes a creature this form ([`Chimera`], [`Typhon`] or
    /// [`Echidna`]), lent through the ledger like any other effect.
    pub grant: Grant,
    /// What the creature is drawn as while it holds the form. [`Renderable`]
    /// itself is left alone.
    ///
    /// [`Renderable`]: crate::components::Renderable
    pub glyph: char,
    /// What the creature is called while it holds the form, taken from the
    /// language table so each build names its forms in its own tongue.
    pub name: &'static str,
}

/// The chimeric forms: (C)himera, (T)yphon, (E)chidna. A creature holds at
/// most one.
pub const FORMS: &[FormDef] = &[
    FormDef {
        grant: Grant::of::<Chimera>(),
        glyph: 'C',
        name: strings::form_chimera(),
    },
    FormDef {
        grant: Grant::of::<Typhon>(),
        glyph: 'T',
        name: strings::form_typhon(),
    },
    FormDef {
        grant: Grant::of::<Echidna>(),
        glyph: 'E',
        name: strings::form_echidna(),
    },
];

/// The three form markers as one optional query term, for the systems that
/// only have a `Query` and so cannot call [`chimeric_form`]. Read the result
/// with [`form_of_marks`].
pub type FormMarks<'a> = Option<AnyOf<(&'a Chimera, &'a Typhon, &'a Echidna)>>;

/// [`chimeric_form`] for a [`FormMarks`] query term. Relies on [`FORMS`] being
/// in the order `Chimera`, `Typhon`, `Echidna`, which `FORMS` pins by its own
/// row order and a test pins from here.
pub fn form_of_marks(
    marks: Option<(Option<&Chimera>, Option<&Typhon>, Option<&Echidna>)>,
) -> Option<&'static FormDef> {
    let (c, t, e) = marks?;
    FORMS
        .iter()
        .zip([c.is_some(), t.is_some(), e.is_some()])
        .find_map(|(form, held)| held.then_some(form))
}

/// The form `entity` has settled into, if it has. The one place a name or a
/// glyph is overridden: [`crate::helpers::item_label`] and the map draw ask
/// here, and nothing stored on the creature changes.
pub fn chimeric_form(world: &World, entity: Entity) -> Option<&'static FormDef> {
    FORMS.iter().find(|f| f.grant.probe(world, entity))
}

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

/// The blowgun's half of the same bargain, for blowdarts.
#[derive(Component, Default, Clone, Copy)]
pub struct FireDart;

/// Airborne: this creature never sets off a floor trap it steps on (a dragon,
/// a griffin, a jabberwock, a kestral). See [`crate::traps::trap_system`].
#[derive(Component, Default, Clone, Copy)]
pub struct Flies;

/// Any treat thrown at this creature takes, every time, whichever kind it is
/// (the dog). See [`crate::companion::offer`].
#[derive(Component, Default, Clone, Copy)]
pub struct AlwaysTamed;

/// Charmed or conjured, this creature is the player's Helper outright, not a
/// plain ally (the dog). See [`crate::companion::charm`].
#[derive(Component, Default, Clone, Copy)]
pub struct AlwaysHelper;

/// A Helper that outranks the ordinary kind (the dog): it never explodes to
/// make room, so any number of them can stand beside you, and recruiting one
/// explodes the ordinary Helper, who is the one that gets replaced. See
/// [`crate::companion::the_helper`].
#[derive(Component, Default, Clone, Copy)]
pub struct PriorityHelper;

/// Underneath, this creature is a faerie shapeshifter. When it dies it is
/// revealed as one and is gone, with no corpse, no gore and no score, the way
/// a spirit poofs (the dog). See [`crate::combat`]'s `reveal_faerie`.
#[derive(Component, Default, Clone, Copy)]
pub struct FaerieOnDeath;

/// Marked to burst on its death (a rune of justice): one fireball, centred on
/// where it fell, rolled off its own power die. It hurts everyone the blast
/// reaches, the reader included. See [`crate::combat`]'s `burst_on_death`.
#[derive(Component, Default, Clone, Copy)]
pub struct ExplodesOnDeath;

/// A kill by this creature sometimes turns it into another random monster
/// (the dog). See [`crate::monsters::shapeshift`].
#[derive(Component, Default, Clone, Copy)]
pub struct ShapeshiftOnKill;

/// A melee kill by this creature turns its body into exactly what it killed
/// (the mirror hound). See [`crate::monsters::mirror`].
#[derive(Component, Default, Clone, Copy)]
pub struct MirrorOnKill;

/// Swims: deep water ([`crate::map::TileType::Water`]) is floor to this
/// creature and a wall to everything else (the eel, the ichthyocentaur). A
/// swimmer is only ever drawn for a water tile; see
/// [`crate::monsters::MonsterDef::pick_aquatic`] and
/// [`crate::map::Map::walkable`].
#[derive(Component, Default, Clone, Copy)]
pub struct Swims;

/// Phasing: terrain means nothing to this creature. It walks through walls
/// and deep water, and neither the diagonal rule nor the room leash holds it
/// (the ghost, and the player's Helper).
#[derive(Component, Default, Clone, Copy)]
pub struct Phasing;

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
/// follows (the medusa). Fired at [`crate::abilities::Moment::OnTargeted`].
#[derive(Component, Default, Clone, Copy)]
pub struct Gorgon;

/// Every hit from this creature drinks a point of its victim's *maximum* HP
/// (the vampire). See [`crate::abilities`].
#[derive(Component, Default, Clone, Copy)]
pub struct Vampiric;

/// This creature's bite saps its victim's base power outright — like the dart
/// trap, but with no floor: it can drive a victim's power negative (the
/// rattlesnake). See [`crate::abilities`].
#[derive(Component, Default, Clone, Copy)]
pub struct Venomous;

/// This creature's kill pays out extra score — a flat multiplier over the
/// usual max-HP payout (the apis guarding a treasure hive). See
/// [`crate::constants::score::BOUNTY_SCORE_MULTIPLIER`] and
/// [`crate::combat::pay_for_the_corpse`].
#[derive(Component, Default, Clone, Copy)]
pub struct ScoreBounty;

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

/// This creature's wounds well up green rather than red (a slime). Purely
/// cosmetic — read by [`crate::helpers::spill_blood`] to colour the stain it
/// leaves.
#[derive(Component, Default, Clone, Copy)]
pub struct GreenBlood;

// ---------------------------------------------------------------------------
// Weapon tricks — lent to the wielder exactly like a monster's innate magic
// (see `crate::catalog::WeaponDef::grants`), so `crate::abilities` and
// `crate::combat` probe the *wielder*, never the item.
// ---------------------------------------------------------------------------

/// The battle axe's whole trick: a swing that connects also lands on every
/// other enemy adjacent to the wielder (the target already struck excepted).
/// See `crate::abilities::cleave_attack`.
#[derive(Component, Default, Clone, Copy)]
pub struct Cleaves;

/// The greatclub's weight: a hit that lands staggers its victim — one turn
/// with no action at all, the same [`Asleep`] a
/// sleep trap uses — and the effort of the swing costs the wielder a beat of
/// their own, played out as one extra monster round immediately after. See
/// `crate::abilities::heavy_stagger` and [`crate::components::ExtraMonsterRound`].
#[derive(Component, Default, Clone, Copy)]
pub struct HeavySwing;

/// The estoc's speed: every attack is thrown twice in the time a plainer
/// blade manages once (`crate::combat::resolve_attack` fired back to back).
#[derive(Component, Default, Clone, Copy)]
pub struct Fencer;

/// Marks the player as a lurk — see [`crate::body`], which is where what a
/// lurk *is* gets written onto the entity.
///
/// A registered effect rather than a plain component for one reason: the
/// ledger is what carries a marker through a save, and the save format is
/// not versioned, so a new field there kills every run in progress. It is
/// the one entry in [`EFFECTS`] that is not magic, and the one
/// [`revoke_all`] refuses to take.
#[derive(Component, Default, Clone, Copy)]
pub struct Lurk;

/// The estoc's reach: closing the last stride of a run lands a lunge instead
/// of a step — see `crate::combat::resolve_lunge`.
///
/// Its own marker rather than half of [`Fencer`] because the two are not one
/// trick, even though the lurk is born with both: a wolf's opening lunge and
/// the second snap of the jaws that follows it are separate things a bestiary
/// entry could grant one without the other.
#[derive(Component, Default, Clone, Copy)]
pub struct Lunges;

/// CHARGE!: Shift + a direction with a creature in view closes the gap and
/// strikes at the price of being [`Vuln`] for the creatures' turn — see
/// `crate::fastmove::charge`. Nihil is born with it.
#[derive(Component, Default, Clone, Copy)]
pub struct Charges;

/// The chain-sickle's whirl: stepping between two tiles both adjacent to the
/// same enemy lands a free attack on it, no swing spent. See
/// `crate::abilities::whirl_attack`.
#[derive(Component, Default, Clone, Copy)]
pub struct WhirlOnMove;

/// The garrote's mercy: any hit against a target already carrying a negative
/// condition slays it outright, the same way a vorpalized blade finding its
/// bane does. See `crate::combat::garrote_vorpal`.
#[derive(Component, Default, Clone, Copy)]
pub struct VorpalOnCondition;

/// The staff's bargain: every damaging move the wielder casts costs
/// [`TURBO_MAGIC_COST_MULT`] times the [`crate::components::Magic`] and deals
/// [`TURBO_MAGIC_POWER_MULT`] times the damage. See
/// [`crate::items::spell_system`].
///
/// [`TURBO_MAGIC_COST_MULT`]: crate::constants::spells::TURBO_MAGIC_COST_MULT
/// [`TURBO_MAGIC_POWER_MULT`]: crate::constants::spells::TURBO_MAGIC_POWER_MULT
#[derive(Component, Default, Clone, Copy)]
pub struct TurboMagic;

/// The ring's bargain: a zap of an attack wand spends two charges and casts
/// twice, and a lone last charge casts once. See [`crate::items::item_system`].
#[derive(Component, Default, Clone, Copy)]
pub struct DualZap;

/// Spikemail's barbs: every blow that lands on the wearer pricks whoever
/// struck it for 1d2, past their armour. See `crate::abilities::spike_prick`.
#[derive(Component, Default, Clone, Copy)]
pub struct Spiked;

/// The chaos blade's price: every hit that connects bites its wielder for a
/// point of their own HP. See `crate::abilities::chaos_recoil`.
#[derive(Component, Default, Clone, Copy)]
pub struct SelfDamageOnHit;

/// The rapier's technique: every hit that lands builds [`Momentum`] on the
/// weapon itself. See `crate::abilities::build_momentum`.
#[derive(Component, Default, Clone, Copy)]
pub struct BuildsMomentum;

// ---------------------------------------------------------------------------
// Numeric modifiers
// ---------------------------------------------------------------------------

/// A number that stacks across every equipped source. Implemented by the
/// modifiers below so [`equipped_total`] can fold any of them with one body.
pub trait Modifier: Component + Copy {
    /// This source's contribution, which may be negative (a cursed blade).
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
    /// is what a weapon's class is worth: a dagger's die is small, a two-handed
    /// sword's large.
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
    /// sharpshooting, or the plus on the bow steadying their aim. Folded from every
    /// equipped source the same way the melee bonus is, so it never matters
    /// which piece of gear supplied it.
    ThrowBonus => throw_bonus,
    /// Flat modifier to the bearer's maximum hit points — a ring of health.
    /// Not read per blow: `crate::equipment` reconciles it into
    /// [`Fighter::max_hp`](crate::components::Fighter) when gear changes.
    MaxHpBonus => max_hp_bonus,
    /// A rapier's built-up momentum: [`MOMENTUM_PER_HIT`](crate::constants::abilities::MOMENTUM_PER_HIT) for every consecutive hit it lands,
    /// reset the moment its wielder stops swinging it (see
    /// [`crate::equipment::force_unequip`] and `crate::abilities::build_momentum`).
    /// Kept apart from [`PowerBonus`] so an enchanted rapier's plus and its
    /// momentum never overwrite each other.
    Momentum => momentum,
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

/// [`OnWear`]'s twin: something that happens once, the moment this item comes
/// *off* on purpose. Called with `(wearer, item)` by
/// [`crate::equipment::toggle_equipped`] after the item is off and its lent
/// effects are gone.
///
/// Deliberately fired only on the deliberate path, never from
/// [`crate::equipment::force_unequip`] — the same asymmetry [`OnWear`] already
/// has against [`crate::equipment::equip_silently`]. Dying, dropping and being
/// disarmed are not ceremonies, and a line about how you feel is out of place
/// in all three.
///
/// One row uses it: the staff, whose whole effect on a run is invisible on the
/// HUD, so the going and the coming back both have to be said out loud.
#[derive(Component, Clone, Copy)]
pub struct OnDoff(pub fn(&mut World, Entity, Entity));

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

/// A transient affliction on the **player** (a monster is confused through
/// [`MovementType::Confused`] instead). Half of every walk or swing while it
/// lasts goes off in a random direction ("You stumble foolishly"), and fast
/// movement, auto-explore and auto-fight all refuse to run. It is treacherous:
/// it does not wear off with time — only using a staircase or being caught by a
/// wand of cancellation clears it (both through
/// `crate::conditions::clear_player_conditions`). Shown in the HUD as `CONF`.
#[derive(Component, Default, Clone, Copy)]
pub struct Confused;
/// The **player** can't see (a potion of blindness). Four things follow, and
/// together they are the nastiest condition in the game:
///
/// * Their viewshed is cut to the 3x3 they could reach out and touch — no room
///   floods in however well lit ([`crate::visibility`]).
/// * Nothing in it has colour: every glyph they can make out is painted white.
/// * No creature is perceptible at all, adjacent or not — every mob is [`Hidden`]
///   while it lasts, so auto-explore and auto-fight have nothing to work with
///   either.
/// * The monsters are not blinded in return: [`crate::ai`] keeps using the view
///   the player *would* have, so this is never a way to hide.
///
/// Everything already explored stays on screen as fog-grey memory. Lifted the
/// same two ways [`Confused`] is. Shown in the HUD as `BLND`.
///
/// A blinded *monster* carries [`MovementType::Confused`] instead — it has no
/// viewshed to put out, so all blindness can do to it is make it grope.
#[derive(Component, Default, Clone, Copy)]
pub struct Blind;
/// Limbs locked up (a potion of paralysis). Whoever carries it has had their
/// [`Speed`] dropped to [`SpeedKind::Slow`]; on the **player** it costs a share
/// of the turns that still leaves them
/// ([`crate::constants::potions::PARALYSIS_LOST_TURN_CHANCE`]) outright — no key
/// read, the monsters move anyway. Lifted the same two ways [`Confused`] is, and
/// shown in the HUD as `PARL` alongside the `SLOW` the slowing earns.
///
/// A paralysed *monster* keeps only the slowing — nothing rolls dice on its
/// behalf — and wears this so the renderer can tint it. See
/// [`crate::conditions::paralyse`].
#[derive(Component, Default, Clone, Copy)]
pub struct Paralyzed;
/// The spell Magic Ward: immunity to elemental/magic damage for the rest of
/// the floor. Set the moment the spell is cast, lifted like any other
/// floor-scoped condition at the next staircase
/// ([`crate::conditions::clear_player_conditions`]).
#[derive(Component, Default, Clone, Copy)]
pub struct MagicWard;
/// A rune of protection: no damage of any kind, however it comes. Held for
/// [`crate::constants::runes::PROTECTION_TURNS`] turns. See
/// [`crate::helpers::apply_hit`] and [`crate::combat::resolve_attack`], the two
/// places HP comes off.
#[derive(Component, Default, Clone, Copy)]
pub struct Protected;
/// Turned up by a potion of detection: this thing draws on the map even where
/// the player cannot see it, dimly, for as long as they stay on this floor.
/// Nothing clears it — leaving the floor despawns everything that carries it.
///
/// It says only *where*: a detected monster's glyph does not animate, take
/// damage or get announced, because the player is sensing it rather than
/// watching it. See `crate::items::potions`.
#[derive(Component, Default, Clone, Copy)]
pub struct Detected;

/// Out cold: no action of any kind until it wears off. Sleeping gas, a
/// greatclub's stagger, a medusa's gaze. The strictest of the three holds —
/// a creature that is `Asleep` does nothing at all, whatever else is on it.
#[derive(Component, Default, Clone, Copy)]
pub struct Asleep;

/// Physically pinned by steel: a bear trap. Movement is impossible and
/// straining at the jaws costs a turn and draws blood
/// ([`crate::traps::bear_trap_thrash`]), but the victim can still strike
/// whatever comes within reach.
#[derive(Component, Default, Clone, Copy)]
pub struct Pinned;

/// Rooted to the spot by somebody else's words — a scroll of hold monster.
/// Mechanically a bear trap without the teeth: it cannot take a step, but it
/// can still strike, and straining at it costs nothing but the turn. Walking
/// away is what the scroll buys you, not free kills.
#[derive(Component, Default, Clone, Copy)]
pub struct Rooted;

/// Clamped in a living creature's jaws — the venus flytrap's bite, and a
/// revealed xeroc's. Mechanically a bear trap: movement is impossible and
/// straining at it costs a turn and draws blood, but the victim can still
/// strike whatever comes within reach.
///
/// Kept apart from [`Pinned`] because the two end differently. A bear trap's
/// hold is the floor's business and only turns lift it; a bite is the
/// biter's, so killing whatever is holding you frees you outright
/// ([`crate::combat::settle_the_dead`]) whether or not its turns have run
/// out.
#[derive(Component, Default, Clone, Copy)]
pub struct Clamped;

/// Which entity's jaws a [`Clamped`] victim is caught in. Two venus flytraps
/// can both be alive in the same room, and only the one that actually bit you
/// should free you by dying — without this, killing an unrelated flytrap
/// would release a bite that flytrap never landed
/// ([`crate::combat::release_biters_grip`]).
#[derive(Component, Clone, Copy)]
pub struct ClampedBy(pub Entity);

/// Lent by a war hammer while it is wielded: mass, and nothing else, gets
/// through stone. A blow from one is not capped by [`Petrified`] — it lands
/// whole, last point of HP included. See [`stone_chip`], whose one exception
/// this is.
#[derive(Component, Default, Clone, Copy)]
pub struct ShattersStone;

/// Turned to stone by a medusa's gaze. Like [`Asleep`] it costs the victim
/// every turn it lasts, and unlike it, it protects: nothing gets more than
/// [`crate::constants::combat::CHIP_DAMAGE`] through a petrified hide and
/// nothing takes its last point of HP ([`stone_chip`], which both damage
/// paths ask). Stone is therefore a hard stop rather than a death — the
/// player comes out of it wherever they went into it, unless what is standing
/// over them is swinging a war hammer ([`ShattersStone`]).
///
/// Deliberately **not** one of the [`HOLDS`]: those are things holding a
/// creature in a place, and a teleport is out of all three. Stone is the
/// victim's own body and travels with them.
#[derive(Component, Default, Clone, Copy)]
pub struct Petrified;

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

/// How long one held effect lasts, and what ends it.
///
/// Every effect an entity holds carries one of these, so "how long" is asked
/// once, on the row, rather than being implied by which of several parallel
/// structures happened to be storing it. The five cases are the five ways an
/// effect has ever ended in this game; a sixth would be a new variant here and
/// one arm wherever it is ended.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Lifetime {
    /// Never ends on its own. A monster's innate magic, and what the wand of
    /// cancellation is for.
    Permanent,
    /// Lent by a worn item, and lifted when that item comes off. Never
    /// serialised: a loaded save re-lends from the gear itself, so the
    /// [`Entity`] here is only ever meaningful within one session.
    WhileEquipped(Entity),
    /// Until the bearer leaves the floor — a potion of see invisible.
    Floor,
    /// Counts down one per turn and ends at zero. A snare, and every debuff
    /// with a duration after it.
    Turns(u32),
    /// Until the bearer does anything at all — Bide, coiled for one blow.
    NextAction,
}

impl Lifetime {
    /// Whether this lifetime survives being written to a save file. A
    /// gear-lent effect does not: the gear is saved, and lending it again is
    /// how it comes back.
    ///
    /// ```
    /// use bevy_ecs::prelude::Entity;
    /// use models::Lifetime;
    ///
    /// assert!(Lifetime::Permanent.is_saved());
    /// assert!(Lifetime::Turns(3).is_saved());
    /// assert!(!Lifetime::WhileEquipped(Entity::PLACEHOLDER).is_saved());
    /// ```
    pub fn is_saved(self) -> bool {
        !matches!(self, Lifetime::WhileEquipped(_))
    }

    /// Whether what is held this way will stop being held. The other two
    /// lifetimes are what a creature *is*: born with it, or wearing it, and
    /// neither is something that happened to it.
    ///
    /// Half of what [`Held::is_condition`] asks, and the half that has to be
    /// asked of the lifetime rather than the effect: the same `Regenerates`
    /// is a temporary boon out of a potion and a permanent one off a ring.
    pub fn is_transient(self) -> bool {
        matches!(
            self,
            Lifetime::Floor | Lifetime::Turns(_) | Lifetime::NextAction
        )
    }
}

/// How many conditions one creature carries at once. A fourth is refused — the
/// body has only so much room to be wrong in, and a player buried under six
/// badges cannot read their own state anyway.
pub const CONDITION_CAP: usize = 3;

/// One effect an entity is holding, and for how long.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Held {
    /// The [`Effect::id`] — a stable string, which is what reaches the disk.
    pub id: &'static str,
    /// What ends it. Only the saved kinds survive a reload; see
    /// [`Lifetime::is_saved`].
    pub lifetime: Lifetime,
}

impl Held {
    /// Whether this entry is a *condition*: something the creature is under
    /// right now, and one of the at most [`CONDITION_CAP`] it can be under.
    ///
    /// Both halves have to hold. A ring of regeneration lends a boon that is
    /// not transient, and a potion of magic detection lends a transient mark
    /// that is nothing the marked creature is feeling — neither is a
    /// condition, and neither may crowd a real one out. Which is why the
    /// effect half is [`crate::conditions::is_condition`], read off the lists
    /// that already declare the conditions, rather than a fresh list here:
    /// anything not on one of those lists is exempt by default, which is the
    /// safe direction for a rule that silently takes things away.
    pub fn is_condition(&self) -> bool {
        self.lifetime.is_transient() && crate::conditions::is_condition(self.id)
    }

    /// Whether this entry says what the creature *is* rather than what it can
    /// do — see [`IDENTITY_EFFECTS`]. The one thing [`revoke_all`] leaves.
    pub fn is_identity(&self) -> bool {
        IDENTITY_EFFECTS.contains(&self.id)
    }
}

impl Grant {
    /// [`Held::is_identity`], asked of the handle: a species' grants are lent
    /// to a polymorphed creature, but what a dog *is* is not.
    pub fn is_identity(&self) -> bool {
        self.effect_id()
            .is_some_and(|id| IDENTITY_EFFECTS.contains(&id))
    }
}

/// Everything an entity is holding: the ledger.
///
/// This records what is attached, from where, and for how long. It does **not**
/// record whether the entity has the property — the marker component does
/// that, the way it always has, and `world.get::<FireImmune>(e)` is still the
/// question the fire code asks.
///
/// One id may appear more than once, and that is the point. A creature born
/// fire-immune and also wearing a ring of fire resistance holds two entries;
/// taking the ring off removes one and the component stays, because the other
/// entry is still there. The three overlapping bitsets this replaced had to
/// intersect each other to work that out at every removal site.
#[derive(Component, Clone, Default, Debug)]
pub struct Effects(pub Vec<Held>);

impl Effects {
    /// Whether any entry still holds `id`.
    pub fn holds(&self, id: &str) -> bool {
        self.0.iter().any(|h| h.id == id)
    }

    /// The entries worth writing to a save file.
    pub fn saved(&self) -> Vec<Held> {
        self.0
            .iter()
            .copied()
            .filter(|h| h.lifetime.is_saved())
            .collect()
    }
}

/// Gives `entity` one effect for as long as `lifetime` says, attaching the
/// component if it is not already there. Returns whether it landed.
///
/// Lending the same effect twice leaves two entries on purpose: two sources,
/// two claims, and losing one must not strip what the other still lends.
///
/// A creature already carrying [`CONDITION_CAP`] conditions refuses another:
/// nothing attaches and this returns `false`. The player is told why; a
/// monster's refusal is silent, the same rule `tick_effects` keeps.
///
/// ```
/// use bevy_ecs::prelude::*;
/// use models::{FireImmune, Grant, Lifetime, lend, revoke, revoke_matching};
///
/// let mut world = World::new();
/// let dragon = world.spawn_empty().id();
/// let fire = Grant::of::<FireImmune>();
///
/// // Two sources lend the same immunity: a birthright, and a potion for the floor.
/// lend(&mut world, dragon, fire, Lifetime::Permanent);
/// lend(&mut world, dragon, fire, Lifetime::Floor);
///
/// // Losing one source leaves the other, so the immunity stays.
/// revoke_matching(&mut world, dragon, |held| held.lifetime == Lifetime::Floor);
/// assert!(fire.probe(&world, dragon));
///
/// revoke(&mut world, dragon, fire);
/// assert!(!fire.probe(&world, dragon));
/// ```
pub fn lend(world: &mut World, entity: Entity, grant: Grant, lifetime: Lifetime) -> bool {
    let Some(id) = grant.effect_id() else {
        return false;
    };
    let held = Held { id, lifetime };
    if held.is_condition() && conditions_held(world, entity) >= CONDITION_CAP {
        if world.get::<Player>(entity).is_some() {
            world
                .resource_mut::<GameLog>()
                .add(strings::too_many_conditions());
        }
        return false;
    }
    let mut e = world.entity_mut(entity);
    grant.attach(&mut e);
    let mut ledger = e.take::<Effects>().unwrap_or_default();
    ledger.0.push(held);
    e.insert(ledger);
    true
}

fn conditions_held(world: &World, entity: Entity) -> usize {
    world
        .get::<Effects>(entity)
        .map_or(0, |l| l.0.iter().filter(|h| h.is_condition()).count())
}

/// Drops every entry `doomed` accepts, and detaches the component behind any
/// id that no entry holds any more.
///
/// This is the one place an effect is taken away, so the "is anything else
/// still lending it?" question is asked once, here, instead of at each caller.
pub fn revoke_matching(world: &mut World, entity: Entity, doomed: impl Fn(&Held) -> bool) {
    let Some(mut ledger) = world.get_mut::<Effects>(entity).map(|l| l.clone()) else {
        return;
    };
    let lost: Vec<&'static str> = ledger
        .0
        .iter()
        .filter(|h| doomed(h))
        .map(|h| h.id)
        .collect();
    if lost.is_empty() {
        return;
    }
    ledger.0.retain(|h| !doomed(h));

    let mut e = world.entity_mut(entity);
    for id in lost {
        if ledger.holds(id) {
            continue;
        }
        if let Some(effect) = Effect::by_id(id) {
            effect.grant.detach(&mut e);
        }
    }
    e.insert(ledger);
}

/// Takes back every entry lending `grant`, whoever lent it: a charge spent, a
/// hold broken out of. The counterpart to [`lend`], and the reason no caller
/// has to spell an effect's id as a string — a misspelled literal compiles,
/// matches nothing, and revokes nothing, which is the same silence the bare
/// `remove` it replaced used to give.
pub fn revoke(world: &mut World, entity: Entity, grant: Grant) {
    let Some(id) = grant.effect_id() else {
        return;
    };
    revoke_matching(world, entity, |h| h.id == id);
}

/// [`revoke`] over several effects at once, in one pass of the ledger.
pub fn revoke_any(world: &mut World, entity: Entity, grants: &[Grant]) {
    let ids: Vec<&'static str> = grants.iter().filter_map(Grant::effect_id).collect();
    revoke_matching(world, entity, |h| ids.contains(&h.id));
}

/// Lends `entity` one effect until it leaves the floor. Lending twice is
/// harmless — the second dose of the same potion is a second entry, and the
/// staircase takes both.
pub fn grant_for_floor(world: &mut World, entity: Entity, grant: Grant) -> bool {
    lend(world, entity, grant, Lifetime::Floor)
}

/// Gives back everything `entity` was lent for this floor — what a staircase
/// does. Anything the creature was born with, or is wearing, is untouched,
/// because those are their own entries.
pub fn clear_floor_grants(world: &mut World, entity: Entity) {
    revoke_matching(world, entity, |h| h.lifetime == Lifetime::Floor);
}

/// Attaches every effect in `grants` to `entity` — how a monster is born with
/// its innate magic.
pub fn grant_all(world: &mut World, entity: Entity, grants: &'static [Grant]) {
    for g in grants {
        lend(world, entity, *g, Lifetime::Permanent);
    }
}

/// Strips every marker effect `entity` has, from any source, and forgets what
/// it was born with — the wand of cancellation. Walking the ledger means a new
/// effect is cancellable the moment it joins the registry.
pub fn revoke_all(world: &mut World, entity: Entity) {
    revoke_matching(world, entity, |h| !h.is_identity());
    let kept: Vec<Held> = world
        .get::<Effects>(entity)
        .map(|l| l.0.iter().copied().filter(|h| h.is_identity()).collect())
        .unwrap_or_default();
    let mut e = world.entity_mut(entity);
    for effect in EFFECTS {
        if IDENTITY_EFFECTS.contains(&effect.id) {
            continue;
        }
        effect.grant.detach(&mut e);
    }
    e.remove::<Grants>();
    e.remove::<Effects>();
    if !kept.is_empty() {
        e.insert(Effects(kept));
    }
}

/// The effects cancellation cannot take, because they are not magic: they
/// say what the creature *is*. A wand that unmade the lurk would leave
/// something wearing plate armour on four legs.
///
/// The line is drawn between a **body** and a **species**, and it is drawn
/// deliberately:
///
/// * A body — nihil, the lurk — is not cancellable. It is the creature the
///   run is about, and no wand in the dungeon is a wand of being something
///   else.
/// * A species is. A player wearing a dragon ([`crate::body::MonsterBody`])
///   keeps the dragon's shape and its dice, because those are not effects at
///   all, but loses its fire immunity to the same zap that would strip a real
///   dragon's — and an orc-bodied player loses [`ItemUser`], which is to say
///   the wits to work a buckle. That is the bargain of wearing something
///   else's magic: it can be taken off you.
/// * A dog is its grants, and nothing cancels them: they are what the dog
///   is, through any change of shape ([`crate::monsters::DOG_GRANTS`]). The
///   one exception is the shapeshifting itself ([`ShapeshiftOnKill`],
///   [`MirrorOnKill`]): a wand of cancellation stops a dog changing shape.
const IDENTITY_EFFECTS: &[&str] = &[
    "lurk",
    "always_tamed",
    "always_helper",
    "priority_helper",
    "faerie_on_death",
];

/// What `entity` holds, for saving. Gear-lent entries are left out; a loaded
/// save lends them again off the gear itself.
pub fn effects_of(world: &World, entity: Entity) -> Vec<Held> {
    world
        .get::<Effects>(entity)
        .map(Effects::saved)
        .unwrap_or_default()
}

/// Puts back what a save recorded.
///
/// An id this build does not have is dropped with a note rather than refused:
/// it means a retired row, or a save from a newer build, and losing one effect
/// beats losing the run. An index-based format could not tell the difference —
/// every index would still have been a valid index.
pub fn attach_effects(entity: &mut EntityWorldMut, held: &[Held]) -> Vec<&'static str> {
    let mut ledger = entity.take::<Effects>().unwrap_or_default();
    let mut unknown = Vec::new();
    for h in held {
        let Some(effect) = Effect::by_id(h.id) else {
            unknown.push(h.id);
            continue;
        };
        effect.grant.attach(entity);
        ledger.0.push(Held {
            id: effect.id,
            lifetime: h.lifetime,
        });
    }
    entity.insert(ledger);
    unknown
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

// ---------------------------------------------------------------------------
// The clock
// ---------------------------------------------------------------------------

/// Ages every effect held for a number of turns, and ends the ones that run
/// out.
///
/// One system for every timed effect there will ever be. It replaced
/// `snare_system`, which did this for exactly one component and could not have
/// done it for a second without being copied.
///
/// Runs at the very top of the turn, in the slot `snare_system` held, so a
/// creature's last turn of being held is spent held.
///
/// Assumes nothing: `smoke_system`, the only step before it, ages puffs and
/// touches no effect. It returns early once `Ending::player_dead` is set, so a
/// finished run stops ageing its ledgers.
pub fn tick_effects(world: &mut World) {
    if world
        .get_resource::<crate::state::Ending>()
        .is_some_and(|e| e.player_dead)
    {
        return;
    }

    let ticking: Vec<Entity> = world
        .query_filtered::<Entity, With<Effects>>()
        .iter(world)
        .collect();

    for entity in ticking {
        let Some(mut ledger) = world.get_mut::<Effects>(entity) else {
            continue;
        };
        let mut ended: Vec<&'static str> = Vec::new();
        for held in ledger.0.iter_mut() {
            let Lifetime::Turns(left) = held.lifetime else {
                continue;
            };
            let left = left.saturating_sub(1);
            held.lifetime = Lifetime::Turns(left);
            if left == 0 {
                ended.push(held.id);
            }
        }
        if ended.is_empty() {
            continue;
        }

        revoke_matching(world, entity, |h| h.lifetime == Lifetime::Turns(0));

        if world.get::<Player>(entity).is_none() {
            continue;
        }
        for id in ended {
            let Some(line) = Effect::by_id(id).and_then(|e| e.ends) else {
                continue;
            };
            world.resource_mut::<GameLog>().add(line.to_string());
        }
    }
}

// ---------------------------------------------------------------------------
// Stone
// ---------------------------------------------------------------------------

/// What a hit for `amount` actually does to a petrified creature, and the one
/// sentence it is reported with.
pub struct Chip {
    /// The HP that gets through the stone: a chip at most, and nothing at all
    /// against a creature already on its last point.
    pub through: i32,
    /// "You are chipped for 1 damage." — the line the blow is reported with,
    /// in place of whatever the source would otherwise have said.
    pub line: String,
}

/// The whole of what being [`Petrified`] does to a hit: at most
/// [`CHIP_DAMAGE`] gets through, and never the creature's last point of HP.
/// `None` when the target is not stone, which is the caller's signal to
/// resolve and report the hit its own way.
///
/// The one thing stone does not stop is a war hammer — see
/// [`crate::combat`], which asks whether the attacker carries
/// [`ShattersStone`] before it asks this at all. A hit with no attacker behind
/// it (a ray, a flame, a falling dart) has nothing to ask.
///
/// It lives here, with the effect, because both damage paths have to obey it
/// and say it identically — [`crate::combat::resolve_attack`], which applies
/// its own HP, and [`crate::helpers::apply_hit`], which every other source of
/// harm in the game goes through. Two copies of this rule would be two
/// answers to "can a petrified player die".
pub fn stone_chip(world: &World, target: Entity, amount: i32) -> Option<Chip> {
    world.get::<Petrified>(target)?;
    let hp = world.get::<Fighter>(target).map_or(0, |f| f.hp);
    let through = amount.clamp(0, CHIP_DAMAGE).min((hp - 1).max(0));
    let (who, verb) = match world.get::<Player>(target).is_some() {
        true => ("You".to_string(), "are"),
        false => (
            format!("The {}", crate::helpers::item_label(world, target)),
            "is",
        ),
    };
    let line = match through {
        0 => format!("{who} {verb} chipped for no damage."),
        n => format!("{who} {verb} chipped for {n} damage."),
    };
    Some(Chip { through, line })
}

/// The four holds: what keeps a creature where it stands, each on its own
/// clock. They are listed once, here, because the two ways out of one — the
/// scroll of teleportation and the wand of teleport away — have to let go of
/// all four, and a fifth hold added to the registry and forgotten at one of
/// those two sites would strand a creature nowhere near what was holding it.
///
/// [`Petrified`] is a hold and is deliberately not here: it is not something
/// holding the victim in a place, it is the victim's own body, and a teleport
/// takes the statue with it.
pub const HOLDS: [Grant; 4] = [
    Grant::of::<Asleep>(),
    Grant::of::<Pinned>(),
    Grant::of::<Rooted>(),
    Grant::of::<Clamped>(),
];

/// Holds `victim` for `turns` more turns — the steel jaws of a bear trap, a
/// lungful of sleeping gas, the words of a scroll of hold monster.
///
/// Deliberately silent: a hold arrives from a trap, a scroll or a cloud of
/// gas, and the sentence the player reads belongs to whichever it was.
/// Returns whether it changed anything — a creature already held this long by
/// the same thing is left alone rather than having its sentence shortened.
pub fn hold(world: &mut World, victim: Entity, grant: Grant, turns: u32) -> bool {
    if turns == 0 {
        return false;
    }
    let Some(id) = grant.effect_id() else {
        return false;
    };
    let standing = world
        .get::<Effects>(victim)
        .map(|l| {
            l.0.iter()
                .filter(|h| h.id == id)
                .filter_map(|h| match h.lifetime {
                    Lifetime::Turns(n) => Some(n),
                    _ => None,
                })
                .max()
                .unwrap_or(0)
        })
        .unwrap_or(0);
    if standing >= turns {
        return false;
    }
    // Replace rather than stack: a second dose of the same gas is a longer
    // sleep, not two sleeps running down side by side.
    revoke_matching(world, victim, |h| h.id == id);
    lend(world, victim, grant, Lifetime::Turns(turns))
}

/// How many turns `entity` has left of `grant`, or `None` if it is not held by
/// it at all.
pub fn turns_left(world: &World, entity: Entity, grant: Grant) -> Option<u32> {
    let id = grant.effect_id()?;
    world
        .get::<Effects>(entity)?
        .0
        .iter()
        .find_map(|h| match (h.id == id, h.lifetime) {
            (true, Lifetime::Turns(n)) => Some(n),
            _ => None,
        })
}

/// Every notable thing a creature carries, named plainly — what `Look` warns
/// about when the reticle lands on it.
///
/// Derived from [`EFFECTS`] rather than listed separately. The engine used to
/// keep its own 15-row copy of this, in another crate, with one closure per
/// marker and nothing holding the two lists in agreement: adding a thirteenth
/// on-hit ability and forgetting that list meant `Look` quietly stopped
/// warning about it, and no test could have said so.
///
/// A wielded launcher is the one entry that is not a marker — it is a question
/// about gear, not a property of the creature — so it is asked separately and
/// last.
pub fn dangers_of(world: &World, entity: Entity) -> Vec<&'static str> {
    let mut seen: Vec<&'static str> = EFFECTS
        .iter()
        .filter(|e| e.grant.probe(world, entity))
        .filter_map(|e| e.beware)
        .collect();
    seen.dedup();
    if crate::equipment::wielded_launcher(world, entity).is_some() {
        seen.push(strings::beware_ranged_shots());
    }
    if world
        .get::<crate::components::Spellset>(entity)
        .is_some_and(|s| !s.slots.is_empty())
    {
        seen.push(strings::beware_spellcasting());
    }
    seen
}
