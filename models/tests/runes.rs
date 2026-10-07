//! Runes: scrolls that go inert instead of crumbling, and wake again on the
//! stairs. What each one does, that it does it once per charge, and the two
//! effects only runes lend (`Protected`, `ExplodesOnDeath`).

mod common;
#[allow(dead_code)] // only the handless fixture is wanted here
#[path = "common/monster.rs"]
mod monster;

use bevy_ecs::prelude::*;
use models::*;

const NOWHERE: Position = Position { x: 0, y: 0 };

fn test_world(seed: u64) -> World {
    let mut w = World::new();
    w.insert_resource(GameRng(ChaCha12Rng::seed_from_u64(seed)));
    w.insert_resource(RngSeed(seed));
    w.init_resource::<GameLog>();
    w.init_resource::<UseQueue>();
    w.init_resource::<AttackQueue>();
    w.init_resource::<Ending>();
    w.insert_resource(PlayerName {
        what: "TESTER".into(),
    });
    initialize_world(&mut w);
    w
}

fn player(w: &mut World) -> Entity {
    w.query_filtered::<Entity, With<Player>>().single(w)
}

/// Pull `item` out of the pack, queue it, run the item system: the engine's
/// "Use" action.
fn use_item(w: &mut World, user: Entity, item: Entity) {
    let idx = w
        .get_mut::<Backpack>(user)
        .unwrap()
        .items
        .iter()
        .position(|&e| e == item);
    if let Some(i) = idx {
        w.get_mut::<Backpack>(user).unwrap().items.remove(i);
        w.resource_mut::<UseQueue>().uses.push(WantsToUse {
            user,
            item,
            target: None,
            slot_idx: Some(i),
        });
    }
    item_system(w);
}

fn stash(w: &mut World, user: Entity, item: Entity) -> Entity {
    w.entity_mut(item).remove::<Position>();
    w.get_mut::<Backpack>(user).unwrap().items.push(item);
    item
}

/// A fresh rune of `effect` in `user`'s pack.
fn pack_rune(w: &mut World, user: Entity, effect: RuneEffect) -> Entity {
    let rune = spawn_rune(w, effect, NOWHERE);
    stash(w, user, rune)
}

fn charged(w: &World, rune: Entity) -> bool {
    w.get::<Rune>(rune).unwrap().charged
}

/// Two open tiles side by side on the hero's row, east of them, nearest first.
fn two_open_east(w: &mut World, hero: Position) -> (Position, Position) {
    let map = w.resource::<Map>();
    (1..12)
        .map(|d| (hero.x + d, hero.y))
        .find(|&(x, y)| !map.blocks(x, y) && !map.blocks(x + 1, y))
        .map(|(x, y)| (Position { x, y }, Position { x: x + 1, y }))
        .expect("two open tiles east of the start")
}

fn make_tough(w: &mut World, e: Entity) {
    let mut f = w.get_mut::<Fighter>(e).unwrap();
    f.hp = 1000;
    f.max_hp = 1000;
}

// ---------------------------------------------------------------------------
// Charge: used once, inert after, back on the stairs
// ---------------------------------------------------------------------------

#[test]
fn a_charged_rune_fires_once_stays_in_its_slot_and_goes_inert() {
    let mut w = test_world(7);
    let p = player(&mut w);
    let rune = pack_rune(&mut w, p, RuneEffect::Displacement);
    let slot = w
        .get::<Backpack>(p)
        .unwrap()
        .items
        .iter()
        .position(|&e| e == rune)
        .unwrap();

    let before = *w.get::<Position>(p).unwrap();
    use_item(&mut w, p, rune);
    let moved = *w.get::<Position>(p).unwrap();
    assert_ne!((moved.x, moved.y), (before.x, before.y), "the rune fired");
    assert!(!charged(&w, rune), "and went inert");
    assert_eq!(
        w.get::<Backpack>(p)
            .unwrap()
            .items
            .iter()
            .position(|&e| e == rune),
        Some(slot),
        "a rune is not consumed: it keeps its pack letter"
    );

    use_item(&mut w, p, rune);
    let after = *w.get::<Position>(p).unwrap();
    assert_eq!(
        (after.x, after.y),
        (moved.x, moved.y),
        "an inert rune does nothing"
    );
    assert!(w.get::<Rune>(rune).is_some(), "and is still there to wake");
}

