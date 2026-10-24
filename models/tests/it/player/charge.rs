//! CHARGE!: Shift + a direction with a creature in view closes the gap and
//! strikes, and leaves the charger VULN (+25% on what hits them) for the
//! creature's turn that follows.

use bevy_ecs::prelude::*;
use bevy_ecs::schedule::Schedule;
use models::constants::combat::VULN_DAMAGE_PERCENT;
use models::*;

use crate::common::monster;

fn resolve_visibility(w: &mut World) {
    let mut schedule = Schedule::default();
    schedule.add_systems(visibility_system);
    schedule.run(w);
}

/// One clean room, x 10..=24, y 5..=13, player at (12, 9), nothing else in it.
fn arena(seed: u64) -> (World, Entity) {
    let mut w = World::new();
    w.insert_resource(GameRng(ChaCha12Rng::seed_from_u64(seed)));
    w.insert_resource(RngSeed(seed));
    w.init_resource::<GameLog>();
    w.init_resource::<Ending>();
    w.init_resource::<FastMove>();
    w.insert_resource(PlayerName {
        what: "TESTER".into(),
    });
    initialize_world(&mut w);

    let clutter: Vec<Entity> = w
        .iter_entities()
        .filter(|e| !e.contains::<Player>() && (e.contains::<Mob>() || e.contains::<Item>()))
        .map(|e| e.id())
        .collect();
    for e in clutter {
        w.despawn(e);
    }
    {
        let mut map = w.resource_mut::<Map>();
        for t in map.tiles.iter_mut() {
            *t = TileType::Wall;
        }
        for y in 5..=13u16 {
            for x in 10..=24u16 {
                map.tiles[tile_index(x, y)] = TileType::Room;
            }
        }
    }
    let player = w.query_filtered::<Entity, With<Player>>().single(&w);
    *w.get_mut::<Position>(player).unwrap() = Position { x: 12, y: 9 };
    w.get_mut::<Viewshed>(player).unwrap().dirty = true;
    resolve_visibility(&mut w);
    (w, player)
}

fn put_monster(w: &mut World, x: u16, y: u16) -> Entity {
    let m = monster::monster(w, "orc", Position { x, y });
    let p = w.query_filtered::<Entity, With<Player>>().single(w);
    w.get_mut::<Viewshed>(p).unwrap().dirty = true;
    resolve_visibility(w);
    m
}

#[test]
fn only_a_creature_in_the_pressed_direction_is_charged() {
    let (mut w, _) = arena(1);
    put_monster(&mut w, 10, 9);
    assert!(matches!(
        fast_move_plan(&mut w, -1, 0),
        FastMovePlan::Charge(_)
    ));
    assert!(matches!(
        fast_move_plan(&mut w, 1, 0),
        FastMovePlan::MonsterInSight
    ));
}

#[test]
fn vuln_lasts_through_one_monster_phase_and_no_longer() {
    let (mut w, p) = arena(1);
    let orc = put_monster(&mut w, 18, 9);
    charge(&mut w, orc);

    tick_effects(&mut w);
    assert!(w.get::<Vuln>(p).is_some(), "still VULN while they swing");
    tick_effects(&mut w);
    assert!(w.get::<Vuln>(p).is_none(), "gone by the next turn");
}

#[test]
fn a_vuln_target_takes_the_vuln_share_more_from_a_monster() {
    let swing = |vuln: bool| {
        let (mut w, p) = arena(7);
        let orc = put_monster(&mut w, 13, 9);
        w.get_mut::<Fighter>(orc).unwrap().power = 40;
        w.get_mut::<Fighter>(p).unwrap().hp = 1000;
        w.get_mut::<Fighter>(p).unwrap().max_hp = 1000;
        if vuln {
            lend(&mut w, p, Grant::of::<Vuln>(), Lifetime::Turns(2));
        }
        melee_attack(&mut w, orc, p);
        1000 - w.get::<Fighter>(p).unwrap().hp
    };
    let plain = swing(false);
    assert!(
        plain >= 4,
        "the fixture should hit hard enough to see the extra"
    );
    assert_eq!(
        swing(true),
        plain + (plain * VULN_DAMAGE_PERCENT + 50) / 100
    );
}

#[test]
fn a_peaceful_spirit_is_not_charged_but_a_hostile_one_is() {
    let (mut w, _) = arena(1);
    let spirit = put_monster(&mut w, 18, 9);
    *w.get_mut::<Faction>(spirit).unwrap() = Faction::Spirits;
    w.insert_resource(SpiritsHostile(false));
    assert!(matches!(
        fast_move_plan(&mut w, 1, 0),
        FastMovePlan::MonsterInSight
    ));
    w.insert_resource(SpiritsHostile(true));
    assert!(matches!(
        fast_move_plan(&mut w, 1, 0),
        FastMovePlan::Charge(_)
    ));
}

#[test]
fn an_adjacent_foe_refuses_the_charge() {
    let (mut w, _) = arena(1);
    put_monster(&mut w, 18, 9);
    put_monster(&mut w, 13, 10);
    assert!(matches!(
        fast_move_plan(&mut w, 1, 0),
        FastMovePlan::ThickOfIt
    ));
}
