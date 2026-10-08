//! CHARGE!: Shift + a direction with a creature in view closes the gap and
//! strikes, and leaves the charger VULN (+25% on what hits them) for the
//! creature's turn that follows.

use bevy_ecs::prelude::*;
use bevy_ecs::schedule::Schedule;
use models::*;

#[path = "common/monster.rs"]
mod monster;

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
fn a_creature_ahead_makes_the_press_a_charge() {
    let (mut w, _) = arena(1);
    let orc = put_monster(&mut w, 18, 9);
    match fast_move_plan(&mut w, 1, 0) {
        FastMovePlan::Charge(t) => assert_eq!(t, orc),
        _ => panic!("expected a charge"),
    }
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
fn a_creature_next_door_is_not_a_charge() {
    let (mut w, _) = arena(1);
    put_monster(&mut w, 13, 9);
    assert!(matches!(
        fast_move_plan(&mut w, 1, 0),
        FastMovePlan::MonsterInSight
    ));
}

#[test]
fn a_wall_in_the_line_is_not_a_charge() {
    let (mut w, _) = arena(1);
    put_monster(&mut w, 18, 9);
    w.resource_mut::<Map>().tiles[tile_index(15, 9)] = TileType::Wall;
    assert!(matches!(
        fast_move_plan(&mut w, 1, 0),
        FastMovePlan::MonsterInSight
    ));
}

#[test]
fn the_nearest_creature_in_the_wedge_is_the_one_charged() {
    let (mut w, _) = arena(1);
    let near = put_monster(&mut w, 16, 10);
    put_monster(&mut w, 22, 9);
    match fast_move_plan(&mut w, 1, 0) {
        FastMovePlan::Charge(t) => assert_eq!(t, near),
        _ => panic!("expected a charge"),
    }
}

#[test]
fn charging_lands_beside_the_target_hits_it_and_leaves_you_vuln() {
    let (mut w, p) = arena(1);
    let orc = put_monster(&mut w, 18, 9);
    w.get_mut::<Fighter>(p).unwrap().power = 20;

    assert!(charge(&mut w, orc));

    let at = *w.get::<Position>(p).unwrap();
    assert_eq!((at.x, at.y), (17, 9));
    assert!(w.get::<Fighter>(orc).unwrap().hp < 100, "the charge struck");
    assert!(w.get::<Vuln>(p).is_some());
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
fn a_vuln_target_takes_a_quarter_more_from_a_monster() {
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
    assert!(plain >= 4, "the fixture should hit hard enough to see 25%");
    assert_eq!(swing(true), (plain * 5 + 2) / 4);
}

#[test]
fn vuln_does_nothing_for_a_blow_the_player_lands() {
    let swing = |vuln: bool| {
        let (mut w, p) = arena(7);
        let orc = put_monster(&mut w, 13, 9);
        w.get_mut::<Fighter>(p).unwrap().power = 40;
        if vuln {
            lend(&mut w, p, Grant::of::<Vuln>(), Lifetime::Turns(2));
        }
        melee_attack(&mut w, p, orc);
        w.get::<Fighter>(orc).unwrap().hp
    };
    assert_eq!(swing(true), swing(false));
}

#[test]
fn only_a_creature_that_charges_can_charge() {
    let (mut w, p) = arena(1);
    put_monster(&mut w, 18, 9);
    revoke(&mut w, p, Grant::of::<Charges>());
    assert!(matches!(
        fast_move_plan(&mut w, 1, 0),
        FastMovePlan::MonsterInSight
    ));
}

#[test]
fn six_tiles_off_is_in_range_and_seven_is_not() {
    let (mut w, _) = arena(1);
    let orc = put_monster(&mut w, 18, 9);
    assert!(matches!(
        fast_move_plan(&mut w, 1, 0),
        FastMovePlan::Charge(_)
    ));
    w.get_mut::<Position>(orc).unwrap().x = 19;
    assert!(matches!(
        fast_move_plan(&mut w, 1, 0),
        FastMovePlan::MonsterInSight
    ));
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
fn landing_on_a_trap_springs_it_and_jumping_over_one_does_not() {
    let (mut w, _) = arena(1);
    let orc = put_monster(&mut w, 18, 9);
    w.spawn(TrapBundle::trapdoor(Position { x: 14, y: 9 }));
    assert!(matches!(
        fast_move_plan(&mut w, 1, 0),
        FastMovePlan::Charge(_)
    ));
    charge(&mut w, orc);
    trap_system(&mut w);
    assert_eq!(w.resource::<Depth>().what, 1, "the trap was jumped over");

    let (mut w, _) = arena(1);
    let orc = put_monster(&mut w, 18, 9);
    w.spawn(TrapBundle::trapdoor(Position { x: 17, y: 9 }));
    charge(&mut w, orc);
    trap_system(&mut w);
    assert_eq!(w.resource::<Depth>().what, 2, "the trap was landed on");
}
