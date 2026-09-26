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
//!   counted in turns from the moment it lands (`crate::effects::tick_effects`
//!   ages it): being pinned is a stretch of time, not a state of the body.
//! * **Every verb reports whether it took hold.** A potion thrown at a monster
//!   only gives away what it was when something plainly happened (see
//!   `crate::items::throwing`), and that `bool` is the answer.

use bevy_ecs::prelude::*;
use rand::Rng;

use crate::components::{
    Fighter, GameLog, LogCategory, Mob, MovementType, Player, Position, Speed, SpeedKind, Viewshed,
};
use crate::constants::potions::PARALYSIS_LOST_TURN_CHANCE;
use crate::effects::{
    Asleep, Bided, Blind, Confused, Effects, Grant, Lifetime, MagicWard, Paralyzed, Petrified,
    Pinned, Rooted, SeesInvisible, Sluggish, Stealthy, SustainsStrength, clear_floor_grants,
};
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
///
/// `category` tags the player-directed line for [`crate::hud::log_paint`] —
/// decided by the caller, since it knows what caused the confusion (a dazzle
/// reads differently from a potion's swimming head) and this function doesn't.
pub fn confuse(
    world: &mut World,
    entity: Entity,
    player_line: &str,
    category: LogCategory,
    mob_verb: &str,
) -> bool {
    if world.get::<Player>(entity).is_some() {
        return confuse_player(world, entity, player_line, category);
    }
    stagger(world, entity, mob_verb)
}

fn confuse_player(world: &mut World, player: Entity, line: &str, category: LogCategory) -> bool {
    if world.get::<Confused>(player).is_some() {
        return false;
    }
    crate::effects::lend(world, player, Grant::of::<Confused>(), Lifetime::Floor);
    world
        .resource_mut::<GameLog>()
        .add_colored(line.to_string(), category);
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
        .add(strings::mob_verb_line(&name, mob_verb));
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
        return stagger(world, entity, strings::blind_mob_verb());
    }
    if world.get::<Blind>(entity).is_some() {
        return false;
    }
    crate::effects::lend(world, entity, Grant::of::<Blind>(), Lifetime::Floor);
    touch_viewshed(world, entity);
    world
        .resource_mut::<GameLog>()
        .add(strings::blind_player_line());
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
///
/// A monster the player can actually see also earns a line of its own, on top
/// of the generic slow-down [`set_speed`] already prints for it — the same
/// "visible only" rule [`report_cure`] holds a mending monster to.
pub fn paralyse(world: &mut World, entity: Entity) -> bool {
    if world.get::<Paralyzed>(entity).is_some() {
        return false;
    }
    crate::effects::lend(world, entity, Grant::of::<Paralyzed>(), Lifetime::Floor);
    let slowed = set_speed(world, entity, SpeedKind::Slow, false);
    if world.get::<Player>(entity).is_none() {
        let pos = world.get::<Position>(entity).copied();
        if pos.is_some_and(|p| crate::helpers::player_sees(world, p.x, p.y)) {
            let name = item_label(world, entity);
            world
                .resource_mut::<GameLog>()
                .add(strings::mob_verb_line(&name, strings::paralyzed_mob_verb()));
        }
        return slowed;
    }
    world
        .resource_mut::<GameLog>()
        .add(strings::paralyse_player_line());
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
        .add(strings::paralysis_lost_turn());
    true
}

// ---------------------------------------------------------------------------
// Snares: pinned, held, out cold
// ---------------------------------------------------------------------------

/// Holds a creature for `turns` more turns. Thin wrapper over
/// [`crate::effects::hold`], kept because "snare" is the word the traps, the
/// scrolls and the abilities all use for it.
pub fn snare(world: &mut World, victim: Entity, grant: Grant, turns: u32) -> bool {
    crate::effects::hold(world, victim, grant, turns)
}

// ---------------------------------------------------------------------------
// The afflictions, as one table
// ---------------------------------------------------------------------------

