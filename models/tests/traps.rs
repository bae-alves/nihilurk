mod common;

use bevy_ecs::prelude::*;
use bevy_ecs::schedule::Schedule;
use models::*;

fn test_world(seed: u64) -> World {
    let mut w = World::new();
    w.insert_resource(GameRng(ChaCha12Rng::seed_from_u64(seed)));
    w.insert_resource(RngSeed(seed));
    w.init_resource::<GameLog>();
    w.insert_resource(PlayerName {
        what: "TESTER".into(),
    });
    initialize_world(&mut w);
    w
}

fn player(w: &mut World) -> Entity {
    w.query_filtered::<Entity, With<Player>>().single(w)
}

fn player_pos(w: &mut World) -> Position {
    let p = player(w);
    *w.get::<Position>(p).unwrap()
}

/// Clear the floor of the traps `initialize_world` scattered, so a test only
/// sees the one it plants itself — and strip the player's starting kit, so the
/// hero facing the trap is the bare, unarmoured one these tests assume.
fn clear_traps(w: &mut World) {
    let traps: Vec<Entity> = w.query_filtered::<Entity, With<Trap>>().iter(w).collect();
    for t in traps {
        w.entity_mut(t).despawn();
    }
    let p = player(w);
    let kit = std::mem::take(&mut w.get_mut::<Backpack>(p).unwrap().items);
    for item in kit {
        w.despawn(item);
    }
    sync_equipment_effects(w, p);
}

/// Drop the player onto `(x, y)` and mark them as having moved there.
fn step_player_onto(w: &mut World, x: u16, y: u16) {
    let p = player(w);
    let pos = w.get_mut::<Position>(p);
    let mut pos = pos.unwrap();
    pos.x = x;
    pos.y = y;
    w.entity_mut(p).insert(EntityMoved);
}

fn log_contains(w: &World, needle: &str) -> bool {
    w.resource::<GameLog>()
        .history
        .iter()
        .any(|l| l.to_lowercase().contains(&needle.to_lowercase()))
}

// ---------------------------------------------------------------------------
// Trapdoor
// ---------------------------------------------------------------------------

#[test]
fn trapdoor_drops_the_player_a_floor_with_no_heal() {
    let mut w = test_world(7);
    clear_traps(&mut w);
    let p = player(&mut w);
    w.get_mut::<Fighter>(p).unwrap().hp = 5;

    let here = player_pos(&mut w);
    w.spawn(TrapBundle::trapdoor(here));
    step_player_onto(&mut w, here.x, here.y);

    trap_system(&mut w);

    assert_eq!(w.resource::<Depth>().what, 2, "fell one floor");
    assert_eq!(
        w.get::<Fighter>(p).unwrap().hp,
        5,
        "a fall is not a rest — no heal"
    );
    assert!(log_contains(&w, "trapdoor"));
}

#[test]
fn trapdoor_on_the_deepest_floor_only_fizzles() {
    let mut w = test_world(7);
    w.resource_mut::<Depth>().what = models::FINAL_DEPTH;
    clear_traps(&mut w);

    let here = player_pos(&mut w);
    w.spawn(TrapBundle::trapdoor(here));
    step_player_onto(&mut w, here.x, here.y);
    trap_system(&mut w);

    assert_eq!(w.resource::<Depth>().what, models::FINAL_DEPTH);
    assert!(log_contains(&w, "solid rock"));
}

#[test]
fn a_monster_that_hits_a_trapdoor_is_gone() {
    let mut w = test_world(7);
    clear_traps(&mut w);
    let here = player_pos(&mut w);
    let spot = Position {
        x: here.x + 2,
        y: here.y,
    };
    w.spawn(TrapBundle::trapdoor(spot));
    let mob = w
        .spawn((
            Name { what: "orc".into() },
            Mob {
                movement_type: MovementType::Static,
            },
            Position {
                x: spot.x,
                y: spot.y,
            },
            Fighter {
                hp: 3,
                max_hp: 3,
                armor: 0,
                power: 1,
                max_power: 1,
                armor_bonus: 0,
                power_bonus: 0,
            },
            Faction::Monster,
            Blood,
            EntityMoved,
        ))
        .id();

    trap_system(&mut w);

    assert!(w.get_entity(mob).is_none(), "the orc dropped through");
    assert_eq!(w.resource::<Depth>().what, 1, "the player stays put");
}

// ---------------------------------------------------------------------------
// Bear trap / sleeping gas — the two "lose your turns" traps
// ---------------------------------------------------------------------------

