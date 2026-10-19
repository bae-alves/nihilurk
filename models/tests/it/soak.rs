//! A seeded bot plays through the real turn schedule: it walks, fights, uses,
//! throws, casts, drops and takes the stairs down. After every turn the world
//! must still satisfy what each system assumes of the one before it. The
//! bot's choices come from its own RNG, never the run's [`GameRng`].
//!
//! Those assumptions are written as prose beside each system. Prose does not
//! fail when a despawn leaves a pack pointing at nothing, or a step forgets to
//! drain its queue; this does.

use std::collections::HashSet;

use bevy_ecs::prelude::*;
use models::*;
use rand::Rng;

use crate::common;

use common::fresh_run as new_run;

fn broken_invariant(w: &mut World) -> Option<String> {
    let dead = w.resource::<Ending>().player_dead;

    if !w.resource::<AttackQueue>().attacks.is_empty() {
        return Some("AttackQueue not drained".into());
    }
    if !w.resource::<UseQueue>().uses.is_empty() {
        return Some("UseQueue not drained".into());
    }
    if !w.resource::<ThrowQueue>().throws.is_empty() {
        return Some("ThrowQueue not drained".into());
    }
    if !w.resource::<SpellQueue>().spells.is_empty() {
        return Some("SpellQueue not drained".into());
    }

    let moved = w
        .query_filtered::<Entity, With<EntityMoved>>()
        .iter(w)
        .count();
    if moved > 0 {
        return Some(format!("{moved} entities still carry EntityMoved"));
    }

    if !dead {
        let walking_dead: Vec<Entity> = w
            .query_filtered::<(Entity, &Fighter), Without<Player>>()
            .iter(w)
            .filter(|(_, f)| f.hp <= 0)
            .map(|(e, _)| e)
            .collect();
        if !walking_dead.is_empty() {
            return Some(format!(
                "{} fighters at 0 hp survived the reaper",
                walking_dead.len()
            ));
        }
    }

    let packs: Vec<(Entity, Vec<Entity>)> = w
        .query::<(Entity, &Backpack)>()
        .iter(w)
        .map(|(e, b)| (e, b.items.clone()))
        .collect();
    let mut carried = HashSet::new();
    for (owner, items) in packs {
        for item in items {
            if !carried.insert(item) {
                return Some(format!("{item:?} sits in more than one pack slot"));
            }
            let Some(item_ref) = w.get_entity(item) else {
                return Some(format!("{owner:?} carries {item:?}, which is gone"));
            };
            if item_ref.contains::<Position>() {
                return Some(format!("{owner:?} carries {item:?}, which has a Position"));
            }
        }
    }

    let worn: Vec<(Entity, Entity)> = w
        .query::<(Entity, &Equipped)>()
        .iter(w)
        .filter_map(|(item, eq)| eq.by.map(|by| (item, by)))
        .collect();
    for (item, by) in worn {
        if w.get_entity(by).is_none() {
            return Some(format!("{item:?} is worn by {by:?}, which is gone"));
        }
    }

    let bad_stack = w
        .query::<(Entity, &Stack)>()
        .iter(w)
        .find(|(_, s)| !(1..=STACK_LIMIT).contains(&s.count))
        .map(|(e, s)| (e, s.count));
    if let Some((item, count)) = bad_stack {
        return Some(format!("{item:?} is a stack of {count}"));
    }

    let mut standing = HashSet::new();
    let shared = w
        .query_filtered::<&Position, Or<(With<Mob>, With<Player>)>>()
        .iter(w)
        .find(|p| !standing.insert((p.x, p.y)))
        .copied();
    if let Some(p) = shared {
        return Some(format!("two creatures share ({}, {})", p.x, p.y));
    }

    None
}

fn toward(from: Position, to: Position) -> (i16, i16) {
    (
        (to.x as i32 - from.x as i32).signum() as i16,
        (to.y as i32 - from.y as i32).signum() as i16,
    )
}

fn step_or_fight(w: &mut World, player: Entity) -> bool {
    let here = *w.get::<Position>(player).unwrap();
    let neighbour = w
        .query_filtered::<&Position, (With<Mob>, Without<Helper>, Without<IceCube>)>()
        .iter(w)
        .find(|p| chebyshev(here, **p) <= 1)
        .copied();
    if let Some(foe) = neighbour {
        let (dx, dy) = toward(here, foe);
        if queue_step(w, dx, dy) {
            return true;
        }
    }
    match explore_step(w) {
        Some((dx, dy)) => queue_step(w, dx, dy),
        None => false,
    }
}

