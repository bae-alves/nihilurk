use bevy_ecs::prelude::*;
use models::*;

fn test_world(seed: u64) -> World {
    let mut w = World::new();
    w.insert_resource(GameRng(ChaCha12Rng::seed_from_u64(seed)));
    w.insert_resource(RngSeed(seed));
    w.init_resource::<GameLog>();
    w.insert_resource(PlayerName { what: "TESTER".into() });
    initialize_world(&mut w);
    w
}

fn player(w: &mut World) -> Entity {
    w.query_filtered::<Entity, With<Player>>().single(w)
}

#[test]
fn descend_generates_new_floor_and_heals() {
    let mut w = test_world(7);
    let p = player(&mut w);

    // Wound the player and confirm they start on the upstairs.
    w.get_mut::<Fighter>(p).unwrap().hp = 4;
    let start = *w.get::<Position>(p).unwrap();
    assert_eq!(w.resource::<Map>().tile(start.x, start.y), TileType::Upstairs);

    // Walk to the downstairs.
    let down = w
        .resource::<Map>()
        .tiles
        .iter()
        .position(|&t| t == TileType::Downstairs)
        .unwrap();
    let (dx, dy) = ((down % MAP_WIDTH as usize) as u16, (down / MAP_WIDTH as usize) as u16);
    w.get_mut::<Position>(p).unwrap().x = dx;
    w.get_mut::<Position>(p).unwrap().y = dy;

    let old_tiles = w.resource::<Map>().tiles.clone();
    assert!(change_level(&mut w, true));

    assert_eq!(w.resource::<Depth>().what, 2);
    assert_ne!(w.resource::<Map>().tiles, old_tiles, "a new floor was generated");
    // Healed 50% of max (30) -> 4 + 15 = 19.
    assert_eq!(w.get::<Fighter>(p).unwrap().hp, 19);
    // Player is back on an upstairs in the new floor's first room.
    let np = *w.get::<Position>(p).unwrap();
    assert_eq!(w.resource::<Map>().tile(np.x, np.y), TileType::Upstairs);
    // No floor items or monsters carried over (backpack wand stays).
    assert_eq!(w.query::<&Mob>().iter(&w).count() <= 3, true);
    assert_eq!(w.query_filtered::<&Backpack, With<Player>>().single(&w).items.len(), 1);
}

#[test]
fn cannot_descend_without_stairs() {
    let mut w = test_world(7);
    let p = player(&mut w);
    // Move somewhere that is not a staircase.
    let plain = w
        .resource::<Map>()
        .tiles
        .iter()
        .position(|&t| t == TileType::Room)
        .unwrap();
    w.get_mut::<Position>(p).unwrap().x = (plain % MAP_WIDTH as usize) as u16;
    w.get_mut::<Position>(p).unwrap().y = (plain / MAP_WIDTH as usize) as u16;

    let before = w.resource::<Map>().tiles.clone();
    assert!(!change_level(&mut w, true));
    assert_eq!(w.resource::<Depth>().what, 1);
    assert_eq!(w.resource::<Map>().tiles, before);
    assert!(w.resource::<GameLog>().history.last().unwrap().contains("cannot go down"));
}
