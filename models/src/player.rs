//! The player's own turn: what a step costs, finds and announces, decided
//! here; `engine` only reads the keys.
//!
//! A step is **planned** when the key is pressed ([`plan_step`], which reads
//! the world and changes nothing but the confusion roll) and **applied** by
//! [`player_action_system`], the schedule's first step. The plan says whether
//! the turn is spent, so a bump into a wall never wakes the monsters. The
//! system does the stepping, so the player acts through the same queue-and-drain
//! path as every other intent.

use bevy_ecs::prelude::*;
use rand::Rng;

use crate::components::{
    GameLog, Helper, Item, LogCategory, Pickup, Player, PlayerAction, PlayerActionQueue, Position,
};
use crate::constants::conditions::CONFUSION_STUMBLE_CHANCE;
use crate::effects::{Clamped, Confused, Pinned, Rooted, Swims};
use crate::equipment::reset_momentum;
use crate::helpers::{mark_moved, mob_at};
use crate::ice::{IceCube, kick_ice_cube};
use crate::map::{Map, special_room_entry_message};
use crate::traps::{bear_trap_thrash, clamped_thrash, player_held_by};
use crate::{
    Backpack, Grant, can_charge, can_teleport_at_will, change_level, charge, display_name,
    drop_refusal, force_unequip, level_change_refusal, melee_attack, resolve_reach_attack,
    sync_equipment_effects, try_lunge, try_whirl_attack, willed_teleport,
};

const STUMBLE_DIRS: [(i16, i16); 8] = [
    (1, 0),
    (-1, 0),
    (0, 1),
    (0, -1),
    (1, 1),
    (-1, -1),
    (1, -1),
    (-1, 1),
];

/// Confusion tax: some share ([`CONFUSION_STUMBLE_CHANCE`]) of intended steps go
/// off in a random direction instead. Returns the step to actually attempt and
/// whether it was hijacked (a hijacked lurch into a wall still burns the turn).
pub fn maybe_stumble(world: &mut World, dx: i16, dy: i16) -> (i16, i16, bool) {
    let confused = world
        .query_filtered::<(), (With<Player>, With<Confused>)>()
        .iter(world)
        .next()
        .is_some();
    if !confused {
        return (dx, dy, false);
    }
    let mut rng = world.resource_mut::<crate::GameRng>();
    if !rng.0.gen_bool(CONFUSION_STUMBLE_CHANCE) {
        return (dx, dy, false);
    }
    let (sx, sy) = STUMBLE_DIRS[rng.0.gen_range(0..STUMBLE_DIRS.len())];
    world
        .resource_mut::<GameLog>()
        .add(strings::stumble_foolishly());
    (sx, sy, true)
}

/// Picks up whatever [`Item`] sits at `(x, y)` for `player_entity`, the way
/// arriving on a tile always does: walking onto it or, just the same, lunging
/// onto it with an estoc. There is no pick-up key. nihilurk has nine pack
/// slots and a floor full of coins that are spent where they lie, so landing
/// on a thing is decision enough.
pub fn pick_up_here(world: &mut World, player_entity: Entity, x: u16, y: u16) {
    let item_entity = world
        .query_filtered::<(Entity, &Position), With<Item>>()
        .iter(world)
        .find(|(_, pos)| pos.x == x && pos.y == y)
        .map(|(entity, _)| entity);
    let Some(item_entity) = item_entity else {
        return;
    };
    let stowable = world.get::<Pickup>(item_entity).is_none();
    match crate::pick_up(world, player_entity, item_entity) {
        Some(msg) => world.resource_mut::<GameLog>().add(msg),
        None if stowable => world.resource_mut::<GameLog>().add(strings::pack_full()),
        None => {}
    }
}

/// Logs a special room's one-line flavor the instant the player's step
/// crosses into it from anywhere else. A no-op off a special room or dark room, past its
/// threshold. The line is painted in the room's wall colour.
pub fn announce_special_room_entry(world: &mut World, old: (u16, u16), new: (u16, u16)) {
    let message = special_room_entry_message(world.resource::<Map>(), old, new);
    if let Some((msg, color)) = message {
        world
            .resource_mut::<GameLog>()
            .add_colored(msg, LogCategory::Room(color));
    }
}

/// What holds the player to their tile.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Hold {
    /// A bear trap: the step is spent thrashing.
    Pinned,
    /// A clamp: the step is spent thrashing.
    Clamped,
    /// Roots: the step is spent straining.
    Rooted,
}

