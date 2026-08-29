use bevy_ecs::prelude::Bundle;
use crossterm::style::Color;
use crate::components::*;

#[derive(Bundle)]
pub struct MonsterBundle {
    pub name: Name,
    pub mob: Mob,
    pub fighter: Fighter,
    pub glyph: Renderable,
    pub position: Position,
    pub faction: Faction,
}

impl MonsterBundle {
    /// Micro-HP design: mobs live on 1-3 HP and survive by winning the armour
    /// roll, not by having a fat health pool.
    pub fn goblin(position: Position) -> Self {
        Self {
            name: Name { what: "goblin".to_string() },
            mob: Mob {
                movement_type: MovementType::Flee,
            },
            // Power/Armor are die sizes: attacks roll 1d4, defence rolls 1d6.
            fighter: Fighter { hp: 1, max_hp: 1, power: 4, armor: 6 },
            glyph: Renderable { glyph: 'g', color: Color::Green },
            position,
            faction: Faction::Monster,
        }
    }

    pub fn orc(position: Position) -> Self {
        Self {
            name: Name { what: "orc".to_string() },
            mob: Mob {
                movement_type: MovementType::Chase,
            },
            // Attacks roll 1d6, defence rolls 1d8.
            fighter: Fighter { hp: 3, max_hp: 3, power: 6, armor: 8 },
            glyph: Renderable { glyph: 'o', color: Color::Red },
            position,
            faction: Faction::Monster,
        }
    }
}
