//! Missiles: the things in the pack that are *for* throwing.
//!
//! Four of them, and one idea. An arrow, a quarrel, a dagger and a dagger all
//! carry `Projectile` — they are spent on what they hit,
//! and nothing ever catches one out of the air. What separates them is two more
//! components: a dagger and a dagger are `Piercing` and run the whole line, while
//! an arrow and a quarrel are `Ammo` and roll their row's `launched_die` instead
//! of their hand-thrown die for anyone holding the bow or crossbow that answers
//! to it: 1d3 for an arrow, 1d5 for a quarrel, 1d2 (and venom) for a blowdart.
//!
//! Ammunition is also the only thing in the game that stacks, so half of this
//! file is about one pack slot holding thirteen arrows and giving them up one at a
//! time.

use crate::common;
use crate::common::monster;

use crate::common::fixtures::{spawn_ammo, spawn_launcher, spawn_named, spawn_ring, spawn_weapon};
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
    w
}

fn player(w: &mut World) -> Entity {
    w.query_filtered::<Entity, With<Player>>().single(w)
}

fn player_pos(w: &mut World) -> Position {
    let p = player(w);
    *w.get::<Position>(p).unwrap()
}

/// The tile an item is spawned on before it is stashed — never looked at.
const NOWHERE: Position = Position { x: 0, y: 0 };

/// Spawn something and put it in the player's pack, as picking it up would.
fn stash(w: &mut World, p: Entity, spawn: impl FnOnce(&mut World) -> Entity) -> Entity {
    let item = spawn(w);
    w.entity_mut(item).remove::<Position>();
    w.get_mut::<Backpack>(p).unwrap().items.push(item);
    item
}

/// Empties the player's starting kit, so a test that asserts on the shape of the
/// pack is asserting only about what it put there itself. The items are despawned,
/// not just unlisted, so a save written afterwards holds only what the test added.
fn empty_pack(w: &mut World, p: Entity) {
    let items = std::mem::take(&mut w.get_mut::<Backpack>(p).unwrap().items);
    for item in items {
        w.despawn(item);
    }
}

/// A pack slot holding `count` arrows (or quarrels).
fn quiver(w: &mut World, p: Entity, name: &str, count: u8) -> Entity {
    let ammo = stash(w, p, |w| spawn_ammo(w, name, NOWHERE));
    w.get_mut::<Stack>(ammo).unwrap().count = count;
    ammo
}

/// The engine's Throw action, in full: the throw names the item's pack row,
/// and `throw_system` lifts it (a stack gives up one of itself) and finds out
/// where the missile ends up.
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

/// A run of `n` open tiles leading away from the player, in whichever of the
/// four directions the floor happens to allow. Every throw here needs a clear
/// lane; which way it points is nobody's business.
fn open_run(w: &mut World, n: u16) -> Vec<Position> {
    let p = player_pos(w);
    let map = w.resource::<Map>().clone();
    for (dx, dy) in [(1i32, 0i32), (-1, 0), (0, 1), (0, -1)] {
        let run: Vec<Position> = (1..=n as i32)
            .map(|k| Position {
                x: (p.x as i32 + dx * k) as u16,
                y: (p.y as i32 + dy * k) as u16,
            })
            .collect();
        if run.iter().all(|q| !map.blocks(q.x, q.y)) {
            return run;
        }
    }
    panic!("no open run of {n} tiles from the player");
}

/// A monster of the given species with enough HP to survive being studied.
fn tough(w: &mut World, name: &str, at: Position) -> Entity {
    let m = monster::plain_monster(w, name, at);
    w.get_mut::<Fighter>(m).unwrap().hp = 500;
    w.get_mut::<Fighter>(m).unwrap().max_hp = 500;
    m
}

fn hp(w: &World, e: Entity) -> i32 {
    w.get::<Fighter>(e).unwrap().hp
}

fn logged(w: &World, needle: &str) -> bool {
    w.resource::<GameLog>()
        .history
        .iter()
        .any(|l| l.contains(needle))
}

