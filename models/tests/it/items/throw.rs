//! Throwing: the third thing the pack screen can do with an item. A hurled
//! object flies to the spot you picked, stops at the first wall or creature in
//! the way, and then behaves like what it is — a missile, a bottle of glass, or
//! a page of instructions for anything literate enough to follow them.

use crate::common;
use crate::common::monster;

use crate::common::fixtures::{
    potion_row, scroll_row, spawn_armor, spawn_launcher, spawn_named, spawn_weapon, spell_row,
    wand_row,
};
use bevy_ecs::prelude::*;
use models::constants::wands::{GRENADE_DIE_PER_CHARGE, GRENADE_RADIUS};
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
    w
}

fn player(w: &mut World) -> Entity {
    w.query_filtered::<Entity, With<Player>>().single(w)
}

fn player_pos(w: &mut World) -> Position {
    let p = player(w);
    *w.get::<Position>(p).unwrap()
}

/// Spawn something and put it in the player's pack, as picking it up would.
fn stash(w: &mut World, p: Entity, spawn: impl FnOnce(&mut World) -> Entity) -> Entity {
    let item = spawn(w);
    w.entity_mut(item).remove::<Position>();
    w.get_mut::<Backpack>(p).unwrap().items.push(item);
    item
}

/// The tile an item is spawned on before it is stashed — never looked at.
const NOWHERE: Position = Position { x: 0, y: 0 };

/// The engine's "Throw" action: the throw names the item's pack row and
/// `throw_system` lifts it and finds out where it ends up. A quiver gives up
/// one arrow and stays in its slot.
fn throw(w: &mut World, thrower: Entity, item: Entity, target: Position) {
    let slot = w
        .get::<Backpack>(thrower)
        .and_then(|bp| bp.items.iter().position(|&e| e == item));
    w.resource_mut::<ThrowQueue>().throws.push(WantsToThrow {
        thrower,
        item,
        target,
        slot_idx: slot,
    });
    throw_system(w);
}

/// A plain monster dropped onto a tile next to the player so the throw has a
/// clear one-step flight.
fn monster(w: &mut World, _name: &str, at: Position) -> Entity {
    monster::plain_monster(w, "test monster", at)
}

/// An open tile `dx` to the east of the player, and the tile beyond it.
fn east_of_player(w: &mut World, dx: u16) -> Position {
    let p = player_pos(w);
    Position {
        x: p.x + dx,
        y: p.y,
    }
}

/// `Position` has no `Debug`, so compare tiles as plain pairs.
fn pos_of(w: &World, e: Entity) -> (u16, u16) {
    let p = w.get::<Position>(e).expect("entity has no position");
    (p.x, p.y)
}

fn logged(w: &World, needle: &str) -> bool {
    w.resource::<GameLog>()
        .history
        .iter()
        .any(|l| l.contains(needle))
}

#[test]
fn a_thrown_weapon_is_caught_and_wielded_by_a_creature_with_hands() {
    let mut w = test_world(7);
    let p = player(&mut w);
    let spot = east_of_player(&mut w, 1);
    let troll = monster::monster(&mut w, "test monster", spot);
    w.get_mut::<Fighter>(troll).unwrap().hp = 20;
    let mace = stash(&mut w, p, |w| spawn_weapon(w, "mace", NOWHERE));

    throw(&mut w, p, mace, spot);

    assert_eq!(w.get::<Equipped>(mace).unwrap().by, Some(troll));
    assert!(w.get::<Position>(mace).is_none());
    assert!(logged(&w, "wields it"));
}

