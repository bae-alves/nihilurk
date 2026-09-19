//! The active-move system: Bide's flat attack bonus (granted once, spent by
//! the next attack or lost the moment anything else happens instead), Magic
//! Ward's blanket immunity to wand-shaped harm, Sting's dart-trap formula,
//! Setup's self-triggering traps, heroic mana teaching a move up to the
//! four-slot cap, and a scroll of amnesia taking one back.

use bevy_ecs::prelude::*;
use models::constants::combat::BIDE_ATTACK_BONUS;
use models::constants::moves::MOVESET_CAP;
use models::constants::traps::DART_POWER_DRAIN_BASE;
use models::*;

fn test_world(seed: u64) -> World {
    let mut w = World::new();
    w.insert_resource(GameRng(ChaCha12Rng::seed_from_u64(seed)));
    w.insert_resource(RngSeed(seed));
    w.init_resource::<GameLog>();
    w.init_resource::<UseQueue>();
    w.init_resource::<AttackQueue>();
    w.init_resource::<Ending>();
    w.init_resource::<MoveQueue>();
    w.insert_resource(PlayerName {
        what: "TESTER".into(),
    });
    initialize_world(&mut w);
    w
}

fn player(w: &mut World) -> Entity {
    w.query_filtered::<Entity, With<Player>>().single(w)
}

/// An open floor tile next to the player, and the player's position.
fn beside_player(w: &mut World) -> (Position, Position) {
    let p = player(w);
    let here = *w.get::<Position>(p).unwrap();
    let map = w.resource::<Map>();
    for (dx, dy) in [(1i32, 0i32), (-1, 0), (0, 1), (0, -1), (1, 1), (-1, -1)] {
        let (nx, ny) = (here.x as i32 + dx, here.y as i32 + dy);
        if nx >= 0 && ny >= 0 && !map.blocks(nx as u16, ny as u16) {
            return (
                here,
                Position {
                    x: nx as u16,
                    y: ny as u16,
                },
            );
        }
    }
    panic!("player is walled in");
}

fn dummy(w: &mut World, at: Position, hp: i32) -> Entity {
    w.spawn((
        Name {
            what: "dummy".into(),
        },
        Mob {
            movement_type: MovementType::Static,
        },
        at,
        Fighter {
            hp,
            max_hp: hp,
            armor: 0,
            power: 1,
            max_power: 1,
            armor_bonus: 0,
            power_bonus: 0,
        },
        Faction::Monster,
        Blood,
        Speed::new(SpeedKind::Normal),
    ))
    .id()
}

fn cast(w: &mut World, user: Entity, effect: MoveEffect, target: Position) {
    w.resource_mut::<MoveQueue>().moves.push(WantsToMove {
        user,
        effect,
        target,
    });
    move_system(w);
}

#[test]
fn bide_adds_its_flat_bonus_to_the_very_next_attack_then_is_spent() {
    fn swing(seed: u64, bided: bool) -> i32 {
        let mut w = test_world(seed);
        let p = player(&mut w);
        let (_here, spot) = beside_player(&mut w);
        let target = dummy(&mut w, spot, 1_000);
        if bided {
            lend(&mut w, p, Grant::of::<Bided>(), Lifetime::NextAction);
        }
        melee_attack(&mut w, p, target);
        1_000 - w.get::<Fighter>(target).unwrap().hp
    }

    let seed = 99;
    let plain = swing(seed, false);
    let bided = swing(seed, true);
    assert_eq!(
        bided - plain,
        BIDE_ATTACK_BONUS,
        "Bide adds exactly its flat bonus on top of an otherwise identical roll"
    );
}

#[test]
fn bide_is_lost_unfired_by_anything_that_is_not_an_attack() {
    let mut w = test_world(1);
    let p = player(&mut w);
    lend(&mut w, p, Grant::of::<Bided>(), Lifetime::NextAction);
    assert!(w.get::<Bided>(p).is_some());

    // The same reset a rapier's built-up momentum gets the moment its
    // wielder does anything else with the turn.
    models::reset_momentum(&mut w, p);
    assert!(
        w.get::<Bided>(p).is_none(),
        "Bide is spent (uselessly) by anything but landing the next attack"
    );
}

#[test]
fn bide_survives_up_to_the_attack_that_spends_it() {
    let mut w = test_world(1);
    let p = player(&mut w);
    let (_here, spot) = beside_player(&mut w);
    let target = dummy(&mut w, spot, 1_000);
    lend(&mut w, p, Grant::of::<Bided>(), Lifetime::NextAction);

    melee_attack(&mut w, p, target);

    assert!(
        w.get::<Bided>(p).is_none(),
        "landing an attack spends Bide, hit or not"
    );
}

#[test]
fn magic_ward_blocks_every_point_of_a_wands_damage() {
    let mut w = test_world(4);
    let p = player(&mut w);
    let (_here, spot) = beside_player(&mut w);
    let target = dummy(&mut w, spot, 40);
    w.entity_mut(target).insert(MagicWard);

    let wand = spawn_wand(&mut w, WandEffect::Fire, Position { x: 0, y: 0 });
    w.entity_mut(wand).remove::<Position>();
    w.get_mut::<Battery>(wand).unwrap().charges = 9;
    let idx = w.get_mut::<Backpack>(p).unwrap().items.len();
    w.get_mut::<Backpack>(p).unwrap().items.push(wand);
    w.resource_mut::<UseQueue>().uses.push(WantsToUse {
        user: p,
        item: wand,
        target: Some(spot),
        slot_idx: Some(idx),
    });
    item_system(&mut w);

    assert_eq!(
        w.get::<Fighter>(target).unwrap().hp,
        40,
        "a warded creature takes nothing from a wand's blast"
    );
}