#[test]
fn bear_trap_holds_for_three_turns_then_lets_go_and_is_spent() {
    let mut w = test_world(1);
    clear_traps(&mut w);
    let p = player(&mut w);
    let here = player_pos(&mut w);
    w.spawn(TrapBundle::bear(here));
    step_player_onto(&mut w, here.x, here.y);

    trap_system(&mut w);

    // Held, and the trap has snapped for good.
    assert!(w.get::<Pinned>(p).is_some(), "snared");
    let held_for = turns_left(&w, p, Grant::of::<Pinned>()).expect("held on a clock");
    assert_eq!(
        held_for,
        TrapDef::of(TrapEffect::Bear).snare_turns,
        "held for something other than the row's own duration"
    );
    assert_eq!(
        w.query_filtered::<(), With<Trap>>().iter(&w).count(),
        0,
        "single activation"
    );

    // The clock runs down one per turn, and the last turn is still spent held.
    for spent in 1..held_for {
        tick_effects(&mut w);
        assert_eq!(
            turns_left(&w, p, Grant::of::<Pinned>()),
            Some(held_for - spent),
            "the clock did not lose exactly one turn"
        );
    }
    tick_effects(&mut w);
    assert!(
        w.get::<Pinned>(p).is_none(),
        "still held after the clock ran out"
    );
    assert!(log_contains(&w, "free of the bear trap"));
}

#[test]
fn sleep_trap_knocks_the_player_out_for_five_turns_and_stays_armed() {
    let mut w = test_world(1);
    clear_traps(&mut w);
    let p = player(&mut w);
    let here = player_pos(&mut w);
    w.spawn(TrapBundle::sleep(here));
    step_player_onto(&mut w, here.x, here.y);

    trap_system(&mut w);

    assert!(w.get::<Asleep>(p).is_some(), "asleep");
    let held_for = turns_left(&w, p, Grant::of::<Asleep>()).expect("out on a clock");
    assert_eq!(
        held_for,
        TrapDef::of(TrapEffect::Sleep).snare_turns,
        "out for something other than the row's own duration"
    );
    assert_eq!(
        w.query_filtered::<(), With<Trap>>().iter(&w).count(),
        1,
        "gas trap is reusable"
    );

    for _ in 0..held_for {
        tick_effects(&mut w);
    }
    assert!(w.get::<Asleep>(p).is_none());
    assert!(!models::player_incapacitated(&mut w));
}

#[test]
fn a_bear_trap_blocks_your_feet_not_your_fists() {
    let mut w = test_world(1);
    clear_traps(&mut w);
    let p = player(&mut w);
    w.get_mut::<Fighter>(p).unwrap().hp = 12;

    let here = player_pos(&mut w);
    w.spawn(TrapBundle::bear(here));
    step_player_onto(&mut w, here.x, here.y);
    trap_system(&mut w);

    // Held, but not "incapacitated" — the engine still reads a key, so a swing
    // at an adjacent foe is possible.
    assert!(
        !models::player_incapacitated(&mut w),
        "a bear trap is not a sleep"
    );

    // Thrashing toward open ground, though, wastes the turn and tears the leg.
    let hp_before = w.get::<Fighter>(p).unwrap().hp;
    models::bear_trap_thrash(&mut w, p);
    assert_eq!(
        w.get::<Fighter>(p).unwrap().hp,
        hp_before - 1,
        "a scratch of damage for the thrash"
    );
    assert!(log_contains(&w, "flays your leg"));
}

#[test]
fn sleeping_gas_leaves_you_incapacitated() {
    let mut w = test_world(1);
    clear_traps(&mut w);
    let here = player_pos(&mut w);
    w.spawn(TrapBundle::sleep(here));
    step_player_onto(&mut w, here.x, here.y);
    trap_system(&mut w);

    assert!(models::player_incapacitated(&mut w), "out cold");
}

#[test]
fn a_bear_trapped_monster_still_bites_an_adjacent_foe() {
    let mut w = test_world(1);
    let p = player(&mut w);
    let here = player_pos(&mut w);
    w.init_resource::<AttackQueue>();

    let spot = Position {
        x: here.x + 1,
        y: here.y,
    };
    let mob = w
        .spawn((
            Name { what: "orc".into() },
            Mob {
                movement_type: MovementType::Chase,
            },
            spot,
            Fighter {
                hp: 3,
                max_hp: 3,
                armor: 0,
                power: 1,
                max_power: 1,
                armor_bonus: 0,
                power_bonus: 0,
            },
            Faction::Monster,
            Blood,
        ))
        .id();
    w.get_mut::<Viewshed>(p).unwrap().visible_tiles = vec![(spot.x, spot.y), (here.x, here.y)];
    hold(&mut w, mob, Grant::of::<Pinned>(), 2);

    let mut s = Schedule::default();
    s.add_systems(ai);
    s.run(&mut w);

    let bit = w
        .resource::<AttackQueue>()
        .attacks
        .iter()
        .any(|a| a.attacker == mob && a.target == p);
    assert!(bit, "the held orc can't step, but it can still lash out");
}

