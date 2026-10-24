//! Ice cubes: what a cold kill leaves behind, and what happens when the player
//! boots one at the nearest weakest foe.

use crate::common::monster;

use bevy_ecs::prelude::*;
use models::constants::traps::{TRICK_SHOT_DAMAGE_DICE, TRICK_SHOT_DAMAGE_SIDES};
use models::*;

const FIRE_IMMUNITY: &[Grant] = &[Grant::of::<FireImmune>()];

fn test_world(seed: u64) -> World {
    let mut w = World::new();
    w.insert_resource(GameRng(ChaCha12Rng::seed_from_u64(seed)));
    w.insert_resource(RngSeed(seed));
    w.init_resource::<GameLog>();
    w.init_resource::<UseQueue>();
    w.init_resource::<SpellQueue>();
    w.init_resource::<AttackQueue>();
    w.init_resource::<Ending>();
    w.init_resource::<PlayerTempo>();
    w.insert_resource(PlayerName {
        what: "TESTER".into(),
    });
    initialize_world(&mut w);
    let natives: Vec<Entity> = w
        .query_filtered::<Entity, (With<Mob>, Without<Player>)>()
        .iter(&w)
        .collect();
    for e in natives {
        w.entity_mut(e).despawn();
    }
    w
}

fn player(w: &mut World) -> Entity {
    w.query_filtered::<Entity, With<Player>>().single(w)
}

fn open_tiles_near_player(w: &mut World, n: usize) -> Vec<Position> {
    let p = player(w);
    let here = *w.get::<Position>(p).unwrap();
    let map = w.resource::<Map>();
    let mut out = Vec::new();
    for r in 1..6i32 {
        for dy in -r..=r {
            for dx in -r..=r {
                let (x, y) = (here.x as i32 + dx, here.y as i32 + dy);
                if x >= 0 && y >= 0 && !map.blocks(x as u16, y as u16) {
                    let pos = Position {
                        x: x as u16,
                        y: y as u16,
                    };
                    if pos != here && !out.contains(&pos) {
                        out.push(pos);
                    }
                }
            }
        }
        if out.len() >= n {
            break;
        }
    }
    out.truncate(n);
    out
}

fn cold_kill(w: &mut World, victim: Entity) {
    let p = player(w);
    let wand = spawn_named(w, "wand of cold", Position { x: 0, y: 0 }).unwrap();
    w.entity_mut(wand).remove::<Position>();
    w.get_mut::<Backpack>(p).unwrap().items.push(wand);
    let at = *w.get::<Position>(victim).unwrap();
    w.get_mut::<Fighter>(victim).unwrap().hp = 1;
    w.resource_mut::<UseQueue>().uses.push(WantsToUse {
        user: p,
        item: wand,
        target: Some(at),
        slot_idx: Some(0),
    });
    item_system(w);
}

fn cubes(w: &mut World) -> Vec<(Entity, Position)> {
    w.query_filtered::<(Entity, &Position), With<IceCube>>()
        .iter(w)
        .map(|(e, p)| (e, *p))
        .collect()
}

#[test]
fn a_kicked_cube_that_kills_freezes_its_victim_in_turn() {
    let mut w = test_world(5);
    let spots = open_tiles_near_player(&mut w, 2);
    let p = player(&mut w);
    let doomed = monster::monster(&mut w, "orc", spots[0]);
    let next = monster::monster(&mut w, "rat", spots[1]);
    cold_kill(&mut w, doomed);
    w.get_mut::<Fighter>(next).unwrap().hp = 1;
    let (cube, _) = cubes(&mut w)[0];

    kick_ice_cube(&mut w, p, cube);

    let found = cubes(&mut w);
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].1, spots[1]);
    assert!(w.get_entity(next).is_none());
}

#[test]
fn a_kicked_cube_with_nothing_to_hit_flies_off_and_shatters() {
    let mut w = test_world(5);
    let spots = open_tiles_near_player(&mut w, 1);
    let p = player(&mut w);
    let doomed = monster::monster(&mut w, "orc", spots[0]);
    cold_kill(&mut w, doomed);
    let (cube, _) = cubes(&mut w)[0];

    assert!(kick_ice_cube(&mut w, p, cube), "the kick spends the turn");

    assert!(cubes(&mut w).is_empty());
}