/// What the bot got done across a run, so the soak can prove it exercised
/// more than walking.
#[derive(Default)]
struct Tally {
    uses: u32,
    throws: u32,
    casts: u32,
    drops: u32,
    descents: u32,
}

/// The tile the bot aims at: the nearest creature in sight, else a tile a few
/// steps off in a random direction. Never the player's own tile.
fn aim(w: &mut World, player: Entity, rng: &mut ChaCha12Rng) -> Position {
    let here = *w.get::<Position>(player).unwrap();
    let foe = w
        .query_filtered::<&Position, (With<Mob>, Without<Player>)>()
        .iter(w)
        .filter(|p| chebyshev(here, **p) <= 6)
        .min_by_key(|p| chebyshev(here, **p))
        .copied();
    foe.unwrap_or_else(|| {
        let (dx, dy) = loop {
            let d = (rng.gen_range(-3i32..=3), rng.gen_range(-3i32..=3));
            if d != (0, 0) {
                break d;
            }
        };
        Position {
            x: (here.x as i32 + dx).clamp(0, MAP_WIDTH as i32 - 1) as u16,
            y: (here.y as i32 + dy).clamp(0, MAP_HEIGHT as i32 - 1) as u16,
        }
    })
}

/// A random item from the pack; with `loose`, only one nobody wears, so the
/// bot does not throw or drop the gear keeping it alive.
fn random_pack_item(
    w: &World,
    player: Entity,
    rng: &mut ChaCha12Rng,
    loose: bool,
) -> Option<Entity> {
    let items: Vec<Entity> = w
        .get::<Backpack>(player)?
        .items
        .iter()
        .copied()
        .filter(|&i| !loose || w.get::<Equipped>(i).is_none_or(|e| e.by.is_none()))
        .collect();
    (!items.is_empty()).then(|| items[rng.gen_range(0..items.len())])
}

/// One of the item verbs, picked by the bot's RNG, or `None` when the roll
/// says walk. Mirrors the engine: a ranged item is aimed, anything else is not.
fn try_a_verb(
    w: &mut World,
    player: Entity,
    rng: &mut ChaCha12Rng,
    tally: &mut Tally,
) -> Option<bool> {
    match rng.gen_range(0..40) {
        0 | 1 => {
            let item = random_pack_item(w, player, rng, false)?;
            let target = w.get::<Ranged>(item).is_some().then(|| aim(w, player, rng));
            let queued = queue_use(w, player, item, target);
            tally.uses += queued as u32;
            Some(queued)
        }
        2 | 3 => {
            let item = random_pack_item(w, player, rng, true)?;
            let target = aim(w, player, rng);
            let queued = queue_throw(w, player, item, target);
            tally.throws += queued as u32;
            Some(queued)
        }
        4 => {
            let slots = w.get::<Spellset>(player).map(|s| s.slots.clone())?;
            let effect = *slots.get(rng.gen_range(0..slots.len().max(1)))?;
            let target = aim(w, player, rng);
            let queued = queue_cast(w, player, effect, target);
            tally.casts += queued as u32;
            Some(queued)
        }
        5 => {
            let item = random_pack_item(w, player, rng, true)?;
            let queued = queue_drop(w, player, item);
            tally.drops += queued as u32;
            Some(queued)
        }
        _ => None,
    }
}

/// Once the floor is explored, or the bot has spent long enough on it, it
/// walks to the downstairs it has seen and takes them.
fn head_down(w: &mut World, tally: &mut Tally) -> Option<bool> {
    let stairs = stair_location(&w.resource::<Map>().clone(), true)?;
    match travel_step(w, stairs) {
        Some((dx, dy)) => Some(queue_step(w, dx, dy)),
        None => {
            let here = player_pos(w);
            if (here.x, here.y) != stairs {
                return None;
            }
            let queued = queue_stairs(w, true);
            tally.descents += queued as u32;
            Some(queued)
        }
    }
}

fn player_pos(w: &mut World) -> Position {
    *w.query_filtered::<&Position, With<Player>>()
        .iter(w)
        .next()
        .unwrap()
}

