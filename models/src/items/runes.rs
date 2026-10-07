//! Invoking a rune.
//!
//! The catalog ([`crate::catalog::RUNES`]) names each rune; this file is where
//! the one on it takes effect, keyed by [`RuneEffect`]. A rune is a scroll that
//! goes inert instead of crumbling, so [`super::resolve_use`] spends its
//! [`Rune::charged`] and calls [`apply_rune_effect`]; a staircase wakes it again
//! through [`recharge_runes`]. Only the player ever invokes one.

use bevy_ecs::{entity::Entity, world::World};

use crate::components::*;
use crate::constants::runes::*;
use crate::effects::{ExplodesOnDeath, Grant, Lifetime, Protected, lend};
use crate::helpers::hostiles_in_view;

use super::scrolls::apply_scroll_effect;
use super::spells::apply_spell_effect;

/// Exhaustive over [`RuneEffect`], deliberately with no catch-all: a rune added
/// to the enum and not given an arm here fails the build instead of reading as
/// nothing — the same guarantee [`super::scrolls::apply_scroll_effect`] gives a
/// new scroll. Chaos and Ice cast the spell they are named for, free; the
/// spell's own flourish and log lines are the rune's.
pub(super) fn apply_rune_effect(world: &mut World, user: Entity, effect: RuneEffect) {
    match effect {
        RuneEffect::Blank => log(world, strings::rune_blank_nothing()),
        RuneEffect::Recharging => recharge_wands(world, user),
        RuneEffect::Displacement => apply_scroll_effect(world, user, ScrollEffect::Teleportation),
        RuneEffect::Justice => mark_the_watchers(world, user),
        RuneEffect::Chaos => cast_free(world, user, SpellEffect::HasteSelf),
        RuneEffect::Ice => cast_free(world, user, SpellEffect::FrostNova),
        RuneEffect::Protection => {
            lend(
                world,
                user,
                Grant::of::<Protected>(),
                Lifetime::Turns(PROTECTION_TURNS),
            );
            let line = strings::rune_protection_cast(PROTECTION_TURNS);
            world.resource_mut::<GameLog>().add(line);
        }
    }
}

/// Wakes every spent rune `player` is carrying: what a staircase does. A blank
/// rune has nothing to wake, so it stays as cancellation left it.
pub(crate) fn recharge_runes(world: &mut World, player: Entity) {
    let carried = world
        .get::<Backpack>(player)
        .map(|bp| bp.items.clone())
        .unwrap_or_default();
    for item in carried {
        if let Some(mut rune) = world.get_mut::<Rune>(item) {
            rune.charged |= rune.effect != RuneEffect::Blank;
        }
    }
}

fn log(world: &mut World, line: &str) {
    world.resource_mut::<GameLog>().add(line.to_string());
}

/// Rune of recharging: [`RECHARGE_STEP`] more in every wand `user` carries,
/// never past [`RECHARGE_CAP`] and never lowering one that is already there.
fn recharge_wands(world: &mut World, user: Entity) {
    let carried = world
        .get::<Backpack>(user)
        .map(|bp| bp.items.clone())
        .unwrap_or_default();
    let mut found = false;
    for item in carried {
        if world.get::<Wand>(item).is_none() {
            continue;
        }
        if let Some(mut battery) = world.get_mut::<Battery>(item) {
            found = true;
            let topped = (battery.charges + RECHARGE_STEP).min(RECHARGE_CAP);
            battery.charges = battery.charges.max(topped);
        }
    }
    log(
        world,
        match found {
            true => strings::rune_recharged(),
            false => strings::rune_recharge_nothing(),
        },
    );
}

/// Rune of justice: every hostile in view will burst when it dies
/// ([`ExplodesOnDeath`]). Held for the floor, which is as long as they live.
fn mark_the_watchers(world: &mut World, user: Entity) {
    let watchers = hostiles_in_view(world, user);
    let line = match watchers.is_empty() {
        true => strings::rune_justice_nobody(),
        false => strings::rune_justice_marks(),
    };
    for watcher in watchers {
        lend(
            world,
            watcher,
            Grant::of::<ExplodesOnDeath>(),
            Lifetime::Floor,
        );
    }
    log(world, line);
}

/// `effect` cast by `user` on the spot at no cost, full strength, aimed at
/// where they stand (both spells here ignore the aim).
fn cast_free(world: &mut World, user: Entity, effect: SpellEffect) {
    let here = world
        .get::<Position>(user)
        .copied()
        .unwrap_or(Position { x: 0, y: 0 });
    apply_spell_effect(world, user, here, effect, 1);
}
