//! Wearing and wielding: toggling equipped state, a curse that sticks until
//! it's lifted, that the folded `Loadout` numbers move by exactly a row's
//! values, and that a plus or a curse stays hidden until the item is worn or
//! identified.

use bevy_ecs::prelude::*;
use models::constants::spells::{TURBO_MAGIC_COST_MULT, TURBO_MAGIC_POWER_MULT};
use models::*;

#[path = "common/monster.rs"]
mod monster;

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
    assert!(w.get::<Backpack>(p).unwrap().items.contains(&sword));

    use_item(&mut w, p, dagger);
    assert!(is_equipped(&w, dagger));
    assert!(!is_equipped(&w, sword));

    use_item(&mut w, p, mail);
    assert!(is_equipped(&w, mail));
    assert!(is_equipped(&w, dagger));

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

    use_item(&mut w, p, cursed_mail);
    assert!(is_equipped(&w, cursed_mail));

    use_item(&mut w, p, cursed_mail);
    assert!(
        is_equipped(&w, cursed_mail),
        "cursed armour should not come off"
    );

    use_item(&mut w, p, plain_mail);
    assert!(
        !is_equipped(&w, plain_mail),
        "cursed armour blocks changing armour"
    );
    assert!(is_equipped(&w, cursed_mail));

    use_item(&mut w, p, scroll);
    assert!(
        !w.entities().contains(cursed_mail),
        "the cursed armour is despawned"
    );
    assert!(
        !w.get::<Backpack>(p).unwrap().items.contains(&cursed_mail),
        "the cursed armour is gone from the pack"
    );

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

    use_item(&mut w, p, worn_ring);

    use_item(&mut w, p, scroll);

    assert!(
        !w.entities().contains(worn_ring),
        "the equipped cursed ring is destroyed"
    );
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
    let mut w = test_world(42);
    let p = player(&mut w);

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

    use_item(&mut w, p, mail);
    assert_eq!(loadout(&w, p).armor_die, before.armor_die);
}

#[test]
fn a_worn_ring_of_protection_soaks_hits() {
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

    assert_eq!(
        with_ring,
        without_ring - 2 * hits,
        "each blow is softened by exactly 2"
    );

    use_item(&mut w, p, ring);
    w.get_mut::<Fighter>(p).unwrap().hp = 100_000;
    for _ in 0..hits {
        resolve_attack(&mut w, foe, p);
    }
    let ring_off = 100_000 - w.get::<Fighter>(p).unwrap().hp;
    assert!(ring_off > with_ring, "an unworn ring gives no protection");
}

#[test]
fn spikemail_pricks_whoever_hits_its_wearer_for_one_or_two() {
    let hits = 600;
    let prick = |wear_it: bool| {
        let mut w = test_world(11);
        let p = player(&mut w);
        let foe = w
            .spawn((
                Name { what: "bag".into() },
                Fighter {
                    hp: 100_000,
                    max_hp: 100_000,
                    armor: 0,
                    power: 20,
                    max_power: 20,
                    armor_bonus: 0,
                    power_bonus: 50,
                },
                Faction::Monster,
                Position { x: 1, y: 1 },
            ))
            .id();
        w.get_mut::<Fighter>(p).unwrap().max_hp = 100_000;
        w.get_mut::<Fighter>(p).unwrap().hp = 100_000;
        let mail = spawn_armor(&mut w, "spikemail", Position { x: 0, y: 0 });
        w.entity_mut(mail).remove::<Position>();
        w.get_mut::<Backpack>(p).unwrap().items.push(mail);
        if wear_it {
            use_item(&mut w, p, mail);
        }
        for _ in 0..hits {
            resolve_attack(&mut w, foe, p);
        }
        100_000 - w.get::<Fighter>(foe).unwrap().hp
    };

    assert_eq!(prick(false), 0, "unworn spikes prick nobody");
    let worn = prick(true);
    assert!(
        worn >= hits,
        "every landed blow costs the attacker at least 1"
    );
    assert!(worn <= 2 * hits, "and never more than 2");
    assert!(worn > hits, "1d2 rolls a 2 sometimes");
}

#[test]
fn a_worn_ring_of_strength_adds_two_to_every_blow() {
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
}

