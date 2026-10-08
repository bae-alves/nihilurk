//! A seeded bot plays the first floor through the real turn schedule, and
//! after every turn the world must still satisfy what each system assumes of
//! the one before it.
//!
//! Those assumptions are written as prose beside each system. Prose does not
//! fail when a despawn leaves a pack pointing at nothing, or a step forgets to
//! drain its queue; this does.

use bevy_ecs::prelude::*;
use models::*;

mod common;

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
    for (owner, items) in packs {
        for item in items {
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
        return queue_step(w, dx, dy);
    }
    match explore_step(w) {
        Some((dx, dy)) => queue_step(w, dx, dy),
        None => false,
    }
}

#[test]
fn a_bot_walking_the_first_floor_never_breaks_what_the_schedule_assumes() {
    for seed in 1..=12u64 {
        let (mut w, player) = new_run(seed);
        let mut schedule = turn_schedule();
        schedule.run(&mut w);

        let mut played = 0;
        for turn in 0..400 {
            if w.resource::<Ending>().player_dead || !step_or_fight(&mut w, player) {
                break;
            }
            schedule.run(&mut w);
            played += 1;
            if let Some(why) = broken_invariant(&mut w) {
                panic!("seed {seed}, turn {turn}: {why}");
            }
        }
        assert!(
            played >= 30,
            "seed {seed}: the bot only played {played} turns"
        );
    }
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
