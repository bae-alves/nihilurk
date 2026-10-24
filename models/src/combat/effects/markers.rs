//! The marker effects: unit components a creature either has or lacks (fire immune,
//! sees invisible, vampiric, splits, lunges), plus the chimeric forms. Add a new one
//! here, then give it a row in `registry` (see `docs/how-to/add-an-effect.md`).

use super::*;

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
/// ring of sustain form). See `crate::items::wands::polymorph_entity_with`.
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
/// `crate::combat::fold_matchup` folds for its bearer, then is spent —
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
/// [`crate::ai`](mod@crate::ai).
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
/// [`Name`](crate::Name) and [`crate::components::Renderable`] are never rewritten.
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
/// glyph is overridden: `crate::helpers::item_label` and the map draw ask
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
/// [`crate::catalog::AmmoDef::launched_by`]).
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
/// (the dog). See `crate::companion::offer`.
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
/// a spirit poofs (the dog). See `crate::combat`'s `reveal_faerie`.
#[derive(Component, Default, Clone, Copy)]
pub struct FaerieOnDeath;

/// Marked to burst on its death (a scroll of justice): one fireball, centred on
/// where it fell, rolled off its own power die. It hurts everyone the blast
/// reaches, the reader included. See `crate::combat`'s `burst_on_death`.
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
/// `crate::combat::pay_for_the_corpse`.
#[derive(Component, Default, Clone, Copy)]
pub struct ScoreBounty;

/// This creature freezes what it touches: a chance on every hit of
/// paralysing the victim outright (the ice monster). See [`crate::abilities`].
#[derive(Component, Default, Clone, Copy)]
pub struct Freezing;

/// Every hit from this creature lifts something loose from its victim's pack,
/// uses it on the spot, and vanishes — the leprechaun's whole (dangerous)
/// routine. See `crate::abilities::leprechaun_theft`.
#[derive(Component, Default, Clone, Copy)]
pub struct StealsAndFlees;

/// Every hit from this creature strips one thing its victim has *equipped*
/// and disappears the instant it does — the nymph. See
/// `crate::abilities::nymph_theft`.
#[derive(Component, Default, Clone, Copy)]
pub struct StealsAndVanishes;

/// Not merely killable: cut down short of the last point of damage, this
/// creature buds a fresh copy of itself at its current HP, if there is
/// somewhere for it to stand (a slime). See [`crate::monsters::maybe_split`].
#[derive(Component, Default, Clone, Copy)]
pub struct Splits;

/// This creature's wounds well up green rather than red (a slime). Purely
/// cosmetic — read by `crate::helpers::spill_blood` to colour the stain it
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

/// The estoc's speed: every attack is thrown this many times in the time a
/// plainer blade manages once (`crate::combat::resolve_attack` fired back to
/// back). The number is the total, not the extra, and it is not on the marker:
/// the grant that lends it says how many — `Grant::counted::<Fencer>(n)` — the
/// ledger row keeps it through a save, and [`count_of`] reads it back, the
/// highest of the rows lending it.
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