/// One affliction: the effect that *is* it, and the words for lifting it.
///
/// This exists because the same four conditions were written out three times
/// — once in [`afflicted`], once in [`cure_one_condition`] and once in
/// [`clear_player_conditions`] — in three different orders, with nothing
/// holding the three lists in agreement. A fifth condition needed three edits
/// and silently half-worked if it got two.
pub struct Affliction {
    /// The effect this condition is. Nothing here is a special component any
    /// more; it is an ordinary [`Grant`] out of `EFFECTS`.
    pub effect: Grant,
    /// The whole sentence the player reads when it is mended.
    pub cured_line: &'static str,
    /// Completes "The rat snaps out of ___." for anything that is not the
    /// player.
    pub cured_noun: &'static str,
    /// Completes "You are no longer ___." when a staircase takes it.
    pub lifted_adjective: &'static str,
    /// What else has to happen when it goes — a viewshed to recompute, a
    /// tempo to put back. Most conditions need nothing.
    pub after: Option<fn(&mut World, Entity)>,
}

/// Every affliction a cure can lift, **worst first**: blindness costs you the
/// floor, paralysis costs you turns, confusion costs you half your steps.
/// [`cure_one_condition`] takes the first one it finds, so the order here is
/// the order they hurt in.
///
/// Slowness is not a row and cannot be: a tempo is a value on [`Speed`], not a
/// marker something either has or has not, so it is handled on its own below.
pub const AFFLICTIONS: &[Affliction] = &[
    Affliction {
        effect: Grant::of::<Blind>(),
        cured_line: strings::blind_cured_line(),
        cured_noun: strings::blind_cured_noun(),
        lifted_adjective: strings::blind_lifted_adjective(),
        after: Some(touch_viewshed),
    },
    Affliction {
        effect: Grant::of::<Paralyzed>(),
        cured_line: strings::paralyzed_cured_line(),
        cured_noun: strings::paralyzed_cured_noun(),
        lifted_adjective: strings::paralyzed_lifted_adjective(),
        after: Some(restore_tempo),
    },
    Affliction {
        effect: Grant::of::<Confused>(),
        cured_line: strings::confused_cured_line(),
        cured_noun: strings::confused_cured_noun(),
        lifted_adjective: strings::confused_lifted_adjective(),
        after: None,
    },
];

/// Held until a staircase, but nothing a cure can lift — a ward is a boon, not
/// an affliction, and a rosé coin should not offer to take it off you.
const FLOOR_BOONS: &[(Grant, &str)] = &[(Grant::of::<MagicWard>(), strings::adjective_warded())];

// ---------------------------------------------------------------------------
// Lifting one of them, and mending what they left
// ---------------------------------------------------------------------------

/// Whether `entity` is carrying anything [`cure_one_condition`] could lift. The
/// question a rosé coin asks before it lets itself be picked up.
pub fn afflicted(world: &World, entity: Entity) -> bool {
    AFFLICTIONS.iter().any(|a| a.effect.probe(world, entity)) || slowed(world, entity)
}

/// Whether `entity` is dragging its feet. Not an [`AFFLICTIONS`] row because a
/// tempo is a value rather than a marker — see the note on the table.
fn slowed(world: &World, entity: Entity) -> bool {
    world
        .get::<Speed>(entity)
        .is_some_and(|s| s.kind == SpeedKind::Slow)
}

/// Lifts the single worst affliction `entity` is carrying and says so, or
/// returns `false` if there was nothing to lift. A ring of regeneration's first
/// call on every roll it wins.
///
/// The order is [`AFFLICTIONS`]' own — the order they hurt in. Holds are left
/// alone deliberately: a bear trap is steel around your ankle, not something
/// wrong with you, and it is already counting itself down.
pub fn cure_one_condition(world: &mut World, entity: Entity) -> bool {
    for affliction in AFFLICTIONS {
        if !affliction.effect.probe(world, entity) {
            continue;
        }
        crate::effects::revoke_matching(world, entity, |h| {
            Some(h.id) == affliction.effect.effect_id()
        });
        if let Some(after) = affliction.after {
            after(world, entity);
        }
        return report_cure(world, entity, affliction.cured_line, affliction.cured_noun);
    }
    if slowed(world, entity) {
        restore_tempo(world, entity);
        return report_cure(
            world,
            entity,
            strings::sluggish_cured_line(),
            strings::sluggish_cured_noun(),
        );
    }
    false
}

