//! The rules of the player's own step that are not about the key pressed:
//! the confusion tax, picking up on arrival, and a special room's greeting.

use bevy_ecs::prelude::*;
use models::*;

fn world_with_player(seed: u64) -> (World, Entity) {
    let mut w = World::new();
    w.insert_resource(GameRng(ChaCha12Rng::seed_from_u64(seed)));
    w.init_resource::<GameLog>();
    w.insert_resource(Map {
        tiles: vec![TileType::Room; MAP_TILE_COUNT],
        dark: fixedbitset::FixedBitSet::with_capacity(MAP_TILE_COUNT),
        inert_doors: fixedbitset::FixedBitSet::with_capacity(MAP_TILE_COUNT),
        special: vec![None; MAP_TILE_COUNT],
        level: None,
    });
    let player = w
        .spawn((
            Player,
            Position { x: 5, y: 5 },
            Backpack { items: Vec::new() },
        ))
        .id();
    (w, player)
}

#[test]
fn a_clear_head_never_stumbles() {
    let (mut w, _) = world_with_player(1);
    for _ in 0..50 {
        assert_eq!(maybe_stumble(&mut w, 1, 0), (1, 0, false));
    }
}

#[test]
fn a_confused_player_sometimes_lurches_somewhere_else_and_says_so() {
    let (mut w, player) = world_with_player(1);
    w.entity_mut(player).insert(Confused);
    let before = w.resource::<GameLog>().unread.len();
    let mut lurches = 0;
    let mut straight = 0;
    for _ in 0..200 {
        match maybe_stumble(&mut w, 1, 0) {
            (1, 0, false) => straight += 1,
            (dx, dy, true) => {
                assert!(dx.abs() <= 1 && dy.abs() <= 1 && (dx, dy) != (0, 0));
                lurches += 1;
            }
            other => panic!("a step that was not hijacked changed: {other:?}"),
        }
    }
    assert!(
        lurches > 0 && straight > 0,
        "{lurches} lurches, {straight} straight"
    );
    assert_eq!(w.resource::<GameLog>().unread.len() - before, lurches);
}

#[test]
fn arriving_on_a_tile_picks_up_what_lies_there() {
    let (mut w, player) = world_with_player(1);
    let potion = spawn_named(&mut w, "potion of healing", Position { x: 5, y: 5 }).unwrap();
    pick_up_here(&mut w, player, 5, 5);
    assert!(w.get::<Backpack>(player).unwrap().items.contains(&potion));
    assert!(w.get::<Position>(potion).is_none());
}

#[test]
fn arriving_on_an_empty_tile_picks_up_nothing() {
    let (mut w, player) = world_with_player(1);
    let before = w.resource::<GameLog>().unread.len();
    pick_up_here(&mut w, player, 5, 5);
    assert!(w.get::<Backpack>(player).unwrap().items.is_empty());
    assert_eq!(w.resource::<GameLog>().unread.len(), before);
}
