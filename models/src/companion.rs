//! The boon companion: the one creature on your side.
//!
//! A snack thrown at a creature without hands, or a fancy of peace thrown at
//! one with them, is eaten either way, and half the time the creature becomes
//! your [`Helper`]. A Helper is an [`Faction::Ally`] that chases whatever the
//! player can see ([`crate::ai`]), follows the player to every new floor
//! ([`crate::map`]'s level change), and trades places with them when they walk
//! into it. There is only ever one: taking a second makes the first explode.

use bevy_ecs::prelude::*;
use crossterm::style::Color;
use rand::Rng;

use crate::components::*;
use crate::constants::helpers::ACCEPT_CHANCE;
use crate::effects::{Asleep, Grant, ItemUser, Phasing, grant_all, revoke, revoke_matching};
use crate::helpers::{death_burst, free_adjacent_tile, item_label};
use crate::map::{BloodStains, GameRng};
use crate::particles::{BlastPalette, Particles, on_map};
use crate::shake::{ShakeKind, kick_shake};

/// What a Helper is born into when recruited: it walks through terrain.
const GHOSTLY: &[Grant] = &[Grant::of::<Phasing>()];

/// The current Helper, if there is one.
pub fn the_helper(world: &mut World) -> Option<Entity> {
    world
        .query_filtered::<Entity, With<Helper>>()
        .iter(world)
        .next()
}

/// Whether `e` is a creature the player should fight: a [`Faction::Monster`].
/// The one question behind every "is that a foe" check a Helper must not fail.
pub fn is_foe(world: &World, e: Entity) -> bool {
    world.get::<Faction>(e) == Some(&Faction::Monster)
}

/// Whether `e` is a plain ally — charmed, but not (yet) the Helper. A treat
/// thrown at one always takes: it already trusts you.
fn is_charmed(world: &World, e: Entity) -> bool {
    world.get::<Faction>(e) == Some(&Faction::Ally) && world.get::<Helper>(e).is_none()
}

/// Whether a treat thrown by `thrower` at `victim` is an offer `victim` can
/// take: the player threw it, `victim` is a monster or an already-charmed
/// ally, and the treat is the right kind for it — a fancy of peace for a
/// creature with hands, a snack for one without. Anything else bounces off.
pub(crate) fn fits(world: &World, thrower: Entity, victim: Entity, treat: Treat) -> bool {
    world.get::<Player>(thrower).is_some()
        && world.get::<Mob>(victim).is_some()
        && (is_foe(world, victim) || is_charmed(world, victim))
        && treat.for_item_users == world.get::<ItemUser>(victim).is_some()
}

/// `victim` eats `item` and becomes the player's Helper. The treat is gone
/// either way: a hostile monster only takes it on an [`ACCEPT_CHANCE`] roll,
/// but a plain ally already trusts you and always takes.
pub(crate) fn offer(world: &mut World, victim: Entity, item: Entity, treat_name: &str) {
    world.entity_mut(item).despawn();
    if is_charmed(world, victim) || world.resource_mut::<GameRng>().0.gen_bool(ACCEPT_CHANCE) {
        recruit(world, victim);
        return;
    }
    let name = item_label(world, victim);
    world
        .resource_mut::<GameLog>()
        .add(strings::refuses_treat(&name, treat_name));
}

/// Makes `mob` a plain ally: a [`Faction::Ally`] that fights the monsters and
/// chases whatever the player can see, same as a Helper — but it is not *the*
/// Helper. Unlike [`recruit`], there is no limit of one (nothing explodes to
/// make room) and it does not follow the player downstairs
/// ([`crate::map::levels`] only carries the [`Helper`] along).
pub fn charm(world: &mut World, mob: Entity) {
    world.entity_mut(mob).insert(Faction::Ally);
    if let Some(mut m) = world.get_mut::<Mob>(mob) {
        m.movement_type = MovementType::Chase;
    }
    revoke(world, mob, Grant::of::<Asleep>());
}

