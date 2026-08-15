use bevy_ecs::prelude::Bundle;
use crossterm::style::Color;
use crate::components::*;

#[derive(Bundle)]
pub struct MonsterBundle {
    pub mob: Mob,
    pub fighter: Fighter,
    pub glyph: Renderable,
    pub position: Position,
    pub faction: Faction,
}

impl MonsterBundle {
    pub fn goblin(position: Position) -> Self {
        Self {
            mob: Mob {
                movement_type: MovementType::Flee,
            },
            fighter: Fighter { hp: 10, max_hp: 10, power: 2, armor: 1 },
            glyph: Renderable { glyph: 'g', color: Color::Green },
            position,
            faction: Faction::Monster,
        }
    }

    pub fn orc(position: Position) -> Self {
        Self {
            mob: Mob {
                movement_type: MovementType::Chase,
            },
            fighter: Fighter { hp: 20, max_hp: 20, power: 5, armor: 2 },
            glyph: Renderable { glyph: 'o', color: Color::Red },
            position,
            faction: Faction::Monster,
        }
    }
}