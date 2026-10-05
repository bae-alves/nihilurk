//! Helpers: a snack for a creature with no hands, a fancy of peace for one
//! with them, thrown for a coin-flip's chance at a boon companion that follows
//! you down the dungeon. You get one. A second one costs you the first.

mod common;
#[path = "common/monster.rs"]
mod monster;

use bevy_ecs::prelude::*;
use models::*;

fn test_world(seed: u64) -> World {
    let mut w = World::new();
    w.insert_resource(GameRng(ChaCha12Rng::seed_from_u64(seed)));
    w.insert_resource(RngSeed(seed));
    w.init_resource::<GameLog>();
    w.init_resource::<UseQueue>();
    w.init_resource::<ThrowQueue>();
    w.init_resource::<AttackQueue>();
    w.init_resource::<Ending>();
    w.init_resource::<PlayerTempo>();
    w.insert_resource(PlayerName {
        what: "TESTER".into(),
    });
    initialize_world(&mut w);
    // Only what each test plants: the floor's own monsters would wander into
    // the Helper's view and change what it chooses to do.
    let strays: Vec<Entity> = w
        .query_filtered::<Entity, (With<Mob>, Without<Player>)>()
        .iter(&w)
        .collect();
    for e in strays {
        w.despawn(e);
    }
    w
}

fn player(w: &mut World) -> Entity {
    w.query_filtered::<Entity, With<Player>>().single(w)
}

fn player_pos(w: &mut World) -> Position {
    let p = player(w);
    *w.get::<Position>(p).unwrap()
}

fn run_visibility(w: &mut World) {
    let mut s = Schedule::default();
    s.add_systems(visibility_system);
    s.run(w);
}

fn helpers(w: &mut World) -> Vec<Entity> {
    w.query_filtered::<Entity, With<Helper>>().iter(w).collect()
}

fn logged(w: &World, needle: &str) -> bool {
    w.resource::<GameLog>()
        .history
        .iter()
        .any(|l| l.contains(needle))
}

/// A walkable tile `dx` east of the player. The start room is wide enough on
/// every seed used here; the assert says so if one isn't.
fn east_of_player(w: &mut World, dx: u16) -> Position {
    let p = player_pos(w);
    let at = Position {
        x: p.x + dx,
        y: p.y,
    };
    assert!(
        w.resource::<Map>().walkable(at.x, at.y, false),
        "fixture tile {dx} east of the player is not floor"
    );
    at
}

/// Put a treat in the player's pack and throw it at `target`, the way the
/// engine's Throw action does.
fn throw_treat(w: &mut World, name: &str, target: Position) -> Entity {
    let p = player(w);
    let treat = spawn_named(w, name, Position { x: 0, y: 0 }).expect("a treat row");
    w.entity_mut(treat).remove::<Position>();
    let missile = draw_one(w, p, treat, None);
    w.resource_mut::<ThrowQueue>().throws.push(WantsToThrow {
        thrower: p,
        item: missile,
        target,
    });
    throw_system(w);
    missile
}

fn is_helper(w: &World, e: Entity) -> bool {
    w.get::<Helper>(e).is_some() && w.get::<Faction>(e) == Some(&Faction::Ally)
}

#[test]
fn a_snack_recruits_a_creature_without_hands_about_half_the_time() {
    let (mut took, mut refused) = (0, 0);
    for seed in 0..30 {
        let mut w = test_world(seed);
        let spot = east_of_player(&mut w, 1);
        let rat = monster::plain_monster(&mut w, "rat", spot);
        let snack = throw_treat(&mut w, "snack", spot);
        assert!(
            w.get_entity(snack).is_none(),
            "a snack the right creature caught is eaten, accepted or not"
        );
        match is_helper(&w, rat) {
            true => took += 1,
            false => {
                assert_eq!(w.get::<Faction>(rat), Some(&Faction::Monster));
                refused += 1;
            }
        }
    }
    assert!(took > 0 && refused > 0, "took {took}, refused {refused}");
}

