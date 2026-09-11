use bevy_ecs::prelude::*;
use models::*;

fn test_world(seed: u64) -> World {
    let mut w = World::new();
    w.insert_resource(GameRng(ChaCha12Rng::seed_from_u64(seed)));
    w.insert_resource(RngSeed(seed));
    w.init_resource::<GameLog>();
    w.init_resource::<UseQueue>();
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
    w.get::<Equipped>(e).is_some_and(|x| x.by.is_some())
}

#[test]
fn using_gear_toggles_equipped_state() {
    let mut w = test_world(1);
    let p = player(&mut w);
    let sword = spawn_weapon(&mut w, "long sword", Position { x: 0, y: 0 });
    let dagger = spawn_weapon(&mut w, "dagger", Position { x: 0, y: 0 });
    let mail = spawn_armor(&mut w, "plate mail", Position { x: 0, y: 0 });
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

    let cursed_mail = spawn_armor(&mut w, "plate mail", Position { x: 0, y: 0 });
    w.entity_mut(cursed_mail).insert(Curse);
    let plain_mail = spawn_armor(&mut w, "leather armor", Position { x: 0, y: 0 });
    let scroll = spawn_scroll(&mut w, ScrollEffect::RemoveCurse, Position { x: 0, y: 0 });
    for e in [cursed_mail, plain_mail, scroll] {
        w.entity_mut(e).remove::<Position>();
        w.get_mut::<Backpack>(p).unwrap().items.push(e);
    }

    // Put the cursed armour on — fine.
    use_item(&mut w, p, cursed_mail);
    assert!(is_equipped(&w, cursed_mail));

    // Can't take it off.
    use_item(&mut w, p, cursed_mail);
    assert!(
        is_equipped(&w, cursed_mail),
        "cursed armour should not come off"
    );

    // Can't swap to other armour while the cursed suit is stuck.
    use_item(&mut w, p, plain_mail);
    assert!(
        !is_equipped(&w, plain_mail),
        "cursed armour blocks changing armour"
    );
    assert!(is_equipped(&w, cursed_mail));

    // Read a scroll of remove curse: the equipped cursed suit is destroyed
    // outright — unequipped, pulled from the pack and despawned.
    use_item(&mut w, p, scroll);
    assert!(
        !w.entities().contains(cursed_mail),
        "the cursed armour is despawned"
    );
    assert!(
        !w.get::<Backpack>(p).unwrap().items.contains(&cursed_mail),
        "the cursed armour is gone from the pack"
    );

    // The plain armour was never cursed, so it's still there and now equippable.
    assert!(w.get::<Backpack>(p).unwrap().items.contains(&plain_mail));
    use_item(&mut w, p, plain_mail);
    assert!(
        is_equipped(&w, plain_mail),
        "plain armour equips once the curse is cleared"
    );
}

#[test]
fn remove_curse_spares_unequipped_cursed_gear() {
    let mut w = test_world(5);
    let p = player(&mut w);

    let worn_ring = spawn_ring(&mut w, RingEffect::Adornment, Position { x: 0, y: 0 });
    w.entity_mut(worn_ring).insert(Curse);
    let stashed_sword = spawn_weapon(&mut w, "long sword", Position { x: 0, y: 0 });
    w.entity_mut(stashed_sword).insert(Curse);
    let scroll = spawn_scroll(&mut w, ScrollEffect::RemoveCurse, Position { x: 0, y: 0 });
    for e in [worn_ring, stashed_sword, scroll] {
        w.entity_mut(e).remove::<Position>();
        w.get_mut::<Backpack>(p).unwrap().items.push(e);
    }

    // Only the ring is equipped.
    use_item(&mut w, p, worn_ring);

    use_item(&mut w, p, scroll);

    // Equipped cursed ring: destroyed.
    assert!(
        !w.entities().contains(worn_ring),
        "the equipped cursed ring is destroyed"
    );
    // Cursed sword just sitting in the pack: untouched, curse and all.
    assert!(
        w.entities().contains(stashed_sword),
        "the stashed cursed sword survives"
    );
    assert!(
        w.get::<Curse>(stashed_sword).is_some(),
        "its curse is left intact"
    );
    assert!(w.get::<Backpack>(p).unwrap().items.contains(&stashed_sword));
}