#[test]
fn ai_skips_a_snared_monster() {
    let mut w = test_world(1);
    let p = player(&mut w);
    let here = player_pos(&mut w);
    w.init_resource::<AttackQueue>();

    let spot = Position {
        x: here.x + 3,
        y: here.y,
    };
    let mob = w
        .spawn((
            Name { what: "orc".into() },
            Mob {
                movement_type: MovementType::Chase,
            },
            Position {
                x: spot.x,
                y: spot.y,
            },
            Fighter {
                hp: 3,
                max_hp: 3,
                armor: 0,
                power: 1,
                max_power: 1,
                armor_bonus: 0,
                power_bonus: 0,
            },
            Faction::Monster,
            Blood,
        ))
        .id();
    // The AI only acts on monsters it can see from the player's eyes.
    w.get_mut::<Viewshed>(p).unwrap().visible_tiles =
        vec![(spot.x, spot.y), (spot.x - 1, spot.y), (here.x, here.y)];

    hold(&mut w, mob, Grant::of::<Pinned>(), 2);

    let mut s = Schedule::default();
    s.add_systems(ai);
    s.run(&mut w);

    let after = *w.get::<Position>(mob).unwrap();
    assert_eq!(
        (after.x, after.y),
        (spot.x, spot.y),
        "a held monster does not chase"
    );
}

// ---------------------------------------------------------------------------
// Teleport
// ---------------------------------------------------------------------------

#[test]
fn teleport_trap_flings_the_player_elsewhere() {
    let mut w = test_world(4);
    clear_traps(&mut w);
    let p = player(&mut w);
    let here = player_pos(&mut w);
    w.spawn(TrapBundle::teleport(here));
    step_player_onto(&mut w, here.x, here.y);
    w.get_mut::<Viewshed>(p).unwrap().dirty = false;

    trap_system(&mut w);

    let now = *w.get::<Position>(p).unwrap();
    assert_ne!((now.x, now.y), (here.x, here.y), "moved");
    assert!(
        !w.resource::<Map>().blocks(now.x, now.y),
        "landed on open ground"
    );
    assert!(
        w.get::<Viewshed>(p).unwrap().dirty,
        "viewshed refresh queued"
    );
}

// ---------------------------------------------------------------------------
// Arrow — the miss drops loot
// ---------------------------------------------------------------------------

#[test]
fn arrow_trap_hits_an_unarmoured_target() {
    let mut w = test_world(2);
    clear_traps(&mut w);
    let p = player(&mut w);
    w.get_mut::<Fighter>(p).unwrap().hp = 12;
    w.get_mut::<Fighter>(p).unwrap().armor_bonus = 0;

    let here = player_pos(&mut w);
    w.spawn(TrapBundle::arrow(here));
    step_player_onto(&mut w, here.x, here.y);
    trap_system(&mut w);

    assert!(w.get::<Fighter>(p).unwrap().hp < 12, "the arrow drew blood");
    // A hit does not litter the floor with a spent arrow.
    let arrows = w
        .query_filtered::<&Name, With<Item>>()
        .iter(&w)
        .filter(|n| n.what == "arrow")
        .count();
    assert_eq!(arrows, 0, "a connecting arrow is not dropped as loot");
}

#[test]
fn a_missed_arrow_lands_on_the_floor_as_loot() {
    let mut w = test_world(2);
    clear_traps(&mut w);
    let p = player(&mut w);
    // Armour plus of 20 guarantees the bolt cannot connect.
    w.get_mut::<Fighter>(p).unwrap().armor_bonus = 20;
    w.get_mut::<Fighter>(p).unwrap().hp = 12;

    let here = player_pos(&mut w);
    w.spawn(TrapBundle::arrow(here));
    step_player_onto(&mut w, here.x, here.y);
    trap_system(&mut w);

    assert_eq!(w.get::<Fighter>(p).unwrap().hp, 12, "no damage on a miss");
    let arrows: Vec<(u16, u16)> = w
        .query_filtered::<(&Position, &Name), With<Item>>()
        .iter(&w)
        .filter(|(_, n)| n.what == "arrow")
        .map(|(p, _)| (p.x, p.y))
        .collect();
    assert_eq!(arrows, vec![(here.x, here.y)], "a spent arrow to pick up");
}

// ---------------------------------------------------------------------------
// Dart — damage plus a permanent strength bite, unless sustained
// ---------------------------------------------------------------------------

