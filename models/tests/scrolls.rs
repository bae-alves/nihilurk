//! The "back half" of the scroll table: teleportation, aggravate monsters,
//! scare monster, create monster, and vorpalize weapon (plus the rule that a
//! glancing blow can never be the killing one).

use bevy_ecs::prelude::*;
use bevy_ecs::schedule::Schedule;
use models::*;

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

/// Pull `item` out of the pack, queue it, run the item system — the engine's
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

fn stash(w: &mut World, user: Entity, item: Entity) {
    w.entity_mut(item).remove::<Position>();
    w.get_mut::<Backpack>(user).unwrap().items.push(item);
}

fn spawn_dummy(w: &mut World, name: &str, x: u16, y: u16, hp: i32, mv: MovementType) -> Entity {
    w.spawn((
        Name { what: name.into() },
        Mob { movement_type: mv },
        Position { x, y },
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
    ))
    .id()
}

fn run_ai(w: &mut World) {
    let mut s = Schedule::default();
    s.add_systems(ai);
    s.run(w);
}

// ---------------------------------------------------------------------------
// Teleportation
// ---------------------------------------------------------------------------

#[test]
fn teleportation_drops_the_reader_somewhere_else_on_the_floor() {
    let mut w = test_world(7);
    let p = player(&mut w);
    let before = *w.get::<Position>(p).unwrap();
    w.get_mut::<Viewshed>(p).unwrap().dirty = false;

    let scroll = spawn_scroll(&mut w, ScrollEffect::Teleportation, Position { x: 0, y: 0 });
    stash(&mut w, p, scroll);
    use_item(&mut w, p, scroll);

    let after = *w.get::<Position>(p).unwrap();
    assert!(
        (after.x, after.y) != (before.x, before.y),
        "the reader should have moved"
    );
    assert!(
        w.get::<Viewshed>(p).unwrap().dirty,
        "the viewshed must be recomputed after a blink"
    );
    assert!(
        w.resource::<Identified>()
            .scrolls
            .contains(&ScrollEffect::Teleportation)
    );
    // Landed on a real walkable tile, not inside a wall.
    assert!(!w.resource::<Map>().blocks(after.x, after.y));
}

// ---------------------------------------------------------------------------
// Aggravate monsters
// ---------------------------------------------------------------------------

#[test]
fn aggravate_turns_every_monster_into_a_hunter_that_closes_in_unseen() {
    let mut w = test_world(1);
    let p = player(&mut w);
    let hero = *w.get::<Position>(p).unwrap();

    // A monster that normally never moves, parked a few tiles down the same row
    // and outside the hero's view.
    let mob_start = {
        let map = w.resource::<Map>();
        (2..8)
            .map(|d| (hero.x + d, hero.y))
            .find(|&(x, y)| !map.blocks(x, y))
            .unwrap()
    };
    let mob = spawn_dummy(
        &mut w,
        "orc",
        mob_start.0,
        mob_start.1,
        3,
        MovementType::Static,
    );
    w.get_mut::<Viewshed>(p).unwrap().visible_tiles = vec![(hero.x, hero.y)];

    let scroll = spawn_scroll(
        &mut w,
        ScrollEffect::AggravateMonsters,
        Position { x: 0, y: 0 },
    );
    stash(&mut w, p, scroll);
    use_item(&mut w, p, scroll);

    // Marked as an aggravated hunter, homing on the tile the hero read it from.
    match w.get::<Mob>(mob).unwrap().movement_type {
        MovementType::Aggravated { tx, ty } => assert_eq!((tx, ty), (hero.x, hero.y)),
        _ => panic!("aggravate should switch the mob to Aggravated"),
    }

    // Even though it is a "Static" bundle and out of sight, it now advances.
    let dist_before = mob_start.0 - hero.x;
    for _ in 0..3 {
        run_ai(&mut w);
    }
    let now = *w.get::<Position>(mob).unwrap();
    assert!(
        hero.x.abs_diff(now.x) < dist_before,
        "an aggravated monster should have closed the distance (was {dist_before}, now {})",
        hero.x.abs_diff(now.x)
    );
}

// ---------------------------------------------------------------------------
// Scare monster
// ---------------------------------------------------------------------------

