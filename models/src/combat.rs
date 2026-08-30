use bevy_ecs::prelude::*;
use rand::Rng;
use rand_chacha::ChaCha12Rng;

use crate::components::*;
use crate::map::GameRng;

/// Chance for the player to land an "excellent hit" (see [`resolve_attack`]).
const EXCELLENT_HIT_CHANCE: f64 = 0.15;
/// An excellent hit rolls extra weapon dice: `1d[Power]` becomes `Nd[Power]`.
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

/// Sums the `(die_increase, flat_bonus)` an entity's *equipped* weapon adds to
/// its attack roll. An entity with no backpack or nothing wielded gets `(0, 0)`,
/// so monsters are unaffected.
fn equipped_wield_bonus(world: &World, entity: Entity) -> (i32, i32) {
    world
        .get::<Backpack>(entity)
        .and_then(|bp| {
            bp.items
                .iter()
                .filter_map(|&i| world.get::<Wield>(i))
                .find(|w| w.wielder == Some(entity))
                .map(|w| (w.pow_increase as i32, w.pow_bonus as i32))
        })
        .unwrap_or((0, 0))
}

/// As [`equipped_wield_bonus`], but for the entity's equipped armour.
fn equipped_wear_bonus(world: &World, entity: Entity) -> (i32, i32) {
    world
        .get::<Backpack>(entity)
        .and_then(|bp| {
            bp.items
                .iter()
                .filter_map(|&i| world.get::<Wear>(i))
                .find(|w| w.wearer == Some(entity))
                .map(|w| (w.arm_increase as i32, w.arm_bonus as i32))
        })
        .unwrap_or((0, 0))
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
/// Damage is `(1d[Power] + PowerBonus) - (1d[Armor] + ArmorBonus)`: the
/// attacker's and defender's roll totals are computed independently and then
/// subtracted. When the *player* is
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

    let (wpn_die, wpn_flat) = equipped_wield_bonus(world, attacker);
    let (arm_die, arm_flat) = equipped_wear_bonus(world, target);

    let attacker_power = world.get::<Fighter>(attacker).map(|f| f.power).unwrap_or(1) + wpn_die;
    let attacker_power_bonus =
        world.get::<Fighter>(attacker).map(|f| f.power_bonus).unwrap_or(0) + wpn_flat;
    let target_armor = world.get::<Fighter>(target).map(|f| f.armor).unwrap_or(0) + arm_die;
    let target_armor_bonus =
        world.get::<Fighter>(target).map(|f| f.armor_bonus).unwrap_or(0) + arm_flat;
    let attacker_is_player = world.get::<Player>(attacker).is_some();

    // --- Independent opposed rolls -----------------------------------------
    let (attack_total, armor_roll, excellent) = {
        let mut rng = world.resource_mut::<GameRng>();
        let excellent = attacker_is_player && rng.0.gen_bool(EXCELLENT_HIT_CHANCE);
        let dice = if excellent { EXCELLENT_HIT_DICE } else { 1 };
        let attack_total: i32 =
            (0..dice).map(|_| roll_die(&mut rng.0, attacker_power)).sum::<i32>() + attacker_power_bonus;
        let armor_roll = roll_die(&mut rng.0, target_armor) + target_armor_bonus;
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
