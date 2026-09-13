//! What an effect does *on its own* — one table per moment it can act on.
//!
//! Some effects are answers to a question another system asks — [`FireImmune`]
//! only matters when a wand of fire goes off. Others act by themselves, and
//! there are two moments they can pick:
//!
//! * **Every turn** ([`PASSIVE_ABILITIES`]) — while you carry
//!   [`AggravatesMonsters`], the floor keeps noticing you.
//! * **On a blow that lands** ([`ON_HIT_ABILITIES`]) — a charmed pair of hands
//!   passes its confusion on, an aquator's touch eats the armour it hit.
//!
//! Both tables name their effect with the same [`Grant`] handle the bestiary and
//! the ring catalog use, so nothing here knows or cares what granted the
//! ability. A ring grants it today; a cursed blade or a monster's aura could
//! grant it tomorrow and the behaviour would follow, untouched. And both are the
//! answer to the same question: *when* does this fire, and *what* does it do.

use bevy_ecs::prelude::*;
use rand::Rng;

use crate::components::{ConfusingTouch, GameLog, Player};
use crate::effects::{AggravatesMonsters, Grant, Regenerates, RustsArmor, Teleportitis};
use crate::map::GameRng;

/// One self-acting passive: the effect that arms it, the odds it fires on any
/// given turn, the mechanic it runs, and the line logged when it goes off.
pub struct PassiveAbility {
    /// The marker component that arms this ability.
    pub effect: Grant,
    /// Probability it fires on each turn its bearer acts.
    pub chance: f64,
    /// The mechanic, run on the bearer. Reports whether it actually did
    /// anything: a ring of regeneration wins its roll every other turn and must
    /// be silent on the ones where there was nothing left to mend.
    pub action: fn(&mut World, Entity) -> bool,
    /// Flavour logged when the mechanic did something. Written in second
    /// person, so it is only logged for the player.
    pub flavour: &'static str,
}

/// Every passive that acts on its own. A new "happens at random while you have
/// it" ability is one row plus the mechanic it calls.
pub const PASSIVE_ABILITIES: &[PassiveAbility] = &[
    PassiveAbility {
        effect: Grant::of::<AggravatesMonsters>(),
        chance: 0.10,
        action: crate::items::aggravate_all_monsters,
        flavour: "You yip! The whole floor turns your way.",
    },
    PassiveAbility {
        effect: Grant::of::<Regenerates>(),
        chance: 0.50,
        action: crate::items::regenerate,
        flavour: "The ring on your finger is warm.",
    },
    // Rogue's teleportitis, at NetHack's odds: 1 in 85 turns, and the jump lands
    // at the top of the bearer's next turn (see `passive_ability_system`).
    PassiveAbility {
        effect: Grant::of::<Teleportitis>(),
        chance: 1.0 / 85.0,
        action: crate::items::teleportitis,
        flavour: "Something on your finger is pleased with itself.",
    },
];

// ---------------------------------------------------------------------------
// On a blow that lands
// ---------------------------------------------------------------------------

/// One thing that happens *because a blow connected*: the effect the attacker
/// must carry, which blows count, and what it does to whoever was hit.
///
/// The two rows are the aquator's corrosive touch and a scroll of monster
/// confusion's charm. Neither is a special case in [`crate::combat`] any more —
/// `resolve_attack` fires the table and never learns what is in it.
pub struct OnHitAbility {
    /// The marker component on the *attacker* that arms this.
    pub effect: Grant,
    /// Whether a glancing scrape counts. Acid does not care that the armour
    /// turned the blow — it landed on the armour. A charm needs to reach skin.
    pub on_glancing: bool,
    /// Whether the killing blow counts. There is no point charming a corpse;
    /// there is no harm eating the plus off what it was wearing.
    pub on_lethal: bool,
    /// The mechanic, run as `(attacker, target)`.
    pub action: fn(&mut World, Entity, Entity),
}

/// Every on-hit ability in the game.
pub const ON_HIT_ABILITIES: &[OnHitAbility] = &[
    OnHitAbility {
        effect: Grant::of::<ConfusingTouch>(),
        on_glancing: false,
        on_lethal: false,
        action: crate::items::discharge_confusing_touch,
    },
    OnHitAbility {
        effect: Grant::of::<RustsArmor>(),
        on_glancing: true,
        on_lethal: true,
        action: corrode,
    },
];

/// [`crate::equipment::corrode_armor`] with the table's shape: an on-hit
/// ability is handed both ends of the blow, and this one only cares about the
/// end that was wearing something.
fn corrode(world: &mut World, _attacker: Entity, target: Entity) {
    crate::equipment::corrode_armor(world, target);
}

/// Fires every on-hit ability `attacker` has armed against `target`. Called by
/// [`crate::combat::resolve_attack`] for every blow that drew blood, with the
/// shape of the blow so each row can bow out of the ones it does not want.
pub fn fire_on_hit(world: &mut World, attacker: Entity, target: Entity, blow: Blow) {
    for ability in ON_HIT_ABILITIES {
        if blow.glancing && !ability.on_glancing {
            continue;
        }
        if blow.lethal && !ability.on_lethal {
            continue;
        }
        if !ability.effect.probe(world, attacker) {
            continue;
        }
        (ability.action)(world, attacker, target);
    }
}

/// What kind of blow just landed, for the rows that care. Damage above zero is
/// assumed — a blow that did nothing never reaches the table.
#[derive(Clone, Copy)]
pub struct Blow {
    /// The armour ate it and the player's chip-damage floor is all that got
    /// through.
    pub glancing: bool,
    /// It was the last one.
    pub lethal: bool,
}

/// Rolls every passive ability its bearer currently has armed.
///
/// Registered at the **tail** of the turn schedule, after the monsters have
/// moved and before visibility is recomputed. The schedule only runs on turns
/// the player took an action, so "every turn" means "every action" — and
/// running last means a passive that *moves* its bearer lands at the top of
/// their next turn: they see where they ended up and act from there before
/// anything on the floor gets another move. That is the difference between a
/// ring of teleportation and a curse.
///
/// The system asks only "who carries this effect" — it never asks where the
/// effect came from, which is the whole point.
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
            let did_something = (ability.action)(world, bearer);
            if did_something && world.get::<Player>(bearer).is_some() {
                world
                    .resource_mut::<GameLog>()
                    .add(ability.flavour.to_string());
            }
        }
    }
}
