use bevy_ecs::prelude::*;
use models::*;
use std::collections::HashSet;

fn test_world(seed: u64) -> World {
    let mut w = World::new();
    w.insert_resource(GameRng(ChaCha12Rng::seed_from_u64(seed)));
    w.insert_resource(RngSeed(seed));
    w.init_resource::<GameLog>();
    w.insert_resource(PlayerName {
        what: "TESTER".into(),
    });
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

    // Wound the player, spend some magic, and confirm they start on the upstairs.
    w.get_mut::<Fighter>(p).unwrap().hp = 4;
    w.get_mut::<Magic>(p).unwrap().points = 1;
    let start = *w.get::<Position>(p).unwrap();
    assert_eq!(
        w.resource::<Map>().tile(start.x, start.y),
        TileType::Upstairs
    );

    // Walk to the downstairs.
    let down = w
        .resource::<Map>()
        .tiles
        .iter()
        .position(|&t| t == TileType::Downstairs)
        .unwrap();
    let (dx, dy) = (
        (down % MAP_WIDTH as usize) as u16,
        (down / MAP_WIDTH as usize) as u16,
    );
    w.get_mut::<Position>(p).unwrap().x = dx;
    w.get_mut::<Position>(p).unwrap().y = dy;

    let old_floor_entities: HashSet<Entity> = w
        .query_filtered::<Entity, (With<Position>, Without<Player>)>()
        .iter(&w)
        .collect();
    let old_tiles = w.resource::<Map>().tiles.clone();
    assert!(change_level(&mut w, true));

    assert_eq!(w.resource::<Depth>().what, 2);
    assert_ne!(
        w.resource::<Map>().tiles,
        old_tiles,
        "a new floor was generated"
    );
    let fighter = w.get::<Fighter>(p).unwrap();
    let expected_hp = 4 + fighter.max_hp / constants::progression::DESCENT_HEAL_DIVISOR;
    assert_eq!(fighter.hp, expected_hp);
    // Magic is fully restored on arrival.
    let magic = w.get::<Magic>(p).unwrap();
    assert_eq!(magic.points, magic.max_points);
    // Player is back on an upstairs in the new floor's first room.
    let np = *w.get::<Position>(p).unwrap();
    assert_eq!(w.resource::<Map>().tile(np.x, np.y), TileType::Upstairs);
    // No floor items or monsters carried over (the starting kit stays).
    assert!(
        old_floor_entities
            .iter()
            .all(|&entity| w.get_entity(entity).is_none()),
        "entities from the old floor carried over"
    );
    assert_eq!(
        w.query_filtered::<&Backpack, With<Player>>()
            .single(&w)
            .items
            .len(),
        5
    );
}

#[test]
fn a_hasted_or_slowed_player_finds_their_tempo_on_the_next_floor() {
    let mut w = test_world(7);
    let p = player(&mut w);
    w.get_mut::<Speed>(p).unwrap().kind = SpeedKind::Fast;
    w.entity_mut(p).insert(Confused);

    let down = w
        .resource::<Map>()
        .tiles
        .iter()
        .position(|&t| t == TileType::Downstairs)
        .unwrap();
    w.get_mut::<Position>(p).unwrap().x = (down % MAP_WIDTH as usize) as u16;
    w.get_mut::<Position>(p).unwrap().y = (down / MAP_WIDTH as usize) as u16;
    assert!(change_level(&mut w, true));

    assert_eq!(
        w.get::<Speed>(p).unwrap().kind,
        SpeedKind::Normal,
        "the stairs wash out a haste"
    );
    assert!(w.get::<Confused>(p).is_none(), "and the dazzle");
    let log = &w.resource::<GameLog>().history;
    assert!(log.iter().any(|l| l == "You are no longer confused."));
}

/// Walk the player onto the current floor's down-stair and descend, repeating
/// until `Depth` reaches `target`.
fn descend_to(w: &mut World, target: u8) {
    let p = w.query_filtered::<Entity, With<Player>>().single(w);
    while w.resource::<Depth>().what < target {
        let down = w
            .resource::<Map>()
            .tiles
            .iter()
            .position(|&t| t == TileType::Downstairs)
            .expect("every floor above the last has a down-stair");
        w.get_mut::<Position>(p).unwrap().x = (down % MAP_WIDTH as usize) as u16;
        w.get_mut::<Position>(p).unwrap().y = (down / MAP_WIDTH as usize) as u16;
        assert!(change_level(w, true));
    }
}

#[test]
fn deepest_floor_swaps_the_downstairs_for_the_element() {
    let mut w = test_world(7);
    descend_to(&mut w, 13);

    // No way down remains on Depth 13.
    assert!(!w.resource::<Map>().tiles.contains(&TileType::Downstairs));

    // The Element of Yoord is lying on the floor where the down-stair would be.
    let elements: Vec<Entity> = w
        .query_filtered::<Entity, (With<Amulet>, With<Position>)>()
        .iter(&w)
        .collect();
    assert_eq!(
        elements.len(),
        1,
        "exactly one Element spawned on the floor"
    );

    // Standing on the down-stair spot: still can't descend (there is no stair).
    let epos = *w.get::<Position>(elements[0]).unwrap();
    let p = w.query_filtered::<Entity, With<Player>>().single(&w);
    *w.get_mut::<Position>(p).unwrap() = epos;
    assert!(!change_level(&mut w, true));
    assert_eq!(w.resource::<Depth>().what, 13);
}

