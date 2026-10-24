//! The status effects: confused, blind, paralyzed, asleep, pinned, rooted, clamped,
//! petrified, and the wards. They are markers like the rest; the ledger caps how many a
//! creature can hold (`CONDITION_CAP`).

use super::*;

/// A transient affliction on the **player** (a monster is confused through
/// `MovementType::Confused` instead). Half of every walk or swing while it
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
/// * No creature is perceptible at all, adjacent or not — every mob is [`Hidden`](crate::Hidden)
///   while it lasts, so auto-explore and auto-fight have nothing to work with
///   either.
/// * The monsters are not blinded in return: [`crate::ai`](mod@crate::ai) keeps using the view
///   the player *would* have, so this is never a way to hide.
///
/// Everything already explored stays on screen as fog-grey memory. Lifted the
/// same two ways [`Confused`] is. Shown in the HUD as `BLND`.
///
/// A blinded *monster* carries `MovementType::Confused` instead — it has no
/// viewshed to put out, so all blindness can do to it is make it grope.
#[derive(Component, Default, Clone, Copy)]
pub struct Blind;
/// Limbs locked up (a potion of paralysis). Whoever wears it loses a share of
/// their turns ([`crate::constants::potions::PARALYSIS_LOST_TURN_CHANCE`])
/// outright, and their tempo stays as it was. The player's lost turn reads no
/// key and the monsters move anyway; a monster's is spent in the monster
/// round. Lifted the same two ways [`Confused`] is, shown in the HUD as
/// `PARL`, and tinted on a monster so the player can see it can't fight back
/// properly. See [`crate::conditions::paralyse`].
#[derive(Component, Default, Clone, Copy)]
pub struct Paralyzed;
/// The spell Magic Ward: immunity to elemental/magic damage for the rest of
/// the floor. Set the moment the spell is cast, lifted like any other
/// floor-scoped condition at the next staircase
/// ([`crate::conditions::clear_player_conditions`]).
#[derive(Component, Default, Clone, Copy)]
pub struct MagicWard;
/// A scroll of protection: no damage of any kind, however it comes. Held for
/// [`crate::constants::scrolls::PROTECTION_TURNS`] turns. See
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
/// (`crate::combat::settle_the_dead`) whether or not its turns have run
/// out.
#[derive(Component, Default, Clone, Copy)]
pub struct Clamped;

/// Which entity's jaws a [`Clamped`] victim is caught in. Two venus flytraps
/// can both be alive in the same room, and only the one that actually bit you
/// should free you by dying — without this, killing an unrelated flytrap
/// would release a bite that flytrap never landed
/// (`crate::combat::release_biters_grip`).
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
