//! The bones ghost, end to end: a dead run's file gets picked up by a later
//! one only when it climbs back through that exact depth carrying the
//! Element of Yoord — never on the way down, and never used twice.

use std::path::PathBuf;
use std::sync::Mutex;

use bevy_ecs::prelude::*;
use models::constants::player::{START_ARMOR, START_HP, START_POWER};
use models::*;

/// How many tests in this binary are inside a [`BonesDir`] right now.
static IN_BONES_DIR: Mutex<usize> = Mutex::new(0);

/// A working directory of the test binary's own. `bones::deposit` and
/// `bones::take` name their file `bones-{depth}.sav` relative to the working
/// directory, so two test runs at once (a second terminal, an IDE, a CI matrix
/// on one checkout) would write and delete each other's files, and a ghost
/// deposited a moment ago would not be there to take.
///
/// The first test in enters a directory named for the process id and the last
/// one out removes it. The working directory is per process, not per test, so
/// every test here shares the one directory: give each its own depth.
struct BonesDir(PathBuf);

impl BonesDir {
    fn enter() -> Self {
        let dir = std::env::temp_dir().join(format!("nihilurk-bones-{}", std::process::id()));
        let mut inside = IN_BONES_DIR.lock().unwrap();
        std::fs::create_dir_all(&dir).expect("the temp directory is writable");
        std::env::set_current_dir(&dir).expect("the scratch directory exists");
        *inside += 1;
        Self(dir)
    }
}

