//! One blow, start to finish: the opposed roll, the weapon tricks riding on
//! top of it, and everything a landed hit sets off — the log lines, the
//! sparks and shakes, and what is left of a creature it killed.
//!
//! [`resolve_attack`] is the one place damage is decided; [`melee_attack`] is
//! what a player's swing actually calls, since it also has to ask a wielded
//! weapon whether it strikes twice or cleaves. Everything past the dice is
//! folded into [`Landed`] once and threaded through the three aftermath
//! stages — [`punctuate`], [`report_blow`], [`settle_the_dead`] — so each
//! reads the blow rather than re-deriving it.
//!
//! * `melee` — the opposed roll. * `reach` — lunge, whirl and reach weapons.
//! * `report` — the log lines and sparks. * `death` — what a death sets off.
//! * `effects/` — the effect vocabulary and the ledger of who holds what.
//! * `helpers` — plumbing every module leans on; only `apply_hit`, `Hit` and a
//!   few small functions are public (see the root re-export).

mod death;
pub mod effects;
pub(crate) mod helpers;
mod melee;
mod reach;
mod report;

pub use death::*;
pub use melee::*;
pub use reach::*;
use report::*;

use crate::Color;
use bevy_ecs::prelude::*;
use rand::Rng;
use rand_chacha::ChaCha12Rng;

use crate::abilities::{Blow, cleave_attack, fire_on_hit, fire_on_struck, fire_on_targeted};
use crate::ai::hostile;
use crate::components::*;
use crate::conditions::afflicted;
use crate::constants::score::BOUNTY_SCORE_MULTIPLIER;
use crate::effects::{
    Asleep, Bided, Binds, Clamped, ClampedBy, Fencer, Grant, Lunges, Lurk, Pinned, Rooted,
    ScoreBounty, ShattersStone, VorpalOnCondition, VorpalTarget, WhirlOnMove, count_of, loadout,
    revoke,
};
use crate::equipment::{Slot, destroy_worn, equipped_in, equipped_items, force_unequip};
use crate::helpers::{
    Hit, apply_hit, chebyshev, death_burst, get_line, mob_at, monster_at, player_sees, roll_dice,
    spill_blood, took_damage,
};
use crate::identify::display_name;
use crate::map::{GameRng, Map};
use crate::particles::Particles;
use crate::score::award_kill;
use crate::shake::{Shake, ShakeKind, kick_shake};
use crate::state::Ending;

// --- Tuning constants ------------------------------------------------------
// Defined and documented in `constants.rs`.
//
//   EXCELLENT_HIT_CHANCE / EXCELLENT_HIT_DICE  the player's Nd[power] crit
//   CHIP_DAMAGE                                the player's guaranteed-1 floor
//   GEAR_SURVIVES_DEATH                        per-item odds a corpse keeps its gear
use crate::constants::combat::{
    BELL_CURVE_DICE, BIDE_ATTACK_BONUS, CHIP_DAMAGE, CURSED_SPLINTER_DIE, CURSED_SPLINTER_RADIUS,
    EXCELLENT_HIT_CHANCE, EXCELLENT_HIT_DICE, GEAR_SURVIVES_DEATH, VULN_DAMAGE_PERCENT,
};
use crate::constants::decks::{BALA_POWER, BOLE_ARMOR};

/// Rolls `1dN`. A non-positive number of sides means "no die", which rolls 0 so
/// an unarmoured/unarmed entity simply contributes nothing to the opposed roll.
fn roll_die(rng: &mut ChaCha12Rng, sides: i32) -> i32 {
    if sides <= 0 {
        return 0;
    }
    rng.gen_range(1..=sides)
}

/// Rolls `1dN` [`BELL_CURVE_DICE`] times and averages, rounding down. Same
/// range as a plain [`roll_die`], just a narrower distribution — the bell
/// curve that takes the swing out of a normal exchange without touching any
/// weapon or armour's tuned die size. Rounding down shades the mean about a
/// quarter point under the plain die's.
pub(crate) fn roll_die_bell(rng: &mut ChaCha12Rng, sides: i32) -> i32 {
    (0..BELL_CURVE_DICE)
        .map(|_| roll_die(rng, sides))
        .sum::<i32>()
        / BELL_CURVE_DICE
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
    if let Some(form) = crate::effects::chimeric_form(world, entity) {
        return form.name.to_string();
    }
    world
        .get::<Name>(entity)
        .map(|n| n.what.clone())
        .unwrap_or_else(|| "something".to_string())
}

/// The combat schedule step: drains the [`AttackQueue`] and resolves every
/// pending attack (currently these are all monster-initiated; the player's
/// melee is resolved inline by the input handler).
///
/// A blow is dropped when its two sides no longer touch: `ai` queued it, but a
/// trap or a swap since then may have moved either one off the tile it struck
/// from, or onto the far side of a doorway corner.
///
/// Assumes `equipment_effects_system` has already folded every lent effect
/// into `Loadout`-relevant components this turn, so the opposed roll below
/// reads a wearer's current gear and never a stale one.
pub fn combat_system(world: &mut World) {
    let attacks = std::mem::take(&mut world.resource_mut::<AttackQueue>().attacks);

    for attack in attacks {
        if still_in_reach(world, attack.attacker, attack.target) {
            resolve_attack(world, attack.attacker, attack.target);
        }
    }
}

fn still_in_reach(world: &World, attacker: Entity, target: Entity) -> bool {
    let (Some(a), Some(t)) = (
        world.get::<Position>(attacker),
        world.get::<Position>(target),
    ) else {
        return true;
    };
    crate::helpers::chebyshev(*a, *t) == 1
        && (world.get::<crate::effects::Phasing>(attacker).is_some()
            || world.resource::<Map>().diagonal_step_ok(a.x, a.y, t.x, t.y))
}
