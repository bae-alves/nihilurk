//! Agents: every mob perceives only while it stands in the player's view, and
//! a rule set turns what it perceives into one action. Out of view it does
//! nothing, unless it is aggravated (it heads for the shriek) or it is the
//! player's Helper (it heels).

mod common;
#[path = "common/monster.rs"]
#[allow(dead_code)] // only the handless fixture is wanted here
mod monster;

use bevy_ecs::prelude::*;
use fixedbitset::FixedBitSet;
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

fn pos(w: &World, e: Entity) -> Position {
    *w.get::<Position>(e).unwrap()
}

fn logged(w: &World, needle: &str) -> bool {
    w.resource::<GameLog>()
        .history
        .iter()
        .any(|l| l.contains(needle))
}

/// A walkable tile `dx` east of the player.
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

/// A walkable tile the player cannot see, well away from them.
fn out_of_view(w: &mut World) -> Position {
    run_visibility(w);
    let p = player(w);
    let here = pos(w, p);
    let seen = w.get::<Viewshed>(p).unwrap().visible_tiles.clone();
    let map = w.resource::<Map>().clone();
    (0..MAP_HEIGHT)
        .flat_map(|y| (0..MAP_WIDTH).map(move |x| Position { x, y }))
        .find(|t| {
            map.walkable(t.x, t.y, false)
                && !seen.contains(&(t.x, t.y))
                && chebyshev(*t, here) > 6
                // Room to move in all four directions, so a wanderer never
                // stands still only because it is boxed in.
                && [(1i32, 0i32), (-1, 0), (0, 1), (0, -1)].iter().all(|(dx, dy)| {
                    map.walkable((t.x as i32 + dx) as u16, (t.y as i32 + dy) as u16, false)
                })
        })
        .expect("the floor has an open tile out of view")
}

#[test]
fn a_mob_out_of_view_does_nothing_even_confused() {
    let mut w = test_world(4);
    let at = out_of_view(&mut w);
    let reeling = monster::plain_monster(&mut w, "rat", at);
    w.get_mut::<Mob>(reeling).unwrap().movement_type = MovementType::Confused;
    for _ in 0..10 {
        run_visibility(&mut w);
        ai(&mut w);
    }
    assert_eq!(pos(&w, reeling), at, "a confused mob off-view wandered");
}

#[test]
fn an_aggravated_mob_out_of_view_heads_for_the_shriek() {
    let mut w = test_world(4);
    let at = out_of_view(&mut w);
    let here = player_pos(&mut w);
    let mob = monster::plain_monster(&mut w, "rat", at);
    w.entity_mut(mob).insert(Aggravated {
        tx: here.x,
        ty: here.y,
    });
    run_visibility(&mut w);
    ai(&mut w);
    assert!(
        chebyshev(pos(&w, mob), here) < chebyshev(at, here),
        "an aggravated mob off-view did not close on the shriek"
    );
}

#[test]
fn an_aggravated_ambusher_in_view_goes_back_to_lying_in_wait() {
    let mut w = test_world(9);
    let here = player_pos(&mut w);
    let at = east_of_player(&mut w, 3);
    let trap = monster::plain_monster(&mut w, "venus flytrap", at);
    w.get_mut::<Mob>(trap).unwrap().movement_type = MovementType::Ambush;
    w.entity_mut(trap).insert(Aggravated {
        tx: here.x,
        ty: here.y,
    });
    run_visibility(&mut w);
    ai(&mut w);
    assert_eq!(pos(&w, trap), at, "an ambusher in view walked in");
}

#[test]
fn a_helper_out_of_view_heels() {
    let mut w = test_world(4);
    let here = player_pos(&mut w);
    let at = out_of_view(&mut w);
    let pal = monster::plain_monster(&mut w, "rat", at);
    recruit(&mut w, pal);
    run_visibility(&mut w);
    ai(&mut w);
    assert!(chebyshev(pos(&w, pal), here) < chebyshev(at, here));
}

