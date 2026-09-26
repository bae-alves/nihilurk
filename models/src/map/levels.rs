//! Moving between floors, and the two ways a run ends.
//!
//! One verb does the work — [`transition_level`] — and everything else here is
//! a reason to call it: the player took a staircase, the Dungeon Lord ran out
//! of patience, a trapdoor opened, a potion of raise level was drunk. The
//! [`LevelChange`] telling them apart changes only the log line and whether
//! the arrival heal applies.
//!
//! This is also where a run is set up ([`initialize_world`]) and where it is
//! won ([`win_with_style`]).
//!
//! # This file will not come out tidy, and that is the honest answer
//!
//! It reaches into every one of its siblings and a dozen modules besides. The
//! import block below is grouped by *what each group is reached for* rather
//! than alphabetically, because those groups are the steps of a transition and
//! that is the only order in which the list means anything.
//!
//! The block understates the coupling rather than overstating it: five more
//! modules — `score`, `items`, `conditions`, `effects`, `magicmap` — are called
//! fully qualified at their one call site each, which is the convention in this
//! crate for a name used once. If you are counting seams, count those too.
//!
//! That is not a split waiting to happen. **Changing floors is the one moment
//! in nihilurk when everything is true at once**: the old floor has to stop
//! existing, the new one has to be built from a seed and stocked from a
//! different seed, the player has to be stood on a stair, healed, paid for a
//! promise they kept and relieved of the conditions they were carrying, and
//! the clock has to be reset — and the order of all of that matters, because
//! the depth has to move before the floor is built and the floor has to exist
//! before anybody stands on it. A module that reaches into a dozen others is
//! what a moment like that looks like written down. Hiding it behind an event
//! bus or a trait would move the coupling somewhere it could not be read.
//!
//! What *has* been done about it: [`transition_level`] is six named steps
//! rather than 140 straight lines, so the ordering constraint is legible from
//! the call site alone. If you are adding to it, add a step; do not add a
//! paragraph to one.

use bevy_ecs::prelude::*;
use crossterm::style::Color;
use fixedbitset::FixedBitSet;
use std::collections::HashSet;

// Building the next floor: the tiles, the things that stand on them, and the
// two seeds that decide each — `layout_rng` for the shape a depth always has,
// `FxRng` for the stock it gets on this visit.
use super::generate::{build_floor, create_map, find_tile};
use super::population::{difficulty_tier, populate_level};
use super::streams::{FxRng, GameRng, RngSeed};
use super::{
    DUNGEON_LORD_PATIENCE, FINAL_DEPTH, MAP_TILE_COUNT, Map, Rooms, TileType, special_level_arrival,
};

// Unbuilding the last one. Blood, corpses and smoke are floor-local: none of
// the three follows anybody down a staircase.
use super::overlays::{BloodStains, Corpses, Smoke};

// Setting a run up — what the hero starts with, worn without a log line about it.
use crate::catalog::{spawn_ammo, spawn_armor, spawn_launcher, spawn_potion, spawn_weapon};
use crate::constants::player::{SIGHT_RANGE, START_ARMOR, START_HP, START_MAGIC, START_POWER};
use crate::equipment::equip_silently;

// A bones ghost, on the way back out: whoever died on this depth before,
// come to make the player pay for it.
use crate::effects::{ArmorBonus, PowerBonus, ThrowBonus};
use crate::monsters::{GHOST, spawn_monster};
use crate::spawn::spawn_named;
use rand::Rng;

// The arrival: how much of the descent is paid back as health.
use crate::constants::progression::DESCENT_HEAL_DIVISOR;

// The vocabulary the whole crate is written in.
use crate::components::*;
use crate::state::*;

/// Whether the player is currently carrying the Element of Yoord.
pub fn holding_element_of_yoord(world: &mut World) -> bool {
    let items: Vec<Entity> = match world
        .query_filtered::<&Backpack, With<Player>>()
        .iter(world)
        .next()
    {
        Some(bp) => bp.items.clone(),
        None => return false,
    };
    items.iter().any(|&e| world.get::<Amulet>(e).is_some())
}

