//! The lurk: the other body written to be played. Quadruped, fanged, clawed,
//! furred — it brings nothing, wears nothing but rings, and gets stronger by
//! eating rather than by shopping.

mod common;
#[path = "common/monster.rs"]
mod monster;

use bevy_ecs::prelude::*;
use models::constants::lurk;
use models::*;

fn lurk_world(seed: u64) -> World {
    let mut w = World::new();
    w.insert_resource(GameRng(ChaCha12Rng::seed_from_u64(seed)));
    w.insert_resource(RngSeed(seed));
    w.init_resource::<GameLog>();
    w.init_resource::<SpellQueue>();
    w.init_resource::<AttackQueue>();
    w.init_resource::<Ending>();
    w.insert_resource(PlayerName {
        what: "TESTER".into(),
    });
    w.insert_resource(StartingBody(Body::Lurk));
    initialize_world(&mut w);
    w
}

fn player(w: &mut World) -> Entity {
    w.query_filtered::<Entity, With<Player>>().single(w)
}

/// An open floor tile next to the player.
fn beside_player(w: &mut World) -> Position {
    let p = player(w);
    let here = *w.get::<Position>(p).unwrap();
    let map = w.resource::<Map>();
    for (dx, dy) in [(1i32, 0i32), (-1, 0), (0, 1), (0, -1)] {
        let (nx, ny) = (here.x as i32 + dx, here.y as i32 + dy);
        if nx >= 0 && ny >= 0 && !map.blocks(nx as u16, ny as u16) {
            return Position {
                x: nx as u16,
                y: ny as u16,
            };
        }
    }
    panic!("the lurk is walled in");
}

#[test]
fn the_lurk_is_its_own_creature() {
    let mut w = lurk_world(7);
    let p = player(&mut w);

    let r = w.get::<Renderable>(p).unwrap();
    assert_eq!((r.glyph, r.color), ('@', crossterm::style::Color::Magenta));

    let f = w.get::<Fighter>(p).unwrap();
    assert_eq!((f.hp, f.max_hp), (lurk::START_HP, lurk::START_HP));
    assert_eq!((f.power, f.armor), (lurk::START_POWER, lurk::START_ARMOR));
    assert_eq!((f.power_bonus, f.armor_bonus), (0, 0));

    let m = w.get::<Magic>(p).unwrap();
    assert_eq!(
        (m.points, m.max_points),
        (lurk::START_MAGIC, lurk::START_MAGIC)
    );

    // It hunts on all fours: half again as fast as anything else on the floor.
    assert_eq!(w.get::<Speed>(p).unwrap().kind, SpeedKind::Quick);

    // Claws and fangs, no kit.
    assert!(w.get::<Backpack>(p).unwrap().items.is_empty());
    assert!(equipment::equipped_items(&w, p).is_empty());

    // The three tricks it is born with, and the one spell it must pay for.
    assert!(w.get::<Lunges>(p).is_some(), "the estoc's lunge");
    assert!(w.get::<BuildsMomentum>(p).is_some(), "the rapier's rhythm");
    assert!(w.get::<Stealthy>(p).is_some(), "a hunter's quiet");
    assert!(w.get::<Fencer>(p).is_none(), "not the estoc's double time");
    let slots = &w.get::<Spellset>(p).unwrap().slots;
    assert_eq!(slots, &vec![SpellEffect::Bide]);
    assert_eq!(
        spell_cost(&w, p, SpellEffect::Bide),
        SpellDef::of(SpellEffect::Bide).cost,
        "Bide is learned, not innate: it costs magic"
    );
}

#[test]
fn quick_is_three_turns_for_every_two_monster_rounds() {
    // Two rounds bought per three turns is what 1.5x means from the floor's
    // side of the clock.
    let mut w = lurk_world(7);
    w.init_resource::<PlayerTempo>();
    let rounds: Vec<u8> = (0..6)
        .map(|_| {
            let mut t = w.resource_mut::<PlayerTempo>();
            t.quick_beat = (t.quick_beat + 1) % 3;
            u8::from(t.quick_beat != 0)
        })
        .collect();
    assert_eq!(rounds, vec![1, 1, 0, 1, 1, 0]);
    assert_eq!(SpeedKind::Quick.rate(), 3, "1.5x the Normal rate of 2");
}

