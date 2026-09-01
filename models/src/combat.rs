use bevy_ecs::prelude::*;
use crossterm::style::Color;
use rand::Rng;
use rand_chacha::ChaCha12Rng;

use crate::components::*;
use crate::helpers::has_ring_effect;
use crate::map::GameRng;
use crate::particles::Particles;
use crate::state::Ending;

/// Chance for the player to land an "excellent hit" (see [`resolve_attack`]).
const EXCELLENT_HIT_CHANCE: f64 = 0.15;
/// An excellent hit rolls extra weapon dice: `1d[Power]` becomes `Nd[Power]`.
const EXCELLENT_HIT_DICE: i32 = 3;
/// Flat bonus a worn ring of protection adds to the wearer's armour roll.
pub const RING_PROTECTION_BONUS: i32 = 2;
/// Flat bonus a worn ring of strength adds to the wearer's damage roll. The same
/// ring also blocks strength drain (see [`crate::traps`]).
pub const RING_STRENGTH_BONUS: i32 = 2;

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

/// `bonus` if `entity` has a ring with `effect` on its finger, `0` otherwise.
/// Monsters never wear rings, so this always folds in `0` for them.
fn ring_bonus(world: &World, entity: Entity, effect: RingEffect, bonus: i32) -> i32 {
    if has_ring_effect(world, entity, effect) { bonus } else { 0 }
}