#[test]
fn the_wrong_treat_bounces_off_and_can_be_picked_up_again() {
    let mut w = test_world(3);
    let spot = east_of_player(&mut w, 1);
    let orc = monster::monster(&mut w, "orc", spot);
    let snack = throw_treat(&mut w, "snack", spot);
    assert!(
        !is_helper(&w, orc),
        "a snack is no offer to something with hands"
    );
    assert_eq!(w.get::<Position>(snack).copied(), Some(spot));

    let mut w = test_world(3);
    let spot = east_of_player(&mut w, 1);
    let rat = monster::plain_monster(&mut w, "rat", spot);
    let fancy = throw_treat(&mut w, "fancy of peace", spot);
    assert!(!is_helper(&w, rat), "a fancy of peace is lost on a rat");
    assert_eq!(w.get::<Position>(fancy).copied(), Some(spot));
}

#[test]
fn a_treat_thrown_at_an_already_charmed_ally_always_takes() {
    for seed in 0..15 {
        let mut w = test_world(seed);
        let spot = east_of_player(&mut w, 1);
        let rat = monster::plain_monster(&mut w, "rat", spot);
        charm(&mut w, rat);
        assert_eq!(w.get::<Faction>(rat), Some(&Faction::Ally));
        assert!(w.get::<Helper>(rat).is_none(), "charmed, not recruited yet");

        throw_treat(&mut w, "snack", spot);
        assert!(is_helper(&w, rat), "an ally always takes the offer");
    }
}

#[test]
fn a_second_helper_costs_you_the_first() {
    let mut w = test_world(5);
    let at = east_of_player(&mut w, 1);
    let a = monster::plain_monster(&mut w, "rat", at);
    let at = east_of_player(&mut w, 2);
    let b = monster::plain_monster(&mut w, "bat", at);
    let score_before = w.query_filtered::<&Score, With<Player>>().single(&w).value;

    recruit(&mut w, a);
    recruit(&mut w, b);

    assert!(w.get_entity(a).is_none(), "the old helper is gone");
    assert_eq!(helpers(&mut w), vec![b]);
    assert!(logged(&w, strings::so_much_for_loyalty()));
    let score_after = w.query_filtered::<&Score, With<Player>>().single(&w).value;
    assert_eq!(score_before, score_after, "an exploded helper pays nothing");
}

#[test]
fn a_helper_closes_on_a_monster_in_view_and_otherwise_on_you() {
    let mut w = test_world(9);
    let at = east_of_player(&mut w, 3);
    let pal = monster::plain_monster(&mut w, "rat", at);
    recruit(&mut w, pal);
    run_visibility(&mut w);

    // Nothing to fight: it heels.
    ai(&mut w);
    let p = player_pos(&mut w);
    let at = *w.get::<Position>(pal).unwrap();
    assert_eq!(chebyshev(at, p), 2, "stepped toward the player");

    // Something to fight, right next to it: it swings rather than heels.
    let foe_at = Position {
        x: at.x + 1,
        y: at.y,
    };
    let foe = monster::plain_monster(&mut w, "kobold", foe_at);
    w.get_mut::<Mob>(foe).unwrap().movement_type = MovementType::Static;
    run_visibility(&mut w);
    w.resource_mut::<AttackQueue>().attacks.clear();
    ai(&mut w);
    let attacks = &w.resource::<AttackQueue>().attacks;
    assert!(
        attacks.iter().any(|a| a.attacker == pal && a.target == foe),
        "the helper went for the monster in view"
    );
}