#[test]
fn a_worn_ring_of_aggravate_monster_periodically_shrieks() {
    let mut w = test_world(9);
    let p = player(&mut w);
    let orc = monster::monster(&mut w, "test monster", Position { x: 40, y: 11 });

    let ring = spawn_ring(
        &mut w,
        RingEffect::AggravateMonster,
        Position { x: 0, y: 0 },
    );
    w.entity_mut(ring).remove::<Position>();
    w.get_mut::<Backpack>(p).unwrap().items.push(ring);

    for _ in 0..200 {
        ability_system(&mut w);
    }
    assert!(matches!(
        w.get::<Mob>(orc).unwrap().movement_type,
        MovementType::Chase
    ));

    use_item(&mut w, p, ring);
    let mut fired_on = None;
    for turn in 0..300 {
        ability_system(&mut w);
        if w.get::<Aggravated>(orc).is_some() {
            fired_on = Some(turn);
            break;
        }
    }
    let turn = fired_on.expect("the ring never shrieked in 300 turns");
    assert!(turn < 150, "10%/turn should trigger fast, not after {turn}");
    let hero = *w.get::<Position>(p).unwrap();
    let ag = w.get::<Aggravated>(orc).expect("the orc is Aggravated");
    assert_eq!((ag.tx, ag.ty), (hero.x, hero.y));
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
    let troll = monster::monster(&mut w, "test monster", Position { x: 40, y: 11 });
    let sword = spawn_weapon(&mut w, "long sword", Position { x: 0, y: 0 });
    w.entity_mut(sword).insert(PowerBonus(2));

    assert!(equip_silently(&mut w, troll, sword));

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

// ---------------------------------------------------------------------------
// Curse merging
// ---------------------------------------------------------------------------

fn plus_of(w: &World, e: Entity) -> i32 {
    w.get::<PowerBonus>(e).map_or(0, |b| b.0)
        + w.get::<ArmorBonus>(e).map_or(0, |b| b.0)
        + w.get::<ThrowBonus>(e).map_or(0, |b| b.0)
}

fn label(w: &World, e: Entity) -> String {
    w.get::<Name>(e).unwrap().what.clone()
}

fn bare(w: &mut World, p: Entity) {
    for e in equipped_items(w, p) {
        w.entity_mut(e).despawn();
    }
    w.get_mut::<Backpack>(p).unwrap().items.clear();
}

fn pack(w: &mut World, p: Entity, e: Entity) {
    w.entity_mut(e).remove::<Position>();
    w.get_mut::<Backpack>(p).unwrap().items.push(e);
}

fn cursed(w: &mut World, e: Entity, plus: i32) {
    apply_bonus(w, e, plus);
    w.entity_mut(e).insert((Curse, KnownQuality));
}

fn worn(w: &mut World, p: Entity, e: Entity) {
    w.get_mut::<Equipped>(e).unwrap().by = Some(p);
}

/// Merges a worn cursed `recv` with a known cursed `donor` across many seeds;
/// returns the surviving item's name per seed after checking every invariant.
fn merge_names(
    make: impl Fn(&mut World, &str) -> Entity,
    recv_name: &str,
    donor_name: &str,
    fresh_name: &str,
) -> Vec<String> {
    (0..300)
        .map(|seed| {
            let mut w = test_world(seed);
            let p = player(&mut w);
            bare(&mut w, p);
            let recv = make(&mut w, recv_name);
            let donor = make(&mut w, donor_name);
            cursed(&mut w, recv, -2);
            cursed(&mut w, donor, 3);
            if recv_name == "long sword" {
                w.entity_mut(donor).insert(Vorpal { bane: "orc".into() });
            }
            pack(&mut w, p, recv);
            worn(&mut w, p, recv);
            let filler = w.get::<Ring>(recv).is_some().then(|| {
                let f = make(&mut w, recv_name);
                pack(&mut w, p, f);
                worn(&mut w, p, f);
                f
            });
            pack(&mut w, p, donor);
            use_item(&mut w, p, donor);

            let mut items = w.get::<Backpack>(p).unwrap().items.clone();
            let mut on = equipped_items(&w, p);
            items.sort();
            on.sort();
            assert_eq!(items, on, "seed {seed}: pack holds only what is worn");
            assert_eq!(on.len(), 1 + filler.iter().count(), "seed {seed}");
            let s = on.into_iter().find(|&e| Some(e) != filler).unwrap();
            assert!(w.get::<Curse>(s).is_some(), "seed {seed}: still cursed");
            assert!(w.get::<KnownQuality>(s).is_some());
            let numberless = fresh_name == "ring of stealth" && label(&w, s) == fresh_name;
            let want = if numberless { 0 } else { 3 };
            assert_eq!(
                plus_of(&w, s),
                want,
                "seed {seed}: donor's plus, not summed"
            );
            if recv_name == "long sword" {
                assert_eq!(w.get::<Vorpal>(s).unwrap().bane, "orc");
            }
            let name = label(&w, s);
            assert!(
                [recv_name, donor_name, fresh_name].contains(&name.as_str()),
                "seed {seed}: {name}"
            );
            name
        })
        .collect()
}

fn assert_split(names: &[String], recv: &str, donor: &str, fresh: &str) {
    let n = |s: &str| names.iter().filter(|x| x.as_str() == s).count();
    assert!(
        (100..=170).contains(&n(recv)),
        "recipient form: {}",
        n(recv)
    );
    assert!((100..=170).contains(&n(donor)), "donor form: {}", n(donor));
    assert!((10..=60).contains(&n(fresh)), "fresh form: {}", n(fresh));
}

#[test]
fn cursed_weapons_merge() {
    let names = merge_names(
        |w, n| spawn_weapon(w, n, Position { x: 0, y: 0 }),
        "long sword",
        "mace",
        "dagger",
    );
    assert_split(&names, "long sword", "mace", "dagger");
}

#[test]
fn cursed_armor_merges() {
    let names = merge_names(
        |w, n| spawn_armor(w, n, Position { x: 0, y: 0 }),
        "plate mail",
        "ring mail",
        "leather armor",
    );
    assert_split(&names, "plate mail", "ring mail", "leather armor");
}

#[test]
fn cursed_rings_merge() {
    let make = |w: &mut World, n: &str| {
        let effect = match n {
            "ring of strength" => RingEffect::Strength,
            _ => RingEffect::Sharpshooting,
        };
        spawn_ring(w, effect, Position { x: 0, y: 0 })
    };
    let names = merge_names(
        make,
        "ring of strength",
        "ring of sharpshooting",
        "ring of stealth",
    );
    assert_split(
        &names,
        "ring of strength",
        "ring of sharpshooting",
        "ring of stealth",
    );
}

#[test]
fn merge_picks_the_first_cursed_ring_in_pack() {
    for seed in 0..50 {
        let mut w = test_world(seed);
        let p = player(&mut w);
        bare(&mut w, p);
        let at = Position { x: 0, y: 0 };
        let a = spawn_ring(&mut w, RingEffect::Protection, at);
        let b = spawn_ring(&mut w, RingEffect::Protection, at);
        let donor = spawn_ring(&mut w, RingEffect::Protection, at);
        cursed(&mut w, a, -1);
        cursed(&mut w, b, -2);
        cursed(&mut w, donor, 3);
        for e in [a, b] {
            pack(&mut w, p, e);
            worn(&mut w, p, e);
        }
        pack(&mut w, p, donor);
        use_item(&mut w, p, donor);
        assert!(is_equipped(&w, b) && plus_of(&w, b) == -2, "seed {seed}");
    }
}

#[test]
fn unknown_cursed_item_does_not_merge() {
    let mut w = test_world(1);
    let p = player(&mut w);
    bare(&mut w, p);
    let at = Position { x: 0, y: 0 };
    let recv = spawn_weapon(&mut w, "long sword", at);
    let donor = spawn_weapon(&mut w, "mace", at);
    cursed(&mut w, recv, -2);
    cursed(&mut w, donor, 3);
    w.entity_mut(donor).remove::<KnownQuality>();
    pack(&mut w, p, recv);
    worn(&mut w, p, recv);
    pack(&mut w, p, donor);
    use_item(&mut w, p, donor);
    assert!(is_equipped(&w, recv) && !is_equipped(&w, donor));
    assert_eq!(plus_of(&w, recv), -2);
}

#[test]
fn cursed_ring_fills_a_free_finger_instead_of_merging() {
    let mut w = test_world(1);
    let p = player(&mut w);
    bare(&mut w, p);
    let at = Position { x: 0, y: 0 };
    let a = spawn_ring(&mut w, RingEffect::Protection, at);
    let b = spawn_ring(&mut w, RingEffect::Protection, at);
    cursed(&mut w, a, -1);
    cursed(&mut w, b, 3);
    pack(&mut w, p, a);
    worn(&mut w, p, a);
    pack(&mut w, p, b);
    use_item(&mut w, p, b);
    assert!(is_equipped(&w, a) && is_equipped(&w, b));
    assert_eq!((plus_of(&w, a), plus_of(&w, b)), (-1, 3));
}

#[test]
fn monsters_merge_cursed_weapons() {
    let (mut recipient, mut donor, mut fresh) = (0, 0, 0);
    for seed in 0..300 {
        let mut w = test_world(seed);
        let at = Position { x: 0, y: 0 };
        let orc = monster::monster(&mut w, "test monster", at);
        let old = spawn_weapon(&mut w, "long sword", at);
        let new = spawn_weapon(&mut w, "mace", at);
        cursed(&mut w, old, -2);
        cursed(&mut w, new, 3);
        w.entity_mut(new).insert(Vorpal { bane: "orc".into() });
        assert!(equip_silently(&mut w, orc, old));
        w.entity_mut(new).remove::<KnownQuality>();
        assert!(equip_merging(&mut w, orc, new));
        let on = equipped_items(&w, orc);
        assert_eq!(on.len(), 1, "seed {seed}");
        let s = on[0];
        assert!(w.get::<Curse>(s).is_some());
        assert_eq!(plus_of(&w, s), 3);
        assert_eq!(w.get::<Vorpal>(s).unwrap().bane, "orc");
        match label(&w, s).as_str() {
            "long sword" => recipient += 1,
            "mace" => donor += 1,
            "dagger" => fresh += 1,
            other => panic!("{other}"),
        }
    }
    assert!((100..=170).contains(&recipient) && (100..=170).contains(&donor));
    assert!((10..=60).contains(&fresh));
}
