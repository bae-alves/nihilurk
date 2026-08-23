use bevy_ecs::{entity::Entity, prelude::Bundle, world::World};
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
#[derive(Bundle)]
pub struct PotionBundle {
    pub glyph: Renderable,
    pub position: Position,
    pub item: Item,
    pub potion: Potion,
    pub consume: Consume,
}

impl PotionBundle {
    pub fn healing(position: Position) -> Self {
        Self {
            glyph: Renderable { glyph: '!', color: Color::Red },
            position,
            item: Item { name: String::from("Health Potion") },
            potion: Potion { effect: PotionEffect::Healing },
            consume: Consume,
        }
    }
    
    pub fn poison(position: Position) -> Self {
        Self {
            glyph: Renderable { glyph: '!', color: Color::Green },
            position,
            item: Item { name: String::from("Poison Potion") },
            potion: Potion { effect: PotionEffect::Poison },
            consume: Consume,
        }
    }
}

pub fn item_system(world: &mut World) {
    let mut use_queue = world.resource_mut::<UseQueue>();
    let uses = std::mem::take(&mut use_queue.uses);
    drop(use_queue);

    for item_use in uses {
        // We store the "work to be done" here
        let mut potion_effect: Option<PotionEffect> = None;
            {
                let item_entity = world.entity_mut(item_use.item);
                // Check components and extract data
                if let Some(p) = item_entity.get::<Potion>() {
                    potion_effect = Some(p.effect);
                }

                // If it is a consumable, despawn it
                if let Some(_consume) = item_entity.get::<Consume>() {
                    item_entity.despawn();
                }

                // Dispatch to specialized functions
                if let Some(eff) = potion_effect {
                    apply_potion_effect(world, item_use.user, eff);
                }
        }
    }
}

fn apply_potion_effect(world: &mut World, user: Entity, effect: PotionEffect) {
    match effect {
        PotionEffect::Healing => {
            if let Some(mut fighter) = world.get_mut::<Fighter>(user) {
                fighter.hp = std::cmp::min(fighter.hp + 10, fighter.max_hp);
                world.resource_mut::<GameLog>().add("Healing!".to_string());
            }
        }
        _ => { /* handle other potion effects */ }
    }
}