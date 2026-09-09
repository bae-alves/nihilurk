//! What a *passive* effect does on its own, every turn.
//!
//! Some effects are answers to a question another system asks — [`FireImmune`]
//! only matters when a wand of fire goes off. Others act by themselves: while
//! you carry [`AggravatesMonsters`], the floor keeps noticing you. Those live
//! here, one row each.
//!
//! The row names the effect with the same [`Grant`] handle the bestiary and the
//! ring catalog use, so nothing in this table knows or cares what granted the
//! ability. A ring grants it today; a cursed blade or a monster's aura could
//! grant it tomorrow and the behaviour would follow, untouched.

use bevy_ecs::prelude::*;
use rand::Rng;

use crate::components::{GameLog, Player};
use crate::effects::{AggravatesMonsters, Grant};
use crate::map::GameRng;

/// One self-acting passive: the effect that arms it, the odds it fires on any
/// given turn, the mechanic it runs, and the line logged when it goes off.
pub struct PassiveAbility {
    /// The marker component that arms this ability.
    pub effect: Grant,
    /// Probability it fires on each turn its bearer acts.
    pub chance: f64,
    /// The mechanic, run on the bearer.
    pub action: fn(&mut World, Entity),
    /// Flavour logged when it fires. Written in second person, so it is only
    /// logged for the player.
    pub flavour: &'static str,
}

/// Every passive that acts on its own. A new "happens at random while you have
/// it" ability is one row plus the mechanic it calls.
pub const PASSIVE_ABILITIES: &[PassiveAbility] = &[PassiveAbility {
    effect: Grant::of::<AggravatesMonsters>(),
    chance: 0.10,
    action: crate::items::aggravate_all_monsters,
    flavour: "You yip! The whole floor turns your way.",
}];

/// Rolls every passive ability its bearer currently has armed.
///
/// Registered in the turn schedule ahead of [`crate::ai`]; the schedule only
/// runs on turns the player took an action, so "every turn" means "every
/// action". The system asks only "who carries this effect" — it never asks
/// where the effect came from, which is the whole point.
pub fn passive_ability_system(world: &mut World) {
    for ability in PASSIVE_ABILITIES {
        let bearers: Vec<Entity> = world
            .iter_entities()
            .map(|e| e.id())
            .filter(|&e| ability.effect.probe(world, e))
            .collect();

        for bearer in bearers {
            if !world.resource_mut::<GameRng>().0.gen_bool(ability.chance) {
                continue;
            }
            (ability.action)(world, bearer);
            if world.get::<Player>(bearer).is_some() {
                world
                    .resource_mut::<GameLog>()
                    .add(ability.flavour.to_string());
            }
        }
    }
}