#[test]
fn a_shot_cube_bursts_into_cold_that_freezes_what_it_kills() {
    let mut w = test_world(5);
    let spots = open_tiles_near_player(&mut w, 3);
    let p = player(&mut w);
    let doomed = monster::monster(&mut w, "orc", spots[0]);
    cold_kill(&mut w, doomed);
    let (cube, at) = cubes(&mut w)[0];
    let near: Vec<Position> = open_tiles_near_player(&mut w, 40)
        .into_iter()
        .filter(|s| *s != at && chebyshev(*s, at) <= 1)
        .collect();
    let weak = monster::monster(&mut w, "rat", near[0]);
    let hardy = monster::monster(&mut w, "troll", near[1]);
    w.get_mut::<Fighter>(weak).unwrap().hp = 1;

    assert_eq!(detonate_at(&mut w, at, Some(p)), Some(TrickShot::IceCube));

    assert!(w.get_entity(cube).is_none(), "the cube shattered");
    assert!(w.get_entity(weak).is_none(), "the cold killed the rat");
    assert!(
        cubes(&mut w).iter().any(|&(_, c)| c == near[0]),
        "and froze it"
    );
    let lost = 100 - w.get::<Fighter>(hardy).unwrap().hp;
    assert!(
        (TRICK_SHOT_DAMAGE_DICE..=TRICK_SHOT_DAMAGE_DICE * TRICK_SHOT_DAMAGE_SIDES).contains(&lost),
        "lost {lost}"
    );
}

fn zap(w: &mut World, name: &str, at: Position) {
    let p = player(w);
    let wand = spawn_named(w, name, Position { x: 0, y: 0 }).unwrap();
    w.entity_mut(wand).remove::<Position>();
    w.resource_mut::<UseQueue>().uses.push(WantsToUse {
        user: p,
        item: wand,
        target: Some(at),
        slot_idx: None,
    });
    item_system(w);
}

fn tile_at_distance(w: &mut World, from: Position, d: i32, not: &[Position]) -> Position {
    open_tiles_near_player(w, 80)
        .into_iter()
        .find(|s| chebyshev(*s, from) == d && !not.contains(s))
        .expect("an open tile at that distance")
}

#[test]
fn fire_on_a_cube_doubles_its_radius_and_damage() {
    let mut w = test_world(5);
    let spot = open_tiles_near_player(&mut w, 1)[0];
    let orc = monster::monster(&mut w, "orc", spot);
    cold_kill(&mut w, orc);
    let (_, at) = cubes(&mut w)[0];
    let far = tile_at_distance(&mut w, at, 2, &[]);
    let bystander = monster::monster(&mut w, "salamander", far);
    grant_all(&mut w, bystander, FIRE_IMMUNITY);

    zap(&mut w, "wand of fire", at);

    let lost = 100 - w.get::<Fighter>(bystander).unwrap().hp;
    let scorched =
        2 * TRICK_SHOT_DAMAGE_DICE..=2 * TRICK_SHOT_DAMAGE_DICE * TRICK_SHOT_DAMAGE_SIDES;
    assert!(scorched.contains(&lost), "lost {lost}");
}

#[test]
fn dying_to_a_cube_is_recorded_as_such() {
    let mut w = test_world(5);
    let p = player(&mut w);
    let here = *w.get::<Position>(p).unwrap();
    let at = open_tiles_near_player(&mut w, 1)[0];
    let orc = monster::monster(&mut w, "orc", at);
    cold_kill(&mut w, orc);
    let suit = equipped_in(&w, p, Slot::Body).unwrap();
    destroy_worn(&mut w, p, &[suit]);
    w.get_mut::<Fighter>(p).unwrap().hp = 1;
    assert!(chebyshev(here, at) <= 1);

    detonate_at(&mut w, at, None);

    let ending = w.resource::<Ending>();
    assert!(ending.player_dead);
    assert_eq!(ending.cause, "Killed by an ice cube");
}

#[test]
fn a_bolt_sets_off_a_cube_it_passes_through() {
    let mut w = test_world(5);
    let p = player(&mut w);
    let here = *w.get::<Position>(p).unwrap();
    let spot = open_tiles_near_player(&mut w, 1)[0];
    let orc = monster::monster(&mut w, "orc", spot);
    cold_kill(&mut w, orc);
    let (cube, _) = cubes(&mut w)[0];
    let beyond = Position {
        x: (spot.x as i32 * 2 - here.x as i32) as u16,
        y: (spot.y as i32 * 2 - here.y as i32) as u16,
    };

    zap(&mut w, "wand of magic missile", beyond);

    assert!(w.get_entity(cube).is_none());
}

#[test]
fn a_dart_spell_sets_off_a_cube_where_it_lands() {
    let mut w = test_world(5);
    let p = player(&mut w);
    let spot = open_tiles_near_player(&mut w, 1)[0];
    let orc = monster::monster(&mut w, "orc", spot);
    cold_kill(&mut w, orc);
    let (cube, at) = cubes(&mut w)[0];
    w.resource_mut::<SpellQueue>().spells.push(WantsToCast {
        user: p,
        spell: spell_row("Thunderbolt").unwrap(),
        target: at,
    });
    spell_system(&mut w);

    assert!(w.get_entity(cube).is_none());
}
