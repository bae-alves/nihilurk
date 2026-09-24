//! The content tables themselves, checked as data.
//!
//! These tests are the safety net under `docs/how-to/`: they fail when a new
//! row is malformed, duplicated, or unreachable by name — the four ways adding
//! content goes wrong quietly.

use std::collections::HashSet;

use bevy_ecs::prelude::*;
use models::*;

fn at(x: u16, y: u16) -> Position {
    Position { x, y }
}

fn rng(seed: u64) -> ChaCha12Rng {
    ChaCha12Rng::seed_from_u64(seed)
}

#[test]
fn every_content_name_is_unique() {
    let mut seen: HashSet<&str> = HashSet::new();
    for (category, name) in content_names() {
        assert!(
            seen.insert(name),
            "two rows are both called {name:?} ({category})"
        );
    }
}

#[test]
fn every_content_name_spawns_and_keeps_its_name() {
    for (category, name) in content_names() {
        let mut w = World::new();
        let entity = spawn_named(&mut w, name, at(5, 5))
            .unwrap_or_else(|| panic!("spawn_named could not build {name:?} ({category})"));
        let spawned = &w
            .get::<Name>(entity)
            .expect("everything spawns with a Name")
            .what;
        // A mimic is the one deliberate exception: it is born wearing a false
        // name on purpose, and only [`crate::monsters::reveal_mimics`] gives it
        // back.
        let mimics = MonsterDef::lookup(name).is_some_and(|m| m.mimics);
        if !mimics {
            assert_eq!(
                spawned, name,
                "{category} row spawned under a different name"
            );
        }
        assert!(
            w.get::<Position>(entity).is_some(),
            "{name:?} spawned nowhere"
        );
    }
}

#[test]
fn a_name_the_tables_do_not_know_spawns_nothing() {
    let mut w = World::new();
    assert!(spawn_named(&mut w, "sandwich", at(1, 1)).is_none());
    assert!(spawn_named(&mut w, "", at(1, 1)).is_none());
    assert!(
        spawn_named(&mut w, "Dragon", at(1, 1)).is_none(),
        "lookup is case-sensitive"
    );
    assert_eq!(
        w.iter_entities().count(),
        0,
        "a failed lookup left something behind"
    );
}

#[test]
fn a_row_spawned_by_name_carries_what_its_row_says() {
    let mut w = World::new();

    // A bestiary row's numbers land on the Fighter it spawns.
    let dragon = spawn_named(&mut w, "dragon", at(1, 1)).unwrap();
    let def = MonsterDef::named("dragon");
    let fighter = w.get::<Fighter>(dragon).unwrap();
    assert_eq!(
        (fighter.hp, fighter.power, fighter.armor),
        (def.hp, def.power, def.armor)
    );

    // A catalog row's components land on the item it spawns.
    let sword = spawn_named(&mut w, "long sword", at(2, 2)).unwrap();
    let row = WEAPONS.iter().find(|d| d.name == "long sword").unwrap();
    assert_eq!(
        w.get::<PowerDie>(sword).copied(),
        Some(PowerDie(row.power_die))
    );
    assert!(
        w.get::<PowerBonus>(sword).is_none(),
        "a named spawn is unenchanted"
    );

    // And a ring's grant list comes from its row, not from ring-specific code.
    let ring = spawn_named(&mut w, "ring of perception", at(3, 3)).unwrap();
    assert!(w.get::<Grants>(ring).is_some());
}

// ---------------------------------------------------------------------------
// Weights and depth
// ---------------------------------------------------------------------------

