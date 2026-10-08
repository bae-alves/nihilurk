//! The turn, in order.
//!
//! One flat list, run once per player turn. Every `.after()`/`.before()` is
//! load-bearing; the order is documented in
//! `docs/reference/input-and-turn-loop.md` and pinned by
//! `models/tests/schedule.rs`.

use bevy_ecs::schedule::{IntoSystemConfigs, Schedule};

use crate::ai::ai;
use crate::visibility::visibility_system;
use crate::{
    ability_system, combat_system, dungeon_lord_system, equipment_effects_system, item_system,
    player_action_system, reaper_system, reveal_mimics, score_turn_system, sink_system,
    smoke_system, spell_system, throw_system, tick_effects, trap_system,
};

/// The schedule every run and every test of the whole turn builds.
pub fn turn_schedule() -> Schedule {
    let mut schedule = Schedule::default();
    schedule.add_systems((
        player_action_system,
        smoke_system
            .after(player_action_system)
            .before(tick_effects),
        tick_effects,
        reveal_mimics.after(tick_effects),
        spell_system.after(reveal_mimics).before(ai),
        item_system
            .after(reveal_mimics)
            .after(spell_system)
            .before(ai),
        throw_system.after(item_system).before(ai),
        ai.after(spell_system),
        trap_system.after(ai),
        equipment_effects_system
            .after(item_system)
            .after(trap_system),
        combat_system.after(equipment_effects_system),
        reaper_system.after(combat_system),
        dungeon_lord_system.after(reaper_system),
        ability_system.after(dungeon_lord_system),
        sink_system.after(ability_system),
        visibility_system.after(sink_system),
        score_turn_system.after(visibility_system),
    ));
    schedule
}
