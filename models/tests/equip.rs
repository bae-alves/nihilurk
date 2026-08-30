use bevy_ecs::prelude::*;
use models::*;

fn test_world(seed: u64) -> World {
    let mut w = World::new();
    w.insert_resource(GameRng(ChaCha12Rng::seed_from_u64(seed)));
    w.insert_resource(RngSeed(seed));
    w.init_resource::<GameLog>();
    w.init_resource::<UseQueue>();
    w.init_resource::<AttackQueue>();
    w.insert_resource(PlayerName { what: "TESTER".into() });
    initialize_world(&mut w);
    w
}

fn player(w: &mut World) -> Entity {
    w.query_filtered::<Entity, With<Player>>().single(w)
}

/// Simulate the inventory "Use" action: pull the item out of the pack, queue it,
/// run the item system (which re-inserts it), exactly like the engine does.
fn use_item(w: &mut World, user: Entity, item: Entity) {
    let idx = w
        .get_mut::<Backpack>(user)
        .unwrap()
        .items
        .iter()
        .position(|&e| e == item);
    if let Some(i) = idx {
        w.get_mut::<Backpack>(user).unwrap().items.remove(i);
        w.resource_mut::<UseQueue>().uses.push(WantsToUse {
            user,
            item,
            target: None,
            slot_idx: Some(i),
        });
    }
    item_system(w);
}

fn is_equipped(w: &World, e: Entity) -> bool {
    w.get::<Wield>(e).is_some_and(|x| x.wielder.is_some())
        || w.get::<Wear>(e).is_some_and(|x| x.wearer.is_some())
}

#[test]
fn using_gear_toggles_equipped_state() {
    let mut w = test_world(1);
    let p = player(&mut w);
    let sword = w.spawn(WeaponsBundle::long_sword(Position { x: 0, y: 0 })).id();
    let dagger = w.spawn(WeaponsBundle::dagger(Position { x: 0, y: 0 })).id();
    let mail = w.spawn(ArmorBundle::plate_mail(Position { x: 0, y: 0 })).id();
    for e in [sword, dagger, mail] {
        w.entity_mut(e).remove::<Position>();
        w.get_mut::<Backpack>(p).unwrap().items.push(e);
    }

    use_item(&mut w, p, sword);
    assert!(is_equipped(&w, sword));
    // Still in the pack, at its slot.
    assert!(w.get::<Backpack>(p).unwrap().items.contains(&sword));

    // Equipping the dagger swaps the sword out — only one weapon at a time.
    use_item(&mut w, p, dagger);
    assert!(is_equipped(&w, dagger));
    assert!(!is_equipped(&w, sword));

    // Armour is a separate slot, so it coexists with the wielded dagger.
    use_item(&mut w, p, mail);
    assert!(is_equipped(&w, mail));
    assert!(is_equipped(&w, dagger));

    // Using an equipped item again unequips it.
    use_item(&mut w, p, dagger);
    assert!(!is_equipped(&w, dagger));
}

#[test]
fn cursed_gear_sticks_until_the_curse_is_lifted() {
    let mut w = test_world(3);
    let p = player(&mut w);

    let cursed_mail = w.spawn(ArmorBundle::plate_mail(Position { x: 0, y: 0 })).id();
    w.entity_mut(cursed_mail).insert(Curse);
    let plain_mail = w.spawn(ArmorBundle::leather_armor(Position { x: 0, y: 0 })).id();
    let scroll = w.spawn(ScrollBundle::remove_curse(Position { x: 0, y: 0 })).id();
    for e in [cursed_mail, plain_mail, scroll] {
        w.entity_mut(e).remove::<Position>();
        w.get_mut::<Backpack>(p).unwrap().items.push(e);
    }

    // Put the cursed armour on — fine.
    use_item(&mut w, p, cursed_mail);
    assert!(is_equipped(&w, cursed_mail));

    // Can't take it off.
    use_item(&mut w, p, cursed_mail);
    assert!(is_equipped(&w, cursed_mail), "cursed armour should not come off");

    // Can't swap to other armour while the cursed suit is stuck.
    use_item(&mut w, p, plain_mail);
    assert!(!is_equipped(&w, plain_mail), "cursed armour blocks changing armour");
    assert!(is_equipped(&w, cursed_mail));

    // Read a scroll of remove curse, then it comes off.
    use_item(&mut w, p, scroll);
    assert!(w.get::<Curse>(cursed_mail).is_none(), "remove curse strips the tag");
    use_item(&mut w, p, cursed_mail);
    assert!(!is_equipped(&w, cursed_mail), "un-cursed armour comes off normally");
}

#[test]
fn equipped_weapon_and_armor_change_combat_math() {
    // A punching bag with a big HP pool and no rolls of its own.
    fn bag(w: &mut World) -> Entity {
        w.spawn((
            Name { what: "bag".into() },
            Fighter { hp: 100_000, max_hp: 100_000, armor: 0, power: 0, armor_bonus: 0, power_bonus: 0 },
            Faction::Monster,
            Position { x: 1, y: 1 },
        ))
        .id()
    }

    // --- Damage dealt: bare fists vs. a wielded two-handed sword. ---
    let mut w = test_world(42);
    let p = player(&mut w);
    let target = bag(&mut w);
    for _ in 0..400 {
        resolve_attack(&mut w, p, target);
    }
    let bare_dealt = 100_000 - w.get::<Fighter>(target).unwrap().hp;

    let mut w = test_world(42);
    let p = player(&mut w);
    let target = bag(&mut w);
    let ths = w.spawn(WeaponsBundle::two_handed_sword(Position { x: 0, y: 0 })).id();
    w.entity_mut(ths).remove::<Position>();
    w.get_mut::<Backpack>(p).unwrap().items.push(ths);
    use_item(&mut w, p, ths);
    for _ in 0..400 {
        resolve_attack(&mut w, p, target);
    }
    let armed_dealt = 100_000 - w.get::<Fighter>(target).unwrap().hp;

    assert!(
        armed_dealt > bare_dealt * 2,
        "two-handed sword should hit far harder: bare={bare_dealt}, armed={armed_dealt}"
    );

    // --- Damage taken: plate mail should soak monster hits. ---
    let mut w = test_world(7);
    let p = player(&mut w);
    let attacker = bag(&mut w);
    w.get_mut::<Fighter>(attacker).unwrap().power = 10;
    w.get_mut::<Fighter>(p).unwrap().max_hp = 100_000;
    w.get_mut::<Fighter>(p).unwrap().hp = 100_000;
    for _ in 0..400 {
        resolve_attack(&mut w, attacker, p);
    }
    let unarmored_taken = 100_000 - w.get::<Fighter>(p).unwrap().hp;

    let mut w = test_world(7);
    let p = player(&mut w);
    let attacker = bag(&mut w);
    w.get_mut::<Fighter>(attacker).unwrap().power = 10;
    w.get_mut::<Fighter>(p).unwrap().max_hp = 100_000;
    w.get_mut::<Fighter>(p).unwrap().hp = 100_000;
    let mail = w.spawn(ArmorBundle::plate_mail(Position { x: 0, y: 0 })).id();
    w.entity_mut(mail).remove::<Position>();
    w.get_mut::<Backpack>(p).unwrap().items.push(mail);
    use_item(&mut w, p, mail);
    for _ in 0..400 {
        resolve_attack(&mut w, attacker, p);
    }
    let armored_taken = 100_000 - w.get::<Fighter>(p).unwrap().hp;

    assert!(
        armored_taken < unarmored_taken,
        "plate mail should reduce damage taken: none={unarmored_taken}, plate={armored_taken}"
    );
}
