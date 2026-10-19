//! The player's verbs other than a step: each is decided at the key (a refusal
//! says why and spends nothing), queued, and carried out by the schedule's first
//! step.

use crate::common;
use crate::common::monster;

use bevy_ecs::prelude::*;
use models::*;

fn at(x: i32, y: i32) -> Position {
    Position {
        x: x as u16,
        y: y as u16,
    }
}

/// A floor with nothing on it but the player, and open ground in a plus around
/// them.
fn quiet_room(seed: u64) -> (World, Entity, Position) {
    let (mut w, player) = common::fresh_run(seed);
    let clutter: Vec<Entity> = w
        .iter_entities()
        .filter(|e| !e.contains::<Player>() && (e.contains::<Mob>() || e.contains::<Item>()))
        .map(|e| e.id())
        .collect();
    for e in clutter {
        w.despawn(e);
    }
    let here = *w.get::<Position>(player).unwrap();
    {
        let mut map = w.resource_mut::<Map>();
        for dx in -4i32..=4 {
            for dy in -2i32..=2 {
                let (x, y) = ((here.x as i32 + dx) as u16, (here.y as i32 + dy) as u16);
                map.tiles[tile_index(x, y)] = TileType::Room;
            }
        }
    }
    (w, player, here)
}

fn resolve(w: &mut World) {
    player_action_system(w);
}

fn log_len(w: &World) -> usize {
    w.resource::<GameLog>().unread.len()
}

#[test]
fn stairs_off_the_stairs_say_so_and_queue_nothing() {
    let (mut w, _, here) = quiet_room(1);
    w.resource_mut::<Map>().tiles[tile_index(here.x, here.y)] = TileType::Room;
    let before = log_len(&w);
    assert!(!queue_stairs(&mut w, true));
    assert!(log_len(&w) > before);
    assert!(w.resource::<PlayerActionQueue>().actions.is_empty());
}

#[test]
fn stairs_down_wait_for_the_schedule_to_change_the_floor() {
    let (mut w, _, here) = quiet_room(1);
    w.resource_mut::<Map>().tiles[tile_index(here.x, here.y)] = TileType::Downstairs;
    assert!(queue_stairs(&mut w, true));
    assert_eq!(w.resource::<Depth>().what, 1);
    resolve(&mut w);
    assert_eq!(w.resource::<Depth>().what, 2);
    assert!(w.resource::<PlayerActionQueue>().actions.is_empty());
}

#[test]
fn a_dropped_item_leaves_the_pack_when_the_schedule_runs() {
    let (mut w, player, here) = quiet_room(1);
    let potion = spawn_named(&mut w, "potion of healing", here).unwrap();
    w.entity_mut(potion).remove::<Position>();
    w.get_mut::<Backpack>(player).unwrap().items.push(potion);
    assert!(queue_drop(&mut w, player, potion));
    assert!(w.get::<Backpack>(player).unwrap().items.contains(&potion));
    resolve(&mut w);
    assert!(!w.get::<Backpack>(player).unwrap().items.contains(&potion));
    assert_eq!(*w.get::<Position>(potion).unwrap(), here);
}

#[test]
fn cursed_gear_on_the_body_will_not_be_dropped() {
    let (mut w, player, here) = quiet_room(1);
    let ring = spawn_named(&mut w, "potion of healing", here).unwrap();
    w.entity_mut(ring).remove::<Position>();
    w.entity_mut(ring).insert((
        Curse,
        Equipped {
            by: Some(player),
            slot: Slot::Hand,
        },
    ));
    w.get_mut::<Backpack>(player).unwrap().items.push(ring);
    let before = log_len(&w);
    assert!(!queue_drop(&mut w, player, ring));
    assert!(log_len(&w) > before);
    assert!(w.resource::<PlayerActionQueue>().actions.is_empty());
    assert!(w.get::<Backpack>(player).unwrap().items.contains(&ring));
}

#[test]
fn willed_teleport_needs_the_gift_and_the_magic() {
    let (mut w, player, _) = quiet_room(1);
    assert!(!queue_willed_teleport(&mut w));
    w.entity_mut(player).insert(Teleportitis);
    w.entity_mut(player).insert(Magic {
        points: 0,
        max_points: 4,
    });
    assert!(!queue_willed_teleport(&mut w));
    assert!(w.resource::<PlayerActionQueue>().actions.is_empty());
}

