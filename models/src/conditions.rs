//! What a creature is *afflicted with* — and the one place an affliction is
//! lifted again.
//!
//! A condition is a component ([`Confused`], [`Blind`], [`Paralyzed`], a
//! [`Snare`], a shifted
//! [`Speed`]); this module is the verbs that put one on, take one off, and print
//! the line the player reads when either happens. It exists because the same
//! affliction arrives from several directions — a wand of light dazzles, a
//! potion of confusion confuses, a potion of paralysis locks your limbs the way
//! a wand of slow monster does — and none of those mechanics should each own
//! their own copy of "is this the player or a monster, and what does that mean".
//!
//! Three rules hold for every condition here:
//!
//! * **The player and a monster take it differently.** The player carries a
//!   marker component the input loop reads; a monster has no viewshed to put out
//!   and no keyboard to ignore, so it gets [`MovementType::Confused`] or a slower
//!   tempo instead. Each verb below does that split once, so its callers never
//!   have to.
//! * **Nothing wears off with time.** A player condition rides along until a
//!   staircase or a wand of cancellation clears it
//!   ([`clear_player_conditions`]) — that is the bargain that makes drinking an
//!   unidentified potion frightening. The one exception is a [`Snare`], which is
//!   counted in turns from the moment it lands (`crate::traps::snare_system`
//!   ages it): being pinned is a stretch of time, not a state of the body.
//! * **Every verb reports whether it took hold.** A potion thrown at a monster
//!   only gives away what it was when something plainly happened (see
//!   `crate::items::throwing`), and that `bool` is the answer.

use bevy_ecs::prelude::*;
use rand::Rng;

use crate::components::{
    Blind, Confused, Fighter, GameLog, Mob, MovementType, Paralyzed, Player, Snare, SnareKind,
    Speed, SpeedKind, Viewshed,
};
use crate::constants::potions::PARALYSIS_LOST_TURN_CHANCE;
use crate::effects::{SeesInvisible, Sluggish, clear_floor_grants};
use crate::helpers::item_label;
use crate::map::GameRng;

// ---------------------------------------------------------------------------
// Confusion
// ---------------------------------------------------------------------------

/// Lands confusion on one creature, whatever confused it. The player picks up
/// the [`Confused`] condition (half of every step and swing goes off in a random
/// direction); a monster is switched to a random walk. `player_line` is the
/// whole sentence the player reads, `mob_verb` completes "The rat ___." for
/// anything else — so a wand's flash and a potion's swimming head read
/// differently while meaning the same thing.
///
/// Returns whether it took hold: false for a creature already confused, and for
/// anything that is neither the player nor a monster.
pub fn confuse(world: &mut World, entity: Entity, player_line: &str, mob_verb: &str) -> bool {
    if world.get::<Player>(entity).is_some() {
        return confuse_player(world, entity, player_line);
    }
    stagger(world, entity, mob_verb)
}

fn confuse_player(world: &mut World, player: Entity, line: &str) -> bool {
    if world.get::<Confused>(player).is_some() {
        return false;
    }
    world.entity_mut(player).insert(Confused);
    world.resource_mut::<GameLog>().add(line.to_string());
    true
}

/// Sets a monster reeling: [`MovementType::Confused`], announced as
/// "The rat `mob_verb`." A no-op on anything that isn't a [`Mob`].
pub fn stagger(world: &mut World, entity: Entity, mob_verb: &str) -> bool {
    if world.get::<Mob>(entity).is_none() {
        return false;
    }
    let name = item_label(world, entity);
    if let Some(mut mob) = world.get_mut::<Mob>(entity) {
        mob.movement_type = MovementType::Confused;
    }
    world
        .resource_mut::<GameLog>()
        .add(format!("The {name} {mob_verb}."));
    true
}

// ---------------------------------------------------------------------------
// Blindness
// ---------------------------------------------------------------------------