#[test]
fn the_grenade_is_wider_and_hotter_than_the_beam() {
    fn burn(seed: u64, throw_it: bool) -> (usize, i32) {
        let mut w = test_world(seed);
        let p = player(&mut w);
        let traps: Vec<Entity> = w.query_filtered::<Entity, With<Trap>>().iter(&w).collect();
        for t in traps {
            w.entity_mut(t).despawn();
        }
        let pp = player_pos(&mut w);
        let orcs: Vec<Entity> = (1..=7)
            .map(|dx| {
                let m = monster(
                    &mut w,
                    "orc",
                    Position {
                        x: pp.x + dx,
                        y: pp.y,
                    },
                );
                w.get_mut::<Fighter>(m).unwrap().hp = 200;
                m
            })
            .collect();
        let target = Position {
            x: pp.x + 1,
            y: pp.y,
        };
        let wand = stash(&mut w, p, |w| {
            spawn_named(w, "wand of fire", NOWHERE).unwrap()
        });
        w.get_mut::<Battery>(wand).unwrap().charges = 5;

        if throw_it {
            throw(&mut w, p, wand, target);
        } else {
            let idx = w
                .get::<Backpack>(p)
                .unwrap()
                .items
                .iter()
                .position(|&e| e == wand);
            w.resource_mut::<UseQueue>().uses.push(WantsToUse {
                user: p,
                item: wand,
                target: Some(target),
                slot_idx: idx,
            });
            item_system(&mut w);
        }

        let burned = orcs
            .iter()
            .filter(|&&o| w.get::<Fighter>(o).unwrap().hp < 200)
            .count();
        let worst = orcs
            .iter()
            .map(|&o| 200 - w.get::<Fighter>(o).unwrap().hp)
            .max()
            .unwrap();
        (burned, worst)
    }

    let (zapped_count, _) = burn(21, false);
    let (thrown_count, _) = burn(21, true);
    assert!(
        thrown_count > zapped_count,
        "the grenade should catch more than the beam ({thrown_count} vs {zapped_count})"
    );

    const CHARGES: i32 = 5;
    let rolls: Vec<i32> = (0..60).map(|seed| burn(seed, true).1).collect();
    let floor = CHARGES;
    let ceiling = CHARGES * GRENADE_DIE_PER_CHARGE;
    assert!(
        rolls.iter().all(|&d| (floor..=ceiling).contains(&d)),
        "a grenade rolled outside {floor}..={ceiling}: {rolls:?}"
    );
    let mean = |v: &[i32]| v.iter().sum::<i32>() as f64 / v.len() as f64;
    let beam = mean(&(0..60).map(|seed| burn(seed, false).1).collect::<Vec<_>>());
    let grenade = mean(&rolls);
    assert!(
        grenade > beam,
        "the grenade should hit harder than the beam (beam {beam:.1}, grenade {grenade:.1})"
    );
}

#[test]
fn a_wand_lobbed_into_open_floor_lands_a_dud() {
    let mut w = test_world(3);
    let p = player(&mut w);
    let spot = east_of_player(&mut w, 2);
    let wand = stash(&mut w, p, |w| {
        spawn_named(w, "wand of fire", NOWHERE).unwrap()
    });
    w.get_mut::<Battery>(wand).unwrap().charges = 7;
    throw(&mut w, p, wand, spot);

    assert_eq!(pos_of(&w, wand), (spot.x, spot.y), "it just lands");
    assert_eq!(w.get::<Battery>(wand).unwrap().charges, 7, "charges intact");
    assert!(logged(&w, "still bottled up"));
}

#[test]
fn a_thrown_wand_of_nothing_makes_confetti_and_nothing_else() {
    let mut w = test_world(3);
    let p = player(&mut w);
    let spot = east_of_player(&mut w, 1);
    let orc = monster(&mut w, "orc", spot);
    let hp = |w: &World| {
        (
            w.get::<Fighter>(p).unwrap().hp,
            w.get::<Fighter>(orc).unwrap().hp,
        )
    };
    let before = hp(&w);
    let wand = stash(&mut w, p, |w| {
        (wand_row("wand of nothing").unwrap()).spawn(w, NOWHERE)
    });

    throw(&mut w, p, wand, spot);

    assert!(w.get_entity(wand).is_none(), "the wand burst");
    assert!(logged(&w, "confetti"));
    assert_eq!(hp(&w), before, "nobody was hurt");
    assert_eq!(pos_of(&w, orc), (spot.x, spot.y), "nobody moved");
}

#[test]
fn a_helper_lost_to_system_shock_is_minus_two_alignment() {
    let mut shocked = 0;
    for seed in 0..64 {
        let mut w = test_world(seed);
        let p = player(&mut w);
        let spot = east_of_player(&mut w, 1);
        let pal = monster(&mut w, "pal", spot);
        recruit(&mut w, pal);
        lend(&mut w, pal, Grant::of::<Polymorphed>(), Lifetime::Floor);
        assert_eq!(w.get::<Alignment>(p).unwrap().0, 1, "recruiting: +1");
        let wand = stash(&mut w, p, |w| {
            spawn_named(w, "wand of polymorph", NOWHERE).unwrap()
        });
        w.get_mut::<Battery>(wand).unwrap().charges = 6;

        throw(&mut w, p, wand, spot);

        let helper_left = w.query::<&Helper>().iter(&w).count() > 0;
        shocked += usize::from(!helper_left);
        let want = if helper_left { 1 } else { -1 };
        assert_eq!(w.get::<Alignment>(p).unwrap().0, want, "seed {seed}");
    }
    assert!(shocked > 0, "no seed shocked");
}

