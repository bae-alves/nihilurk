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
        assert_eq!(
            spawned, name,
            "{category} row spawned under a different name"
        );
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
    assert_eq!(w.get::<PowerDie>(sword).copied(), Some(PowerDie(8)));
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
    for depth in 1..=13u8 {
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
    let mut r = rng(13);
    let deep: HashSet<&str> = (0..2_000)
        .map(|_| MonsterDef::pick(13, &mut r).name)
        .filter(|n| MonsterDef::named(n).min_depth >= 7)
        .collect();
    assert!(
        deep.len() >= 3,
        "floor 13 should mix in the deepest tier, saw {deep:?}"
    );
}

#[test]
fn floor_one_draws_only_from_the_shallow_bestiary() {
    let mut r = rng(17);
    let seen: HashSet<&str> = (0..1_000)
        .map(|_| MonsterDef::pick(1, &mut r).name)
        .collect();
    assert!(seen.contains("goblin"));
    assert!(
        !seen.contains("dragon"),
        "a dragon on floor 1 would end the run there"
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

#[test]
fn the_loot_table_covers_every_category_over_a_long_run() {
    let mut w = World::new();
    let mut r = rng(19);
    let mut seen: HashSet<String> = HashSet::new();
    for _ in 0..20_000 {
        let item = roll_item(&mut w, &mut r, 13, at(1, 1));
        seen.insert(w.get::<Name>(item).unwrap().what.clone());
        w.despawn(item);
    }
    for category in DROPS {
        assert!(
            category.rows(13).iter().any(|n| seen.contains(*n)),
            "20k drops produced nothing from {}",
            category.name
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
        "all six traps are equally likely on floor 1"
    );
}

// ---------------------------------------------------------------------------
// The ROOG_SPAWN shortcut
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

/// Appearances are zipped against the catalog, and a zip stops at the shorter
/// side: a 21st potion would silently spawn with no appearance and read as a
/// generic "potion" forever. Adding a row to POTIONS, SCROLLS, WANDS or RINGS
/// means checking the matching pool in `identify.rs` is still long enough.
#[test]
fn every_identifiable_type_gets_an_appearance() {
    let appearances = ItemAppearances::generate(&mut rng(31));
    assert_eq!(
        appearances.potions.len(),
        POTIONS.len(),
        "the potion appearance pool is short"
    );
    assert_eq!(
        appearances.scrolls.len(),
        SCROLLS.len(),
        "the scroll appearance pool is short"
    );
    assert_eq!(
        appearances.wands.len(),
        WANDS.len(),
        "the wand appearance pool is short"
    );
    assert_eq!(
        appearances.rings.len(),
        RINGS.len(),
        "the ring appearance pool is short"
    );
}