/// Puts out the player's eyes: while [`Blind`] their view collapses to the 3x3
/// they could reach out and touch, so the map stops filling in, monsters stop
/// being spotted, and everything they already knew stays on screen as cold
/// memory (see [`crate::visibility`]).
///
/// A monster has no viewshed to put out, so blindness reads on it the way a
/// dazzle does — it gropes about at random.
pub fn blind(world: &mut World, entity: Entity) -> bool {
    if world.get::<Player>(entity).is_none() {
        return stagger(world, entity, "gropes about, blinded");
    }
    if world.get::<Blind>(entity).is_some() {
        return false;
    }
    world.entity_mut(entity).insert(Blind);
    touch_viewshed(world, entity);
    world
        .resource_mut::<GameLog>()
        .add("A darkness closes over your eyes. You can't see a thing!".to_string());
    true
}

// ---------------------------------------------------------------------------
// Paralysis
// ---------------------------------------------------------------------------

/// Locks a creature's limbs: [`Paralyzed`] and slowed, both. The player then
/// forfeits a share of the turns the slowing still grants them
/// ([`paralysis_forfeits_turn`]); a monster keeps the slowing alone — the same
/// bargain without the coin flip — and wears the tag so the renderer can tint it
/// as something that can't fight back properly.
pub fn paralyse(world: &mut World, entity: Entity) -> bool {
    if world.get::<Paralyzed>(entity).is_some() {
        return false;
    }
    world.entity_mut(entity).insert(Paralyzed);
    let slowed = set_speed(world, entity, SpeedKind::Slow, false);
    if world.get::<Player>(entity).is_none() {
        return slowed;
    }
    world
        .resource_mut::<GameLog>()
        .add("Your limbs seize up. You can barely move!".to_string());
    true
}

/// Whether paralysis eats this turn whole. Rolled **once per turn** by the input
/// handler before a key is read, which is what makes the lost turn a turn rather
/// than a dropped keystroke: the monsters get their move and the player gets
/// nothing.
pub fn paralysis_forfeits_turn(world: &mut World) -> bool {
    let paralysed = world
        .query_filtered::<(), (With<Player>, With<Paralyzed>)>()
        .iter(world)
        .next()
        .is_some();
    if !paralysed {
        return false;
    }
    let lost = world
        .resource_mut::<GameRng>()
        .0
        .gen_bool(PARALYSIS_LOST_TURN_CHANCE);
    if !lost {
        return false;
    }
    world
        .resource_mut::<GameLog>()
        .add("Your body will not answer you.".to_string());
    true
}

// ---------------------------------------------------------------------------
// Snares: pinned, held, out cold
// ---------------------------------------------------------------------------

/// Pins a creature for `turns` more turns — the steel jaws of a bear trap, a
/// lungful of sleeping gas, the words of a scroll of hold monster. What each
/// kind costs its victim is [`SnareKind`]'s business and `crate::ai`'s; all this
/// does is put the tag on.
///
/// Deliberately silent: a snare arrives from a trap, a scroll or a cloud of gas,
/// and the sentence the player reads belongs to whichever it was. Returns
/// whether it changed anything — a creature already pinned for at least this
/// long by the same thing is left alone rather than having its sentence
/// shortened.
pub fn snare(world: &mut World, victim: Entity, kind: SnareKind, turns: u32) -> bool {
    if turns == 0 {
        return false;
    }
    let standing = world
        .get::<Snare>(victim)
        .filter(|s| s.kind == kind)
        .map_or(0, |s| s.turns);
    if standing >= turns {
        return false;
    }
    world.entity_mut(victim).insert(Snare { turns, kind });
    true
}

// ---------------------------------------------------------------------------
// Lifting one of them, and mending what they left
// ---------------------------------------------------------------------------

/// Whether `entity` is carrying anything [`cure_one_condition`] could lift. The
/// question a rosé coin asks before it lets itself be picked up.
pub fn afflicted(world: &World, entity: Entity) -> bool {
    world.get::<Blind>(entity).is_some()
        || world.get::<Paralyzed>(entity).is_some()
        || world.get::<Confused>(entity).is_some()
        || world
            .get::<Speed>(entity)
            .is_some_and(|s| s.kind == SpeedKind::Slow)
}

