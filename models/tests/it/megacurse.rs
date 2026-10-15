//! MEGACURSED gear: a small share of cursed drops that no scroll, demon or
//! card can lift or destroy. Only a suit taking the killing blow still goes.

use bevy_ecs::prelude::*;
use models::constants::loot::{MEGACURSED_BONUS_MAX, MEGACURSED_BONUS_MIN};
use models::constants::rings::{
    EXCEPTIONAL_BONUS, MEGACURSED_BONUS_MAX as RING_BONUS_MAX,
    MEGACURSED_BONUS_MIN as RING_BONUS_MIN,
};
use models::*;

fn test_world(seed: u64) -> World {
    let mut w = World::new();
    w.insert_resource(GameRng(ChaCha12Rng::seed_from_u64(seed)));
    w.insert_resource(RngSeed(seed));
    w.init_resource::<GameLog>();
    w.init_resource::<UseQueue>();
    w.init_resource::<AttackQueue>();
    w.init_resource::<Ending>();
    w.insert_resource(PlayerName {
        what: "TESTER".into(),
    });
    initialize_world(&mut w);
    w
}

fn player(w: &mut World) -> Entity {
    w.query_filtered::<Entity, With<Player>>().single(w)
}

fn wield_mega_blade(w: &mut World, p: Entity) -> Entity {
    for item in equipped_items(w, p) {
        force_unequip(w, item);
    }
    let sword = spawn_weapon(w, "long sword", Position { x: 0, y: 0 });
    w.entity_mut(sword).remove::<Position>();
    w.get_mut::<Backpack>(p).unwrap().items.push(sword);
    w.get_mut::<Equipped>(sword).unwrap().by = Some(p);
    w.entity_mut(sword)
        .insert((PowerBonus(-3), Curse, MegaCurse, KnownQuality));
    sword
}

fn read(w: &mut World, user: Entity, effect: ScrollEffect) {
    let scroll = spawn_scroll(w, effect, Position { x: 0, y: 0 });
    w.entity_mut(scroll).remove::<Position>();
    w.resource_mut::<UseQueue>().uses.push(WantsToUse {
        user,
        item: scroll,
        target: None,
        slot_idx: Some(0),
    });
    item_system(w);
}

#[test]
fn about_one_cursed_drop_in_a_hundred_is_megacursed_and_stays_in_range() {
    let mut w = test_world(7);
    let mut rng = ChaCha12Rng::seed_from_u64(7);
    let (mut cursed, mut mega) = (0u32, 0u32);
    for _ in 0..40_000 {
        let item = spawn_weapon(&mut w, "dagger", Position { x: 0, y: 0 });
        enchant_equipment(&mut w, &mut rng, item);
        if w.get::<Curse>(item).is_none() {
            assert!(w.get::<MegaCurse>(item).is_none(), "a megacurse is a curse");
            continue;
        }
        cursed += 1;
        if w.get::<MegaCurse>(item).is_some() {
            mega += 1;
            let plus = w.get::<PowerBonus>(item).unwrap().0;
            assert!((MEGACURSED_BONUS_MIN..=MEGACURSED_BONUS_MAX).contains(&plus));
        }
    }
    let share = f64::from(mega) / f64::from(cursed);
    assert!(
        (0.005..0.02).contains(&share),
        "{mega} of {cursed} cursed drops were MEGACURSED"
    );
}

#[test]
fn remove_curse_leaves_megacursed_gear_on() {
    let mut w = test_world(2);
    let p = player(&mut w);
    let sword = wield_mega_blade(&mut w, p);
    let plain = spawn_armor(&mut w, "leather armor", Position { x: 0, y: 0 });
    w.entity_mut(plain).remove::<Position>();
    w.get_mut::<Backpack>(p).unwrap().items.push(plain);
    w.get_mut::<Equipped>(plain).unwrap().by = Some(p);
    w.entity_mut(plain).insert(Curse);

    read(&mut w, p, ScrollEffect::RemoveCurse);

    assert!(
        w.get_entity(sword).is_some(),
        "the MEGACURSED blade survives"
    );
    assert!(w.get::<Equipped>(sword).unwrap().by == Some(p));
    assert!(
        w.get_entity(plain).is_none(),
        "an ordinary curse still goes"
    );
}

#[test]
fn enchanting_mends_the_plus_but_never_the_megacurse() {
    let mut w = test_world(2);
    let p = player(&mut w);
    let sword = wield_mega_blade(&mut w, p);

    read(&mut w, p, ScrollEffect::EnchantWeapon);

    assert!(w.get::<Curse>(sword).is_some());
    assert!(w.get::<MegaCurse>(sword).is_some());
}

#[test]
fn a_megacursed_suit_still_takes_the_killing_blow() {
    let mut w = test_world(1);
    let p = player(&mut w);
    let mail = equipped_in(&w, p, Slot::Body).expect("the hero starts in ring mail");
    w.entity_mut(mail)
        .insert((ArmorBonus(-2), Curse, MegaCurse));

    apply_hit(&mut w, p, Hit::magic(50), None);

    assert_eq!(w.get::<Fighter>(p).unwrap().hp, 1);
    assert!(!w.entities().contains(mail));
}

#[test]
fn the_sword_will_not_come_off() {
    let mut w = test_world(2);
    let p = player(&mut w);
    let sword = wield_mega_blade(&mut w, p);

    assert!(!toggle_equipped(&mut w, p, sword));
    assert!(w.get::<Equipped>(sword).unwrap().by == Some(p));
}

#[test]
fn it_reads_megacursed_in_capitals() {
    let mut w = test_world(2);
    let p = player(&mut w);
    let sword = wield_mega_blade(&mut w, p);

    assert!(display_name(&w, sword).contains("(MEGACURSED)"));
}

#[test]
fn the_pink_demon_cannot_destroy_megacursed_gear() {
    let mut w = test_world(2);
    let p = player(&mut w);
    let sword = wield_mega_blade(&mut w, p);

    let demon = spawn_monster(
        &mut w,
        MonsterDef::named("pink demon"),
        Position { x: 5, y: 5 },
    );
    resolve_attack(&mut w, p, demon);

    assert!(w.get_entity(sword).is_some());
}

#[test]
fn megacursed_numeric_rings_roll_their_own_range() {
    let mut w = test_world(3);
    let mut rng = ChaCha12Rng::seed_from_u64(3);
    let mut top = 0;
    for _ in 0..20_000 {
        let ring = RingDef::of(RingEffect::Protection).spawn_as_loot(
            &mut w,
            &mut rng,
            Position { x: 0, y: 0 },
        );
        if w.get::<MegaCurse>(ring).is_none() {
            continue;
        }
        let plus = w.get::<ArmorBonus>(ring).unwrap().0;
        assert!((RING_BONUS_MIN..=RING_BONUS_MAX).contains(&plus), "{plus}");
        top = top.max(plus);
    }
    assert!(
        top > EXCEPTIONAL_BONUS,
        "the range should reach well past a normal ring, got {top}"
    );
}
