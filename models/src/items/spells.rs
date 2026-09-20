//! Triggering an active spell.
//!
//! A spell is coded the way a potion, scroll or wand is: one identity enum
//! ([`SpellEffect`]), one catalog row ([`crate::catalog::SpellDef`]), one
//! mechanic keyed off it — [`apply_spell_effect`], the exhaustive match below.
//! What sets it apart from every one of those is that it is never an entity:
//! it has no [`Item`] marker, no [`Position`] on the floor, no pack slot, and
//! it cannot be dropped or thrown. It lives permanently in the triggering
//! creature's [`Spellset`] and costs [`Magic`] per use instead of a battery
//! running dry — see [`spell_system`], the schedule step that spends that cost
//! and calls this.
//!
//! Several spells borrow another category's own machinery outright rather
//! than reinvent it: Identify and Magic Mapping are literally
//! [`super::scrolls::apply_scroll_effect`] under a different name, and Lux
//! and Meteor Strike are [`elemental_blast`] played with a stand-in battery.
//! That is deliberate — a spell is a *delivery method*, and the dungeon
//! already knows what a wand of light or a scroll of magic mapping does.

use bevy_ecs::{entity::Entity, world::World};
use crossterm::style::Color;
use rand::Rng;

use crate::components::*;
use crate::conditions::{cure_one_condition, hasten, paralyse};
use crate::effects::{Bided, Grant, Lifetime, MagicWard, TurboMagic, loadout};
use crate::helpers::{
    Hit, apply_hit, get_entities_at_position, get_line, hostiles_in_view, item_label, monster_at,
    roll_dice, spark_burst_at, tile_of, total_armor_plus,
};
use crate::map::{GameRng, Map};
use crate::particles::{BlastPalette, Particles};
use crate::shake::{ShakeKind, kick_shake};
use crate::traps::{TrapBundle, spring_trap, trap_at, trap_damage_tier};

use super::wands::{dazzle, elemental_blast};

// --- Tuning constants ------------------------------------------------------
// Defined and documented in `crate::constants::wands` / `crate::constants::traps`
// / `crate::constants::spells`.
use crate::constants::spells::{
    CIRCLE_OF_DEATH_DAMAGE_DICE, CIRCLE_OF_DEATH_DAMAGE_SIDES, FORCE_LANCE_DAMAGE_DICE,
    FORCE_LANCE_DAMAGE_SIDES, FROST_NOVA_DAMAGE_DICE, FROST_NOVA_DAMAGE_SIDES, LUX_CHARGES,
    METEOR_STRIKE_CHAIN_CHANCE, METEOR_STRIKE_CHAIN_SPREAD, METEOR_STRIKE_CHARGES,
    THUNDERBOLT_DAMAGE_DICE, THUNDERBOLT_DAMAGE_SIDES, THUNDERBOLT_PARALYZE_CHANCE,
    TURBO_MAGIC_COST_MULT, TURBO_MAGIC_POWER_MULT,
};
use crate::constants::traps::{
    DART_DAMAGE_DICE, DART_DAMAGE_SIDES, DART_POWER_DRAIN_BASE, DART_POWER_DRAIN_PER_TIER,
};
use crate::constants::wands::{BLAST_RADIUS, GRENADE_DIE_PER_CHARGE, GRENADE_RADIUS};

/// Whether `user` is wielding a staff — every damaging spell they cast costs
/// double [`Magic`] and deals triple damage, in exchange for the fury behind
/// it. [`TurboMagic`] is lent to the wielder the moment the staff goes on
/// (see [`crate::catalog::WeaponDef::grants`]), so this asks `user` directly,
/// exactly the way any other weapon trick is probed on its wielder rather
/// than the item.
fn wields_turbo_magic(world: &World, user: Entity) -> bool {
    world.get::<TurboMagic>(user).is_some()
}

