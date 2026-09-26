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
/// creature that did it to you is gone. A hand that closes on the Element of
/// Yoord instead is the last thing the leprechaun does
/// ([`element_bursts_thief`]).
pub(crate) fn leprechaun_theft(world: &mut World, attacker: Entity, target: Entity) {
    let Some(item) = steal_unequipped_item(world, target) else {
        return;
    };
    if world.get::<Amulet>(item).is_some() {
        element_bursts_thief(world, attacker, target);
        return;
    }
    let item_name = item_label(world, item);
    let attacker_name = item_label(world, attacker);
    let target_label = victim_label(world, target);
    world
        .resource_mut::<GameLog>()
        .add(strings::leprechaun_theft(
            &attacker_name,
            &item_name,
            &target_label,
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
    world.resource_mut::<GameLog>().add(strings::nymph_theft(
        &attacker_name,
        &item_name,
        &target_label,
    ));
    if let Some(pos) = world.get::<Position>(attacker).copied() {
        leave_smoke(world, pos);
    }
    world.entity_mut(item).despawn();
    world.entity_mut(attacker).despawn();
}

/// Damage enough for the biggest splatter [`crate::helpers::spill_blood`]
/// draws: every droplet it has, flung as far as it flings them.
const GORE: i32 = 64;

/// A thief whose hand closes on the Element of Yoord does not keep the hand,
/// or anything else. It bursts apart in gore where it stands and dies the way
/// anything killed without a blow does, its corpse flung away from the
/// Element's owner. The Element never left the pack.
fn element_bursts_thief(world: &mut World, thief: Entity, owner: Entity) {
    let name = item_label(world, thief);
    world
        .resource_mut::<GameLog>()
        .add(strings::element_bursts_thief(&name));
    crate::helpers::spill_blood(world, thief, GORE, false);
    if let Some(at) = world.get::<Position>(thief).copied()
        && crate::helpers::player_sees(world, at.x, at.y)
    {
        crate::shake::kick_shake(world, crate::shake::ShakeKind::Heavy);
    }
    let from = world.get::<Position>(owner).copied();
    crate::combat::finish_indirect_kill(world, thief, from);
}

/// `"you"` for the player, `"the orc"` for anything else — the object half of
/// a theft's sentence.
fn victim_label(world: &World, target: Entity) -> String {
    match world.get::<Player>(target).is_some() {
        true => "you".to_string(),
        false => strings::the(&item_label(world, target)),
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