/// What became of an attempt to drain a creature's melee strength.
pub enum Drain {
    /// Something sustained it — a ring of strength. The caller says so in its
    /// own words; the rule is the same wherever it is asked.
    Resisted,
    /// It took.
    Took,
    /// There was nothing to take: no `Fighter`, or already at the floor.
    Nothing,
}

/// Drains `amount` of melee strength — a permanent hit to the attack die
/// itself, not a modifier — unless [`SustainsStrength`] turns it aside.
///
/// `floor` is the lowest the power can be driven to; `None` means no floor at
/// all, which is the rattlesnake's bite: a long enough fight drives a victim
/// negative.
///
/// The guard used to be written out three times — in the dart trap, in the
/// spell that lances the same dart at range, and in the snake's bite — each
/// with its own copy of "is this sustained, and is the player being told".
/// The rule is here; the prose stays with whoever is inflicting it, because a
/// trap and a snake do not sound alike.
pub fn drain_power(world: &mut World, victim: Entity, amount: i32, floor: Option<i32>) -> Drain {
    if world.get::<SustainsStrength>(victim).is_some() {
        return Drain::Resisted;
    }
    let Some(mut fighter) = world.get_mut::<Fighter>(victim) else {
        return Drain::Nothing;
    };
    let before = fighter.power;
    let after = fighter.power - amount;
    fighter.power = match floor {
        Some(low) => after.max(low),
        None => after,
    };
    match fighter.power == before {
        true => Drain::Nothing,
        false => Drain::Took,
    }
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
    report_cure(
        world,
        entity,
        strings::power_restored_line(),
        strings::power_restored_noun(),
    )
}

/// Puts `entity` back to [`SpeedKind::Normal`] without the ceremony
/// [`set_speed`] makes of it — the tempo half of lifting a condition, whose log
/// line belongs to the cure, not to the tempo.
fn restore_tempo(world: &mut World, entity: Entity) {
    if let Some(mut speed) = world.get_mut::<Speed>(entity) {
        speed.kind = SpeedKind::Normal;
    }
}