#[test]
fn carrying_the_element_flips_the_staircases() {
    let mut w = test_world(7);
    descend_to(&mut w, 13);

    let p = w.query_filtered::<Entity, With<Player>>().single(&w);
    let element = w
        .query_filtered::<Entity, (With<Amulet>, With<Position>)>()
        .single(&w);

    // Pick it up.
    w.entity_mut(element).remove::<Position>();
    w.get_mut::<Backpack>(p).unwrap().items.push(element);

    // Down is now refused, wherever you stand — here, on Depth 13's up-stair
    // (there is no down-stair on this floor to begin with), so the message is
    // the plain refusal, not the Element's flavor text (see
    // `element_message_only_shows_on_the_actual_downstairs`).
    assert!(!change_level(&mut w, true));
    assert!(
        w.resource::<GameLog>()
            .history
            .last()
            .unwrap()
            .contains("cannot go down")
    );

    // Standing on the up-stair, `<` carries you back toward the surface.
    let up = w
        .resource::<Map>()
        .tiles
        .iter()
        .position(|&t| t == TileType::Upstairs)
        .unwrap();
    w.get_mut::<Position>(p).unwrap().x = (up % MAP_WIDTH as usize) as u16;
    w.get_mut::<Position>(p).unwrap().y = (up / MAP_WIDTH as usize) as u16;
    assert!(change_level(&mut w, false));
    assert_eq!(w.resource::<Depth>().what, 12);
    // The Element rode along in the pack.
    assert!(
        w.query_filtered::<&Backpack, With<Player>>()
            .single(&w)
            .items
            .iter()
            .any(|&e| w.get::<Amulet>(e).is_some())
    );
}

/// Walk the player onto the up-stair and climb, repeating until `Depth` is 1.
fn ascend_to_surface(w: &mut World) {
    let p = w.query_filtered::<Entity, With<Player>>().single(w);
    while w.resource::<Depth>().what > 1 {
        let up = w
            .resource::<Map>()
            .tiles
            .iter()
            .position(|&t| t == TileType::Upstairs)
            .unwrap();
        w.get_mut::<Position>(p).unwrap().x = (up % MAP_WIDTH as usize) as u16;
        w.get_mut::<Position>(p).unwrap().y = (up / MAP_WIDTH as usize) as u16;
        assert!(change_level(w, false));
    }
}

#[test]
fn climbing_the_last_stair_with_the_element_wins_the_run() {
    let mut w = test_world(7);
    w.init_resource::<Ending>();
    descend_to(&mut w, 13);

    let p = w.query_filtered::<Entity, With<Player>>().single(&w);
    let element = w
        .query_filtered::<Entity, (With<Amulet>, With<Position>)>()
        .single(&w);
    w.entity_mut(element).remove::<Position>();
    w.get_mut::<Backpack>(p).unwrap().items.push(element);

    ascend_to_surface(&mut w);
    assert_eq!(w.resource::<Depth>().what, 1);
    assert!(
        !w.resource::<Ending>().player_won,
        "not won until the final stair"
    );

    // Stand on the Depth-1 up-stair and take it.
    let up = w
        .resource::<Map>()
        .tiles
        .iter()
        .position(|&t| t == TileType::Upstairs)
        .unwrap();
    w.get_mut::<Position>(p).unwrap().x = (up % MAP_WIDTH as usize) as u16;
    w.get_mut::<Position>(p).unwrap().y = (up / MAP_WIDTH as usize) as u16;

    assert!(
        change_level(&mut w, false),
        "the final climb consumes a turn"
    );
    assert!(w.resource::<Ending>().player_won);
    assert_eq!(
        w.resource::<Depth>().what,
        1,
        "you leave the dungeon, depth is unchanged"
    );
}

#[test]
fn the_portal_never_wins_the_run() {
    let mut w = test_world(7);
    w.init_resource::<Ending>();
    descend_to(&mut w, 13);

    let p = w.query_filtered::<Entity, With<Player>>().single(&w);
    let element = w
        .query_filtered::<Entity, (With<Amulet>, With<Position>)>()
        .single(&w);
    w.entity_mut(element).remove::<Position>();
    w.get_mut::<Backpack>(p).unwrap().items.push(element);
    ascend_to_surface(&mut w);
    assert_eq!(w.resource::<Depth>().what, 1);

    // Sit off the stairs and let the Dungeon Lord's patience run out repeatedly.
    let plain = w
        .resource::<Map>()
        .tiles
        .iter()
        .position(|&t| t == TileType::Room)
        .unwrap();
    w.get_mut::<Position>(p).unwrap().x = (plain % MAP_WIDTH as usize) as u16;
    w.get_mut::<Position>(p).unwrap().y = (plain / MAP_WIDTH as usize) as u16;

    for _ in 0..5 {
        w.insert_resource(DungeonLord {
            idle_turns: DUNGEON_LORD_PATIENCE - 1,
        });
        dungeon_lord_system(&mut w);
    }
    assert!(
        !w.resource::<Ending>().player_won,
        "a portal cannot win the run"
    );
    assert_eq!(w.resource::<Depth>().what, 1);
}