#[test]
fn a_monster_next_to_your_helper_but_not_you_fights_the_helper() {
    let mut w = test_world(9);
    let at = east_of_player(&mut w, 3);
    let pal = monster::plain_monster(&mut w, "rat", at);
    recruit(&mut w, pal);
    // Pinned down so it neither heels nor steps: the test is about the foe.
    w.get_mut::<Mob>(pal).unwrap().movement_type = MovementType::Static;
    w.entity_mut(pal).remove::<Speed>();
    let at = east_of_player(&mut w, 4);
    let foe = monster::plain_monster(&mut w, "kobold", at);
    run_visibility(&mut w);
    w.resource_mut::<AttackQueue>().attacks.clear();
    ai(&mut w);
    let attacks = &w.resource::<AttackQueue>().attacks;
    assert!(
        attacks.iter().any(|a| a.attacker == foe && a.target == pal),
        "the monster ignored the helper standing next to it"
    );
}

#[test]
fn a_helper_follows_you_downstairs_healed_and_still_dressed() {
    let mut w = test_world(7);
    let p = player(&mut w);
    let at = east_of_player(&mut w, 1);
    let pal = monster::monster(&mut w, "orc", at);
    let dagger = spawn_weapon(&mut w, "dagger", Position { x: 0, y: 0 });
    assert!(equip_silently(&mut w, pal, dagger));
    recruit(&mut w, pal);
    w.get_mut::<Fighter>(pal).unwrap().hp = 1;

    let down = stair_location(&w.resource::<Map>().clone(), true).unwrap();
    *w.get_mut::<Position>(p).unwrap() = Position {
        x: down.0,
        y: down.1,
    };
    assert!(change_level(&mut w, true));

    let here = *w.get::<Position>(p).unwrap();
    let there = *w.get::<Position>(pal).expect("the helper came along");
    assert_eq!(chebyshev(here, there), 1, "it arrives at your side");
    let f = w.get::<Fighter>(pal).unwrap();
    assert_eq!(f.hp, f.max_hp, "and rested");
    assert_eq!(w.get::<Equipped>(dagger).and_then(|e| e.by), Some(pal));
}

#[test]
fn a_dead_helper_pays_no_score() {
    let mut w = test_world(4);
    let at = east_of_player(&mut w, 1);
    let pal = monster::plain_monster(&mut w, "rat", at);
    recruit(&mut w, pal);
    let before = w.query_filtered::<&Score, With<Player>>().single(&w).value;
    w.get_mut::<Fighter>(pal).unwrap().hp = 0;
    reaper_system(&mut w);
    assert!(w.get_entity(pal).is_none());
    let after = w.query_filtered::<&Score, With<Player>>().single(&w).value;
    assert_eq!(before, after);
}

#[test]
fn a_helper_and_a_bag_of_snacks_survive_a_save() {
    let mut w = test_world(11);
    let p = player(&mut w);
    let at = east_of_player(&mut w, 1);
    let pal = monster::plain_monster(&mut w, "rat", at);
    recruit(&mut w, pal);
    for _ in 0..3 {
        let snack = spawn_named(&mut w, "snack", Position { x: 0, y: 0 }).unwrap();
        stow(&mut w, p, snack);
    }

    let save = common::SaveFile::new("helper");
    save_game(&mut w, save.path()).unwrap();
    let mut w2 = World::new();
    w2.insert_resource(GameRng(ChaCha12Rng::seed_from_u64(12)));
    w2.insert_resource(RngSeed(12));
    w2.init_resource::<GameLog>();
    w2.insert_resource(PlayerName { what: "X".into() });
    load_game(&mut w2, save.path()).unwrap();

    let pal2 = helpers(&mut w2);
    assert_eq!(pal2.len(), 1);
    assert_eq!(w2.get::<Faction>(pal2[0]), Some(&Faction::Ally));
    let p2 = w2.query_filtered::<Entity, With<Player>>().single(&w2);
    let bag = w2
        .get::<Backpack>(p2)
        .unwrap()
        .items
        .iter()
        .copied()
        .find(|&e| w2.get::<Treat>(e).is_some())
        .expect("the snacks came back as treats");
    assert_eq!(w2.get::<Stack>(bag).map(|s| s.count), Some(3));
}

