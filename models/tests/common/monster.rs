//! A bare-bones monster fixture for tests that need something to stand on a
//! tile and fight, without any bestiary row's behaviour or spawn rolls.

use bevy_ecs::prelude::*;
use crossterm::style::Color;
use models::*;

/// A neutral item-capable monster with no bestiary behavior or spawn rolls.
pub fn monster(world: &mut World, name: &str, position: Position) -> Entity {
    let entity = plain_monster(world, name, position);
    world.entity_mut(entity).insert(ItemUser);
    entity
}

/// A neutral monster with no item-use capability.
pub fn plain_monster(world: &mut World, name: &str, position: Position) -> Entity {
    world
        .spawn((
            Name {
                what: name.to_string(),
            },
            Mob {
                movement_type: MovementType::Chase,
            },
            Fighter {
                hp: 100,
                max_hp: 100,
                power: 1,
                max_power: 1,
                power_bonus: 0,
                armor: 0,
                armor_bonus: 0,
            },
            Renderable {
                glyph: 'M',
                color: Color::White,
            },
            position,
            Faction::Monster,
            Blood,
            Speed::new(SpeedKind::Normal),
        ))
        .id()
}