#[test]
fn dungeon_lord_portal_shunts_the_dawdler_onward() {
    let mut w = test_world(7);
    w.insert_resource(DungeonLord {
        idle_turns: DUNGEON_LORD_PATIENCE - 2,
    });
    let p = w.query_filtered::<Entity, With<Player>>().single(&w);

    // Sit on plain floor, nowhere near a staircase.
    let plain = w
        .resource::<Map>()
        .tiles
        .iter()
        .position(|&t| t == TileType::Room)
        .unwrap();
    w.get_mut::<Position>(p).unwrap().x = (plain % MAP_WIDTH as usize) as u16;
    w.get_mut::<Position>(p).unwrap().y = (plain / MAP_WIDTH as usize) as u16;

    // One turn short: nothing happens.
    dungeon_lord_system(&mut w);
    assert_eq!(w.resource::<Depth>().what, 1);

    // Patience runs out: a portal drops the player to Depth 2.
    dungeon_lord_system(&mut w);
    assert_eq!(w.resource::<Depth>().what, 2);
    assert_eq!(w.resource::<DungeonLord>().idle_turns, 0);
    assert!(
        w.resource::<GameLog>()
            .history
            .last()
            .unwrap()
            .contains("portal")
    );
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
    assert!(
        w.resource::<GameLog>()
            .history
            .last()
            .unwrap()
            .contains("cannot go down")
    );
}

/// Standing anywhere but the downstairs, carrying the Element, `>` still
/// refuses — but the message should say you're not on a staircase, not blame
/// the Element for blocking a descent you were never lined up for. Mirrors
/// the tile-aware message `change_level` already gives for `<` without the
/// Element (see `cannot_ascend_without_the_dungeon_lords_blessing`, below).
#[test]
fn element_message_only_shows_on_the_actual_downstairs() {
    let mut w = test_world(7);
    let p = player(&mut w);
    let plain = w
        .resource::<Map>()
        .tiles
        .iter()
        .position(|&t| t == TileType::Room)
        .unwrap();
    w.get_mut::<Position>(p).unwrap().x = (plain % MAP_WIDTH as usize) as u16;
    w.get_mut::<Position>(p).unwrap().y = (plain / MAP_WIDTH as usize) as u16;
    let element = w
        .spawn((
            Amulet,
            Name {
                what: "the Element of Yoord".into(),
            },
        ))
        .id();
    w.get_mut::<Backpack>(p).unwrap().items.push(element);

    assert!(!change_level(&mut w, true));
    assert!(
        w.resource::<GameLog>()
            .history
            .last()
            .unwrap()
            .contains("cannot go down"),
        "not on the downstairs at all -- the generic refusal, not the Element flavor text"
    );
}

/// Standing on the actual downstairs while carrying the Element still gets the
/// flavor text -- only the message picked for every *other* tile changed.
#[test]
fn element_message_shows_on_the_actual_downstairs() {
    let mut w = test_world(7);
    let p = player(&mut w);
    let down = w
        .resource::<Map>()
        .tiles
        .iter()
        .position(|&t| t == TileType::Downstairs)
        .unwrap();
    w.get_mut::<Position>(p).unwrap().x = (down % MAP_WIDTH as usize) as u16;
    w.get_mut::<Position>(p).unwrap().y = (down / MAP_WIDTH as usize) as u16;
    let element = w
        .spawn((
            Amulet,
            Name {
                what: "the Element of Yoord".into(),
            },
        ))
        .id();
    w.get_mut::<Backpack>(p).unwrap().items.push(element);

    assert!(!change_level(&mut w, true));
    assert!(
        w.resource::<GameLog>()
            .history
            .last()
            .unwrap()
            .contains("seeks the sun")
    );
}

/// The Dungeon Lord's blocked-ascent flavor message only shows when actually
/// standing on the up-stair; anywhere else without the Element, `<` gives the
/// plain refusal instead.
#[test]
fn cannot_ascend_without_the_dungeon_lords_blessing() {
    let mut w = test_world(7);
    let p = player(&mut w);
    let down = w
        .resource::<Map>()
        .tiles
        .iter()
        .position(|&t| t == TileType::Downstairs)
        .unwrap();
    w.get_mut::<Position>(p).unwrap().x = (down % MAP_WIDTH as usize) as u16;
    w.get_mut::<Position>(p).unwrap().y = (down / MAP_WIDTH as usize) as u16;

    assert!(!change_level(&mut w, false));
    assert!(
        w.resource::<GameLog>()
            .history
            .last()
            .unwrap()
            .contains("cannot go up"),
        "standing on the downstairs, not the upstairs -- the generic refusal, not the Dungeon Lord's"
    );
}
