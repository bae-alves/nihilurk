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
        _ => false, /* handle other potion effects */
    }
}
