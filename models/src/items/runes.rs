//! Invoking a rune.
//!
//! Each row of [`crate::catalog::RUNES`] names one of the handlers here as its
//! `apply`. A rune is a scroll that goes inert instead of crumbling, so
//! [`super::resolve_use`] spends its [`Rune::charged`] and calls the row's
//! handler; a staircase wakes it again through [`recharge_runes`]. Only the
//! player ever invokes one. Chaos and Ice cast the spell they are named for,
//! free; the spell's own flourish and log lines are the rune's.

use bevy_ecs::{entity::Entity, world::World};

use crate::catalog::{RUNES, SPELLS, SpellDef};
use crate::components::*;
use crate::constants::runes::*;
use crate::effects::{ExplodesOnDeath, Grant, Lifetime, Protected, lend};
use crate::helpers::hostiles_in_view;

use super::spells::cast_spell;

use crate::catalog::index_of;

const HASTE_SELF: usize = index_of!(SPELLS, "Haste Self");
const FROST_NOVA: usize = index_of!(SPELLS, "Frost Nova");
const BLANK_RUNE: usize = index_of!(RUNES, "blank rune");

/// The blank rune: does nothing, and says so.
pub(crate) fn blank(world: &mut World, _user: Entity) {
    log(world, strings::rune_blank_nothing());
}

/// Rune of displacement: a scroll of teleportation.
pub(crate) fn displacement(world: &mut World, user: Entity) {
    super::scrolls::teleport_reader(world, user);
}

/// Rune of chaos: the spell Haste Self, free.
pub(crate) fn chaos(world: &mut World, user: Entity) {
    cast_free(world, user, SPELLS[HASTE_SELF]);
}

/// Rune of ice: the spell Frost Nova, free.
pub(crate) fn ice(world: &mut World, user: Entity) {
    cast_free(world, user, SPELLS[FROST_NOVA]);
}

/// Rune of protection: no damage at all for [`PROTECTION_TURNS`] turns
/// ([`Protected`]).
pub(crate) fn protection(world: &mut World, user: Entity) {
    lend(
        world,
        user,
        Grant::of::<Protected>(),
        Lifetime::Turns(PROTECTION_TURNS),
    );
    let line = strings::rune_protection_cast(PROTECTION_TURNS);
    world.resource_mut::<GameLog>().add(line);
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
            rune.charged |= rune.row.name != RUNES[BLANK_RUNE].name;
        }
    }
}

fn log(world: &mut World, line: &str) {
    world.resource_mut::<GameLog>().add(line.to_string());
}

/// Rune of recharging: [`RECHARGE_STEP`] more in every wand `user` carries,
/// never past [`RECHARGE_CAP`] and never lowering one that is already there.
pub(crate) fn recharging(world: &mut World, user: Entity) {
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
pub(crate) fn justice(world: &mut World, user: Entity) {
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

/// `spell` cast by `user` on the spot at no cost, full strength, aimed at
/// where they stand (both spells here ignore the aim).
fn cast_free(world: &mut World, user: Entity, spell: &'static SpellDef) {
    let here = world
        .get::<Position>(user)
        .copied()
        .unwrap_or(Position { x: 0, y: 0 });
    cast_spell(world, spell, user, here, 1);
}
