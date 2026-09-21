//! The wand table: armour-ignoring bolt damage, the charge roll, the drain-life
//! lifesteal, elemental immunities, and the utility wands (light, polymorph,
//! haste/slow, teleport, cancellation) plus the speed system they lean on.
//!
//! Where a test needs to know how big a wand's dice are it reads
//! `constants::wands` rather than naming a number: the dice are a balance
//! decision, and a test that pins one is a test that fails the next time
//! somebody makes one.

#[path = "common/monster.rs"]
mod monster;

use bevy_ecs::prelude::*;
use bevy_ecs::schedule::Schedule;
use models::constants::wands::{DAMAGE_DICE, DAMAGE_SIDES, WAND_CHARGES};
use models::*;

const UNDEAD: &[Grant] = &[Grant::of::<Undead>()];
const FIRE_IMMUNITY: &[Grant] = &[Grant::of::<FireImmune>()];
const COLD_IMMUNITY: &[Grant] = &[Grant::of::<ColdImmune>()];

fn test_world(seed: u64) -> World {
    let mut w = World::new();
    w.insert_resource(GameRng(ChaCha12Rng::seed_from_u64(seed)));
    w.insert_resource(RngSeed(seed));
    w.init_resource::<GameLog>();
    w.init_resource::<UseQueue>();
    w.init_resource::<AttackQueue>();
    w.init_resource::<Ending>();
    w.init_resource::<PlayerTempo>();
    w.insert_resource(PlayerName {
        what: "TESTER".into(),
    });
    initialize_world(&mut w);
    w
}

fn player(w: &mut World) -> Entity {
    w.query_filtered::<Entity, With<Player>>().single(w)
}

/// Give the player a fresh, well-charged wand of the given kind, already in the
/// pack.
fn give_wand(w: &mut World, p: Entity, effect: WandEffect) -> Entity {
    let wand = spawn_wand(w, effect, Position { x: 0, y: 0 });
    w.entity_mut(wand).remove::<Position>();
    w.get_mut::<Battery>(wand).unwrap().charges = 9;
    w.get_mut::<Backpack>(p).unwrap().items.push(wand);
    wand
}

/// Zap `wand` at `target` — the engine's targeted "Use".
fn zap(w: &mut World, user: Entity, wand: Entity, target: Position) {
    let i = w
        .get_mut::<Backpack>(user)
        .unwrap()
        .items
        .iter()
        .position(|&e| e == wand);
    if let Some(i) = i {
        w.get_mut::<Backpack>(user).unwrap().items.remove(i);
        w.resource_mut::<UseQueue>().uses.push(WantsToUse {
            user,
            item: wand,
            target: Some(target),
            slot_idx: Some(i),
        });
    }
    item_system(w);
}

/// Zap a self-targeted wand (light).
fn zap_self(w: &mut World, user: Entity, wand: Entity) {
    let i = w
        .get_mut::<Backpack>(user)
        .unwrap()
        .items
        .iter()
        .position(|&e| e == wand);
    if let Some(i) = i {
        w.get_mut::<Backpack>(user).unwrap().items.remove(i);
        w.resource_mut::<UseQueue>().uses.push(WantsToUse {
            user,
            item: wand,
            target: None,
            slot_idx: Some(i),
        });
    }
    item_system(w);
}