/// The [`Magic`] cost `user` will actually be charged for casting `effect` —
/// [`SpellDef::cost`](crate::catalog::SpellDef::cost) doubled when `user` wields
/// a staff and the spell is an [`Attack`](SpellKind::Attack). Shared by
/// [`spell_system`] and by callers that need to show or check that true cost
/// before the spell is queued (the reticle affordability check, the spells
/// menu's `Ma` label) so none of them can drift from what will actually be
/// spent.
pub fn spell_cost(world: &World, user: Entity, effect: SpellEffect) -> u8 {
    // What a creature was born with is free to use: see
    // [`crate::abilities::INNATE_SPELLS`]. A dragon has no [`Magic`] at all,
    // so a dragon's breath that cost magic points would never fire; a
    // dragon-bodied player breathes on the same terms.
    if crate::abilities::casts_innately(world, user, effect) {
        return 0;
    }
    let def = crate::catalog::SpellDef::of(effect);
    let turbo = def.kind == SpellKind::Attack && wields_turbo_magic(world, user);
    if turbo {
        def.cost * TURBO_MAGIC_COST_MULT
    } else {
        def.cost
    }
}

/// The schedule step that resolves every spell triggered this turn. Spends the
/// [`Magic`] cost first — a stray drain between opening the reticle and
/// confirming it (a wand of cancellation, say) is the one way this can still
/// refuse — then hands off to [`apply_spell_effect`].
///
/// Assumes nothing upstream but a filled [`SpellQueue`]: affordability was
/// already checked once at the reticle, which is why this re-checks it
/// rather than trusting it — the one queue whose entry can go stale between
/// being queued and being drained.
pub fn spell_system(world: &mut World) {
    let queued = std::mem::take(&mut world.resource_mut::<SpellQueue>().spells);
    for wants in queued {
        // Casting a spell is one of the things that lets go of a rapier's
        // built-up momentum — see `crate::equipment::reset_momentum`.
        crate::equipment::reset_momentum(world, wants.user);
        let def = crate::catalog::SpellDef::of(wants.effect);
        let turbo = def.kind == SpellKind::Attack && wields_turbo_magic(world, wants.user);
        let cost = spell_cost(world, wants.user, wants.effect);
        let affordable = world
            .get::<Magic>(wants.user)
            .is_some_and(|m| m.points >= cost);
        if !affordable {
            if world.get::<Player>(wants.user).is_some() {
                world
                    .resource_mut::<GameLog>()
                    .add("You don't have the magic for that.".to_string());
            }
            continue;
        }
        if let Some(mut magic) = world.get_mut::<Magic>(wants.user) {
            magic.points -= cost;
        }
        world
            .resource_mut::<GameLog>()
            .add(format!("You cast {}!", def.name));
        let power_mult = if turbo { TURBO_MAGIC_POWER_MULT } else { 1 };
        apply_spell_effect(world, wants.user, wants.target, wants.effect, power_mult);
    }
}

/// Exhaustive over [`SpellEffect`], deliberately with no catch-all: a spell
/// added to the enum and not given an arm here fails the build instead of
/// spending its cost for nothing — the same guarantee every other effect
/// table in the game gives.
pub(crate) fn apply_spell_effect(
    world: &mut World,
    user: Entity,
    target: Position,
    effect: SpellEffect,
    power_mult: i32,
) {
    match effect {
        SpellEffect::DragonBreath => breathe_fire(world, user, target, power_mult),
        SpellEffect::Sting => sting(world, user, target, power_mult),
        SpellEffect::Thunderbolt => thunderbolt(world, user, target, power_mult),
        SpellEffect::Cure => cure_self(world, user),
        SpellEffect::Bide => bide(world, user),
        SpellEffect::ForceLance => force_lance(world, user, target, power_mult),
        SpellEffect::Identify => {
            super::scrolls::apply_scroll_effect(world, user, ScrollEffect::Identify)
        }
        SpellEffect::Setup => setup(world, user),
        SpellEffect::Lux => lux(world, user, target, power_mult),
        SpellEffect::CircleOfDeath => circle_of_death(world, user, power_mult),
        SpellEffect::MagicWard => magic_ward(world, user),
        SpellEffect::Heal => heal_self(world, user),
        SpellEffect::MeteorStrike => meteor_strike(world, user, target, power_mult),
        SpellEffect::FrostNova => frost_nova(world, user, power_mult),
        SpellEffect::MagicMapping => {
            super::scrolls::apply_scroll_effect(world, user, ScrollEffect::MagicMapping)
        }
        SpellEffect::HasteSelf => haste_self(world, user),
    }
}