#[test]
fn magic_ward_is_lifted_at_the_next_staircase() {
    let mut w = test_world(5);
    let p = player(&mut w);
    w.entity_mut(p).insert(MagicWard);
    clear_player_conditions(&mut w, p);
    assert!(w.get::<MagicWard>(p).is_none());
}

#[test]
fn sting_bites_like_the_dart_trap_it_borrows_from() {
    let mut w = test_world(6);
    let p = player(&mut w);
    let (_here, spot) = beside_player(&mut w);
    let target = dummy(&mut w, spot, 40);
    w.get_mut::<Fighter>(target).unwrap().power = 5;
    w.get_mut::<Fighter>(target).unwrap().max_power = 5;
    w.get_mut::<Magic>(p).unwrap().points = 10;
    w.get_mut::<Magic>(p).unwrap().max_points = 10;

    cast(&mut w, p, MoveEffect::Sting, spot);

    assert!(
        w.get::<Fighter>(target).unwrap().hp < 40,
        "an unarmoured target always takes some venom"
    );
    assert_eq!(
        w.get::<Fighter>(target).unwrap().power,
        5 - DART_POWER_DRAIN_BASE,
        "the venom saps melee power exactly like the dart trap does at depth 1"
    );
}

#[test]
fn setup_plants_four_revealed_traps_and_trips_one_under_a_bystander() {
    let mut w = test_world(7);
    let p = player(&mut w);
    let here = *w.get::<Position>(p).unwrap();
    w.get_mut::<Magic>(p).unwrap().points = 10;
    w.get_mut::<Magic>(p).unwrap().max_points = 10;

    // Stand something on one of the four diagonals so Setup has a bystander
    // to trip over.
    let diag = Position {
        x: here.x + 1,
        y: here.y + 1,
    };
    let bystander = dummy(&mut w, diag, 40);

    // The floor's own population may already have laid traps elsewhere —
    // Setup's own are the ones that appear after casting it.
    let before: std::collections::HashSet<Entity> =
        w.query_filtered::<Entity, With<Trap>>().iter(&w).collect();

    cast(&mut w, p, MoveEffect::Setup, here);

    let traps: Vec<Entity> = w
        .query_filtered::<Entity, With<Trap>>()
        .iter(&w)
        .filter(|e| !before.contains(e))
        .collect();
    assert!(
        !traps.is_empty(),
        "Setup plants at least one trap where there's room for it"
    );
    for &trap in &traps {
        assert!(
            w.get::<Hidden>(trap).is_none(),
            "every trap Setup plants is revealed, not hidden"
        );
        assert!(w.get::<Trap>(trap).unwrap().revealed);
    }
    assert!(
        w.get::<Fighter>(bystander).unwrap().hp < 40,
        "a trap planted under a bystander trips immediately"
    );
}

#[test]
fn heroic_mana_teaches_a_move_up_to_the_four_slot_cap() {
    let mut w = test_world(8);
    let p = player(&mut w);
    assert!(w.get::<Moveset>(p).unwrap().slots.is_empty());

    for _ in 0..MOVESET_CAP {
        let (_here, spot) = beside_player(&mut w);
        let mana = spawn_named(&mut w, "heroic mana", spot).unwrap();
        pick_up(&mut w, p, mana);
    }
    assert_eq!(w.get::<Moveset>(p).unwrap().slots.len(), MOVESET_CAP);

    // A fifth is left on the floor: `would_help` refuses a full moveset.
    let (_here, spot) = beside_player(&mut w);
    let mana = spawn_named(&mut w, "heroic mana", spot).unwrap();
    assert!(!would_help(&w, p, PickupEffect::Mana));
    assert!(pick_up(&mut w, p, mana).is_none());
}

#[test]
fn amnesia_forgets_one_move_and_every_tile_seen_this_floor() {
    let mut w = test_world(9);
    let p = player(&mut w);
    w.get_mut::<Moveset>(p)
        .unwrap()
        .slots
        .push(MoveEffect::Sting);
    w.get_mut::<Moveset>(p)
        .unwrap()
        .slots
        .push(MoveEffect::Cure);
    // Seed a few "seen" tiles by hand — a fresh headless world never runs the
    // visibility system that would normally fill these in.
    w.get_mut::<Viewshed>(p).unwrap().revealed_tiles.insert(0);
    w.get_mut::<Viewshed>(p).unwrap().revealed_tiles.insert(1);
    assert!(w.get::<Viewshed>(p).unwrap().revealed_tiles.count_ones(..) > 0);

    let scroll = spawn_scroll(&mut w, ScrollEffect::Amnesia, Position { x: 0, y: 0 });
    w.entity_mut(scroll).remove::<Position>();
    let idx = w.get_mut::<Backpack>(p).unwrap().items.len();
    w.get_mut::<Backpack>(p).unwrap().items.push(scroll);
    w.resource_mut::<UseQueue>().uses.push(WantsToUse {
        user: p,
        item: scroll,
        target: None,
        slot_idx: Some(idx),
    });
    item_system(&mut w);

    assert_eq!(
        w.get::<Moveset>(p).unwrap().slots.len(),
        1,
        "amnesia takes exactly one move back"
    );
    assert_eq!(
        w.get::<Viewshed>(p).unwrap().revealed_tiles.count_ones(..),
        0,
        "every tile seen on this floor is forgotten with it"
    );
}