#[test]
fn equipped_weapon_and_armor_change_combat_math() {
    // A punching bag with a big HP pool and no rolls of its own.
    fn bag(w: &mut World) -> Entity {
        w.spawn((
            Name { what: "bag".into() },
            Fighter {
                hp: 100_000,
                max_hp: 100_000,
                armor: 0,
                power: 0,
                max_power: 0,
                armor_bonus: 0,
                power_bonus: 0,
            },
            Faction::Monster,
            Position { x: 1, y: 1 },
        ))
        .id()
    }

    // Strips whatever the player starts equipped in — the +1 ring mail and mace.
    fn disarm(w: &mut World, p: Entity) {
        for item in equipped_items(w, p) {
            force_unequip(w, item);
        }
    }

    // --- Damage dealt: bare fists vs. a wielded two-handed sword. ---
    let mut w = test_world(42);
    let p = player(&mut w);
    disarm(&mut w, p);
    let target = bag(&mut w);
    for _ in 0..400 {
        resolve_attack(&mut w, p, target);
    }
    let bare_dealt = 100_000 - w.get::<Fighter>(target).unwrap().hp;

    let mut w = test_world(42);
    let p = player(&mut w);
    let target = bag(&mut w);
    let ths = spawn_weapon(&mut w, "two-handed sword", Position { x: 0, y: 0 });
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
    disarm(&mut w, p);
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
    let mail = spawn_armor(&mut w, "plate mail", Position { x: 0, y: 0 });
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

#[test]
fn a_worn_ring_of_protection_soaks_hits() {
    // Hits hard enough (1d20 + 50) that the armour roll never fully absorbs a
    // blow, so the ring's effect shows up as an exact -2 per hit.
    fn attacker(w: &mut World) -> Entity {
        w.spawn((
            Name { what: "bag".into() },
            Fighter {
                hp: 1,
                max_hp: 1,
                armor: 0,
                power: 20,
                max_power: 20,
                armor_bonus: 0,
                power_bonus: 50,
            },
            Faction::Monster,
            Position { x: 1, y: 1 },
        ))
        .id()
    }

    let hits = 600;

    let mut w = test_world(11);
    let p = player(&mut w);
    let foe = attacker(&mut w);
    w.get_mut::<Fighter>(p).unwrap().max_hp = 100_000;
    w.get_mut::<Fighter>(p).unwrap().hp = 100_000;
    for _ in 0..hits {
        resolve_attack(&mut w, foe, p);
    }
    let without_ring = 100_000 - w.get::<Fighter>(p).unwrap().hp;

    let mut w = test_world(11);
    let p = player(&mut w);
    let foe = attacker(&mut w);
    w.get_mut::<Fighter>(p).unwrap().max_hp = 100_000;
    w.get_mut::<Fighter>(p).unwrap().hp = 100_000;
    let ring = spawn_ring(&mut w, RingEffect::Protection, Position { x: 0, y: 0 });
    w.entity_mut(ring).remove::<Position>();
    w.get_mut::<Backpack>(p).unwrap().items.push(ring);
    use_item(&mut w, p, ring);
    assert_eq!(w.get::<Equipped>(ring).unwrap().by, Some(p));
    for _ in 0..hits {
        resolve_attack(&mut w, foe, p);
    }
    let with_ring = 100_000 - w.get::<Fighter>(p).unwrap().hp;

    // +2 to every armour roll over `hits` blows.
    assert_eq!(
        with_ring,
        without_ring - 2 * hits,
        "each blow is softened by exactly 2"
    );

    // Taking the ring back off drops the protection.
    use_item(&mut w, p, ring);
    w.get_mut::<Fighter>(p).unwrap().hp = 100_000;
    for _ in 0..hits {
        resolve_attack(&mut w, foe, p);
    }
    let ring_off = 100_000 - w.get::<Fighter>(p).unwrap().hp;
    assert!(ring_off > with_ring, "an unworn ring gives no protection");
}

#[test]
fn a_worn_ring_of_strength_adds_two_to_every_blow() {
    // Zero armour on the bag, so no roll ever absorbs the blow and the ring's
    // +2 flows straight through as an exact +2 per hit.
    fn bag(w: &mut World) -> Entity {
        w.spawn((
            Name { what: "bag".into() },
            Fighter {
                hp: 100_000,
                max_hp: 100_000,
                armor: 0,
                power: 0,
                max_power: 0,
                armor_bonus: 0,
                power_bonus: 0,
            },
            Faction::Monster,
            Position { x: 1, y: 1 },
        ))
        .id()
    }

    let hits = 600;

    let mut w = test_world(4);
    let p = player(&mut w);
    let target = bag(&mut w);
    for _ in 0..hits {
        resolve_attack(&mut w, p, target);
    }
    let bare = 100_000 - w.get::<Fighter>(target).unwrap().hp;

    let mut w = test_world(4);
    let p = player(&mut w);
    let target = bag(&mut w);
    let ring = spawn_ring(&mut w, RingEffect::Strength, Position { x: 0, y: 0 });
    w.entity_mut(ring).remove::<Position>();
    w.get_mut::<Backpack>(p).unwrap().items.push(ring);
    use_item(&mut w, p, ring);
    assert_eq!(w.get::<Equipped>(ring).unwrap().by, Some(p));
    for _ in 0..hits {
        resolve_attack(&mut w, p, target);
    }
    let ringed = 100_000 - w.get::<Fighter>(target).unwrap().hp;

    assert_eq!(ringed, bare + 2 * hits, "every blow lands exactly 2 harder");

    // The same ring also sustains strength — covered by the dart-trap test in
    // tests/traps.rs.
}

#[test]
fn a_worn_ring_of_aggravate_monster_periodically_shrieks() {
    let mut w = test_world(9);
    let p = player(&mut w);
    let orc = spawn_monster(&mut w, MonsterDef::named("orc"), Position { x: 40, y: 11 });

    let ring = spawn_ring(
        &mut w,
        RingEffect::AggravateMonster,
        Position { x: 0, y: 0 },
    );
    w.entity_mut(ring).remove::<Position>();
    w.get_mut::<Backpack>(p).unwrap().items.push(ring);

    // Not worn yet: rolling the per-turn system does nothing.
    for _ in 0..200 {
        passive_ability_system(&mut w);
    }
    assert!(matches!(
        w.get::<Mob>(orc).unwrap().movement_type,
        MovementType::Chase
    ));

    // Put it on. Within a sane number of turns the ~10% roll fires and the whole
    // floor is aggravated on the player.
    use_item(&mut w, p, ring);
    let mut fired_on = None;
    for turn in 0..300 {
        passive_ability_system(&mut w);
        if matches!(
            w.get::<Mob>(orc).unwrap().movement_type,
            MovementType::Aggravated { .. }
        ) {
            fired_on = Some(turn);
            break;
        }
    }
    let turn = fired_on.expect("the ring never shrieked in 300 turns");
    assert!(turn < 150, "10%/turn should trigger fast, not after {turn}");
    let hero = *w.get::<Position>(p).unwrap();
    match w.get::<Mob>(orc).unwrap().movement_type {
        MovementType::Aggravated { tx, ty } => assert_eq!((tx, ty), (hero.x, hero.y)),
        _ => panic!("expected the orc to be Aggravated on the hero"),
    }
}

#[test]
fn a_plus_and_curse_stay_hidden_until_the_item_is_worn() {
    let mut w = test_world(9);
    let p = player(&mut w);

    let sword = spawn_weapon(&mut w, "long sword", Position { x: 0, y: 0 });
    w.entity_mut(sword).insert(PowerBonus(1));
    let cursed_mace = spawn_weapon(&mut w, "mace", Position { x: 0, y: 0 });
    w.entity_mut(cursed_mace).insert((PowerBonus(-2), Curse));
    for e in [sword, cursed_mace] {
        w.entity_mut(e).remove::<Position>();
        w.get_mut::<Backpack>(p).unwrap().items.push(e);
    }

    assert_eq!(display_name(&w, sword), "long sword");
    assert_eq!(display_name(&w, cursed_mace), "mace");

    use_item(&mut w, p, sword);
    assert_eq!(display_name(&w, sword), "+1 long sword");

    use_item(&mut w, p, cursed_mace);
    assert_eq!(display_name(&w, cursed_mace), "-2 mace (cursed)");
    assert!(
        w.resource::<GameLog>()
            .history
            .iter()
            .any(|l| l.contains("welds itself to your grip! It is cursed!")),
        "wearing a cursed weapon should announce the curse: {:?}",
        w.resource::<GameLog>().history
    );
}

#[test]
fn a_scroll_of_identify_can_single_out_an_unworn_plus_or_curse() {
    let mut w = test_world(11);
    let p = player(&mut w);
    let items = std::mem::take(&mut w.get_mut::<Backpack>(p).unwrap().items);
    for item in items {
        w.despawn(item);
    }

    let mail = spawn_armor(&mut w, "ring mail", Position { x: 0, y: 0 });
    w.entity_mut(mail).insert((ArmorBonus(2), Curse));
    let scroll = spawn_scroll(&mut w, ScrollEffect::Identify, Position { x: 0, y: 0 });
    for e in [mail, scroll] {
        w.entity_mut(e).remove::<Position>();
        w.get_mut::<Backpack>(p).unwrap().items.push(e);
    }

    assert_eq!(display_name(&w, mail), "ring mail", "still unworn, unread");
    use_item(&mut w, p, scroll);
    assert_eq!(
        display_name(&w, mail),
        "+2 ring mail (cursed)",
        "the scroll should have singled out the only thing left to learn"
    );
}

#[test]
fn a_vorpal_weapons_bane_only_shows_once_its_quality_is_known() {
    let mut w = test_world(13);
    let p = player(&mut w);

    let sword = spawn_weapon(&mut w, "long sword", Position { x: 0, y: 0 });
    w.entity_mut(sword).insert(Vorpal {
        bane: "orc".to_string(),
    });
    w.entity_mut(sword).remove::<Position>();
    w.get_mut::<Backpack>(p).unwrap().items.push(sword);

    assert_eq!(display_name(&w, sword), "long sword");
    use_item(&mut w, p, sword);
    assert_eq!(display_name(&w, sword), "long sword (vorpal vs. orc)");
}
