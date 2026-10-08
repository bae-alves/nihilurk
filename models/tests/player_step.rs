//! A step is planned when the key is pressed and applied by the first step of
//! the turn: the plan decides whether the turn is spent, the schedule does the
//! stepping.

mod common;

use bevy_ecs::prelude::*;
use models::*;

/// A floor with nothing on it but the player, and open ground in a plus around
/// them.
fn quiet_room(seed: u64) -> (World, Entity, Position) {
    let (mut w, player) = common::fresh_run(seed);
    let clutter: Vec<Entity> = w
        .iter_entities()
        .filter(|e| !e.contains::<Player>() && (e.contains::<Mob>() || e.contains::<Item>()))
        .map(|e| e.id())
        .collect();
    for e in clutter {
        w.despawn(e);
    }
    let here = *w.get::<Position>(player).unwrap();
    {
        let mut map = w.resource_mut::<Map>();
        for dx in -2i32..=2 {
            for dy in -2i32..=2 {
                let (x, y) = ((here.x as i32 + dx) as u16, (here.y as i32 + dy) as u16);
                map.tiles[tile_index(x, y)] = TileType::Room;
            }
        }
    }
    (w, player, here)
}

fn at(x: i32, y: i32) -> Position {
    Position {
        x: x as u16,
        y: y as u16,
    }
}

fn resolve(w: &mut World) {
    player_action_system(w);
}

#[test]
fn a_step_into_a_wall_spends_no_turn_and_queues_nothing() {
    let (mut w, player, here) = quiet_room(1);
    w.resource_mut::<Map>().tiles[tile_index(here.x + 1, here.y)] = TileType::Wall;
    assert!(!queue_step(&mut w, 1, 0));
    assert!(w.resource::<PlayerActionQueue>().actions.is_empty());
    resolve(&mut w);
    assert_eq!(*w.get::<Position>(player).unwrap(), here);
}

#[test]
fn an_open_step_spends_the_turn_but_waits_for_the_schedule_to_move() {
    let (mut w, player, here) = quiet_room(1);
    assert!(queue_step(&mut w, 1, 0));
    assert_eq!(*w.get::<Position>(player).unwrap(), here);
    resolve(&mut w);
    assert_eq!(
        *w.get::<Position>(player).unwrap(),
        at(here.x as i32 + 1, here.y as i32)
    );
    assert!(w.get::<EntityMoved>(player).is_some());
    assert!(w.resource::<PlayerActionQueue>().actions.is_empty());
}

#[test]
fn a_step_into_a_monster_is_a_blow_and_the_player_stays_put() {
    let (mut w, player, here) = quiet_room(1);
    let target = at(here.x as i32 + 1, here.y as i32);
    let foe = spawn_monster(&mut w, &BESTIARY[0], target);
    let before = w.resource::<GameLog>().unread.len();
    assert!(queue_step(&mut w, 1, 0));
    resolve(&mut w);
    assert_eq!(*w.get::<Position>(player).unwrap(), here);
    assert!(w.get_entity(foe).is_none() || *w.get::<Position>(foe).unwrap() == target);
    assert!(w.resource::<GameLog>().unread.len() > before);
}

#[test]
fn stepping_into_a_helper_swaps_places_with_it() {
    let (mut w, player, here) = quiet_room(1);
    let target = at(here.x as i32 + 1, here.y as i32);
    let pet = spawn_monster(&mut w, &BESTIARY[0], target);
    w.entity_mut(pet).insert(Helper);
    assert!(queue_step(&mut w, 1, 0));
    resolve(&mut w);
    assert_eq!(*w.get::<Position>(player).unwrap(), target);
    assert_eq!(*w.get::<Position>(pet).unwrap(), here);
    assert!(w.get::<EntityMoved>(pet).is_some());
}

#[test]
fn a_rooted_player_strains_and_stays_where_they_are() {
    let (mut w, player, here) = quiet_room(1);
    lend(&mut w, player, Grant::of::<Rooted>(), Lifetime::Turns(3));
    assert!(queue_step(&mut w, 1, 0));
    resolve(&mut w);
    assert_eq!(*w.get::<Position>(player).unwrap(), here);
}

#[test]
fn an_estoc_lunges_across_the_empty_tile_at_the_foe_beyond() {
    let (mut w, player, here) = quiet_room(1);
    w.entity_mut(player).insert(Lunges);
    let foe = spawn_monster(&mut w, &BESTIARY[0], at(here.x as i32 + 2, here.y as i32));
    assert!(queue_step(&mut w, 1, 0));
    resolve(&mut w);
    assert_eq!(
        *w.get::<Position>(player).unwrap(),
        at(here.x as i32 + 1, here.y as i32)
    );
    let _ = foe;
}

#[test]
fn arriving_on_an_item_picks_it_up() {
    let (mut w, player, here) = quiet_room(1);
    let target = at(here.x as i32 + 1, here.y as i32);
    let potion = spawn_named(&mut w, "potion of healing", target).unwrap();
    assert!(queue_step(&mut w, 1, 0));
    resolve(&mut w);
    assert!(w.get::<Backpack>(player).unwrap().items.contains(&potion));
}
