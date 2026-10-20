use super::*;
use crate::map::{MAP_TILE_COUNT, tile_index};
use fixedbitset::FixedBitSet;

const OTHERS: [Faction; 3] = [Faction::Player, Faction::Ally, Faction::Monster];

/// A mob the pass already listed can be gone by its turn: polymorph
/// despawns its target and spawns the new shape as a fresh entity.
#[test]
fn a_mob_despawned_mid_pass_forfeits_its_turn() {
    let mut world = World::new();
    let player = world.spawn(Player).id();
    let mob = world
        .spawn(Mob {
            movement_type: MovementType::Chase,
        })
        .id();
    world.despawn(mob);
    let map = Map {
        tiles: vec![TileType::Room; MAP_TILE_COUNT],
        dark: FixedBitSet::with_capacity(MAP_TILE_COUNT),
        special: vec![None; MAP_TILE_COUNT],
        level: None,
    };
    let visible = HashSet::new();
    let ctx = AiCtx {
        pass: 0,
        player,
        player_pos: Position { x: 1, y: 1 },
        player_faction: Faction::Player,
        visible: &visible,
        stealthy: false,
        warded: false,
        map: &map,
    };
    assert!(!step_one_mob(&mut world, mob, &ctx, &mut HashMap::new()));
}

#[test]
fn a_paralysed_mob_loses_about_half_its_turns_at_full_tempo() {
    let mut world = World::new();
    world.insert_resource(GameRng(rand::SeedableRng::seed_from_u64(7)));
    let player = world.spawn((Player, Position { x: 20, y: 5 })).id();
    let mob = world
        .spawn((
            Mob {
                movement_type: MovementType::Chase,
            },
            Position { x: 5, y: 5 },
            Faction::Monster,
            Speed::new(SpeedKind::Normal),
        ))
        .id();
    crate::conditions::paralyse(&mut world, mob);
    let map = Map {
        tiles: vec![TileType::Room; MAP_TILE_COUNT],
        dark: FixedBitSet::with_capacity(MAP_TILE_COUNT),
        special: vec![None; MAP_TILE_COUNT],
        level: None,
    };
    let visible: HashSet<(u16, u16)> = (0..40).map(|x| (x, 5)).collect();
    let ctx = AiCtx {
        pass: 0,
        player,
        player_pos: Position { x: 20, y: 5 },
        player_faction: Faction::Player,
        visible: &visible,
        stealthy: false,
        warded: false,
        map: &map,
    };
    let mut lost = 0;
    for _ in 0..200 {
        world.get_mut::<Speed>(mob).unwrap().energy = Speed::COST;
        world.get_mut::<Position>(mob).unwrap().x = 5;
        let mut spatial = HashMap::from([((20, 5), (player, Faction::Player))]);
        lost += !step_one_mob(&mut world, mob, &ctx, &mut spatial) as i32;
    }
    assert!(
        (60..140).contains(&lost),
        "about half should go (got {lost})"
    );
}

#[test]
fn a_normal_mob_with_banked_energy_still_acts_once_per_round() {
    let mut world = World::new();
    world.insert_resource(GameRng(rand::SeedableRng::seed_from_u64(7)));
    world.init_resource::<AttackQueue>();
    let player = world
        .spawn((Player, Position { x: 7, y: 5 }, Faction::Player))
        .id();
    let mob = world
        .spawn((
            Mob {
                movement_type: MovementType::Chase,
            },
            Position { x: 5, y: 5 },
            Faction::Monster,
            Speed::new(SpeedKind::Normal),
        ))
        .id();
    world.get_mut::<Speed>(mob).unwrap().energy = 2 * Speed::COST;
    let map = open_floor();
    let visible: HashSet<(u16, u16)> = (0..40).map(|x| (x, 5)).collect();
    let mut ctx = AiCtx {
        pass: 0,
        player,
        player_pos: Position { x: 7, y: 5 },
        player_faction: Faction::Player,
        visible: &visible,
        stealthy: false,
        warded: false,
        map: &map,
    };
    monster_round(&mut world, &mut ctx);
    assert_eq!(world.get::<Position>(mob).unwrap().x, 6);
    assert!(
        world.resource::<AttackQueue>().attacks.is_empty(),
        "walked up and struck in one round"
    );
}

