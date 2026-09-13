use crate::components::*;
use crate::effects::Stealthy;
use crate::map::{Map, TileType};
use bevy_ecs::prelude::*;
use std::collections::{HashMap, HashSet};

// --- Tuning constants ------------------------------------------------------
// Defined and documented in `constants.rs`.
//
//   STEALTH_RANGE  how close a stealthy player must be to be noticed
use crate::constants::rings::STEALTH_RANGE;

/// Monster turn. Exclusive so it can move each mob more than once: the player is
/// the clock, and every creature banks [`Speed`] energy each of the player's
/// turns, spending [`Speed::COST`] per step. A `Fast` monster gets two moves per
/// player turn and a `Slow` one moves every other turn. A mob with no [`Speed`]
/// component (test dummies) simply acts once.
///
/// The player's own tempo scales how many monster *rounds* a single turn buys: a
/// `Fast` player's turns alternate one-round / no-round (tracked by
/// [`PlayerTempo::fast_parity`]), and a `Slow` player's one turn buys two rounds.
/// Every other schedule step (traps, visibility, the Dungeon Lord's patience)
/// still ticks exactly once per player turn.
pub fn ai(world: &mut World) {
    // 1. Player snapshot: entity, position, the tiles it can see, its faction.
    #[allow(clippy::type_complexity)] // one query for the whole player snapshot
    let Some((player_entity, player_pos, visible_tiles, player_blind, player_faction)) = ({
        let mut q = world
            .query_filtered::<(Entity, &Position, &Viewshed, Option<&Blind>, &Faction), With<Player>>(
            );
        q.iter(world).next().map(|(e, p, v, b, f)| {
            let seen: HashSet<(u16, u16)> = v.visible_tiles.iter().copied().collect();
            (e, *p, seen, b.is_some(), *f)
        })
    }) else {
        return;
    };
    // The tempo the player is *acting* at, gear and all — a ring of slow
    // digestion is a slowing like any other, and buys the floor the same extra
    // round a potion of paralysis would.
    let player_speed = crate::conditions::tempo(world, player_entity);
    // A stealthy player is not there as far as the floor is concerned until
    // they are within arm's reach (see [`notices`]).
    let player_stealthy = world.get::<Stealthy>(player_entity).is_some();

    // 2. How many monster rounds this one player turn is worth.
    let rounds = match player_speed {
        SpeedKind::Normal => 1,
        SpeedKind::Slow => 2,
        SpeedKind::Fast => {
            // Two player turns per monster round: act on the turn the parity
            // flips back off, skip the other.
            let act = world
                .get_resource_mut::<PlayerTempo>()
                .map(|mut t| {
                    t.fast_parity = !t.fast_parity;
                    !t.fast_parity
                })
                .unwrap_or(true);
            if act { 1 } else { 0 }
        }
    };

    let map = world.resource::<Map>().clone();

    // Blindness is the player's problem, not the dungeon's. A blinded hero's
    // viewshed is cut to the 3x3 they can feel around them, but the monsters in
    // the lit room they are standing in can all still see them perfectly well —
    // so the AI works off the view the player *would* have with their eyes open.
    // Without this, drinking a potion of blindness would be a way to hide.
    let visible_tiles = match player_blind {
        true => crate::visibility::visible_from(&map, &player_pos, false),
        false => visible_tiles,
    };

    for _round in 0..rounds {
        monster_round(
            world,
            player_entity,
            player_pos,
            &visible_tiles,
            player_faction,
            player_stealthy,
            &map,
        );
    }
}

/// Whether a mob standing on `mob_pos` knows where the player is this turn.
///
/// Ordinarily that is simply "is its tile in the player's view" — sight is
/// symmetrical in roog. A ring of stealth breaks the symmetry: the player can
/// see the length of a lit room and nothing in it can see them back until they
/// are [`STEALTH_RANGE`] tiles away, at which point being quiet stops helping.
fn notices(seen: bool, stealthy: bool, player_pos: Position, mob_pos: Position) -> bool {
    if !seen {
        return false;
    }
    if !stealthy {
        return true;
    }
    let dx = (player_pos.x as i32 - mob_pos.x as i32).abs();
    let dy = (player_pos.y as i32 - mob_pos.y as i32).abs();
    dx.max(dy) <= STEALTH_RANGE
}