// ---------------------------------------------------------------------------
// Shared flourishes
// ---------------------------------------------------------------------------

/// A cosmetic arrow (or bolt) flight from `from` to `to`, `from`'s own tile
/// excluded — shared by every spell whose flavour is "something flies in on a
/// line": Sting's green dart, Thunderbolt's double bolt.
fn fly_arrow(world: &mut World, from: Position, to: Position, glyph: char, color: Color) {
    let cells: Vec<(u16, u16)> = get_line(from, to)
        .into_iter()
        .filter(|&p| p != from)
        .map(|p| (p.x, p.y))
        .collect();
    if let Some(mut fx) = world.get_resource_mut::<Particles>() {
        fx.hurl(&cells, glyph, color);
    }
}

// ---------------------------------------------------------------------------
// 1 Ma
// ---------------------------------------------------------------------------

/// Sting: the dart trap's own venomed bite, lanced at range instead of laid
/// on the floor — the same [`DART_DAMAGE_DICE`] roll against the target's
/// armour plus, the same depth-scaled poison drain
/// ([`crate::traps::trap_damage_tier`]). A green dart, because the trap's own
/// needle is cyan and this one means it personally.
fn sting(world: &mut World, user: Entity, target: Position, power_mult: i32) {
    let Some(user_pos) = world.get::<Position>(user).copied() else {
        return;
    };
    fly_arrow(world, user_pos, target, '↑', Color::Green);

    let Some(victim) = monster_at(world, target) else {
        world
            .resource_mut::<GameLog>()
            .add("The dart of venom finds nothing to bite.".to_string());
        return;
    };
    let tier = trap_damage_tier(world.resource::<Depth>().what);
    let armor_plus = total_armor_plus(world, victim);
    let roll = roll_dice(world, DART_DAMAGE_DICE, DART_DAMAGE_SIDES);
    let damage = (roll - armor_plus).max(0) * power_mult;
    let name = item_label(world, victim);

    if damage <= 0 {
        world
            .resource_mut::<GameLog>()
            .add(format!("The dart glances off the {name}."));
        return;
    }
    let line = format!("A green dart of venom pricks the {name} for {damage} damage!");
    if apply_hit(world, victim, Hit::magic(damage), Some(&line)) == 0 {
        return;
    }
    if let Some((x, y)) = tile_of(world, victim) {
        if let Some(mut fx) = world.get_resource_mut::<Particles>() {
            fx.hit_spark(x, y);
        }
    }
    let drain = DART_POWER_DRAIN_BASE + tier * DART_POWER_DRAIN_PER_TIER;
    crate::conditions::drain_power(world, victim, drain, Some(1));
}

/// Thunderbolt: `THUNDERBOLT_DAMAGE_DICE`d`THUNDERBOLT_DAMAGE_SIDES`
/// armour-ignoring damage, and [`THUNDERBOLT_PARALYZE_CHANCE`] to lock the
/// target's limbs up on top of it. A yellow double bolt, because one arrow's
/// worth of thunder never seemed like enough.
fn thunderbolt(world: &mut World, user: Entity, target: Position, power_mult: i32) {
    let Some(user_pos) = world.get::<Position>(user).copied() else {
        return;
    };
    fly_arrow(world, user_pos, target, '⇈', Color::Yellow);

    let Some(victim) = monster_at(world, target) else {
        world
            .resource_mut::<GameLog>()
            .add("Thunder cracks over empty stone.".to_string());
        return;
    };
    let damage = roll_dice(world, THUNDERBOLT_DAMAGE_DICE, THUNDERBOLT_DAMAGE_SIDES) * power_mult;
    let name = item_label(world, victim);
    let line = format!("A bolt of thunder slams into the {name} for {damage} damage!");
    if apply_hit(world, victim, Hit::magic(damage), Some(&line)) == 0 {
        return;
    }
    if let Some((x, y)) = tile_of(world, victim) {
        if let Some(mut fx) = world.get_resource_mut::<Particles>() {
            fx.impact_sparks(x, y, Color::Yellow, 0.0);
        }
    }
    let survived = world.get::<Fighter>(victim).is_some_and(|f| f.hp > 0);
    let paralyzes = world
        .resource_mut::<GameRng>()
        .0
        .gen_bool(THUNDERBOLT_PARALYZE_CHANCE);
    if survived && paralyzes {
        paralyse(world, victim);
    }
}