#[test]
fn a_normal_mob_never_acts_twice_in_one_turn_against_a_lurking_player() {
    let mut world = World::new();
    world.insert_resource(GameRng(rand::SeedableRng::seed_from_u64(7)));
    world.init_resource::<AttackQueue>();
    world.init_resource::<PlayerTempo>();
    world.init_resource::<ExtraMonsterRound>();
    world.insert_resource(open_floor());
    world.spawn((
        Player,
        Position { x: 12, y: 5 },
        Faction::Player,
        Speed::new(SpeedKind::Quick),
        Viewshed {
            visible_tiles: (0..40).map(|x| (x, 5)).collect(),
            revealed_tiles: FixedBitSet::with_capacity(MAP_TILE_COUNT),
            range: 8,
            dirty: false,
        },
    ));
    let mob = world
        .spawn((
            Mob {
                movement_type: MovementType::Chase,
            },
            Position { x: 5, y: 5 },
            Faction::Monster,
            Speed::new(SpeedKind::Normal),
        ))
        .id();
    world.get_mut::<Speed>(mob).unwrap().energy = 2 * Speed::COST;
    for turn in 0..12 {
        let before = *world.get::<Position>(mob).unwrap();
        let blows = world.resource::<AttackQueue>().attacks.len();
        ai(&mut world);
        let after = *world.get::<Position>(mob).unwrap();
        let acted =
            (after != before) as usize + world.resource::<AttackQueue>().attacks.len() - blows;
        assert!(acted <= 1, "turn {turn}: {acted} actions");
    }
}

fn stagger_once(seed: u64) -> (u16, u16) {
    let mut world = World::new();
    world.insert_resource(GameRng(rand::SeedableRng::seed_from_u64(seed)));
    let player = world.spawn((Player, Position { x: 20, y: 5 })).id();
    let mob = world
        .spawn((
            Mob {
                movement_type: MovementType::Confused,
            },
            Position { x: 5, y: 5 },
            Faction::Monster,
            Speed::new(SpeedKind::Normal),
        ))
        .id();
    world.get_mut::<Speed>(mob).unwrap().energy = Speed::COST;
    let map = open_floor();
    let visible: HashSet<(u16, u16)> = (0..40).map(|x| (x, 5)).collect();
    let ctx = AiCtx {
        pass: 0,
        player,
        player_pos: Position { x: 20, y: 5 },
        player_faction: Faction::Player,
        visible: &visible,
        stealthy: false,
        warded: false,
        map: &map,
    };
    let mut spatial = HashMap::from([((20, 5), (player, Faction::Player))]);
    step_one_mob(&mut world, mob, &ctx, &mut spatial);
    let now = *world.get::<Position>(mob).unwrap();
    (now.x, now.y)
}

#[test]
fn where_a_staggering_mob_lurches_follows_the_seed() {
    for seed in 0..32 {
        assert_eq!(stagger_once(seed), stagger_once(seed), "seed {seed}");
    }
    let seen: HashSet<(u16, u16)> = (0..32).map(stagger_once).collect();
    assert!(seen.len() > 1, "every seed lurched the same way");
}

#[test]
fn a_peaceful_spirit_fights_nobody_and_nobody_fights_it() {
    let mut world = World::new();
    world.init_resource::<SpiritsHostile>();
    for other in OTHERS {
        assert!(!hostile(&world, Faction::Spirits, other), "{other:?}");
        assert!(!hostile(&world, other, Faction::Spirits), "{other:?}");
    }
}

#[test]
fn an_angered_spirit_fights_everyone_but_its_own() {
    let mut world = World::new();
    world.insert_resource(SpiritsHostile(true));
    for other in OTHERS {
        assert!(hostile(&world, Faction::Spirits, other), "{other:?}");
        assert!(hostile(&world, other, Faction::Spirits), "{other:?}");
    }
    assert!(!hostile(&world, Faction::Spirits, Faction::Spirits));
}

#[test]
fn room_monsters_are_room_leashed() {
    let mut map = Map {
        tiles: vec![TileType::Wall; MAP_TILE_COUNT],
        dark: FixedBitSet::with_capacity(MAP_TILE_COUNT),
        special: vec![None; MAP_TILE_COUNT],
        level: None,
    };
    map.tiles[tile_index(5, 5)] = TileType::Room;
    map.tiles[tile_index(6, 5)] = TileType::Passage;

    assert!(!mob_can_enter(
        &map,
        true,
        Position { x: 5, y: 5 },
        6,
        5,
        false,
    ));
}

