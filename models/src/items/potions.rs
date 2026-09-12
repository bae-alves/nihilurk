//! Quaffing a potion.
//!
//! The catalog ([`crate::catalog::POTIONS`]) says what each potion is called and
//! how it draws; this file is the one place a dose actually *does* something,
//! keyed by [`PotionEffect`].

use bevy_ecs::{entity::Entity, world::World};

use crate::components::*;
use crate::helpers::item_label;

/// Works a potion on `user`. Returns whether the dose visibly took hold — the
/// player learns a potion by drinking it either way, but a potion *thrown* at a
/// monster only gives itself away when something plainly happens (see
/// [`super::throwing`]).
///
/// Exhaustive over `PotionEffect`, deliberately with no catch-all: a potion
/// effect added to the enum and not given an arm here fails the build instead
/// of silently doing nothing — the same guarantee `crate::traps::apply_trap_effect`
/// gives a new `TrapEffect`. See `docs/explanation/data-driven-content.md`.
///
/// Every variant below `Healing` is a row in the catalog with no mechanic
/// behind it yet — that gap is unchanged by this match being exhaustive; it
/// is just no longer possible to add a *fifteenth* such gap by accident.
pub(super) fn apply_potion_effect(world: &mut World, user: Entity, effect: PotionEffect) -> bool {
    match effect {
        PotionEffect::Healing => {
            let Some(mut fighter) = world.get_mut::<Fighter>(user) else {
                return false;
            };
            let before = fighter.hp;
            fighter.hp = fighter.max_hp;
            let healed = fighter.hp > before;
            let msg = if world.get::<Player>(user).is_some() {
                "You feel refreshed as your wounds mend!".to_string()
            } else {
                format!(
                    "The {} glows eerily, wounds closing.",
                    item_label(world, user)
                )
            };
            world.resource_mut::<GameLog>().add(msg);
            healed
        }
        PotionEffect::Blindness
        | PotionEffect::Confusion
        | PotionEffect::ExtraHealing
        | PotionEffect::FruitJuice
        | PotionEffect::GainStrength
        | PotionEffect::Haste
        | PotionEffect::MagicDetection
        | PotionEffect::MonsterDetection
        | PotionEffect::Paralysis
        | PotionEffect::Poison
        | PotionEffect::RaiseLevel
        | PotionEffect::RestoreStrength
        | PotionEffect::SeeInvisible
        | PotionEffect::Water => false,
    }
}
