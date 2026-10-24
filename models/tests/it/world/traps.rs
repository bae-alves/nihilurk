//! The trap table: what each effect does to a player or a monster that
//! springs it, the reveal styles that bring a hidden one to light, the odds a
//! sprung trap breaks, and the holds (bear trap, sleeping gas) that block
//! movement without blocking a swing at whatever is already in reach.

use crate::common;

use bevy_ecs::prelude::*;
use bevy_ecs::schedule::Schedule;
use models::constants::traps::TRAP_BREAK_CHANCE;
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
fn a_fall_gives_no_level_feeling() {
    let mut felt = 0;
    for seed in 1..=40 {
        let mut w = test_world(seed);
        clear_traps(&mut w);
        let here = player_pos(&mut w);
        spawn_named(&mut w, "trapdoor", here).unwrap();
        step_player_onto(&mut w, here.x, here.y);
        trap_system(&mut w);
        assert_eq!(w.resource::<Depth>().what, 2);

        let f = level_feeling(&mut w);
        felt += (f.danger.is_some() || f.riches.is_some()) as u32;
        for line in [
            strings::feeling_safe(),
            strings::feeling_dangerous(),
            strings::feeling_poor(),
            strings::feeling_rich(),
        ] {
            assert!(!log_contains(&w, line), "seed {seed}: a fall said {line}");
        }
    }
    assert!(felt > 0, "no floor in forty had a feeling to leave out");
}

#[test]
fn trapdoor_on_the_deepest_floor_only_fizzles() {
    let mut w = test_world(7);
    w.resource_mut::<Depth>().what = models::FINAL_DEPTH;
    clear_traps(&mut w);

    let here = player_pos(&mut w);
    spawn_named(&mut w, "trapdoor", here).unwrap();
    step_player_onto(&mut w, here.x, here.y);
    trap_system(&mut w);

    assert_eq!(w.resource::<Depth>().what, models::FINAL_DEPTH);
    assert!(log_contains(&w, "solid rock"));
}

// ---------------------------------------------------------------------------
// Bear trap / sleeping gas — the two "lose your turns" traps
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// Wear and tear
// ---------------------------------------------------------------------------

/// A trap that is not single-shot still gives out about
/// [`TRAP_BREAK_CHANCE`] of the times it springs, and says so when it does.
#[test]
fn a_sprung_trap_sometimes_breaks() {
    const RUNS: u64 = 120;
    let mut broke = 0u64;

    for seed in 0..RUNS {
        let mut w = test_world(seed);
        clear_traps(&mut w);
        let here = player_pos(&mut w);
        spawn_named(&mut w, "sleeping gas trap", here).unwrap();
        step_player_onto(&mut w, here.x, here.y);

        trap_system(&mut w);

        let gone = w.query_filtered::<(), With<Trap>>().iter(&w).count() == 0;
        assert_eq!(
            gone,
            log_contains(&w, "the sleeping gas trap breaks!"),
            "a trap that broke should say so, and one that held should not"
        );
        broke += u64::from(gone);
    }

    let expected = TRAP_BREAK_CHANCE * RUNS as f64;
    assert!(
        (broke as f64 - expected).abs() < expected / 2.0,
        "{broke} of {RUNS} broke, expected around {expected}"
    );
}

#[test]
fn a_bear_trap_blocks_your_feet_not_your_fists() {
    let mut w = test_world(1);
    clear_traps(&mut w);
    let p = player(&mut w);
    w.get_mut::<Fighter>(p).unwrap().hp = 12;

    let here = player_pos(&mut w);
    spawn_named(&mut w, "bear trap", here).unwrap();
    step_player_onto(&mut w, here.x, here.y);
    trap_system(&mut w);

    assert!(
        !models::player_incapacitated(&mut w),
        "a bear trap is not a sleep"
    );

    let hp_before = w.get::<Fighter>(p).unwrap().hp;
    models::bear_trap_thrash(&mut w, p);
    assert_eq!(
        w.get::<Fighter>(p).unwrap().hp,
        hp_before - 1,
        "a scratch of damage for the thrash"
    );
    assert!(log_contains(&w, "flays your leg"));
}