/// What a step will do, decided before anything moves.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum StepPlan {
    /// Nothing happens.
    Refused {
        /// Whether confusion hijacked the step, which burns the turn even
        /// though the lurch went nowhere.
        stumbled: bool,
    },
    /// An estoc lunge across the empty tile ahead.
    Lunge {
        /// The step's direction on x.
        dx: i16,
        /// The step's direction on y.
        dy: i16,
    },
    /// A kick that sends a frozen corpse flying.
    Kick {
        /// The cube kicked.
        cube: Entity,
    },
    /// A blow at the creature in the way.
    Melee {
        /// Whoever stands there.
        target: Entity,
    },
    /// A turn spent held in place.
    Held(Hold),
    /// A step to `to`, trading places with `swap` when a helper stands there.
    Move {
        /// The tile left.
        from: Position,
        /// The tile entered.
        to: Position,
        /// The helper on `to`, if any, who takes `from`.
        swap: Option<Entity>,
    },
}

impl StepPlan {
    /// Whether the plan costs the turn. Only a refusal that was not a
    /// confused lurch is free.
    pub fn spends_turn(&self) -> bool {
        !matches!(self, StepPlan::Refused { stumbled: false })
    }
}

/// Decides what a step toward `(dx, dy)` will do. Rolls the confusion tax, which
/// may turn the step; reads everything else and changes nothing.
pub fn plan_step(world: &mut World, dx: i16, dy: i16) -> StepPlan {
    let (dx, dy, stumbled) = maybe_stumble(world, dx, dy);
    let refused = StepPlan::Refused { stumbled };

    let Some((player, from)) = world
        .query_filtered::<(Entity, &Position), With<Player>>()
        .iter(world)
        .next()
        .map(|(e, p)| (e, *p))
    else {
        return StepPlan::Refused { stumbled: false };
    };
    let to = Position {
        x: from.x.saturating_add_signed(dx),
        y: from.y.saturating_add_signed(dy),
    };

    if crate::lunge_target(world, player, dx, dy).is_some() {
        return StepPlan::Lunge { dx, dy };
    }

    let swims = world.get::<Swims>(player).is_some();
    {
        let map = world.resource::<Map>();
        if !map.walkable(to.x, to.y, swims) || !map.diagonal_step_ok(from.x, from.y, to.x, to.y) {
            return refused;
        }
    }

    let mut swap = None;
    if let Some(other) = mob_at(world, to) {
        if world.get::<IceCube>(other).is_some() {
            return StepPlan::Kick { cube: other };
        }
        if world.get::<Helper>(other).is_none() {
            return StepPlan::Melee { target: other };
        }
        swap = Some(other);
    }

    for (grant, hold) in [
        (Grant::of::<Pinned>(), Hold::Pinned),
        (Grant::of::<Clamped>(), Hold::Clamped),
        (Grant::of::<Rooted>(), Hold::Rooted),
    ] {
        if player_held_by(world, grant) {
            return StepPlan::Held(hold);
        }
    }
    StepPlan::Move { from, to, swap }
}

/// Plans a step and, unless it was refused, queues it for
/// [`player_action_system`]. Returns whether the turn is spent, which is the
/// caller's cue to run the schedule.
pub fn queue_step(world: &mut World, dx: i16, dy: i16) -> bool {
    let plan = plan_step(world, dx, dy);
    if !matches!(plan, StepPlan::Refused { .. }) {
        world
            .resource_mut::<PlayerActionQueue>()
            .actions
            .push(PlayerAction::Step(plan));
    }
    plan.spends_turn()
}

fn apply_step(world: &mut World, plan: StepPlan) {
    let Some((player, here)) = world
        .query_filtered::<(Entity, &Position), With<Player>>()
        .iter(world)
        .next()
        .map(|(e, p)| (e, *p))
    else {
        return;
    };
    match plan {
        StepPlan::Refused { .. } => {}
        StepPlan::Lunge { dx, dy } => {
            if try_lunge(world, player, dx, dy)
                && let Some(now) = world.get::<Position>(player).copied()
            {
                announce_special_room_entry(world, (here.x, here.y), (now.x, now.y));
                pick_up_here(world, player, now.x, now.y);
            }
        }
        StepPlan::Kick { cube } => {
            kick_ice_cube(world, player, cube);
        }
        StepPlan::Melee { target } => melee_attack(world, player, target),
        StepPlan::Held(hold) => {
            reset_momentum(world, player);
            match hold {
                Hold::Pinned => bear_trap_thrash(world, player),
                Hold::Clamped => clamped_thrash(world, player),
                Hold::Rooted => world
                    .resource_mut::<GameLog>()
                    .add(strings::strain_against_rooted()),
            }
        }
        StepPlan::Move { from, to, swap } => {
            reset_momentum(world, player);
            if let Some(mut pos) = world.get_mut::<Position>(player) {
                *pos = to;
            }
            if let Some(helper) = swap {
                world.entity_mut(helper).insert(from);
                mark_moved(world, helper);
            }
            mark_moved(world, player);
            announce_special_room_entry(world, (from.x, from.y), (to.x, to.y));
            try_whirl_attack(world, player, from, to);
            pick_up_here(world, player, to.x, to.y);
        }
    }
}