#[test]
fn scare_monster_routs_what_you_can_see_and_leaves_the_rest_alone() {
    let mut w = test_world(3);
    let p = player(&mut w);
    let hero = *w.get::<Position>(p).unwrap();

    let seen = spawn_dummy(&mut w, "troll", hero.x + 1, hero.y, 4, MovementType::Chase);
    let unseen = spawn_dummy(&mut w, "troll", hero.x + 2, hero.y, 4, MovementType::Chase);
    w.get_mut::<Viewshed>(p).unwrap().visible_tiles = vec![(hero.x, hero.y), (hero.x + 1, hero.y)];

    let scroll = spawn_scroll(&mut w, ScrollEffect::ScareMonster, Position { x: 0, y: 0 });
    stash(&mut w, p, scroll);
    use_item(&mut w, p, scroll);

    assert!(
        matches!(
            w.get::<Mob>(seen).unwrap().movement_type,
            MovementType::Flee
        ),
        "a monster in view is scared off"
    );
    assert!(
        matches!(
            w.get::<Mob>(unseen).unwrap().movement_type,
            MovementType::Chase
        ),
        "a monster out of view is untouched"
    );
}

// ---------------------------------------------------------------------------
// Create monster
// ---------------------------------------------------------------------------

#[test]
fn create_monster_conjures_a_fresh_creature_on_the_floor() {
    let mut w = test_world(5);
    let p = player(&mut w);

    let before = w.query_filtered::<(), With<Mob>>().iter(&w).count();

    let scroll = spawn_scroll(&mut w, ScrollEffect::CreateMonster, Position { x: 0, y: 0 });
    stash(&mut w, p, scroll);
    use_item(&mut w, p, scroll);

    let after = w.query_filtered::<(), With<Mob>>().iter(&w).count();
    assert_eq!(after, before + 1, "exactly one monster is added");

    // It stands on a real tile that isn't the player's.
    let hero = *w.get::<Position>(p).unwrap();
    let mut q = w.query_filtered::<(&Position, &Faction), With<Mob>>();
    let placed_ok = q
        .iter(&w)
        .any(|(pos, f)| *f == Faction::Monster && (pos.x, pos.y) != (hero.x, hero.y));
    assert!(placed_ok);
}

// ---------------------------------------------------------------------------
// Vorpalize weapon
// ---------------------------------------------------------------------------

/// Give the player a wielded weapon and return its entity.
fn wield_a_blade(w: &mut World, p: Entity) -> Entity {
    // Put down the starting mace first — this blade is the only thing in hand.
    for item in equipped_items(w, p) {
        force_unequip(w, item);
    }
    let sword = spawn_weapon(w, "long sword", Position { x: 0, y: 0 });
    w.entity_mut(sword).remove::<Position>();
    w.get_mut::<Backpack>(p).unwrap().items.push(sword);
    w.get_mut::<Equipped>(sword).unwrap().by = Some(p);
    sword
}

#[test]
fn vorpalize_brands_the_wielded_blade_and_names_a_bane() {
    let mut w = test_world(11);
    let p = player(&mut w);
    let sword = wield_a_blade(&mut w, p);

    let scroll = spawn_scroll(
        &mut w,
        ScrollEffect::VorpalizeWeapon,
        Position { x: 0, y: 0 },
    );
    stash(&mut w, p, scroll);
    use_item(&mut w, p, scroll);

    let bane = w
        .get::<Vorpal>(sword)
        .expect("the blade is now vorpal")
        .bane
        .clone();
    assert!(!bane.is_empty());
}

#[test]
fn vorpalizing_an_already_vorpal_weapon_crumbles_it() {
    let mut w = test_world(11);
    let p = player(&mut w);
    let sword = wield_a_blade(&mut w, p);
    w.entity_mut(sword).insert(Vorpal { bane: "orc".into() });

    let scroll = spawn_scroll(
        &mut w,
        ScrollEffect::VorpalizeWeapon,
        Position { x: 0, y: 0 },
    );
    stash(&mut w, p, scroll);
    use_item(&mut w, p, scroll);

    assert!(
        !w.entities().contains(sword),
        "a twice-vorpalized blade is destroyed"
    );
    assert!(
        !w.get::<Backpack>(p).unwrap().items.contains(&sword),
        "…and gone from the pack"
    );
    assert!(
        w.resource::<GameLog>()
            .history
            .iter()
            .any(|l| l.contains("crumbles to dust"))
    );
}

#[test]
fn vorpalize_with_empty_hands_just_fizzles() {
    let mut w = test_world(11);
    let p = player(&mut w);
    for item in equipped_items(&w, p) {
        force_unequip(&mut w, item);
    }

    let scroll = spawn_scroll(
        &mut w,
        ScrollEffect::VorpalizeWeapon,
        Position { x: 0, y: 0 },
    );
    stash(&mut w, p, scroll);
    use_item(&mut w, p, scroll);

    // Nothing to brand — and no weapon quietly grew a Vorpal tag.
    assert_eq!(w.query::<&Vorpal>().iter(&w).count(), 0);
}

