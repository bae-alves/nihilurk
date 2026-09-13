//! Triggering an active move.
//!
//! A move is coded the way a potion, scroll or wand is: one identity enum
//! ([`MoveEffect`]), one catalog row ([`crate::catalog::MoveDef`]), one
//! mechanic keyed off it — [`apply_move_effect`], the exhaustive match below.
//! What sets it apart from every one of those is that it is never an entity:
//! it has no [`Item`] marker, no [`Position`] on the floor, no pack slot, and
//! it cannot be dropped or thrown. It lives permanently in the triggering
//! creature's [`Moveset`] and costs [`Magic`] per use instead of a battery
//! running dry — see [`move_system`], the schedule step that spends that cost
//! and calls this.

use bevy_ecs::{entity::Entity, world::World};
use rand::Rng;

use crate::components::*;
use crate::effects::loadout;
use crate::helpers::item_label;
use crate::map::GameRng;
use crate::particles::BlastPalette;

use super::wands::elemental_blast;

// --- Tuning constants ------------------------------------------------------
// Defined and documented in `crate::constants::wands`.
//
//   BLAST_RADIUS  the disc a fire blast covers
use crate::constants::wands::BLAST_RADIUS;

/// The schedule step that resolves every move triggered this turn. Spends the
/// [`Magic`] cost first — a stray drain between opening the reticle and
/// confirming it (a wand of cancellation, say) is the one way this can still
/// refuse — then hands off to [`apply_move_effect`].
pub fn move_system(world: &mut World) {
    let moves = std::mem::take(&mut world.resource_mut::<MoveQueue>().moves);
    for wants in moves {
        let def = crate::catalog::MoveDef::of(wants.effect);
        let affordable = world
            .get::<Magic>(wants.user)
            .is_some_and(|m| m.points >= def.cost);
        if !affordable {
            if world.get::<Player>(wants.user).is_some() {
                world
                    .resource_mut::<GameLog>()
                    .add("You don't have the magic for that.".to_string());
            }
            continue;
        }
        if let Some(mut magic) = world.get_mut::<Magic>(wants.user) {
            magic.points -= def.cost;
        }
        world
            .resource_mut::<GameLog>()
            .add(format!("You focus, and unleash your {}!", def.name));
        apply_move_effect(world, wants.user, wants.target, wants.effect);
    }
}

/// Exhaustive over [`MoveEffect`], deliberately with no catch-all: a move
/// added to the enum and not given an arm here fails the build instead of
/// spending its cost for nothing — the same guarantee every other effect
/// table in the game gives.
fn apply_move_effect(world: &mut World, user: Entity, target: Position, effect: MoveEffect) {
    match effect {
        MoveEffect::DragonBreath => breathe_fire(world, user, target),
    }
}

/// Dragon's breath: identical to a zapped wand of fire — the same
/// [`BLAST_RADIUS`] disc, the same armour-ignoring elemental damage — except
/// the damage is whatever the user's own claws (or fists) would deal this
/// swing, not the wand's own dice. See [`crate::items::dragon_breath`], the
/// dragon's own copy of the same trick.
fn breathe_fire(world: &mut World, user: Entity, target: Position) {
    let (power, power_bonus) = {
        let fighter = world.get::<Fighter>(user);
        let loadout = loadout(world, user);
        (
            fighter.map_or(0, |f| f.power) + loadout.power_die,
            fighter.map_or(0, |f| f.power_bonus) + loadout.power_bonus,
        )
    };
    let damage = (world
        .resource_mut::<GameRng>()
        .0
        .gen_range(1..=power.max(1))
        + power_bonus)
        .max(0);
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