/// Handles the player using a staircase.
///
/// Without the Element of Yoord the descent rules apply: `>` on a
/// [`TileType::Downstairs`] works, `<` is blocked by the Dungeon Lord's power.
/// Once the Element is in the pack the rules invert — `<` on a
/// [`TileType::Upstairs`] carries the player back up and `>` is dead. On success
/// a fresh floor is built, the player repositioned, [`Depth`] adjusted,
/// [`DESCENT_HEAL_DIVISOR`]'s share of max HP restored and `true` returned (a
/// turn passes); otherwise a log line is added and `false` returned so no turn
/// is consumed.
pub fn change_level(world: &mut World, going_down: bool) -> bool {
    let player_entity = world
        .query_filtered::<Entity, With<Player>>()
        .iter(world)
        .next()
        .unwrap();
    let player_pos = *world.get::<Position>(player_entity).unwrap();
    let tile = world.resource::<Map>().tile(player_pos.x, player_pos.y);
    let has_element = holding_element_of_yoord(world);

    if going_down {
        if has_element {
            world
                .resource_mut::<GameLog>()
                .add(if tile == TileType::Downstairs {
                    strings::element_seeks_the_sun()
                } else {
                    strings::cannot_go_down()
                });
            return false;
        }
        if tile != TileType::Downstairs {
            world
                .resource_mut::<GameLog>()
                .add(strings::cannot_go_down());
            return false;
        }
        award_stair_score(world);
        transition_level(world, true, LevelChange::Stairs);
        return true;
    }

    // Going up.
    if !has_element {
        world
            .resource_mut::<GameLog>()
            .add(if tile == TileType::Upstairs {
                strings::dungeon_lord_prevents_up()
            } else {
                strings::cannot_go_up()
            });
        return false;
    }
    if tile != TileType::Upstairs {
        world.resource_mut::<GameLog>().add(strings::cannot_go_up());
        return false;
    }
    if world.resource::<Depth>().what <= 1 {
        // The surface at last — and only ever by the player's own hand on the
        // stair. The run is won.
        award_stair_score(world);
        world
            .resource_mut::<GameLog>()
            .add(strings::climb_last_stair());
        // Nobody walks out of that dungeon quietly: the last stair is always
        // taken with style, fireworks and doubled score and all, and the engine
        // plays it out before the WIN panel.
        win_with_style(world);
        return true;
    }
    award_stair_score(world);
    transition_level(world, false, LevelChange::Stairs);
    true
}

/// Pays for a flight of stairs: [`crate::constants::score::STAIR_PER_TIER`] per
/// difficulty tier of the floor being left. Only a staircase pays — a trapdoor,
/// a portal and a potion of raise level all move you between floors without
/// anybody earning anything.
fn award_stair_score(world: &mut World) {
    let depth = world.resource::<Depth>().what;
    crate::score::award_stairs(world, difficulty_tier(depth));
}

/// Ends the run in triumph. The flourish first (it has a score to double while
/// there is still a run to score), then the flag the main loop is watching for.
///
/// Shared by the two ways out of the dungeon — the last stair and a potion of
/// raise level drunk on Depth 1 — because they are the same achievement, and
/// the game has no business rewarding one of them less.
pub(crate) fn win_with_style(world: &mut World) {
    crate::items::rings::do_it_with_style(world);
    if let Some(mut ending) = world.get_resource_mut::<Ending>() {
        ending.player_won = true;
    }
}

/// Why the player is being moved between floors — only affects the log line and
/// whether the arrival heal applies (a trapdoor plunge does not heal).
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum LevelChange {
    Stairs,
    Portal,
    Trapdoor,
    /// A potion of raise level, which only ever goes up. Unlike the portal it
    /// does not care whether the Element of Yoord is in the pack (see
    /// `crate::items`'s `potions` submodule).
    Potion,
}