/// Logs `player_line` if the mended creature is the player; for a monster the
/// player can actually see, logs `"The {name} snaps out of {mob_noun}."`
/// instead — a griffin, a troll or a vampire mending itself off its own
/// [`Regenerates`](crate::effects::Regenerates) is the only way this branch is
/// reached today. Reports `true` either way: something was mended whether or
/// not anybody was told about it.
fn report_cure(world: &mut World, entity: Entity, player_line: &str, mob_noun: &str) -> bool {
    if world.get::<Player>(entity).is_some() {
        world.resource_mut::<GameLog>().add(player_line.to_string());
        return true;
    }
    let pos = world.get::<Position>(entity).copied();
    if pos.is_some_and(|p| crate::helpers::player_sees(world, p.x, p.y)) {
        let name = item_label(world, entity);
        world
            .resource_mut::<GameLog>()
            .add(strings::snaps_out_of(&name, mob_noun));
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
    let (line, category) = speed_shift_message(&name, faster, is_player, changed);
    world.resource_mut::<GameLog>().add_colored(line, category);
    changed
}

/// The line a speed change prints, and the category it earns — split out so
/// the message can early-return its way through the cases instead of
/// threading one `if`/`else` chain. Only a *player's own* tempo shifting gets
/// coloured (a monster's speed is nobody's HUD moment); which colour depends
/// on the direction, not on whether it actually changed — the "already as
/// quick/sluggish as you can be" refusal reads as the same kind of news.
fn speed_shift_message(
    name: &str,
    faster: bool,
    is_player: bool,
    changed: bool,
) -> (String, LogCategory) {
    let extreme = match faster {
        true => strings::extreme_quick(),
        false => strings::extreme_sluggish(),
    };
    let category = match is_player {
        true if faster => LogCategory::Haste,
        true => LogCategory::Slowed,
        false => LogCategory::Plain,
    };
    let text = if !changed && is_player {
        strings::already_as_extreme_player(extreme)
    } else if !changed {
        strings::already_as_extreme_mob(name, extreme)
    } else {
        match (is_player, faster) {
            (true, true) => strings::haste_player_line().to_string(),
            (true, false) => strings::slow_player_line().to_string(),
            (false, true) => strings::haste_mob_line(name),
            (false, false) => strings::slow_mob_line(name),
        }
    };
    (text, category)
}

// ---------------------------------------------------------------------------
// Lifting them again
// ---------------------------------------------------------------------------
/// Everything a staircase takes off the player, and one line for each.
///
/// The conditions are held for [`Lifetime::Floor`], so lifting them is
/// [`clear_floor_grants`] and nothing else — the same machinery a potion of
/// see invisible has always used. All this adds is the sentence, worked out by
/// asking what the player held before and what they hold after.
pub fn clear_player_conditions(world: &mut World, player: Entity) {
    let mut lifted: Vec<&str> = Vec::new();

    // Tempo first, and by hand: it is a value on `Speed` rather than an effect
    // something either has or has not.
    //
    // Back to the body's own tempo: `Normal` for nihil, `Quick` for a lurk,
    // and whatever the bestiary row says for a player wearing a species
    // (`-am wraith` is born fast). A staircase lifts what the *floor* lent;
    // it has no business lifting what the player is made of.
    let innate = crate::body::innate_tempo(world, player);
    if let Some(mut speed) = world.get_mut::<Speed>(player) {
        let was = speed.kind;
        speed.kind = innate;
        // Named against the body's own tempo rather than against `Normal`: a
        // lurk walked up the stairs at `Normal` was *slowed*, even though a
        // human at `Normal` is nothing of the kind.
        match was.rate().cmp(&innate.rate()) {
            std::cmp::Ordering::Greater => lifted.push("hasted"),
            std::cmp::Ordering::Less => lifted.push("slowed"),
            std::cmp::Ordering::Equal => {}
        }
    }

    // What the floor lent, named before it goes so the difference can be read
    // afterwards. Asked either side rather than simply listed, so a *ring* of
    // perception still worn keeps the sight and prints nothing.
    let named: Vec<(Grant, &'static str)> = AFFLICTIONS
        .iter()
        .map(|a| (a.effect, a.lifted_adjective))
        .chain(FLOOR_BOONS.iter().copied())
        .chain(std::iter::once((
            Grant::of::<SeesInvisible>(),
            strings::adjective_sees_unseen(),
        )))
        .collect();
    let before: Vec<bool> = named.iter().map(|(g, _)| g.probe(world, player)).collect();

    // The afflictions and the ward are floor-scoped *by definition*, so lift
    // them whether or not a `Lifetime::Floor` entry stands behind each one. A
    // condition attached with a bare `insert` — by a mechanic that forgot
    // `lend`, or by a test — has no ledger entry and therefore no lifetime,
    // and would otherwise quietly become permanent.
    //
    // Never take one some *other* source is still lending, though: that is the
    // whole reason the ledger records who lent what.
    {
        let lent_beyond_the_floor: Vec<&'static str> = world
            .get::<Effects>(player)
            .map(|l| {
                l.0.iter()
                    .filter(|h| h.lifetime != Lifetime::Floor)
                    .map(|h| h.id)
                    .collect()
            })
            .unwrap_or_default();
        let mut e = world.entity_mut(player);
        for grant in AFFLICTIONS
            .iter()
            .map(|a| a.effect)
            .chain(FLOOR_BOONS.iter().map(|(g, _)| *g))
        {
            let borrowed = grant
                .effect_id()
                .is_some_and(|id| lent_beyond_the_floor.contains(&id));
            if borrowed {
                continue;
            }
            grant.detach(&mut e);
        }
    }

    // Everything else the floor lent — a potion of see invisible — goes
    // through the ledger, which is the only record of what was on loan. A ring
    // of perception still worn is a `WhileEquipped` entry, so the sight stays.
    clear_floor_grants(world, player);

    for ((grant, adjective), had) in named.iter().zip(before) {
        if had && !grant.probe(world, player) {
            lifted.push(adjective);
        }
    }

    if !lifted.is_empty() {
        touch_viewshed(world, player);
    }
    for cond in lifted {
        world
            .resource_mut::<GameLog>()
            .add(strings::no_longer(cond));
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

/// Everything that is a condition without being an affliction, a hold or a
/// floor boon: the rest of what the HUD badges, each with the adjective that
/// completes "You are no longer ___." A ward is already a [`FLOOR_BOONS`] row;
/// these four have no list of their own because nothing else ever had to ask
/// about them together.
const OTHER_CONDITIONS: &[(Grant, &str)] = &[
    (Grant::of::<Bided>(), strings::adjective_coiled()),
    (Grant::of::<Petrified>(), strings::adjective_stone()),
    (Grant::of::<Stealthy>(), strings::adjective_stealthy()),
    (Grant::of::<Sluggish>(), strings::adjective_sluggish()),
];

/// Every condition in the game, each with its "no longer ___" adjective: the
/// set the HUD puts a badge on, and the set [`crate::effects::CONDITION_CAP`]
/// counts.
///
/// Composed from the lists that already declare conditions rather than
/// restated as a fourth one, so a new condition joins the ceiling by being
/// added where it belongs.
fn conditions() -> impl Iterator<Item = (Grant, &'static str)> {
    AFFLICTIONS
        .iter()
        .map(|a| (a.effect, a.lifted_adjective))
        .chain(FLOOR_BOONS.iter().copied())
        .chain(HOLD_ADJECTIVES.iter().copied())
        .chain(OTHER_CONDITIONS.iter().copied())
}

/// The three [`crate::effects::HOLDS`], with the adjective each would need if
/// it ever had to be named in a sentence it has no line of its own for. It
/// never has yet — every hold in [`crate::effects::EFFECTS`] carries an
/// `ends` line, and [`shed_line`] prefers that — so this is the fallback that
/// keeps a hold from being the one condition that could go in silence.
const HOLD_ADJECTIVES: &[(Grant, &str)] = &[
    (Grant::of::<Asleep>(), strings::adjective_asleep()),
    (Grant::of::<Pinned>(), strings::adjective_pinned()),
    (Grant::of::<Rooted>(), strings::adjective_held()),
];

/// Whether the effect `id` is something the creature is *under* — the set the
/// HUD puts a badge on, and the set [`crate::effects::CONDITION_CAP`] counts.
///
/// The default is the safe way round: an effect [`conditions`] does not name
/// is not a condition, so a ring's lent boon or the mark a potion of magic
/// detection leaves on a monster can never shoulder a real condition off.
pub fn is_condition(id: &str) -> bool {
    conditions().any(|(g, _)| g.effect_id() == Some(id))
}

/// The sentence the player reads when the effect `id` is shed to make room for
/// a fourth condition, or `None` for anything that is not a condition at all.
///
/// A hold says what it says when its own clock runs out — the words are
/// already written and they fit either ending. Everything else borrows the
/// staircase's phrasing, because "you are no longer blind" is the sentence the
/// player has already learned to read as "that one is over".
pub fn shed_line(id: &str) -> Option<String> {
    let (_, adjective) = conditions().find(|(g, _)| g.effect_id() == Some(id))?;
    let ends = crate::effects::Effect::by_id(id).and_then(|e| e.ends);
    Some(match ends {
        Some(line) => line.to_string(),
        None => strings::no_longer(adjective),
    })
}

/// What still has to happen when the effect `id` comes off a creature by some
/// route other than a cure — [`crate::effects::shed_oldest_condition`] making
/// room for a fourth condition.
///
/// The same `after` column [`cure_one_condition`] runs, read off the same
/// table, so a condition that needs a viewshed recomputed needs it whichever
/// way it left.
pub fn after_lifted(world: &mut World, entity: Entity, id: &str) {
    let after = AFFLICTIONS
        .iter()
        .find(|a| a.effect.effect_id() == Some(id))
        .and_then(|a| a.after);
    if let Some(after) = after {
        after(world, entity);
    }
}
