//! A blow queued by `ai` lands only if attacker and target still stand
//! within reach when `combat_system` resolves it.

use crate::common;
use crate::common::monster;

use bevy_ecs::prelude::*;
use models::*;

fn hp(w: &World, e: Entity) -> i32 {
    w.get::<Fighter>(e).unwrap().hp
}

fn queue_blow(w: &mut World, attacker: Entity, target: Entity) {
    w.resource_mut::<AttackQueue>()
        .attacks
        .push(WantsToAttack { attacker, target });
}

fn hp_after_fifty_blows(dx: u16) -> i32 {
    let (mut w, player) = common::fresh_run(1);
    let at = *w.get::<Position>(player).unwrap();
    let from = Position {
        x: at.x + dx,
        y: at.y,
    };
    let mob = monster::plain_monster(&mut w, "Brute", from);
    w.get_mut::<Fighter>(mob).unwrap().power = 50;
    w.get_mut::<Fighter>(player).unwrap().hp = 1000;
    for _ in 0..50 {
        queue_blow(&mut w, mob, player);
    }
    combat_system(&mut w);
    hp(&w, player)
}

#[test]
fn a_queued_blow_lands_while_the_target_is_in_reach() {
    assert!(hp_after_fifty_blows(1) < 1000);
}

#[test]
fn a_queued_blow_is_dropped_once_the_target_is_out_of_reach() {
    assert_eq!(hp_after_fifty_blows(5), 1000);
}