#[test]
fn dart_trap_saps_melee_power_for_good() {
    let mut w = test_world(2);
    clear_traps(&mut w);
    let p = player(&mut w);
    w.get_mut::<Fighter>(p).unwrap().armor_bonus = 0;
    w.get_mut::<Fighter>(p).unwrap().hp = 12;
    let power_before = w.get::<Fighter>(p).unwrap().power;
    let max_power_before = w.get::<Fighter>(p).unwrap().max_power;

    let here = player_pos(&mut w);
    w.spawn(TrapBundle::dart(here));
    step_player_onto(&mut w, here.x, here.y);
    trap_system(&mut w);

    let f = w.get::<Fighter>(p).unwrap();
    assert!(f.hp < 12, "the dart stings");
    assert_eq!(
        f.power,
        power_before - 1,
        "the attack die itself is drained"
    );
    assert_eq!(
        f.max_power, max_power_before,
        "max_power is the ceiling, untouched"
    );
}

#[test]
fn a_ring_of_strength_stops_the_dart_poison() {
    let mut w = test_world(2);
    clear_traps(&mut w);
    let p = player(&mut w);
    w.get_mut::<Fighter>(p).unwrap().armor_bonus = 0;
    let power_before = w.get::<Fighter>(p).unwrap().power;

    let ring = spawn_ring(&mut w, RingEffect::Strength, Position { x: 0, y: 0 });
    w.entity_mut(ring).remove::<Position>();
    w.get_mut::<Equipped>(ring).unwrap().by = Some(p);
    sync_equipment_effects(&mut w, p);
    w.get_mut::<Backpack>(p).unwrap().items.push(ring);

    let here = player_pos(&mut w);
    w.spawn(TrapBundle::dart(here));
    step_player_onto(&mut w, here.x, here.y);
    trap_system(&mut w);

    assert_eq!(
        w.get::<Fighter>(p).unwrap().power,
        power_before,
        "strength held"
    );
}

#[test]
fn damage_traps_ignore_the_armour_die_but_not_the_armour_plus() {
    // A huge armour die does nothing against a trap...
    let mut w = test_world(2);
    clear_traps(&mut w);
    let p = player(&mut w);
    w.get_mut::<Fighter>(p).unwrap().armor = 100;
    w.get_mut::<Fighter>(p).unwrap().armor_bonus = 0;
    w.get_mut::<Fighter>(p).unwrap().hp = 12;
    let here = player_pos(&mut w);
    w.spawn(TrapBundle::dart(here));
    step_player_onto(&mut w, here.x, here.y);
    trap_system(&mut w);
    assert!(
        w.get::<Fighter>(p).unwrap().hp < 12,
        "the armour die is ignored"
    );

    // ...but a big flat bonus shrugs it off entirely.
    let mut w = test_world(2);
    clear_traps(&mut w);
    let p = player(&mut w);
    w.get_mut::<Fighter>(p).unwrap().armor = 0;
    w.get_mut::<Fighter>(p).unwrap().armor_bonus = 50;
    w.get_mut::<Fighter>(p).unwrap().hp = 12;
    let power_before = w.get::<Fighter>(p).unwrap().power;
    let here = player_pos(&mut w);
    w.spawn(TrapBundle::dart(here));
    step_player_onto(&mut w, here.x, here.y);
    trap_system(&mut w);
    assert_eq!(
        w.get::<Fighter>(p).unwrap().hp,
        12,
        "armour plus absorbs it"
    );
    assert_eq!(
        w.get::<Fighter>(p).unwrap().power,
        power_before,
        "no hit, no poison"
    );
}

// ---------------------------------------------------------------------------
// Depth scaling — three tiers ending at floors 4, 8, 13
// ---------------------------------------------------------------------------

#[test]
fn the_dart_trap_drains_more_strength_the_deeper_you_are() {
    for (depth, expected_drain) in [(1u8, 1i32), (4, 1), (5, 2), (8, 2), (9, 3), (13, 3)] {
        let mut w = test_world(2);
        clear_traps(&mut w);
        w.resource_mut::<Depth>().what = depth;
        let p = player(&mut w);
        w.get_mut::<Fighter>(p).unwrap().armor_bonus = 0;
        w.get_mut::<Fighter>(p).unwrap().hp = 12;
        w.get_mut::<Fighter>(p).unwrap().power = 12;
        let power_before = w.get::<Fighter>(p).unwrap().power;

        let here = player_pos(&mut w);
        w.spawn(TrapBundle::dart(here));
        step_player_onto(&mut w, here.x, here.y);
        trap_system(&mut w);

        assert_eq!(
            power_before - w.get::<Fighter>(p).unwrap().power,
            expected_drain,
            "at depth {depth} the dart should drain {expected_drain}"
        );
    }
}