/// Moves the player one floor in the given direction: clears the current floor,
/// builds the adjacent one, repositions the player (on the up-stair when
/// descending, on the down-stair when ascending), re-populates, adjusts
/// [`Depth`], heals [`DESCENT_HEAL_DIVISOR`]'s share of max HP and resets the
/// Dungeon Lord's patience. `cause` only changes the log line and — for
/// [`LevelChange::Trapdoor`] — suppresses the arrival heal.
pub(crate) fn transition_level(world: &mut World, going_down: bool, cause: LevelChange) {
    let player = world
        .query_filtered::<Entity, With<Player>>()
        .iter(world)
        .next()
        .unwrap();

    tear_down_the_floor(world);
    let depth = step_depth(world, going_down);
    let rooms = build_the_floor(world, depth);
    let start = put_the_player_down(world, player, going_down);
    populate_level(world, &rooms, start);
    settle_arrival(world, player, cause);
    world
        .resource_mut::<GameLog>()
        .add(arrival_line(cause, going_down, depth));
    if world.resource::<crate::bones::Bones>().enabled && holding_element_of_yoord(world) {
        spawn_bones_ghost(world, depth, &rooms);
    }
    if let Some(level) = world.resource::<Map>().level {
        let lines = special_level_arrival(level, &mut world.resource_mut::<FxRng>().0);
        let mut log = world.resource_mut::<GameLog>();
        for line in lines {
            log.add(line);
        }
    }
}

/// Everything on the old floor stops existing.
///
/// Gear a monster picked up is carried with no `Position` of its own, so it is
/// laid out on the monster's tile first — otherwise the sweep below walks
/// straight past it and it haunts the save forever. Backpack contents are the
/// other `Position`-less things and are deliberately left alone: that is what
/// makes them the pack.
fn tear_down_the_floor(world: &mut World) {
    let armed_mobs: Vec<(Entity, Position)> = world
        .query_filtered::<(Entity, &Position), (With<Mob>, Without<Player>)>()
        .iter(world)
        .map(|(e, p)| (e, *p))
        .collect();
    for (mob, pos) in armed_mobs {
        crate::equipment::drop_equipment(world, mob, pos);
    }

    let backpacked: HashSet<Entity> = world
        .query::<&Backpack>()
        .iter(world)
        .flat_map(|bp| bp.items.iter().copied())
        .collect();
    let to_despawn: Vec<Entity> = world
        .iter_entities()
        .filter(|e| {
            !e.contains::<Player>() && e.contains::<Position>() && !backpacked.contains(&e.id())
        })
        .map(|e| e.id())
        .collect();
    for e in to_despawn {
        world.despawn(e);
    }
}

/// Moves [`Depth`] one floor and bumps [`FloorChanges`], returning the depth
/// arrived at.
///
/// Both have to happen before anything is built. The layout is a pure function
/// of `(seed, depth)` ([`super::streams::layout_rng`]) and the contents are a
/// function of that plus the staircase count ([`content_rng`]), so building
/// first would build the floor you just left.
fn step_depth(world: &mut World, going_down: bool) -> u8 {
    if let Some(mut fc) = world.get_resource_mut::<FloorChanges>() {
        fc.count = fc.count.saturating_add(1);
    }
    let mut d = world.resource_mut::<Depth>();
    d.what = match going_down {
        true => d.what.saturating_add(1),
        false => d.what.saturating_sub(1).max(1),
    };
    d.what
}

/// Carves the new floor into the [`Map`] resource and wipes the overlays the
/// old one dirtied. Returns its rooms, which the caller needs twice over — to
/// stand the player in one and to populate the rest.
fn build_the_floor(world: &mut World, depth: u8) -> Rooms {
    let seed = world.resource::<RngSeed>().0;
    let (map, rooms) = build_floor(seed, depth);
    world.insert_resource(map);
    world.resource_mut::<BloodStains>().clear();
    world.resource_mut::<Smoke>().clear();
    world.resource_mut::<Corpses>().clear();
    rooms
}