#[test]
fn willed_teleport_spends_the_magic_when_the_schedule_runs() {
    let (mut w, player, here) = quiet_room(1);
    w.entity_mut(player).insert(Teleportitis);
    w.entity_mut(player).insert(Magic {
        points: 4,
        max_points: 4,
    });
    assert!(queue_willed_teleport(&mut w));
    assert_eq!(w.get::<Magic>(player).unwrap().points, 4);
    assert_eq!(*w.get::<Position>(player).unwrap(), here);
    resolve(&mut w);
    assert!(w.get::<Magic>(player).unwrap().points < 4);
}

#[test]
fn a_charge_closes_on_the_foe_when_the_schedule_runs() {
    let (mut w, player, here) = quiet_room(1);
    let foe = monster::plain_monster(&mut w, "foe", at(here.x as i32 + 3, here.y as i32));
    assert!(queue_charge(&mut w, foe));
    assert_eq!(*w.get::<Position>(player).unwrap(), here);
    resolve(&mut w);
    assert_eq!(
        *w.get::<Position>(player).unwrap(),
        at(here.x as i32 + 2, here.y as i32)
    );
    assert!(w.get::<EntityMoved>(player).is_some());
}

#[test]
fn a_charge_through_a_wall_is_refused() {
    let (mut w, _, here) = quiet_room(1);
    let foe = monster::plain_monster(&mut w, "foe", at(here.x as i32 + 3, here.y as i32));
    w.resource_mut::<Map>().tiles[tile_index(here.x + 1, here.y)] = TileType::Wall;
    assert!(!queue_charge(&mut w, foe));
    assert!(w.resource::<PlayerActionQueue>().actions.is_empty());
}

#[test]
fn a_reach_attack_strikes_when_the_schedule_runs() {
    let (mut w, player, here) = quiet_room(1);
    let weapon = spawn_named(&mut w, "potion of healing", here).unwrap();
    w.entity_mut(weapon).remove::<Position>();
    w.entity_mut(weapon).insert(Reach(2));
    let foe_at = at(here.x as i32 + 2, here.y as i32);
    let foe = monster::plain_monster(&mut w, "foe", foe_at);
    let hp = w.get::<Fighter>(foe).unwrap().hp;
    let before = log_len(&w);
    queue_reach_attack(&mut w, weapon, foe_at);
    assert_eq!(log_len(&w), before);
    resolve(&mut w);
    assert!(log_len(&w) > before || w.get::<Fighter>(foe).is_none_or(|f| f.hp < hp));
    let _ = player;
}

/// Puts a named weapon in the pack, wielded when `wield` is set.
fn carry_weapon(w: &mut World, player: Entity, here: Position, name: &str, wield: bool) -> Entity {
    let item = spawn_named(w, name, here).unwrap();
    w.entity_mut(item).remove::<Position>();
    w.entity_mut(item).insert(KnownQuality);
    w.get_mut::<Backpack>(player).unwrap().items.push(item);
    if wield {
        assert!(toggle_equipped(w, player, item));
    }
    item
}

#[test]
fn swapping_wields_the_next_weapon_and_wraps() {
    let (mut w, player, here) = quiet_room(1);
    let dagger = carry_weapon(&mut w, player, here, "dagger", true);
    let bow = carry_weapon(&mut w, player, here, "short bow", false);

    assert!(queue_swap_weapon(&mut w, player));
    resolve(&mut w);
    assert_eq!(equipped_in(&w, player, Slot::Hand), Some(bow));

    assert!(queue_swap_weapon(&mut w, player));
    resolve(&mut w);
    assert_eq!(equipped_in(&w, player, Slot::Hand), Some(dagger));
}

#[test]
fn swapping_with_empty_hands_wields_the_first_weapon() {
    let (mut w, player, here) = quiet_room(1);
    let dagger = carry_weapon(&mut w, player, here, "dagger", false);
    carry_weapon(&mut w, player, here, "short bow", false);
    assert!(queue_swap_weapon(&mut w, player));
    resolve(&mut w);
    assert_eq!(equipped_in(&w, player, Slot::Hand), Some(dagger));
}