#[test]
fn the_arrow_trap_hits_harder_the_deeper_you_are() {
    // Same seeds, same unarmoured target: sum the damage a shallow arrow trap
    // deals against a deep one. The per-tier bonus should make the deep floor
    // visibly nastier.
    let total_at = |depth: u8| -> i32 {
        (0..40u64)
            .map(|seed| {
                let mut w = test_world(seed);
                clear_traps(&mut w);
                w.resource_mut::<Depth>().what = depth;
                let p = player(&mut w);
                w.get_mut::<Fighter>(p).unwrap().armor_bonus = 0;
                w.get_mut::<Fighter>(p).unwrap().hp = 100;

                let here = player_pos(&mut w);
                w.spawn(TrapBundle::arrow(here));
                step_player_onto(&mut w, here.x, here.y);
                trap_system(&mut w);
                100 - w.get::<Fighter>(p).unwrap().hp
            })
            .sum()
    };

    assert!(
        total_at(13) > total_at(1),
        "a depth-13 arrow trap should out-hit a depth-1 one"
    );
}

// ---------------------------------------------------------------------------
// Discovery styles
// ---------------------------------------------------------------------------

fn vis() -> Schedule {
    let mut s = Schedule::default();
    s.add_systems(visibility_system);
    s
}

/// A visible tile at least 2 away from the player (so "in sight" and "adjacent"
/// can be told apart), plus the player entity.
fn far_visible_tile(w: &mut World) -> (Entity, (u16, u16)) {
    let p = player(w);
    vis().run(w);
    let here = *w.get::<Position>(p).unwrap();
    let visible: Vec<(u16, u16)> = w.get::<Viewshed>(p).unwrap().visible_tiles.clone();
    let map = w.resource::<Map>().clone();
    let cheb = |a: (u16, u16), b: (u16, u16)| {
        (a.0 as i32 - b.0 as i32)
            .abs()
            .max((a.1 as i32 - b.1 as i32).abs())
    };
    let tile = visible
        .iter()
        .copied()
        .filter(|&(x, y)| {
            cheb((x, y), (here.x, here.y)) >= 2
                && !map.blocks(x, y)
                && x > 0
                && !map.blocks(x - 1, y)
        })
        .max_by_key(|&t| cheb(t, (here.x, here.y)))
        .expect("a walkable tile in view but not adjacent");
    (p, tile)
}

#[test]
fn a_sight_trap_reveals_itself_as_soon_as_it_is_in_view() {
    let mut w = test_world(11);
    clear_traps(&mut w);
    let (p, (tx, ty)) = far_visible_tile(&mut w);
    let trap = w
        .spawn(TrapBundle::trapdoor(Position { x: tx, y: ty }))
        .id();
    assert!(w.get::<Hidden>(trap).is_some());

    w.get_mut::<Viewshed>(p).unwrap().dirty = true;
    vis().run(&mut w);

    assert!(w.get::<Hidden>(trap).is_none(), "spotted on sight");
    assert!(w.get::<Trap>(trap).unwrap().revealed);
}

#[test]
fn an_adjacent_trap_stays_hidden_until_you_are_next_to_it() {
    let mut w = test_world(11);
    clear_traps(&mut w);
    let (p, (tx, ty)) = far_visible_tile(&mut w);
    let trap = w
        .spawn(TrapBundle::trapdoor(Position { x: tx, y: ty }))
        .id();
    w.get_mut::<Trap>(trap).unwrap().reveal = TrapReveal::Adjacent;

    w.get_mut::<Viewshed>(p).unwrap().dirty = true;
    vis().run(&mut w);
    assert!(w.get::<Hidden>(trap).is_some(), "in view is not enough");

    // Stand right next to it.
    {
        let mut pos = w.get_mut::<Position>(p).unwrap();
        pos.x = tx - 1;
        pos.y = ty;
    }
    w.get_mut::<Viewshed>(p).unwrap().dirty = true;
    vis().run(&mut w);
    assert!(w.get::<Hidden>(trap).is_none(), "revealed once adjacent");
}

#[test]
fn a_triggered_trap_is_invisible_until_it_goes_off() {
    let mut w = test_world(11);
    clear_traps(&mut w);
    let (p, (tx, ty)) = far_visible_tile(&mut w);
    let trap = w.spawn(TrapBundle::dart(Position { x: tx, y: ty })).id();
    w.get_mut::<Trap>(trap).unwrap().reveal = TrapReveal::Triggered;

    // Walk right up to it and stare: still nothing.
    {
        let mut pos = w.get_mut::<Position>(p).unwrap();
        pos.x = tx - 1;
        pos.y = ty;
    }
    w.get_mut::<Viewshed>(p).unwrap().dirty = true;
    vis().run(&mut w);
    assert!(w.get::<Hidden>(trap).is_some(), "no warning at all");

    // Step on it.
    step_player_onto(&mut w, tx, ty);
    trap_system(&mut w);
    assert!(w.get::<Hidden>(trap).is_none(), "known the hard way");
}

