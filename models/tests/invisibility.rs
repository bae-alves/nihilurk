//! The phantom and the ring of perception: invisible monsters stay unseen (and
//! attack as "Something") until a perception ring turns them up, which also
//! reveals hidden traps in view and any invisibly-stashed item.

use bevy_ecs::prelude::*;
use bevy_ecs::schedule::Schedule;
use models::*;

fn test_world(seed: u64) -> World {
    let mut w = World::new();
    w.insert_resource(GameRng(ChaCha12Rng::seed_from_u64(seed)));
    w.insert_resource(RngSeed(seed));
    w.init_resource::<GameLog>();
    w.init_resource::<AttackQueue>();
    w.init_resource::<Ending>();
    w.insert_resource(PlayerName {
        what: "TESTER".into(),
    });
    initialize_world(&mut w);
    // Start from a clean floor so a test only sees what it plants.
    let traps: Vec<Entity> = w.query_filtered::<Entity, With<Trap>>().iter(&w).collect();
    for t in traps {
        w.entity_mut(t).despawn();
    }
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
    for (dx, dy) in [(1i32, 0i32), (-1, 0), (0, 1), (0, -1), (1, 1), (-1, -1)] {
        let (nx, ny) = (here.x as i32 + dx, here.y as i32 + dy);
        if nx >= 0 && ny >= 0 && !map.blocks(nx as u16, ny as u16) {
            return Position {
                x: nx as u16,
                y: ny as u16,
            };
        }
    }
    panic!("player is walled in");
}

fn run_visibility(w: &mut World) {
    let p = player(w);
    w.get_mut::<Viewshed>(p).unwrap().dirty = true;
    let mut s = Schedule::default();
    s.add_systems(visibility_system);
    s.run(w);
}

fn wear_ring(w: &mut World, p: Entity, effect: RingEffect) -> Entity {
    let ring = spawn_ring(w, effect, Position { x: 0, y: 0 });
    w.entity_mut(ring).remove::<Position>();
    w.get_mut::<Backpack>(p).unwrap().items.push(ring);
    w.get_mut::<Equipped>(ring).unwrap().by = Some(p);
    sync_equipment_effects(w, p);
    ring
}

fn log_has(w: &World, needle: &str) -> bool {
    w.resource::<GameLog>()
        .history
        .iter()
        .any(|l| l.contains(needle))
}

#[test]
fn the_phantom_is_born_invisible() {
    let mut w = test_world(1);
    let spot = beside_player(&mut w);
    let phantom = spawn_monster(&mut w, MonsterDef::named("phantom"), spot);
    assert!(w.get::<Invisible>(phantom).is_some());

    // A plain orc, right next to it, is not.
    let orc = spawn_monster(&mut w, MonsterDef::named("orc"), spot);
    assert!(w.get::<Invisible>(orc).is_none());
}

#[test]
fn an_invisible_phantom_in_view_stays_hidden_and_unannounced() {
    let mut w = test_world(1);
    let spot = beside_player(&mut w);
    let phantom = spawn_monster(&mut w, MonsterDef::named("phantom"), spot);

    run_visibility(&mut w);

    assert!(
        w.get::<Hidden>(phantom).is_some(),
        "invisible, so not drawn"
    );
    assert!(w.get::<Spotted>(phantom).is_none());
    assert!(!log_has(&w, "phantom"), "never announced");
}

#[test]
fn a_ring_of_perception_turns_up_the_phantom() {
    let mut w = test_world(1);
    let p = player(&mut w);
    let spot = beside_player(&mut w);
    let phantom = spawn_monster(&mut w, MonsterDef::named("phantom"), spot);
    wear_ring(&mut w, p, RingEffect::Perception);

    run_visibility(&mut w);

    assert!(w.get::<Hidden>(phantom).is_none(), "the ring reveals it");
    assert!(w.get::<Spotted>(phantom).is_some());
    assert!(log_has(&w, "You spotted a phantom"));
}

