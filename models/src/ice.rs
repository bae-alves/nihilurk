//! Ice cubes: what a creature becomes when cold kills it.
//!
//! A cube is a [`Mob`] without a [`Fighter`], so everything that treats a
//! creature as something standing on a tile (the player's step, a monster's
//! path, a missile's line) treats it as a wall of ice. Nothing can hurt it. The
//! player breaks it by walking into it: it flies at the nearest weakest foe
//! ([`auto_fight_target`], the same pick Tab makes) for cold damage, and
//! shatters on impact.

use bevy_ecs::prelude::*;
use crossterm::style::Color;
use rand::Rng;

use crate::BlastPalette;
use crate::autofight::auto_fight_target;
use crate::components::{Element, Faction, Fighter, GameLog, Helper, Mob, MovementType};
use crate::constants::ice::{BONE_SHARDS, DAMAGE_DICE, DAMAGE_SIDES, FLIGHT_RANGE, VAPOR_RADIUS};
use crate::helpers::{Hit, apply_hit, get_line, player_sees, roll_dice};
use crate::map::{FxRng, Map};
use crate::particles::Particles;
use crate::shake::{ShakeKind, kick_shake};
use crate::{Name, Position, Renderable};

/// A frozen corpse: blocks like a creature, has nothing to hurt, and breaks
/// when the player walks into it ([`kick_ice_cube`]).
#[derive(Component)]
pub struct IceCube;

/// Set by [`apply_hit`] on a creature a cold hit just dropped to zero, so
/// whatever finishes the kill knows to freeze it. A creature a cold hit only
/// wounded never gets it.
#[derive(Component)]
pub struct ColdSlain;

/// Marks `entity` [`ColdSlain`] if `hit` was cold and left it dead.
pub(crate) fn note_cold_kill(world: &mut World, entity: Entity, hit: Hit) {
    let dead = world.get::<Fighter>(entity).is_some_and(|f| f.hp <= 0);
    if dead && hit.element == Some(Element::Cold) {
        world.entity_mut(entity).insert(ColdSlain);
    }
}

/// Whether a death should leave a cube: cold dealt the last blow, and the dead
/// are an enemy (a [`Helper`] is mourned, not frozen).
pub(crate) fn freezes(world: &World, entity: Entity) -> bool {
    world.get::<ColdSlain>(entity).is_some() && world.get::<Helper>(entity).is_none()
}

/// A cube on `pos`. The one constructor: [`encase`] and the save loader both
/// use it.
pub(crate) fn spawn_ice_cube(world: &mut World, pos: Position) -> Entity {
    world
        .spawn((
            IceCube,
            Mob {
                movement_type: MovementType::Static,
            },
            Faction::Inert,
            Name {
                what: strings::ice_cube_name().to_string(),
            },
            Renderable {
                glyph: '#',
                color: Color::Cyan,
            },
            pos,
        ))
        .id()
}

/// Replaces the dead `victim` with a cube where it fell, with a little vapor
/// to make up for the missing gore. Despawns the victim.
pub(crate) fn encase(world: &mut World, victim: Entity) {
    let Some(pos) = world.get::<Position>(victim).copied() else {
        return;
    };
    crate::spirits::poof(world, victim);
    spawn_ice_cube(world, pos);
    if let Some(mut fx) = world.get_resource_mut::<Particles>() {
        fx.explosion(&[(pos.x, pos.y, 0.0)], BlastPalette::Frost);
        fx.tinted_poof(pos.x, pos.y, 0.0, Color::Cyan);
    }
}

