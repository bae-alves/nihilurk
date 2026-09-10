use bevy_ecs::prelude::*;
use crossterm::style::Color;
use rand::Rng;
use rand_chacha::ChaCha12Rng;

use crate::components::*;
use crate::effects::{
    ArmorBonus, ArmorDie, PowerBonus, PowerDie, VorpalTarget, equipped_total, melee_cap,
};
use crate::equipment::{equipped_items, force_unequip};
use crate::map::GameRng;
use crate::particles::Particles;
use crate::state::Ending;

// --- Tuning constants ------------------------------------------------------
// Defined and documented in `constants.rs`.
//
//   EXCELLENT_HIT_CHANCE / EXCELLENT_HIT_DICE  the player's Nd[power] crit
//   CHIP_DAMAGE                                the player's guaranteed-1 floor
//   GEAR_SURVIVES_DEATH                        per-item odds a corpse keeps its gear
use crate::constants::combat::{
    CHIP_DAMAGE, EXCELLENT_HIT_CHANCE, EXCELLENT_HIT_DICE, GEAR_SURVIVES_DEATH,
};

/// Rolls `1dN`. A non-positive number of sides means "no die", which rolls 0 so
/// an unarmoured/unarmed entity simply contributes nothing to the opposed roll.
fn roll_die(rng: &mut ChaCha12Rng, sides: i32) -> i32 {
    if sides <= 0 {
        0
    } else {
        rng.gen_range(1..=sides)
    }
}

/// The `bane` of the attacker's currently-wielded weapon, if that weapon has
/// been vorpalized (scroll of vorpalize weapon). `None` for an unarmed attacker
/// or a plain weapon — so monsters, which never wield, are unaffected.
fn wielded_vorpal_bane(world: &World, entity: Entity) -> Option<String> {
    equipped_items(world, entity)
        .into_iter()
        .find_map(|i| world.get::<Vorpal>(i).map(|v| v.bane.clone()))
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
    let attacks = std::mem::take(&mut world.resource_mut::<AttackQueue>().attacks);

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
            world
                .resource_mut::<GameLog>()
                .add(format!("The {name} dies."));
            leave_gear_behind(world, entity);
            world.despawn(entity);
        }
    }
}

/// Settles what a dying creature was wearing, item by item. Each piece gets its
/// own [`GEAR_SURVIVES_DEATH`] coin flip: heads it clatters onto the corpse's
/// tile, announced so the player knows there is something to go back for; tails
/// it is destroyed with its owner and never mentioned again.
///
/// This is what stops a thrown dagger an orc caught (see
/// [`crate::items::throw_system`]) from either vanishing silently into the dead
/// entity or coming back every single time.
fn leave_gear_behind(world: &mut World, entity: Entity) {
    let Some(pos) = world.get::<Position>(entity).copied() else {
        return;
    };
    for item in equipped_items(world, entity) {
        force_unequip(world, item);
        if world
            .resource_mut::<GameRng>()
            .0
            .gen_bool(GEAR_SURVIVES_DEATH)
        {
            let name = crate::identify::display_name(world, item);
            world.entity_mut(item).insert(pos);
            world
                .resource_mut::<GameLog>()
                .add(format!("The {name} clatters to the floor."));
        } else {
            world.entity_mut(item).despawn();
        }
    }
}