/// Lifts the single worst affliction `entity` is carrying and says so, or
/// returns `false` if there was nothing to lift. A ring of regeneration's first
/// call on every roll it wins.
///
/// The order is the order they hurt in: blindness costs you the floor,
/// paralysis costs you turns, confusion costs you half your steps, being slowed
/// costs you the difference. Snares are left alone deliberately — a bear trap is
/// steel around your ankle, not something wrong with you, and it is already
/// counting itself down.
pub fn cure_one_condition(world: &mut World, entity: Entity) -> bool {
    if world.get::<Blind>(entity).is_some() {
        world.entity_mut(entity).remove::<Blind>();
        touch_viewshed(world, entity);
        return report_cure(world, entity, "The darkness lifts from your eyes.");
    }
    if world.get::<Paralyzed>(entity).is_some() {
        world.entity_mut(entity).remove::<Paralyzed>();
        restore_tempo(world, entity);
        return report_cure(world, entity, "Your limbs are your own again.");
    }
    if world.get::<Confused>(entity).is_some() {
        world.entity_mut(entity).remove::<Confused>();
        return report_cure(world, entity, "Your head clears.");
    }
    if world
        .get::<Speed>(entity)
        .is_some_and(|s| s.kind == SpeedKind::Slow)
    {
        restore_tempo(world, entity);
        return report_cure(world, entity, "The lead goes out of your legs.");
    }
    false
}

/// Gives `entity` back one point of the melee strength a poisoned dart drank,
/// up to [`Fighter::max_power`] and never past it. Returns whether there was
/// any to give back — an unpoisoned arm is what tells a ring of regeneration
/// there is nothing left to do.
pub fn restore_one_power(world: &mut World, entity: Entity) -> bool {
    let Some(mut fighter) = world.get_mut::<Fighter>(entity) else {
        return false;
    };
    if fighter.power >= fighter.max_power {
        return false;
    }
    fighter.power += 1;
    report_cure(world, entity, "Strength trickles back into your arm.")
}

/// Puts `entity` back to [`SpeedKind::Normal`] without the ceremony
/// [`set_speed`] makes of it — the tempo half of lifting a condition, whose log
/// line belongs to the cure, not to the tempo.
fn restore_tempo(world: &mut World, entity: Entity) {
    if let Some(mut speed) = world.get_mut::<Speed>(entity) {
        speed.kind = SpeedKind::Normal;
    }
}

/// Logs `player_line` if the mended creature is the player, and reports `true`
/// either way — something was mended whether or not anybody was told about it.
fn report_cure(world: &mut World, entity: Entity, player_line: &str) -> bool {
    if world.get::<Player>(entity).is_some() {
        world.resource_mut::<GameLog>().add(player_line.to_string());
    }
    true
}

// ---------------------------------------------------------------------------
// Tempo
// ---------------------------------------------------------------------------

/// The tempo `entity` actually acts at: its own [`Speed`], dropped one notch if
/// it is wearing something that weighs it down ([`Sluggish`] — a ring of slow
/// digestion).
///
/// Everything that reads a tempo reads it through here rather than off the
/// component, which is what lets a ring slow you without touching the condition
/// a potion of haste set — take the ring off and the haste is still there,
/// exactly as it was.
pub fn tempo(world: &World, entity: Entity) -> SpeedKind {
    let base = world
        .get::<Speed>(entity)
        .map_or(SpeedKind::Normal, |s| s.kind);
    match world.get::<Sluggish>(entity).is_some() {
        true => base.slower(),
        false => base,
    }
}

/// Wand of haste / slow monster: step one creature — monster or player — one
/// notch along the speed scale. Permanent for a monster; a hasted or slowed
/// *player* loses it on the next staircase ([`clear_player_conditions`]).
pub fn shift_entity_speed(world: &mut World, victim: Entity, faster: bool) {
    let Some(speed) = world.get::<Speed>(victim) else {
        return;
    };
    let target = match faster {
        true => speed.kind.faster(),
        false => speed.kind.slower(),
    };
    set_speed(world, victim, target, faster);
}

