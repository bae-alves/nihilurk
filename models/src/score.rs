//! The one place the number on the HUD moves — and the flash it makes moving.
//!
//! Score is a single [`Score`] component on the player, and everything that
//! pays into it does so through [`award`]. Keeping the arithmetic here means the
//! whole scoring table is one screen of code rather than a constant scattered
//! across combat, the stairs and the endgame:
//!
//! | What | Worth |
//! |---|---|
//! | Killing a creature | [`KILL_PER_MAX_HP`] × the creature's `max_hp` |
//! | Killing more than one in a turn | the turn's kills, ×(1 + [`COMBO_BONUS_PER_KILL`] per extra corpse) |
//! | Taking a staircase | [`STAIR_PER_TIER`] × the floor's difficulty tier |
//! | Putting on a ring of adornment | double whatever you have |
//! | Climbing out with the Element | double whatever you have |
//!
//! The two doublings are the same verb ([`double`]) because they are the same
//! idea: the run is not scored on what you killed, it is scored on getting out
//! with style. See [`crate::items::rings::do_it_with_style`].
//!
//! Nothing here is ever silent. Every payment sets a [`ScoreFlash`], and the HUD
//! spends the scorekeeper's own space on it for a beat: `+700` in a colour
//! picked at random, `COMBO! +800` when a second corpse hits the floor in the
//! same turn, or, for the two doublings, the word `DOUBLE`. The words come in
//! the stripes of whatever flag the run is flying ([`crate::pride`]); the
//! number never does. An arcade scoreboard that does not react is just a
//! number.

use bevy_ecs::prelude::*;
use crossterm::style::Color;
use rand::Rng;

use crate::components::{GameLog, LogCategory, Player, Score};
use crate::map::FxRng;
use crate::particles::GLORY_COLORS;

// --- Tuning constants ------------------------------------------------------
// Defined and documented in `constants.rs`.
//
//   KILL_PER_MAX_HP       what one point of a slain creature's max_hp pays
//   COMBO_BONUS_PER_KILL  what each corpse past the first adds to the turn
//   COMBO_PRIDE_CHANCE    how often a combo is done with pride instead
//   STAIR_PER_TIER        what one difficulty tier pays, each staircase
//   SCORE_FLASH_TURNS     how long a payment stays lit on the scorekeeper
pub use crate::constants::score::{
    COMBO_BONUS_PER_KILL, COMBO_PRIDE_CHANCE, KILL_PER_MAX_HP, SCORE_FLASH_TURNS, STAIR_PER_TIER,
};

/// What the scorekeeper is shouting this instant, if anything.
///
/// Set by every payment, aged once per turn by [`score_turn_system`] at the
/// tail of the schedule — so a flash armed anywhere in a turn (the turn's
/// killing, settled at that same tail; a staircase before it) survives exactly
/// one full frame of screen time and is gone by the player's next action.
///
/// `colors` is read per character and cycled, so one colour paints the whole
/// string and six paint `DOUBLE` a letter each.
#[derive(Resource, Default)]
pub struct ScoreFlash {
    pub text: String,
    pub colors: Vec<Color>,
    turns: u8,
}

impl ScoreFlash {
    /// Whether the scorekeeper should be showing the flash instead of the
    /// score right now.
    pub fn lit(&self) -> bool {
        self.turns > 0 && !self.text.is_empty()
    }

    /// The colour for character `i` of [`ScoreFlash::text`].
    pub fn color_at(&self, i: usize) -> Color {
        match self.colors.is_empty() {
            true => Color::White,
            false => self.colors[i % self.colors.len()],
        }
    }

    fn light(&mut self, text: String, colors: Vec<Color>) {
        self.text = text;
        self.colors = colors;
        self.turns = SCORE_FLASH_TURNS;
    }
}

/// What has died *this turn*, and what it is worth so far.
///
/// A turn's kills are scored as one pile, once, when the dying is over — not a
/// payment per corpse. Every corpse past the first is worth
/// [`COMBO_BONUS_PER_KILL`] more on the whole pile, so a blast that takes three
/// at once pays double what the same three would piece by piece, and the
/// scoreboard says so with a single number and a single line rather than
/// counting them off one at a time.
///
/// Settled and emptied by [`score_turn_system`] at the tail of the turn — one
/// turn's killing never combos into the next one's.
#[derive(Resource, Default)]
pub struct Combo {
    kills: u32,
    /// The turn's kills at face value, before the multiplier.
    base: i32,
}

impl Combo {
    /// The pile as it stands: what the turn's dead are worth with the
    /// multiplier folded in, and how many of them there were. `(0, 0)` for a
    /// turn nothing died in.
    fn total(&self) -> (i32, u32) {
        if self.kills == 0 {
            return (0, 0);
        }
        let multiplier = 1.0 + COMBO_BONUS_PER_KILL * (self.kills - 1) as f32;
        ((self.base as f32 * multiplier) as i32, self.kills)
    }
}

/// Closes the turn out on the scorekeeper: ages the flash, then pays for
/// everything that died.
///
/// Registered at the tail of the turn schedule, after everything that can kill.
/// That placement is the whole design — the corpses that combo together are
/// exactly the ones that fell between two of these, the multiplier is only
/// applied once the dying is over, and the flash it arms is still lit for this
/// turn's render (the ageing above happens first) and dark by the next.
/// Assumes everything capable of killing this turn — `combat_system`,
/// `reaper_system`, any trap or blast resolved upstream of them — has already
/// run, so the combo totalled here is complete and final, never applied to a
/// pile still growing.
pub fn score_turn_system(world: &mut World) {
    if let Some(mut flash) = world.get_resource_mut::<ScoreFlash>() {
        flash.turns = flash.turns.saturating_sub(1);
    }
    settle_kills(world);
}