/// Cure: lifts the caster's single worst affliction — [`cure_one_condition`],
/// the exact mechanic a rosé coin spends on you.
fn cure_self(world: &mut World, user: Entity) {
    spark_burst_at(world, user, Color::Green);
    if !cure_one_condition(world, user) {
        world
            .resource_mut::<GameLog>()
            .add("There's nothing wrong with you to cure.".to_string());
    }
}

/// Bide: does nothing to the world and everything to the next swing — see
/// [`Bided`], folded into the very next attack roll
/// [`crate::combat::resolve_attack`] makes for its bearer.
fn bide(world: &mut World, user: Entity) {
    // Through the ledger, with the lifetime that says what ends it: nothing
    // lends this one, so the ledger is the only record, and a run saved mid-coil
    // is reopened still coiled.
    crate::effects::lend(world, user, Grant::of::<Bided>(), Lifetime::NextAction);
    if let Some((x, y)) = tile_of(world, user) {
        if let Some(mut fx) = world.get_resource_mut::<Particles>() {
            fx.condition_mark(x, y, '≡', Color::DarkYellow, 0.0);
            fx.spark_burst(x, y, Color::DarkYellow);
        }
    }
    world
        .resource_mut::<GameLog>()
        .add("You coil, gathering strength for the blow to come.".to_string());
}

// ---------------------------------------------------------------------------
// 2 Ma
// ---------------------------------------------------------------------------

/// Dragon's breath, known to the player as Fireball: identical to a zapped
/// wand of fire — the same [`BLAST_RADIUS`] disc, the same armour-ignoring
/// elemental damage — except the damage is whatever the user's own claws (or
/// fists) would deal this swing, not the wand's own dice.
///
/// The dragon's own breath is this, not a copy of it: its ability row casts
/// the spell (see [`crate::abilities::INNATE_SPELLS`]), which is why the
/// damage is read off the caster's [`Fighter`] rather than a dice row — the
/// same sentence describes a dragon clawing and a player punching.
/// `power_mult` is a staff's [`TurboMagic`] tripling the damage on top of
/// doubling the cost — 1 for anyone casting bare-handed.
fn breathe_fire(world: &mut World, user: Entity, target: Position, power_mult: i32) {
    let (power, power_bonus) = {
        let fighter = world.get::<Fighter>(user);
        let loadout = loadout(world, user);
        (
            fighter.map_or(0, |f| f.power) + loadout.power_die,
            fighter.map_or(0, |f| f.power_bonus) + loadout.power_bonus,
        )
    };
    let damage = ((world
        .resource_mut::<GameRng>()
        .0
        .gen_range(1..=power.max(1))
        + power_bonus)
        .max(0))
        * power_mult;
    let line = match world.get::<Player>(user).is_some() {
        true => "A gout of flame erupts from you!".to_string(),
        false => format!(
            "A gout of flame erupts from the {}!",
            item_label(world, user)
        ),
    };
    world.resource_mut::<GameLog>().add(line);
    elemental_blast(
        world,
        Some(user),
        target,
        BLAST_RADIUS,
        damage,
        Some(Element::Fire),
        BlastPalette::Fire,
    );
}