#[test]
fn pick_weighted_is_proportional_and_skips_zeroes() {
    let mut r = rng(7);
    let mut counts = [0usize; 3];
    for _ in 0..6_000 {
        counts[pick_weighted(&[10, 0, 30], &mut r).unwrap()] += 1;
    }
    assert_eq!(counts[1], 0, "a weight of zero must never be drawn");
    let ratio = counts[2] as f64 / counts[0] as f64;
    assert!(
        (2.5..3.5).contains(&ratio),
        "30:10 should draw about 3:1, got {ratio:.2}"
    );

    assert!(pick_weighted(&[], &mut r).is_none());
    assert!(pick_weighted(&[0, 0], &mut r).is_none());
}

#[test]
fn a_species_never_appears_above_its_min_depth() {
    let mut r = rng(11);
    for depth in 1..=FINAL_DEPTH {
        for _ in 0..300 {
            let def = MonsterDef::pick(depth, &mut r);
            assert!(
                def.min_depth <= depth,
                "{} (min_depth {}) turned up on floor {depth}",
                def.name,
                def.min_depth
            );
        }
    }
}

#[test]
fn the_deep_letters_do_eventually_turn_up() {
    // "The deepest tier" is the deepest `min_depth` shared by at least three
    // species, read off the table — rebalancing which floor a monster debuts
    // on must not turn this red. A single rarer capstone above it (the xeroc,
    // deeper still and alone up there) is deliberately not what this checks:
    // that one is asked for on its own in `xeroc_only_turns_up_past_its_debut`.
    let mut tiers: Vec<u8> = BESTIARY.iter().map(|m| m.min_depth).collect();
    tiers.sort_unstable();
    tiers.dedup();
    let deepest = *tiers
        .iter()
        .rev()
        .find(|&&d| BESTIARY.iter().filter(|m| m.min_depth == d).count() >= 3)
        .expect("some tier in the bestiary holds at least three species");
    let mut r = rng(13);
    let deep: HashSet<&str> = (0..2_000)
        .map(|_| MonsterDef::pick(FINAL_DEPTH, &mut r).name)
        .filter(|n| MonsterDef::named(n).min_depth >= deepest)
        .collect();
    assert!(
        deep.len() >= 3,
        "the deepest floor should mix in the deepest tier, saw {deep:?}"
    );
}

#[test]
fn xeroc_only_turns_up_past_its_debut() {
    let mimic = BESTIARY
        .iter()
        .find(|m| m.mimics)
        .expect("the bestiary has a mimic");
    let mut r = rng(37);
    for depth in 1..mimic.min_depth {
        for _ in 0..300 {
            assert_ne!(MonsterDef::pick(depth, &mut r).name, mimic.name);
        }
    }
    let seen: HashSet<&str> = (0..500)
        .map(|_| MonsterDef::pick(mimic.min_depth, &mut r).name)
        .collect();
    assert!(
        seen.contains(mimic.name),
        "the mimic should turn up once its own floor unlocks it"
    );
}

#[test]
fn pick_any_ignores_the_depth_gate() {
    // The climb out with the Element of Yoord: every floor draws from the whole
    // bestiary, so the deepest letters can turn up regardless of depth.
    let shallowest = BESTIARY
        .iter()
        .min_by_key(|m| m.min_depth)
        .expect("the bestiary is not empty");
    let deepest = BESTIARY
        .iter()
        .max_by_key(|m| m.min_depth)
        .expect("the bestiary is not empty");
    let mut r = rng(19);
    let seen: HashSet<&str> = (0..3_000)
        .map(|_| MonsterDef::pick_any(&mut r).name)
        .collect();
    assert!(
        seen.contains(shallowest.name) && seen.contains(deepest.name),
        "pick_any should mix the whole table, saw {seen:?}"
    );
}

#[test]
fn floor_one_draws_only_from_the_shallow_bestiary() {
    let shallowest = BESTIARY
        .iter()
        .min_by_key(|m| m.min_depth)
        .expect("the bestiary is not empty");
    let deepest = BESTIARY
        .iter()
        .max_by_key(|m| m.min_depth)
        .expect("the bestiary is not empty");
    let mut r = rng(17);
    let seen: HashSet<&str> = (0..1_000)
        .map(|_| MonsterDef::pick(1, &mut r).name)
        .collect();
    assert!(seen.contains(shallowest.name));
    assert!(
        !seen.contains(deepest.name),
        "a deep monster on floor 1 would end the run there"
    );
}

