//! A bones ghost sharing the player's own name is fought as "yourself," in
//! its own log colour — on both ends of the fight, not just in flavor text.
//! An ordinary ghost (a different name) reads exactly like any other monster.

use bevy_ecs::prelude::*;
use models::*;

fn combat_world(seed: u64) -> World {
    let mut w = World::new();
    w.insert_resource(GameRng(ChaCha12Rng::seed_from_u64(seed)));
    w.init_resource::<GameLog>();
    w
}

fn spawn_player(w: &mut World, power: i32) -> Entity {
    w.spawn((
        Player,
        Name { what: "BAE".into() },
        Fighter {
            hp: 20,
            max_hp: 20,
            armor: 0,
            power,
            max_power: power,
            armor_bonus: 0,
            power_bonus: 0,
        },
    ))
    .id()
}

fn spawn_ghost(w: &mut World, name: &str, is_own: bool) -> Entity {
    let e = w
        .spawn((
            Name { what: name.into() },
            Fighter {
                hp: 20,
                max_hp: 20,
                armor: 0,
                power: 1,
                max_power: 1,
                armor_bonus: 0,
                power_bonus: 0,
            },
        ))
        .id();
    if is_own {
        w.entity_mut(e).insert(GhostOfPlayer);
    }
    e
}

fn only_category(w: &World) -> Vec<LogCategory> {
    w.resource::<GameLog>()
        .unread
        .iter()
        .map(|l| l.category)
        .collect()
}

#[test]
fn hitting_your_own_ghost_says_yourself_in_the_ghost_colour() {
    let mut w = combat_world(1);
    let player = spawn_player(&mut w, 10);
    let ghost = spawn_ghost(&mut w, "BAE", true);

    resolve_attack(&mut w, player, ghost);

    let log = w.resource::<GameLog>();
    assert!(
        log.unread.iter().any(|l| l.contains("yourself")),
        "the hit line should read as \"yourself\", not the ghost's name"
    );
    assert!(
        !log.unread.iter().any(|l| l.contains("BAE")),
        "the ghost's literal name should never appear once it's your own"
    );
    assert!(
        only_category(&w).contains(&LogCategory::Ghost),
        "the self-ghost line should be painted in its own colour"
    );
}

#[test]
fn your_own_ghost_hitting_you_back_also_says_yourself() {
    let mut w = combat_world(2);
    let player = spawn_player(&mut w, 1);
    let ghost = spawn_ghost(&mut w, "BAE", true);

    resolve_attack(&mut w, ghost, player);

    let log = w.resource::<GameLog>();
    assert!(
        log.unread
            .iter()
            .any(|l| l.contains("yourself") || l.contains("you miss")),
        "a self-ghost's own attack should read in first/second person, not as a named monster hitting you: {:?}",
        log.unread
            .iter()
            .map(|l| l.text.clone())
            .collect::<Vec<_>>()
    );
    assert!(
        only_category(&w).contains(&LogCategory::Ghost),
        "the self-ghost's own attack line should be painted in its own colour"
    );
}

#[test]
fn an_ordinary_ghost_with_a_different_name_reads_normally() {
    let mut w = combat_world(3);
    let player = spawn_player(&mut w, 10);
    let ghost = spawn_ghost(&mut w, "STRANGER", false);

    resolve_attack(&mut w, player, ghost);

    let log = w.resource::<GameLog>();
    assert!(
        log.unread.iter().any(|l| l.contains("STRANGER")),
        "an unrelated ghost is still named plainly"
    );
    assert!(
        !only_category(&w).contains(&LogCategory::Ghost),
        "only a self-ghost's own lines use the ghost colour"
    );
}