#[test]
fn swapping_with_no_other_weapon_says_so_and_spends_nothing() {
    let (mut w, player, here) = quiet_room(1);
    let dagger = carry_weapon(&mut w, player, here, "dagger", true);
    let before = log_len(&w);
    assert!(!queue_swap_weapon(&mut w, player));
    assert!(log_len(&w) > before);
    assert!(w.resource::<PlayerActionQueue>().actions.is_empty());
    assert_eq!(equipped_in(&w, player, Slot::Hand), Some(dagger));
}

#[test]
fn a_cursed_weapon_blocks_the_swap() {
    let (mut w, player, here) = quiet_room(1);
    let dagger = carry_weapon(&mut w, player, here, "dagger", true);
    w.entity_mut(dagger).insert(Curse);
    carry_weapon(&mut w, player, here, "short bow", false);
    let before = log_len(&w);
    assert!(!queue_swap_weapon(&mut w, player));
    assert!(log_len(&w) > before);
    assert!(w.resource::<PlayerActionQueue>().actions.is_empty());
}

#[test]
fn swapping_skips_weapons_not_yet_identified() {
    let (mut w, player, here) = quiet_room(1);
    let dagger = carry_weapon(&mut w, player, here, "dagger", true);
    let mystery = carry_weapon(&mut w, player, here, "short bow", false);
    w.entity_mut(mystery).remove::<KnownQuality>();
    let before = log_len(&w);
    assert!(!queue_swap_weapon(&mut w, player));
    assert!(log_len(&w) > before);
    assert_eq!(equipped_in(&w, player, Slot::Hand), Some(dagger));
}

#[test]
fn swapping_skips_weapons_known_to_be_cursed() {
    let (mut w, player, here) = quiet_room(1);
    let dagger = carry_weapon(&mut w, player, here, "dagger", true);
    let doomed = carry_weapon(&mut w, player, here, "short bow", false);
    w.entity_mut(doomed).insert(Curse);
    let before = log_len(&w);
    assert!(!queue_swap_weapon(&mut w, player));
    assert!(log_len(&w) > before);
    assert!(w.resource::<PlayerActionQueue>().actions.is_empty());
    assert_eq!(equipped_in(&w, player, Slot::Hand), Some(dagger));
}

#[test]
fn swapping_never_trades_a_melee_weapon_for_another() {
    let (mut w, player, here) = quiet_room(1);
    let dagger = carry_weapon(&mut w, player, here, "dagger", true);
    let before = log_len(&w);
    let spare = spawn_named(&mut w, "dagger", here).unwrap();
    w.entity_mut(spare).remove::<Position>();
    w.entity_mut(spare).insert(KnownQuality);
    w.get_mut::<Backpack>(player).unwrap().items.push(spare);
    assert!(!queue_swap_weapon(&mut w, player));
    assert!(log_len(&w) > before);
    assert_eq!(equipped_in(&w, player, Slot::Hand), Some(dagger));
}

#[test]
fn the_starting_mace_swaps_to_the_starting_bow() {
    let (mut w, player) = common::fresh_run(1);
    assert!(queue_swap_weapon(&mut w, player));
    resolve(&mut w);
    let held = equipped_in(&w, player, Slot::Hand).unwrap();
    assert!(w.get::<Launcher>(held).is_some());
}

/// Puts a named item at the end of the pack.
fn carry(w: &mut World, player: Entity, here: Position, name: &str) -> Entity {
    let item = spawn_named(w, name, here).unwrap();
    w.entity_mut(item).remove::<Position>();
    w.get_mut::<Backpack>(player).unwrap().items.push(item);
    item
}

#[test]
fn a_thrown_arrow_leaves_the_rest_of_the_quiver_in_its_slot() {
    let (mut w, player, here) = quiet_room(1);
    w.get_mut::<Backpack>(player).unwrap().items.clear();
    carry(&mut w, player, here, "potion of healing");
    let quiver = carry(&mut w, player, here, "arrow");
    w.entity_mut(quiver).insert(Stack { count: 7 });
    let target = at(here.x as i32 + 3, here.y as i32);

    assert!(queue_throw(&mut w, player, quiver, target));
    throw_system(&mut w);

    assert_eq!(w.get::<Backpack>(player).unwrap().items[1], quiver);
    assert_eq!(w.get::<Stack>(quiver).unwrap().count, 6);
}