/// A bow wears its plus on the throw, not on a melee roll — the third kind
/// of gear, and the one a "weapons and armour" reading of the rule misses.
#[test]
fn a_thrown_wand_of_cancellation_devastates_the_player_it_catches() {
    let mut w = test_world(4);
    let p = player(&mut w);

    let blade = stash(&mut w, p, |w| spawn_weapon(w, "long sword", NOWHERE));
    w.entity_mut(blade).insert((PowerBonus(3), Curse));
    let bow = stash(&mut w, p, |w| spawn_launcher(w, "short bow", NOWHERE));
    w.entity_mut(bow).insert(ThrowBonus(3));
    let scroll = stash(&mut w, p, |w| {
        spawn_named(w, "scroll of teleportation", NOWHERE).unwrap()
    });
    let potion = stash(&mut w, p, |w| {
        spawn_named(w, "potion of poison", NOWHERE).unwrap()
    });

    let spot = east_of_player(&mut w, 1);
    monster(&mut w, "bat", spot);
    let wand = stash(&mut w, p, |w| {
        spawn_named(w, "wand of cancellation", NOWHERE).unwrap()
    });
    w.get_mut::<Battery>(wand).unwrap().charges = 4;
    throw(&mut w, p, wand, spot);

    assert_eq!(
        w.get::<PowerBonus>(blade).map(|b| b.0),
        Some(0),
        "the plus is wiped"
    );
    assert!(
        w.get::<Curse>(blade).is_none(),
        "but the curse lifts, blade intact"
    );
    assert_eq!(
        w.get::<ThrowBonus>(bow).map(|b| b.0),
        Some(0),
        "a bow's plus is an enchantment like any other, and goes the same way"
    );
    assert_eq!(
        w.get::<Scroll>(scroll).unwrap().row.name,
        scroll_row("scroll of blank paper").unwrap().name
    );
    assert_eq!(
        w.get::<Potion>(potion).unwrap().row.name,
        potion_row("potion of thirst quenching").unwrap().name
    );
}

#[test]
fn a_thrown_wand_of_light_dazzles_and_burns_everyone_in_the_blast() {
    let mut w = test_world(6);
    let p = player(&mut w);
    let spot = east_of_player(&mut w, 1);
    let orc = monster(&mut w, "orc", spot);
    w.get_mut::<Fighter>(orc).unwrap().hp = 40;

    let wand = stash(&mut w, p, |w| {
        spawn_named(w, "wand of light", NOWHERE).unwrap()
    });
    w.get_mut::<Battery>(wand).unwrap().charges = 5;
    throw(&mut w, p, wand, spot);

    assert!(w.get_entity(wand).is_none(), "the wand burst");
    assert!(
        w.get::<Fighter>(orc).unwrap().hp < 40,
        "the flash still burns like an attack wand"
    );
    assert!(
        matches!(
            w.get::<Mob>(orc).unwrap().movement_type,
            MovementType::Confused
        ),
        "and it dazzles what it catches"
    );
    assert!(logged(&w, "dazzled"));
}

#[test]
fn a_thrown_utility_wand_blast_deals_no_damage() {
    let mut w = test_world(8);
    let p = player(&mut w);
    let spot = east_of_player(&mut w, 1);
    let orc = monster(&mut w, "orc", spot);
    w.get_mut::<Fighter>(orc).unwrap().hp = 30;

    let wand = stash(&mut w, p, |w| {
        spawn_named(w, "wand of slow monster", NOWHERE).unwrap()
    });
    w.get_mut::<Battery>(wand).unwrap().charges = 9;
    throw(&mut w, p, wand, spot);

    assert_eq!(
        w.get::<Fighter>(orc).unwrap().hp,
        30,
        "the payload is the effect, not damage"
    );
    assert_eq!(w.get::<Speed>(orc).unwrap().kind, SpeedKind::Slow);
}

