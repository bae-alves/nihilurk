use bevy_ecs::prelude::Bundle;
use crossterm::style::Color;
use crate::components::*;

#[derive(Bundle)]
pub struct ItemBundle {
    pub glyph: Renderable,
    pub position: Position,
    pub value: Value,
    pub item: Item,
}

impl ItemBundle {
    pub fn gold_coin(position: Position) -> Self {
        Self {
            glyph: Renderable { glyph: '$', color: Color::Yellow },
            position,
            value: Value { amount: 1000 },
            item: Item {name : String::from("Gold coin")},
        }
    }

    pub fn silver_coin(position: Position) -> Self {
        Self {
            glyph: Renderable { glyph: '$', color: Color::Grey },
            position,
            value: Value { amount: 100 },
            item: Item {name : String::from("Silver coin")},
        }
    }
}