#[test]
fn the_stairs_wake_every_inert_rune_in_the_pack() {
    let mut w = test_world(7);
    let p = player(&mut w);
    let a = pack_rune(&mut w, p, RuneEffect::Ice);
    let b = pack_rune(&mut w, p, RuneEffect::Chaos);
    w.get_mut::<Rune>(a).unwrap().charged = false;
    w.get_mut::<Rune>(b).unwrap().charged = false;

    let down = w
        .resource::<Map>()
        .tiles
        .iter()
        .position(|&t| t == TileType::Downstairs)
        .unwrap();
    let mut pos = w.get_mut::<Position>(p).unwrap();
    pos.x = (down % MAP_WIDTH as usize) as u16;
    pos.y = (down / MAP_WIDTH as usize) as u16;

    assert!(change_level(&mut w, true));
    assert!(charged(&w, a));
    assert!(charged(&w, b));
}

#[test]
fn a_trapdoor_does_not_wake_a_rune() {
    let mut w = test_world(7);
    let p = player(&mut w);
    let rune = pack_rune(&mut w, p, RuneEffect::Ice);
    w.get_mut::<Rune>(rune).unwrap().charged = false;

    let pit = spawn_scroll(&mut w, ScrollEffect::Pitfall, NOWHERE);
    stash(&mut w, p, pit);
    use_item(&mut w, p, pit);

    assert_eq!(w.resource::<Depth>().what, 2, "the pitfall dropped us");
    assert!(!charged(&w, rune), "only a staircase recharges");
}

#[test]
fn the_read_menu_lists_runes() {
    let mut w = test_world(7);
    let p = player(&mut w);
    let rune = pack_rune(&mut w, p, RuneEffect::Ice);
    assert!(PackMode::Read.admits(&w, rune));
}

// ---------------------------------------------------------------------------
// Recharging
// ---------------------------------------------------------------------------

#[test]
fn recharging_adds_one_charge_to_every_wand_up_to_the_cap() {
    let mut w = test_world(7);
    let p = player(&mut w);
    let charges = [3, 5, constants::wands::WAND_CHARGES];
    let wands: Vec<Entity> = charges
        .iter()
        .map(|&c| {
            let wand = spawn_wand(&mut w, WandEffect::Light, NOWHERE);
            w.get_mut::<Battery>(wand).unwrap().charges = c;
            stash(&mut w, p, wand)
        })
        .collect();

    let rune = pack_rune(&mut w, p, RuneEffect::Recharging);
    use_item(&mut w, p, rune);

    let now: Vec<i8> = wands
        .iter()
        .map(|&e| w.get::<Battery>(e).unwrap().charges)
        .collect();
    let cap = constants::wands::WAND_CHARGES;
    assert_eq!(now, vec![4, cap, cap]);
}

// ---------------------------------------------------------------------------
// Chaos, Ice, Displacement
// ---------------------------------------------------------------------------

#[test]
fn chaos_hastes_the_reader() {
    let mut w = test_world(7);
    let p = player(&mut w);
    let rune = pack_rune(&mut w, p, RuneEffect::Chaos);
    use_item(&mut w, p, rune);
    assert_eq!(w.get::<Speed>(p).unwrap().kind, SpeedKind::Fast);
}