/// Stands the player on the stair they arrive at and blanks their memory of the
/// floor. Descending drops them on the new floor's up-stair; ascending brings
/// them out at the shallower floor's down-stair, which every floor above the
/// last one has.
fn put_the_player_down(world: &mut World, player: Entity, going_down: bool) -> (u16, u16) {
    let stair = match going_down {
        true => TileType::Upstairs,
        false => TileType::Downstairs,
    };
    let start = find_tile(&world.resource::<Map>().tiles, stair)
        .expect("a floor arrived at by stair has the stair to arrive on");
    if let Some(mut pos) = world.get_mut::<Position>(player) {
        pos.x = start.0;
        pos.y = start.1;
    }
    // Fog of war is not carried between visits: walk back up through a floor
    // you cleared and it is blank again, even though the walls are identical.
    if let Some(mut viewshed) = world.get_mut::<Viewshed>(player) {
        viewshed.visible_tiles.clear();
        viewshed.revealed_tiles.clear();
        viewshed.dirty = true;
    }
    start
}

/// What arriving does *to the player*, which is where the four causes stop
/// being the same event: the rest, the promises, the conditions and the clock.
fn settle_arrival(world: &mut World, player: Entity, cause: LevelChange) {
    // A trapdoor plunge is a fall, not a rest: no arrival heal, no magic.
    if cause != LevelChange::Trapdoor {
        if let Some(mut fighter) = world.get_mut::<Fighter>(player) {
            let heal = fighter.max_hp / DESCENT_HEAL_DIVISOR;
            fighter.hp = (fighter.hp + heal).min(fighter.max_hp);
        }
        if let Some(mut magic) = world.get_mut::<Magic>(player) {
            magic.points = magic.max_points;
        }
    }

    // A staircase reached unhurt is what the platinum and forge coins asked
    // for, and this is where they pay. Only a staircase: a trapdoor is not
    // arriving somewhere, it is falling, and neither is the Dungeon Lord's
    // portal or a potion drunk to skip a floor.
    if cause == LevelChange::Stairs {
        crate::items::settle_promises(world, player);
    }

    // Transient conditions (haste, slow, dazzle, blindness, paralysis, a
    // potion's floor-long second sight) are treacherous but they do not survive
    // a level change — this is one of only two things that clears them.
    crate::conditions::clear_player_conditions(world, player);

    if let Some(mut dl) = world.get_resource_mut::<DungeonLord>() {
        dl.idle_turns = 0;
    }
}

/// The one sentence the player reads about how they got here.
fn arrival_line(cause: LevelChange, going_down: bool, depth: u8) -> String {
    match cause {
        // Descending, it is the Dungeon Lord who wrenches you down; once you
        // carry the Element it is the Element that tears the way open upward.
        LevelChange::Portal if going_down => strings::portal_down(depth),
        LevelChange::Portal => strings::portal_up(depth),
        LevelChange::Trapdoor => strings::trapdoor_arrival(depth),
        LevelChange::Potion => strings::potion_arrival(depth),
        LevelChange::Stairs if going_down => strings::descend_stairs(depth),
        LevelChange::Stairs => strings::climb_stairs(depth),
    }
}

