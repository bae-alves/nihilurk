//! Wearing and wielding: toggling equipped state, a curse that sticks until
//! it's lifted, that the folded `Loadout` numbers move by exactly a row's
//! values, and that a plus or a curse stays hidden until the item is worn or
//! identified.

use bevy_ecs::prelude::*;
use models::constants::spells::{TURBO_MAGIC_COST_MULT, TURBO_MAGIC_POWER_MULT};
use models::*;

use crate::common::monster;

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

/// Simulate the inventory "Use" action: queue the item at its pack row,
/// run the item system (which re-inserts it), exactly like the engine does.
fn use_item(w: &mut World, user: Entity, item: Entity) {
    let idx = w
        .get_mut::<Backpack>(user)
        .unwrap()
        .items
        .iter()
        .position(|&e| e == item);
    if let Some(i) = idx {
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
fn a_third_ring_evicts_an_uncursed_one_but_not_a_cursed_pair() {
    let mut w = test_world(19);
    let p = player(&mut w);

    let first = spawn_ring(&mut w, "ring of protection", Position { x: 0, y: 0 });
    let second = spawn_ring(&mut w, "ring of strength", Position { x: 0, y: 0 });
    let third = spawn_ring(&mut w, "ring of sharpshooting", Position { x: 0, y: 0 });
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
    let fourth = spawn_ring(&mut w, "ring of adornment", Position { x: 0, y: 0 });
    w.entity_mut(fourth).remove::<Position>();
    w.get_mut::<Backpack>(p).unwrap().items.push(fourth);
    use_item(&mut w, p, fourth);
    assert!(
        !is_equipped(&w, fourth),
        "both fingers cursed shut should refuse a fourth ring"
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
            spell: spell_row("Fireball").unwrap(),
            target,
        });
        spell_system(&mut w);

        let spent = magic_before - w.get::<Magic>(p).unwrap().points;
        let damage = 1_000 - w.get::<Fighter>(dummy).unwrap().hp;
        (spent, damage)
    }

    let (plain_cost, plain_damage) = cast_fireball(23, "estoc");
    let (turbo_cost, turbo_damage) = cast_fireball(23, "wizard staff");

    assert_eq!(
        plain_cost,
        spell_row("Fireball").unwrap().cost,
        "Fireball's own row cost"
    );
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

/// The staff is the one weapon whose flourish fires at both ends. Wielding it
/// multiplies what every attacking spell costs and what it does, and putting it
/// away takes both back — and neither change shows anywhere on the HUD, so the
/// two log lines are the only notice the player gets. `OnWear` says the first;
/// `OnDoff`, the staff's own reason for existing, says the second.
#[test]
fn the_staff_says_so_going_on_and_coming_off() {
    let mut w = test_world(5);
    let p = player(&mut w);
    let staff = spawn_weapon(&mut w, "wizard staff", Position { x: 0, y: 0 });
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
            let Some(s) = on.into_iter().find(|&e| Some(e) != filler) else {
                assert!(w.get_entity(recv).is_none() && w.get_entity(donor).is_none());
                return String::new();
            };
            assert!(w.get::<Curse>(s).is_some(), "seed {seed}: still cursed");
            assert!(w.get::<KnownQuality>(s).is_some());
            assert_eq!(plus_of(&w, s), 3, "seed {seed}: donor's plus, not summed");
            if recv_name == "long sword" {
                assert_eq!(w.get::<Vorpal>(s).unwrap().bane, "orc");
            }
            let name = label(&w, s);
            assert!(
                [recv_name, donor_name].contains(&name.as_str()),
                "seed {seed}: {name}"
            );
            name
        })
        .collect()
}

fn assert_split(names: &[String], recv: &str, donor: &str) {
    let n = |s: &str| names.iter().filter(|x| x.as_str() == s).count();
    assert!(n(recv) > 0, "recipient form never came up");
    assert!(n(donor) > 0, "donor form never came up");
    assert!(n("") > 0, "a merge never broke");
}

#[test]
fn cursed_armor_merges() {
    let names = merge_names(
        |w, n| spawn_armor(w, n, Position { x: 0, y: 0 }),
        "plate mail",
        "ring mail",
    );
    assert_split(&names, "plate mail", "ring mail");
}

#[test]
fn cursed_rings_merge() {
    let make = |w: &mut World, n: &str| spawn_ring(w, n, Position { x: 0, y: 0 });
    let names = merge_names(make, "ring of strength", "ring of sharpshooting");
    assert_split(&names, "ring of strength", "ring of sharpshooting");
}

#[test]
fn merge_picks_the_first_cursed_ring_in_pack() {
    for seed in 0..50 {
        let mut w = test_world(seed);
        let p = player(&mut w);
        bare(&mut w, p);
        let at = Position { x: 0, y: 0 };
        let a = spawn_ring(&mut w, "ring of protection", at);
        let b = spawn_ring(&mut w, "ring of protection", at);
        let donor = spawn_ring(&mut w, "ring of protection", at);
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
    let a = spawn_ring(&mut w, "ring of protection", at);
    let b = spawn_ring(&mut w, "ring of protection", at);
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
    let (mut recipient, mut donor, mut broke) = (0, 0, 0);
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
        if on.is_empty() {
            broke += 1;
            continue;
        }
        assert_eq!(on.len(), 1, "seed {seed}");
        let s = on[0];
        assert!(w.get::<Curse>(s).is_some());
        assert_eq!(plus_of(&w, s), 3);
        assert_eq!(w.get::<Vorpal>(s).unwrap().bane, "orc");
        match label(&w, s).as_str() {
            "long sword" => recipient += 1,
            "mace" => donor += 1,
            other => panic!("{other}"),
        }
    }
    assert!(recipient > 0 && donor > 0, "a form never came up");
    assert!(broke > 0, "a merge never broke");
}