#[test]
fn ice_freezes_what_the_reader_can_see_and_only_that() {
    let mut w = test_world(7);
    let p = player(&mut w);
    let hero = *w.get::<Position>(p).unwrap();
    let (near, far) = two_open_east(&mut w, hero);
    let seen = monster::plain_monster(&mut w, "troll", near);
    let unseen = monster::plain_monster(&mut w, "troll", far);
    w.get_mut::<Viewshed>(p).unwrap().visible_tiles = vec![(hero.x, hero.y), (near.x, near.y)];

    let rune = pack_rune(&mut w, p, RuneEffect::Ice);
    use_item(&mut w, p, rune);

    assert!(w.get::<Fighter>(seen).unwrap().hp < 100, "it is hurt");
    assert!(w.get::<Paralyzed>(seen).is_some(), "and frozen");
    assert_eq!(w.get::<Fighter>(unseen).unwrap().hp, 100);
}

// ---------------------------------------------------------------------------
// Protection
// ---------------------------------------------------------------------------

#[test]
fn protection_stops_all_damage_for_six_turns_and_then_stops_stopping_it() {
    let mut w = test_world(7);
    let p = player(&mut w);
    make_tough(&mut w, p);
    let start = w.get::<Fighter>(p).unwrap().hp;
    let hero = *w.get::<Position>(p).unwrap();
    let (near, _) = two_open_east(&mut w, hero);
    let orc = monster::plain_monster(&mut w, "orc", near);
    w.get_mut::<Fighter>(orc).unwrap().power = 8;

    let rune = pack_rune(&mut w, p, RuneEffect::Protection);
    use_item(&mut w, p, rune);
    assert!(w.get::<Protected>(p).is_some());

    assert_eq!(apply_hit(&mut w, p, Hit::physical(9), None), 0);
    assert_eq!(
        apply_hit(
            &mut w,
            p,
            Hit {
                amount: 9,
                element: Some(Element::Fire),
                magical: true,
            },
            None
        ),
        0
    );
    for _ in 0..100 {
        resolve_attack(&mut w, orc, p);
    }
    assert_eq!(w.get::<Fighter>(p).unwrap().hp, start, "untouched");

    for _ in 0..constants::runes::PROTECTION_TURNS {
        assert!(w.get::<Protected>(p).is_some());
        tick_effects(&mut w);
    }
    assert!(w.get::<Protected>(p).is_none(), "six turns, then it ends");

    assert!(apply_hit(&mut w, p, Hit::physical(9), None) > 0);
    for _ in 0..100 {
        resolve_attack(&mut w, orc, p);
    }
    assert!(
        w.get::<Fighter>(p).unwrap().hp < start - 9,
        "blades bite again"
    );
}

// ---------------------------------------------------------------------------
// Justice
// ---------------------------------------------------------------------------

/// The hero, a marked monster `a` beside them, and a one-HP bystander `b` next
/// to `a`, out of the hero's sight so the rune never marked it. `a`'s power die
/// is what the burst rolls.
fn justice_scene(w: &mut World) -> (Entity, Entity, Entity) {
    let p = player(w);
    make_tough(w, p);
    let hero = *w.get::<Position>(p).unwrap();
    let (at_a, at_b) = two_open_east(w, hero);
    let a = monster::plain_monster(w, "orc", at_a);
    let b = monster::plain_monster(w, "bat", at_b);
    {
        let mut f = w.get_mut::<Fighter>(a).unwrap();
        f.hp = 1;
        f.max_hp = 1;
        f.power = 6;
        f.max_power = 6;
    }
    {
        let mut f = w.get_mut::<Fighter>(b).unwrap();
        f.hp = 1;
        f.max_hp = 1;
    }
    w.get_mut::<Viewshed>(p).unwrap().visible_tiles = vec![(hero.x, hero.y), (at_a.x, at_a.y)];

    let rune = pack_rune(w, p, RuneEffect::Justice);
    use_item(w, p, rune);
    (p, a, b)
}