/// Makes `mob` the player's Helper. A Helper already at the player's side
/// explodes first: there is only ever one.
pub fn recruit(world: &mut World, mob: Entity) {
    if let Some(old) = the_helper(world).filter(|&old| old != mob) {
        explode(world, old);
    }
    world.entity_mut(mob).insert((Helper, Faction::Ally));
    grant_all(world, mob, GHOSTLY);
    if let Some(mut m) = world.get_mut::<Mob>(mob) {
        m.movement_type = MovementType::Chase;
    }
    revoke(world, mob, Grant::of::<Asleep>());
    let name = item_label(world, mob);
    world
        .resource_mut::<GameLog>()
        .add(strings::becomes_helper(&name));
}

/// The old Helper's send-off when a new one is taken: all gore, no harm. It
/// hurts nobody, pays no score, and drops what it wore the way any corpse does.
fn explode(world: &mut World, old: Entity) {
    let name = item_label(world, old);
    if let Some(pos) = world.get::<Position>(old).copied() {
        let mut cells = Vec::new();
        for dy in -2i32..=2 {
            for dx in -2i32..=2 {
                let Some((x, y)) = on_map(pos.x as i32 + dx, pos.y as i32 + dy) else {
                    continue;
                };
                cells.push((x, y, ((dx * dx + dy * dy) as f32).sqrt()));
                if world.get::<Blood>(old).is_some()
                    && world.resource::<crate::map::Map>().walkable(x, y, true)
                {
                    world.resource_mut::<BloodStains>().stain(x, y);
                }
            }
        }
        if let Some(mut fx) = world.get_resource_mut::<Particles>() {
            fx.explosion(&cells, BlastPalette::Death);
        }
        death_burst(world, old, None);
        kick_shake(world, ShakeKind::Heavy);
    }
    let mut log = world.resource_mut::<GameLog>();
    log.add(strings::old_helper_explodes(&name));
    log.add(strings::so_much_for_loyalty());
    crate::combat::leave_gear_behind(world, old);
    world.despawn(old);
}

/// What a Helper's death adds to an ordinary one: the line, a heavy shake, and
/// a heart over the body. The death burst itself already runs in slow motion
/// for a Helper ([`death_burst`]). Called by both death funnels in
/// `crate::combat`, which also skip paying for the corpse.
pub(crate) fn mourn(world: &mut World, helper: Entity) {
    let name = item_label(world, helper);
    world
        .resource_mut::<GameLog>()
        .add(strings::helper_dies(&name));
    kick_shake(world, ShakeKind::Heavy);
    let pos = world.get::<Position>(helper).copied();
    let color = world
        .get::<Renderable>(helper)
        .map_or(Color::White, |r| r.color);
    if let (Some(pos), Some(mut fx)) = (pos, world.get_resource_mut::<Particles>()) {
        fx.spark_burst(pos.x, pos.y, Color::Red);
        fx.condition_mark(pos.x, pos.y, '♥', color, 0.0);
    }
}

/// Stands the Helper beside the player on a floor they just arrived on, rested
/// and clear of whatever the old floor did to it. With no room beside the
/// player, it is left behind, and what it wore stays with it on the old floor.
///
/// Called after the new floor is populated, so nothing spawned there can land
/// on the Helper's tile.
pub(crate) fn follow_downstairs(world: &mut World, helper: Entity, beside: Position) {
    let Some((x, y)) = free_adjacent_tile(world, beside) else {
        let name = item_label(world, helper);
        world
            .resource_mut::<GameLog>()
            .add(strings::helper_left_behind(&name));
        for item in crate::equipment::equipped_items(world, helper) {
            world.despawn(item);
        }
        world.despawn(helper);
        return;
    };
    world.entity_mut(helper).insert(Position { x, y });
    if let Some(mut f) = world.get_mut::<Fighter>(helper) {
        f.hp = f.max_hp;
    }
    revoke_matching(world, helper, |h| h.is_condition());
}