#[test]
fn a_thrown_wand_of_teleport_to_with_no_target_sends_a_victim_to_itself() {
    let mut w = test_world(5);
    let p = player(&mut w);
    let spot = east_of_player(&mut w, 1);
    let orc = monster(&mut w, "orc", spot);
    let before = pos_of(&w, orc);

    let wand = stash(&mut w, p, |w| {
        spawn_named(w, "wand of teleport to", NOWHERE).unwrap()
    });
    w.get_mut::<Battery>(wand).unwrap().charges = 5;
    throw(&mut w, p, wand, spot);

    assert_eq!(pos_of(&w, orc), before, "it arrives exactly where it stood");
    assert!(logged(&w, "teleports directly to themselves"));
}

#[test]
fn a_thrown_wand_of_swapping_shuffles_everyone_it_catches() {
    let mut w = test_world(3);
    let p = player(&mut w);
    let near = east_of_player(&mut w, 1);
    let far = east_of_player(&mut w, 2);
    let a = monster(&mut w, "orc", near);
    let b = monster(&mut w, "orc", far);
    let before = [p, a, b].map(|e| pos_of(&w, e));

    let wand = stash(&mut w, p, |w| {
        spawn_named(w, "wand of swapping", NOWHERE).unwrap()
    });
    w.get_mut::<Battery>(wand).unwrap().charges = 5;
    throw(&mut w, p, wand, near);

    let after = [p, a, b].map(|e| pos_of(&w, e));
    for (was, now) in before.iter().zip(&after) {
        assert_ne!(was, now, "nobody keeps their own tile");
    }
    let (mut was, mut now) = (before, after);
    was.sort();
    now.sort();
    assert_eq!(was, now, "and every tile still holds one of them");
}

#[test]
fn armor_thrown_at_a_monster_that_cannot_wear_it_binds_it() {
    let mut w = test_world(7);
    let p = player(&mut w);
    let spot = east_of_player(&mut w, 1);
    let bat = monster(&mut w, "bat", spot);
    let mail = stash(&mut w, p, |w| spawn_armor(w, "leather armor", NOWHERE));

    throw(&mut w, p, mail, spot);

    assert!(w.get::<Rooted>(bat).is_some(), "the bat is held");
    assert!(w.get_entity(mail).is_none(), "the armor is spent");
    assert!(logged(&w, "is somehow bound by the"));
    assert!(logged(&w, "But they can still attack!"));
}

#[test]
fn a_thrown_potion_is_drunk_by_its_target_and_names_itself_when_it_works() {
    let mut w = test_world(5);
    let p = player(&mut w);
    let spot = east_of_player(&mut w, 1);
    let orc = monster::plain_monster(&mut w, "test monster", spot);
    {
        let mut f = w.get_mut::<Fighter>(orc).unwrap();
        f.max_hp = 10;
        f.hp = 1;
    }
    let old_max_hp = w.get::<Fighter>(orc).unwrap().max_hp;
    let potion = stash(&mut w, p, |w| {
        spawn_named(w, "potion of healing", NOWHERE).unwrap()
    });

    throw(&mut w, p, potion, spot);

    let healed = w.get::<Fighter>(orc).unwrap();
    assert_eq!(healed.hp, healed.max_hp);
    assert!(
        healed.max_hp > old_max_hp,
        "healing raised the target's ceiling"
    );
    assert!(w.get_entity(potion).is_none(), "the bottle broke");
}

/// A scroll that does nothing: this test is about who reads it, not what it says.
static NOOP_SCROLL: ScrollDef = ScrollDef::new("blank test scroll", |_, _| {});

#[test]
fn only_a_creature_that_understands_items_reads_a_thrown_scroll() {
    let mut w = test_world(9);
    let p = player(&mut w);
    let spot = east_of_player(&mut w, 1);
    let bat = monster(&mut w, "bat", spot);
    w.get_mut::<Fighter>(bat).unwrap().hp = 20;
    let scroll = stash(&mut w, p, |w| (&NOOP_SCROLL).spawn(w, NOWHERE));

    throw(&mut w, p, scroll, spot);

    assert_eq!(pos_of(&w, scroll), (spot.x, spot.y));

    let scroll = stash(&mut w, p, |w| (&NOOP_SCROLL).spawn(w, NOWHERE));
    w.despawn(bat);
    let orc = monster::monster(&mut w, "test monster", spot);
    w.get_mut::<Fighter>(orc).unwrap().hp = 20;

    throw(&mut w, p, scroll, spot);

    assert!(
        w.get_entity(scroll).is_none(),
        "the scroll crumbled as it was read"
    );
    assert!(logged(&w, "reads it aloud"));
}

