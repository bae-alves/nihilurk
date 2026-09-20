//! The three rings that *do* something rather than *are* something.
//!
//! Most of the catalog's rings need no code at all: a number on the row becomes
//! a [`Modifier`](crate::effects::Modifier) component, a `.grants(...)` becomes a
//! marker every other system already asks about, and nothing here is involved.
//! What lives in this file is the handful of rings whose effect is a *verb* —
//! something that happens at a moment rather than a property held while worn:
//!
//! * **Adornment** — [`wear_adornment`], a one-shot fired the instant it goes on
//!   ([`OnWear`]), which spends the ring.
//! * **Regeneration** — [`regenerate`], rolled each turn by
//!   [`crate::abilities`].
//! * **Teleportation** — [`teleportitis`], likewise.
//!
//! Even these are named by the catalog row and dispatched through a general
//! mechanism; no file outside this one matches on a [`RingEffect`] to decide
//! what happens.
//!
//! [`RingEffect`]: crate::components::RingEffect

use bevy_ecs::prelude::*;
use crossterm::style::Color;
use rand::Rng;

use crate::components::{Consume, GameLog, Magic, Player, Position};
use crate::effects::{OnWear, Teleportitis};
use crate::helpers::item_label;
use crate::map::FxRng;
use crate::particles::{BlastPalette, Particles, on_map};
use crate::shake::{ShakeKind, kick_shake};

// --- Tuning constants ------------------------------------------------------
// Defined and documented in `constants.rs`.
//
//   TELEPORT_MAGIC_COST  what one deliberate `T` jump costs the wearer
use crate::constants::rings::TELEPORT_MAGIC_COST;

/// The sixteen tiles a ring of adornment throws a firework onto: the eight
/// within arm's reach, clockwise from due north, then Frost Nova's own star
/// points a few tiles further out, clockwise again. Twice the fireworks, so
/// twice the chain to sit through — the flourish is the one thing in the game
/// allowed to take its time.
const AROUND: [(i32, i32); 16] = [
    (0, -1),
    (1, -1),
    (1, 0),
    (1, 1),
    (0, 1),
    (-1, 1),
    (-1, 0),
    (-1, -1),
    (0, -3),
    (2, -2),
    (3, 0),
    (2, 2),
    (0, 3),
    (-2, 2),
    (-3, 0),
    (-2, -2),
];

/// The three colours a flourish comes in. Not the full bright set the score
/// numbers draw from ([`GLORY_COLORS`](crate::particles::GLORY_COLORS)): a
/// ring of adornment has a palette, and it is magenta, cyan and yellow.
const GLAM_COLORS: [Color; 3] = [Color::Magenta, Color::Cyan, Color::Yellow];

/// Milliseconds between one firework and the next.
const FIREWORK_STAGGER_MS: f32 = 90.0;

/// The lines the flourish reads out, in order. Deliberately more words than
/// anything else in the game gives one event: this is the only moment in a run
/// that exists purely to be looked at.
const FANFARE: [&str; 4] = [
    "Magenta, cyan and gold pour off you all at once.",
    "The dungeon, briefly, is a ballroom.",
    "And you do it with style!",
    "Your score is doubled!",
];

// ---------------------------------------------------------------------------
// Adornment
// ---------------------------------------------------------------------------

/// Doing it with style: the whole flourish, score and fireworks and fanfare.
///
/// Two things earn it and they are the same thing twice — putting on a ring of
/// adornment, and walking out of the dungeon with the Element of Yoord. A run
/// is not scored on what it killed; it is scored on how it left.
///
/// Everything here is played from the player's own tile, so it works equally
/// well as the last thing a run does (the victory climb queues it, and the
/// engine plays it out before the WIN panel) and as a thing that happens in the
/// middle of a floor.
pub(crate) fn do_it_with_style(world: &mut World) {
    let Some(at) = player_tile(world) else {
        return;
    };
    fireworks(world, at);
    kick_shake(world, ShakeKind::Heavy);
    crate::score::double(world);
    let mut log = world.resource_mut::<GameLog>();
    for line in FANFARE {
        log.add(line.to_string());
    }
}