/// Force Lance: a line of `FORCE_LANCE_DAMAGE_DICE`d`FORCE_LANCE_DAMAGE_SIDES`
/// armour-ignoring damage — a wand of striking's own bolt, cast rather than
/// zapped.
fn force_lance(world: &mut World, user: Entity, target: Position, power_mult: i32) {
    let Some(user_pos) = world.get::<Position>(user).copied() else {
        return;
    };
    let map = world.resource::<Map>().clone();
    let mut cells: Vec<(u16, u16)> = Vec::new();
    let mut victims: Vec<Entity> = Vec::new();
    for pos in get_line(user_pos, target) {
        if pos == user_pos {
            continue;
        }
        if map.blocks(pos.x, pos.y) {
            break;
        }
        cells.push((pos.x, pos.y));
        for entity in get_entities_at_position(world, pos) {
            if entity != user && world.get::<Fighter>(entity).is_some() {
                victims.push(entity);
            }
        }
    }

    world
        .resource_mut::<GameLog>()
        .add("An invisible fist hammers down the line!".to_string());
    let mut flight_ms = 0.0;
    if let Some(mut fx) = world.get_resource_mut::<Particles>() {
        flight_ms = fx.beam(&cells, Color::White);
    }

    for victim in victims {
        let damage =
            roll_dice(world, FORCE_LANCE_DAMAGE_DICE, FORCE_LANCE_DAMAGE_SIDES) * power_mult;
        let name = item_label(world, victim);
        let line = format!("The force lance slams the {name} for {damage} damage!");
        if apply_hit(world, victim, Hit::magic(damage), Some(&line)) == 0 {
            continue;
        }
        if let Some((x, y)) = tile_of(world, victim) {
            if let Some(mut fx) = world.get_resource_mut::<Particles>() {
                fx.impact_sparks(x, y, Color::White, flight_ms);
            }
        }
    }
}

/// Setup: plants a revealed arrow trap on each of the caster's four
/// diagonals — springs, cocked, in plain sight. One standing under a
/// creature already trips the instant it's laid.
fn setup(world: &mut World, user: Entity) {
    let Some(pos) = world.get::<Position>(user).copied() else {
        return;
    };
    const DIAGONALS: [(i32, i32); 4] = [(-1, -1), (1, -1), (-1, 1), (1, 1)];
    let map = world.resource::<Map>().clone();
    let mut placed = 0;

    for (dx, dy) in DIAGONALS {
        let Some((x, y)) = crate::particles::on_map(pos.x as i32 + dx, pos.y as i32 + dy) else {
            continue;
        };
        let spot = Position { x, y };
        if map.blocks(x, y) || trap_at(world, spot).is_some() {
            continue;
        }
        let trap = world.spawn(TrapBundle::arrow(spot)).id();
        world.entity_mut(trap).remove::<Hidden>();
        if let Some(mut t) = world.get_mut::<Trap>(trap) {
            t.revealed = true;
        }
        placed += 1;
        if let Some(mut fx) = world.get_resource_mut::<Particles>() {
            fx.blip(x, y, '^', Color::DarkCyan);
        }

        let occupant = get_entities_at_position(world, spot)
            .into_iter()
            .find(|&e| e != user && world.get::<Fighter>(e).is_some());
        if let Some(victim) = occupant {
            spring_trap(world, trap, victim);
        }
    }

    let msg = if placed > 0 {
        "You plant arrow traps at your flanks, springs cocked in plain sight."
    } else {
        "There's no room at your flanks for a trap."
    };
    world.resource_mut::<GameLog>().add(msg.to_string());
}

// ---------------------------------------------------------------------------
// 3 Ma
// ---------------------------------------------------------------------------

/// Lux: a wand of light hurled rather than zapped — the same wide, hot
/// grenade a thrown attack wand bursts as (see
/// [`super::throwing::resolve_wand_throw`]), except this one blinds too. No
/// battery to read a charge count off, so [`LUX_CHARGES`] stands in for one.
fn lux(world: &mut World, user: Entity, target: Position, power_mult: i32) {
    world
        .resource_mut::<GameLog>()
        .add("You hurl a shard of pure light!".to_string());
    let damage = roll_dice(world, LUX_CHARGES, GRENADE_DIE_PER_CHARGE) * power_mult;
    let caught = elemental_blast(
        world,
        Some(user),
        target,
        GRENADE_RADIUS,
        damage,
        None,
        BlastPalette::Glam,
    );
    for entity in caught {
        let is_creature =
            world.get::<Mob>(entity).is_some() || world.get::<Player>(entity).is_some();
        if !is_creature {
            continue;
        }
        dazzle(world, entity);
    }
}

