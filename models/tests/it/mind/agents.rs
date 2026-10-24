//! Agents: every mob perceives only while it stands in the player's view, and
//! a rule set turns what it perceives into one action. Out of view it does
//! nothing, unless it is aggravated (it heads for the shriek) or it is the
//! player's Helper (it heels).

use crate::common;
use crate::common::monster;

use bevy_ecs::prelude::*;
use fixedbitset::FixedBitSet;
use models::*;

fn test_world(seed: u64) -> World {
    let mut w = World::new();
    w.insert_resource(GameRng(ChaCha12Rng::seed_from_u64(seed)));
    w.insert_resource(RngSeed(seed));
    w.init_resource::<GameLog>();
    w.init_resource::<UseQueue>();
    w.init_resource::<ThrowQueue>();
    w.init_resource::<AttackQueue>();
    w.init_resource::<Ending>();
    w.init_resource::<PlayerTempo>();
    w.insert_resource(PlayerName {
        what: "TESTER".into(),
    });
    initialize_world(&mut w);
    let strays: Vec<Entity> = w
        .query_filtered::<Entity, (With<Mob>, Without<Player>)>()
        .iter(&w)
        .collect();
    for e in strays {
        w.despawn(e);
    }
    w
}

fn player(w: &mut World) -> Entity {
    w.query_filtered::<Entity, With<Player>>().single(w)
}

fn player_pos(w: &mut World) -> Position {
    let p = player(w);
    *w.get::<Position>(p).unwrap()
}

fn run_visibility(w: &mut World) {
    let mut s = Schedule::default();
    s.add_systems(visibility_system);
    s.run(w);
}

fn pos(w: &World, e: Entity) -> Position {
    *w.get::<Position>(e).unwrap()
}

/// A walkable tile `dx` east of the player.
fn east_of_player(w: &mut World, dx: u16) -> Position {
    let p = player_pos(w);
    let at = Position {
        x: p.x + dx,
        y: p.y,
    };
    assert!(
        w.resource::<Map>().walkable(at.x, at.y, false),
        "fixture tile {dx} east of the player is not floor"
    );
    at
}

/// A hand-built floor: a room at x 5..=9, a door at x 10 and a corridor east of
/// it along row 5. Returns the world with the player standing in the corridor.
fn room_and_corridor(w: &mut World) {
    let mut map = Map {
        tiles: vec![TileType::Wall; MAP_TILE_COUNT],
        dark: FixedBitSet::with_capacity(MAP_TILE_COUNT),
        special: vec![None; MAP_TILE_COUNT],
        level: None,
    };
    for y in 4..=6 {
        for x in 5..=9 {
            map.tiles[tile_index(x, y)] = TileType::Room;
        }
    }
    map.tiles[tile_index(10, 5)] = TileType::Door;
    for x in 11..=16 {
        map.tiles[tile_index(x, 5)] = TileType::Passage;
    }
    w.insert_resource(map);
    let p = player(w);
    *w.get_mut::<Position>(p).unwrap() = Position { x: 14, y: 5 };
    w.get_mut::<Viewshed>(p).unwrap().dirty = true;
}

#[test]
fn a_helper_is_not_room_leashed() {
    let mut w = test_world(4);
    room_and_corridor(&mut w);
    let pal = monster::plain_monster(&mut w, "rat", Position { x: 9, y: 5 });
    recruit(&mut w, pal);
    run_visibility(&mut w);
    ai(&mut w);
    assert_eq!(
        pos(&w, pal),
        Position { x: 10, y: 5 },
        "the helper would not leave the room"
    );
}

#[test]
fn aggravation_survives_a_save_and_an_old_save_still_loads() {
    let mut w = test_world(11);
    let at = east_of_player(&mut w, 2);
    let a = monster::plain_monster(&mut w, "rat", at);
    w.entity_mut(a).insert(Aggravated { tx: 3, ty: 4 });
    let at = east_of_player(&mut w, 3);
    let b = monster::plain_monster(&mut w, "bat", at);
    w.get_mut::<Mob>(b).unwrap().movement_type = MovementType::Aggravated { tx: 7, ty: 8 };

    let save = common::SaveFile::new("aggravated");
    save_game(&mut w, save.path()).unwrap();
    let mut w2 = World::new();
    w2.insert_resource(GameRng(ChaCha12Rng::seed_from_u64(12)));
    w2.insert_resource(RngSeed(12));
    w2.init_resource::<GameLog>();
    w2.insert_resource(PlayerName { what: "X".into() });
    load_game(&mut w2, save.path()).unwrap();

    let mut found: Vec<(u16, u16)> = w2
        .query::<(&Mob, &Aggravated)>()
        .iter(&w2)
        .map(|(m, ag)| {
            assert!(
                matches!(m.movement_type, MovementType::Chase),
                "loaded with the retired movement type"
            );
            (ag.tx, ag.ty)
        })
        .collect();
    found.sort();
    assert_eq!(found, vec![(3, 4), (7, 8)]);
}