#[test]
fn a_treat_or_an_arrow_is_for_throwing() {
    let mut w = test_world(1);
    for name in ["snack", "fancy of peace", "arrow"] {
        let item = spawn_named(&mut w, name, Position { x: 0, y: 0 }).unwrap();
        assert!(use_refusal(&w, item).is_some(), "{name} is not for using");
    }
    let potion = spawn_named(&mut w, "potion of healing", Position { x: 0, y: 0 }).unwrap();
    assert!(use_refusal(&w, potion).is_none());
}

#[test]
fn a_helper_in_view_neither_halts_the_walk_nor_gets_announced() {
    let mut w = test_world(9);
    let at = east_of_player(&mut w, 2);
    let pal = monster::plain_monster(&mut w, "rat", at);
    recruit(&mut w, pal);
    w.resource_mut::<GameLog>().history.clear();
    run_visibility(&mut w);
    assert!(
        w.get::<Hidden>(pal).is_none(),
        "the fixture helper is in view"
    );
    assert!(
        !monster_in_sight(&mut w),
        "a helper is not a reason to stop"
    );
    assert!(!logged(&w, "rat"), "a helper is not news");
}

#[test]
fn a_cleave_and_a_whirl_spare_your_helper() {
    let mut w = test_world(9);
    let p = player(&mut w);
    let here = player_pos(&mut w);
    let at = east_of_player(&mut w, 1);
    let foe = monster::plain_monster(&mut w, "kobold", at);
    let beside = Position {
        x: here.x,
        y: here.y + 1,
    };
    let pal = monster::plain_monster(&mut w, "rat", beside);
    recruit(&mut w, pal);

    let axe = spawn_weapon(&mut w, "battle axe", Position { x: 0, y: 0 });
    w.entity_mut(axe).remove::<Position>();
    w.get_mut::<Backpack>(p).unwrap().items.push(axe);
    assert!(toggle_equipped(&mut w, p, axe));
    melee_attack(&mut w, p, foe);
    assert_eq!(
        w.get::<Fighter>(pal).unwrap().hp,
        100,
        "the axe swept past it"
    );

    let sickle = spawn_weapon(&mut w, "chain-sickle", Position { x: 0, y: 0 });
    w.entity_mut(sickle).remove::<Position>();
    w.get_mut::<Backpack>(p).unwrap().items.push(sickle);
    assert!(toggle_equipped(&mut w, p, sickle));
    w.despawn(foe);
    // A step east, with the helper alongside both tiles.
    let east = Position {
        x: here.x + 1,
        y: here.y,
    };
    try_whirl_attack(&mut w, p, here, east);
    assert_eq!(
        w.get::<Fighter>(pal).unwrap().hp,
        100,
        "the sickle swung past it"
    );
}

#[test]
fn a_helper_ignores_terrain_like_a_ghost() {
    let mut w = test_world(9);
    let at = east_of_player(&mut w, 3);
    let pal = monster::plain_monster(&mut w, "rat", at);
    recruit(&mut w, pal);
    // Walled in on every side, and the tile toward the player is deep water.
    {
        let mut map = w.resource_mut::<Map>();
        for dy in -1i32..=1 {
            for dx in -1i32..=1 {
                let (x, y) = ((at.x as i32 + dx) as u16, (at.y as i32 + dy) as u16);
                map.tiles[tile_index(x, y)] = TileType::Wall;
            }
        }
        map.tiles[tile_index(at.x - 1, at.y)] = TileType::Water;
        map.tiles[tile_index(at.x, at.y)] = TileType::Room;
    }
    run_visibility(&mut w);

    ai(&mut w);
    let p = player_pos(&mut w);
    let now = *w.get::<Position>(pal).unwrap();
    assert!(
        chebyshev(now, p) < 3,
        "walked out of its cell toward the player"
    );
}

