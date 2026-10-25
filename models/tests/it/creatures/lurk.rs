//! The lurk: the other body written to be played. Quadruped, fanged, clawed,
//! furred — it brings nothing, wears nothing but rings, and gets stronger by
//! eating rather than by shopping.

use crate::common;
use crate::common::monster;

use crate::common::fixtures::spawn_named;
use bevy_ecs::prelude::*;
use models::*;

fn lurk_world(seed: u64) -> World {
    let mut w = World::new();
    w.insert_resource(GameRng(ChaCha12Rng::seed_from_u64(seed)));
    w.insert_resource(RngSeed(seed));
    w.init_resource::<GameLog>();
    w.init_resource::<SpellQueue>();
    w.init_resource::<AttackQueue>();
    w.init_resource::<Ending>();
    w.insert_resource(PlayerName {
        what: "TESTER".into(),
    });
    w.insert_resource(StartingBody(Body::Row(body_row("lurk").unwrap())));
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
    for (dx, dy) in [(1i32, 0i32), (-1, 0), (0, 1), (0, -1)] {
        let (nx, ny) = (here.x as i32 + dx, here.y as i32 + dy);
        if nx >= 0 && ny >= 0 && !map.blocks(nx as u16, ny as u16) {
            return Position {
                x: nx as u16,
                y: ny as u16,
            };
        }
    }
    panic!("the lurk is walled in");
}

#[test]
fn momentum_builds_on_the_lurk_itself() {
    let mut w = lurk_world(7);
    let p = player(&mut w);
    let at = beside_player(&mut w);
    let prey = monster::monster(&mut w, "test prey", at);
    w.get_mut::<Fighter>(prey).unwrap().max_hp = 99;
    w.get_mut::<Fighter>(prey).unwrap().hp = 99;

    assert!(w.get::<Momentum>(p).is_none());
    melee_attack(&mut w, p, prey);
    let built = w.get::<Momentum>(p).map_or(0, |m| m.0);
    assert!(built > 0, "a landed blow winds the next one up");

    equipment::reset_momentum(&mut w, p);
    assert_eq!(w.get::<Momentum>(p).map_or(0, |m| m.0), 0);
}

/// The four numbers on the status line, totalled — which is how many points
/// of growth the lurk is carrying, whichever of them it landed on.
fn stat_total(w: &mut World) -> i32 {
    let p = player(w);
    let f = w.get::<Fighter>(p).unwrap();
    let m = w.get::<Magic>(p).unwrap();
    f.max_hp + f.max_power + f.armor + i32::from(m.max_points)
}

#[test]
fn eating_grows_the_lurk_at_the_stated_odds() {
    let mut w = lurk_world(7);
    let before = stat_total(&mut w);

    const FEEDS: i32 = 400;
    let mut growths = 0;
    for _ in 0..FEEDS {
        let before_feed = stat_total(&mut w);
        body::feed(&mut w);
        growths += stat_total(&mut w) - before_feed;
    }
    let grown = stat_total(&mut w) - before;

    assert!(grown > 0, "feeding eventually grows the lurk");
    assert_eq!(grown, growths, "feeding does not lose or invent growth");
    assert!(
        w.resource::<GameLog>()
            .unread
            .iter()
            .any(|line| line.contains("FEAR THE LURK!"))
    );
}

#[test]
fn cancellation_takes_the_magic_not_the_creature() {
    let mut w = lurk_world(7);
    let p = player(&mut w);
    revoke_all(&mut w, p);

    assert!(w.get::<Stealthy>(p).is_none());
    assert!(w.get::<Lunges>(p).is_none());
    assert!(w.get::<Lurk>(p).is_some());
    let at = *w.get::<Position>(p).unwrap();
    let mail = spawn_named(&mut w, "ring mail", at).unwrap();
    assert!(!equipment::toggle_equipped(&mut w, p, mail));
    let save = common::SaveFile::new("cancelled-lurk");
    save_game(&mut w, save.path()).unwrap();
    let mut w2 = World::new();
    w2.insert_resource(GameRng(ChaCha12Rng::seed_from_u64(1)));
    w2.insert_resource(RngSeed(1));
    w2.init_resource::<GameLog>();
    w2.insert_resource(PlayerName {
        what: "TESTER".into(),
    });
    load_game(&mut w2, save.path()).unwrap();
    let p2 = player(&mut w2);
    assert!(w2.get::<Lurk>(p2).is_some());
}

#[test]
fn hasting_a_lurk_never_slows_it() {
    let mut w = lurk_world(7);
    let p = player(&mut w);
    assert_eq!(w.get::<Speed>(p).unwrap().kind, SpeedKind::Quick);

    for _ in 0..3 {
        shift_entity_speed(&mut w, p, true);
        assert_eq!(w.get::<Speed>(p).unwrap().kind, SpeedKind::Fast);
    }
}

#[test]
fn the_lurk_sees_its_stealth_range_on_floor_tiles_only() {
    use models::constants::rings::STEALTH_RANGE;
    let mut w = lurk_world(1);
    let p = player(&mut w);
    let here = *w.get::<Position>(p).unwrap();
    let door = Position {
        x: here.x + 1,
        y: here.y,
    };
    w.resource_mut::<Map>().tiles[tile_index(door.x, door.y)] = TileType::Door;
    let tiles = stealth_range_tiles(&w, p);
    assert!(!tiles.is_empty());
    assert!(!tiles.contains(&(door.x, door.y)), "a doorway is not floor");
    let map = w.resource::<Map>();
    for &(x, y) in &tiles {
        assert!(matches!(map.tile(x, y), TileType::Room | TileType::Passage));
        let reach = (x as i32 - here.x as i32)
            .abs()
            .max((y as i32 - here.y as i32).abs());
        assert!(reach <= STEALTH_RANGE);
    }
}

#[test]
fn the_lurk_shows_no_stealth_range_in_an_unlit_dark_room() {
    let mut w = lurk_world(1);
    let p = player(&mut w);
    let here = *w.get::<Position>(p).unwrap();
    w.resource_mut::<Map>()
        .dark
        .insert(tile_index(here.x, here.y));
    assert!(stealth_range_tiles(&w, p).is_empty());
}

#[test]
fn nihil_has_no_stealth_range_to_see() {
    let (w, p) = common::fresh_run(1);
    assert!(stealth_range_tiles(&w, p).is_empty());
}
