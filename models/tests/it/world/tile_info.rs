//! What `?` on the look reticle (and a look click) reads off a tile beyond
//! the sighting line: stats for a creature, mechanics for a trap, the pack's
//! Info lines for an item.

use crate::common::fixtures::spawn_named;
use bevy_ecs::prelude::*;
use models::*;

fn world() -> World {
    let mut w = World::new();
    w.insert_resource(GameRng(ChaCha12Rng::seed_from_u64(1)));
    w.insert_resource(RngSeed(1));
    w.init_resource::<GameLog>();
    w.insert_resource(PlayerName {
        what: "TESTER".into(),
    });
    initialize_world(&mut w);
    w
}

fn spawn(w: &mut World, name: &str) -> Entity {
    spawn_named(w, name, Position { x: 0, y: 0 }).expect("row exists")
}

#[test]
fn a_creature_reports_health_and_speed() {
    let mut w = world();
    let mob = spawn(&mut w, "kestral");
    let hp = w.get::<Fighter>(mob).unwrap().hp;
    let lines = tile_info(&w, mob);
    assert!(lines.contains(&strings::info_health(hp, hp)), "{lines:?}");
    assert!(lines.iter().any(|l| l.starts_with("Speed:")), "{lines:?}");
}

#[test]
fn a_bare_entity_has_nothing_to_report() {
    let mut w = world();
    let e = w.spawn(Position { x: 0, y: 0 }).id();
    assert!(tile_info(&w, e).is_empty());
}

#[test]
fn a_creature_states_the_odds_against_the_hero() {
    let mut w = world();
    let mob = spawn(&mut w, "kestral");
    let lines = tile_info(&w, mob);
    assert!(
        lines
            .iter()
            .any(|l| l.starts_with("They will likely kill you in")
                || l.starts_with("They cannot hurt you")),
        "{lines:?}"
    );
    assert!(
        lines
            .iter()
            .any(|l| l.starts_with("You will likely kill them in")
                || l.starts_with("You cannot hurt them")),
        "{lines:?}"
    );
}

#[test]
fn a_weaker_blow_takes_more_hits() {
    let mut w = world();
    let mob = spawn(&mut w, "kestral");
    let player = w
        .iter_entities()
        .find(|e| e.contains::<Player>())
        .map(|e| e.id())
        .expect("hero");
    w.get_mut::<Fighter>(mob).unwrap().armor_bonus = 0;
    let before = mean_blow(&w, player, mob);
    w.get_mut::<Fighter>(mob).unwrap().armor_bonus = 3;
    assert!(mean_blow(&w, player, mob) < before);
}

/// A thing's blurb is the last of its Info lines, below its stats; an empty
/// one adds no line.
#[test]
fn a_blurb_closes_the_info_and_an_empty_one_adds_nothing() {
    let mut w = world();
    for (_, name) in content_names() {
        let Some(e) = spawn_named(&mut w, name, Position { x: 0, y: 0 }) else {
            continue;
        };
        let lines = tile_info(&w, e);
        assert!(!lines.iter().any(String::is_empty), "{name}: {lines:?}");
        if let Some(blurb) = strings::blurb(name).filter(|b| !b.is_empty()) {
            assert_eq!(lines.last().map(String::as_str), Some(blurb), "{name}");
        }
    }
}