#[test]
fn a_queued_throw_and_use_leave_the_pack_alone_until_the_schedule_runs() {
    let (mut w, player, here) = quiet_room(1);
    w.get_mut::<Backpack>(player).unwrap().items.clear();
    let potion = carry(&mut w, player, here, "potion of healing");
    let quiver = carry(&mut w, player, here, "arrow");
    w.entity_mut(quiver).insert(Stack { count: 7 });
    let target = at(here.x as i32 + 3, here.y as i32);

    assert!(queue_throw(&mut w, player, quiver, target));
    assert!(queue_use(&mut w, player, potion, None));

    assert_eq!(w.get::<Backpack>(player).unwrap().items, vec![potion, quiver]);
    assert_eq!(w.get::<Stack>(quiver).unwrap().count, 7);
    let throw = w.resource::<ThrowQueue>().throws[0];
    assert_eq!((throw.item, throw.slot_idx), (quiver, Some(1)));
    let used = w.resource::<UseQueue>().uses[0];
    assert_eq!((used.item, used.slot_idx), (potion, Some(0)));
}

#[test]
fn cursed_gear_on_the_body_will_not_be_thrown() {
    let (mut w, player, here) = quiet_room(1);
    let dagger = carry_weapon(&mut w, player, here, "dagger", true);
    w.entity_mut(dagger).insert(Curse);
    let before = log_len(&w);
    assert!(!queue_throw(
        &mut w,
        player,
        dagger,
        at(here.x as i32 + 2, here.y as i32)
    ));
    assert!(log_len(&w) > before);
    assert!(w.resource::<ThrowQueue>().throws.is_empty());
    assert!(w.get::<Backpack>(player).unwrap().items.contains(&dagger));
}

#[test]
fn a_used_potion_is_queued_with_the_slot_it_came_from() {
    let (mut w, player, here) = quiet_room(1);
    w.get_mut::<Backpack>(player).unwrap().items.clear();
    carry(&mut w, player, here, "arrow");
    let potion = carry(&mut w, player, here, "potion of healing");

    assert!(queue_use(&mut w, player, potion, None));

    let uses = &w.resource::<UseQueue>().uses;
    assert_eq!(uses.len(), 1);
    assert_eq!(uses[0].item, potion);
    assert_eq!(uses[0].slot_idx, Some(1));
}

#[test]
fn an_arrow_will_not_be_used() {
    let (mut w, player, here) = quiet_room(1);
    let arrow = carry(&mut w, player, here, "arrow");
    let before = log_len(&w);
    assert!(!queue_use(&mut w, player, arrow, None));
    assert!(log_len(&w) > before);
    assert!(w.resource::<UseQueue>().uses.is_empty());
    assert!(w.get::<Backpack>(player).unwrap().items.contains(&arrow));
}

#[test]
fn a_spell_the_pool_cannot_cover_is_refused() {
    let (mut w, player, here) = quiet_room(1);
    w.entity_mut(player).insert(Magic {
        points: 0,
        max_points: 4,
    });
    let before = log_len(&w);
    assert!(!queue_cast(&mut w, player, SpellEffect::HasteSelf, here));
    assert!(log_len(&w) > before);
    assert!(w.resource::<SpellQueue>().spells.is_empty());
}

#[test]
fn an_affordable_spell_is_queued() {
    let (mut w, player, here) = quiet_room(1);
    w.entity_mut(player).insert(Magic {
        points: 50,
        max_points: 50,
    });
    assert!(queue_cast(&mut w, player, SpellEffect::HasteSelf, here));
    assert_eq!(w.resource::<SpellQueue>().spells.len(), 1);
    assert_eq!(w.get::<Magic>(player).unwrap().points, 50);
}

#[test]
fn a_queued_item_that_left_the_pack_before_the_schedule_is_neither_used_nor_thrown() {
    let (mut w, player, here) = quiet_room(1);
    w.get_mut::<Backpack>(player).unwrap().items.clear();
    let potion = carry(&mut w, player, here, "potion of healing");
    let dagger = carry(&mut w, player, here, "dagger");
    let target = at(here.x as i32 + 2, here.y as i32);
    assert!(queue_use(&mut w, player, potion, None));
    assert!(queue_throw(&mut w, player, dagger, target));
    w.get_mut::<Backpack>(player).unwrap().items.clear();

    item_system(&mut w);
    throw_system(&mut w);

    assert!(w.get_entity(potion).is_some(), "the potion was not drunk");
    assert!(w.get::<Position>(dagger).is_none(), "the dagger never flew");
    assert!(w.get::<Backpack>(player).unwrap().items.is_empty());
}
