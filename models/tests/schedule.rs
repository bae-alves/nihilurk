//! The turn schedule's order, read off the real dependency graph.
//!
//! An `.after()` that is dropped or pointed at the wrong step compiles and
//! plays, and only shows up as a bug a turn later. `.githooks/pre-commit`
//! asks for a docs page when an edge changes; this is what checks the edge.

use std::collections::BTreeSet;

use bevy_ecs::schedule::{NodeId, Schedule};
use models::turn_schedule;

fn edges_of(schedule: &Schedule) -> BTreeSet<(String, String)> {
    let graph = schedule.graph();
    let name = |id: NodeId| {
        let system = graph
            .hierarchy()
            .graph()
            .neighbors(id)
            .find(|n| n.is_system())
            .unwrap_or(id);
        let full = graph.system_at(system).name().to_string();
        full.rsplit("::").next().unwrap_or(&full).to_string()
    };
    graph
        .dependency()
        .graph()
        .all_edges()
        .map(|(a, b, _)| (name(a), name(b)))
        .collect()
}

#[test]
fn the_turn_schedule_holds_exactly_these_orderings() {
    let expected: BTreeSet<(String, String)> = [
        ("player_action_system", "smoke_system"),
        ("smoke_system", "tick_effects"),
        ("tick_effects", "reveal_mimics"),
        ("reveal_mimics", "spell_system"),
        ("reveal_mimics", "item_system"),
        ("spell_system", "item_system"),
        ("item_system", "throw_system"),
        ("spell_system", "ai"),
        ("item_system", "ai"),
        ("throw_system", "ai"),
        ("ai", "trap_system"),
        ("item_system", "equipment_effects_system"),
        ("trap_system", "equipment_effects_system"),
        ("equipment_effects_system", "combat_system"),
        ("combat_system", "reaper_system"),
        ("reaper_system", "dungeon_lord_system"),
        ("dungeon_lord_system", "ability_system"),
        ("ability_system", "sink_system"),
        ("sink_system", "visibility_system"),
        ("visibility_system", "score_turn_system"),
    ]
    .into_iter()
    .map(|(a, b)| (a.to_string(), b.to_string()))
    .collect();
    assert_eq!(edges_of(&turn_schedule()), expected);
}

#[test]
fn the_turn_schedule_has_seventeen_steps() {
    assert_eq!(turn_schedule().graph().systems().count(), 17);
}

#[test]
fn no_two_steps_are_left_unordered() {
    let schedule = turn_schedule();
    let edges = edges_of(&schedule);
    let steps: BTreeSet<&String> = edges.iter().flat_map(|(a, b)| [a, b]).collect();
    assert_eq!(steps.len(), 17);

    let reaches = |from: &String, to: &String| {
        let mut seen = BTreeSet::new();
        let mut open = vec![from];
        while let Some(at) = open.pop() {
            for (a, b) in &edges {
                if a == at && seen.insert(b) {
                    open.push(b);
                }
            }
        }
        seen.contains(to)
    };

    let unordered: Vec<String> = steps
        .iter()
        .flat_map(|a| steps.iter().map(move |b| (*a, *b)))
        .filter(|(a, b)| a < b && !reaches(a, b) && !reaches(b, a))
        .map(|(a, b)| format!("{a} / {b}"))
        .collect();
    assert!(unordered.is_empty(), "unordered pairs: {unordered:?}");
}
