//! Intents and their queues. An input handler pushes one; the matching system drains it.
//! See `docs/explanation/adr-0008-player-actions-are-queued-intents.md`.

use super::*;

// ===========================================================================
// Events and their queues
//
// An input handler pushes an intent onto a queue resource; the matching system
// drains the queue once per turn and resolves each one. The `Event` derive is
// kept for the types even though they travel by queue.
// ===========================================================================

/// Intent: `attacker` swings at `target`. Drained by `crate::combat`.
#[derive(Event, Clone, Copy)]
pub struct WantsToAttack {
    /// Whoever swings.
    pub attacker: Entity,
    /// Whoever is swung at.
    pub target: Entity,
}

/// Intent: `user` uses `item` — quaff, read, zap or (un)equip, depending on what
/// it is. `target` is the aimed tile for a wand; `slot_idx` is the pack row it
/// sits in, so it can go back exactly there. Drained by
/// [`item_system`](crate::items::item_system).
#[derive(Event, Clone, Copy)]
pub struct WantsToUse {
    /// Whoever quaffs, reads, zaps or equips.
    pub user: Entity,
    /// The item used.
    pub item: Entity,
    /// The aimed tile. `None` for anything that is not aimed.
    pub target: Option<Position>,
    /// The pack row the item sits in; [`item_system`](crate::items::item_system)
    /// lifts it from there. `None` when it is not in a pack.
    pub slot_idx: Option<usize>,
}

/// A hurled item, in flight from `thrower` towards `target`. Resolved by
/// [`crate::items::throw_system`], which is where it finds out what it hits.
#[derive(Event, Clone, Copy)]
pub struct WantsToThrow {
    /// Whoever throws.
    pub thrower: Entity,
    /// What is thrown.
    pub item: Entity,
    /// The tile it is thrown at.
    pub target: Position,
    /// The pack row the item sits in; [`crate::items::throw_system`] lifts it
    /// from there. A [`Returns`] item lands in that row again. `None` when it is
    /// not in a pack.
    pub slot_idx: Option<usize>,
}

/// The turn's pending attacks. Filled by input / AI, drained by
/// `crate::combat`.
#[derive(Resource, Default)]
pub struct AttackQueue {
    /// Taken whole when combat runs. An attack pushed while those resolve waits
    /// for the next run.
    pub attacks: Vec<WantsToAttack>,
}

/// The turn's pending item uses. Drained by
/// [`item_system`](crate::items::item_system).
#[derive(Resource, Default)]
pub struct UseQueue {
    /// Taken whole when the item system runs; a use pushed meanwhile waits for
    /// the next run.
    pub uses: Vec<WantsToUse>,
}

/// The turn's pending throws. Drained by [`crate::items::throw_system`].
#[derive(Resource, Default)]
pub struct ThrowQueue {
    /// Taken whole when the throw system runs; a throw pushed meanwhile waits
    /// for the next run.
    pub throws: Vec<WantsToThrow>,
}

/// One thing the player chose to do with the turn, decided when the key was
/// pressed and applied by [`player_action_system`](crate::player::player_action_system),
/// the schedule's first step.
pub enum PlayerAction {
    /// A step, a blow, a lunge or a struggle: see [`StepPlan`](crate::player::StepPlan).
    Step(crate::player::StepPlan),
    /// Take the stairs under the player.
    Stairs {
        /// Down when true, up when false.
        going_down: bool,
    },
    /// Put a carried item on the floor.
    Drop {
        /// The item let go.
        item: Entity,
    },
    /// Wield another weapon in place of the current one.
    SwapWeapon {
        /// The weapon taken up.
        item: Entity,
    },
    /// The ring of teleportation's jump, on command.
    WilledTeleport,
    /// Run at `target` and strike it.
    Charge {
        /// The creature charged.
        target: Entity,
    },
    /// A reach weapon's strike at a tile.
    Reach {
        /// The weapon swung.
        weapon: Entity,
        /// The tile aimed at.
        at: Position,
    },
}

/// The turn's pending player action. Drained by
/// [`player_action_system`](crate::player::player_action_system).
#[derive(Resource, Default)]
pub struct PlayerActionQueue {
    /// Taken whole when the system runs; an action pushed meanwhile waits for
    /// the next run.
    pub actions: Vec<PlayerAction>,
}

/// Intent: `user` triggers active spell `spell` at `target` — a spell's twin of
/// [`WantsToUse`], minus everything about an item because a spell isn't one.
/// Drained by [`spell_system`](crate::items::spell_system).
#[derive(Event, Clone, Copy)]
pub struct WantsToCast {
    /// Whoever casts.
    pub user: Entity,
    /// The spell cast.
    pub spell: &'static crate::catalog::SpellDef,
    /// The aimed tile.
    pub target: Position,
}

/// The turn's pending spells. Drained by
/// [`spell_system`](crate::items::spell_system).
#[derive(Resource, Default)]
pub struct SpellQueue {
    /// Taken whole when the spell system runs; a cast pushed meanwhile waits
    /// for the next run.
    pub spells: Vec<WantsToCast>,
}
