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
        return 0;
    }
    rng.gen_range(1..=sides)
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
/// resolve its own lethality — a wand bolt, and any blast casualty
/// [`crate::items::wands::elemental_blast`] didn't already finish off itself
/// (it does, when it has a blast centre to fling a corpse away from; this is
/// the catch-all for the rest). Melee kills are still finalised inline by
/// [`resolve_attack`], so by the time this runs the only casualties left are
/// indirect ones with no known source to fling a corpse away from.
pub fn reaper_system(world: &mut World) {
    let doomed: Vec<Entity> = {
        let mut q = world.query::<(Entity, &Fighter)>();
        q.iter(world)
            .filter(|(_, f)| f.hp <= 0)
            .map(|(e, _)| e)
            .collect()
    };

    for entity in doomed {
        finish_indirect_kill(world, entity, None);
    }
}

/// A tick of recoil for a creature dying where the player can see it: a kill is
/// worth exactly one frame of punctuation, which is why it takes the short kick
/// rather than the heavy one an excellent hit gets. Gated on sight like every
/// other cosmetic, so a monster dying in a room the player has never entered
/// doesn't announce itself through the floor.
///
/// It never steps on a bigger shake: a blast that killed a whole room is
/// already rocking harder than this, and [`Shake::kick`](crate::shake::Shake::kick)
/// keeps whichever is worth more.
fn kill_shake(world: &mut World, victim: Entity) {
    let Some(pos) = world.get::<Position>(victim).copied() else {
        return;
    };
    if !crate::helpers::player_sees(world, pos.x, pos.y) {
        return;
    }
    crate::shake::kick_shake(world, crate::shake::ShakeKind::Kill);
}

/// Blanks an entity's on-screen glyph to a blank space — used only to hide
/// the player's `@` the instant they die, so the death burst's flung corpse
/// and bone shrapnel read as *them* exploding rather than a corpse detaching
/// from a body still visibly standing there. The player entity is never
/// despawned (the death screen still needs it), so there's nothing to
/// restore it: the run is over.
fn blank_player_glyph(world: &mut World, entity: Entity) {
    if let Some(mut r) = world.get_mut::<Renderable>(entity) {
        r.glyph = ' ';
    }
}

/// Finalises one creature that dropped to lethal HP through a source with no
/// attacker entity to report — a wand bolt, or a blast casualty
/// [`crate::items::wands::elemental_blast`] hands off directly rather than
/// waiting for [`reaper_system`]'s next sweep. Plays the death burst, then
/// either despawns a monster (gear settled first, a plain "dies" line
/// logged) or, for the player, blanks their glyph and flags [`Ending`] —
/// guarded so a player already marked dead this run is neither burst nor
/// re-flagged a second time by a later casualty in the same blast.
///
/// `source` is where the corpse should fly away from — a blast's centre, say
/// — or `None` for a random fling direction.
pub(crate) fn finish_indirect_kill(world: &mut World, entity: Entity, source: Option<Position>) {
    if world.get::<Player>(entity).is_some() {
        if world.resource::<Ending>().player_dead {
            return;
        }
        crate::helpers::death_burst(world, entity, source);
        blank_player_glyph(world, entity);
        crate::shake::kick_shake(world, crate::shake::ShakeKind::Death);
        let mut ending = world.resource_mut::<Ending>();
        ending.player_dead = true;
        ending.cause = "Killer unknown".to_string();
        return;
    }

    let name = entity_name(world, entity);
    world
        .resource_mut::<GameLog>()
        .add(format!("The {name} dies."));
    pay_for_the_corpse(world, entity);
    kill_shake(world, entity);
    crate::helpers::death_burst(world, entity, source);
    leave_gear_behind(world, entity);
    world.despawn(entity);
}