#[test]
fn justice_marks_what_the_reader_sees_and_nothing_else() {
    let mut w = test_world(7);
    let (_, a, b) = justice_scene(&mut w);
    assert!(w.get::<ExplodesOnDeath>(a).is_some());
    assert!(w.get::<ExplodesOnDeath>(b).is_none());
}

#[test]
fn a_marked_monster_that_dies_by_anything_but_a_blade_takes_its_neighbour_with_it() {
    let mut w = test_world(7);
    let (_, a, b) = justice_scene(&mut w);

    apply_hit(&mut w, a, Hit::physical(5), None);
    reaper_system(&mut w);

    assert!(w.get_entity(a).is_none(), "it died once");
    assert!(w.get_entity(b).is_none(), "and the burst took the bat");
}

#[test]
fn a_marked_monster_cut_down_in_melee_bursts_too() {
    let mut w = test_world(7);
    let (p, a, b) = justice_scene(&mut w);

    for _ in 0..200 {
        if w.get_entity(a).is_none() {
            break;
        }
        resolve_attack(&mut w, p, a);
    }

    assert!(w.get_entity(a).is_none(), "the orc fell");
    assert!(w.get_entity(b).is_none(), "and the burst took the bat");
}

// ---------------------------------------------------------------------------
// Displacement
// ---------------------------------------------------------------------------

#[test]
fn displacement_is_a_scroll_of_teleportation() {
    let mut w = test_world(7);
    let p = player(&mut w);
    let before = *w.get::<Position>(p).unwrap();
    let rune = pack_rune(&mut w, p, RuneEffect::Displacement);
    use_item(&mut w, p, rune);
    let after = *w.get::<Position>(p).unwrap();
    assert_ne!((after.x, after.y), (before.x, before.y));
    assert!(!w.resource::<Map>().blocks(after.x, after.y));
}

// ---------------------------------------------------------------------------
// Blank
// ---------------------------------------------------------------------------

#[test]
fn a_blank_rune_does_nothing_and_says_so() {
    let mut w = test_world(7);
    let p = player(&mut w);
    let before = *w.get::<Position>(p).unwrap();
    let rune = pack_rune(&mut w, p, RuneEffect::Justice);
    w.get_mut::<Rune>(rune).unwrap().effect = RuneEffect::Blank;
    use_item(&mut w, p, rune);
    let after = *w.get::<Position>(p).unwrap();
    assert_eq!((after.x, after.y), (before.x, before.y));
    assert!(w.get::<Rune>(rune).is_some(), "a rune never crumbles");
}

// ---------------------------------------------------------------------------
// Saves
// ---------------------------------------------------------------------------

#[test]
fn a_rune_keeps_its_kind_and_its_charge_through_a_save() {
    let mut w = test_world(3);
    let p = player(&mut w);
    pack_rune(&mut w, p, RuneEffect::Justice);
    let spent = pack_rune(&mut w, p, RuneEffect::Ice);
    w.get_mut::<Rune>(spent).unwrap().charged = false;

    let file = common::SaveFile::new("runes");
    save_game(&mut w, file.path()).unwrap();

    let mut loaded = World::new();
    loaded.insert_resource(GameRng(ChaCha12Rng::seed_from_u64(3)));
    loaded.insert_resource(RngSeed(3));
    loaded.init_resource::<GameLog>();
    loaded.insert_resource(PlayerName {
        what: "TESTER".into(),
    });
    load_game(&mut loaded, file.path()).unwrap();

    let lp = player(&mut loaded);
    let pack = loaded.get::<Backpack>(lp).unwrap().items.clone();
    let runes: Vec<(RuneEffect, bool)> = pack
        .iter()
        .filter_map(|&e| loaded.get::<Rune>(e))
        .map(|r| (r.effect, r.charged))
        .collect();
    assert_eq!(
        runes,
        vec![(RuneEffect::Justice, true), (RuneEffect::Ice, false)]
    );
}