/// A hand-built floor: a room at x 5..=9, a door at x 10 and a corridor east of
/// it along row 5. Returns the world with the player standing in the corridor.
fn room_and_corridor(w: &mut World) {
    let mut map = Map {
        tiles: vec![TileType::Wall; MAP_TILE_COUNT],
        dark: FixedBitSet::with_capacity(MAP_TILE_COUNT),
        special: vec![None; MAP_TILE_COUNT],
        level: None,
    };
    for y in 4..=6 {
        for x in 5..=9 {
            map.tiles[tile_index(x, y)] = TileType::Room;
        }
    }
    map.tiles[tile_index(10, 5)] = TileType::Door;
    for x in 11..=16 {
        map.tiles[tile_index(x, 5)] = TileType::Passage;
    }
    w.insert_resource(map);
    let p = player(w);
    *w.get_mut::<Position>(p).unwrap() = Position { x: 14, y: 5 };
    w.get_mut::<Viewshed>(p).unwrap().dirty = true;
}

#[test]
fn a_helper_is_not_room_leashed() {
    let mut w = test_world(4);
    room_and_corridor(&mut w);
    let pal = monster::plain_monster(&mut w, "rat", Position { x: 9, y: 5 });
    recruit(&mut w, pal);
    run_visibility(&mut w);
    ai(&mut w);
    assert_eq!(
        pos(&w, pal),
        Position { x: 10, y: 5 },
        "the helper would not leave the room"
    );
}

#[test]
fn a_tamed_dragon_breathes_only_where_the_fire_cannot_reach_you() {
    let mut w = test_world(9);
    let dragon_at = east_of_player(&mut w, 1);
    let foe_at = east_of_player(&mut w, 5);
    let dragon = spawn_monster(&mut w, MonsterDef::named("dragon"), dragon_at);
    recruit(&mut w, dragon);
    let foe = monster::plain_monster(&mut w, "kobold", foe_at);
    w.get_mut::<Mob>(foe).unwrap().movement_type = MovementType::Static;
    run_visibility(&mut w);
    w.resource_mut::<GameLog>().history.clear();
    ai(&mut w);
    assert!(
        logged(&w, &strings::breathe_fire_mob("dragon")),
        "the dragon held fire on a foe the blast could not carry to you"
    );

    let edge = constants::wands::BLAST_RADIUS as u16;
    let mut w = test_world(9);
    let dragon_at = east_of_player(&mut w, edge + 1);
    let foe_at = east_of_player(&mut w, edge);
    let dragon = spawn_monster(&mut w, MonsterDef::named("dragon"), dragon_at);
    recruit(&mut w, dragon);
    let foe = monster::plain_monster(&mut w, "kobold", foe_at);
    w.get_mut::<Mob>(foe).unwrap().movement_type = MovementType::Static;
    run_visibility(&mut w);
    w.resource_mut::<GameLog>().history.clear();
    for _ in 0..12 {
        w.resource_mut::<AttackQueue>().attacks.clear();
        ai(&mut w);
        assert!(
            !logged(&w, &strings::breathe_fire_mob("dragon")),
            "the dragon breathed fire that would catch the player"
        );
        let attacks = &w.resource::<AttackQueue>().attacks;
        assert!(
            attacks
                .iter()
                .any(|a| a.attacker == dragon && a.target == foe)
        );
    }
}

#[test]
fn a_charmed_monster_walks_up_to_a_monster_and_fights_it() {
    let mut w = test_world(9);
    let pal_at = east_of_player(&mut w, 1);
    let foe_at = east_of_player(&mut w, 5);
    let pal = monster::plain_monster(&mut w, "rat", pal_at);
    charm(&mut w, pal);
    let foe = monster::plain_monster(&mut w, "kobold", foe_at);
    w.get_mut::<Mob>(foe).unwrap().movement_type = MovementType::Static;
    run_visibility(&mut w);
    let mut struck = false;
    for _ in 0..8 {
        w.resource_mut::<AttackQueue>().attacks.clear();
        ai(&mut w);
        struck |= w
            .resource::<AttackQueue>()
            .attacks
            .iter()
            .any(|a| a.attacker == pal && a.target == foe);
    }
    assert!(
        struck,
        "a charmed monster stood by while a monster was in view"
    );
}