#[test]
fn every_drop_category_can_actually_produce_something() {
    for category in DROPS {
        assert!(
            category.weight > 0,
            "{} would never be drawn",
            category.name
        );
        assert!(
            !category.rows(category.min_depth).is_empty(),
            "{} has no row available on the floor it debuts",
            category.name
        );
    }
}

/// Every loot category can actually be drawn, and has something to give when
/// it is.
///
/// This used to roll 20,000 items off a hand-picked seed and check that each
/// category had turned up at least once. That measured the sampler rather than
/// the table: it was the slowest test in the suite, it depended on a seed
/// nobody could re-derive, and a category whose rows had all become
/// unreachable would have been indistinguishable from a run of bad luck. Worse,
/// it could only ever get *weaker* as the table grew -- add a category rarer
/// than `launcher` and 20,000 draws stops being enough.
///
/// The claim is about the table, so it is asked of the table. A zero weight
/// means `pick_weighted` can never choose the category; an empty row list means
/// choosing it would have nothing to hand back. Neither can hide behind a seed.
#[test]
fn every_loot_category_can_be_drawn_and_has_rows_to_give() {
    for category in DROPS {
        assert!(
            category.weight > 0,
            "{} has no weight, so it can never be drawn",
            category.name
        );
        // At the shallowest floor it claims, and at the deepest in the game:
        // a category that empties out at depth is a category that stops
        // existing without saying so.
        for depth in [category.min_depth, FINAL_DEPTH] {
            assert!(
                !category.rows(depth).is_empty(),
                "{} has no rows at depth {depth}",
                category.name
            );
        }
    }
}

/// Every bestiary row can actually turn up, and does not outlive the dungeon.
///
/// `pick`/`pick_any` weight-draw the same way `DROPS` does, and a row can go
/// silently unreachable the same way a loot category can: a `.weight(0)`
/// nobody meant, or a `min_depth` typo'd past `FINAL_DEPTH` so the species
/// waits for a floor the run never has. Neither is a bug a player reports --
/// it just reads as a species nobody happens to have met.
#[test]
fn every_bestiary_row_can_actually_be_drawn() {
    for def in BESTIARY {
        assert!(
            def.weight > 0,
            "{} has no weight, so it can never be drawn",
            def.name
        );
        assert!(
            def.min_depth <= FINAL_DEPTH,
            "{} debuts on floor {} -- past the last floor, {FINAL_DEPTH}",
            def.name,
            def.min_depth
        );
    }
}

/// A mimic hides by disguise ([`MonsterDef::mimics`]); an invisible creature
/// hides by not being drawn at all. Nothing about combining them is
/// mechanically wrong today -- [`crate::monsters::reveal_mimics`] would strip
/// the disguise correctly -- but the result is a creature that, having been
/// noticed, still cannot be seen: a state neither mechanic was written to
/// produce together. See the doc comment on `MonsterDef::mimics`.
#[test]
fn a_mimic_is_never_also_invisible() {
    for def in BESTIARY {
        assert!(
            !(def.mimics && def.invisible),
            "{} is both a mimic and invisible -- pick one kind of hiding",
            def.name
        );
    }
}

// ---------------------------------------------------------------------------
// Traps
// ---------------------------------------------------------------------------

#[test]
fn a_traps_label_is_its_row() {
    for def in TRAPS {
        assert_eq!(def.effect.label(), def.name);
        assert_eq!(TrapDef::of(def.effect).name, def.name);
        assert!(TrapDef::lookup(def.name).is_some());
    }
    assert!(TrapDef::lookup("pit trap").is_none());
}

