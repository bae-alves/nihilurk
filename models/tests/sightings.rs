//! Things that come into view together read as one line, not one per thing.

#[path = "common/monster.rs"]
mod monster;

use bevy_ecs::prelude::*;
use bevy_ecs::schedule::Schedule;
use models::*;

fn test_world(seed: u64) -> World {
    let mut w = World::new();
    w.insert_resource(GameRng(ChaCha12Rng::seed_from_u64(seed)));
    w.insert_resource(RngSeed(seed));
    w.init_resource::<GameLog>();
    w.init_resource::<AttackQueue>();
    w.init_resource::<Ending>();
    w.insert_resource(PlayerName {
        what: "TESTER".into(),
    });
    initialize_world(&mut w);
    let loot: Vec<Entity> = w
        .query_filtered::<Entity, Or<(With<Item>, With<Mob>)>>()
        .iter(&w)
        .collect();
    for e in loot {
        w.despawn(e);
    }
    w
}

/// An open floor tile next to the player.
fn beside_player(w: &mut World) -> Position {
    let p = w.query_filtered::<Entity, With<Player>>().single(w);
    let here = *w.get::<Position>(p).unwrap();
    let map = w.resource::<Map>();
    for (dx, dy) in [(1i32, 0i32), (-1, 0), (0, 1), (0, -1), (1, 1), (-1, -1)] {
        let (nx, ny) = (here.x as i32 + dx, here.y as i32 + dy);
        if nx >= 0 && ny >= 0 && !map.blocks(nx as u16, ny as u16) {
            return Position {
                x: nx as u16,
                y: ny as u16,
            };
        }
    }
    panic!("player is walled in");
}

/// One visibility pass, and everything it logged.
fn sightings(w: &mut World) -> Vec<String> {
    let p = w.query_filtered::<Entity, With<Player>>().single(w);
    w.resource_mut::<GameLog>().history.clear();
    w.get_mut::<Viewshed>(p).unwrap().dirty = true;
    let mut s = Schedule::default();
    s.add_systems(visibility_system);
    s.run(w);
    w.resource::<GameLog>().history.iter().cloned().collect()
}

#[test]
fn identical_items_spotted_together_are_one_counted_line() {
    let mut w = test_world(1);
    let spot = beside_player(&mut w);
    for _ in 0..3 {
        spawn_weapon(&mut w, "dagger", spot);
    }
    let said = sightings(&mut w);
    assert!(
        said.iter().any(|l| l == "You spotted 3 daggers."),
        "{said:?}"
    );
    assert!(
        !said.iter().any(|l| l == "You spotted a dagger."),
        "{said:?}"
    );
}

#[test]
fn a_stack_counts_for_its_whole_size() {
    let mut w = test_world(2);
    let spot = beside_player(&mut w);
    for count in [4, 3] {
        let arrows = spawn_ammo(&mut w, "arrow", spot);
        w.get_mut::<Stack>(arrows).unwrap().count = count;
    }
    let said = sightings(&mut w);
    assert!(
        said.iter().any(|l| l == "You spotted 7 arrows."),
        "{said:?}"
    );
}

#[test]
fn different_items_stay_on_their_own_lines() {
    let mut w = test_world(3);
    let spot = beside_player(&mut w);
    spawn_weapon(&mut w, "dagger", spot);
    spawn_armor(&mut w, "ring mail", spot);
    let said = sightings(&mut w);
    assert!(
        said.iter().any(|l| l == "You spotted a dagger."),
        "{said:?}"
    );
    assert!(
        said.iter().any(|l| l == "You spotted a ring mail."),
        "{said:?}"
    );
}

#[test]
fn identical_monsters_spotted_together_are_one_counted_line() {
    let mut w = test_world(4);
    let spot = beside_player(&mut w);
    for _ in 0..3 {
        monster::monster(&mut w, "emu", spot);
    }
    let said = sightings(&mut w);
    assert!(said.iter().any(|l| l == "You spotted 3 emus."), "{said:?}");
    assert!(!said.iter().any(|l| l == "You spotted an emu."), "{said:?}");
}

#[test]
fn monsters_in_different_gear_stay_on_their_own_lines() {
    let mut w = test_world(5);
    let spot = beside_player(&mut w);
    monster::monster(&mut w, "emu", spot);
    let armed = monster::monster(&mut w, "emu", spot);
    let bow = spawn_launcher(&mut w, "short bow", spot);
    assert!(equip_silently(&mut w, armed, bow));
    let said = sightings(&mut w);
    assert!(said.iter().any(|l| l == "You spotted an emu."), "{said:?}");
    assert!(
        said.iter()
            .any(|l| l == "You spotted an emu (w. a short bow)."),
        "{said:?}"
    );
}