/// The player boots `cube`. It homes on the nearest weakest foe in sight for
/// [`DAMAGE_DICE`]d[`DAMAGE_SIDES`] cold damage; with none, it flies the way it
/// was kicked until a wall stops it. Either way it shatters. Always spends the
/// turn.
pub fn kick_ice_cube(world: &mut World, player: Entity, cube: Entity) -> bool {
    let (Some(from), Some(me)) = (
        world.get::<Position>(cube).copied(),
        world.get::<Position>(player).copied(),
    ) else {
        return false;
    };
    let target = auto_fight_target(world);
    let landing = match target.and_then(|t| world.get::<Position>(t).copied()) {
        Some(at) => at,
        None => fly_until_wall(world, from, me),
    };
    world.entity_mut(cube).despawn();

    let pts: Vec<(u16, u16)> = get_line(from, landing)
        .into_iter()
        .filter(|&p| p != from)
        .map(|p| (p.x, p.y))
        .collect();
    if let Some(mut fx) = world.get_resource_mut::<Particles>() {
        fx.hurl_at(&pts, '#', Color::Cyan, crate::particles::THROW_SPEEDUP);
    }

    if let Some(foe) = target {
        let dmg = roll_dice(world, DAMAGE_DICE, DAMAGE_SIDES);
        let name = crate::helpers::item_label(world, foe);
        let line = strings::ice_cube_slams(&name);
        apply_hit(world, foe, Hit::elemental(dmg, Element::Cold), Some(&line));
        if world.get::<Fighter>(foe).is_some_and(|f| f.hp <= 0) {
            crate::combat::finish_indirect_kill(world, foe, Some(from));
        }
    }
    shatter(world, landing);
    true
}

/// The last open tile on the ray from `me` through `from`, at most
/// [`FLIGHT_RANGE`] out.
fn fly_until_wall(world: &World, from: Position, me: Position) -> Position {
    let dx = (from.x as i32 - me.x as i32).signum();
    let dy = (from.y as i32 - me.y as i32).signum();
    let map = world.resource::<Map>();
    let mut at = from;
    for _ in 0..FLIGHT_RANGE {
        let (x, y) = (at.x as i32 + dx, at.y as i32 + dy);
        if x < 0 || y < 0 || map.blocks(x as u16, y as u16) {
            break;
        }
        at = Position {
            x: x as u16,
            y: y as u16,
        };
    }
    at
}

/// The cells from `pos` out along `(dx, dy)`, `reach` long, ending on the first
/// wall (included, so blood lands on it).
fn ray(map: &Map, pos: Position, (dx, dy): (i32, i32), reach: i32) -> Vec<(u16, u16)> {
    let mut out = Vec::new();
    for i in 1..=reach {
        let (x, y) = (pos.x as i32 + dx * i, pos.y as i32 + dy * i);
        let Some(cell) = crate::particles::on_map(x, y) else {
            break;
        };
        out.push(cell);
        if map.blocks(cell.0, cell.1) {
            break;
        }
    }
    out
}

const AROUND: [(i32, i32); 8] = [
    (-1, -1),
    (0, -1),
    (1, -1),
    (-1, 0),
    (1, 0),
    (-1, 1),
    (0, 1),
    (1, 1),
];

/// Whatever was frozen inside comes out as bones, and the cold it was holding
/// comes out as vapor. No blood: the cube kept it. Cosmetic.
pub(crate) fn shatter(world: &mut World, pos: Position) {
    world
        .resource_mut::<GameLog>()
        .add(strings::ice_cube_shatters().to_string());
    if player_sees(world, pos.x, pos.y) {
        kick_shake(world, ShakeKind::Heavy);
    }
    if world.get_resource::<FxRng>().is_none() {
        return;
    }
    let map = world.resource::<Map>().clone();

    let mut vapor: Vec<(u16, u16, f32)> = Vec::new();
    for dy in -VAPOR_RADIUS..=VAPOR_RADIUS {
        for dx in -VAPOR_RADIUS..=VAPOR_RADIUS {
            if let Some(c) = crate::particles::on_map(pos.x as i32 + dx, pos.y as i32 + dy) {
                if !map.blocks(c.0, c.1) {
                    vapor.push((c.0, c.1, ((dx * dx + dy * dy) as f32).sqrt()));
                }
            }
        }
    }
    const SHARDS: [char; 5] = ['/', '\\', '|', '¡', '*'];
    let flights: Vec<Vec<(u16, u16)>> = {
        let mut rng = world.resource_mut::<FxRng>();
        (0..BONE_SHARDS)
            .map(|_| {
                let dir = AROUND[rng.0.gen_range(0..AROUND.len())];
                ray(&map, pos, dir, rng.0.gen_range(2..=5))
            })
            .collect()
    };
    if let Some(mut fx) = world.get_resource_mut::<Particles>() {
        fx.explosion(&vapor, BlastPalette::Frost);
        fx.smoke_burst(&vapor);
        for (i, path) in flights.iter().enumerate() {
            fx.bone_shard(path, SHARDS[i % SHARDS.len()], 1.0);
        }
    }
}
