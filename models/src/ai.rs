use crate::components::*;
use crate::map::{Map, TileType};
use crate::traps::{EntityMoved, Snare};
use bevy_ecs::prelude::*;
use std::collections::{HashMap, HashSet};

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
    let Some((player_entity, player_pos, visible_tiles, player_faction, player_speed)) = ({
        let mut q = world
            .query_filtered::<(Entity, &Position, &Viewshed, &Faction, Option<&Speed>), With<Player>>();
        q.iter(world).next().map(|(e, p, v, f, s)| {
            let seen: HashSet<(u16, u16)> = v.visible_tiles.iter().copied().collect();
            (e, *p, seen, *f, s.map_or(SpeedKind::Normal, |s| s.kind))
        })
    }) else {
        return;
    };

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

    for _round in 0..rounds {
        monster_round(
            world,
            player_entity,
            player_pos,
            &visible_tiles,
            player_faction,
            &map,
        );
    }
}

/// One full round of monster movement: bank energy, then up to two passes so a
/// `Fast` monster can act twice.
fn monster_round(
    world: &mut World,
    player_entity: Entity,
    player_pos: Position,
    visible_tiles: &HashSet<(u16, u16)>,
    player_faction: Faction,
    map: &Map,
) {
    // Bank this round's energy for every actor with a tempo. The pool is capped
    // so a monster left alone off-screen can't hoard a dozen free moves for when
    // it finally reaches you.
    {
        let mut q = world.query::<&mut Speed>();
        for mut speed in q.iter_mut(world) {
            let rate = speed.kind.rate();
            speed.energy = (speed.energy + rate).min(2 * Speed::COST);
        }
    }

    // Up to two movement passes — enough for a `Fast` monster to act twice.
    for pass in 0..2 {
        let mob_list: Vec<Entity> = world
            .query_filtered::<Entity, (With<Mob>, Without<Player>)>()
            .iter(world)
            .collect();

        // The actor spatial map, rebuilt each pass so a mob that moved in pass 0
        // is seen in its new tile in pass 1.
        let mut spatial: HashMap<(u16, u16), (Entity, Faction)> = HashMap::new();
        spatial.insert(
            (player_pos.x, player_pos.y),
            (player_entity, player_faction),
        );
        {
            let mut q = world
                .query_filtered::<(Entity, &Position, &Faction), (With<Mob>, Without<Player>)>();
            for (e, p, f) in q.iter(world) {
                spatial.insert((p.x, p.y), (e, *f));
            }
        }

        let mut any_acted = false;

        for mob_entity in mob_list {
            // Held fast by a bear trap or asleep in gas: forfeit the turn.
            if world.get::<Snare>(mob_entity).is_some() {
                continue;
            }

            // Energy gate. With a tempo, the mob must be able to afford a step;
            // without one it acts on the first pass only.
            match world.get::<Speed>(mob_entity).map(|s| s.energy) {
                Some(energy) => {
                    if energy < Speed::COST {
                        continue;
                    }
                }
                None => {
                    if pass > 0 {
                        continue;
                    }
                }
            }

            let mob_pos = *world.get::<Position>(mob_entity).unwrap();
            let movement_type = world.get::<Mob>(mob_entity).unwrap().movement_type;
            let mob_faction = *world.get::<Faction>(mob_entity).unwrap();

            // Out of the player's sight, only ambient movers keep going.
            if !visible_tiles.contains(&(mob_pos.x, mob_pos.y)) {
                match movement_type {
                    MovementType::Chase | MovementType::Flee => continue,
                    _ => {}
                }
            }

            let (step_x, step_y) = match movement_type {
                MovementType::Static => continue,
                MovementType::Chase => {
                    let dx = player_pos.x as i16 - mob_pos.x as i16;
                    let dy = player_pos.y as i16 - mob_pos.y as i16;
                    (dx.signum(), dy.signum())
                }
                MovementType::Flee => {
                    let dx = player_pos.x as i16 - mob_pos.x as i16;
                    let dy = player_pos.y as i16 - mob_pos.y as i16;
                    (-dx.signum(), -dy.signum())
                }
                MovementType::Confused => {
                    let directions = [(1, 0), (-1, 0), (0, 1), (0, -1)];
                    let idx = match getrandom::u32() {
                        Ok(val) => (val as usize) % directions.len(),
                        Err(_) => 0,
                    };
                    directions[idx]
                }
                MovementType::Aggravated { tx, ty } => {
                    // Head for the tile the shriek came from — from anywhere on
                    // the floor, seen or unseen — but lunge once the player is
                    // right alongside.
                    let pdx = (player_pos.x as i16 - mob_pos.x as i16).abs();
                    let pdy = (player_pos.y as i16 - mob_pos.y as i16).abs();
                    let (goal_x, goal_y) = if pdx <= 1 && pdy <= 1 {
                        (player_pos.x, player_pos.y)
                    } else {
                        (tx, ty)
                    };
                    (
                        (goal_x as i16 - mob_pos.x as i16).signum(),
                        (goal_y as i16 - mob_pos.y as i16).signum(),
                    )
                }
            };

            let new_x = (mob_pos.x as i16 + step_x) as u16;
            let new_y = (mob_pos.y as i16 + step_y) as u16;

            if new_x >= 80 || new_y >= 22 {
                continue;
            }
            if map.blocks(new_x, new_y) {
                continue;
            }
            // A diagonal step only connects tiles of the same kind.
            if !map.diagonal_step_ok(mob_pos.x, mob_pos.y, new_x, new_y) {
                continue;
            }

            // Room leash: a chaser won't follow the player out of a room into a
            // corridor or doorway.
            if matches!(movement_type, MovementType::Chase) {
                let current_tile = map.tile(mob_pos.x, mob_pos.y);
                let target_tile = map.tile(new_x, new_y);
                if current_tile == TileType::Room
                    && matches!(target_tile, TileType::Passage | TileType::Door)
                {
                    continue;
                }
            }

            // Entity in the way: attack if hostile, otherwise stand.
            if let Some(&(target_entity, target_faction)) = spatial.get(&(new_x, new_y)) {
                let is_hostile = matches!(
                    (mob_faction, target_faction),
                    (Faction::Monster, Faction::Player)
                        | (Faction::Monster, Faction::Ally)
                        | (Faction::Ally, Faction::Monster)
                );
                if is_hostile {
                    world
                        .resource_mut::<AttackQueue>()
                        .attacks
                        .push(WantsToAttack {
                            attacker: mob_entity,
                            target: target_entity,
                        });
                    spend_energy(world, mob_entity);
                    any_acted = true;
                }
                continue;
            }

            // Path clear: move.
            spatial.remove(&(mob_pos.x, mob_pos.y));
            if let Some(mut pos) = world.get_mut::<Position>(mob_entity) {
                pos.x = new_x;
                pos.y = new_y;
            }
            spatial.insert((new_x, new_y), (mob_entity, mob_faction));
            world.entity_mut(mob_entity).insert(EntityMoved);
            spend_energy(world, mob_entity);
            any_acted = true;
        }

        if !any_acted {
            break;
        }
    }
}

/// Deducts one action's worth of energy from `entity`, if it has a tempo.
fn spend_energy(world: &mut World, entity: Entity) {
    if let Some(mut speed) = world.get_mut::<Speed>(entity) {
        speed.energy -= Speed::COST;
    }
}