// ---------------------------------------------------------------------------
// The dog: four traits, each its own effect. They are tested one at a time on
// a plain monster, because separate is the point.
// ---------------------------------------------------------------------------

use models::constants::helpers::SHAPESHIFT_CHANCE;

fn lend_forever<G: 'static + Send + Sync + Component + Default + Clone + Copy>(
    w: &mut World,
    e: Entity,
) {
    lend(w, e, Grant::of::<G>(), Lifetime::Permanent);
}

fn all_dog_traits(w: &mut World, e: Entity) {
    lend_forever::<ShapeshiftOnKill>(w, e);
    lend_forever::<AlwaysTamed>(w, e);
    lend_forever::<AlwaysHelper>(w, e);
    lend_forever::<PriorityHelper>(w, e);
}

#[test]
fn always_tamed_takes_any_treat_every_time() {
    for seed in 0..30 {
        let mut w = test_world(seed);
        let at = east_of_player(&mut w, 1);
        // Hands, so a snack is the wrong treat for it: it takes it anyway.
        let mob = monster::monster(&mut w, "orc", at);
        lend_forever::<AlwaysTamed>(&mut w, mob);
        throw_treat(&mut w, "snack", at);
        assert!(is_helper(&w, mob), "seed {seed}: it turned a snack down");
    }
}

#[test]
fn always_helper_turns_a_charm_into_the_helper() {
    let mut w = test_world(3);
    let at = east_of_player(&mut w, 1);
    let plain = monster::plain_monster(&mut w, "rat", at);
    charm(&mut w, plain);
    assert!(w.get::<Helper>(plain).is_none(), "a charm is a plain ally");

    let at = east_of_player(&mut w, 2);
    let eager = monster::plain_monster(&mut w, "pup", at);
    lend_forever::<AlwaysHelper>(&mut w, eager);
    charm(&mut w, eager);
    assert!(is_helper(&w, eager));
}

#[test]
fn a_priority_helper_never_explodes_and_still_explodes_the_other_kind() {
    let mut w = test_world(4);
    let spawn = |w: &mut World, dx: u16, name: &str, priority: bool| {
        let at = east_of_player(w, dx);
        let e = monster::plain_monster(w, name, at);
        if priority {
            lend_forever::<PriorityHelper>(w, e);
        }
        e
    };
    let (a, b) = (spawn(&mut w, 1, "a", true), spawn(&mut w, 2, "b", true));
    recruit(&mut w, a);
    recruit(&mut w, b);
    assert!(is_helper(&w, a) && is_helper(&w, b), "any number of dogs");

    let x = spawn(&mut w, 3, "x", false);
    recruit(&mut w, x);
    assert!(is_helper(&w, a) && is_helper(&w, b) && is_helper(&w, x));

    let y = spawn(&mut w, 4, "y", false);
    recruit(&mut w, y);
    assert!(
        w.get_entity(x).is_none(),
        "a second plain helper blows up the first"
    );
    assert!(is_helper(&w, a) && is_helper(&w, b) && is_helper(&w, y));

    let z = spawn(&mut w, 5, "z", true);
    recruit(&mut w, z);
    assert!(w.get_entity(y).is_none(), "a dog blows up the plain helper");
    assert!(is_helper(&w, a) && is_helper(&w, b) && is_helper(&w, z));
}

#[test]
fn every_priority_helper_follows_you_downstairs() {
    let mut w = test_world(8);
    let p = player(&mut w);
    let mut pups = Vec::new();
    for dx in 1..=3 {
        let at = east_of_player(&mut w, dx);
        let pup = monster::plain_monster(&mut w, "pup", at);
        lend_forever::<PriorityHelper>(&mut w, pup);
        recruit(&mut w, pup);
        pups.push(pup);
    }
    let down = stair_location(&w.resource::<Map>().clone(), true).unwrap();
    *w.get_mut::<Position>(p).unwrap() = Position {
        x: down.0,
        y: down.1,
    };
    assert!(change_level(&mut w, true));
    for pup in pups {
        assert!(w.get::<Position>(pup).is_some(), "a pup was left behind");
    }
}