// ---------------------------------------------------------------------------
// Teleport
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// Arrow — the miss drops loot
// ---------------------------------------------------------------------------

/// Arrows lying on one tile.
fn arrows_on(w: &mut World, spot: Position) -> usize {
    w.query_filtered::<(&Position, &Name), With<Item>>()
        .iter(w)
        .filter(|(pos, n)| n.what == "arrow" && (pos.x, pos.y) == (spot.x, spot.y))
        .count()
}

#[test]
fn a_missed_arrow_lands_on_the_floor_as_loot() {
    let mut w = test_world(2);
    clear_traps(&mut w);
    let p = player(&mut w);
    w.get_mut::<Fighter>(p).unwrap().armor_bonus = 20;
    w.get_mut::<Fighter>(p).unwrap().hp = 12;

    let here = player_pos(&mut w);
    spawn_named(&mut w, "arrow trap", here).unwrap();
    step_player_onto(&mut w, here.x, here.y);
    trap_system(&mut w);

    assert_eq!(w.get::<Fighter>(p).unwrap().hp, 12, "no damage on a miss");
    assert_eq!(arrows_on(&mut w, here), 1, "a spent arrow to pick up");
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
    spawn_named(&mut w, "dart trap", here).unwrap();
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

    let ring = spawn_ring(&mut w, "ring of strength", Position { x: 0, y: 0 });
    w.entity_mut(ring).remove::<Position>();
    w.get_mut::<Equipped>(ring).unwrap().by = Some(p);
    sync_equipment_effects(&mut w, p);
    w.get_mut::<Backpack>(p).unwrap().items.push(ring);

    let here = player_pos(&mut w);
    spawn_named(&mut w, "dart trap", here).unwrap();
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
    let mut w = test_world(2);
    clear_traps(&mut w);
    let p = player(&mut w);
    w.get_mut::<Fighter>(p).unwrap().armor = 100;
    w.get_mut::<Fighter>(p).unwrap().armor_bonus = 0;
    w.get_mut::<Fighter>(p).unwrap().hp = 12;
    let here = player_pos(&mut w);
    spawn_named(&mut w, "dart trap", here).unwrap();
    step_player_onto(&mut w, here.x, here.y);
    trap_system(&mut w);
    assert!(
        w.get::<Fighter>(p).unwrap().hp < 12,
        "the armour die is ignored"
    );

    let mut w = test_world(2);
    clear_traps(&mut w);
    let p = player(&mut w);
    w.get_mut::<Fighter>(p).unwrap().armor = 0;
    w.get_mut::<Fighter>(p).unwrap().armor_bonus = 50;
    w.get_mut::<Fighter>(p).unwrap().hp = 12;
    let power_before = w.get::<Fighter>(p).unwrap().power;
    let here = player_pos(&mut w);
    spawn_named(&mut w, "dart trap", here).unwrap();
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
                spawn_named(&mut w, "arrow trap", here).unwrap();
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
fn an_adjacent_trap_stays_hidden_until_you_are_next_to_it() {
    let mut w = test_world(11);
    clear_traps(&mut w);
    let (p, (tx, ty)) = far_visible_tile(&mut w);
    let trap = spawn_named(&mut w, "trapdoor", Position { x: tx, y: ty }).unwrap();
    w.get_mut::<Trap>(trap).unwrap().reveal = TrapReveal::Adjacent;

    w.get_mut::<Viewshed>(p).unwrap().dirty = true;
    vis().run(&mut w);
    assert!(w.get::<Hidden>(trap).is_some(), "in view is not enough");

    {
        let mut pos = w.get_mut::<Position>(p).unwrap();
        pos.x = tx - 1;
        pos.y = ty;
    }
    w.get_mut::<Viewshed>(p).unwrap().dirty = true;
    vis().run(&mut w);
    assert!(w.get::<Hidden>(trap).is_none(), "revealed once adjacent");
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

    spawn_named(&mut w, "teleport trap", Position { x: 10, y: 5 }).unwrap();
    let known = spawn_named(&mut w, "arrow trap", Position { x: 12, y: 5 }).unwrap();
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

    let mut names: Vec<&str> = w2.query::<&Trap>().iter(&w2).map(|t| t.row.name).collect();
    names.sort();
    assert_eq!(names, vec!["arrow trap", "teleport trap"]);

    let revealed = w2
        .query::<(&Trap, Option<&Hidden>)>()
        .iter(&w2)
        .find(|(t, _)| t.row.name == TrapDef::lookup("arrow trap").unwrap().name)
        .map(|(t, h)| (t.revealed, h.is_some()))
        .unwrap();
    assert_eq!(revealed, (true, false), "the known arrow trap stays known");

    let reloaded = w2.query_filtered::<Entity, With<Player>>().single(&w2);
    assert!(w2.get::<Asleep>(reloaded).is_some(), "woke up on load");
    assert_eq!(
        turns_left(&w2, reloaded, Grant::of::<Asleep>()),
        Some(OUT_FOR),
        "the clock did not survive the save"
    );
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

    let trap = spawn_named(&mut w, "bear trap", site).unwrap();
    let near = [orc_at(&mut w, ring[0]), orc_at(&mut w, ring[1])];
    let clear = orc_at(&mut w, two_off);

    assert!(
        detonate_trap(&mut w, trap, None),
        "there was a trap to set off"
    );

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

    let trap = spawn_named(&mut w, "bear trap", site).unwrap();
    let orc = orc_at(&mut w, Position { x: tx - 1, y: ty });

    detonate_trap(&mut w, trap, None);

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
fn a_trick_shot_that_catches_you_asks_why() {
    let mut w = test_world(12);
    clear_traps(&mut w);
    clear_mobs(&mut w);
    let p = player(&mut w);
    w.get_mut::<Fighter>(p).unwrap().hp = 30;
    let (site, ring, _) = blast_site(&mut w);

    *w.get_mut::<Position>(p).unwrap() = ring[0];

    let trap = spawn_named(&mut w, "bear trap", site).unwrap();
    detonate_trap(&mut w, trap, None);

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
fn a_monster_that_falls_through_a_trapdoor_leaves_smoke() {
    let mut w = test_world(15);
    clear_traps(&mut w);
    clear_mobs(&mut w);
    let (site, _, _) = blast_site(&mut w);

    spawn_named(&mut w, "trapdoor", site).unwrap();
    let doomed = orc_at(&mut w, site);
    w.entity_mut(doomed).insert(EntityMoved);

    trap_system(&mut w);

    assert!(w.get_entity(doomed).is_none(), "gone through the floor");
    assert!(
        w.resource::<Smoke>().is_smoky(site.x, site.y),
        "dust where the floor used to be"
    );
}

/// A doorway is the tile every route into a room has to cross, so a trap just
/// inside one is unavoidable rather than unlucky. `place_one_trap` refuses any
/// room tile orthogonally next to a door — and must still fill the floor's trap
/// budget while doing it, rather than quietly spending its draws on refusals.
#[test]
fn no_trap_is_planted_in_a_doorway_or_the_tile_just_inside_one() {
    const FLOORS: u64 = 200;
    let mut floors_with_traps = 0;
    for seed in 0..FLOORS {
        let mut w = test_world(seed);
        let map = w.resource::<Map>().clone();
        let traps: Vec<Position> = w
            .query_filtered::<&Position, With<Trap>>()
            .iter(&w)
            .copied()
            .collect();
        floors_with_traps += usize::from(!traps.is_empty());

        for at in traps {
            for (nx, ny) in [
                (at.x.wrapping_sub(1), at.y),
                (at.x + 1, at.y),
                (at.x, at.y.wrapping_sub(1)),
                (at.x, at.y + 1),
            ] {
                assert_ne!(
                    map.tile(nx, ny),
                    TileType::Door,
                    "seed {seed}: a trap at ({}, {}) sits against the door at ({nx}, {ny})",
                    at.x,
                    at.y
                );
            }
            assert_ne!(
                map.tile(at.x, at.y),
                TileType::Door,
                "seed {seed}: a trap is standing in the doorway at ({}, {})",
                at.x,
                at.y
            );
        }
    }
    assert!(
        floors_with_traps * 4 >= FLOORS as usize * 3,
        "only {floors_with_traps}/{FLOORS} floors got any trap at all — the door rule is starving placement"
    );
}

// ---------------------------------------------------------------------------
// Animation
// ---------------------------------------------------------------------------

/// Every trap in the catalog puts something on the effect layer when it
/// springs — a trap that goes off invisibly reads as a bug in the log. Walks
/// the whole of `TRAPS`, so a new row whose flourish shows nothing fails here.
#[test]
fn every_trap_animates_when_it_springs() {
    for def in models::TRAPS {
        let mut w = test_world(7);
        clear_traps(&mut w);
        w.insert_resource(Particles::new());

        let here = player_pos(&mut w);
        w.spawn(TrapBundle::from_def(def, TrapReveal::Sight, here));
        step_player_onto(&mut w, here.x, here.y);

        trap_system(&mut w);

        assert!(
            w.resource::<Particles>().any_alive(),
            "{} sprang and left the screen blank",
            def.name
        );
    }
}

// ---------------------------------------------------------------------------
// The shooting traps shake whether or not the bolt connects
// ---------------------------------------------------------------------------

#[test]
fn a_shooting_trap_shakes_the_map_even_on_a_miss() {
    for name in ["arrow trap", "dart trap"] {
        let mut w = test_world(2);
        clear_traps(&mut w);
        w.init_resource::<Shake>();
        let p = player(&mut w);
        w.get_mut::<Fighter>(p).unwrap().armor_bonus = 20;

        let here = player_pos(&mut w);
        spawn_named(&mut w, name, here).unwrap();
        step_player_onto(&mut w, here.x, here.y);
        trap_system(&mut w);

        assert!(
            w.resource::<Shake>().active(),
            "the trap firing is a shake of its own"
        );
    }
}

// ---------------------------------------------------------------------------
// What a shot may and may not set off, and what one shot sets off next
// ---------------------------------------------------------------------------

#[test]
fn a_whole_chain_goes_off_at_once_with_its_dead() {
    let mut w = test_world(11);
    clear_traps(&mut w);
    clear_mobs(&mut w);
    w.init_resource::<Particles>();
    let (site, _, two_off) = blast_site(&mut w);
    let one_off = Position {
        x: site.x + 1,
        y: site.y,
    };
    let first = spawn_named(&mut w, "bear trap", site).unwrap();
    w.entity_mut(first).remove::<Hidden>();
    spawn_named(&mut w, "potion of healing", one_off).unwrap();
    spawn_named(&mut w, "bear trap", two_off).unwrap();
    let far = Position {
        x: two_off.x + 1,
        y: two_off.y,
    };
    let orc = orc_at(&mut w, far);
    w.get_mut::<Fighter>(orc).unwrap().hp = 1;

    detonate_at(&mut w, site, None);

    assert!(w.get_entity(orc).is_none(), "the last link reached the orc");
    let live = &w.resource::<Particles>().live;
    let opens = live
        .iter()
        .filter(|p| (p.x, p.y) == (far.x, far.y))
        .map(|p| p.delay_ms)
        .fold(f32::MAX, f32::min);
    let falls = live
        .iter()
        .filter(|p| p.frames.iter().any(|&(g, _)| g == '%'))
        .map(|p| p.delay_ms)
        .fold(f32::MAX, f32::min);
    assert!(
        opens < 100.0,
        "the last link opens at {opens} ms, queued behind the others"
    );
    assert!(
        falls < 100.0,
        "its dead fall at {falls} ms, queued behind the others"
    );
}
