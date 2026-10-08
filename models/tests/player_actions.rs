//! The player's verbs other than a step: each is decided at the key (a refusal
//! says why and spends nothing), queued, and carried out by the schedule's first
//! step.

mod common;

use bevy_ecs::prelude::*;
use models::*;

fn at(x: i32, y: i32) -> Position {
    Position {
        x: x as u16,
        y: y as u16,
    }
}

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
        for dx in -4i32..=4 {
            for dy in -2i32..=2 {
                let (x, y) = ((here.x as i32 + dx) as u16, (here.y as i32 + dy) as u16);
                map.tiles[tile_index(x, y)] = TileType::Room;
            }
        }
    }
    (w, player, here)
}

fn resolve(w: &mut World) {
    player_action_system(w);
}

fn log_len(w: &World) -> usize {
    w.resource::<GameLog>().unread.len()
}

#[test]
fn stairs_off_the_stairs_say_so_and_queue_nothing() {
    let (mut w, _, here) = quiet_room(1);
    w.resource_mut::<Map>().tiles[tile_index(here.x, here.y)] = TileType::Room;
    let before = log_len(&w);
    assert!(!queue_stairs(&mut w, true));
    assert!(log_len(&w) > before);
    assert!(w.resource::<PlayerActionQueue>().actions.is_empty());
}

#[test]
fn stairs_down_wait_for_the_schedule_to_change_the_floor() {
    let (mut w, _, here) = quiet_room(1);
    w.resource_mut::<Map>().tiles[tile_index(here.x, here.y)] = TileType::Downstairs;
    assert!(queue_stairs(&mut w, true));
    assert_eq!(w.resource::<Depth>().what, 1);
    resolve(&mut w);
    assert_eq!(w.resource::<Depth>().what, 2);
    assert!(w.resource::<PlayerActionQueue>().actions.is_empty());
}

#[test]
fn a_dropped_item_leaves_the_pack_when_the_schedule_runs() {
    let (mut w, player, here) = quiet_room(1);
    let potion = spawn_named(&mut w, "potion of healing", here).unwrap();
    w.entity_mut(potion).remove::<Position>();
    w.get_mut::<Backpack>(player).unwrap().items.push(potion);
    assert!(queue_drop(&mut w, player, potion));
    assert!(w.get::<Backpack>(player).unwrap().items.contains(&potion));
    resolve(&mut w);
    assert!(!w.get::<Backpack>(player).unwrap().items.contains(&potion));
    assert_eq!(*w.get::<Position>(potion).unwrap(), here);
}

#[test]
fn cursed_gear_on_the_body_will_not_be_dropped() {
    let (mut w, player, here) = quiet_room(1);
    let ring = spawn_named(&mut w, "potion of healing", here).unwrap();
    w.entity_mut(ring).remove::<Position>();
    w.entity_mut(ring).insert((
        Curse,
        Equipped {
            by: Some(player),
            slot: Slot::Hand,
        },
    ));
    w.get_mut::<Backpack>(player).unwrap().items.push(ring);
    let before = log_len(&w);
    assert!(!queue_drop(&mut w, player, ring));
    assert!(log_len(&w) > before);
    assert!(w.resource::<PlayerActionQueue>().actions.is_empty());
    assert!(w.get::<Backpack>(player).unwrap().items.contains(&ring));
}

#[test]
fn willed_teleport_needs_the_gift_and_the_magic() {
    let (mut w, player, _) = quiet_room(1);
    assert!(!queue_willed_teleport(&mut w));
    w.entity_mut(player).insert(Teleportitis);
    w.entity_mut(player).insert(Magic {
        points: 0,
        max_points: 4,
    });
    assert!(!queue_willed_teleport(&mut w));
    assert!(w.resource::<PlayerActionQueue>().actions.is_empty());
}

#[test]
fn willed_teleport_spends_the_magic_when_the_schedule_runs() {
    let (mut w, player, here) = quiet_room(1);
    w.entity_mut(player).insert(Teleportitis);
    w.entity_mut(player).insert(Magic {
        points: 4,
        max_points: 4,
    });
    assert!(queue_willed_teleport(&mut w));
    assert_eq!(w.get::<Magic>(player).unwrap().points, 4);
    assert_eq!(*w.get::<Position>(player).unwrap(), here);
    resolve(&mut w);
    assert!(w.get::<Magic>(player).unwrap().points < 4);
}

#[test]
fn a_charge_closes_on_the_foe_when_the_schedule_runs() {
    let (mut w, player, here) = quiet_room(1);
    let foe = spawn_monster(&mut w, &BESTIARY[0], at(here.x as i32 + 3, here.y as i32));
    assert!(queue_charge(&mut w, foe));
    assert_eq!(*w.get::<Position>(player).unwrap(), here);
    resolve(&mut w);
    assert_eq!(
        *w.get::<Position>(player).unwrap(),
        at(here.x as i32 + 2, here.y as i32)
    );
    assert!(w.get::<EntityMoved>(player).is_some());
}

#[test]
fn a_charge_through_a_wall_is_refused() {
    let (mut w, _, here) = quiet_room(1);
    let foe = spawn_monster(&mut w, &BESTIARY[0], at(here.x as i32 + 3, here.y as i32));
    w.resource_mut::<Map>().tiles[tile_index(here.x + 1, here.y)] = TileType::Wall;
    assert!(!queue_charge(&mut w, foe));
    assert!(w.resource::<PlayerActionQueue>().actions.is_empty());
}

#[test]
fn a_reach_attack_strikes_when_the_schedule_runs() {
    let (mut w, player, here) = quiet_room(1);
    let weapon = spawn_named(&mut w, "potion of healing", here).unwrap();
    w.entity_mut(weapon).remove::<Position>();
    w.entity_mut(weapon).insert(Reach(2));
    let foe_at = at(here.x as i32 + 2, here.y as i32);
    let foe = spawn_monster(&mut w, &BESTIARY[0], foe_at);
    let hp = w.get::<Fighter>(foe).unwrap().hp;
    let before = log_len(&w);
    queue_reach_attack(&mut w, weapon, foe_at);
    assert_eq!(log_len(&w), before);
    resolve(&mut w);
    assert!(log_len(&w) > before || w.get::<Fighter>(foe).is_none_or(|f| f.hp < hp));
    let _ = player;
}
