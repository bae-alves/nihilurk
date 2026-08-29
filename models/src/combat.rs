use bevy_ecs::prelude::*;
use rand::Rng;
use rand_chacha::ChaCha12Rng;

use crate::components::*;
use crate::map::GameRng;

/// Chance for the player to land an "excellent hit" (see [`resolve_attack`]).
const EXCELLENT_HIT_CHANCE: f64 = 0.15;
/// An excellent hit rolls extra weapon dice: `1d[Power]` becomes `Nd[Power]`.
/// 3d8 instead of 1d8 keeps the swing high *and* consistent — a crit you can
/// rely on, not a bigger gamble.
const EXCELLENT_HIT_DICE: i32 = 3;

/// Rolls `1dN`. A non-positive number of sides means "no die", which rolls 0 so
/// an unarmoured/unarmed entity simply contributes nothing to the opposed roll.
fn roll_die(rng: &mut ChaCha12Rng, sides: i32) -> i32 {
    if sides <= 0 {
        0
    } else {
        rng.gen_range(1..=sides)
    }
}

/// Looks up an entity's display name, falling back to a vague noun so the log
/// never prints a raw entity id at the player.
fn entity_name(world: &World, entity: Entity) -> String {
    world
        .get::<Name>(entity)
        .map(|n| n.what.clone())
        .unwrap_or_else(|| "something".to_string())
}

/// The combat schedule step: drains the [`AttackQueue`] and resolves every
/// pending attack (currently these are all monster-initiated; the player's
/// melee is resolved inline by the input handler).
pub fn combat_system(world: &mut World) {
    let mut attack_queue = world.resource_mut::<AttackQueue>();
    let attacks = std::mem::take(&mut attack_queue.attacks);
    drop(attack_queue);

    for attack in attacks {
        resolve_attack(world, attack.attacker, attack.target);
    }
}

/// Resolves a single opposed-roll attack of `attacker` against `target`.
///
/// Damage is `(1d[Power]) - (1d[Armor])`: the attacker's and defender's roll
/// totals are computed independently and then subtracted. When the *player* is
/// the attacker two extra rules apply:
///
/// * **Excellent hit** — a [`EXCELLENT_HIT_CHANCE`] chance for a clean strike
///   that rolls [`EXCELLENT_HIT_DICE`] weapon dice (`Nd[Power]`) before the
///   armour is subtracted.
/// * **Chip damage** — the player always deals at least 1 damage, even when the
///   armour roll fully absorbs the weapon roll (logged as a "glancing blow").
pub fn resolve_attack(world: &mut World, attacker: Entity, target: Entity) {
    // Missing attacker or target: nothing to resolve.
    if world.get_entity(attacker).is_none() || world.get_entity(target).is_none() {
        return;
    }

    let attacker_power = world.get::<Fighter>(attacker).map(|f| f.power).unwrap_or(1);
    let target_armor = world.get::<Fighter>(target).map(|f| f.armor).unwrap_or(0);
    let attacker_is_player = world.get::<Player>(attacker).is_some();

    // --- Independent opposed rolls -----------------------------------------
    let (attack_total, armor_roll, excellent) = {
        let mut rng = world.resource_mut::<GameRng>();
        let excellent = attacker_is_player && rng.0.gen_bool(EXCELLENT_HIT_CHANCE);
        let dice = if excellent { EXCELLENT_HIT_DICE } else { 1 };
        let attack_total: i32 = (0..dice).map(|_| roll_die(&mut rng.0, attacker_power)).sum();
        let armor_roll = roll_die(&mut rng.0, target_armor);
        (attack_total, armor_roll, excellent)
    };

    let mut damage = attack_total - armor_roll;

    // --- Player-only chip damage floor -----------------------------------
    let mut glancing = false;
    if attacker_is_player && damage < 1 {
        damage = 1;
        glancing = true;
    }
    let damage = damage.max(0);

    // --- Apply & report --------------------------------------------------
    let attacker_name = entity_name(world, attacker);
    let target_name = entity_name(world, target);
    let target_is_player = world.get::<Player>(target).is_some();

    let mut lethal = false;
    if let Some(mut fighter) = world.get_mut::<Fighter>(target) {
        fighter.hp -= damage;
        lethal = fighter.hp <= 0;
    }

    let mut log = world.resource_mut::<GameLog>();
    if attacker_is_player {
        if excellent {
            log.add(format!(
                "You score an excellent hit on the {target_name} for {damage} damage!"
            ));
        } else if glancing {
            log.add(format!("You deal a glancing blow to the {target_name}."));
        } else {
            log.add(format!("You hit the {target_name} for {damage} damage."));
        }
        if lethal {
            log.add(format!("You have slain the {target_name}!"));
        }
    } else {
        let target_label = if target_is_player {
            "you".to_string()
        } else {
            format!("the {target_name}")
        };
        if damage == 0 {
            log.add(format!("The {attacker_name} misses {target_label}."));
        } else {
            log.add(format!(
                "The {attacker_name} hits {target_label} for {damage} damage."
            ));
        }
        if lethal {
            if target_is_player {
                log.add(format!("The {attacker_name} strikes you down..."));
            } else {
                log.add(format!("The {attacker_name} kills the {target_name}!"));
            }
        }
    }
    drop(log);

    if lethal {
        world.despawn(target);
    }
}
