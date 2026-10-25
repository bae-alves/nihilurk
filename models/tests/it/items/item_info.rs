//! The pack's Info row: what an item tells you about itself, and what it must
//! not tell you before you have earned it.

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
fn a_known_curse_is_explained() {
    let mut w = world();
    let cursed = spawn(&mut w, "dagger");
    w.entity_mut(cursed).insert((Curse, KnownQuality));
    let lines = item_info(&w, cursed);
    assert!(lines.iter().any(|l| l == strings::cursed_explained()));
}