#[test]
fn the_element_of_yoord_never_leaves_your_hand() {
    let mut w = test_world(4);
    let p = player(&mut w);
    let element = stash(&mut w, p, |w| spawn_element_of_yoord(w, NOWHERE));

    assert!(throw_refusal(&w, p, element).is_some());
    assert!(drop_refusal(&w, p, element).is_some());
}

/// Arm a troll with a thrown mace, kill it, and report whether the mace
/// survived the death roll.
fn kill_an_armed_troll(seed: u64) -> bool {
    let mut w = test_world(seed);
    let p = player(&mut w);
    let spot = east_of_player(&mut w, 1);
    let troll = monster::monster(&mut w, "test monster", spot);
    w.get_mut::<Fighter>(troll).unwrap().hp = 20;
    let mace = stash(&mut w, p, |w| spawn_weapon(w, "mace", NOWHERE));
    throw(&mut w, p, mace, spot);
    assert_eq!(w.get::<Equipped>(mace).unwrap().by, Some(troll));

    w.get_mut::<Fighter>(troll).unwrap().hp = 0;
    reaper_system(&mut w);
    assert!(w.get_entity(troll).is_none());

    match w.get_entity(mace) {
        Some(_) => {
            assert_eq!(pos_of(&w, mace), (spot.x, spot.y));
            assert!(w.get::<Equipped>(mace).unwrap().by.is_none());
            assert!(logged(&w, "clatters to the floor"));
            true
        }
        None => {
            assert!(!logged(&w, "clatters to the floor"));
            false
        }
    }
}

#[test]
fn a_slain_catcher_leaves_its_gear_or_takes_it_with_it() {
    let survivals = (0..40).filter(|&seed| kill_an_armed_troll(seed)).count();
    assert!(survivals > 0, "no gear ever survived a death");
    assert!(survivals < 40, "gear always survived a death");
}

#[test]
fn a_weapon_keeps_its_thrown_damage_across_a_save() {
    let mut w = test_world(2);
    let p = player(&mut w);
    let row = &WEAPONS[0];
    let sword = stash(&mut w, p, |w| row.spawn(w, NOWHERE));
    let thrown = w.get::<ThrownDamage>(sword).expect("a weapon throws").0;

    let save = common::SaveFile::new("thrown-damage");
    save_game(&mut w, save.path()).unwrap();
    let mut loaded = test_world(2);
    load_game(&mut loaded, save.path()).unwrap();

    let dice: Vec<i32> = loaded
        .iter_entities()
        .filter(|e| e.get::<Name>().is_some_and(|n| n.what == row.name))
        .filter_map(|e| e.get::<ThrownDamage>().map(|t| t.0))
        .collect();
    assert_eq!(dice, vec![thrown]);
}

/// A shot that comes down on a trap the player has found sets it off — and a
/// trap nobody is standing on bursts instead of biting. The dagger never
/// touched the orc beside the trap; the trap did.
#[test]
fn a_shot_that_lands_on_a_trap_sets_it_off() {
    let mut w = test_world(7);
    let p = player(&mut w);
    let spot = east_of_player(&mut w, 2);
    assert!(!w.resource::<Map>().blocks(spot.x, spot.y), "open floor");

    let trap = spawn_named(&mut w, "sleeping gas trap", spot).unwrap();
    w.entity_mut(trap).remove::<Hidden>();
    let beside_it = east_of_player(&mut w, 3);
    let bystander = monster(&mut w, "orc", beside_it);
    w.get_mut::<Fighter>(bystander).unwrap().hp = 30;
    let dagger = stash(&mut w, p, |w| spawn_weapon(w, "dagger", NOWHERE));

    throw(&mut w, p, dagger, spot);

    assert!(
        w.get_entity(trap).is_none(),
        "the trap went off with the shot"
    );
    assert!(
        w.get::<Fighter>(bystander).unwrap().hp < 30,
        "the burst caught the orc standing next to the trap"
    );
    assert_eq!(
        w.get::<Asleep>(bystander).is_some(),
        true,
        "and so did the gas the trap was holding"
    );
}