/// The bot's turn: maybe an item verb, else the stairs when it is time to go
/// down, else a step or a blow. `false` when it has nothing left to do.
fn bot_turn(
    w: &mut World,
    player: Entity,
    rng: &mut ChaCha12Rng,
    tally: &mut Tally,
    restless: bool,
) -> bool {
    if let Some(queued) = try_a_verb(w, player, rng, tally)
        && queued
    {
        return true;
    }
    if restless && let Some(queued) = head_down(w, tally) {
        return queued;
    }
    step_or_fight(w, player) || head_down(w, tally).unwrap_or(false)
}

#[test]
fn a_bot_that_drinks_throws_and_descends_never_breaks_what_the_schedule_assumes() {
    let mut tally = Tally::default();
    for seed in 1..=24u64 {
        let (mut w, player) = new_run(seed);
        let mut rng = ChaCha12Rng::seed_from_u64(seed ^ 0x50a6);
        let mut schedule = turn_schedule();
        schedule.run(&mut w);

        let mut played = 0;
        let mut on_floor = 0;
        let mut depth = w.resource::<Depth>().what;
        for turn in 0..400 {
            let restless = on_floor > 150;
            if w.resource::<Ending>().player_dead
                || !bot_turn(&mut w, player, &mut rng, &mut tally, restless)
            {
                break;
            }
            schedule.run(&mut w);
            played += 1;
            on_floor += 1;
            if w.resource::<Depth>().what != depth {
                depth = w.resource::<Depth>().what;
                on_floor = 0;
            }
            if let Some(why) = broken_invariant(&mut w) {
                panic!("seed {seed}, turn {turn}: {why}");
            }
        }
        assert!(
            played >= 30 || w.resource::<Ending>().player_dead,
            "seed {seed}: the bot stalled after {played} turns"
        );
    }
    eprintln!(
        "soak: {} uses, {} throws, {} casts, {} drops, {} descents",
        tally.uses, tally.throws, tally.casts, tally.drops, tally.descents
    );
    assert!(tally.uses > 0, "the bot never used an item");
    assert!(tally.throws > 0, "the bot never threw anything");
    assert!(tally.descents > 0, "the bot never took the stairs down");
}

#[test]
fn the_check_sees_a_pack_that_points_at_a_despawned_item() {
    let (mut w, player) = new_run(1);
    let item = w.spawn_empty().id();
    w.get_mut::<Backpack>(player).unwrap().items.push(item);
    assert_eq!(broken_invariant(&mut w), None);
    w.despawn(item);
    assert!(broken_invariant(&mut w).is_some_and(|why| why.contains("is gone")));
}

#[test]
fn the_check_sees_a_leftover_move_marker_and_an_undrained_queue() {
    let (mut w, player) = new_run(1);
    w.entity_mut(player).insert(EntityMoved);
    assert!(broken_invariant(&mut w).is_some_and(|why| why.contains("EntityMoved")));
    w.entity_mut(player).remove::<EntityMoved>();
    w.resource_mut::<AttackQueue>().attacks.push(WantsToAttack {
        attacker: player,
        target: player,
    });
    assert!(broken_invariant(&mut w).is_some_and(|why| why.contains("AttackQueue")));
}

#[test]
fn the_check_sees_an_item_in_two_pack_slots() {
    let (mut w, player) = new_run(1);
    let item = w.spawn_empty().id();
    w.get_mut::<Backpack>(player).unwrap().items.push(item);
    assert_eq!(broken_invariant(&mut w), None);
    w.get_mut::<Backpack>(player).unwrap().items.push(item);
    assert!(broken_invariant(&mut w).is_some_and(|why| why.contains("more than one pack slot")));
}

#[test]
fn the_check_sees_an_empty_stack_and_an_overfull_one() {
    let (mut w, player) = new_run(1);
    let item = w.spawn(Stack { count: 1 }).id();
    w.get_mut::<Backpack>(player).unwrap().items.push(item);
    assert_eq!(broken_invariant(&mut w), None);
    w.get_mut::<Stack>(item).unwrap().count = 0;
    assert!(broken_invariant(&mut w).is_some_and(|why| why.contains("stack of 0")));
    w.get_mut::<Stack>(item).unwrap().count = STACK_LIMIT + 1;
    assert!(broken_invariant(&mut w).is_some_and(|why| why.contains("stack of")));
}

#[test]
fn the_check_sees_two_creatures_on_one_tile() {
    let (mut w, player) = new_run(1);
    let here = *w.get::<Position>(player).unwrap();
    assert_eq!(broken_invariant(&mut w), None);
    common::monster::plain_monster(&mut w, "test monster", here);
    assert!(broken_invariant(&mut w).is_some_and(|why| why.contains("two creatures share")));
}
