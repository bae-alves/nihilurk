//! What a couple of thieving monsters do with what they take.
//!
//! The leprechaun and the nymph both live behind [`crate::abilities`]'s
//! on-hit table, which owns *finding* what to steal
//! ([`crate::abilities::steal_unequipped_item`] /
//! [`crate::abilities::steal_equipped_item`]). What happens once the theft has
//! actually happened — the leprechaun's half puts its haul to immediate use —
//! lives here instead, because that reaches into `potions` and `scrolls`,
//! siblings this crate keeps private outside `crate::items`.

use bevy_ecs::{entity::Entity, world::World};

use crate::abilities::{steal_equipped_item, steal_unequipped_item};
use crate::components::*;
use crate::helpers::{item_label, leave_smoke};

use super::potions::apply_potion_effect;
use super::scrolls::apply_scroll_effect;
use super::wands::teleport_entity_away;

/// The leprechaun's on-hit: lift something loose from the victim's pack,
/// press it into immediate use, and vanish. Genuinely dangerous — whatever it
/// stole is answered on the spot, against you or for the thief, and then the
/// creature that did it to you is gone.
pub(crate) fn leprechaun_theft(world: &mut World, attacker: Entity, target: Entity) {
    let Some(item) = steal_unequipped_item(world, target) else {
        return;
    };
    let item_name = item_label(world, item);
    let attacker_name = item_label(world, attacker);
    let target_label = victim_label(world, target);
    world.resource_mut::<GameLog>().add(format!(
        "The {attacker_name} snatches the {item_name} from {target_label} and cackles!"
    ));
    use_stolen_item(world, attacker, item);
    teleport_entity_away(world, attacker);
}

/// The nymph's on-hit: strip one piece of equipped gear and disappear the
/// instant it does — no immediate use, no theatre, just gone with it.
pub(crate) fn nymph_theft(world: &mut World, attacker: Entity, target: Entity) {
    let Some(item) = steal_equipped_item(world, target) else {
        return;
    };
    let item_name = item_label(world, item);
    let attacker_name = item_label(world, attacker);
    let target_label = victim_label(world, target);
    world.resource_mut::<GameLog>().add(format!(
        "The {attacker_name} rips the {item_name} from {target_label} and vanishes in a puff of smoke!"
    ));
    if let Some(pos) = world.get::<Position>(attacker).copied() {
        leave_smoke(world, pos);
    }
    world.entity_mut(item).despawn();
    world.entity_mut(attacker).despawn();
}

/// `"you"` for the player, `"the orc"` for anything else — the object half of
/// a theft's sentence.
fn victim_label(world: &World, target: Entity) -> String {
    match world.get::<Player>(target).is_some() {
        true => "you".to_string(),
        false => format!("the {}", item_label(world, target)),
    }
}

/// Whatever the leprechaun grabbed, put to use on the spot: a potion is drunk,
/// a scroll read aloud. A piece of gear is worn or wielded — the leprechaun
/// keeps it on, and it stays on it until *it* dies. A wand has no sane target
/// in the half-second the leprechaun holds it, so it's simply kept, unused.
fn use_stolen_item(world: &mut World, thief: Entity, item: Entity) {
    if let Some(effect) = world.get::<Potion>(item).map(|p| p.effect) {
        apply_potion_effect(world, thief, effect);
        world.entity_mut(item).despawn();
        return;
    }
    if let Some(effect) = world.get::<Scroll>(item).map(|s| s.effect) {
        apply_scroll_effect(world, thief, effect);
        world.entity_mut(item).despawn();
        return;
    }
    if world.get::<crate::equipment::Equipped>(item).is_some() {
        crate::equipment::equip_silently(world, thief, item);
    }
}
