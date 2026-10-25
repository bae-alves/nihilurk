//! What a thief does with what it takes.
//!
//! The thief lives behind [`crate::abilities`]'s on-hit table, which owns
//! *finding* what to steal ([`crate::abilities::steal_equipped_item`]).

use bevy_ecs::{entity::Entity, world::World};

use crate::abilities::steal_equipped_item;
use crate::components::*;
use crate::helpers::{item_label, leave_smoke};

/// The thief's on-hit: strip one piece of equipped gear and disappear the
/// instant it does — no immediate use, no theatre, just gone with it.
pub(crate) fn theft(world: &mut World, attacker: Entity, target: Entity) {
    let Some(item) = steal_equipped_item(world, target) else {
        return;
    };
    let item_name = item_label(world, item);
    let attacker_name = item_label(world, attacker);
    let target_label = victim_label(world, target);
    world
        .resource_mut::<GameLog>()
        .add(strings::theft(&attacker_name, &item_name, &target_label));
    if let Some(pos) = world.get::<Position>(attacker).copied() {
        leave_smoke(world, pos);
    }
    world.entity_mut(item).despawn();
    crate::spirits::poof(world, attacker);
}

/// `"you"` for the player, `"the orc"` for anything else — the object half of
/// a theft's sentence.
fn victim_label(world: &World, target: Entity) -> String {
    match world.get::<Player>(target).is_some() {
        true => "you".to_string(),
        false => strings::the(&item_label(world, target)),
    }
}