/// Pays for the turn's dead and empties the pile. Also called by [`double`], so
/// a run that ends on the same turn as a kill is doubling a score that already
/// includes it.
fn settle_kills(world: &mut World) {
    let Some(mut combo) = world.get_resource_mut::<Combo>() else {
        return;
    };
    let (total, kills) = combo.total();
    *combo = Combo::default();
    if kills == 0 {
        return;
    }
    pay(world, total, kills);
}

/// Adds `points` to the player's score and lights the scorekeeper with what was
/// just earned. Silent in the log: whatever earned the points has already said
/// so in words, and this is the part that says it in colour.
pub fn award(world: &mut World, points: i32) {
    pay(world, points, 1);
}

/// Files one corpse worth `max_hp` under this turn's killing. Nothing is paid
/// here: the pile is settled once the turn's dying is done
/// ([`score_turn_system`]), because a multiplier on a number still growing is
/// not a multiplier anybody can read.
pub fn award_kill(world: &mut World, max_hp: i32) {
    let base = max_hp * KILL_PER_MAX_HP;
    let Some(mut combo) = world.get_resource_mut::<Combo>() else {
        return pay(world, base, 1);
    };
    combo.kills += 1;
    combo.base += base;
}

/// Hands over `points` and shouts about it. `kills` is how many corpses this
/// turn has produced, which is the only thing that decides whether the shout
/// says `COMBO!` first.
fn pay(world: &mut World, points: i32, kills: u32) {
    let Some(mut score) = player_score(world) else {
        return;
    };
    score.value = score.value.saturating_add(points as i64);

    let color = random_bright(world);
    let amount = format!("+{points}");
    if kills < 2 {
        let colors = vec![color; amount.chars().count()];
        return light(world, amount, colors);
    }
    let text = format!("{} {amount}", strings::combo_word());
    let mut colors = crate::pride::stripes(world).to_vec();
    colors.push(Color::DarkGrey);
    colors.extend(std::iter::repeat_n(color, amount.chars().count()));
    light(world, text, colors);
    announce_combo(world);
}

/// The line a combo gets in the log. Killing two things at once is the same
/// achievement the ring of adornment sells, so it gets the ring's own words, in
/// the ring's own magenta — except for the [`COMBO_PRIDE_CHANCE`] of the time
/// it gets better ones, and the flag to say them in (both colourings are
/// [`crate::hud::log_paint`]'s doing; this only picks the sentence).
///
/// The roll is off [`FxRng`], like everything else here: which of two sentences
/// the log prints is decoration, and decoration does not get to move the
/// gameplay dice.
fn announce_combo(world: &mut World) {
    let proud = world
        .get_resource_mut::<FxRng>()
        .is_some_and(|mut rng| rng.0.gen_bool(COMBO_PRIDE_CHANCE));
    let (line, category) = match proud {
        true => (crate::hud::pride_line(), LogCategory::Pride),
        false => (strings::with_style(), LogCategory::Combo),
    };
    // `GameLog` is required here, not optional: it is initialised before any
    // schedule step runs (`engine/src/main.rs`), and every other logging call
    // in the tree already assumes it. The `get_resource_mut` this replaced
    // protected nothing reachable and hid that assumption instead of stating it.
    world
        .resource_mut::<GameLog>()
        .add_colored(line.to_string(), category);
}

/// What a staircase is worth on a floor of difficulty `tier` (0-based, as
/// [`crate::map::difficulty_tier`] counts them — this is where it becomes the
/// 1-based tier the score is paid on).
pub fn award_stairs(world: &mut World, tier: u32) {
    award(world, STAIR_PER_TIER * (tier as i32 + 1));
}

/// Doubles the player's score, and reports what it came to. `None` if there is
/// no player to pay — the only case a caller has to think about is a test world.
///
/// The scorekeeper does not show a number for this one. Doubling is not an
/// amount, it is an event, and it gets the word.
pub fn double(world: &mut World) -> Option<i64> {
    // Anything that died this turn is paid for first, so a run that ends on the
    // same turn as a kill doubles a score that already counts it.
    settle_kills(world);
    let mut score = player_score(world)?;
    // Saturating, because nothing caps how many rings of adornment a run can
    // find and every one of them lands here. A wrapping double walks a score
    // — always a multiple of a hundred — onto exactly zero.
    score.value = score.value.saturating_mul(2);
    let doubled = score.value;
    let stripes = crate::pride::stripes(world).to_vec();
    light(world, "DOUBLE".to_string(), stripes);
    Some(doubled)
}

/// One of the bright colours, drawn from the same set the ring of adornment's
/// fireworks come in.
///
/// Off [`FxRng`], never the shared [`GameRng`](crate::map::GameRng): what
/// colour a number flashes in is pure decoration, and a decoration that
/// consumed the gameplay stream would mean every kill quietly reshuffled the
/// dice for everything after it. Falls back to white in a world with no
/// cosmetic stream (a bare test world).
fn random_bright(world: &mut World) -> Color {
    let Some(mut rng) = world.get_resource_mut::<FxRng>() else {
        return Color::White;
    };
    GLORY_COLORS[rng.0.gen_range(0..GLORY_COLORS.len())]
}

/// Lights the scorekeeper, if this world has one to light.
fn light(world: &mut World, text: String, colors: Vec<Color>) {
    if let Some(mut flash) = world.get_resource_mut::<ScoreFlash>() {
        flash.light(text, colors);
    }
}

/// The player's score, ready to be written to.
fn player_score(world: &mut World) -> Option<Mut<'_, Score>> {
    let player = world
        .query_filtered::<Entity, With<Player>>()
        .iter(world)
        .next()?;
    world.get_mut::<Score>(player)
}
