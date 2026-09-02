use bevy_ecs::{entity::Entity, world::World};
use rand::Rng;

use crate::map::{BloodStains, GameRng};
use crate::effects::{equipped_total, ArmorBonus};
use crate::{Blood, Fighter, Position};

pub fn get_line(start: Position, end: Position) -> Vec<Position> {
    let mut points = Vec::new();
    
    // Converte para i32 para permitir números negativos e cálculos seguros
    let mut x0 = start.x as i32;
    let mut y0 = start.y as i32;
    let x1 = end.x as i32;
    let y1 = end.y as i32;

    let dx = (x1 - x0).abs();
    let dy = (y1 - y0).abs();
    let sx = if x0 < x1 { 1 } else { -1 };
    let sy = if y0 < y1 { 1 } else { -1 };
    let mut err = dx - dy;

    loop {
        // Converte de volta para u16 ao empurrar para o Position
        points.push(Position { 
            x: x0 as u16, 
            y: y0 as u16 
        });
        
        if x0 == x1 && y0 == y1 { 
            break; 
        }
        
        let e2 = 2 * err;
        if e2 > -dy {
            err -= dy;
            x0 += sx;
        }
        if e2 < dx {
            err += dx;
            y0 += sy;
        }
    }
    points
}

/// The defender's "armour plus": the flat `armor_bonus` on its [`Fighter`] plus
/// every [`ArmorBonus`] its equipped gear contributes. This is the *only* part
/// of a target's defence that a trap's damage is measured against — traps ignore
/// the armour die entirely.
pub fn total_armor_plus(world: &World, entity: Entity) -> i32 {
    let base = world.get::<Fighter>(entity).map(|f| f.armor_bonus).unwrap_or(0);
    base + equipped_total::<ArmorBonus>(world, entity)
}

pub fn get_entities_at_position(world: &mut World, pos: Position) -> Vec<Entity> {
    let mut query = world.query::<(Entity, &Position)>();
    query.iter(world).filter(|(_, p)| **p == pos).map(|(e, _)| e).collect()
}

// Aplica dano reduzindo a vida da entidade
pub fn apply_damage(world: &mut World, entity: Entity, amount: i32) {
    if let Some(mut fighter) = world.get_mut::<Fighter>(entity) {
        fighter.hp -= amount;
        // Aqui você também pode checar se a vida chegou a 0 para despawnar a entidade
    }
    if amount > 0 {
        spill_blood(world, entity, amount, false);
    }
}

const DIRS: [(i32, i32); 8] = [
    (-1, -1), (0, -1), (1, -1),
    (-1, 0),           (1, 0),
    (-1, 1),  (0, 1),  (1, 1),
];

/// If `entity` bleeds (has [`Blood`]), stain the tile it is standing on and,
/// depending on how hard it was hit, splatter blood onto nearby tiles. Purely
/// cosmetic; call this whenever a creature takes damage.
///
/// `damage` is the HP actually lost and `glancing` marks a chip-damage-only hit.
/// A glancing blow never splatters; otherwise both the number of droplets and
/// how far they can fly scale with the damage dealt.
pub fn spill_blood(world: &mut World, entity: Entity, damage: i32, glancing: bool) {
    if world.get::<Blood>(entity).is_none() {
        return;
    }
    let Some(&pos) = world.get::<Position>(entity) else {
        return;
    };

    // Bail before touching the RNG stream if blood is switched off.
    if !world.resource::<BloodStains>().enabled {
        return;
    }

    // Droplet count and reach both grow with the wound. A glancing blow only
    // wets the tile underfoot.
    let (droplets, max_reach) = if glancing {
        (0, 0)
    } else {
        ((damage / 4).clamp(0, 8), (1 + damage / 8).clamp(1, 4))
    };

    let splats: Vec<(i32, i32)> = {
        let mut rng = world.resource_mut::<GameRng>();
        (0..droplets)
            .map(|_| {
                let (dx, dy) = DIRS[rng.0.gen_range(0..DIRS.len())];
                let reach = rng.0.gen_range(1..=max_reach);
                (dx * reach, dy * reach)
            })
            .collect()
    };

    let mut stains = world.resource_mut::<BloodStains>();
    stains.stain(pos.x, pos.y);
    for (dx, dy) in splats {
        let sx = pos.x as i32 + dx;
        let sy = pos.y as i32 + dy;
        if sx >= 0 && sy >= 0 {
            stains.stain(sx as u16, sy as u16);
        }
    }
}