#[test]
fn a_shapeshifted_dog_keeps_its_traits_its_loyalty_and_says_so() {
    let mut w = test_world(6);
    let at = east_of_player(&mut w, 1);
    let pup = monster::plain_monster(&mut w, "dog", at);
    all_dog_traits(&mut w, pup);
    recruit(&mut w, pup);

    let grown = shapeshift(&mut w, pup).expect("a dog shapeshifts");
    assert!(w.get_entity(pup).is_none(), "the old body is gone");
    assert_eq!(*w.get::<Position>(grown).unwrap(), at);
    assert!(is_helper(&w, grown), "still your helper");
    assert!(
        w.get::<ShapeshiftOnKill>(grown).is_some()
            && w.get::<AlwaysTamed>(grown).is_some()
            && w.get::<AlwaysHelper>(grown).is_some()
            && w.get::<PriorityHelper>(grown).is_some(),
        "the grants came through the change"
    );
    let now = w.get::<Name>(grown).unwrap().what.clone();
    assert_ne!(now, "dog", "it became another monster");
    assert!(logged(
        &w,
        &strings::shapeshift_reveal(article_for("dog"), "dog", article_for(&now), &now)
    ));
}

/// Kills `n` hostile monsters with a fresh killer carrying `traits` and
/// reports how many times the killer came out a different creature.
fn shapeshifts_in(n: u32, shifts: bool) -> u32 {
    let mut w = test_world(21);
    let mut changed = 0;
    for _ in 0..n {
        let a = east_of_player(&mut w, 1);
        let b = east_of_player(&mut w, 2);
        let killer = monster::plain_monster(&mut w, "dog", a);
        if shifts {
            lend_forever::<ShapeshiftOnKill>(&mut w, killer);
        }
        recruit(&mut w, killer);
        let victim = monster::plain_monster(&mut w, "rat", b);
        w.get_mut::<Fighter>(victim).unwrap().hp = 1;
        for _ in 0..60 {
            if w.get_entity(victim).is_none() || w.get_entity(killer).is_none() {
                break;
            }
            resolve_attack(&mut w, killer, victim);
        }
        assert!(w.get_entity(victim).is_none(), "the fixture never killed");
        if w.get_entity(killer).is_none() {
            changed += 1;
        }
        let stray: Vec<Entity> = w
            .query_filtered::<Entity, (With<Mob>, Without<Player>)>()
            .iter(&w)
            .collect();
        for e in stray {
            w.despawn(e);
        }
    }
    changed
}

#[test]
fn a_kill_sometimes_shapeshifts_the_killer_and_only_one_that_can() {
    let n = 300;
    let expected = n as f64 * SHAPESHIFT_CHANCE;
    let got = shapeshifts_in(n, true) as f64;
    assert!(
        got > expected * 0.3 && got < expected * 2.0,
        "{got} shapeshifts in {n} kills, about {expected} expected"
    );
    assert_eq!(shapeshifts_in(100, false), 0, "no trait, no shapeshifting");
}

#[test]
fn a_shapeshifted_dog_is_still_a_dog_after_a_save() {
    let mut w = test_world(13);
    let at = east_of_player(&mut w, 1);
    let pup = spawn_monster(&mut w, MonsterDef::named("dog"), at);
    recruit(&mut w, pup);
    let grown = shapeshift(&mut w, pup).expect("a dog shapeshifts");

    let save = common::SaveFile::new("shapeshifted_dog");
    save_game(&mut w, save.path()).unwrap();
    let mut w2 = World::new();
    w2.insert_resource(GameRng(ChaCha12Rng::seed_from_u64(14)));
    w2.insert_resource(RngSeed(14));
    w2.init_resource::<GameLog>();
    w2.insert_resource(PlayerName { what: "X".into() });
    load_game(&mut w2, save.path()).unwrap();

    let name = w.get::<Name>(grown).unwrap().what.clone();
    let back = helpers(&mut w2);
    assert_eq!(back.len(), 1);
    assert_eq!(w2.get::<Name>(back[0]).unwrap().what, name);
    for (grant, id) in DOG_GRANTS.iter().zip([
        "always_tamed",
        "always_helper",
        "priority_helper",
        "shapeshift_on_kill",
    ]) {
        assert!(grant.probe(&w2, back[0]), "{id} was lost in the save");
    }
}