/// The `bane` of the attacker's currently-wielded weapon, if that weapon has
/// been vorpalized (scroll of vorpalize weapon). `None` for an unarmed attacker
/// or a plain weapon — so monsters, which never wield, are unaffected.
fn wielded_vorpal_bane(world: &World, entity: Entity) -> Option<String> {
    world.get::<Backpack>(entity)?.items.iter().find_map(|&i| {
        let wielded = world.get::<Wield>(i).is_some_and(|w| w.wielder == Some(entity));
        wielded.then(|| world.get::<Vorpal>(i).map(|v| v.bane.clone())).flatten()
    })
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

/// Sweeps up anything that has been reduced to 0 HP by a source that doesn't
/// resolve its own lethality — wand bolts, fire/cold blasts, and any future
/// indirect damage. Melee kills are still finalised inline by [`resolve_attack`],
/// so by the time this runs the only casualties left are the indirect ones.
///
/// A dead monster is despawned with a plain death line. A dead player does not
/// leave the world; we just flag the [`Ending`]. Because no attacker entity is
/// available here, the cause of death is recorded as "Killer unknown".
pub fn reaper_system(world: &mut World) {
    let doomed: Vec<Entity> = {
        let mut q = world.query::<(Entity, &Fighter)>();
        q.iter(world)
            .filter(|(_, f)| f.hp <= 0)
            .map(|(e, _)| e)
            .collect()
    };

    for entity in doomed {
        if world.get::<Player>(entity).is_some() {
            let mut ending = world.resource_mut::<Ending>();
            if !ending.player_dead {
                ending.player_dead = true;
                ending.cause = "Killer unknown".to_string();
            }
        } else {
            let name = entity_name(world, entity);
            world.resource_mut::<GameLog>().add(format!("The {name} dies."));
            world.despawn(entity);
        }
    }
}

/// Resolves a single opposed-roll attack of `attacker` against `target`.
///
/// Damage is `(1d[Power] + PowerBonus) - (1d[Armor] + ArmorBonus)`: the
/// attacker's and defender's roll totals are computed independently and then
/// subtracted. Equipped gear folds into both sides; a worn ring of strength adds
/// [`RING_STRENGTH_BONUS`] to the attacker's damage roll and a worn ring of
/// protection adds [`RING_PROTECTION_BONUS`] to the defender's armour roll. When
/// the *player* is the attacker two extra rules apply:
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
    let attacker_power_bonus = world.get::<Fighter>(attacker).map(|f| f.power_bonus).unwrap_or(0)
        + wpn_flat
        + ring_bonus(world, attacker, RingEffect::Strength, RING_STRENGTH_BONUS);
    let target_armor = world.get::<Fighter>(target).map(|f| f.armor).unwrap_or(0) + arm_die;
    let target_armor_bonus = world.get::<Fighter>(target).map(|f| f.armor_bonus).unwrap_or(0)
        + arm_flat
        + ring_bonus(world, target, RingEffect::Protection, RING_PROTECTION_BONUS);
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
    // The player always scrapes off at least 1 HP even when the armour roll
    // eats the whole blow — but a blow that weak can never be the killing one.
    // It can leave a foe on 1 HP; it can't take the last point.
    let mut glancing = false;
    if attacker_is_player && damage < 1 {
        damage = 1;
        glancing = true;
    }
    let mut damage = damage.max(0);
    if let Some(f) = world.get::<Fighter>(target).filter(|_| glancing) {
        damage = damage.min((f.hp - 1).max(0));
    }

    // --- Apply & report --------------------------------------------------
    let attacker_name = entity_name(world, attacker);
    let target_name = entity_name(world, target);
    let target_is_player = world.get::<Player>(target).is_some();
    // An attacker the player can't see — an invisible phantom, or a mob still off
    // in the dark — is reported only as "Something".
    let attacker_unseen = target_is_player && world.get::<Hidden>(attacker).is_some();

    // A vorpalized weapon that draws blood slays its bane outright — and any
    // creature whose `Traits::vorpal_target` is set (the Jabberwock), whatever
    // the bane. A glancing scrape never triggers it.
    let vorpal = !glancing
        && damage > 0
        && wielded_vorpal_bane(world, attacker).is_some_and(|bane| {
            world.get::<Traits>(target).is_some_and(|t| t.vorpal_target)
                || world.get::<Name>(target).is_some_and(|n| n.what == bane)
        });

    let mut lethal = false;
    if let Some(mut fighter) = world.get_mut::<Fighter>(target) {
        fighter.hp -= damage;
        if vorpal {
            fighter.hp = 0;
        }
        lethal = fighter.hp <= 0;
    }
    if damage > 0 {
        crate::helpers::spill_blood(world, target, damage, glancing);
    }

    // Instant hit feedback: a spark where the blow landed, or a faint tick for a
    // blow that did nothing. Purely cosmetic; `target` still has its Position
    // here even on a lethal hit (the despawn happens further down).
    if let Some(tpos) = world.get::<Position>(target).copied() {
        // The effect layer is optional (tests run without it).
        if let Some(mut fx) = world.get_resource_mut::<Particles>() {
            if damage > 0 {
                fx.hit_spark(tpos.x, tpos.y);
            } else {
                fx.blip(tpos.x, tpos.y, '·', Color::DarkGrey);
            }
        }
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
            if vorpal {
                log.add(format!("Snicker-snack! The blade shears clean through the {target_name}!"));
            }
            log.add(format!("You have slain the {target_name}!"));
        }
    } else {
        let target_label = if target_is_player {
            "you".to_string()
        } else {
            format!("the {target_name}")
        };
        let atk = if attacker_unseen {
            "Something".to_string()
        } else {
            format!("The {attacker_name}")
        };
        if damage == 0 {
            log.add(format!("{atk} misses {target_label}."));
        } else {
            log.add(format!("{atk} hits {target_label} for {damage} damage."));
        }
        if lethal {
            if target_is_player {
                log.add(format!("{atk} strikes you down..."));
            } else {
                log.add(format!("{atk} kills the {target_name}!"));
            }
        }
    }
    drop(log);

    if lethal {
        if target_is_player {
            // The player does not leave the world; the main loop notices the
            // Ending resource, tears down the save, and shows the death screen.
            let mut ending = world.resource_mut::<Ending>();
            ending.player_dead = true;
            ending.cause = format!("Slain by the {attacker_name}");
        } else {
            world.despawn(target);
        }
    }
}
