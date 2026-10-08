//! A suit of armour takes the blow that would have killed its wearer: it is
//! destroyed and the wearer is left on 1 HP. Only a cursed suit or one with a
//! plus on it will do; plain armour dies with its wearer. A lurk wears none,
//! so it starts one point up on every number instead.

use bevy_ecs::prelude::*;
use models::*;

#[path = "common/monster.rs"]
mod monster;

fn test_world(seed: u64) -> World {
    let mut w = World::new();
    w.insert_resource(GameRng(ChaCha12Rng::seed_from_u64(seed)));
    w.insert_resource(RngSeed(seed));
    w.init_resource::<GameLog>();
    w.init_resource::<AttackQueue>();
    w.insert_resource(PlayerName {
        what: "TESTER".into(),
    });
    initialize_world(&mut w);
    w
}

fn player(w: &mut World) -> Entity {
    w.query_filtered::<Entity, With<Player>>().single(w)
}

fn suit_up(w: &mut World, p: Entity) -> Entity {
    let mail = equipped_in(w, p, Slot::Body).expect("nihil starts in armour");
    w.get_mut::<Fighter>(p).unwrap().hp = 3;
    w.entity_mut(mail).insert(ArmorBonus(2));
    mail
}

#[test]
fn armour_takes_a_spell_that_would_kill() {
    let mut w = test_world(1);
    let p = player(&mut w);
    let mail = suit_up(&mut w, p);

    apply_hit(&mut w, p, Hit::magic(50), None);

    assert_eq!(w.get::<Fighter>(p).unwrap().hp, 1);
    assert!(!w.entities().contains(mail), "the armour survived the blow");
}

#[test]
fn armour_takes_a_blade_that_would_kill() {
    let mut w = test_world(1);
    let p = player(&mut w);
    let mail = suit_up(&mut w, p);
    w.get_mut::<Fighter>(p).unwrap().hp = 1;
    let here = *w.get::<Position>(p).unwrap();
    let orc = monster::monster(
        &mut w,
        "orc",
        Position {
            x: here.x + 1,
            y: here.y,
        },
    );

    w.get_mut::<Fighter>(orc).unwrap().power = 100;

    for _ in 0..200 {
        if !w.entities().contains(mail) {
            break;
        }
        resolve_attack(&mut w, orc, p);
        w.get_mut::<Fighter>(p).unwrap().hp = 1;
    }

    assert!(!w.entities().contains(mail), "no blow ever landed");
}

#[test]
fn a_blow_that_does_not_kill_spares_the_armour() {
    let mut w = test_world(1);
    let p = player(&mut w);
    let mail = suit_up(&mut w, p);

    apply_hit(&mut w, p, Hit::magic(1), None);

    assert_eq!(w.get::<Fighter>(p).unwrap().hp, 2);
    assert!(w.entities().contains(mail));
}

#[test]
fn no_armour_no_reprieve() {
    let mut w = test_world(1);
    let p = player(&mut w);
    let mail = equipped_in(&w, p, Slot::Body).unwrap();
    destroy_worn(&mut w, p, &[mail]);
    w.get_mut::<Fighter>(p).unwrap().hp = 3;

    apply_hit(&mut w, p, Hit::magic(50), None);

    assert!(w.get::<Fighter>(p).unwrap().hp <= 0);
}

#[test]
fn the_second_killing_blow_kills() {
    let mut w = test_world(1);
    let p = player(&mut w);
    suit_up(&mut w, p);

    apply_hit(&mut w, p, Hit::magic(50), None);
    apply_hit(&mut w, p, Hit::magic(50), None);

    assert!(w.get::<Fighter>(p).unwrap().hp <= 0);
}

#[test]
fn a_cursed_suit_takes_the_blow() {
    let mut w = test_world(1);
    let p = player(&mut w);
    let mail = suit_up(&mut w, p);
    w.entity_mut(mail).insert((ArmorBonus(-2), Curse));

    apply_hit(&mut w, p, Hit::magic(50), None);

    assert_eq!(w.get::<Fighter>(p).unwrap().hp, 1);
    assert!(!w.entities().contains(mail));
}

#[test]
fn a_plain_suit_does_not() {
    let mut w = test_world(1);
    let p = player(&mut w);
    let mail = suit_up(&mut w, p);
    w.entity_mut(mail).remove::<ArmorBonus>();

    apply_hit(&mut w, p, Hit::magic(50), None);

    assert!(w.get::<Fighter>(p).unwrap().hp <= 0);
    assert!(w.entities().contains(mail));
}
