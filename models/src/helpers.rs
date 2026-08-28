use bevy_ecs::{entity::Entity, world::World};

use crate::map::Map;
use crate::{Fighter, Position};

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

pub fn is_wall_at(world: &mut World, pos: Position) -> bool {
    world
        .get_resource::<Map>()
        .map_or(false, |m| m.blocks(pos.x, pos.y))
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
}