/// One full round of monster movement: bank energy, then up to two passes so a
/// `Fast` monster can act twice. A pass that moves nobody ends the round.
fn monster_round(
    world: &mut World,
    player_entity: Entity,
    player_pos: Position,
    visible_tiles: &HashSet<(u16, u16)>,
    player_faction: Faction,
    player_stealthy: bool,
    map: &Map,
) {
    // Bank this round's energy for every actor with a tempo, at the tempo it is
    // actually acting at — gear that weighs a creature down banks it less. The
    // pool is capped so a monster left alone off-screen can't hoard a dozen free
    // moves for when it finally reaches you.
    {
        let actors: Vec<Entity> = world
            .query_filtered::<Entity, With<Speed>>()
            .iter(world)
            .collect();
        for actor in actors {
            let rate = crate::conditions::tempo(world, actor).rate();
            if let Some(mut speed) = world.get_mut::<Speed>(actor) {
                speed.energy = (speed.energy + rate).min(2 * Speed::COST);
            }
        }
    }

    for pass in 0..2 {
        let mob_list: Vec<Entity> = world
            .query_filtered::<Entity, (With<Mob>, Without<Player>)>()
            .iter(world)
            .collect();

        // Rebuilt each pass so a mob that moved in pass 0 is seen in its new
        // tile in pass 1.
        let mut spatial = actor_positions(world, player_entity, player_pos, player_faction);

        let mut any_acted = false;
        for mob in mob_list {
            any_acted |= step_one_mob(
                world,
                mob,
                pass,
                player_pos,
                visible_tiles,
                player_stealthy,
                map,
                &mut spatial,
            );
        }
        if !any_acted {
            break;
        }
    }
}

/// The tile → actor map for a pass: the player, plus every mob at its current
/// position.
fn actor_positions(
    world: &mut World,
    player_entity: Entity,
    player_pos: Position,
    player_faction: Faction,
) -> HashMap<(u16, u16), (Entity, Faction)> {
    let mut spatial = HashMap::new();
    spatial.insert(
        (player_pos.x, player_pos.y),
        (player_entity, player_faction),
    );
    let mut q =
        world.query_filtered::<(Entity, &Position, &Faction), (With<Mob>, Without<Player>)>();
    for (e, p, f) in q.iter(world) {
        spatial.insert((p.x, p.y), (e, *f));
    }
    spatial
}

/// One mob's turn within a pass: forfeit if asleep or out of energy (a
/// bear-trapped mob may still strike but not step), work out where it wants to
/// go, then either queue an attack or take the step. Returns whether it did
/// anything — a whole idle pass ends the round.
#[allow(clippy::too_many_arguments)] // one mob's whole turn, and the turn's facts
fn step_one_mob(
    world: &mut World,
    mob: Entity,
    pass: usize,
    player_pos: Position,
    visible_tiles: &HashSet<(u16, u16)>,
    player_stealthy: bool,
    map: &Map,
    spatial: &mut HashMap<(u16, u16), (Entity, Faction)>,
) -> bool {
    // Asleep in gas: forfeit the turn outright. Caught in a bear trap or bound
    // by a scroll of hold monster: the mob can't take a step, but a foe within
    // reach still gets bitten.
    let pinned = match world.get::<Snare>(mob).map(|s| s.kind) {
        Some(SnareKind::Sleep) => return false,
        Some(SnareKind::Bear | SnareKind::Hold) => true,
        None => false,
    };
    if !can_afford_step(world, mob, pass) {
        return false;
    }

    let mob_pos = *world.get::<Position>(mob).unwrap();
    let movement_type = world.get::<Mob>(mob).unwrap().movement_type;
    let mob_faction = *world.get::<Faction>(mob).unwrap();

    let in_view = visible_tiles.contains(&(mob_pos.x, mob_pos.y));
    let seen = notices(in_view, player_stealthy, player_pos, mob_pos);
    let Some((step_x, step_y)) = desired_step(movement_type, player_pos, mob_pos, seen) else {
        return false;
    };
    let new_x = (mob_pos.x as i16 + step_x) as u16;
    let new_y = (mob_pos.y as i16 + step_y) as u16;
    if !mob_can_enter(map, movement_type, mob_pos, new_x, new_y) {
        return false;
    }

    // Entity in the way: attack if hostile, otherwise stand.
    if let Some(&(target_entity, target_faction)) = spatial.get(&(new_x, new_y)) {
        if !hostile(mob_faction, target_faction) {
            return false;
        }
        world
            .resource_mut::<AttackQueue>()
            .attacks
            .push(WantsToAttack {
                attacker: mob,
                target: target_entity,
            });
        spend_energy(world, mob);
        return true;
    }

    // Pinned where it stands: it may lash out (above) but not step.
    if pinned {
        return false;
    }

    // Path clear: move.
    spatial.remove(&(mob_pos.x, mob_pos.y));
    if let Some(mut pos) = world.get_mut::<Position>(mob) {
        pos.x = new_x;
        pos.y = new_y;
    }
    spatial.insert((new_x, new_y), (mob, mob_faction));
    world.entity_mut(mob).insert(EntityMoved);
    spend_energy(world, mob);
    true
}