// ---------------------------------------------------------------------------
// Placement
// ---------------------------------------------------------------------------

#[test]
fn traps_scale_with_depth_and_never_exceed_ten_per_floor() {
    let mut shallow = 0usize;
    let mut deep = 0usize;
    let mut floors_with_a_trap = 0usize;

    for seed in 0..60u64 {
        let mut w = test_world(seed);
        let p = player(&mut w);

        let count = |w: &mut World| w.query_filtered::<(), With<Trap>>().iter(w).count();

        let n = count(&mut w);
        assert!(
            n <= 10,
            "trap budget stays well under ten per floor, got {n}"
        );
        shallow += n;
        if n > 0 {
            floors_with_a_trap += 1;
        }

        // Dive to depth 10 and sample there.
        while w.resource::<Depth>().what < 10 {
            let down = w
                .resource::<Map>()
                .tiles
                .iter()
                .position(|&t| t == TileType::Downstairs)
                .unwrap();
            w.get_mut::<Position>(p).unwrap().x = (down % MAP_WIDTH as usize) as u16;
            w.get_mut::<Position>(p).unwrap().y = (down / MAP_WIDTH as usize) as u16;
            assert!(change_level(&mut w, true));
        }
        let n = count(&mut w);
        assert!(
            n <= 10,
            "trap budget stays well under ten per floor, got {n}"
        );
        deep += n;
        if n > 0 {
            floors_with_a_trap += 1;
        }
    }

    assert!(floors_with_a_trap > 0, "some floor should have trapped");
    assert!(
        deep > shallow * 2,
        "deep floors ({deep}) should be far trappier than shallow ones ({shallow})"
    );
}

// ---------------------------------------------------------------------------
// Persistence
// ---------------------------------------------------------------------------

#[test]
fn traps_and_snares_survive_a_save_and_reload() {
    let mut w = test_world(5);
    clear_traps(&mut w);
    let p = player(&mut w);

    w.spawn(TrapBundle::teleport(Position { x: 10, y: 5 }));
    let known = w.spawn(TrapBundle::arrow(Position { x: 12, y: 5 })).id();
    w.entity_mut(known).remove::<Hidden>();
    w.get_mut::<Trap>(known).unwrap().revealed = true;
    const OUT_FOR: u32 = 4;
    hold(&mut w, p, Grant::of::<Asleep>(), OUT_FOR);

    let save = common::SaveFile::new("trap-roundtrip");
    let sp = save.path();
    save_game(&mut w, sp).unwrap();

    let mut w2 = World::new();
    w2.insert_resource(GameRng(ChaCha12Rng::seed_from_u64(99)));
    w2.insert_resource(RngSeed(99));
    w2.init_resource::<GameLog>();
    w2.insert_resource(PlayerName { what: "X".into() });
    load_game(&mut w2, sp).unwrap();

    let mut effects: Vec<TrapEffect> = w2.query::<&Trap>().iter(&w2).map(|t| t.effect).collect();
    effects.sort_by_key(|e| format!("{e:?}"));
    assert_eq!(effects, vec![TrapEffect::Arrow, TrapEffect::Teleport]);

    let revealed = w2
        .query::<(&Trap, Option<&Hidden>)>()
        .iter(&w2)
        .find(|(t, _)| t.effect == TrapEffect::Arrow)
        .map(|(t, h)| (t.revealed, h.is_some()))
        .unwrap();
    assert_eq!(revealed, (true, false), "the known arrow trap stays known");

    // The hold and, crucially, what is left on its clock: a `Lifetime::Turns`
    // has to survive the trip or a reload would quietly set the sleeper free.
    let reloaded = w2.query_filtered::<Entity, With<Player>>().single(&w2);
    assert!(w2.get::<Asleep>(reloaded).is_some(), "woke up on load");
    assert_eq!(
        turns_left(&w2, reloaded, Grant::of::<Asleep>()),
        Some(OUT_FOR),
        "the clock did not survive the save"
    );
}