/// Whoever died on `depth` before, come back for the player — only when this
/// character has reached it carrying the Element of Yoord (see the caller in
/// [`transition_level`]), and only when a bones file is actually waiting
/// there (`crate::bones::take`, which also deletes it: one encounter per
/// death). A no-op otherwise.
fn spawn_bones_ghost(world: &mut World, depth: u8, rooms: &Rooms) {
    let Some(bones) = crate::bones::take(depth) else {
        return;
    };

    let occupied: HashSet<(u16, u16)> = world
        .query::<&Position>()
        .iter(world)
        .map(|p| (p.x, p.y))
        .collect();
    let mut rng = world.remove_resource::<GameRng>().unwrap().0;
    let mut pos = None;
    for _ in 0..10 {
        let room = &rooms[rng.gen_range(0..rooms.len())];
        let spot = room[rng.gen_range(0..room.len())];
        if !occupied.contains(&spot) {
            pos = Some(spot);
            break;
        }
    }
    world.insert_resource(GameRng(rng));
    let Some((x, y)) = pos else { return };
    let pos = Position { x, y };

    let ghost = spawn_monster(world, &GHOST, pos);
    world.entity_mut(ghost).insert(Name {
        what: bones.name.clone(),
    });
    let is_self = bones.name == world.resource::<PlayerName>().what;
    if is_self {
        world.entity_mut(ghost).insert(GhostOfPlayer);
    }

    for (name, slot, power_bonus, armor_bonus, throw_bonus, stack) in bones.items() {
        let Some(item) = spawn_named(world, name, pos) else {
            continue;
        };
        {
            let mut e = world.entity_mut(item);
            e.insert(PowerBonus(power_bonus));
            e.insert(ArmorBonus(armor_bonus));
            e.insert(ThrowBonus(throw_bonus));
            if let Some(count) = stack {
                e.insert(Stack { count });
            }
            if slot.is_some() {
                e.insert(Curse);
            }
        }
        if slot.is_some() {
            equip_silently(world, ghost, item);
        }
    }

    let line = match is_self {
        true => strings::bones_ghost_arrives_self(),
        false => strings::bones_ghost_arrives(&bones.name),
    };
    world
        .resource_mut::<GameLog>()
        .add_colored(line, LogCategory::Ghost);
}

/// Exclusive system, run each turn just before visibility is recomputed. Ages
/// the Dungeon Lord's patience; when it runs out, a portal shunts the player to
/// the next level — deeper on the way in, back up once they carry the Element of
/// Yoord. On the deepest floor (without the Element) or the shallowest floor
/// (with it) the portal has nowhere to send them and only flickers.
///
/// Assumes `reaper_system`, immediately ahead of it in the schedule, has
/// already settled this turn's fatalities — a melee kill through
/// `combat_system`'s own `settle_the_dead`, or an indirect one (a wand bolt,
/// a blast) `reaper_system` finishes itself — so `Ending::player_dead` is
/// already set if this turn killed the player, and a portal here never opens
/// under someone already gone. Checked directly, not merely scheduled: the
/// `player_dead` guard below is that assumption enforced, not incidental
/// bookkeeping.
pub fn dungeon_lord_system(world: &mut World) {
    if world
        .get_resource::<Ending>()
        .map(|e| e.player_dead)
        .unwrap_or(false)
    {
        return;
    }
    match world.get_resource_mut::<DungeonLord>() {
        Some(mut dl) => {
            dl.idle_turns += 1;
            if dl.idle_turns < DUNGEON_LORD_PATIENCE {
                return;
            }
            dl.idle_turns = 0;
        }
        None => return,
    }

    let has_element = holding_element_of_yoord(world);
    let depth = world.resource::<Depth>().what;

    if has_element {
        if depth <= 1 {
            world
                .resource_mut::<GameLog>()
                .add(strings::element_wont_let_you_land());
            return;
        }
        transition_level(world, false, LevelChange::Portal);
        return;
    }
    if depth >= FINAL_DEPTH {
        world
            .resource_mut::<GameLog>()
            .add(strings::portal_no_deeper_floor());
        return;
    }
    transition_level(world, true, LevelChange::Portal);
}