/// A chaser three tiles east of a player who stands on a doorway, and where
/// the player stands. `moved` is whether the player's last turn was a step.
fn doorway_standoff(w: &mut World, moved: bool) -> (Entity, Position, Position) {
    let p = player(w);
    let here = player_pos(w);
    for dx in 1..=3 {
        w.resource_mut::<Map>().tiles[tile_index(here.x + dx, here.y)] = TileType::Room;
    }
    w.resource_mut::<Map>().tiles[tile_index(here.x, here.y)] = TileType::Door;
    let perch = east_of_player(w, 3);
    let chaser = monster(w, "chaser", perch);
    w.get_mut::<Mob>(chaser).unwrap().movement_type = MovementType::Chase;
    if moved {
        w.entity_mut(p).insert(EntityMoved);
    }
    let mut s = Schedule::default();
    s.add_systems(visibility_system);
    s.run(w);
    (chaser, here, perch)
}

/// The ward is one turn long: do anything but move while you stand there and
/// they come for you. The door itself is untouched, so the next step onto it
/// wards again.
#[test]
fn the_ward_lasts_only_the_turn_you_step_onto_the_doorway() {
    let mut w = test_world(11);
    let (chaser, _, perch) = doorway_standoff(&mut w, false);

    ai(&mut w);

    assert_ne!(pos_of(&w, chaser), (perch.x, perch.y), "it came for you");

    let p = player(&mut w);
    w.entity_mut(p).insert(EntityMoved);
    let before = pos_of(&w, chaser);
    ai(&mut w);
    assert_eq!(
        pos_of(&w, chaser),
        before,
        "stepping onto the doorway wards again"
    );
}

/// The room leash stops a chaser walking out into a doorway, not striking
/// someone who stands in one: past the first turn, a creature next to you
/// hits you.
#[test]
fn a_room_chaser_strikes_a_player_standing_in_the_doorway() {
    let mut w = test_world(11);
    let (chaser, _, _) = doorway_standoff(&mut w, false);
    let next_to_the_door = east_of_player(&mut w, 1);
    w.entity_mut(chaser).insert(next_to_the_door);
    let mut s = Schedule::default();
    s.add_systems(visibility_system);
    s.run(&mut w);

    ai(&mut w);

    assert_eq!(
        w.resource::<AttackQueue>().attacks.len(),
        1,
        "it swung at you"
    );
}

/// A wand of digging that bursts on impact leaves a crater: every wall inside
/// the grenade's radius goes, in line of sight or not, and none outside it.
#[test]
fn a_thrown_wand_of_digging_makes_a_crater() {
    let mut w = test_world(5);
    let p = player(&mut w);
    {
        let mut map = w.resource_mut::<Map>();
        map.tiles.fill(TileType::Wall);
        for x in 20..=22 {
            map.tiles[tile_index(x, 10)] = TileType::Room;
        }
    }
    w.get_mut::<Position>(p).unwrap().x = 20;
    w.get_mut::<Position>(p).unwrap().y = 10;
    let wand = stash(&mut w, p, |w| {
        spawn_named(w, "wand of digging", NOWHERE).unwrap()
    });
    throw(&mut w, p, wand, Position { x: 23, y: 10 });

    let r = GRENADE_RADIUS;
    let map = w.resource::<Map>();
    for y in 1..MAP_HEIGHT - 1 {
        for x in 1..MAP_WIDTH - 1 {
            let (dx, dy) = (x as f32 - 22.0, y as f32 - 10.0);
            let inside = (dx * dx + dy * dy).sqrt() <= r;
            let was_rock = !(20..=22).contains(&x) || y != 10;
            if was_rock {
                assert_eq!(
                    map.tile(x, y) != TileType::Wall,
                    inside,
                    "({x},{y}) {}",
                    if inside {
                        "survived the crater"
                    } else {
                        "was dug outside it"
                    }
                );
            }
        }
    }
}