#[test]
fn nothing_cancels_a_dog() {
    let mut w = test_world(15);
    let at = east_of_player(&mut w, 1);
    let pup = spawn_monster(&mut w, MonsterDef::named("dog"), at);
    recruit(&mut w, pup);
    revoke_all(&mut w, pup);
    for grant in DOG_GRANTS {
        assert!(grant.probe(&w, pup), "cancellation stripped a dog grant");
    }
    assert!(is_helper(&w, pup));
}

/// Kills `victim` with `killer` by plain melee, however many swings it takes.
fn kill(w: &mut World, killer: Entity, victim: Entity) {
    w.get_mut::<Fighter>(victim).unwrap().hp = 1;
    for _ in 0..60 {
        if w.get_entity(victim).is_none() {
            return;
        }
        resolve_attack(w, killer, victim);
    }
    panic!("the fixture never landed the blow");
}

fn score(w: &mut World) -> u64 {
    w.query_filtered::<&Score, With<Player>>().single(w).value as u64
}

fn is_gone_without_gore(w: &mut World, at: Position, score_before: u64) -> bool {
    !w.resource::<BloodStains>().is_bloody(at.x, at.y)
        && !w.resource::<Corpses>().has(at.x, at.y)
        && !logged(w, "dies")
        && score(w) == score_before
}

#[test]
fn a_dog_that_dies_is_revealed_as_a_faerie_shapeshifter_and_is_gone_without_gore() {
    let mut w = test_world(30);
    let at = east_of_player(&mut w, 1);
    let pup = monster::plain_monster(&mut w, "dog", at);
    lend_forever::<FaerieOnDeath>(&mut w, pup);
    recruit(&mut w, pup);
    let foe_at = east_of_player(&mut w, 2);
    let foe = monster::plain_monster(&mut w, "rat", foe_at);
    let before = score(&mut w);

    kill(&mut w, foe, pup);

    assert!(logged(&w, &strings::faerie_reveal("a", "dog")));
    assert!(is_gone_without_gore(&mut w, at, before), "it left a mess");
    assert!(
        !logged(&w, "kills"),
        "the reveal takes the place of the kill line"
    );
    assert!(
        w.resource::<GameLog>()
            .unread
            .iter()
            .any(|l| l.category == LogCategory::Faerie),
        "the reveal is its own colour"
    );
}

#[test]
fn an_indirect_death_reveals_the_faerie_too() {
    let mut w = test_world(31);
    let at = east_of_player(&mut w, 1);
    let pup = monster::plain_monster(&mut w, "dog", at);
    lend_forever::<FaerieOnDeath>(&mut w, pup);
    let before = score(&mut w);
    w.get_mut::<Fighter>(pup).unwrap().hp = 0;
    reaper_system(&mut w);
    assert!(w.get_entity(pup).is_none());
    assert!(logged(&w, &strings::faerie_reveal("a", "dog")));
    assert!(is_gone_without_gore(&mut w, at, before));
}

#[test]
fn an_ordinary_death_still_leaves_gore() {
    let mut w = test_world(32);
    let at = east_of_player(&mut w, 1);
    let pal = monster::plain_monster(&mut w, "rat", at);
    w.get_mut::<Fighter>(pal).unwrap().hp = 0;
    reaper_system(&mut w);
    assert!(logged(&w, "dies"));
}
