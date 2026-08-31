use bevy_ecs::prelude::*;
use models::*;

fn test_world(seed: u64) -> World {
    let mut w = World::new();
    w.insert_resource(GameRng(ChaCha12Rng::seed_from_u64(seed)));
    w.insert_resource(RngSeed(seed));
    w.init_resource::<GameLog>();
    w.init_resource::<UseQueue>();
    w.insert_resource(PlayerName { what: "TESTER".into() });
    initialize_world(&mut w);
    w
}

fn player(w: &mut World) -> Entity {
    w.query_filtered::<Entity, With<Player>>().single(w)
}

/// Simulate the inventory "Use" action, exactly like the engine does.
fn use_item(w: &mut World, user: Entity, item: Entity) {
    let idx = w.get_mut::<Backpack>(user).unwrap().items.iter().position(|&e| e == item);
    if let Some(i) = idx {
        w.get_mut::<Backpack>(user).unwrap().items.remove(i);
        w.resource_mut::<UseQueue>().uses.push(WantsToUse { user, item, target: None, slot_idx: Some(i) });
    }
    item_system(w);
}

fn stash(w: &mut World, user: Entity, item: Entity) {
    w.entity_mut(item).remove::<Position>();
    w.get_mut::<Backpack>(user).unwrap().items.push(item);
}

#[test]
fn reading_magic_mapping_arms_the_reveal_and_the_sweep_maps_every_tile() {
    let mut w = test_world(7);
    let p = player(&mut w);

    // Fresh floor: only the starting room is remembered.
    let known_before = w.get::<Viewshed>(p).unwrap().revealed_tiles.count_ones(..);
    assert!(known_before < MAP_TILE_COUNT, "should not start with the whole map known");

    let scroll = w.spawn(ScrollBundle::magic_mapping(Position { x: 0, y: 0 })).id();
    stash(&mut w, p, scroll);
    use_item(&mut w, p, scroll);

    // The scroll is consumed and identified, and the wipe is armed but hasn't
    // committed anything yet — that is the engine's job, frame by frame.
    assert!(w.get::<Backpack>(p).unwrap().items.iter().all(|&e| e != scroll));
    assert!(w.resource::<Identified>().scrolls.contains(&ScrollEffect::MagicMapping));
    assert!(w.resource::<MagicMapReveal>().active, "reveal should be armed");
    assert_eq!(
        w.get::<Viewshed>(p).unwrap().revealed_tiles.count_ones(..),
        known_before,
        "no tiles revealed until the sweep runs"
    );

    // Drive the sweep the way the engine does: one row per frame.
    let mut frames = 0;
    while magic_map_reveal_step(&mut w) {
        frames += 1;
        assert!(frames <= MAP_HEIGHT, "sweep must terminate within one pass");
    }

    // Every tile on the floor is now in the player's memory, and the resource
    // has switched itself back off.
    assert!(!w.resource::<MagicMapReveal>().active);
    let vs = w.get::<Viewshed>(p).unwrap();
    for y in 0..MAP_HEIGHT {
        for x in 0..MAP_WIDTH {
            assert!(
                vs.revealed_tiles.contains(tile_index(x, y)),
                "tile ({x},{y}) should be revealed after magic mapping"
            );
        }
    }

    let log = w.resource::<GameLog>();
    assert!(log.history.iter().any(|m| m.contains("The dungeon's shape springs into your mind")));
}

#[test]
fn magic_map_reveal_step_is_a_noop_when_nothing_is_armed() {
    let mut w = test_world(3);
    assert!(!magic_map_reveal_step(&mut w));
    assert!(!w.resource::<MagicMapReveal>().active);
}
