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

#[derive(Bundle)]
pub struct WandBundle {
    pub glyph: Renderable,
    pub position: Position,
    pub item: Item,
    pub wand: Wand,
    pub ranged: Ranged,
    pub battery: Battery,
}

impl WandBundle {
    pub fn magic_missile(position: Position) -> Self {
        Self {
            glyph: Renderable { glyph: '/', color: Color::Cyan },
            position,
            item: Item { name: String::from("Wand of Magic Missile") },
            wand: Wand { effect: WandEffect::MagicMissile },
            ranged: Ranged { range: 6 },
            battery: Battery { charges: 5 },
        }
    }

    pub fn fireball(position: Position) -> Self {
        Self {
            glyph: Renderable { glyph: '/', color: Color::Red },
            position,
            item: Item { name: String::from("Wand of Fireball") },
            wand: Wand { effect: WandEffect::Fireball },
            ranged: Ranged { range: 8 },
            battery: Battery { charges: 3 },
        }
    }
}

fn apply_wand_effect(world: &mut World, user: Entity, target: Option<Position>, effect: WandEffect) {
    let target_pos = match target {
        Some(pos) => pos,
        None => return, // Safety catch: Wands require targets!
    };

    match effect {
        WandEffect::MagicMissile => {
            let mut log = world.resource_mut::<GameLog>();
            log.add("A brilliant cyan bolt leaps from the wand!".to_string());
            
            // TODO: Query for a Mob at target_pos and apply damage
        }
        WandEffect::Fireball => {
            let mut log = world.resource_mut::<GameLog>();
            log.add("A roaring sphere of fire erupts!".to_string());
            
            // TODO: Query for all Mobs within an area around target_pos and apply damage
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
        let mut wand_effect: Option<WandEffect> = None;
        let mut destroy_item = false;
        let mut return_to_inventory = false;

        {
            let mut item_entity = world.entity_mut(item_use.item);
            
            // Check for Potion
            if let Some(p) = item_entity.get::<Potion>() {
                potion_effect = Some(p.effect);
            }

            // Check for Wand
            if let Some(w) = item_entity.get::<Wand>() {
                wand_effect = Some(w.effect);
            }

            // Handle Wands / Battery logic
            if let Some(mut battery) = item_entity.get_mut::<Battery>() {
                battery.charges -= 1;
                if battery.charges <= 0 {
                    destroy_item = true;
                } else {
                    // Item survives! We need to put it back in the user's bag.
                    return_to_inventory = true;
                }
            }

            // Handle basic consumables
            if let Some(_consume) = item_entity.get::<Consume>() {
                destroy_item = true;
            }
        } // Drop the entity_mut borrow so we can freely use the world again

        // 1. Manage the item's physical existence
        if return_to_inventory {
            if let Some(mut backpack) = world.get_mut::<Backpack>(item_use.user) {
                if let Some(idx) = item_use.slot_idx {
                    // Put it back in its exact slot (clamp if inventory shifted somehow)
                    let insert_pos = std::cmp::min(idx, backpack.items.len());
                    backpack.items.insert(insert_pos, item_use.item);
                } else {
                    backpack.items.push(item_use.item); // Fallback
                }
            }
        }

        if destroy_item {
            let is_wand = world.get::<Wand>(item_use.item).is_some();
            let is_potion = world.get::<Potion>(item_use.item).is_some();

            let mut log = world.resource_mut::<GameLog>();
            if is_wand {
                log.add("The wand crumbles to dust!".to_string());
            } else if is_potion {
                log.add("You drink the potion.".to_string());
            } else {
                log.add("The item turns to dust!".to_string());
            }

            world.entity_mut(item_use.item).despawn();
        }

        // 2. Dispatch to specialized functions
        if let Some(eff) = potion_effect {
            apply_potion_effect(world, item_use.user, eff);
        }

        if let Some(eff) = wand_effect {
            apply_wand_effect(world, item_use.user, item_use.target, eff);
        }
    }
}