/// The schedule's first step: applies what the player chose to do this turn.
///
/// Assumes the plan was made against the world as it stands now. Nothing runs
/// between a key press and the schedule, so it was. It runs before every other
/// step because everything the player does resolves before the monsters move.
pub fn player_action_system(world: &mut World) {
    let actions = std::mem::take(&mut world.resource_mut::<PlayerActionQueue>().actions);
    for action in actions {
        match action {
            PlayerAction::Step(plan) => apply_step(world, plan),
            PlayerAction::Stairs { going_down } => {
                change_level(world, going_down);
            }
            PlayerAction::Drop { item } => apply_drop(world, item),
            PlayerAction::WilledTeleport => {
                willed_teleport(world);
            }
            PlayerAction::Charge { target } => {
                if charge(world, target)
                    && let Some(now) = player_position(world)
                {
                    pick_up_here(world, now.0, now.1.x, now.1.y);
                }
            }
            PlayerAction::Reach { weapon, at } => {
                if let Some((player, _)) = player_position(world) {
                    resolve_reach_attack(world, player, weapon, at);
                }
            }
        }
    }
}

fn player_position(world: &mut World) -> Option<(Entity, Position)> {
    world
        .query_filtered::<(Entity, &Position), With<Player>>()
        .iter(world)
        .next()
        .map(|(e, p)| (e, *p))
}

fn queue(world: &mut World, action: PlayerAction) {
    world
        .resource_mut::<PlayerActionQueue>()
        .actions
        .push(action);
}

/// Queues taking the stairs under the player. Returns whether the turn is
/// spent; a refusal says why in the log and spends nothing.
pub fn queue_stairs(world: &mut World, going_down: bool) -> bool {
    if let Some(refusal) = level_change_refusal(world, going_down) {
        world.resource_mut::<GameLog>().add(refusal);
        return false;
    }
    queue(world, PlayerAction::Stairs { going_down });
    true
}

/// Queues putting `item` on the floor. Cursed gear on the body and the Element
/// of Yoord will not leave the hand: a refusal says so and spends nothing.
pub fn queue_drop(world: &mut World, player: Entity, item: Entity) -> bool {
    if let Some(refusal) = drop_refusal(world, player, item) {
        world.resource_mut::<GameLog>().add(refusal);
        return false;
    }
    queue(world, PlayerAction::Drop { item });
    true
}

/// Queues the jump a ring of teleportation grants on command. Silent when it
/// cannot fire, on purpose: a refusal that talks gives the secret away.
pub fn queue_willed_teleport(world: &mut World) -> bool {
    if !can_teleport_at_will(world) {
        return false;
    }
    queue(world, PlayerAction::WilledTeleport);
    true
}

/// Queues a charge at `target`. Refused when the straight line to the tile
/// beside it is not clear.
pub fn queue_charge(world: &mut World, target: Entity) -> bool {
    if !can_charge(world, target) {
        return false;
    }
    queue(world, PlayerAction::Charge { target });
    true
}

/// Queues a reach weapon's strike at `at`. The reticle has already checked the
/// aim, so this always spends the turn.
pub fn queue_reach_attack(world: &mut World, weapon: Entity, at: Position) {
    queue(world, PlayerAction::Reach { weapon, at });
}

fn apply_drop(world: &mut World, item: Entity) {
    let Some((player, here)) = player_position(world) else {
        return;
    };
    if let Some(mut pack) = world.get_mut::<Backpack>(player) {
        pack.items.retain(|&held| held != item);
    }
    force_unequip(world, item);
    sync_equipment_effects(world, player);
    world.entity_mut(item).insert(here);
    let name = display_name(world, item);
    world
        .resource_mut::<GameLog>()
        .add(strings::you_drop(&name));
}