#[test]
fn rings_and_nothing_else() {
    let mut w = lurk_world(7);
    let p = player(&mut w);
    let at = *w.get::<Position>(p).unwrap();

    for name in ["long sword", "ring mail", "short bow"] {
        let item = spawn_named(&mut w, name, at).unwrap();
        assert!(
            !equipment::toggle_equipped(&mut w, p, item),
            "a lurk cannot wear a {name}"
        );
    }
    let ring = spawn_named(&mut w, "ring of protection", at).unwrap();
    assert!(
        equipment::toggle_equipped(&mut w, p, ring),
        "a ring goes on a claw"
    );
}

#[test]
fn momentum_builds_on_the_lurk_itself() {
    let mut w = lurk_world(7);
    let p = player(&mut w);
    let at = beside_player(&mut w);
    let prey = monster::monster(&mut w, "test prey", at);
    w.get_mut::<Fighter>(prey).unwrap().max_hp = 99;
    w.get_mut::<Fighter>(prey).unwrap().hp = 99;

    // Nothing in hand to build it on, so it builds on the creature.
    assert!(w.get::<Momentum>(p).is_none());
    melee_attack(&mut w, p, prey);
    let built = w.get::<Momentum>(p).map_or(0, |m| m.0);
    assert!(built > 0, "a landed blow winds the next one up");

    // And letting go of the rhythm puts it back to nothing.
    equipment::reset_momentum(&mut w, p);
    assert_eq!(w.get::<Momentum>(p).map_or(0, |m| m.0), 0);
}

/// The four numbers on the status line, totalled — which is how many points
/// of growth the lurk is carrying, whichever of them it landed on.
fn stat_total(w: &mut World) -> i32 {
    let p = player(w);
    let f = w.get::<Fighter>(p).unwrap();
    let m = w.get::<Magic>(p).unwrap();
    f.max_hp + f.max_power + f.armor + i32::from(m.max_points)
}

#[test]
fn eating_grows_the_lurk_at_the_stated_odds() {
    let mut w = lurk_world(7);
    let before = stat_total(&mut w);

    const CORPSES: i32 = 400;
    for _ in 0..CORPSES {
        body::feed(&mut w);
    }
    let grown = stat_total(&mut w) - before;

    // Every growth is worth exactly one point of exactly one of the four, so
    // the total *is* the number of times it fired. A wide band: this is here
    // to catch a rate that has changed by a factor, not to pin the RNG.
    let expected = (f64::from(CORPSES) * lurk::GROWTH_CHANCE) as i32;
    assert!(
        (grown - expected).abs() < expected / 2,
        "{grown} growths in {CORPSES} kills, expected about {expected}"
    );
    assert!(
        w.resource::<GameLog>()
            .unread
            .iter()
            .any(|line| line.contains("FEAR THE WOLF!"))
    );
}

#[test]
fn one_growth_is_one_point_of_one_stat() {
    // Rolled per corpse, so feed until one lands rather than counting to a
    // tenth: what is being checked is the size of a growth, not its odds.
    let mut w = lurk_world(7);
    let before = stat_total(&mut w);
    for _ in 0..100 {
        body::feed(&mut w);
        let grown = stat_total(&mut w) - before;
        if grown > 0 {
            assert_eq!(grown, lurk::GROWTH_STEP, "one point, on one of the four");
            return;
        }
    }
    panic!("a hundred corpses and nothing at 15%");
}