/// Straight to [`SpeedKind::Fast`], skipping the notches — a potion of haste
/// self. Returns whether the tempo actually moved.
pub fn hasten(world: &mut World, victim: Entity) -> bool {
    set_speed(world, victim, SpeedKind::Fast, true)
}

/// Puts `victim` at `kind` and logs it. `faster` is the *intent*, not the
/// outcome — it picks which flavour of "already as fast/slow as it gets" a
/// no-op prints. Returns whether the tempo changed.
fn set_speed(world: &mut World, victim: Entity, kind: SpeedKind, faster: bool) -> bool {
    let is_player = world.get::<Player>(victim).is_some();
    let name = item_label(world, victim);
    let Some(mut speed) = world.get_mut::<Speed>(victim) else {
        return false;
    };
    let before = speed.kind;
    speed.kind = kind;
    let changed = kind != before;
    world
        .resource_mut::<GameLog>()
        .add(speed_shift_message(&name, faster, is_player, changed));
    changed
}

/// The line a speed change prints, split out so it can early-return its way
/// through the cases instead of threading one `if`/`else` chain.
fn speed_shift_message(name: &str, faster: bool, is_player: bool, changed: bool) -> String {
    let extreme = match faster {
        true => "quick",
        false => "sluggish",
    };
    if !changed && is_player {
        return format!("You are already as {extreme} as you can be.");
    }
    if !changed {
        return format!("The {name} is already as {extreme} as it can be.");
    }
    match (is_player, faster) {
        (true, true) => "The world lurches into slow motion around you.".to_string(),
        (true, false) => "Your limbs turn to lead.".to_string(),
        (false, true) => format!("The {name} blurs into sudden speed."),
        (false, false) => format!("The {name} lurches into slow motion."),
    }
}

// ---------------------------------------------------------------------------
// Lifting them again
// ---------------------------------------------------------------------------

/// Clears every transient condition the player is carrying — [`Speed`]
/// haste/slow, [`Confused`], [`Blind`], [`Paralyzed`], and whatever a potion
/// lent them for the floor ([`crate::effects::GrantedForFloor`]) — and logs each
/// one it lifts. Only two things trigger it: taking a staircase
/// ([`crate::map::transition_level`]) and being caught by a wand of
/// cancellation.
pub fn clear_player_conditions(world: &mut World, player: Entity) {
    let mut lifted: Vec<&str> = Vec::new();
    if let Some(mut speed) = world.get_mut::<Speed>(player) {
        match speed.kind {
            SpeedKind::Fast => {
                speed.kind = SpeedKind::Normal;
                lifted.push("hasted");
            }
            SpeedKind::Slow => {
                speed.kind = SpeedKind::Normal;
                lifted.push("slowed");
            }
            SpeedKind::Normal => {}
        }
    }
    if world.get::<Confused>(player).is_some() {
        world.entity_mut(player).remove::<Confused>();
        lifted.push("confused");
    }
    if world.get::<Blind>(player).is_some() {
        world.entity_mut(player).remove::<Blind>();
        lifted.push("blind");
    }
    if world.get::<Paralyzed>(player).is_some() {
        world.entity_mut(player).remove::<Paralyzed>();
        lifted.push("paralysed");
    }

    // A potion of see invisible only lasts the floor. Asked before and after so
    // a *ring* of perception still worn keeps the sight and prints nothing.
    let saw_invisible = world.get::<SeesInvisible>(player).is_some();
    clear_floor_grants(world, player);
    if saw_invisible && world.get::<SeesInvisible>(player).is_none() {
        lifted.push("able to see the unseen");
    }

    if !lifted.is_empty() {
        touch_viewshed(world, player);
    }
    for cond in lifted {
        world
            .resource_mut::<GameLog>()
            .add(format!("You are no longer {cond}."));
    }
}

/// Marks `entity`'s viewshed dirty so the visibility system recomputes it before
/// the next frame. A condition that changes what the player can *see* — going
/// blind, gaining or losing second sight — has to land this turn rather than
/// waiting for them to take a step.
fn touch_viewshed(world: &mut World, entity: Entity) {
    if let Some(mut vs) = world.get_mut::<Viewshed>(entity) {
        vs.dirty = true;
    }
}