/// Resolves a single opposed-roll attack of `attacker` against `target`.
///
/// Damage is `(1d[Power] + PowerBonus) - (1d[Armor] + ArmorBonus)`: the
/// attacker's and defender's roll totals are computed independently and then
/// subtracted. Every equipped source of a [`crate::effects::Modifier`] folds
/// into those four numbers — a weapon's die, an enchantment's flat bonus, a
/// ring of protection's — and this function never learns which kind of item any
/// of them came from. When the *player* is the attacker two extra rules apply:
///
/// * **Excellent hit** — a [`EXCELLENT_HIT_CHANCE`] chance for a clean strike
///   that rolls [`EXCELLENT_HIT_DICE`] weapon dice (`Nd[Power]`) before the
///   armour is subtracted.
/// * **Chip damage** — the player always deals at least 1 damage, even when the
///   armour roll fully absorbs the weapon roll (logged as a "glancing blow").
///
/// Finally, gear that carries a [`MeleeCap`](crate::effects::MeleeCap) — a bow,
/// a crossbow — clamps the result. A launcher is worth nothing swung, which is
/// what pays for how good it is drawn.
pub fn resolve_attack(world: &mut World, attacker: Entity, target: Entity) {
    // Missing attacker or target: nothing to resolve.
    if world.get_entity(attacker).is_none() || world.get_entity(target).is_none() {
        return;
    }

    // Every equipped source of a modifier folds in the same way — a sword, a
    // suit of plate, a ring of strength. Nothing here knows which is which.
    let attacker_power = world.get::<Fighter>(attacker).map(|f| f.power).unwrap_or(1)
        + equipped_total::<PowerDie>(world, attacker);
    let attacker_power_bonus = world
        .get::<Fighter>(attacker)
        .map(|f| f.power_bonus)
        .unwrap_or(0)
        + equipped_total::<PowerBonus>(world, attacker);
    let target_armor = world.get::<Fighter>(target).map(|f| f.armor).unwrap_or(0)
        + equipped_total::<ArmorDie>(world, target);
    let target_armor_bonus = world
        .get::<Fighter>(target)
        .map(|f| f.armor_bonus)
        .unwrap_or(0)
        + equipped_total::<ArmorBonus>(world, target);
    let attacker_is_player = world.get::<Player>(attacker).is_some();

    // --- Independent opposed rolls -----------------------------------------
    let (attack_total, armor_roll, excellent) = {
        let mut rng = world.resource_mut::<GameRng>();
        let excellent = attacker_is_player && rng.0.gen_bool(EXCELLENT_HIT_CHANCE);
        let dice = if excellent { EXCELLENT_HIT_DICE } else { 1 };
        let attack_total: i32 = (0..dice)
            .map(|_| roll_die(&mut rng.0, attacker_power))
            .sum::<i32>()
            + attacker_power_bonus;
        let armor_roll = roll_die(&mut rng.0, target_armor) + target_armor_bonus;
        (attack_total, armor_roll, excellent)
    };

    let mut damage = attack_total - armor_roll;

    // --- Player-only chip damage floor -----------------------------------
    // The player always scrapes off at least 1 HP even when the armour roll
    // eats the whole blow — but a blow that weak can never be the killing one.
    // It can leave a foe on 1 HP; it can't take the last point.
    let mut glancing = false;
    if attacker_is_player && damage < CHIP_DAMAGE {
        damage = CHIP_DAMAGE;
        glancing = true;
    }
    let mut damage = damage.max(0);
    if let Some(f) = world.get::<Fighter>(target).filter(|_| glancing) {
        damage = damage.min((f.hp - 1).max(0));
    }

    // Last of all, the ceiling. A bow in the hand caps the swing at a bruise
    // however the dice fell, and it is applied after the chip-damage floor so a
    // cap of 0 really is 0. Nothing here knows what a bow is: it asks the gear.
    if let Some(cap) = melee_cap(world, attacker) {
        damage = damage.min(cap);
    }

    // --- Apply & report --------------------------------------------------
    let attacker_name = entity_name(world, attacker);
    let target_name = entity_name(world, target);
    let target_is_player = world.get::<Player>(target).is_some();
    // An attacker the player can't see — an invisible phantom, or a mob still off
    // in the dark — is reported only as "Something".
    let attacker_unseen = target_is_player && world.get::<Hidden>(attacker).is_some();

    // A vorpalized weapon that draws blood slays its bane outright — and any
    // creature carrying `VorpalTarget` (the Jabberwock), whatever
    // the bane. A glancing scrape never triggers it.
    let vorpal = !glancing
        && damage > 0
        && wielded_vorpal_bane(world, attacker).is_some_and(|bane| {
            world.get::<VorpalTarget>(target).is_some()
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
                log.add(format!(
                    "Snicker-snack! The blade shears clean through the {target_name}!"
                ));
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

    if lethal {
        if target_is_player {
            // The player does not leave the world; the main loop notices the
            // Ending resource, tears down the save, and shows the death screen.
            let mut ending = world.resource_mut::<Ending>();
            ending.player_dead = true;
            ending.cause = format!("Slain by the {attacker_name}");
        } else {
            leave_gear_behind(world, target);
            world.despawn(target);
        }
    }
}