impl Drop for BonesDir {
    fn drop(&mut self) {
        let mut inside = IN_BONES_DIR.lock().unwrap();
        *inside -= 1;
        if *inside == 0 {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
}

fn test_world(seed: u64, name: &str) -> World {
    let mut w = World::new();
    w.insert_resource(GameRng(ChaCha12Rng::seed_from_u64(seed)));
    w.insert_resource(RngSeed(seed));
    w.init_resource::<GameLog>();
    w.insert_resource(PlayerName { what: name.into() });
    initialize_world(&mut w);
    w
}

fn player(w: &mut World) -> Entity {
    w.query_filtered::<Entity, With<Player>>().single(w)
}

fn ghost_named(w: &mut World, name: &str) -> Option<Entity> {
    w.query_filtered::<(Entity, &Name), Without<Player>>()
        .iter(w)
        .find(|(_, n)| n.what == name)
        .map(|(e, _)| e)
}

/// Kills off a would-be "victim" run at `depth`, having it die with one
/// cursed-on-worn weapon and one loose potion, and writes its bones file.
fn die_and_leave_bones(depth: u8, name: &str) {
    let mut w = test_world(1, name);
    let p = player(&mut w);
    w.resource_mut::<Depth>().what = depth;

    let sword = catalog::spawn_weapon(&mut w, "long sword", Position { x: 0, y: 0 });
    w.entity_mut(sword).insert(equipment::Equipped {
        by: Some(p),
        slot: equipment::Slot::Hand,
    });
    let potion = catalog::spawn_potion(&mut w, PotionEffect::Healing, Position { x: 0, y: 0 });
    w.get_mut::<Backpack>(p).unwrap().items.push(potion);

    bones::deposit(&mut w).unwrap();
}

/// Puts `w`'s player one floor above `depth`, holding the Element of Yoord and
/// standing on the up-stair `initialize_world` already spawned them on, then
/// climbs — landing exactly on `depth`.
fn climb_into(w: &mut World, depth: u8) {
    let p = player(w);
    w.resource_mut::<Depth>().what = depth + 1;
    let amulet = w.spawn(Amulet).id();
    w.get_mut::<Backpack>(p).unwrap().items.push(amulet);
    assert!(change_level(w, false), "the climb itself should succeed");
}

#[test]
fn a_ghost_only_shows_up_climbing_back_through_its_death_depth() {
    let _dir = BonesDir::enter();
    let depth = 201;
    let _ = std::fs::remove_file(format!("bones-{depth}.sav"));
    die_and_leave_bones(depth, "VICTIM");

    let mut w = test_world(2, "HERO");
    climb_into(&mut w, depth);

    assert!(
        ghost_named(&mut w, "VICTIM").is_some(),
        "the ghost should be waiting on the depth its owner died on"
    );
}

#[test]
fn descending_through_a_death_depth_never_wakes_the_ghost() {
    let _dir = BonesDir::enter();
    let depth = 202;
    let _ = std::fs::remove_file(format!("bones-{depth}.sav"));
    die_and_leave_bones(depth, "VICTIM2");

    let mut w = test_world(3, "HERO2");
    let p = player(&mut w);
    w.resource_mut::<Depth>().what = depth - 1;
    let down = w
        .resource::<Map>()
        .tiles
        .iter()
        .position(|&t| t == TileType::Downstairs)
        .expect("every floor has a down-stair");
    w.get_mut::<Position>(p).unwrap().x = (down % MAP_WIDTH as usize) as u16;
    w.get_mut::<Position>(p).unwrap().y = (down / MAP_WIDTH as usize) as u16;
    assert!(
        change_level(&mut w, true),
        "the descent itself should succeed"
    );

    assert!(
        ghost_named(&mut w, "VICTIM2").is_none(),
        "descending must never trigger the encounter, only ascending"
    );
    assert!(
        bones::take(depth).is_some(),
        "descending must not consume the bones file"
    );
}

#[test]
fn the_ghost_carries_its_gear_back_cursed_and_worn() {
    let _dir = BonesDir::enter();
    let depth = 203;
    let _ = std::fs::remove_file(format!("bones-{depth}.sav"));
    die_and_leave_bones(depth, "VICTIM3");

    let mut w = test_world(4, "HERO3");
    climb_into(&mut w, depth);
    let ghost = ghost_named(&mut w, "VICTIM3").expect("the ghost spawned");

    let fighter = w.get::<Fighter>(ghost).unwrap();
    assert_eq!(fighter.hp, START_HP);
    assert_eq!(fighter.power, START_POWER);
    assert_eq!(fighter.armor, START_ARMOR);

    let worn = equipment::equipped_items(&w, ghost);
    assert!(
        worn.iter()
            .any(|&e| w.get::<Name>(e).unwrap().what == "long sword"
                && w.get::<Curse>(e).is_some()),
        "the sword came back worn and cursed"
    );
    let ghost_pos = *w.get::<Position>(ghost).unwrap();
    let potion_here = w
        .query::<(&Name, &Position)>()
        .iter(&w)
        .any(|(n, p)| n.what == "potion of healing" && *p == ghost_pos);
    assert!(
        potion_here,
        "the potion came back loose on the ghost's own tile"
    );
}

#[test]
fn a_ghost_sharing_the_climber_s_own_name_is_marked_as_them() {
    let _dir = BonesDir::enter();
    let depth = 204;
    let _ = std::fs::remove_file(format!("bones-{depth}.sav"));
    die_and_leave_bones(depth, "SAME");

    let mut w = test_world(5, "SAME");
    climb_into(&mut w, depth);
    let ghost = ghost_named(&mut w, "SAME").expect("the ghost spawned");

    assert!(w.get::<GhostOfPlayer>(ghost).is_some());
}

#[test]
fn a_ghost_with_a_different_name_is_not_marked_as_the_player() {
    let _dir = BonesDir::enter();
    let depth = 205;
    let _ = std::fs::remove_file(format!("bones-{depth}.sav"));
    die_and_leave_bones(depth, "STRANGER");

    let mut w = test_world(6, "HERO5");
    climb_into(&mut w, depth);
    let ghost = ghost_named(&mut w, "STRANGER").expect("the ghost spawned");

    assert!(w.get::<GhostOfPlayer>(ghost).is_none());
}
