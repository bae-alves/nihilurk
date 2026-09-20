use bevy_ecs::prelude::*;
use models::constants::spells::{TURBO_MAGIC_COST_MULT, TURBO_MAGIC_POWER_MULT};
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
fn equipping_gear_moves_the_folded_numbers_by_exactly_the_rows_values() {
    // This used to swing 400 times bare-handed, 400 times with a two-handed
    // sword, and assert the second total was more than twice the first. That
    // measured the dice, not the rule: it was slow, it only held for one seed,
    // and it would have gone red for any change that shifted how many draws a
    // blow makes -- while still passing if a weapon's die had stopped being
    // read at all, as long as the totals happened to stay far enough apart.
    //
    // The rule it was reaching for is exact and has no dice in it: what a piece
    // of gear is worth in the fold is what its catalog row says it is worth.
    // The expected numbers are read off the spawned item rather than written
    // here, so the row stays the only place they live.
    let mut w = test_world(42);
    let p = player(&mut w);

    // The hero starts in a +1 ring mail with a mace in hand. Strip both, so
    // what is measured below is the gear going on rather than one row swapping
    // for another -- the swap is `two_rings_can_be_worn_at_once`'s business.
    for item in equipped_items(&mut w, p) {
        force_unequip(&mut w, item);
    }
    let bare = loadout(&w, p);
    assert_eq!((bare.power_die, bare.armor_die), (0, 0), "stripped");

    let sword = spawn_weapon(&mut w, "two-handed sword", Position { x: 0, y: 0 });
    w.entity_mut(sword).remove::<Position>();
    let sword_die = w
        .get::<PowerDie>(sword)
        .expect("a weapon row carries a die")
        .0;
    w.get_mut::<Backpack>(p).unwrap().items.push(sword);

    let before = loadout(&w, p);
    use_item(&mut w, p, sword);
    let after = loadout(&w, p);

    assert_eq!(
        after.power_die - before.power_die,
        sword_die,
        "wielding the sword adds exactly its row's die"
    );
    assert_eq!(
        after.power_die, sword_die,
        "and it is the only weapon in hand"
    );
    assert_eq!(
        after.armor_die, before.armor_die,
        "a weapon is worth nothing on the armour roll"
    );

    let mail = spawn_armor(&mut w, "plate mail", Position { x: 0, y: 0 });
    w.entity_mut(mail).remove::<Position>();
    let mail_die = w
        .get::<ArmorDie>(mail)
        .expect("an armour row carries a die")
        .0;
    w.get_mut::<Backpack>(p).unwrap().items.push(mail);

    let before = loadout(&w, p);
    use_item(&mut w, p, mail);
    let after = loadout(&w, p);

    assert_eq!(
        after.armor_die - before.armor_die,
        mail_die,
        "wearing the mail adds exactly its row's die"
    );
    assert_eq!(
        after.power_die, before.power_die,
        "armour is worth nothing on the attack roll"
    );

    // And taking it back off gives the numbers back. An `Equipped` slot that
    // leaked would show up here and nowhere else in this file.
    use_item(&mut w, p, mail);
    assert_eq!(loadout(&w, p).armor_die, before.armor_die);
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
        ability_system(&mut w);
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
        ability_system(&mut w);
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

#[test]
fn two_rings_can_be_worn_at_once_and_both_effects_stack() {
    let mut w = test_world(17);
    let p = player(&mut w);
    // The starting mace carries its own +1; clear the whole kit so the totals
    // below are the rings' contribution alone.
    for item in equipped_items(&w, p) {
        force_unequip(&mut w, item);
    }
    let items = std::mem::take(&mut w.get_mut::<Backpack>(p).unwrap().items);
    for item in items {
        w.despawn(item);
    }

    let protection = spawn_ring(&mut w, RingEffect::Protection, Position { x: 0, y: 0 });
    let strength = spawn_ring(&mut w, RingEffect::Strength, Position { x: 0, y: 0 });
    for e in [protection, strength] {
        w.entity_mut(e).remove::<Position>();
        w.get_mut::<Backpack>(p).unwrap().items.push(e);
    }

    use_item(&mut w, p, protection);
    assert!(is_equipped(&w, protection));
    use_item(&mut w, p, strength);
    assert!(
        is_equipped(&w, protection),
        "putting on a second ring should not bump the first"
    );
    assert!(is_equipped(&w, strength));

    // Both rings' bonuses fold in together, not one replacing the other.
    assert_eq!(equipped_total::<ArmorBonus>(&w, p), 2);
    assert_eq!(equipped_total::<PowerBonus>(&w, p), 2);
}

#[test]
fn a_third_ring_evicts_an_uncursed_one_but_not_a_cursed_pair() {
    let mut w = test_world(19);
    let p = player(&mut w);

    let first = spawn_ring(&mut w, RingEffect::Protection, Position { x: 0, y: 0 });
    let second = spawn_ring(&mut w, RingEffect::Strength, Position { x: 0, y: 0 });
    let third = spawn_ring(&mut w, RingEffect::Sharpshooting, Position { x: 0, y: 0 });
    for e in [first, second, third] {
        w.entity_mut(e).remove::<Position>();
        w.get_mut::<Backpack>(p).unwrap().items.push(e);
    }
    use_item(&mut w, p, first);
    use_item(&mut w, p, second);

    // Both fingers full, neither cursed: the third bumps one of them off.
    use_item(&mut w, p, third);
    assert!(is_equipped(&w, third));
    let still_on = [first, second]
        .into_iter()
        .filter(|&e| is_equipped(&w, e))
        .count();
    assert_eq!(
        still_on, 1,
        "putting on a third ring should evict exactly one"
    );

    // Now curse both worn rings: nothing can budge them for a fourth.
    let worn: Vec<Entity> = [first, second, third]
        .into_iter()
        .filter(|&e| is_equipped(&w, e))
        .collect();
    assert_eq!(worn.len(), 2);
    for &e in &worn {
        w.entity_mut(e).insert(Curse);
    }
    let fourth = spawn_ring(&mut w, RingEffect::Adornment, Position { x: 0, y: 0 });
    w.entity_mut(fourth).remove::<Position>();
    w.get_mut::<Backpack>(p).unwrap().items.push(fourth);
    use_item(&mut w, p, fourth);
    assert!(
        !is_equipped(&w, fourth),
        "both fingers cursed shut should refuse a fourth ring"
    );
}

#[test]
fn a_monster_equipped_silently_does_not_identify_its_own_gear() {
    let mut w = test_world(5);
    let orc = spawn_monster(&mut w, MonsterDef::named("orc"), Position { x: 40, y: 11 });
    let sword = spawn_weapon(&mut w, "long sword", Position { x: 0, y: 0 });
    w.entity_mut(sword).insert(PowerBonus(2));

    assert!(equip_silently(&mut w, orc, sword));

    assert!(
        w.get::<KnownQuality>(sword).is_none(),
        "a monster spawning with, or picking up, gear must not identify it for the player"
    );
}

#[test]
fn the_player_worn_via_equip_silently_still_identifies_immediately() {
    let mut w = test_world(6);
    let p = player(&mut w);
    for item in equipped_items(&w, p) {
        force_unequip(&mut w, item);
    }
    let sword = spawn_weapon(&mut w, "long sword", Position { x: 0, y: 0 });
    w.entity_mut(sword).insert(PowerBonus(2));
    w.entity_mut(sword).remove::<Position>();

    assert!(equip_silently(&mut w, p, sword));

    assert!(
        w.get::<KnownQuality>(sword).is_some(),
        "the player's own gear, even handed over already worn, is known from turn one"
    );
}

/// A staff doubles the Magic cost and triples the damage of a damaging spell —
/// Fireball here, chosen because its damage scales off the caster's own
/// melee power rather than fixed dice, which is exactly what makes this test
/// possible. Two identically seeded worlds, one
/// wielding a staff and one wielding an estoc — the only other weapon that
/// happens to share the staff's 5 power die, so both worlds hand
/// `breathe_fire` the exact same roll range and, from the same seed, draw the
/// exact same underlying die. Whatever `TurboMagic` does to the outcome is
/// then the *only* thing that can make the two numbers differ.
#[test]
fn a_staff_doubles_the_cost_and_triples_the_damage_of_a_damaging_spell() {
    fn cast_fireball(seed: u64, weapon_name: &str) -> (u8, i32) {
        let mut w = test_world(seed);
        w.init_resource::<SpellQueue>();
        let p = player(&mut w);
        for item in equipped_items(&w, p) {
            force_unequip(&mut w, item);
        }
        let weapon = spawn_weapon(&mut w, weapon_name, Position { x: 0, y: 0 });
        w.entity_mut(weapon).remove::<Position>();
        w.get_mut::<Backpack>(p).unwrap().items.push(weapon);
        use_item(&mut w, p, weapon);
        assert!(is_equipped(&w, weapon));

        let target = *w.get::<Position>(p).unwrap();
        let dummy = w
            .spawn((
                Name {
                    what: "dummy".into(),
                },
                Fighter {
                    hp: 1_000,
                    max_hp: 1_000,
                    armor: 0,
                    power: 0,
                    max_power: 0,
                    armor_bonus: 0,
                    power_bonus: 0,
                },
                Faction::Monster,
                target,
            ))
            .id();

        let magic_before = w.get::<Magic>(p).unwrap().points;
        w.resource_mut::<SpellQueue>().spells.push(WantsToCast {
            user: p,
            effect: SpellEffect::DragonBreath,
            target,
        });
        spell_system(&mut w);

        let spent = magic_before - w.get::<Magic>(p).unwrap().points;
        let damage = 1_000 - w.get::<Fighter>(dummy).unwrap().hp;
        (spent, damage)
    }

    let (plain_cost, plain_damage) = cast_fireball(23, "estoc");
    let (turbo_cost, turbo_damage) = cast_fireball(23, "staff");

    assert_eq!(plain_cost, 2, "Fireball's own row cost");
    assert_eq!(
        turbo_cost,
        plain_cost * TURBO_MAGIC_COST_MULT,
        "a staff multiplies the Magic cost"
    );
    assert!(plain_damage > 0, "the estoc cast should have dealt damage");
    assert_eq!(
        turbo_damage,
        plain_damage * TURBO_MAGIC_POWER_MULT,
        "a staff multiplies the damage of the same roll"
    );
}

/// The war hammer's whole point is a mechanic in `combat` (`ShattersStone`,
/// which takes a blow through a petrified hide whole) reached only through the
/// catalog row that lends it. The mechanic has its own tests; this is the wire
/// between them — wield the hammer, and the wielder carries the effect.
#[test]
fn wielding_the_war_hammer_lends_what_goes_through_stone() {
    let mut w = test_world(1);
    let p = player(&mut w);
    let hammer = spawn_weapon(&mut w, "war hammer", Position { x: 0, y: 0 });
    w.entity_mut(hammer).remove::<Position>();
    w.get_mut::<Backpack>(p).unwrap().items.push(hammer);

    assert!(
        w.get::<ShattersStone>(p).is_none(),
        "the hammer worked from inside the pack"
    );
    use_item(&mut w, p, hammer);
    assert!(
        w.get::<ShattersStone>(p).is_some(),
        "a wielded war hammer lent its wielder nothing"
    );
}

/// The staff is the one weapon whose flourish fires at both ends. Wielding it
/// multiplies what every attacking spell costs and what it does, and putting it
/// away takes both back — and neither change shows anywhere on the HUD, so the
/// two log lines are the only notice the player gets. `OnWear` says the first;
/// `OnDoff`, the staff's own reason for existing, says the second.
#[test]
fn the_staff_says_so_going_on_and_coming_off() {
    let mut w = test_world(5);
    let p = player(&mut w);
    let staff = spawn_weapon(&mut w, "staff", Position { x: 0, y: 0 });
    w.entity_mut(staff).remove::<Position>();
    w.get_mut::<Backpack>(p).unwrap().items.push(staff);

    use_item(&mut w, p, staff);
    assert!(is_equipped(&w, staff));
    assert!(
        w.resource::<GameLog>()
            .history
            .iter()
            .any(|l| l.contains("You're a wizard now!")),
        "wielding the staff said nothing: {:?}",
        w.resource::<GameLog>().history
    );

    use_item(&mut w, p, staff);
    assert!(!is_equipped(&w, staff));
    assert!(
        w.resource::<GameLog>()
            .history
            .iter()
            .any(|l| l.contains("You're no longer that magical.")),
        "putting the staff away said nothing: {:?}",
        w.resource::<GameLog>().history
    );
}