#[test]
fn aggravation_survives_a_save_and_an_old_save_still_loads() {
    let mut w = test_world(11);
    let at = east_of_player(&mut w, 2);
    let a = monster::plain_monster(&mut w, "rat", at);
    w.entity_mut(a).insert(Aggravated { tx: 3, ty: 4 });
    let at = east_of_player(&mut w, 3);
    let b = monster::plain_monster(&mut w, "bat", at);
    w.get_mut::<Mob>(b).unwrap().movement_type = MovementType::Aggravated { tx: 7, ty: 8 };

    let save = common::SaveFile::new("aggravated");
    save_game(&mut w, save.path()).unwrap();
    let mut w2 = World::new();
    w2.insert_resource(GameRng(ChaCha12Rng::seed_from_u64(12)));
    w2.insert_resource(RngSeed(12));
    w2.init_resource::<GameLog>();
    w2.insert_resource(PlayerName { what: "X".into() });
    load_game(&mut w2, save.path()).unwrap();

    let mut found: Vec<(u16, u16)> = w2
        .query::<(&Mob, &Aggravated)>()
        .iter(&w2)
        .map(|(m, ag)| {
            assert!(
                matches!(m.movement_type, MovementType::Chase),
                "loaded with the retired movement type"
            );
            (ag.tx, ag.ty)
        })
        .collect();
    found.sort();
    assert_eq!(found, vec![(3, 4), (7, 8)]);
}

/// A caster spends the Ma a spell costs, and a dry one goes back to its claws:
/// the wild dragon's fireball is a spell like the player's, not a free trick.
#[test]
fn a_wild_dragon_pays_for_its_fireball_and_runs_dry() {
    let mut w = test_world(9);
    let p = player(&mut w);
    w.get_mut::<Fighter>(p).unwrap().hp = 10_000;
    w.get_mut::<Fighter>(p).unwrap().max_hp = 10_000;
    let at = east_of_player(&mut w, 3);
    let dragon = spawn_monster(&mut w, MonsterDef::named("dragon"), at);
    let cost = SpellDef::of(SpellEffect::DragonBreath).cost;
    *w.get_mut::<Magic>(dragon)
        .expect("a dragon is born with a Magic pool") = Magic {
        points: cost,
        max_points: cost,
    };
    run_visibility(&mut w);
    w.resource_mut::<GameLog>().history.clear();
    ai(&mut w);
    assert!(
        logged(&w, &strings::breathe_fire_mob("dragon")),
        "the dragon did not breathe with the Ma to do it"
    );
    assert_eq!(w.get::<Magic>(dragon).unwrap().points, 0);

    w.resource_mut::<GameLog>().history.clear();
    for _ in 0..12 {
        ai(&mut w);
    }
    assert!(
        !logged(&w, &strings::breathe_fire_mob("dragon")),
        "the dragon breathed fire on no Ma"
    );
}

/// The eel's lightning is picked from its spellset like any other spell, and
/// the bolt has to land on the player, which a spell written for the player to
/// cast never had to manage.
#[test]
fn a_wild_eel_answers_with_lightning_that_finds_the_player() {
    let mut w = test_world(9);
    let p = player(&mut w);
    w.get_mut::<Fighter>(p).unwrap().hp = 10_000;
    w.get_mut::<Fighter>(p).unwrap().max_hp = 10_000;
    let at = east_of_player(&mut w, 3);
    let eel = monster::plain_monster(&mut w, "eel", at);
    w.entity_mut(eel).insert((
        Spellset {
            slots: vec![SpellEffect::Thunderbolt],
        },
        Magic {
            points: 1,
            max_points: 1,
        },
    ));
    run_visibility(&mut w);
    ai(&mut w);
    assert!(
        w.get::<Fighter>(p).unwrap().hp < 10_000,
        "the eel's bolt did not find the player"
    );
}
