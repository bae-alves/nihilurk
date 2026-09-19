//! The scroll table, now that every row does something: teleportation,
//! aggravate monsters, scare monster, create monster and vorpalize weapon (plus
//! the rule that a glancing blow can never be the killing one), then the six
//! that used to be readable and inert — the two enchantments, monster
//! confusion, hold monster, sleep and food detection.

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

/// Put a fresh scroll of `effect` in `user`'s pack and read it.
fn read(w: &mut World, user: Entity, effect: ScrollEffect) {
    let scroll = spawn_scroll(w, effect, Position { x: 0, y: 0 });
    stash(w, user, scroll);
    use_item(w, user, scroll);
}

fn logged(w: &World, needle: &str) -> bool {
    w.resource::<GameLog>()
        .history
        .iter()
        .any(|l| l.contains(needle))
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
fn vorpalize_with_a_bow_in_hand_also_just_fizzles() {
    let mut w = test_world(11);
    let p = player(&mut w);
    for item in equipped_items(&w, p) {
        force_unequip(&mut w, item);
    }
    let bow = spawn_launcher(&mut w, "short bow", Position { x: 0, y: 0 });
    stash(&mut w, p, bow);
    toggle_equipped(&mut w, p, bow);

    let scroll = spawn_scroll(
        &mut w,
        ScrollEffect::VorpalizeWeapon,
        Position { x: 0, y: 0 },
    );
    stash(&mut w, p, scroll);
    use_item(&mut w, p, scroll);

    // A launcher has no edge to enchant — the scroll fizzles as if the hand
    // were empty, and the bow stays a plain bow.
    assert_eq!(w.query::<&Vorpal>().iter(&w).count(), 0);
    assert!(
        w.resource::<GameLog>()
            .history
            .iter()
            .any(|l| l.contains("gutters out"))
    );
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
    // Seed picked to roll no excellent hit across the 40 swings below. An
    // excellent hit is not a glancing one — see
    // `an_excellent_hit_can_finish_a_foe_a_glancing_blow_could_not` — so a
    // seed that rolled one here would (correctly) kill the target partway
    // through and this test would no longer be exercising a pure string of
    // glancing blows.
    let mut w = test_world(564);
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

// ---------------------------------------------------------------------------
// Enchant weapon / enchant armor
// ---------------------------------------------------------------------------

#[test]
fn enchant_weapon_adds_a_plus_to_the_blade_in_hand() {
    let mut w = test_world(2);
    let p = player(&mut w);
    let sword = wield_a_blade(&mut w, p);

    read(&mut w, p, ScrollEffect::EnchantWeapon);

    assert_eq!(
        w.get::<PowerBonus>(sword).map(|b| b.0),
        Some(1),
        "a plain blade comes out +1"
    );
    assert!(
        w.get::<KnownQuality>(sword).is_some(),
        "you watched it take — there is nothing left to be coy about"
    );
    assert!(logged(&w, "orange sparks"));
}

#[test]
fn enchant_weapon_mends_a_minus_all_the_way_to_plus_zero_and_burns_the_curse_off() {
    let mut w = test_world(2);
    let p = player(&mut w);
    let sword = wield_a_blade(&mut w, p);
    w.entity_mut(sword).insert(PowerBonus(-4));
    w.entity_mut(sword).insert(Curse);

    read(&mut w, p, ScrollEffect::EnchantWeapon);

    assert_eq!(
        w.get::<PowerBonus>(sword).map(|b| b.0),
        Some(0),
        "a minus is mended whole, not nudged one point"
    );
    assert!(
        w.get::<Curse>(sword).is_none(),
        "…and the curse goes with it, without destroying the blade"
    );
    assert!(logged(&w, "curse on the long sword burns away"));
}

#[test]
fn enchant_armor_raises_the_armour_actually_being_worn() {
    let mut w = test_world(2);
    let p = player(&mut w);
    let worn = equipped_in(&w, p, Slot::Body).expect("the hero starts in ring mail");
    let before = w.get::<ArmorBonus>(worn).map_or(0, |b| b.0);

    read(&mut w, p, ScrollEffect::EnchantArmor);

    assert_eq!(w.get::<ArmorBonus>(worn).map(|b| b.0), Some(before + 1));
}

#[test]
fn enchanting_a_bow_lands_on_the_throw_the_way_the_dungeon_rolls_one() {
    let mut w = test_world(2);
    let p = player(&mut w);
    for item in equipped_items(&w, p) {
        force_unequip(&mut w, item);
    }
    let bow = spawn_launcher(&mut w, "short bow", Position { x: 0, y: 0 });
    stash(&mut w, p, bow);
    toggle_equipped(&mut w, p, bow);

    read(&mut w, p, ScrollEffect::EnchantWeapon);

    assert_eq!(
        w.get::<ThrowBonus>(bow).map(|b| b.0),
        Some(1),
        "a bow's plus steadies its aim"
    );
    assert!(
        w.get::<PowerBonus>(bow).is_none(),
        "a bow has no melee roll for a plus to land on"
    );
}

#[test]
fn enchanting_with_nothing_in_the_slot_just_gutters_out() {
    let mut w = test_world(2);
    let p = player(&mut w);
    for item in equipped_items(&w, p) {
        force_unequip(&mut w, item);
    }

    read(&mut w, p, ScrollEffect::EnchantArmor);

    assert!(logged(&w, "bare skin"));
}

// ---------------------------------------------------------------------------
// Monster confusion
// ---------------------------------------------------------------------------

#[test]
fn monster_confusion_charges_the_hands_and_the_next_landed_blow_spends_them() {
    let mut w = test_world(1);
    let p = player(&mut w);

    read(&mut w, p, ScrollEffect::MonsterConfusion);
    assert!(
        w.get::<ConfusingTouch>(p).is_some(),
        "the charm waits on the hands rather than going off now"
    );

    // Big power against a fat, unarmoured target: the blow lands, and can't kill.
    w.get_mut::<Fighter>(p).unwrap().power = 20;
    let hero = *w.get::<Position>(p).unwrap();
    let orc = spawn_dummy(&mut w, "orc", hero.x + 1, hero.y, 999, MovementType::Chase);

    resolve_attack(&mut w, p, orc);

    assert!(
        matches!(
            w.get::<Mob>(orc).unwrap().movement_type,
            MovementType::Confused
        ),
        "what you hit reels"
    );
    assert!(
        w.get::<ConfusingTouch>(p).is_none(),
        "and the charge is spent doing it"
    );
}

#[test]
fn a_glancing_scrape_never_passes_the_charm_on() {
    // The seed `a_glancing_blow_chips_a_foe...` uses: no excellent hit, so this
    // single swing really is the glancing one the test is about.
    let mut w = test_world(564);
    let p = player(&mut w);
    for item in equipped_items(&w, p) {
        force_unequip(&mut w, item);
    }
    w.get_mut::<Fighter>(p).unwrap().power = 1;
    w.get_mut::<Fighter>(p).unwrap().power_bonus = 0;

    read(&mut w, p, ScrollEffect::MonsterConfusion);

    let hero = *w.get::<Position>(p).unwrap();
    let wall = spawn_dummy(&mut w, "orc", hero.x + 1, hero.y, 30, MovementType::Chase);
    w.get_mut::<Fighter>(wall).unwrap().armor_bonus = 10;

    resolve_attack(&mut w, p, wall);

    assert!(
        w.get::<ConfusingTouch>(p).is_some(),
        "a scrape off armour never gets close enough to pass the charm on"
    );
    assert!(matches!(
        w.get::<Mob>(wall).unwrap().movement_type,
        MovementType::Chase
    ));
}

// ---------------------------------------------------------------------------
// Hold monster
// ---------------------------------------------------------------------------

#[test]
fn hold_monster_roots_what_you_can_see_and_leaves_the_rest_alone() {
    let mut w = test_world(3);
    let p = player(&mut w);
    let hero = *w.get::<Position>(p).unwrap();

    let seen = spawn_dummy(&mut w, "troll", hero.x + 1, hero.y, 4, MovementType::Chase);
    let unseen = spawn_dummy(&mut w, "troll", hero.x + 2, hero.y, 4, MovementType::Chase);
    w.get_mut::<Viewshed>(p).unwrap().visible_tiles = vec![(hero.x, hero.y), (hero.x + 1, hero.y)];

    read(&mut w, p, ScrollEffect::HoldMonster);

    assert!(
        w.get::<Rooted>(seen).is_some(),
        "a monster in sight is bound"
    );
    assert!(
        turns_left(&w, seen, Grant::of::<Rooted>()).is_some_and(|n| n > 0),
        "bound for no turns at all"
    );
    assert!(
        w.get::<Rooted>(unseen).is_none(),
        "a monster out of view is untouched"
    );
}

#[test]
fn a_held_monster_cannot_step_but_still_bites_what_comes_in_reach() {
    let mut w = test_world(3);
    let p = player(&mut w);
    let hero = *w.get::<Position>(p).unwrap();

    // One two tiles off (it would close), one already in reach (it would bite).
    let far = spawn_dummy(&mut w, "troll", hero.x + 2, hero.y, 4, MovementType::Chase);
    let near = spawn_dummy(&mut w, "troll", hero.x, hero.y + 1, 4, MovementType::Chase);
    w.get_mut::<Viewshed>(p).unwrap().visible_tiles =
        vec![(hero.x, hero.y), (hero.x + 2, hero.y), (hero.x, hero.y + 1)];

    read(&mut w, p, ScrollEffect::HoldMonster);
    let stood = *w.get::<Position>(far).unwrap();
    run_ai(&mut w);

    assert_eq!(
        *w.get::<Position>(far).unwrap(),
        stood,
        "rooted where it stands"
    );
    assert!(
        w.resource::<AttackQueue>()
            .attacks
            .iter()
            .any(|a| a.attacker == near),
        "…but a held monster is pinned, not helpless: in reach, it still bites"
    );
}

// ---------------------------------------------------------------------------
// Sleep
// ---------------------------------------------------------------------------

/// Reads a scroll of sleep with one monster in view, and reports whether the
/// room went down (`false`) or the reader did (`true`).
fn read_sleep_once(seed: u64) -> bool {
    let mut w = test_world(seed);
    let p = player(&mut w);
    let hero = *w.get::<Position>(p).unwrap();
    let mob = spawn_dummy(&mut w, "rat", hero.x + 1, hero.y, 4, MovementType::Chase);
    w.get_mut::<Viewshed>(p).unwrap().visible_tiles = vec![(hero.x, hero.y), (hero.x + 1, hero.y)];

    read(&mut w, p, ScrollEffect::Sleep);

    let reader_out = w.get::<Asleep>(p).is_some();
    let room_out = w.get::<Asleep>(mob).is_some();
    assert!(
        reader_out != room_out,
        "seed {seed}: the scroll takes the room or the reader, never both and never neither"
    );
    reader_out
}

#[test]
fn sleep_usually_takes_the_room_and_sometimes_the_reader() {
    let backfires = (0..40u64).filter(|&s| read_sleep_once(s)).count();
    assert!(
        backfires > 0,
        "the backfire is the whole gamble — it has to actually happen"
    );
    assert!(
        backfires * 2 < 40,
        "…but it is the minority case (backfired {backfires} times in 40)"
    );
}

#[test]
fn a_sleeping_monster_forfeits_its_turn_outright() {
    let mut w = test_world(3);
    let p = player(&mut w);
    let hero = *w.get::<Position>(p).unwrap();
    let mob = spawn_dummy(&mut w, "rat", hero.x, hero.y + 1, 4, MovementType::Chase);
    hold(&mut w, mob, Grant::of::<Asleep>(), 3);

    run_ai(&mut w);

    assert!(
        w.resource::<AttackQueue>().attacks.is_empty(),
        "asleep in reach of the player and it still does nothing"
    );
}

// ---------------------------------------------------------------------------
// Food detection, and the one thing both detections find
// ---------------------------------------------------------------------------

#[test]
fn food_detection_turns_up_the_plain_things_and_leaves_the_magic_alone() {
    let mut w = test_world(2);
    let p = player(&mut w);
    let spot = Position { x: 1, y: 1 };

    let plain = spawn_weapon(&mut w, "dagger", spot);
    let potion = spawn_potion(&mut w, PotionEffect::Healing, spot);
    let cursed = spawn_weapon(&mut w, "mace", spot);
    w.entity_mut(cursed).insert(Curse);

    read(&mut w, p, ScrollEffect::FoodDetection);

    assert!(
        w.get::<Detected>(plain).is_some(),
        "a plain +0 dagger is exactly what this scroll is for"
    );
    assert!(
        w.get::<Detected>(potion).is_none(),
        "a potion is the other scroll's business"
    );
    assert!(
        w.get::<Detected>(cursed).is_none(),
        "so is a cursed mace — a curse is magic, and being warned is the potion's job"
    );
}

#[test]
fn both_detections_find_the_element_of_yoord() {
    let spot = Position { x: 1, y: 1 };

    let mut w = test_world(2);
    let p = player(&mut w);
    let relic = spawn_element_of_yoord(&mut w, spot);
    read(&mut w, p, ScrollEffect::FoodDetection);
    assert!(
        w.get::<Detected>(relic).is_some(),
        "the scroll finds the relic"
    );

    let mut w = test_world(2);
    let p = player(&mut w);
    let relic = spawn_element_of_yoord(&mut w, spot);
    let potion = spawn_potion(
        &mut w,
        PotionEffect::MagicDetection,
        Position { x: 0, y: 0 },
    );
    stash(&mut w, p, potion);
    use_item(&mut w, p, potion);
    assert!(
        w.get::<Detected>(relic).is_some(),
        "and so does the potion — it is the run, and neither sense misses it"
    );
}

// ---------------------------------------------------------------------------
// The juice
// ---------------------------------------------------------------------------

/// The effect layer is optional — every test above runs without one, the way a
/// headless caller does. With one present the new scrolls have to actually
/// queue their flourish, which is the half those tests can't see.
#[test]
fn the_new_scrolls_queue_their_flourish_when_there_is_an_effect_layer() {
    let cases = [
        ScrollEffect::EnchantWeapon,
        ScrollEffect::EnchantArmor,
        ScrollEffect::MonsterConfusion,
        ScrollEffect::HoldMonster,
        ScrollEffect::Sleep,
        ScrollEffect::FoodDetection,
        ScrollEffect::Teleportation,
    ];
    for effect in cases {
        let mut w = test_world(6);
        w.insert_resource(Particles::new());
        let p = player(&mut w);
        let hero = *w.get::<Position>(p).unwrap();
        spawn_dummy(&mut w, "rat", hero.x + 1, hero.y, 4, MovementType::Chase);
        w.get_mut::<Viewshed>(p).unwrap().visible_tiles =
            vec![(hero.x, hero.y), (hero.x + 1, hero.y)];

        read(&mut w, p, effect);

        let fx = w.resource::<Particles>();
        assert!(
            fx.pending && fx.any_alive(),
            "{effect:?} should have queued something to look at"
        );
    }
}