/// Circle of Death: [`CIRCLE_OF_DEATH_DAMAGE_DICE`]d[`CIRCLE_OF_DEATH_DAMAGE_SIDES`]
/// armour-ignoring drain, rolled once per hostile in view and handed straight
/// back to the caster as HP. Unnecessary flames, and everything they touch
/// turns grey.
fn circle_of_death(world: &mut World, user: Entity, power_mult: i32) {
    let targets = hostiles_in_view(world, user);
    if targets.is_empty() {
        world
            .resource_mut::<GameLog>()
            .add("Ashen light gathers around you and finds nothing at all to feed on.".to_string());
        return;
    }

    world.resource_mut::<GameLog>().add(
        "Ashen light rises off the floor. Unnecessary flames roar through the room, and \
         everything they touch turns grey."
            .to_string(),
    );
    let cells: Vec<(u16, u16, f32)> = targets
        .iter()
        .filter_map(|&e| tile_of(world, e))
        .map(|(x, y)| (x, y, 0.0))
        .collect();
    if let Some(mut fx) = world.get_resource_mut::<Particles>() {
        fx.explosion(&cells, BlastPalette::Death);
    }
    kick_shake(world, ShakeKind::Heavy);

    let mut drained = 0;
    for victim in targets {
        let dmg = roll_dice(
            world,
            CIRCLE_OF_DEATH_DAMAGE_DICE,
            CIRCLE_OF_DEATH_DAMAGE_SIDES,
        ) * power_mult;
        drained += apply_hit(world, victim, Hit::magic(dmg), None);
    }
    if drained <= 0 {
        return;
    }
    if let Some(mut fighter) = world.get_mut::<Fighter>(user) {
        fighter.hp = (fighter.hp + drained).min(fighter.max_hp);
    }
    world
        .resource_mut::<GameLog>()
        .add(format!("You drain {drained} life from the circle."));
}

/// Magic Ward: for the rest of this floor, nothing that isn't the caster's
/// own magic can touch them — every wand-shaped source of harm bounces off
/// outright ([`crate::items::wands::damage_with_element`]), and nothing a
/// monster's blow carries with it takes hold either
/// ([`crate::abilities::fire_on_hit`]). Lifted at the next staircase like any
/// other floor-scoped condition.
fn magic_ward(world: &mut World, user: Entity) {
    crate::effects::lend(world, user, Grant::of::<MagicWard>(), Lifetime::Floor);
    if let Some((x, y)) = tile_of(world, user) {
        if let Some(mut fx) = world.get_resource_mut::<Particles>() {
            fx.spark_burst(x, y, Color::Cyan);
            fx.firework(x, y, Color::White, 120.0);
        }
    }
    world.resource_mut::<GameLog>().add(
        "A cold, silver skin closes over you. Nothing but your own magic can touch you now — \
         not for the rest of this floor."
            .to_string(),
    );
}

/// Heal: refills the caster's HP to their current ceiling — a potion of
/// healing's dose, minus the ceiling raise a potion leaves behind.
fn heal_self(world: &mut World, user: Entity) {
    let Some(mut fighter) = world.get_mut::<Fighter>(user) else {
        return;
    };
    let before = fighter.hp;
    fighter.hp = fighter.max_hp;
    let healed = fighter.hp - before;
    spark_burst_at(world, user, Color::Green);
    let msg = if healed > 0 {
        format!("Warmth floods through you, and your wounds close. (+{healed} HP)")
    } else {
        "You are already at full strength.".to_string()
    };
    world.resource_mut::<GameLog>().add(msg);
}

// ---------------------------------------------------------------------------
// 4 Ma
// ---------------------------------------------------------------------------

