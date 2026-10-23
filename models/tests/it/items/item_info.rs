//! The pack's Info row: what an item tells you about itself, and what it must
//! not tell you before you have earned it.

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
fn a_weapon_reports_its_die() {
    let mut w = world();
    let dagger = spawn(&mut w, "dagger");
    let text = item_info(&w, dagger).join("\n");
    assert!(text.contains("d4"), "{text}");
}

#[test]
fn a_hidden_enchantment_is_not_in_the_text() {
    let mut w = world();
    let plain = spawn(&mut w, "dagger");
    let boosted = spawn(&mut w, "dagger");
    w.entity_mut(boosted).insert(PowerBonus(3));
    assert_eq!(item_info(&w, plain), item_info(&w, boosted));
}

#[test]
fn a_known_enchantment_is_in_the_text() {
    let mut w = world();
    let boosted = spawn(&mut w, "dagger");
    w.entity_mut(boosted).insert((PowerBonus(3), KnownQuality));
    let text = item_info(&w, boosted).join("\n");
    assert!(text.contains("+3"), "{text}");
}

#[test]
fn an_unknown_curse_is_not_explained() {
    let mut w = world();
    let plain = spawn(&mut w, "dagger");
    let cursed = spawn(&mut w, "dagger");
    w.entity_mut(cursed).insert(Curse);
    assert_eq!(item_info(&w, plain), item_info(&w, cursed));
}

#[test]
fn a_known_curse_is_explained() {
    let mut w = world();
    let cursed = spawn(&mut w, "dagger");
    w.entity_mut(cursed).insert((Curse, KnownQuality));
    let lines = item_info(&w, cursed);
    assert!(lines.iter().any(|l| l == strings::cursed_explained()));
}
