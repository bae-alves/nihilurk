//! Regression coverage for `resolve_attack`'s excellent-hit floor.

use bevy_ecs::prelude::*;
use models::*;

fn combat_world(seed: u64) -> World {
    let mut w = World::new();
    w.insert_resource(GameRng(ChaCha12Rng::seed_from_u64(seed)));
    w.init_resource::<GameLog>();
    w
}

fn spawn_attacker(w: &mut World, power: i32) -> Entity {
    w.spawn((
        Player,
        Name { what: "you".into() },
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

fn spawn_target(w: &mut World, hp: i32, armor: i32) -> Entity {
    w.spawn((
        Name {
            what: "dummy".into(),
        },
        Fighter {
            hp,
            max_hp: hp,
            armor,
            power: 1,
            max_power: 1,
            armor_bonus: 0,
            power_bonus: 0,
        },
    ))
    .id()
}

/// A weak attacker (die size 1) against heavy armour, on a foe left at exactly
/// 1 HP: the combination that used to clamp an "excellent hit" down to 0
/// damage — the "a glancing blow can't take the last point" rule firing on a
/// swing that was never glancing in the first place. Sweeps enough seeds that
/// an excellent hit (15% chance) is certain to land at least once.
#[test]
fn an_excellent_hit_never_deals_or_reports_zero_damage() {
    let mut saw_excellent = false;
    for seed in 0..2000u64 {
        let mut w = combat_world(seed);
        let attacker = spawn_attacker(&mut w, 1);
        let target = spawn_target(&mut w, 1, 100);
        resolve_attack(&mut w, attacker, target);

        let log = w.resource::<GameLog>();
        let Some(line) = log.unread.iter().find(|l| l.contains("excellent hit")) else {
            continue;
        };
        saw_excellent = true;
        assert!(
            !line.contains("for 0 damage"),
            "seed {seed}: excellent hit reported 0 damage: {line}"
        );
    }
    assert!(
        saw_excellent,
        "no excellent hit landed across 2000 seeds; the sweep isn't exercising the path"
    );
}

/// The other half of the same fix: a plain chip/glancing blow can never take a
/// foe's last point of HP (see `scrolls.rs`'s
/// `a_glancing_blow_chips_a_foe_down_to_one_but_never_finishes_it`), but an
/// excellent hit is a real hit, not a whiff that got rounded up — so unlike a
/// glancing blow, it's allowed to land the killing blow even against a foe at
/// 1 HP behind heavy armour.
#[test]
fn an_excellent_hit_can_finish_a_foe_a_glancing_blow_could_not() {
    for seed in 0..2000u64 {
        let mut w = combat_world(seed);
        let attacker = spawn_attacker(&mut w, 1);
        let target = spawn_target(&mut w, 1, 100);
        resolve_attack(&mut w, attacker, target);

        let log = w.resource::<GameLog>();
        let excellent = log.unread.iter().any(|l| l.contains("excellent hit"));
        if excellent && !w.entities().contains(target) {
            return; // found one: an excellent hit finished a 1-HP target.
        }
    }
    panic!("no excellent hit finished a 1-HP target across 2000 seeds");
}