/// A trap sprung by a monster is revealed just as if the player had found it —
/// `spring_trap` unhides and marks it known regardless of who steps on it —
/// even for a reveal style (`Adjacent`) that would otherwise keep it hidden
/// from the player alone.
#[test]
fn a_trap_a_monster_steps_on_is_revealed_even_unseen() {
    let mut w = test_world(9);
    clear_traps(&mut w);
    let here = player_pos(&mut w);
    let spot = Position {
        x: here.x + 5,
        y: here.y + 5,
    };
    let trap = w
        .spawn(TrapBundle::from_def(
            TrapDef::of(TrapEffect::Dart),
            TrapReveal::Adjacent,
            spot,
        ))
        .id();
    assert!(w.get::<Hidden>(trap).is_some(), "starts hidden");

    w.spawn((
        Name { what: "orc".into() },
        Mob {
            movement_type: MovementType::Static,
        },
        spot,
        Fighter {
            hp: 3,
            max_hp: 3,
            armor: 0,
            power: 1,
            max_power: 1,
            armor_bonus: 0,
            power_bonus: 0,
        },
        Faction::Monster,
        Blood,
        EntityMoved,
    ));

    trap_system(&mut w);

    assert!(
        w.get::<Hidden>(trap).is_none(),
        "revealed once the orc steps on it, even out of the player's sight"
    );
    assert!(w.get::<Trap>(trap).unwrap().revealed);
}

// ---------------------------------------------------------------------------
// Trick shots — a trap set off from a distance
// ---------------------------------------------------------------------------

const RING: [(i32, i32); 8] = [
    (-1, -1),
    (0, -1),
    (1, -1),
    (-1, 0),
    (1, 0),
    (-1, 1),
    (0, 1),
    (1, 1),
];

/// Clear the floor of the monsters `initialize_world` scattered, so a burst
/// test only counts the ones it stood up itself.
fn clear_mobs(w: &mut World) {
    let mobs: Vec<Entity> = w.query_filtered::<Entity, With<Mob>>().iter(w).collect();
    for m in mobs {
        w.entity_mut(m).despawn();
    }
}

/// Every open tile touching `at`.
fn open_ring(w: &World, at: Position) -> Vec<Position> {
    let map = w.resource::<Map>();
    RING.iter()
        .map(|&(dx, dy)| Position {
            x: (at.x as i32 + dx) as u16,
            y: (at.y as i32 + dy) as u16,
        })
        .filter(|p| !map.blocks(p.x, p.y))
        .collect()
}

/// Somewhere out of the player's sight with real floor around it: a tile, two
/// open neighbours to stand monsters on, and one open tile two steps off to
/// prove the burst stops where it says it does.
fn blast_site(w: &mut World) -> (Position, Vec<Position>, Position) {
    let hero = player_pos(w);
    let far_from_hero = |x: u16, y: u16| {
        (x as i32 - hero.x as i32)
            .abs()
            .max((y as i32 - hero.y as i32).abs())
            > 4
    };
    let candidates: Vec<Position> = {
        let map = w.resource::<Map>();
        (2..MAP_HEIGHT - 2)
            .flat_map(|y| (2..MAP_WIDTH - 2).map(move |x| Position { x, y }))
            .filter(|p| !map.blocks(p.x, p.y) && far_from_hero(p.x, p.y))
            .collect()
    };
    for site in candidates {
        let ring = open_ring(w, site);
        let two_off = Position {
            x: site.x + 2,
            y: site.y,
        };
        if ring.len() >= 2 && !w.resource::<Map>().blocks(two_off.x, two_off.y) {
            return (site, ring.into_iter().take(2).collect(), two_off);
        }
    }
    panic!("no open tile with room around it on this floor");
}

/// A hardy monster on `at`, tough enough to survive a burst and be measured.
fn orc_at(w: &mut World, at: Position) -> Entity {
    w.spawn((
        Name { what: "orc".into() },
        Mob {
            movement_type: MovementType::Static,
        },
        at,
        Fighter {
            hp: 30,
            max_hp: 30,
            armor: 0,
            power: 1,
            max_power: 1,
            armor_bonus: 0,
            power_bonus: 0,
        },
        Faction::Monster,
        Blood,
    ))
    .id()
}

#[test]
fn a_detonated_trap_bursts_over_its_whole_three_by_three_and_no_further() {
    let mut w = test_world(11);
    clear_traps(&mut w);
    clear_mobs(&mut w);
    let (site, ring, two_off) = blast_site(&mut w);

    let trap = w.spawn(TrapBundle::bear(site)).id();
    let near = [orc_at(&mut w, ring[0]), orc_at(&mut w, ring[1])];
    let clear = orc_at(&mut w, two_off);

    assert!(detonate_trap(&mut w, trap), "there was a trap to set off");

    let hp = |w: &World, e: Entity| w.get::<Fighter>(e).unwrap().hp;
    assert_eq!(
        hp(&w, near[0]),
        hp(&w, near[1]),
        "one roll, applied whole to everyone caught"
    );
    assert!(
        (24..=28).contains(&hp(&w, near[0])),
        "2d3 off a 30 HP orc, got {}",
        hp(&w, near[0])
    );
    assert_eq!(hp(&w, clear), 30, "two tiles off is out of the burst");
    assert!(
        w.get_entity(trap).is_none(),
        "the trap is spent by going off"
    );
    assert!(
        !log_contains(&w, "trick shot"),
        "a burst the player cannot see stays quiet"
    );
}

