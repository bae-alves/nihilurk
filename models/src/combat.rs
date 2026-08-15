use bevy_ecs::prelude::*;
use crate::components::*;

pub fn combat_system(world: &mut World) {
    // 1. Take all pending attacks out of the queue for this frame
    let mut attack_queue = world.resource_mut::<AttackQueue>();
    let attacks = std::mem::take(&mut attack_queue.attacks);
    drop(attack_queue); // Drop the resource borrow before mutating entities

    // 2. Resolve each attack
    for attack in attacks {
        let attacker_power = world
            .get::<Fighter>(attack.attacker)
            .map(|f| f.power)
            .unwrap_or(1);

        let target_armor = world
            .get::<Fighter>(attack.target)
            .map(|f| f.armor)
            .unwrap_or(0);

        let damage = (attacker_power - target_armor).max(0);

        if let Some(mut target_fighter) = world.get_mut::<Fighter>(attack.target) {
            target_fighter.hp -= damage;
            if target_fighter.hp <= 0 {
                world.despawn(attack.target);
            }
        }
    }
}