/// Sixteen blasts — every tile within arm's reach, then a star of them further
/// out — each in one of [`GLAM_COLORS`] and each a beat behind the last, over a
/// [`BlastPalette::Glam`] burst on the wearer's own tile, the way Frost Nova
/// layers its star. The colour of each is drawn at random, so no two flourishes
/// look alike.
fn fireworks(world: &mut World, at: Position) {
    // Off `FxRng`, the cosmetic stream: which colour each firework comes in is
    // decoration, and decoration never moves the gameplay dice.
    let colors: Vec<Color> = {
        let Some(mut rng) = world.get_resource_mut::<FxRng>() else {
            return;
        };
        (0..AROUND.len())
            .map(|_| GLAM_COLORS[rng.0.gen_range(0..GLAM_COLORS.len())])
            .collect()
    };
    let Some(mut fx) = world.get_resource_mut::<Particles>() else {
        return;
    };
    for (i, &(dx, dy)) in AROUND.iter().enumerate() {
        let Some((x, y)) = on_map(at.x as i32 + dx, at.y as i32 + dy) else {
            continue;
        };
        fx.firework(x, y, colors[i], i as f32 * FIREWORK_STAGGER_MS);
    }
    fx.explosion(&[(at.x, at.y, 0.0)], BlastPalette::Glam);
}

/// Ring of adornment, the moment it goes on ([`OnWear`]): the flourish fires,
/// and the ring — having said everything it had to say — goes with it.
///
/// It is the one piece of gear in the dungeon that is worth exactly one action.
/// A monster that puts one on gets nothing: there is no score to double, so the
/// ring simply stays a ring.
pub(crate) fn wear_adornment(world: &mut World, wearer: Entity, item: Entity) {
    if world.get::<Player>(wearer).is_none() {
        return;
    }
    do_it_with_style(world);
    let name = item_label(world, item);
    world
        .resource_mut::<GameLog>()
        .add(format!("The {name} has nothing left to give."));
    // The pack is not this module's to reach into — tagging the ring is how
    // anything in the game asks to be spent, and `crate::items` does the rest.
    world.entity_mut(item).insert(Consume);
}

/// The handle the catalog row names, so `RINGS` can chain `.on_wear(ADORNMENT)`
/// without a function pointer's worth of punctuation on the table.
pub(crate) const ADORNMENT: OnWear = OnWear(wear_adornment);

// ---------------------------------------------------------------------------
// Regeneration
// ---------------------------------------------------------------------------

/// Ring of regeneration, one roll's worth: whatever is wrong with the bearer,
/// a little of it stops being wrong. An affliction goes first — being blind
/// costs more than being weak — and only once there is nothing left to cure does
/// the ring start giving back the strength a poisoned dart took.
///
/// Returns whether it found anything to mend; a bearer in perfect health rolls
/// this every other turn and must do it silently.
pub(crate) fn regenerate(world: &mut World, bearer: Entity) -> bool {
    if crate::conditions::cure_one_condition(world, bearer) {
        return true;
    }
    crate::conditions::restore_one_power(world, bearer)
}

// ---------------------------------------------------------------------------
// Teleportation
// ---------------------------------------------------------------------------

/// Ring of teleportation, one roll's worth: the bearer is somewhere else.
///
/// Rolled at the tail of the turn schedule, which is the top of the bearer's
/// next turn — so the player lands, sees where they landed, and acts from there
/// before anything on the floor has moved. That free action is the whole
/// difference between a ring you might keep on and a curse.
pub(crate) fn teleportitis(world: &mut World, bearer: Entity) -> bool {
    super::wands::teleport_entity_away(world, bearer);
    true
}

/// `T`: the jump on purpose, for [`TELEPORT_MAGIC_COST`] points of magic.
///
/// The ring gives you teleportitis whether you like it or not; what it *also*
/// gives you, if you work it out, is a handle on it. Returns whether a turn was
/// spent.
///
/// Nothing is logged when it refuses — no "you can't do that", no hint that the
/// key did anything at all. A player who has never worn the ring must never
/// learn there is a key here by pressing it, and a player wearing it with an
/// empty pool has already been told what an empty pool means. It is the one
/// secret in the game, and a refusal that talks is not a secret.
pub fn willed_teleport(world: &mut World) -> bool {
    let Some(player) = player_entity(world) else {
        return false;
    };
    if world.get::<Teleportitis>(player).is_none() {
        return false;
    }
    let Some(magic) = world.get::<Magic>(player).copied() else {
        return false;
    };
    if magic.points < TELEPORT_MAGIC_COST {
        return false;
    }
    if let Some(mut magic) = world.get_mut::<Magic>(player) {
        magic.points -= TELEPORT_MAGIC_COST;
    }
    super::wands::teleport_entity_away(world, player);
    true
}

// ---------------------------------------------------------------------------

/// Where the player is standing.
fn player_tile(world: &mut World) -> Option<Position> {
    let mut q = world.query_filtered::<&Position, With<Player>>();
    q.iter(world).next().copied()
}

/// The player, if there is one.
fn player_entity(world: &mut World) -> Option<Entity> {
    world
        .query_filtered::<Entity, With<Player>>()
        .iter(world)
        .next()
}