pub fn initialize_world(world: &mut World) {
    world.insert_resource(GameState::new());
    world.insert_resource(Depth { what: 1 });
    world.insert_resource(FloorChanges::default());
    world.insert_resource(BloodStains::new());
    world.insert_resource(Smoke::new());
    world.insert_resource(Corpses::new());
    world.init_resource::<crate::bones::Bones>();
    world.init_resource::<crate::magicmap::MagicMapReveal>();
    world.init_resource::<crate::score::ScoreFlash>();
    world.init_resource::<crate::score::Combo>();
    let seed = world.resource::<RngSeed>().0;
    world.insert_resource(FxRng::new(seed));

    let ((player_x, player_y), rooms) = create_map(world);

    // What the player wakes up as: nihil, a lurk, or a bestiary row.
    let body = world
        .get_resource::<crate::body::StartingBody>()
        .map_or(crate::body::Body::default(), |b| b.0);

    // The starting gear — *nihil's* starting gear. Nothing else here has the
    // hands for a mace, and a monster is not stocked the way a floor stocks
    // one.
    let kit = match body.brings_a_pack() {
        true => starting_kit(world),
        false => Vec::new(),
    };

    let player_name = world.resource::<PlayerName>().what.clone();

    let player = world
        .spawn((
            Player,
            Name { what: player_name },
            Position {
                x: player_x,
                y: player_y,
            },
            Renderable {
                glyph: '@',
                color: Color::Yellow,
            },
            Viewshed {
                visible_tiles: Vec::new(),
                revealed_tiles: FixedBitSet::with_capacity(MAP_TILE_COUNT),
                range: SIGHT_RANGE,
                dirty: true,
            },
            Fighter {
                hp: START_HP,
                max_hp: START_HP,
                armor: START_ARMOR,
                power: START_POWER,
                max_power: START_POWER,
                armor_bonus: 0,
                power_bonus: 0,
            },
            Magic {
                points: START_MAGIC,
                max_points: START_MAGIC,
            },
            Faction::Player,
            Backpack { items: kit.clone() },
            Score { value: 0 },
            Blood,
            Speed::new(SpeedKind::Normal),
            // Empty at the start of a run: every spell is learned from a
            // hero coin (see `crate::items::pickups::learn_spell`), up to four.
            Spellset::default(),
        ))
        .id();

    // The body goes on last: it overwrites the stats, glyph and tempo the
    // spawn above just laid down with whatever this creature actually is.
    crate::body::wear(world, player, body);
    if body.brings_a_pack() {
        // Wear the armour and wield the mace. The bow and arrows wait in the
        // pack: both weapons want the same hand, and which one the player
        // reaches for first is the first decision the game asks them to make.
        equip_silently(world, player, kit[KIT_ARMOR]);
        equip_silently(world, player, kit[KIT_WEAPON]);
    }

    populate_level(world, &rooms, (player_x, player_y));
}

/// Where the two pieces the player starts out already wearing sit in
/// [`starting_kit`]'s list.
const KIT_ARMOR: usize = 0;
const KIT_WEAPON: usize = 1;

/// What nihil starts with: ring mail, a mace, a short bow with a full quiver and one
/// potion of healing. Every piece is spawned at the origin like a drop, then
/// lifted straight into the pack (Position stripped, the way a picked-up item
/// loses it) so it never shows up as floor loot. The armour, mace and bow are
/// handed over enchanted to +1 rather than rolled.
fn starting_kit(world: &mut World) -> Vec<Entity> {
    let origin = Position { x: 0, y: 0 };
    let pack_up = |world: &mut World, item: Entity| {
        world.entity_mut(item).remove::<Position>();
    };

    let ring_mail = spawn_armor(world, "ring mail", origin);
    world
        .entity_mut(ring_mail)
        .insert(crate::effects::ArmorBonus(1));
    pack_up(world, ring_mail);

    let mace = spawn_weapon(world, "mace", origin);
    world.entity_mut(mace).insert(crate::effects::PowerBonus(1));
    pack_up(world, mace);

    let shortbow = spawn_launcher(world, "short bow", origin);
    world
        .entity_mut(shortbow)
        .insert(crate::effects::ThrowBonus(1));
    pack_up(world, shortbow);

    let arrows = spawn_ammo(world, "arrow", origin);
    if let Some(mut stack) = world.get_mut::<Stack>(arrows) {
        stack.count = STACK_LIMIT;
    }
    pack_up(world, arrows);

    let healing = spawn_potion(world, PotionEffect::Healing, origin);
    pack_up(world, healing);

    vec![ring_mail, mace, shortbow, arrows, healing]
}