#[test]
fn an_unseen_attacker_is_only_ever_something() {
    let mut w = test_world(2);
    let p = player(&mut w);
    // Strip the starting armour so every blow lands and logs a damage line.
    for item in equipped_items(&w, p) {
        force_unequip(&mut w, item);
    }
    w.get_mut::<Fighter>(p).unwrap().hp = 500;
    w.get_mut::<Fighter>(p).unwrap().max_hp = 500;
    w.get_mut::<Fighter>(p).unwrap().armor = 0;
    w.get_mut::<Fighter>(p).unwrap().armor_bonus = 0;
    let spot = beside_player(&mut w);
    let phantom = spawn_monster(&mut w, MonsterDef::named("phantom"), spot);

    // Visibility marks the in-view-but-invisible phantom Hidden.
    run_visibility(&mut w);
    assert!(w.get::<Hidden>(phantom).is_some());

    resolve_attack(&mut w, phantom, p);
    assert!(
        log_has(&w, "Something hits you for"),
        "no name for the unseen"
    );
    assert!(!log_has(&w, "phantom hits you"));

    // Now perceive it: the same phantom attacks by name.
    wear_ring(&mut w, p, RingEffect::Perception);
    run_visibility(&mut w);
    assert!(w.get::<Hidden>(phantom).is_none());
    resolve_attack(&mut w, phantom, p);
    assert!(log_has(&w, "phantom hits you for"));
}

#[test]
fn perception_reveals_traps_in_view_only() {
    let mut w = test_world(3);
    let p = player(&mut w);

    // Two triggered traps: neither reveals itself by sight alone.
    let far = out_of_view(&mut w);
    let far_trap = w.spawn(TrapBundle::dart(far)).id();
    w.get_mut::<Trap>(far_trap).unwrap().reveal = TrapReveal::Triggered;
    let near = beside_player(&mut w);
    let near_trap = w.spawn(TrapBundle::dart(near)).id();
    w.get_mut::<Trap>(near_trap).unwrap().reveal = TrapReveal::Triggered;

    run_visibility(&mut w);
    assert!(
        w.get::<Hidden>(far_trap).is_some(),
        "hidden without the ring"
    );
    assert!(
        w.get::<Hidden>(near_trap).is_some(),
        "hidden without the ring"
    );

    wear_ring(&mut w, p, RingEffect::Perception);
    run_visibility(&mut w);
    assert!(
        w.get::<Hidden>(near_trap).is_none(),
        "the ring lays it bare"
    );
    assert!(w.get::<Trap>(near_trap).unwrap().revealed);
    assert!(log_has(&w, "You spot"));

    assert!(
        w.get::<Hidden>(far_trap).is_some(),
        "second sight still stops at the viewshed"
    );
    assert!(!w.get::<Trap>(far_trap).unwrap().revealed);
}

#[test]
fn an_invisible_item_hides_until_perception_or_a_misstep() {
    let mut w = test_world(4);
    let p = player(&mut w);
    let spot = beside_player(&mut w);
    let item = spawn_random_item_for_test(&mut w, spot);
    w.entity_mut(item).insert((Hidden, Invisible));

    run_visibility(&mut w);
    assert!(w.get::<Hidden>(item).is_some(), "still stashed");
    assert!(!log_has(&w, "something here"));
    assert!(w.get::<Spotted>(item).is_none(), "never announced");

    wear_ring(&mut w, p, RingEffect::Perception);
    run_visibility(&mut w);
    assert!(w.get::<Hidden>(item).is_none());
    assert!(w.get::<Invisible>(item).is_none());
    assert!(log_has(&w, "Hey! There's something here!"));
}

/// An open floor tile the player cannot currently see.
fn out_of_view(w: &mut World) -> Position {
    run_visibility(w);
    let p = player(w);
    let seen: Vec<(u16, u16)> = w.get::<Viewshed>(p).unwrap().visible_tiles.clone();
    let map = w.resource::<Map>();
    for y in 0..MAP_HEIGHT {
        for x in 0..MAP_WIDTH {
            if !map.blocks(x, y) && !seen.contains(&(x, y)) {
                return Position { x, y };
            }
        }
    }
    panic!("the whole floor is in view");
}

/// A stand-in floor item: `spawn_random_item` is private, so just drop a scroll.
fn spawn_random_item_for_test(w: &mut World, pos: Position) -> Entity {
    spawn_scroll(w, ScrollEffect::Identify, pos)
}
