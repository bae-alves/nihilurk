use bevy_ecs::prelude::*;
use bevy_ecs::schedule::Schedule;
use models::*;

fn test_world(seed: u64) -> World {
    let mut w = World::new();
    w.insert_resource(GameRng(ChaCha12Rng::seed_from_u64(seed)));
    w.insert_resource(RngSeed(seed));
    w.init_resource::<GameLog>();
    w.insert_resource(PlayerName { what: "TESTER".into() });
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
/// sees the one it plants itself.
fn clear_traps(w: &mut World) {
    let traps: Vec<Entity> = w.query_filtered::<Entity, With<Trap>>().iter(w).collect();
    for t in traps {
        w.entity_mut(t).despawn();
    }
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
    assert_eq!(w.get::<Fighter>(p).unwrap().hp, 5, "a fall is not a rest — no heal");
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
    let spot = Position { x: here.x + 2, y: here.y };
    w.spawn(TrapBundle::trapdoor(spot));
    let mob = w
        .spawn((
            Name { what: "orc".into() },
            Mob { movement_type: MovementType::Static },
            Position { x: spot.x, y: spot.y },
            Fighter { hp: 3, max_hp: 3, armor: 0, power: 1, max_power: 1, armor_bonus: 0, power_bonus: 0 },
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
    let snare = w.get::<Snare>(p).expect("snared");
    assert_eq!(snare.turns, 3);
    assert_eq!(snare.kind, SnareKind::Bear);
    assert_eq!(w.query_filtered::<(), With<Trap>>().iter(&w).count(), 0, "single activation");

    snare_system(&mut w);
    assert_eq!(w.get::<Snare>(p).unwrap().turns, 2);
    snare_system(&mut w);
    assert_eq!(w.get::<Snare>(p).unwrap().turns, 1);
    snare_system(&mut w);
    assert!(w.get::<Snare>(p).is_none(), "free after the third turn");
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

    let snare = w.get::<Snare>(p).expect("asleep");
    assert_eq!(snare.turns, 5);
    assert_eq!(snare.kind, SnareKind::Sleep);
    assert_eq!(w.query_filtered::<(), With<Trap>>().iter(&w).count(), 1, "gas trap is reusable");

    for _ in 0..5 {
        snare_system(&mut w);
    }
    assert!(w.get::<Snare>(p).is_none());
    assert!(models::player_snare(&mut w).is_none());
}

#[test]
fn ai_skips_a_snared_monster() {
    let mut w = test_world(1);
    let p = player(&mut w);
    let here = player_pos(&mut w);
    w.init_resource::<AttackQueue>();

    let spot = Position { x: here.x + 3, y: here.y };
    let mob = w
        .spawn((
            Name { what: "orc".into() },
            Mob { movement_type: MovementType::Chase },
            Position { x: spot.x, y: spot.y },
            Fighter { hp: 3, max_hp: 3, armor: 0, power: 1, max_power: 1, armor_bonus: 0, power_bonus: 0 },
            Faction::Monster,
            Blood,
        ))
        .id();
    // The AI only acts on monsters it can see from the player's eyes.
    w.get_mut::<Viewshed>(p).unwrap().visible_tiles =
        vec![(spot.x, spot.y), (spot.x - 1, spot.y), (here.x, here.y)];

    w.entity_mut(mob).insert(Snare { turns: 2, kind: SnareKind::Bear });

    let mut s = Schedule::default();
    s.add_systems(ai);
    s.run(&mut w);

    let after = *w.get::<Position>(mob).unwrap();
    assert_eq!((after.x, after.y), (spot.x, spot.y), "a held monster does not chase");
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
    assert!(!w.resource::<Map>().blocks(now.x, now.y), "landed on open ground");
    assert!(w.get::<Viewshed>(p).unwrap().dirty, "viewshed refresh queued");
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
    // Armour plus of 20 guarantees the 1d8+2 bolt cannot connect.
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
    assert_eq!(f.power, power_before - 1, "the attack die itself is drained");
    assert_eq!(f.max_power, max_power_before, "max_power is the ceiling, untouched");
}

#[test]
fn a_ring_of_sustain_strength_stops_the_dart_poison() {
    let mut w = test_world(2);
    clear_traps(&mut w);
    let p = player(&mut w);
    w.get_mut::<Fighter>(p).unwrap().armor_bonus = 0;
    let power_before = w.get::<Fighter>(p).unwrap().power;

    let ring = w.spawn(RingBundle::new(RingEffect::SustainStrength, Position { x: 0, y: 0 })).id();
    w.entity_mut(ring).remove::<Position>();
    w.get_mut::<PutOn>(ring).unwrap().bearer = Some(p);
    w.get_mut::<Backpack>(p).unwrap().items.push(ring);

    let here = player_pos(&mut w);
    w.spawn(TrapBundle::dart(here));
    step_player_onto(&mut w, here.x, here.y);
    trap_system(&mut w);

    assert_eq!(w.get::<Fighter>(p).unwrap().power, power_before, "strength held");
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
    assert!(w.get::<Fighter>(p).unwrap().hp < 12, "the armour die is ignored");

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
    assert_eq!(w.get::<Fighter>(p).unwrap().hp, 12, "armour plus absorbs it");
    assert_eq!(w.get::<Fighter>(p).unwrap().power, power_before, "no hit, no poison");
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
        (a.0 as i32 - b.0 as i32).abs().max((a.1 as i32 - b.1 as i32).abs())
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
    let trap = w.spawn(TrapBundle::trapdoor(Position { x: tx, y: ty })).id();
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
    let trap = w.spawn(TrapBundle::trapdoor(Position { x: tx, y: ty })).id();
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
        assert!(n <= 10, "trap budget stays well under ten per floor, got {n}");
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
        assert!(n <= 10, "trap budget stays well under ten per floor, got {n}");
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
    w.entity_mut(p).insert(Snare { turns: 4, kind: SnareKind::Sleep });

    let path = std::env::temp_dir().join("roog_trap_roundtrip.sav");
    let sp = path.to_str().unwrap();
    save_game(&mut w, sp).unwrap();

    let mut w2 = World::new();
    w2.insert_resource(GameRng(ChaCha12Rng::seed_from_u64(99)));
    w2.insert_resource(RngSeed(99));
    w2.init_resource::<GameLog>();
    w2.insert_resource(PlayerName { what: "X".into() });
    load_game(&mut w2, sp).unwrap();

    let mut effects: Vec<TrapEffect> = w2
        .query::<&Trap>()
        .iter(&w2)
        .map(|t| t.effect)
        .collect();
    effects.sort_by_key(|e| format!("{e:?}"));
    assert_eq!(effects, vec![TrapEffect::Arrow, TrapEffect::Teleport]);

    let revealed = w2.query::<(&Trap, Option<&Hidden>)>().iter(&w2)
        .find(|(t, _)| t.effect == TrapEffect::Arrow)
        .map(|(t, h)| (t.revealed, h.is_some()))
        .unwrap();
    assert_eq!(revealed, (true, false), "the known arrow trap stays known");

    let snare = w2.query_filtered::<&Snare, With<Player>>().single(&w2);
    assert_eq!((snare.turns, snare.kind), (4, SnareKind::Sleep));

    let _ = std::fs::remove_file(sp);
}