/// Meteor Strike: a wand of fire hurled at the sky rather than the floor —
/// the same grenade Lux throws, elemental and hot, with no battery to read a
/// charge count off ([`METEOR_STRIKE_CHARGES`] stands in). After every impact
/// there is a [`METEOR_STRIKE_CHAIN_CHANCE`] chance the sky tears open again,
/// somewhere within [`METEOR_STRIKE_CHAIN_SPREAD`] tiles of the last one —
/// possibly forever, though the odds of it going more than a handful of times
/// running are the odds of flipping a very long streak of heads.
fn meteor_strike(world: &mut World, user: Entity, target: Position, power_mult: i32) {
    world.resource_mut::<GameLog>().add(
        "You hurl fire at the sky. It does not come back down where you'd expect.".to_string(),
    );
    let mut at = target;
    loop {
        let damage = roll_dice(world, METEOR_STRIKE_CHARGES, GRENADE_DIE_PER_CHARGE) * power_mult;
        world
            .resource_mut::<GameLog>()
            .add("A meteor screams down!".to_string());
        elemental_blast(
            world,
            Some(user),
            at,
            GRENADE_RADIUS,
            damage,
            Some(Element::Fire),
            BlastPalette::Fire,
        );
        kick_shake(world, ShakeKind::Heavy);

        let chains = world
            .resource_mut::<GameRng>()
            .0
            .gen_bool(METEOR_STRIKE_CHAIN_CHANCE);
        if !chains {
            return;
        }
        let (dx, dy) = {
            let mut rng = world.resource_mut::<GameRng>();
            (
                rng.0
                    .gen_range(-METEOR_STRIKE_CHAIN_SPREAD..=METEOR_STRIKE_CHAIN_SPREAD),
                rng.0
                    .gen_range(-METEOR_STRIKE_CHAIN_SPREAD..=METEOR_STRIKE_CHAIN_SPREAD),
            )
        };
        let Some((x, y)) = crate::particles::on_map(at.x as i32 + dx, at.y as i32 + dy) else {
            return;
        };
        world
            .resource_mut::<GameLog>()
            .add("The sky tears open again!".to_string());
        at = Position { x, y };
    }
}

/// Frost Nova: [`FROST_NOVA_DAMAGE_DICE`]d[`FROST_NOVA_DAMAGE_SIDES`]
/// armour-ignoring cold damage to everything in view, paralysed on top of it
/// if it survives. Glam rock, cyan, and entirely too much of both.
fn frost_nova(world: &mut World, user: Entity, power_mult: i32) {
    let Some(pos) = world.get::<Position>(user).copied() else {
        return;
    };
    let targets = hostiles_in_view(world, user);

    world.resource_mut::<GameLog>().add(
        "A CYAN STAR erupts around you — ice, glitter, and entirely too much of both.".to_string(),
    );
    if let Some(mut fx) = world.get_resource_mut::<Particles>() {
        const POINTS: [(i32, i32); 8] = [
            (0, -3),
            (2, -2),
            (3, 0),
            (2, 2),
            (0, 3),
            (-2, 2),
            (-3, 0),
            (-2, -2),
        ];
        for (i, (dx, dy)) in POINTS.into_iter().enumerate() {
            if let Some((x, y)) = crate::particles::on_map(pos.x as i32 + dx, pos.y as i32 + dy) {
                fx.firework(x, y, Color::Cyan, i as f32 * 30.0);
            }
        }
        fx.explosion(&[(pos.x, pos.y, 0.0)], BlastPalette::Glam);
    }
    kick_shake(world, ShakeKind::Heavy);

    for victim in targets {
        // The ward and the cold-immunity were written out here by hand, the
        // second a copy of `damage_with_element`'s. `apply_hit` owns both.
        let dmg = roll_dice(world, FROST_NOVA_DAMAGE_DICE, FROST_NOVA_DAMAGE_SIDES) * power_mult;
        if apply_hit(world, victim, Hit::elemental(dmg, Element::Cold), None) == 0 {
            continue;
        }
        if world.get::<Fighter>(victim).is_some_and(|f| f.hp > 0) {
            paralyse(world, victim);
        }
    }
}

/// Haste Self: not merely quick. Not merely fast. THE FAST — a rainbow of
/// fireworks around the caster and every ounce of ceremony a plain potion of
/// haste never got.
fn haste_self(world: &mut World, user: Entity) {
    let Some(pos) = world.get::<Position>(user).copied() else {
        return;
    };
    world
        .resource_mut::<GameLog>()
        .add("Power gathers. Power gathers more.".to_string());
    hasten(world, user);
    if let Some(mut fx) = world.get_resource_mut::<Particles>() {
        const COLORS: [Color; 6] = [
            Color::Yellow,
            Color::Magenta,
            Color::Cyan,
            Color::White,
            Color::Red,
            Color::Yellow,
        ];
        for (i, &color) in COLORS.iter().enumerate() {
            fx.firework(pos.x, pos.y, color, i as f32 * 110.0);
        }
    }
    world
        .resource_mut::<GameLog>()
        .add("You are not quick. You are not swift. You are THE FAST.".to_string());
}