#[test]
fn nihil_is_untouched_by_any_of_it() {
    let mut w = World::new();
    w.insert_resource(GameRng(ChaCha12Rng::seed_from_u64(7)));
    w.insert_resource(RngSeed(7));
    w.init_resource::<GameLog>();
    w.insert_resource(PlayerName {
        what: "TESTER".into(),
    });
    initialize_world(&mut w);
    let p = player(&mut w);

    assert_eq!(w.get::<Speed>(p).unwrap().kind, SpeedKind::Normal);
    assert!(w.get::<Lurk>(p).is_none());
    assert!(w.get::<Lunges>(p).is_none());
    assert_eq!(w.get::<Backpack>(p).unwrap().items.len(), 5);
    body::feed(&mut w);
    assert_eq!(
        w.get::<Fighter>(p).unwrap().max_hp,
        constants::player::START_HP
    );
}

#[test]
fn the_lurk_survives_a_save() {
    let mut w = lurk_world(7);
    let p = player(&mut w);
    w.get_mut::<Fighter>(p).unwrap().max_power += 1;

    let save = common::SaveFile::new("lurk");
    save_game(&mut w, save.path()).unwrap();

    let mut w2 = World::new();
    w2.insert_resource(GameRng(ChaCha12Rng::seed_from_u64(99)));
    w2.insert_resource(RngSeed(99));
    w2.init_resource::<GameLog>();
    w2.insert_resource(PlayerName {
        what: "TESTER".into(),
    });
    load_game(&mut w2, save.path()).unwrap();

    let p2 = player(&mut w2);
    assert!(w2.get::<Lurk>(p2).is_some(), "still a lurk");
    assert!(w2.get::<Lunges>(p2).is_some());
    assert!(w2.get::<Stealthy>(p2).is_some());
    assert_eq!(w2.get::<Speed>(p2).unwrap().kind, SpeedKind::Quick);
    assert_eq!(
        w2.get::<Fighter>(p2).unwrap().max_power,
        lurk::START_POWER + 1
    );
    assert_eq!(
        w2.get::<Spellset>(p2).unwrap().slots,
        vec![SpellEffect::Bide]
    );
}

#[test]
fn a_staircase_cannot_take_the_lurk_s_legs() {
    let mut w = lurk_world(7);
    let p = player(&mut w);
    let down = w
        .resource::<Map>()
        .tiles
        .iter()
        .position(|&t| t == TileType::Downstairs)
        .unwrap();
    w.get_mut::<Position>(p).unwrap().x = (down % MAP_WIDTH as usize) as u16;
    w.get_mut::<Position>(p).unwrap().y = (down / MAP_WIDTH as usize) as u16;
    assert!(change_level(&mut w, true));
    assert_eq!(w.get::<Speed>(p).unwrap().kind, SpeedKind::Quick);
    // Nor its quiet, which a staircase does take from anyone who merely
    // drank it.
    assert!(w.get::<Stealthy>(p).is_some());
    assert!(w.get::<Lunges>(p).is_some());
}

#[test]
fn cancellation_takes_the_magic_not_the_creature() {
    let mut w = lurk_world(7);
    let p = player(&mut w);
    revoke_all(&mut w, p);

    // A wand of cancellation unmakes what a creature *has*.
    assert!(w.get::<Stealthy>(p).is_none());
    assert!(w.get::<Lunges>(p).is_none());
    // It cannot unmake what a creature *is*, or there would be something
    // wearing plate armour on four legs.
    assert!(w.get::<Lurk>(p).is_some());
    let at = *w.get::<Position>(p).unwrap();
    let mail = spawn_named(&mut w, "ring mail", at).unwrap();
    assert!(!equipment::toggle_equipped(&mut w, p, mail));
    // And it is still a lurk after a save, which means the ledger entry
    // survived the sweep too.
    let save = common::SaveFile::new("cancelled-lurk");
    save_game(&mut w, save.path()).unwrap();
    let mut w2 = World::new();
    w2.insert_resource(GameRng(ChaCha12Rng::seed_from_u64(1)));
    w2.insert_resource(RngSeed(1));
    w2.init_resource::<GameLog>();
    w2.insert_resource(PlayerName {
        what: "TESTER".into(),
    });
    load_game(&mut w2, save.path()).unwrap();
    let p2 = player(&mut w2);
    assert!(w2.get::<Lurk>(p2).is_some());
}