#[test]
fn the_trap_a_floor_lays_comes_from_the_table() {
    let mut r = rng(23);
    let seen: HashSet<&str> = (0..600).map(|_| TrapDef::pick(1, &mut r).name).collect();
    assert_eq!(
        seen.len(),
        TRAPS.len(),
        "every trap in the table is available on floor 1"
    );
}

/// [`every_bestiary_row_can_actually_be_drawn`]'s twin for `TRAPS`: a zero
/// weight or a `min_depth` past `FINAL_DEPTH` leaves a trap nobody ever
/// springs.
#[test]
fn every_trap_row_can_actually_be_drawn() {
    for def in TRAPS {
        assert!(
            def.weight > 0,
            "{} has no weight, so it can never be laid",
            def.name
        );
        assert!(
            def.min_depth <= FINAL_DEPTH,
            "{} debuts on floor {} -- past the last floor, {FINAL_DEPTH}",
            def.name,
            def.min_depth
        );
    }
}

// ---------------------------------------------------------------------------
// The NIHILURK_SPAWN shortcut
// ---------------------------------------------------------------------------

#[test]
fn a_spawn_list_drops_each_name_on_its_own_free_tile() {
    let mut w = World::new();
    w.insert_resource(GameRng(rng(29)));
    w.insert_resource(RngSeed(29));
    w.init_resource::<GameLog>();
    w.insert_resource(PlayerName {
        what: "TESTER".into(),
    });
    initialize_world(&mut w);

    let start = *w.query_filtered::<&Position, With<Player>>().single(&w);
    let mut occupied: HashSet<(u16, u16)> = HashSet::from([(start.x, start.y)]);

    // The floor is already populated, so only the entities that appear *after*
    // the call are the ones the list asked for.
    let before: HashSet<Entity> = w.iter_entities().map(|e| e.id()).collect();
    let recognised = models::spawn_list(
        &mut w,
        "dragon, long sword, sandwich, ,dart trap",
        start,
        &mut occupied,
    );
    assert_eq!(recognised, 3, "an unknown name is skipped, not counted");

    let placed: Vec<(String, Position)> = w
        .iter_entities()
        .filter(|e| !before.contains(&e.id()))
        .filter_map(|e| Some((e.get::<Name>()?.what.clone(), *e.get::<Position>()?)))
        .collect();
    assert_eq!(placed.len(), 3, "got {placed:?}");
    let names: HashSet<&str> = placed.iter().map(|(n, _)| n.as_str()).collect();
    assert_eq!(names, HashSet::from(["dragon", "long sword", "dart trap"]));

    let map = w.resource::<Map>();
    for (name, pos) in &placed {
        assert!(!map.blocks(pos.x, pos.y), "{name} landed in a wall");
    }
    let tiles: HashSet<(u16, u16)> = placed.iter().map(|(_, p)| (p.x, p.y)).collect();
    assert_eq!(tiles.len(), 3, "two requested things shared a tile");
    assert!(
        !tiles.contains(&(start.x, start.y)),
        "something landed on the player"
    );
}

// ---------------------------------------------------------------------------
// Identification
// ---------------------------------------------------------------------------

/// Dud effects only ever happen as the result of a wand of cancellation
/// striking an item (see `items::wands::cancel_entity`) — the dungeon itself
/// must never roll one as normal loot.
#[test]
fn the_dungeon_never_generates_a_dud_as_normal_loot() {
    assert!(
        !POTIONS.iter().any(|d| d.effect == PotionEffect::Water),
        "potion of thirst quenching must not be a spawnable row"
    );
    assert!(
        !SCROLLS.iter().any(|d| d.effect == ScrollEffect::BlankPaper),
        "scroll of blank paper must not be a spawnable row"
    );
    assert!(
        !WANDS.iter().any(|d| d.effect == WandEffect::Nothing),
        "wand of nothing must not be a spawnable row"
    );
}