#[test]
fn a_vorpal_blade_beheads_its_bane_in_one_blow() {
    let mut w = test_world(1);
    let p = player(&mut w);
    let sword = wield_a_blade(&mut w, p);
    w.entity_mut(sword).insert(Vorpal { bane: "orc".into() });
    // Big power, zero enemy armour: every hit deals damage and never glances.
    w.get_mut::<Fighter>(p).unwrap().power = 100;

    let hero = *w.get::<Position>(p).unwrap();
    let orc = spawn_dummy(&mut w, "orc", hero.x + 1, hero.y, 999, MovementType::Static);

    resolve_attack(&mut w, p, orc);

    assert!(
        !w.entities().contains(orc),
        "the bane is slain outright despite its 999 HP"
    );
    assert!(
        w.resource::<GameLog>()
            .history
            .iter()
            .any(|l| l.contains("Snicker-snack"))
    );
}

#[test]
fn every_vorpal_blade_beheads_a_jabberwock_whatever_its_bane() {
    let mut w = test_world(1);
    let p = player(&mut w);
    let sword = wield_a_blade(&mut w, p);
    w.entity_mut(sword).insert(Vorpal { bane: "orc".into() }); // bane is NOT the jabberwock
    w.get_mut::<Fighter>(p).unwrap().power = 100;

    let hero = *w.get::<Position>(p).unwrap();
    let jab = spawn_monster(
        &mut w,
        MonsterDef::named("jabberwock"),
        Position {
            x: hero.x + 1,
            y: hero.y,
        },
    );
    w.get_mut::<Fighter>(jab).unwrap().hp = 999;

    resolve_attack(&mut w, p, jab);
    assert!(
        !w.entities().contains(jab),
        "any vorpal weapon fells the Jabberwock"
    );
}

#[test]
fn a_vorpal_blade_is_just_a_blade_against_anything_else() {
    let mut w = test_world(1);
    let p = player(&mut w);
    let sword = wield_a_blade(&mut w, p);
    w.entity_mut(sword).insert(Vorpal { bane: "orc".into() });
    w.get_mut::<Fighter>(p).unwrap().power = 100;

    let hero = *w.get::<Position>(p).unwrap();
    let bat = spawn_dummy(&mut w, "bat", hero.x + 1, hero.y, 999, MovementType::Static);

    resolve_attack(&mut w, p, bat);
    assert!(
        w.entities().contains(bat),
        "a non-bane, non-jabberwock takes ordinary damage"
    );
    assert!(
        w.get::<Fighter>(bat).unwrap().hp < 999,
        "…ordinary damage, but damage all the same"
    );
}

// ---------------------------------------------------------------------------
// Glancing blows can never kill
// ---------------------------------------------------------------------------

#[test]
fn a_glancing_blow_chips_a_foe_down_to_one_but_never_finishes_it() {
    let mut w = test_world(4);
    let p = player(&mut w);
    // Feeble hero, heavily armoured target: every hit is a chip-damage glance.
    for item in equipped_items(&w, p) {
        force_unequip(&mut w, item);
    }
    w.get_mut::<Fighter>(p).unwrap().power = 1;
    w.get_mut::<Fighter>(p).unwrap().power_bonus = 0;

    let hero = *w.get::<Position>(p).unwrap();
    let foe = w
        .spawn((
            Name {
                what: "wall of a monster".into(),
            },
            Mob {
                movement_type: MovementType::Static,
            },
            Position {
                x: hero.x + 1,
                y: hero.y,
            },
            Fighter {
                hp: 5,
                max_hp: 5,
                armor: 2,
                power: 1,
                max_power: 1,
                armor_bonus: 10,
                power_bonus: 0,
            },
            Faction::Monster,
            Blood,
        ))
        .id();

    for _ in 0..40 {
        resolve_attack(&mut w, p, foe);
    }

    assert!(
        w.entities().contains(foe),
        "a string of glancing blows never kills"
    );
    assert_eq!(
        w.get::<Fighter>(foe).unwrap().hp,
        1,
        "they chip it to 1 HP and no further"
    );
    assert!(
        w.resource::<GameLog>()
            .history
            .iter()
            .any(|l| l.contains("glancing blow"))
    );
}
