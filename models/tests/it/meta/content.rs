//! The content tables themselves, checked as data.
//!
//! These tests are the safety net under `docs/how-to/`: they fail when a new
//! row is malformed, duplicated, or unreachable by name — the four ways adding
//! content goes wrong quietly.

use std::collections::HashSet;

use models::*;

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

/// Every loot category can actually be drawn, and has something to give when
/// it is.
///
/// The claim is about the table, so it is asked of the table, not of a seeded
/// run of draws. A zero weight
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

// ---------------------------------------------------------------------------
// Identification
// ---------------------------------------------------------------------------

/// A swimmer lives in the water, so only a water tile ever asks for one: the
/// ordinary draws never hand one out, however deep, and the aquatic draw
/// hands out nothing else.
#[test]
fn a_swimmer_is_only_drawn_for_the_water() {
    let mut r = rng(23);
    for _ in 0..2_000 {
        assert!(!MonsterDef::pick(FINAL_DEPTH, &mut r).swims());
        assert!(!MonsterDef::pick_any(&mut r).swims());
        assert!(MonsterDef::pick_aquatic(&mut r).swims());
    }
}