/// Whether `mob` can spend a step this pass: with a tempo it must be able to
/// afford [`Speed::COST`]; without one it acts on pass 0 only.
fn can_afford_step(world: &mut World, mob: Entity, pass: usize) -> bool {
    match world.get::<Speed>(mob).map(|s| s.energy) {
        Some(energy) => energy >= Speed::COST,
        None => pass == 0,
    }
}

/// Whether these two factions come to blows.
fn hostile(a: Faction, b: Faction) -> bool {
    matches!(
        (a, b),
        (Faction::Monster, Faction::Player)
            | (Faction::Monster, Faction::Ally)
            | (Faction::Ally, Faction::Monster)
    )
}

/// The one-tile step a mob wants this turn, or `None` when it holds position —
/// a `Static` mob always, a `Chase`/`Flee` mob whose tile the player can't see.
fn desired_step(
    movement_type: MovementType,
    player_pos: Position,
    mob_pos: Position,
    seen: bool,
) -> Option<(i16, i16)> {
    let toward = |gx: u16, gy: u16| {
        (
            (gx as i16 - mob_pos.x as i16).signum(),
            (gy as i16 - mob_pos.y as i16).signum(),
        )
    };
    match movement_type {
        MovementType::Static => None,
        MovementType::Chase if !seen => None,
        MovementType::Flee if !seen => None,
        MovementType::Chase => Some(toward(player_pos.x, player_pos.y)),
        MovementType::Flee => {
            let (sx, sy) = toward(player_pos.x, player_pos.y);
            Some((-sx, -sy))
        }
        MovementType::Confused => Some(random_orthogonal_step()),
        MovementType::Aggravated { tx, ty } => {
            // Head for the tile the shriek came from, from anywhere on the floor
            // — but lunge once the player is right alongside.
            let adjacent = (player_pos.x as i16 - mob_pos.x as i16).abs() <= 1
                && (player_pos.y as i16 - mob_pos.y as i16).abs() <= 1;
            let (gx, gy) = if adjacent {
                (player_pos.x, player_pos.y)
            } else {
                (tx, ty)
            };
            Some(toward(gx, gy))
        }
    }
}

/// A random N/S/E/W step, for a confused monster. A failed RNG draw goes east.
fn random_orthogonal_step() -> (i16, i16) {
    const DIRS: [(i16, i16); 4] = [(1, 0), (-1, 0), (0, 1), (0, -1)];
    let idx = getrandom::u32().map_or(0, |v| v as usize) % DIRS.len();
    DIRS[idx]
}

/// Whether `mob` may step onto `(new_x, new_y)`: on the map, not a wall, a
/// legal diagonal, and — for a chaser — not out of a room into a corridor or
/// doorway (the room leash).
fn mob_can_enter(
    map: &Map,
    movement_type: MovementType,
    mob_pos: Position,
    new_x: u16,
    new_y: u16,
) -> bool {
    if new_x >= 80 || new_y >= 22 {
        return false;
    }
    if map.blocks(new_x, new_y) {
        return false;
    }
    if !map.diagonal_step_ok(mob_pos.x, mob_pos.y, new_x, new_y) {
        return false;
    }
    // Room leash: a chaser won't step from a room into a corridor or doorway.
    let leashed = matches!(movement_type, MovementType::Chase)
        && map.tile(mob_pos.x, mob_pos.y) == TileType::Room
        && matches!(map.tile(new_x, new_y), TileType::Passage | TileType::Door);
    !leashed
}

/// Deducts one action's worth of energy from `entity`, if it has a tempo.
fn spend_energy(world: &mut World, entity: Entity) {
    if let Some(mut speed) = world.get_mut::<Speed>(entity) {
        speed.energy -= Speed::COST;
    }
}