/// What a corpse is worth, paid into the player's score the moment a creature
/// stops being one ([`crate::score::award_kill`]).
///
/// It never asks whose blade it was. Half the ways a monster dies in roog have
/// no swinger to ask about — a bolt, a blast a room away, a trapdoor it walked
/// into — and a scoreboard that paid for some of those and not others would
/// only be teaching the player to kill things in the approved fashion.
fn pay_for_the_corpse(world: &mut World, victim: Entity) {
    let Some(max_hp) = world.get::<Fighter>(victim).map(|f| f.max_hp) else {
        return;
    };
    crate::score::award_kill(world, max_hp);
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
        if !world
            .resource_mut::<GameRng>()
            .0
            .gen_bool(GEAR_SURVIVES_DEATH)
        {
            world.entity_mut(item).despawn();
            continue;
        }
        let name = crate::identify::display_name(world, item);
        world.entity_mut(item).insert(pos);
        world
            .resource_mut::<GameLog>()
            .add(format!("The {name} clatters to the floor."));
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
///   armour is subtracted. Always lands for at least [`CHIP_DAMAGE`], whatever
///   the armour roll or a melee cap left it at — a crit is never reported as
///   having done nothing.
/// * **Chip damage** — a non-excellent player swing still deals at least 1
///   damage, even when the armour roll fully absorbs the weapon roll (logged
///   as a "glancing blow"). Unlike an excellent hit, a glancing blow can never
///   be the killing one — it leaves a foe on 1 HP.
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
    // It can leave a foe on 1 HP; it can't take the last point. An excellent
    // hit is not this: it is a good roll that still happened to net under the
    // floor after a hard armour roll, not a whiff — so it skips the "can't
    // finish them" clamp entirely and gets its own floor below instead.
    let mut glancing = false;
    if attacker_is_player && !excellent && damage < CHIP_DAMAGE {
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

    // An excellent hit is never reported as having done nothing — whatever the
    // armour roll or a launcher's melee cap left it at, it lands for at least
    // [`CHIP_DAMAGE`].
    if excellent {
        damage = damage.max(CHIP_DAMAGE);
    }

    // --- Apply & report --------------------------------------------------
    // Captured before the target (on a lethal hit) or the attacker (should
    // this ever run after the attacker itself died) leaves the world.
    let attacker_pos = world.get::<Position>(attacker).copied();
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
    let hp_before = world.get::<Fighter>(target).map(|f| f.hp);
    if let Some(mut fighter) = world.get_mut::<Fighter>(target) {
        fighter.hp -= damage;
        if vorpal {
            fighter.hp = 0;
        }
        lethal = fighter.hp <= 0;
    }
    // Whatever the attacker's own magic does to something it just hit — a
    // charmed pair of hands passing its confusion on, an aquator's touch eating
    // the armour. One table (`crate::abilities::ON_HIT_ABILITIES`), and combat
    // never learns what is in it: it only says what kind of blow this was.
    if damage > 0 {
        let blow = crate::abilities::Blow { glancing, lethal };
        crate::abilities::fire_on_hit(world, attacker, target, blow);
    }
    if damage > 0 {
        crate::helpers::spill_blood(world, target, damage, glancing);
        // A blow landed in melee costs its victim exactly what a dart or a bolt
        // would — a broken promise, the low-HP warning. This is the only damage
        // path that doesn't run through `helpers::apply_damage`, so it has to
        // ask for that by hand.
        crate::helpers::took_damage(world, target, hp_before);
    }

    // Instant hit feedback: a spark where the blow landed, a cold clink for a
    // swing the armour turned aside, or a faint tick for one that did nothing
    // at all. Purely cosmetic; `target` still has its Position here even on a
    // lethal hit (the despawn happens further down).
    if let Some(tpos) = world.get::<Position>(target).copied() {
        // The effect layer is optional (tests run without it).
        if let Some(mut fx) = world.get_resource_mut::<Particles>() {
            match (damage, glancing) {
                (0, _) => fx.blip(tpos.x, tpos.y, '·', Color::DarkGrey),
                // A glancing blow scrapes off its chip of HP without ever
                // getting through the armour, and the spark says so: it is
                // the one hit that draws blood and still gets no shake.
                (_, true) => fx.clink_spark(tpos.x, tpos.y),
                _ => fx.hit_spark(tpos.x, tpos.y),
            }
        }
    }

    // ...and a thump through the whole map for the one swing in seven that
    // lands clean. The spark says *where* the blow landed; the shake says how
    // hard. Only the player's own hits shake the screen — a monster's blow
    // reaches the map through the low-HP crossing, or not at all.
    if excellent {
        crate::shake::kick_shake(world, crate::shake::ShakeKind::Heavy);
    }

    // Every other swing of theirs that got through armour gets the lightest
    // kick in the set. Three exclusions, and each is somebody else's shake or
    // nobody's: a crit already took the heavy one above, a kill takes its own
    // below, and a glancing scrape is the game saying the armour ate the blow
    // — chip damage exists so the swing isn't *nothing*, not so it thumps.
    if attacker_is_player && !excellent && !glancing && !lethal && damage > 0 {
        crate::shake::kick_shake(world, crate::shake::ShakeKind::Hit);
    }

    // A kill is worth its own, shorter kick — and it has to be asked for here,
    // while the corpse still has the Position the sight gate reads.
    if lethal && !target_is_player {
        kill_shake(world, target);
    }

    // The Mortal-Kombat-style death flourish — flung corpse, bone shrapnel,
    // a wall splatter if it earns one. Needs `target`'s Position/Renderable,
    // so it must run before the despawn further down.
    if lethal {
        crate::helpers::death_burst(world, target, attacker_pos);
    }

    let mut log = world.resource_mut::<GameLog>();
    if attacker_is_player {
        report_player_hit(
            &mut log,
            &target_name,
            damage,
            excellent,
            glancing,
            lethal,
            vorpal,
        );
    }
    if !attacker_is_player {
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
        report_monster_hit(
            &mut log,
            &atk,
            &target_label,
            &target_name,
            damage,
            lethal,
            target_is_player,
        );
    }

    if lethal && target_is_player {
        // The player does not leave the world; the main loop notices the
        // Ending resource, tears down the save, and shows the death screen.
        // Their `@` is blanked so the death burst's flung corpse reads as
        // them exploding, not detaching from a body still standing there, and
        // the map takes the biggest lurch it has in it on the way out.
        blank_player_glyph(world, target);
        crate::shake::kick_shake(world, crate::shake::ShakeKind::Death);
        let mut ending = world.resource_mut::<Ending>();
        ending.player_dead = true;
        ending.cause = format!("Slain by the {attacker_name}");
    }
    if lethal && !target_is_player {
        pay_for_the_corpse(world, target);
        leave_gear_behind(world, target);
        world.despawn(target);
    }
}

/// Writes the player-attacked-something lines to the log: the hit line (an
/// excellent hit, a glancing scrape, or a plain blow) and, on a kill, the
/// vorpal flourish and the slain line.
fn report_player_hit(
    log: &mut GameLog,
    target_name: &str,
    damage: i32,
    excellent: bool,
    glancing: bool,
    lethal: bool,
    vorpal: bool,
) {
    match (excellent, glancing) {
        (true, _) => log.add(format!(
            "You score an excellent hit on the {target_name} for {damage} damage!"
        )),
        (_, true) => log.add(format!("You deal a glancing blow to the {target_name}.")),
        _ => log.add(format!("You hit the {target_name} for {damage} damage.")),
    }
    if lethal && vorpal {
        log.add(format!(
            "Snicker-snack! The blade shears clean through the {target_name}!"
        ));
    }
    if lethal {
        log.add(format!("You have slain the {target_name}!"));
    }
}

/// Writes the something-attacked-a-creature lines to the log. `atk` and
/// `target_label` are pre-rendered ("The orc" / "Something", "you" / "the rat")
/// so this never has to know whether the player was on either end.
fn report_monster_hit(
    log: &mut GameLog,
    atk: &str,
    target_label: &str,
    target_name: &str,
    damage: i32,
    lethal: bool,
    target_is_player: bool,
) {
    match damage {
        0 => log.add(format!("{atk} misses {target_label}.")),
        _ => log.add(format!("{atk} hits {target_label} for {damage} damage.")),
    }
    if lethal && target_is_player {
        log.add(format!("{atk} strikes you down..."));
    }
    if lethal && !target_is_player {
        log.add(format!("{atk} kills the {target_name}!"));
    }
}