#[test]
fn a_trick_shot_in_sight_shouts_bam() {
    let mut w = test_world(11);
    clear_traps(&mut w);
    clear_mobs(&mut w);
    let (_, (tx, ty)) = far_visible_tile(&mut w);
    let site = Position { x: tx, y: ty };

    let trap = w.spawn(TrapBundle::bear(site)).id();
    let orc = orc_at(&mut w, Position { x: tx - 1, y: ty });

    detonate_trap(&mut w, trap);

    assert!(log_contains(&w, "BAM! Trick shot!"));
    assert!(
        w.get::<Fighter>(orc).unwrap().hp < 30,
        "caught in the burst"
    );
    assert_eq!(
        w.get::<Pinned>(orc).is_some(),
        true,
        "and the trap's own jaws close on it"
    );
}

#[test]
fn a_trap_that_is_merely_stepped_on_does_not_burst() {
    let mut w = test_world(11);
    clear_traps(&mut w);
    clear_mobs(&mut w);
    let (site, ring, _) = blast_site(&mut w);

    w.spawn(TrapBundle::bear(site));
    let bystander = orc_at(&mut w, ring[0]);
    let victim = orc_at(&mut w, site);
    w.entity_mut(victim).insert(EntityMoved);

    trap_system(&mut w);

    assert!(w.get::<Pinned>(victim).is_some(), "the jaws close on it");
    assert_eq!(
        w.get::<Fighter>(bystander).unwrap().hp,
        30,
        "a trap underfoot bites one victim, it does not go off"
    );
    assert!(!log_contains(&w, "trick shot"));
}

#[test]
fn a_trick_shot_that_catches_you_asks_why() {
    let mut w = test_world(12);
    clear_traps(&mut w);
    clear_mobs(&mut w);
    let p = player(&mut w);
    w.get_mut::<Fighter>(p).unwrap().hp = 30;
    let (site, ring, _) = blast_site(&mut w);

    // Standing right next to your own shot.
    *w.get_mut::<Position>(p).unwrap() = ring[0];

    let trap = w.spawn(TrapBundle::bear(site)).id();
    detonate_trap(&mut w, trap);

    assert!(log_contains(&w, "WHY! Trick shot!"));
    assert!(!log_contains(&w, "BAM!"));
    assert!(
        w.get::<Fighter>(p).unwrap().hp < 30,
        "your own burst does not spare you"
    );
    assert_eq!(
        w.get::<Pinned>(p).is_some(),
        true,
        "and the trap's own effect lands on you too"
    );
}

#[test]
fn a_detonated_trap_works_its_effect_on_everyone_it_catches() {
    let mut w = test_world(13);
    clear_traps(&mut w);
    clear_mobs(&mut w);
    let (site, ring, _) = blast_site(&mut w);

    let trap = w.spawn(TrapBundle::sleep(site)).id();
    let caught = [orc_at(&mut w, ring[0]), orc_at(&mut w, ring[1])];

    detonate_trap(&mut w, trap);

    for orc in caught {
        assert!(w.get::<Asleep>(orc).is_some(), "a lungful of gas each");
    }
}

#[test]
fn a_teleport_trap_leaves_smoke_where_the_victim_stood() {
    let mut w = test_world(14);
    clear_traps(&mut w);
    let here = player_pos(&mut w);
    w.spawn(TrapBundle::teleport(here));
    step_player_onto(&mut w, here.x, here.y);

    trap_system(&mut w);

    assert!(
        w.resource::<Smoke>().is_smoky(here.x, here.y),
        "a puff marks the spot they vanished from"
    );
}

#[test]
fn a_monster_that_falls_through_a_trapdoor_leaves_smoke() {
    let mut w = test_world(15);
    clear_traps(&mut w);
    clear_mobs(&mut w);
    let (site, _, _) = blast_site(&mut w);

    w.spawn(TrapBundle::trapdoor(site));
    let doomed = orc_at(&mut w, site);
    w.entity_mut(doomed).insert(EntityMoved);

    trap_system(&mut w);

    assert!(w.get_entity(doomed).is_none(), "gone through the floor");
    assert!(
        w.resource::<Smoke>().is_smoky(site.x, site.y),
        "dust where the floor used to be"
    );
}