/// Throws one piece of `ammo` at a punching bag `rounds` times and reports every
/// damage figure. `setup` runs once, on the player, before any of it — that is
/// where a bow gets wielded or a ring goes on.
fn damage_samples(
    seed: u64,
    ammo: &'static str,
    rounds: usize,
    setup: impl FnOnce(&mut World, Entity),
) -> Vec<i32> {
    let mut w = test_world(seed);
    let p = player(&mut w);
    setup(&mut w, p);
    let spot = open_run(&mut w, 1)[0];
    let bag = tough(&mut w, "bat", spot);
    w.get_mut::<Fighter>(bag).unwrap().hp = 100_000;

    (0..rounds)
        .map(|_| {
            let before = hp(&w, bag);
            let one = quiver(&mut w, p, ammo, 1);
            throw(&mut w, p, one, spot);
            before - hp(&w, bag)
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Launchers
// ---------------------------------------------------------------------------

#[test]
fn ammo_noun_names_whichever_the_drawn_launcher_takes() {
    let mut w = test_world(3);
    let p = player(&mut w);
    empty_pack(&mut w, p);

    let bow = stash(&mut w, p, |w| spawn_launcher(w, "short bow", NOWHERE));
    toggle_equipped(&mut w, p, bow);
    assert_eq!(ammo_noun(&w, p), "arrows");

    toggle_equipped(&mut w, p, bow);
    let crossbow = stash(&mut w, p, |w| spawn_launcher(w, "crossbow", NOWHERE));
    toggle_equipped(&mut w, p, crossbow);
    assert_eq!(ammo_noun(&w, p), "quarrels");
}

#[test]
fn firing_a_quarrel_from_a_crossbow_says_fire_not_throw() {
    let mut w = test_world(2);
    let p = player(&mut w);
    let spot = open_run(&mut w, 1)[0];

    let bow = stash(&mut w, p, |w| spawn_launcher(w, "short bow", NOWHERE));
    toggle_equipped(&mut w, p, bow);
    let mishandled = quiver(&mut w, p, "quarrel", 1);
    throw(&mut w, p, mishandled, spot);
    assert!(
        logged(&w, "You throw the quarrel."),
        "wrong launcher, still a throw: {:?}",
        w.resource::<GameLog>().history
    );

    let crossbow = stash(&mut w, p, |w| spawn_launcher(w, "crossbow", NOWHERE));
    toggle_equipped(&mut w, p, crossbow);
    let loosed = quiver(&mut w, p, "quarrel", 1);
    throw(&mut w, p, loosed, spot);
    assert!(
        logged(&w, "You fire a quarrel."),
        "from a crossbow, it's fired: {:?}",
        w.resource::<GameLog>().history
    );
}

// ---------------------------------------------------------------------------
// The pluses
// ---------------------------------------------------------------------------

/// The enchantment has no melee roll to land on, so it lands on the throw.
#[test]
fn a_bows_plus_rides_along_on_what_it_looses() {
    let sharp = damage_samples(6, "arrow", 300, |w, p| {
        let bow = stash(w, p, |w| spawn_launcher(w, "short bow", NOWHERE));
        w.entity_mut(bow).insert(ThrowBonus(3));
        toggle_equipped(w, p, bow);
    });
    assert!(
        sharp.iter().all(|&d| (4..=9).contains(&d)),
        "1d6+3: {sharp:?}"
    );
}

#[test]
fn only_armour_plus_blunts_a_throw_and_the_armour_die_never_does() {
    fn worst(weapon: &'static str, plus: i32) -> i32 {
        let mut w = test_world(8);
        let p = player(&mut w);
        let spot = open_run(&mut w, 1)[0];
        let bag = tough(&mut w, "bat", spot);
        w.get_mut::<Fighter>(bag).unwrap().armor = 20;
        w.get_mut::<Fighter>(bag).unwrap().armor_bonus = plus;
        w.get_mut::<Fighter>(bag).unwrap().hp = 100_000;
        (0..300)
            .map(|_| {
                let item = stash(&mut w, p, |w| spawn_weapon(w, weapon, NOWHERE));
                let before = hp(&w, bag);
                throw(&mut w, p, item, spot);
                before - hp(&w, bag)
            })
            .max()
            .unwrap()
    }

    fn thrown_damage(weapon: &'static str) -> i32 {
        let mut w = test_world(8);
        let item = spawn_weapon(&mut w, weapon, NOWHERE);
        w.get::<ThrownDamage>(item).expect("a weapon throws").0
    }

    for weapon in ["dagger", "mace"] {
        assert_eq!(
            worst(weapon, 0),
            thrown_damage(weapon),
            "{weapon}: armour die 20 blunted the throw"
        );
        assert!(
            worst(weapon, 3) < worst(weapon, 0),
            "{weapon}: armour plus did not blunt the throw"
        );
    }
}

#[test]
fn a_blowdart_that_draws_blood_saps_the_victims_power() {
    let mut w = test_world(13);
    let p = player(&mut w);
    let gun = stash(&mut w, p, |w| spawn_launcher(w, "blowgun", NOWHERE));
    toggle_equipped(&mut w, p, gun);
    let spot = open_run(&mut w, 1)[0];
    let bag = tough(&mut w, "bat", spot);
    let power = w.get::<Fighter>(bag).unwrap().power;
    let dart = quiver(&mut w, p, "blowdart", 1);

    throw(&mut w, p, dart, spot);

    assert_eq!(w.get::<Fighter>(bag).unwrap().power, power - 1);
    assert!(logged(&w, "Venom courses through"));
}

// ---------------------------------------------------------------------------
// Piercing, and stopping
// ---------------------------------------------------------------------------

#[test]
fn an_improvised_missile_also_resolves_on_the_first_target() {
    let mut w = test_world(10);
    let p = player(&mut w);
    let run = open_run(&mut w, 2);
    let near = tough(&mut w, "bat", run[0]);
    let far = tough(&mut w, "bat", run[1]);
    let mace = stash(&mut w, p, |w| spawn_weapon(w, "mace", NOWHERE));

    throw(&mut w, p, mace, run[1]);

    assert!(hp(&w, near) < 500);
    assert_eq!(hp(&w, far), 500);
    let at = w.get::<Position>(mace).unwrap();
    assert_eq!((at.x, at.y), (run[0].x, run[0].y));
}

// ---------------------------------------------------------------------------
// Coming home
// ---------------------------------------------------------------------------

fn pack_count(w: &World, p: Entity, item: Entity) -> usize {
    w.get::<Backpack>(p)
        .unwrap()
        .items
        .iter()
        .filter(|&&e| e == item)
        .count()
}

#[test]
fn a_boomerang_hits_the_first_creature_only_and_comes_back_to_the_pack() {
    let mut w = test_world(40);
    let p = player(&mut w);
    let run = open_run(&mut w, 3);
    let row: Vec<Entity> = run.iter().map(|&at| tough(&mut w, "bat", at)).collect();
    let boomerang = stash(&mut w, p, |w| spawn_weapon(w, "boomerang", NOWHERE));

    throw(&mut w, p, boomerang, run[2]);

    assert!(hp(&w, row[0]) < 500, "the near bat took it");
    assert_eq!(hp(&w, row[1]), 500);
    assert_eq!(hp(&w, row[2]), 500);
    assert_eq!(pack_count(&w, p, boomerang), 1, "back in the pack, once");
    assert!(
        w.get::<Position>(boomerang).is_none(),
        "and not on the floor"
    );
    assert_ne!(
        equipped_in(&w, p, Slot::Hand),
        Some(boomerang),
        "but not in hand"
    );
    assert!(logged(&w, "boomerang returns"));
}

#[test]
fn a_wielded_boomerang_is_wielded_again_when_it_comes_back() {
    let mut w = test_world(42);
    let p = player(&mut w);
    let spot = open_run(&mut w, 1)[0];
    let bat = tough(&mut w, "bat", spot);
    let boomerang = stash(&mut w, p, |w| spawn_weapon(w, "boomerang", NOWHERE));
    toggle_equipped(&mut w, p, boomerang);
    assert_eq!(equipped_in(&w, p, Slot::Hand), Some(boomerang));

    throw(&mut w, p, boomerang, spot);

    assert!(hp(&w, bat) < 500);
    assert_eq!(equipped_in(&w, p, Slot::Hand), Some(boomerang));
    assert_eq!(pack_count(&w, p, boomerang), 1);
}

// ---------------------------------------------------------------------------
// Spent, or not
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// Stacking
// ---------------------------------------------------------------------------

#[test]
fn quarrels_never_go_in_with_arrows() {
    let mut w = test_world(16);
    let p = player(&mut w);
    empty_pack(&mut w, p);
    let arrows = quiver(&mut w, p, "arrow", 4);

    let bolts = spawn_ammo(&mut w, "quarrel", NOWHERE);
    w.get_mut::<Stack>(bolts).unwrap().count = 3;
    stow(&mut w, p, bolts).unwrap();

    assert_eq!(w.get::<Stack>(arrows).unwrap().count, 4);
    assert_eq!(w.get::<Backpack>(p).unwrap().items, vec![arrows, bolts]);
}

#[test]
fn a_quiver_tops_out_at_stack_limit_and_the_rest_takes_a_slot_of_its_own() {
    let mut w = test_world(17);
    let p = player(&mut w);
    empty_pack(&mut w, p);
    let carried = quiver(&mut w, p, "arrow", STACK_LIMIT - 2);

    let pile = spawn_ammo(&mut w, "arrow", NOWHERE);
    w.get_mut::<Stack>(pile).unwrap().count = 9;
    let taken = stow(&mut w, p, pile).expect("all nine came with you");

    assert_eq!(taken, "9 arrows");
    assert_eq!(
        w.get::<Stack>(carried).unwrap().count,
        STACK_LIMIT,
        "filled to the brim"
    );
    assert_eq!(
        w.get::<Stack>(pile).unwrap().count,
        7,
        "and the overflow rides along"
    );
    assert_eq!(w.get::<Backpack>(p).unwrap().items, vec![carried, pile]);
    assert!(w.get::<Position>(pile).is_none(), "it left the floor");

    let more = spawn_ammo(&mut w, "arrow", NOWHERE);
    w.get_mut::<Stack>(more).unwrap().count = 4;
    stow(&mut w, p, more).unwrap();
    assert_eq!(w.get::<Stack>(pile).unwrap().count, 11);
    assert!(w.get_entity(more).is_none());
    assert_eq!(w.get::<Backpack>(p).unwrap().items.len(), 2);
}

#[test]
fn a_stack_counts_itself_in_the_pack_screen() {
    let mut w = test_world(18);
    let p = player(&mut w);
    let arrows = quiver(&mut w, p, "arrow", 7);
    assert_eq!(display_name(&w, arrows), "7 arrows");
    assert_eq!(with_article(&w, arrows), "7 arrows");

    w.get_mut::<Stack>(arrows).unwrap().count = 1;
    assert_eq!(display_name(&w, arrows), "arrow");
    assert_eq!(with_article(&w, arrows), "an arrow");

    let dagger = stash(&mut w, p, |w| spawn_weapon(w, "dagger", NOWHERE));
    assert_eq!(with_article(&w, dagger), "a dagger");
}

// ---------------------------------------------------------------------------
// Across a save
// ---------------------------------------------------------------------------

#[test]
fn a_quiver_and_a_bow_come_back_whole_from_a_save() {
    let mut w = test_world(19);
    let p = player(&mut w);
    empty_pack(&mut w, p);
    let ammo_row = &AMMO[0];
    let arrows = stash(&mut w, p, |w| ammo_row.spawn(w, NOWHERE));
    w.get_mut::<Stack>(arrows).unwrap().count = 13;
    let bow = stash(&mut w, p, |w| LAUNCHERS[0].spawn(w, NOWHERE));
    w.entity_mut(bow).insert(ThrowBonus(2));
    let weapon_row = &WEAPONS[0];
    let dagger = stash(&mut w, p, |w| weapon_row.spawn(w, NOWHERE));
    let was_piercing = w.get::<Piercing>(dagger).is_some();
    let arrow_damage = *w.get::<ThrownDamage>(arrows).unwrap();
    let arrow_row = w.get::<Ammo>(arrows).unwrap().row;
    let dagger_damage = *w.get::<ThrownDamage>(dagger).unwrap();

    let save = common::SaveFile::new("missiles");
    save_game(&mut w, save.path()).unwrap();
    let mut loaded = World::new();
    loaded.insert_resource(GameRng(ChaCha12Rng::seed_from_u64(19)));
    loaded.insert_resource(RngSeed(19));
    loaded.init_resource::<GameLog>();
    loaded.insert_resource(PlayerName {
        what: "TESTER".into(),
    });
    load_game(&mut loaded, save.path()).unwrap();

    let hero = player(&mut loaded);
    let packed = loaded.get::<Backpack>(hero).unwrap().items.clone();
    let arrows = *packed
        .iter()
        .find(|&&e| loaded.get::<Stack>(e).is_some() && loaded.get::<Ammo>(e).is_some())
        .expect("the quiver survived the save");
    assert_eq!(loaded.get::<Stack>(arrows).unwrap().count, 13);
    assert_eq!(loaded.get::<ThrownDamage>(arrows), Some(&arrow_damage));
    assert_eq!(loaded.get::<Ammo>(arrows).unwrap().row.name, arrow_row.name);
    assert!(loaded.get::<Projectile>(arrows).is_some());

    let bow = loaded
        .query_filtered::<Entity, With<Launcher>>()
        .iter(&loaded)
        .find(|&entity| {
            loaded
                .get::<ThrowBonus>(entity)
                .is_some_and(|bonus| bonus.0 == 2)
        })
        .expect("the test launcher survived the save");
    assert!(loaded.get::<Launcher>(bow).is_some());
    assert_eq!(loaded.get::<ThrowBonus>(bow), Some(&ThrowBonus(2)));

    let dagger = *packed
        .iter()
        .find(|&&e| {
            loaded
                .get::<Name>(e)
                .is_some_and(|n| n.what == weapon_row.name)
        })
        .expect("the dagger survived the save");
    assert_eq!(loaded.get::<ThrownDamage>(dagger), Some(&dagger_damage));
    assert_eq!(loaded.get::<Piercing>(dagger).is_some(), was_piercing);
    assert!(loaded.get::<Projectile>(dagger).is_some());

    toggle_equipped(&mut loaded, hero, bow);
    assert!(ammo_row.launched_by.probe(&loaded, hero));
}

// ---------------------------------------------------------------------------
// The price of the hand
// ---------------------------------------------------------------------------

/// A target that cannot die and cannot defend, so what lands on it is exactly
/// what the attacker rolled.
fn punching_bag(w: &mut World, at: Position) -> Entity {
    w.spawn((
        Name { what: "bag".into() },
        Fighter {
            hp: 100_000,
            max_hp: 100_000,
            armor: 0,
            power: 0,
            max_power: 0,
            armor_bonus: 0,
            power_bonus: 0,
        },
        Faction::Monster,
        at,
    ))
    .id()
}

/// A launcher occupies the hand a sword would have had, and that hand is the
/// price of how good it is at range. Swung, a bow is a stick: at most one point
/// a blow, whatever the dice, the enchantment or the rings say.
#[test]
fn swinging_a_bow_is_worth_a_bruise_and_no_more() {
    for launcher in ["short bow", "crossbow"] {
        let mut w = test_world(11);
        let p = player(&mut w);
        let target = punching_bag(&mut w, Position { x: 1, y: 1 });

        let weapon = stash(&mut w, p, |w| spawn_launcher(w, launcher, NOWHERE));
        w.entity_mut(weapon).insert(ThrowBonus(5));
        w.entity_mut(weapon).insert(PowerBonus(5));
        toggle_equipped(&mut w, p, weapon);

        let before = hp(&w, target);
        for _ in 0..300 {
            resolve_attack(&mut w, p, target);
        }
        let dealt = before - hp(&w, target);

        assert!(
            dealt > 0,
            "a {launcher} should still be worth something swung"
        );
        assert!(
            dealt <= 300,
            "a {launcher} swing must never exceed 1 point: 300 swings dealt {dealt}"
        );
    }
}

/// The cap is on the swing, not on the shot. The same bow that is a stick in a
/// corridor still ups an arrow's die when it is drawn.
#[test]
fn the_melee_cap_does_not_follow_the_arrow() {
    let mut w = test_world(11);
    let p = player(&mut w);
    empty_pack(&mut w, p);

    let lane = open_run(&mut w, 4);
    let target = tough(&mut w, "troll", lane[3]);

    let bow = stash(&mut w, p, |w| spawn_launcher(w, "short bow", NOWHERE));
    toggle_equipped(&mut w, p, bow);
    let arrows = quiver(&mut w, p, "arrow", 30);

    let before = hp(&w, target);
    for _ in 0..30 {
        throw(&mut w, p, arrows, lane[3]);
    }
    let dealt = before - hp(&w, target);

    assert!(
        dealt > 30,
        "30 loosed arrows should far exceed the melee ceiling, dealt {dealt}"
    );
}

/// Weight decides the leash for anything not on a bowstring: a potion, scroll,
/// wand or ring is flicked, a dagger is heaved.
#[test]
fn the_small_stuff_carries_further_out_of_a_bare_hand() {
    let mut w = test_world(11);
    let p = player(&mut w);
    empty_pack(&mut w, p);

    let dagger = stash(&mut w, p, |w| spawn_weapon(w, "dagger", NOWHERE));
    assert_eq!(throw_reach(&w, p, dagger), THROW_RANGE);

    let light = [
        stash(&mut w, p, |w| {
            spawn_named(w, "potion of healing", NOWHERE).unwrap()
        }),
        stash(&mut w, p, |w| {
            spawn_named(w, "scroll of identify", NOWHERE).unwrap()
        }),
        stash(&mut w, p, |w| {
            spawn_named(w, "wand of lightning", NOWHERE).unwrap()
        }),
        stash(&mut w, p, |w| spawn_ring(w, "ring of protection", NOWHERE)),
    ];
    for item in light {
        assert_eq!(
            throw_reach(&w, p, item),
            LIGHT_THROW_RANGE,
            "{} is light enough to flick",
            display_name(&w, item)
        );
    }
    assert!(LIGHT_THROW_RANGE > THROW_RANGE);
}

// ---------------------------------------------------------------------------
// The moon blade
// ---------------------------------------------------------------------------

fn strikes_logged(w: &World) -> usize {
    w.resource::<GameLog>()
        .history
        .iter()
        .filter(|l| l.starts_with("The moon blade") && l.contains("the bat"))
        .count()
}

fn can_see(w: &mut World, p: Entity, tiles: &[Position]) {
    w.get_mut::<Viewshed>(p).unwrap().visible_tiles = tiles.iter().map(|t| (t.x, t.y)).collect();
}

#[test]
fn a_moon_blade_ignores_what_the_thrower_cannot_see() {
    let mut w = test_world(61);
    let p = player(&mut w);
    let run = open_run(&mut w, 2);
    tough(&mut w, "bat", run[0]);
    let unseen = tough(&mut w, "bat", run[1]);
    can_see(&mut w, p, &run[..1]);
    let blade = stash(&mut w, p, |w| spawn_weapon(w, "moon blade", NOWHERE));

    throw(&mut w, p, blade, run[1]);

    assert_eq!(strikes_logged(&w), 1);
    assert_eq!(hp(&w, unseen), 500);
    assert_eq!(pack_count(&w, p, blade), 1);
}

#[test]
fn a_monsters_moon_blade_finds_its_own_enemies() {
    let mut w = test_world(62);
    let run = open_run(&mut w, 3);
    let thrower = tough(&mut w, "bat", run[2]);
    w.entity_mut(thrower).insert(Backpack { items: vec![] });
    for at in [run[0], run[1]] {
        let ally = tough(&mut w, "bat", at);
        w.entity_mut(ally).insert(Faction::Ally);
    }
    let blade = stash(&mut w, thrower, |w| spawn_weapon(w, "moon blade", NOWHERE));

    throw(&mut w, thrower, blade, run[1]);

    let strikes = w
        .resource::<GameLog>()
        .history
        .iter()
        .filter(|l| l.starts_with("The moon blade") && !l.contains("flies"))
        .count();
    assert_eq!(strikes, 3, "an ally, an ally, then the player");
    assert_eq!(pack_count(&w, thrower, blade), 1);
}

/// A stack gives up one unit of its own kind, whatever the kind is called: the
/// row rides on the stack, so no table has to know the name.
#[test]
fn a_quiver_gives_up_one_of_its_own_kind_whatever_it_is_called() {
    static BOLT: AmmoDef = AmmoDef {
        name: "a bolt no table lists",
        color: Color::Grey,
        die: 2,
        launched_die: 4,
        launched_by: Grant::of::<FireArrow>(),
        on_blood: |_, _| {},
    };
    let mut w = test_world(3);
    let p = player(&mut w);
    empty_pack(&mut w, p);
    let quiver = stash(&mut w, p, |w| (&BOLT).spawn(w, NOWHERE));
    w.get_mut::<Stack>(quiver).unwrap().count = 5;

    let slot = w
        .get::<Backpack>(p)
        .unwrap()
        .items
        .iter()
        .position(|&e| e == quiver);
    w.get_mut::<Backpack>(p)
        .unwrap()
        .items
        .retain(|&e| e != quiver);
    let one = draw_one(&mut w, p, quiver, slot);

    assert_ne!(one, quiver, "one unit left the stack, not the whole stack");
    assert_eq!(w.get::<Ammo>(one).unwrap().row.name, BOLT.name);
    assert_eq!(w.get::<Stack>(one).unwrap().count, 1);
    assert_eq!(w.get::<Stack>(quiver).unwrap().count, 4);
    assert!(w.get::<Backpack>(p).unwrap().items.contains(&quiver));
}
