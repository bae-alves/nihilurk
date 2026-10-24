//! A step is planned when the key is pressed and applied by the first step of
//! the turn: the plan decides whether the turn is spent, the schedule does the
//! stepping.

use crate::common;
use crate::common::monster;

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
    let foe = monster::plain_monster(&mut w, "foe", at(here.x as i32 + 2, here.y as i32));
    assert!(queue_step(&mut w, 1, 0));
    resolve(&mut w);
    assert_eq!(
        *w.get::<Position>(player).unwrap(),
        at(here.x as i32 + 1, here.y as i32)
    );
    let _ = foe;
}