fn dummy(w: &mut World, name: &str, at: Position, hp: i32) -> Entity {
    w.spawn((
        Name { what: name.into() },
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

fn run_ai(w: &mut World) {
    let mut s = Schedule::default();
    s.add_systems(ai);
    s.run(w);
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

// ---------------------------------------------------------------------------
// Charges & damage
// ---------------------------------------------------------------------------

#[test]
fn a_fresh_wand_always_spawns_fully_charged() {
    let mut w = test_world(1);
    let wand = spawn_wand(&mut w, WandEffect::MagicMissile, Position { x: 0, y: 0 });
    let charges = w.get::<Battery>(wand).unwrap().charges;
    assert_eq!(
        charges, WAND_CHARGES,
        "a fresh wand should always spawn with exactly WAND_CHARGES"
    );
}

#[test]
fn a_wand_bolt_ignores_armour_entirely() {
    let mut w = test_world(2);
    let p = player(&mut w);
    let (_here, spot) = beside_player(&mut w);
    // Absurd armour — a wand bolt should not care.
    let target = w
        .spawn((
            Name {
                what: "tank".into(),
            },
            Mob {
                movement_type: MovementType::Static,
            },
            spot,
            Fighter {
                hp: 40,
                max_hp: 40,
                armor: 99,
                power: 1,
                max_power: 1,
                armor_bonus: 99,
                power_bonus: 0,
            },
            Faction::Monster,
            Blood,
        ))
        .id();

    let wand = give_wand(&mut w, p, WandEffect::MagicMissile);
    zap(&mut w, p, wand, spot);

    // The dice are `wands::DAMAGE_DICE d DAMAGE_SIDES` and belong to whoever is
    // balancing wands. What this test is for is the *armour* clause: the target
    // above is wearing the best armour the numbers allow and the bolt goes
    // through all of it, so the loss has to be a bare roll of those dice.
    let lost = 40 - w.get::<Fighter>(target).unwrap().hp;
    let (floor, ceiling) = (DAMAGE_DICE, DAMAGE_DICE * DAMAGE_SIDES);
    assert!(
        (floor..=ceiling).contains(&lost),
        "a bolt through armour 99+99 lost {lost}, outside {floor}..={ceiling}"
    );
}

// ---------------------------------------------------------------------------
// Drain life
// ---------------------------------------------------------------------------

#[test]
fn drain_life_heals_the_zapper_by_the_damage_dealt() {
    let mut w = test_world(3);
    let p = player(&mut w);
    let (_here, spot) = beside_player(&mut w);
    let target = dummy(&mut w, "orc", spot, 30);
    w.get_mut::<Fighter>(p).unwrap().max_hp = 50;
    w.get_mut::<Fighter>(p).unwrap().hp = 10;

    let wand = give_wand(&mut w, p, WandEffect::DrainLife);
    zap(&mut w, p, wand, spot);

    let dealt = 30 - w.get::<Fighter>(target).unwrap().hp;
    let healed = w.get::<Fighter>(p).unwrap().hp - 10;
    assert!(dealt > 0);
    assert_eq!(
        healed, dealt,
        "the zapper recovers exactly the life drained"
    );
}

#[test]
fn undead_are_immune_to_draining_and_grant_no_lifesteal() {
    let mut w = test_world(3);
    let p = player(&mut w);
    let (_here, spot) = beside_player(&mut w);
    let zombie = monster::monster(&mut w, "zombie", spot);
    grant_all(&mut w, zombie, UNDEAD);
    assert!(w.get::<Undead>(zombie).is_some());
    let zhp = w.get::<Fighter>(zombie).unwrap().hp;
    w.get_mut::<Fighter>(p).unwrap().hp = 5;

    let wand = give_wand(&mut w, p, WandEffect::DrainLife);
    zap(&mut w, p, wand, spot);

    assert_eq!(
        w.get::<Fighter>(zombie).unwrap().hp,
        zhp,
        "the undead takes no drain damage"
    );
    assert_eq!(
        w.get::<Fighter>(p).unwrap().hp,
        5,
        "and the zapper heals nothing"
    );
    // The wand's flavour noun for draining is "evil magic" (see `Element::noun`).
    assert!(
        w.resource::<GameLog>()
            .history
            .iter()
            .any(|l| l.contains("unharmed by the evil magic"))
    );
}

// ---------------------------------------------------------------------------
// Elemental immunities
// ---------------------------------------------------------------------------

#[test]
fn a_dragon_shrugs_off_fire_and_a_yeti_shrugs_off_cold() {
    let mut w = test_world(7);
    let p = player(&mut w);
    let (_here, spot) = beside_player(&mut w);

    let dragon = monster::monster(&mut w, "dragon", spot);
    grant_all(&mut w, dragon, FIRE_IMMUNITY);
    let dhp = w.get::<Fighter>(dragon).unwrap().hp;
    let fire = give_wand(&mut w, p, WandEffect::Fire);
    zap(&mut w, p, fire, spot);
    assert_eq!(
        w.get::<Fighter>(dragon).unwrap().hp,
        dhp,
        "fire cannot burn the dragon"
    );
    w.entity_mut(dragon).despawn();

    let yeti = monster::monster(&mut w, "yeti", spot);
    grant_all(&mut w, yeti, COLD_IMMUNITY);
    let yhp = w.get::<Fighter>(yeti).unwrap().hp;
    let cold = give_wand(&mut w, p, WandEffect::Cold);
    zap(&mut w, p, cold, spot);
    assert_eq!(
        w.get::<Fighter>(yeti).unwrap().hp,
        yhp,
        "cold cannot freeze the yeti"
    );
}

// ---------------------------------------------------------------------------
// Polymorph
// ---------------------------------------------------------------------------

#[test]
fn polymorph_swaps_the_target_for_a_different_species_on_the_same_tile() {
    let mut w = test_world(5);
    let p = player(&mut w);
    let (_here, spot) = beside_player(&mut w);
    let orc = monster::monster(&mut w, "test monster", spot);
    let before = w.query_filtered::<(), With<Mob>>().iter(&w).count();

    let wand = give_wand(&mut w, p, WandEffect::Polymorph);
    zap(&mut w, p, wand, spot);

    assert!(!w.entities().contains(orc), "the original is gone");
    let after = w.query_filtered::<(), With<Mob>>().iter(&w).count();
    assert_eq!(after, before, "one in, one out");
    let mut q = w.query_filtered::<(&Position, &Name), With<Mob>>();
    let replacement = q
        .iter(&w)
        .find(|(pos, _)| pos.x == spot.x && pos.y == spot.y);
    let (_, name) = replacement.expect("something now stands on that tile");
    assert_ne!(name.what, "orc", "and it is a different creature");
}

// ---------------------------------------------------------------------------
// Speed: haste / slow the monster, and the turn economy
// ---------------------------------------------------------------------------

#[test]
fn haste_and_slow_step_the_target_along_the_speed_scale() {
    let mut w = test_world(1);
    let p = player(&mut w);
    let (_here, spot) = beside_player(&mut w);
    let mob = dummy(&mut w, "orc", spot, 5);

    let haste = give_wand(&mut w, p, WandEffect::HasteMonster);
    zap(&mut w, p, haste, spot);
    assert_eq!(w.get::<Speed>(mob).unwrap().kind, SpeedKind::Fast);

    let slow = give_wand(&mut w, p, WandEffect::SlowMonster);
    zap(&mut w, p, slow, spot);
    assert_eq!(
        w.get::<Speed>(mob).unwrap().kind,
        SpeedKind::Normal,
        "slow undoes a haste"
    );
}

#[test]
fn a_fast_monster_moves_twice_and_a_slow_one_every_other_turn() {
    let mut w = test_world(9);
    let p = player(&mut w);
    let here = *w.get::<Position>(p).unwrap();

    // A clear horizontal run of floor to the player's right.
    let row: Vec<u16> = {
        let map = w.resource::<Map>();
        (1..=6)
            .map(|d| here.x + d)
            .take_while(|&x| !map.blocks(x, here.y))
            .collect()
    };
    assert!(row.len() >= 5, "need open floor for the walk test");
    let vis: Vec<(u16, u16)> = row
        .iter()
        .map(|&x| (x, here.y))
        .chain([(here.x, here.y)])
        .collect();

    let start = Position {
        x: row[4],
        y: here.y,
    };
    let fast = w
        .spawn((
            Name {
                what: "cheetah".into(),
            },
            Mob {
                movement_type: MovementType::Chase,
            },
            start,
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
            Speed::new(SpeedKind::Fast),
        ))
        .id();
    w.get_mut::<Viewshed>(p).unwrap().visible_tiles = vis.clone();

    run_ai(&mut w);
    assert_eq!(
        w.get::<Position>(fast).unwrap().x,
        row[2],
        "a fast chaser closes two tiles in one turn"
    );

    // A slow creature: one step every second turn.
    w.entity_mut(fast).despawn();
    let slow = w
        .spawn((
            Name {
                what: "slug".into(),
            },
            Mob {
                movement_type: MovementType::Chase,
            },
            Position {
                x: row[4],
                y: here.y,
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
            Speed::new(SpeedKind::Slow),
        ))
        .id();
    w.get_mut::<Viewshed>(p).unwrap().visible_tiles = vis;

    run_ai(&mut w);
    assert_eq!(
        w.get::<Position>(slow).unwrap().x,
        row[4],
        "turn 1: the slug bides"
    );
    run_ai(&mut w);
    assert_eq!(
        w.get::<Position>(slow).unwrap().x,
        row[3],
        "turn 2: the slug takes its step"
    );
}

// ---------------------------------------------------------------------------
// Teleport away / to
// ---------------------------------------------------------------------------

#[test]
fn teleport_away_relocates_the_target() {
    let mut w = test_world(4);
    let p = player(&mut w);
    let (_here, spot) = beside_player(&mut w);
    let mob = dummy(&mut w, "orc", spot, 5);

    let wand = give_wand(&mut w, p, WandEffect::TeleportAway);
    zap(&mut w, p, wand, spot);

    let now = *w.get::<Position>(mob).unwrap();
    assert_ne!((now.x, now.y), (spot.x, spot.y), "it moved");
    assert!(
        !w.resource::<Map>().blocks(now.x, now.y),
        "onto open ground"
    );
}

#[test]
fn teleport_to_drags_the_target_next_to_the_zapper() {
    let mut w = test_world(4);
    let p = player(&mut w);
    let here = *w.get::<Position>(p).unwrap();
    // Somewhere else on the floor.
    let far = {
        let map = w.resource::<Map>();
        (0..MAP_HEIGHT)
            .flat_map(|y| (0..MAP_WIDTH).map(move |x| (x, y)))
            .find(|&(x, y)| !map.blocks(x, y) && x.abs_diff(here.x) + y.abs_diff(here.y) > 6)
            .map(|(x, y)| Position { x, y })
            .unwrap()
    };
    let mob = dummy(&mut w, "orc", far, 5);

    let wand = give_wand(&mut w, p, WandEffect::TeleportTo);
    zap(&mut w, p, wand, far);

    let now = *w.get::<Position>(mob).unwrap();
    assert!(
        now.x.abs_diff(here.x) <= 1 && now.y.abs_diff(here.y) <= 1,
        "now at the zapper's side"
    );
}

/// The drag lands the victim *next to the zapper*, and a dagger lying on the
/// floor beside them is not a reason it can't. `free_adjacent_tile` used to
/// count every entity with a `Position` as an occupant — floor loot included —
/// so a single dropped item next to the player sent the whole spell down its
/// "nowhere to put them" path and flung the target to a random tile instead,
/// while the log still said it had been dragged to your side.
#[test]
fn teleport_to_is_not_blocked_by_loot_lying_on_the_floor() {
    let mut w = test_world(4);
    let p = player(&mut w);
    let here = *w.get::<Position>(p).unwrap();

    // Cover every open tile around the player in dropped scrolls.
    let ring: Vec<Position> = {
        let map = w.resource::<Map>();
        (-1i32..=1)
            .flat_map(|dy| (-1i32..=1).map(move |dx| (dx, dy)))
            .filter(|&(dx, dy)| (dx, dy) != (0, 0))
            .map(|(dx, dy)| (here.x as i32 + dx, here.y as i32 + dy))
            .filter(|&(x, y)| x >= 0 && y >= 0 && !map.blocks(x as u16, y as u16))
            .map(|(x, y)| Position {
                x: x as u16,
                y: y as u16,
            })
            .collect()
    };
    assert!(!ring.is_empty(), "the player has somewhere to drag them to");
    for spot in &ring {
        spawn_scroll(&mut w, ScrollEffect::Identify, *spot);
    }

    let far = {
        let map = w.resource::<Map>();
        (0..MAP_HEIGHT)
            .flat_map(|y| (0..MAP_WIDTH).map(move |x| (x, y)))
            .find(|&(x, y)| !map.blocks(x, y) && x.abs_diff(here.x) + y.abs_diff(here.y) > 6)
            .map(|(x, y)| Position { x, y })
            .unwrap()
    };
    let mob = dummy(&mut w, "orc", far, 5);

    let wand = give_wand(&mut w, p, WandEffect::TeleportTo);
    zap(&mut w, p, wand, far);

    let now = *w.get::<Position>(mob).unwrap();
    assert!(
        now.x.abs_diff(here.x) <= 1 && now.y.abs_diff(here.y) <= 1,
        "dragged to the zapper's side, loot or no loot (landed at {now:?})"
    );
}

/// With genuinely nowhere to put them, the pull happens anyway and the target
/// does not survive it. It used to fall back to `random_open_tile` — a
/// teleport *away* wearing teleport-to's "dragged to your side!" message,
/// which is the one outcome this wand exists not to produce.
#[test]
fn teleport_to_with_no_room_bursts_the_target() {
    let mut w = test_world(4);
    let p = player(&mut w);
    let here = *w.get::<Position>(p).unwrap();

    // Wall the player in with creatures: every open neighbour taken.
    let ring: Vec<Position> = {
        let map = w.resource::<Map>();
        (-1i32..=1)
            .flat_map(|dy| (-1i32..=1).map(move |dx| (dx, dy)))
            .filter(|&(dx, dy)| (dx, dy) != (0, 0))
            .map(|(dx, dy)| (here.x as i32 + dx, here.y as i32 + dy))
            .filter(|&(x, y)| x >= 0 && y >= 0 && !map.blocks(x as u16, y as u16))
            .map(|(x, y)| Position {
                x: x as u16,
                y: y as u16,
            })
            .collect()
    };
    for (i, spot) in ring.iter().enumerate() {
        dummy(&mut w, &format!("rat{i}"), *spot, 3);
    }

    let far = {
        let map = w.resource::<Map>();
        (0..MAP_HEIGHT)
            .flat_map(|y| (0..MAP_WIDTH).map(move |x| (x, y)))
            .find(|&(x, y)| !map.blocks(x, y) && x.abs_diff(here.x) + y.abs_diff(here.y) > 6)
            .map(|(x, y)| Position { x, y })
            .unwrap()
    };
    let mob = dummy(&mut w, "orc", far, 5);

    let wand = give_wand(&mut w, p, WandEffect::TeleportTo);
    zap(&mut w, p, wand, far);

    assert!(
        w.get_entity(mob).is_none(),
        "no room at your side means it comes apart, not a random relocation"
    );
    assert!(
        w.resource::<GameLog>()
            .unread
            .iter()
            .any(|l| l.contains("gore")),
        "and the player is told what happened"
    );
}

/// The same rule at the other end: a floor with nowhere left to put the target
/// doesn't make the yank a no-op, it makes a mess. (A thrown wand's blast
/// resolves through the same function, so it gibs the same way.)
#[test]
fn teleport_away_with_nowhere_to_land_bursts_the_target() {
    let mut w = test_world(4);
    let p = player(&mut w);
    let (_here, spot) = beside_player(&mut w);
    let mob = dummy(&mut w, "orc", spot, 5);

    // Nowhere on the floor is open any more.
    {
        let mut map = w.resource_mut::<Map>();
        for tile in map.tiles.iter_mut() {
            *tile = TileType::Wall;
        }
    }

    let wand = give_wand(&mut w, p, WandEffect::TeleportAway);
    zap(&mut w, p, wand, spot);

    assert!(w.get_entity(mob).is_none(), "it did not arrive anywhere");
}

// ---------------------------------------------------------------------------
// Cancellation
// ---------------------------------------------------------------------------

#[test]
fn cancellation_strips_the_magic_but_leaves_the_creature() {
    let mut w = test_world(6);
    let p = player(&mut w);
    let (_here, spot) = beside_player(&mut w);
    let dragon = monster::monster(&mut w, "dragon", spot);
    grant_all(&mut w, dragon, FIRE_IMMUNITY);
    w.get_mut::<Speed>(dragon).unwrap().kind = SpeedKind::Fast;

    let wand = give_wand(&mut w, p, WandEffect::Cancellation);
    zap(&mut w, p, wand, spot);

    assert!(
        w.get::<FireImmune>(dragon).is_none(),
        "its innate magic is gone"
    );
    assert_eq!(
        w.get::<Speed>(dragon).unwrap().kind,
        SpeedKind::Normal,
        "back to a normal tempo"
    );
    assert!(w.get::<Fighter>(dragon).is_some(), "still a fighter");
    assert_eq!(
        w.get::<Name>(dragon).unwrap().what,
        "dragon",
        "still a dragon by name"
    );

    // With the immunity cancelled, fire now bites.
    let dhp = w.get::<Fighter>(dragon).unwrap().hp;
    let fire = give_wand(&mut w, p, WandEffect::Fire);
    zap(&mut w, p, fire, spot);
    assert!(
        w.get::<Fighter>(dragon).unwrap().hp < dhp,
        "a cancelled dragon burns"
    );
}

// ---------------------------------------------------------------------------
// Light
// ---------------------------------------------------------------------------

#[test]
fn light_clears_a_dark_room_and_reveals_its_traps() {
    let mut w = test_world(8);
    let p = player(&mut w);
    let here = *w.get::<Position>(p).unwrap();

    // Darken the player's tile and its room-floor neighbours, and hide a trap
    // on one of them.
    let mut room_tiles: Vec<Position> = Vec::new();
    {
        let map = w.resource::<Map>();
        for dy in -1i32..=1 {
            for dx in -1i32..=1 {
                let (nx, ny) = (here.x as i32 + dx, here.y as i32 + dy);
                if nx < 0 || ny < 0 {
                    continue;
                }
                let (nx, ny) = (nx as u16, ny as u16);
                if matches!(map.tile(nx, ny), TileType::Room | TileType::Upstairs) {
                    room_tiles.push(Position { x: nx, y: ny });
                }
            }
        }
    }
    {
        let mut map = w.resource_mut::<Map>();
        for t in &room_tiles {
            map.dark
                .insert((t.y as usize) * (MAP_WIDTH as usize) + t.x as usize);
        }
    }
    assert!(
        w.resource::<Map>().is_dark(here.x, here.y),
        "we made the room dark"
    );

    let trap_spot = *room_tiles
        .iter()
        .find(|t| (t.x, t.y) != (here.x, here.y))
        .unwrap();
    let trap_bundle = {
        let mut rng = w.resource_mut::<GameRng>();
        TrapBundle::random(&mut rng.0, 1, trap_spot)
    };
    let trap = w.spawn(trap_bundle).id();

    let wand = give_wand(&mut w, p, WandEffect::Light);
    zap_self(&mut w, p, wand);

    assert!(
        !w.resource::<Map>().is_dark(here.x, here.y),
        "the room is lit for good"
    );
    assert!(
        w.get::<Hidden>(trap).is_none(),
        "the trap is no longer hidden"
    );
    assert!(
        w.get::<Trap>(trap).unwrap().revealed,
        "…and marked discovered"
    );
    assert!(
        w.get::<Viewshed>(p).unwrap().dirty,
        "viewshed queued for a recompute"
    );
}

/// Fire and cold are the wands that *blast*, and a blast that washes over a
/// trap sets it off — the same trick shot a missile makes, worked with a stick.
#[test]
fn a_blast_sets_off_a_trap_it_washes_over() {
    let mut w = test_world(3);
    let p = player(&mut w);
    let (_, next_door) = beside_player(&mut w);
    let trap = w.spawn(TrapBundle::sleep(next_door)).id();

    let wand = give_wand(&mut w, p, WandEffect::Fire);
    zap(&mut w, p, wand, next_door);

    assert!(w.get_entity(trap).is_none(), "the blast set the trap off");
    assert!(
        w.resource::<GameLog>()
            .history
            .iter()
            .any(|l| l.contains("Trick shot!")),
        "and said so"
    );
    assert_eq!(
        w.get::<Asleep>(p).is_some(),
        true,
        "the gas rolls over the zapper standing right beside it"
    );
}

/// Teleporting a held creature lets it go — and the ledger has to hear about
/// it too. A `Turns` row left behind with no component under it still counts
/// down, and `effects::hold` reads those rows to decide whether a fresh hold
/// is worth applying: while the orphan lasts, the next bear trap the orc walks
/// into closes on nothing.
#[test]
fn teleporting_a_pinned_monster_clears_the_ledger_not_just_the_component() {
    let mut w = test_world(4);
    let p = player(&mut w);
    let (_here, spot) = beside_player(&mut w);
    let mob = dummy(&mut w, "orc", spot, 5);

    // Pinned the way a bear trap pins: through the ledger, for six turns.
    assert!(models::effects::hold(&mut w, mob, Grant::of::<Pinned>(), 6));

    let wand = give_wand(&mut w, p, WandEffect::TeleportAway);
    zap(&mut w, p, wand, spot);

    assert!(w.get::<Pinned>(mob).is_none(), "the jaws let go");
    assert_eq!(
        models::effects::turns_left(&w, mob, Grant::of::<Pinned>()),
        None,
        "and the ledger no longer thinks they are holding it"
    );
    // The symptom the stale row causes: a shorter, fresh hold is refused.
    assert!(
        models::effects::hold(&mut w, mob, Grant::of::<Pinned>(), 3),
        "a bear trap on the far side of the floor can still pin it"
    );
}
