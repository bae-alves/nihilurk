//! Gear a creature is wearing rides along in the lines that name it: the
//! sighting log and `look` both hang a `"(w. …)"` on the creature.

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
    w
}

fn player(w: &mut World) -> Entity {
    w.query_filtered::<Entity, With<Player>>().single(w)
}

/// An open floor tile next to the player.
fn beside_player(w: &mut World) -> Position {
    let p = player(w);
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

fn run_visibility(w: &mut World) {
    let p = player(w);
    w.get_mut::<Viewshed>(p).unwrap().dirty = true;
    let mut s = Schedule::default();
    s.add_systems(visibility_system);
    s.run(w);
}

fn log_has(w: &World, needle: &str) -> bool {
    w.resource::<GameLog>()
        .history
        .iter()
        .any(|l| l.contains(needle))
}

/// A monster standing next to the player with `gear` already on.
fn armed_neighbour(w: &mut World, gear: &[&str]) -> Entity {
    let spot = beside_player(w);
    let mob = monster::monster(w, "emu", spot);
    for name in gear {
        let item = spawn_gear(w, name, spot);
        assert!(equip_silently(w, mob, item), "{name} went on");
    }
    mob
}

fn spawn_gear(w: &mut World, name: &str, pos: Position) -> Entity {
    match name {
        "short bow" | "crossbow" => spawn_launcher(w, name, pos),
        "ring mail" | "plate mail" => spawn_armor(w, name, pos),
        _ => spawn_weapon(w, name, pos),
    }
}

#[test]
fn a_sighting_names_what_the_creature_is_wearing() {
    let mut w = test_world(1);
    armed_neighbour(&mut w, &["short bow"]);

    run_visibility(&mut w);

    assert!(
        log_has(&w, "You spotted an emu (w. a short bow)."),
        "{:?}",
        w.resource::<GameLog>().history
    );
}

#[test]
fn a_bare_creature_gets_no_parenthesis() {
    let mut w = test_world(2);
    armed_neighbour(&mut w, &[]);

    run_visibility(&mut w);

    assert!(
        log_has(&w, "You spotted an emu."),
        "{:?}",
        w.resource::<GameLog>().history
    );
}

#[test]
fn every_worn_piece_is_listed() {
    let mut w = test_world(3);
    let mob = armed_neighbour(&mut w, &["short bow", "ring mail"]);

    let tag = worn_tag(&w, mob);
    assert!(tag.contains("a short bow"), "{tag}");
    assert!(
        tag.contains("ring mail") && !tag.contains("a ring mail"),
        "{tag}"
    );
    assert!(tag.starts_with(" (w. ") && tag.ends_with(')'), "{tag}");
}

#[test]
fn worn_gear_is_not_also_lying_on_the_floor() {
    let mut w = test_world(4);
    let mob = armed_neighbour(&mut w, &["short bow"]);
    let bow = equipped_items(&w, mob)[0];

    run_visibility(&mut w);

    assert!(
        w.get::<Position>(bow).is_none(),
        "a worn bow is carried, not loot"
    );
    assert!(!log_has(&w, "You spotted a short bow."));
}