#[test]
fn a_centaur_spawns_with_a_bow_and_no_arrows_underfoot() {
    let mut w = test_world(1);
    let def = MonsterDef::named("centaur");
    let mut rng = ChaCha12Rng::seed_from_u64(3);
    let at = Position { x: 1, y: 1 };
    let mobs: Vec<_> = (0..20)
        .map(|_| spawn_monster_with_rng(&mut w, def, at, &mut rng))
        .collect();
    let strapped = mobs
        .iter()
        .filter(|&&m| wielded_launcher(&w, m).is_some())
        .count();
    assert!(strapped > 0);
    let arrows = w
        .query::<(&Name, &Position)>()
        .iter(&w)
        .filter(|(n, _)| n.what.contains("arrow"))
        .count();
    assert_eq!(arrows, 0);
}

/// The reticle's preview of the blast a confirm would set off, as a set.
fn preview(
    w: &mut World,
    item: Option<Entity>,
    throwing: bool,
    spell_effect: Option<&'static SpellDef>,
    at: Position,
) -> std::collections::HashSet<(u16, u16)> {
    w.insert_resource(TargetingState {
        active: true,
        item,
        throwing,
        spell_effect,
        cursor_x: at.x as i16,
        cursor_y: at.y as i16,
        ..Default::default()
    });
    aim_footprint(w).into_iter().collect()
}

/// Whether `cells` is a disc of `radius` around `center`: the centre in it and
/// nothing further out.
fn centred_on(
    cells: &std::collections::HashSet<(u16, u16)>,
    center: Position,
    radius: f32,
) -> bool {
    cells.contains(&(center.x, center.y))
        && cells.iter().all(|&(x, y)| {
            let (dx, dy) = (x as f32 - center.x as f32, y as f32 - center.y as f32);
            (dx * dx + dy * dy).sqrt() <= radius
        })
}

#[test]
fn a_thrown_wand_previews_its_burst_on_the_first_creature_in_the_way() {
    let mut w = test_world(13);
    let p = player(&mut w);
    let near = east_of_player(&mut w, 1);
    monster(&mut w, "orc", near);
    let wand = stash(&mut w, p, |w| {
        spawn_named(w, "wand of fire", NOWHERE).unwrap()
    });
    let cursor = east_of_player(&mut w, 2 + GRENADE_RADIUS as u16);

    let cells = preview(&mut w, Some(wand), true, None, cursor);

    assert!(centred_on(&cells, near, GRENADE_RADIUS), "{cells:?}");
    assert!(!cells.contains(&(cursor.x, cursor.y)));
}

#[test]
fn a_creature_the_player_cannot_see_does_not_move_the_preview() {
    let mut w = test_world(13);
    let p = player(&mut w);
    let near = east_of_player(&mut w, 1);
    let unseen = monster(&mut w, "orc", near);
    w.entity_mut(unseen).insert(Hidden);
    let wand = stash(&mut w, p, |w| {
        spawn_named(w, "wand of fire", NOWHERE).unwrap()
    });
    let cursor = east_of_player(&mut w, 2);

    assert!(preview(&mut w, Some(wand), true, None, cursor).is_empty());
}

#[test]
fn a_thrown_potion_previews_its_splash_where_it_lands() {
    use models::constants::potions::POTION_SPLASH_RADIUS;
    let mut w = test_world(13);
    let p = player(&mut w);
    let near = east_of_player(&mut w, 1);
    monster(&mut w, "orc", near);
    let potion = stash(&mut w, p, |w| {
        spawn_named(w, "potion of healing", NOWHERE).unwrap()
    });
    let cursor = east_of_player(&mut w, 3);

    let cells = preview(&mut w, Some(potion), true, None, cursor);

    assert!(centred_on(&cells, near, POTION_SPLASH_RADIUS), "{cells:?}");
}

#[test]
fn a_bolt_a_line_spell_and_a_look_preview_nothing() {
    let mut w = test_world(13);
    let p = player(&mut w);
    let wand = stash(&mut w, p, |w| {
        spawn_named(w, "wand of striking", NOWHERE).unwrap()
    });
    let cursor = east_of_player(&mut w, 3);

    assert!(preview(&mut w, Some(wand), false, None, cursor).is_empty());
    assert!(
        preview(
            &mut w,
            None,
            false,
            Some(spell_row("Force Lance").unwrap()),
            cursor
        )
        .is_empty()
    );
    w.resource_mut::<TargetingState>().looking = true;
    assert!(aim_footprint(&mut w).is_empty());
}
