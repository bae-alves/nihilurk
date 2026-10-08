//! Fencer carries the number of attacks a fencer throws, and the grant that
//! lends it names the number.

mod common;
#[path = "common/monster.rs"]
mod monster;

use bevy_ecs::prelude::*;
use models::constants::items::ESTOC_NUMBER_OF_ATTACKS;
use models::constants::lurk::NUMBER_OF_ATTACKS as LURK_NUMBER_OF_ATTACKS;
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
    w.insert_resource(StartingBody(Body::Lurk));
    initialize_world(&mut w);
    w
}

fn player(w: &mut World) -> Entity {
    w.query_filtered::<Entity, With<Player>>().single(w)
}

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
fn a_counted_grant_lends_its_number_and_the_ledger_remembers_it() {
    let mut w = World::new();
    let e = w.spawn_empty().id();
    lend(&mut w, e, Grant::counted::<Fencer>(3), Lifetime::Permanent);

    assert!(w.get::<Fencer>(e).is_some());
    assert_eq!(count_of(&w, e, Grant::of::<Fencer>()), Some(3));
    assert_eq!(effects_of(&w, e)[0].count, 3);
}

#[test]
fn a_loaded_ledger_row_brings_its_number_back() {
    let mut w = World::new();
    let e = w.spawn_empty().id();
    let held = [Held {
        id: "fencer",
        lifetime: Lifetime::Permanent,
        count: 3,
    }];
    attach_effects(&mut w.entity_mut(e), &held);

    assert!(w.get::<Fencer>(e).is_some());
    assert_eq!(count_of(&w, e, Grant::of::<Fencer>()), Some(3));
}

#[test]
fn the_highest_count_among_the_sources_wins() {
    let mut w = World::new();
    let e = w.spawn_empty().id();
    lend(&mut w, e, Grant::counted::<Fencer>(2), Lifetime::Permanent);
    lend(&mut w, e, Grant::counted::<Fencer>(3), Lifetime::Floor);
    lend(&mut w, e, Grant::counted::<Fencer>(1), Lifetime::Floor);
    assert_eq!(count_of(&w, e, Grant::of::<Fencer>()), Some(3));
}

#[test]
fn the_estoc_and_the_lurk_lend_their_own_counts() {
    let mut w = lurk_world(3);
    let p = player(&mut w);
    assert_eq!(
        count_of(&w, p, Grant::of::<Fencer>()),
        Some(LURK_NUMBER_OF_ATTACKS)
    );

    let at = beside_player(&mut w);
    let estoc = spawn_named(&mut w, "estoc", at).unwrap();
    let grants = w.get::<Grants>(estoc).unwrap().0;
    let fencer = grants
        .iter()
        .find(|g| g.effect_id() == Some("fencer"))
        .unwrap();
    let mut probe = World::new();
    let e = probe.spawn_empty().id();
    lend(&mut probe, e, *fencer, Lifetime::Permanent);
    assert_eq!(
        count_of(&probe, e, Grant::of::<Fencer>()),
        Some(ESTOC_NUMBER_OF_ATTACKS)
    );
}

#[test]
fn a_blow_is_thrown_as_many_times_as_the_fencer_has_attacks() {
    let swings = |count: u8| -> usize {
        let mut total = 0;
        for seed in 0..20 {
            let mut w = lurk_world(seed);
            let p = player(&mut w);
            let at = beside_player(&mut w);
            let prey = monster::monster(&mut w, "test prey", at);
            w.get_mut::<Fighter>(prey).unwrap().max_hp = 9999;
            w.get_mut::<Fighter>(prey).unwrap().hp = 9999;
            revoke(&mut w, p, Grant::of::<Fencer>());
            lend(
                &mut w,
                p,
                Grant::counted::<Fencer>(count),
                Lifetime::Permanent,
            );
            let before = w.resource::<GameLog>().history.len();
            melee_attack(&mut w, p, prey);
            total += w.resource::<GameLog>().history.len() - before;
        }
        total
    };
    assert_eq!(swings(3), 3 * swings(1));
}

#[test]
fn a_full_set_of_conditions_never_trades_places_with_a_fencer() {
    let mut w = World::new();
    let e = w.spawn_empty().id();
    let turns = Lifetime::Turns(5);
    for hold in [
        Grant::of::<Asleep>(),
        Grant::of::<Pinned>(),
        Grant::of::<Rooted>(),
    ] {
        assert!(lend(&mut w, e, hold, turns));
    }
    assert!(
        !lend(&mut w, e, Grant::of::<Clamped>(), turns),
        "a fourth condition is refused"
    );

    assert!(lend(
        &mut w,
        e,
        Grant::counted::<Fencer>(2),
        Lifetime::Permanent
    ));
    assert_eq!(count_of(&w, e, Grant::of::<Fencer>()), Some(2));
    for hold in [
        Grant::of::<Asleep>(),
        Grant::of::<Pinned>(),
        Grant::of::<Rooted>(),
    ] {
        assert!(
            hold.probe(&w, e),
            "lending a fencer knocked a condition out"
        );
    }

    let f = w.spawn_empty().id();
    lend(&mut w, f, Grant::counted::<Fencer>(2), Lifetime::Permanent);
    for hold in [
        Grant::of::<Asleep>(),
        Grant::of::<Pinned>(),
        Grant::of::<Rooted>(),
    ] {
        assert!(
            lend(&mut w, f, hold, turns),
            "a fencer used up a condition slot"
        );
    }
}