#[test]
fn the_leash_follows_the_tile_not_the_monster() {
    let mut map = Map {
        tiles: vec![TileType::Wall; MAP_TILE_COUNT],
        dark: FixedBitSet::with_capacity(MAP_TILE_COUNT),
        special: vec![None; MAP_TILE_COUNT],
        level: None,
    };
    map.tiles[tile_index(5, 5)] = TileType::Passage;
    map.tiles[tile_index(6, 5)] = TileType::Room;

    assert!(mob_can_enter(
        &map,
        true,
        Position { x: 5, y: 5 },
        6,
        5,
        false,
    ));
}

#[test]
fn corridor_monsters_can_step_through_a_door() {
    let mut map = Map {
        tiles: vec![TileType::Wall; MAP_TILE_COUNT],
        dark: FixedBitSet::with_capacity(MAP_TILE_COUNT),
        special: vec![None; MAP_TILE_COUNT],
        level: None,
    };
    map.tiles[tile_index(5, 5)] = TileType::Passage;
    map.tiles[tile_index(6, 5)] = TileType::Door;

    assert!(mob_can_enter(
        &map,
        true,
        Position { x: 5, y: 5 },
        6,
        5,
        false,
    ));
}

fn open_floor() -> Map {
    Map {
        tiles: vec![TileType::Room; MAP_TILE_COUNT],
        dark: FixedBitSet::with_capacity(MAP_TILE_COUNT),
        special: vec![None; MAP_TILE_COUNT],
        level: None,
    }
}

fn slide_once(seed: u64) -> (u16, u16) {
    let map = open_floor();
    let mut world = World::new();
    world.insert_resource(GameRng(rand::SeedableRng::seed_from_u64(seed)));
    let at = Position { x: 5, y: 5 };
    let mob = world.spawn((at, Faction::Monster)).id();
    let other = world
        .spawn((Position { x: 6, y: 5 }, Faction::Monster))
        .id();
    let mut spatial = HashMap::from([
        ((5, 5), (mob, Faction::Monster)),
        ((6, 5), (other, Faction::Monster)),
    ]);
    let moved = act(
        &mut world,
        mob,
        Action::Step(1, 0),
        at,
        false,
        false,
        false,
        &map,
        &mut spatial,
    );
    assert!(moved, "a bystander in the way costs it no turn");
    let now = *world.get::<Position>(mob).unwrap();
    (now.x, now.y)
}

#[test]
fn a_step_blocked_by_a_bystander_slides_to_a_diagonal_toward_the_goal() {
    let seen: HashSet<(u16, u16)> = (0..64).map(slide_once).collect();
    assert_eq!(
        seen,
        HashSet::from([(6, 4), (6, 6)]),
        "either side, both used"
    );
}

#[test]
fn the_side_a_blocked_step_slides_to_follows_the_seed() {
    for seed in 0..32 {
        assert_eq!(slide_once(seed), slide_once(seed), "seed {seed}");
    }
}

#[test]
fn a_blocked_step_with_no_open_diagonal_stays_put() {
    let mut map = open_floor();
    map.tiles[tile_index(6, 4)] = TileType::Wall;
    map.tiles[tile_index(6, 6)] = TileType::Wall;
    let mut world = World::new();
    world.insert_resource(GameRng(rand::SeedableRng::seed_from_u64(1)));
    let at = Position { x: 5, y: 5 };
    let mob = world.spawn((at, Faction::Monster)).id();
    let other = world
        .spawn((Position { x: 6, y: 5 }, Faction::Monster))
        .id();
    let mut spatial = HashMap::from([
        ((5, 5), (mob, Faction::Monster)),
        ((6, 5), (other, Faction::Monster)),
    ]);
    let moved = act(
        &mut world,
        mob,
        Action::Step(1, 0),
        at,
        false,
        false,
        false,
        &map,
        &mut spatial,
    );
    assert!(!moved);
    assert_eq!(*world.get::<Position>(mob).unwrap(), at);
}

#[test]
fn only_a_swimmer_takes_to_the_water() {
    let mut map = Map {
        tiles: vec![TileType::Wall; MAP_TILE_COUNT],
        dark: FixedBitSet::with_capacity(MAP_TILE_COUNT),
        special: vec![None; MAP_TILE_COUNT],
        level: None,
    };
    map.tiles[tile_index(5, 5)] = TileType::Room;
    map.tiles[tile_index(6, 5)] = TileType::Water;

    let steps = |swims| mob_can_enter(&map, true, Position { x: 5, y: 5 }, 6, 5, swims);
    assert!(!steps(false), "a walker stops at the shore");
    assert!(steps(true), "a swimmer goes in");
}
