//! Missiles: the things in the pack that are *for* throwing.
//!
//! Four of them, and one idea. An arrow, a quarrel, a dagger and a spear all
//! carry `Projectile` — they go around armour, they are spent on what they hit,
//! and nothing ever catches one out of the air. What separates them is two more
//! components: a dagger and a spear are `Piercing` and run the whole line, while
//! an arrow and a quarrel carry `LaunchedBy` and roll twice the die for anyone
//! holding the bow or crossbow that answers to it.
//!
//! Ammunition is also the only thing in the game that stacks, so half of this
//! file is about one pack slot holding thirteen arrows and giving them up one at a
//! time.

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

/// The engine's Throw action, in full: the item leaves the pack, a stack gives
/// up one of itself, and `throw_system` finds out where the missile ends up.
/// Returns whatever actually took flight.
fn throw(w: &mut World, thrower: Entity, item: Entity, target: Position) -> Entity {
    let mut slot = None;
    if let Some(mut bp) = w.get_mut::<Backpack>(thrower) {
        slot = bp.items.iter().position(|&e| e == item);
        bp.items.retain(|&e| e != item);
    }
    let missile = draw_one(w, thrower, item, slot);
    w.resource_mut::<ThrowQueue>().throws.push(WantsToThrow {
        thrower,
        item: missile,
        target,
    });
    throw_system(w);
    missile
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
fn tough(w: &mut World, species: &str, at: Position) -> Entity {
    let m = spawn_monster(w, MonsterDef::named(species), at);
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
    // The bat is a wall of meat, not a fighter: nothing it does matters here.
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

fn mean(v: &[i32]) -> f64 {
    v.iter().sum::<i32>() as f64 / v.len() as f64
}

// ---------------------------------------------------------------------------
// Launchers
// ---------------------------------------------------------------------------

#[test]
fn a_bow_doubles_the_die_of_an_arrow_and_nothing_else() {
    let by_hand = damage_samples(1, "arrow", 400, |_, _| {});
    let from_a_bow = damage_samples(1, "arrow", 400, |w, p| {
        let bow = stash(w, p, |w| spawn_launcher(w, "short bow", NOWHERE));
        toggle_equipped(w, p, bow);
        assert!(
            w.get::<FireArrow>(p).is_some(),
            "drawing a bow arms the arrow"
        );
    });

    // 1d4 lobbed, 1d8 loosed: the honest range of each, and twice the average.
    assert!(by_hand.iter().all(|&d| (1..=4).contains(&d)), "{by_hand:?}");
    assert!(
        from_a_bow.iter().all(|&d| (1..=8).contains(&d)),
        "{from_a_bow:?}"
    );
    assert_eq!(*by_hand.iter().max().unwrap(), 4);
    assert_eq!(*from_a_bow.iter().max().unwrap(), 8);
    let (lobbed, loosed) = (mean(&by_hand), mean(&from_a_bow));
    assert!(
        loosed > lobbed * 1.6 && loosed < lobbed * 2.4,
        "a bow should about double an arrow (by hand {lobbed:.2}, from a bow {loosed:.2})"
    );
}

#[test]
fn a_crossbow_doubles_a_quarrel_and_a_bow_does_not() {
    let by_hand = damage_samples(2, "quarrel", 400, |_, _| {});
    let from_a_crossbow = damage_samples(2, "quarrel", 400, |w, p| {
        let cb = stash(w, p, |w| spawn_launcher(w, "crossbow", NOWHERE));
        toggle_equipped(w, p, cb);
    });
    // The wrong launcher is no launcher: a bow does nothing for a quarrel.
    let wrong_launcher = damage_samples(2, "quarrel", 400, |w, p| {
        let bow = stash(w, p, |w| spawn_launcher(w, "short bow", NOWHERE));
        toggle_equipped(w, p, bow);
    });

    assert_eq!(*by_hand.iter().max().unwrap(), 6, "1d6 lobbed");
    assert_eq!(*from_a_crossbow.iter().max().unwrap(), 12, "1d12 loosed");
    assert!(
        wrong_launcher.iter().all(|&d| (1..=6).contains(&d)),
        "{wrong_launcher:?}"
    );
}

#[test]
fn a_launcher_is_a_grant_and_nothing_more() {
    // No attack die, no armour die: a bow is worth exactly nothing swung, which
    // is the price of what it does to an arrow.
    let mut w = test_world(3);
    let p = player(&mut w);
    let bow = stash(&mut w, p, |w| spawn_launcher(w, "short bow", NOWHERE));

    assert!(w.get::<PowerDie>(bow).is_none());
    assert!(w.get::<ArmorDie>(bow).is_none());
    assert!(w.get::<FireArrow>(p).is_none());

    toggle_equipped(&mut w, p, bow);
    assert!(w.get::<FireArrow>(p).is_some());
    assert_eq!(equipped_total::<PowerDie>(&w, p), 0);

    // Taking it off takes the effect back with it, exactly like a ring.
    toggle_equipped(&mut w, p, bow);
    assert!(w.get::<FireArrow>(p).is_none());
}

#[test]
fn a_bow_and_a_sword_want_the_same_hand() {
    let mut w = test_world(3);
    let p = player(&mut w);
    let sword = stash(&mut w, p, |w| spawn_weapon(w, "long sword", NOWHERE));
    let bow = stash(&mut w, p, |w| spawn_launcher(w, "short bow", NOWHERE));

    toggle_equipped(&mut w, p, sword);
    toggle_equipped(&mut w, p, bow);

    assert_eq!(equipped_in(&w, p, Slot::Hand), Some(bow));
    assert_eq!(
        equipped_total::<PowerDie>(&w, p),
        0,
        "the sword is back in the pack"
    );
}

#[test]
fn firing_an_arrow_from_a_bow_reads_differently_than_throwing_one_by_hand() {
    let mut w = test_world(1);
    let p = player(&mut w);
    let spot = open_run(&mut w, 1)[0];

    let lobbed = quiver(&mut w, p, "arrow", 1);
    throw(&mut w, p, lobbed, spot);
    assert!(
        logged(&w, "You throw the arrow."),
        "by hand, it's a throw: {:?}",
        w.resource::<GameLog>().history
    );

    let bow = stash(&mut w, p, |w| spawn_launcher(w, "short bow", NOWHERE));
    toggle_equipped(&mut w, p, bow);
    let loosed = quiver(&mut w, p, "arrow", 1);
    throw(&mut w, p, loosed, spot);
    assert!(
        logged(&w, "You fire an arrow."),
        "from a bow, it's fired: {:?}",
        w.resource::<GameLog>().history
    );
}

#[test]
fn firing_a_quarrel_from_a_crossbow_says_fire_not_throw() {
    let mut w = test_world(2);
    let p = player(&mut w);
    let spot = open_run(&mut w, 1)[0];

    // The wrong launcher (a bow) is no launcher: still a throw.
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

#[test]
fn a_ring_of_dexterity_adds_two_to_everything_you_throw() {
    let plain = damage_samples(5, "arrow", 300, |_, _| {});
    let deft = damage_samples(5, "arrow", 300, |w, p| {
        let ring = stash(w, p, |w| spawn_ring(w, RingEffect::Dexterity, NOWHERE));
        toggle_equipped(w, p, ring);
        assert_eq!(equipped_total::<ThrowBonus>(w, p), 2);
    });

    assert!(
        deft.iter().all(|&d| (3..=6).contains(&d)),
        "1d4+2: {deft:?}"
    );
    assert!((mean(&deft) - mean(&plain) - 2.0).abs() < 0.4);
}

#[test]
fn a_bows_plus_rides_along_on_what_it_looses() {
    // The enchantment has no melee roll to land on, so it lands on the throw.
    let sharp = damage_samples(6, "arrow", 300, |w, p| {
        let bow = stash(w, p, |w| spawn_launcher(w, "short bow", NOWHERE));
        w.entity_mut(bow).insert(ThrowBonus(3));
        toggle_equipped(w, p, bow);
    });
    assert!(
        sharp.iter().all(|&d| (4..=11).contains(&d)),
        "1d8+3: {sharp:?}"
    );
}

#[test]
fn a_daggers_plus_rides_along_on_the_dagger() {
    let mut w = test_world(7);
    let p = player(&mut w);
    let spot = open_run(&mut w, 1)[0];
    let bag = tough(&mut w, "bat", spot);

    let mut seen = Vec::new();
    for _ in 0..200 {
        let dagger = stash(&mut w, p, |w| spawn_weapon(w, "dagger", NOWHERE));
        w.entity_mut(dagger).insert(PowerBonus(2));
        let before = hp(&w, bag);
        throw(&mut w, p, dagger, spot);
        seen.push(before - hp(&w, bag));
    }
    assert!(
        seen.iter().all(|&d| (3..=6).contains(&d)),
        "1d4+2: {seen:?}"
    );
}

#[test]
fn a_projectile_goes_around_armour_and_an_improvised_one_does_not() {
    /// Throw `weapon` at a bat wearing `plus` points of armour bonus, and report
    /// the worst it ever managed.
    fn worst(weapon: &'static str, plus: i32) -> i32 {
        let mut w = test_world(8);
        let p = player(&mut w);
        let spot = open_run(&mut w, 1)[0];
        let bag = tough(&mut w, "bat", spot);
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

    // A dagger is a point already in the air: plate mail is not in the argument.
    assert_eq!(worst("dagger", 0), 4);
    assert_eq!(worst("dagger", 3), 4);
    // A mace is a lump you happened to let go of, and armour still counts.
    assert_eq!(worst("mace", 0), 6);
    assert_eq!(worst("mace", 3), 3);
}

// ---------------------------------------------------------------------------
// Piercing, and stopping
// ---------------------------------------------------------------------------

#[test]
fn a_spear_runs_the_whole_line_and_an_arrow_stops_at_the_first_thing_it_hits() {
    fn line_of_three(seed: u64) -> (World, Entity, Vec<Entity>, Position) {
        let mut w = test_world(seed);
        let p = player(&mut w);
        let run = open_run(&mut w, 3);
        let row: Vec<Entity> = run.iter().map(|&at| tough(&mut w, "bat", at)).collect();
        let far = run[2];
        (w, p, row, far)
    }

    // The spear spends itself on all three, in the order it reached them.
    let (mut w, p, row, far) = line_of_three(9);
    let before: Vec<i32> = row.iter().map(|&e| hp(&w, e)).collect();
    let spear = stash(&mut w, p, |w| spawn_weapon(w, "spear", NOWHERE));
    throw(&mut w, p, spear, far);
    for (i, &bat) in row.iter().enumerate() {
        assert!(hp(&w, bat) < before[i], "the spear missed bat {i}");
    }

    // A dagger does the same — NecroDancer, not Rogue.
    let (mut w, p, row, far) = line_of_three(9);
    let dagger = stash(&mut w, p, |w| spawn_weapon(w, "dagger", NOWHERE));
    throw(&mut w, p, dagger, far);
    assert!(
        row.iter().all(|&bat| hp(&w, bat) < 500),
        "the dagger stopped short"
    );

    // An arrow aimed at the far bat never gets past the near one. This is the
    // rule for everything that is not a dagger or a spear.
    let (mut w, p, row, far) = line_of_three(9);
    let arrow = quiver(&mut w, p, "arrow", 1);
    throw(&mut w, p, arrow, far);
    assert!(hp(&w, row[0]) < 500, "the near bat took it");
    assert_eq!(hp(&w, row[1]), 500, "and nothing got past it");
    assert_eq!(hp(&w, row[2]), 500);
}

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
    // And it stopped there, rather than carrying on to the aimed tile.
    let at = w.get::<Position>(mace).unwrap();
    assert_eq!((at.x, at.y), (run[0].x, run[0].y));
}

// ---------------------------------------------------------------------------
// Spent, or not
// ---------------------------------------------------------------------------

#[test]
fn a_projectile_that_hits_something_is_spent() {
    let mut w = test_world(11);
    let p = player(&mut w);
    let spot = open_run(&mut w, 1)[0];
    tough(&mut w, "bat", spot);

    for weapon in ["dagger", "spear"] {
        let item = stash(&mut w, p, |w| spawn_weapon(w, weapon, NOWHERE));
        throw(&mut w, p, item, spot);
        assert!(w.get_entity(item).is_none(), "the {weapon} should be gone");
    }

    let arrow = quiver(&mut w, p, "arrow", 1);
    let loosed = throw(&mut w, p, arrow, spot);
    assert!(w.get_entity(loosed).is_none(), "the arrow snapped");
}

#[test]
fn a_projectile_that_hits_nothing_falls_where_it_landed() {
    let mut w = test_world(12);
    let p = player(&mut w);
    let spot = open_run(&mut w, 1)[0];
    let dagger = stash(&mut w, p, |w| spawn_weapon(w, "dagger", NOWHERE));

    throw(&mut w, p, dagger, spot);

    let at = w
        .get::<Position>(dagger)
        .expect("it should be on the floor");
    assert_eq!((at.x, at.y), (spot.x, spot.y));
}

#[test]
fn nothing_catches_a_projectile_out_of_the_air() {
    // An orc will happily field a thrown mace and start using it. A spear
    // arrives point first, and there is nothing left to pick up either way.
    let mut w = test_world(13);
    let p = player(&mut w);
    let spot = open_run(&mut w, 1)[0];
    let orc = tough(&mut w, "orc", spot);
    assert!(w.get::<ItemUser>(orc).is_some());

    let spear = stash(&mut w, p, |w| spawn_weapon(w, "spear", NOWHERE));
    throw(&mut w, p, spear, spot);

    assert!(w.get_entity(spear).is_none());
    assert!(
        equipped_in(&w, orc, Slot::Hand).is_none(),
        "the orc caught nothing"
    );
    assert!(!logged(&w, "wields it"));
}

#[test]
fn the_dagger_in_your_hand_can_be_thrown_unless_it_is_cursed() {
    let mut w = test_world(21);
    let p = player(&mut w);
    let spot = open_run(&mut w, 1)[0];
    let dagger = stash(&mut w, p, |w| spawn_weapon(w, "dagger", NOWHERE));
    toggle_equipped(&mut w, p, dagger);

    // Wielding it is no obstacle: it comes off on the way out of your hand.
    assert!(throw_refusal(&w, p, dagger).is_none());
    throw(&mut w, p, dagger, spot);
    assert_eq!(equipped_total::<PowerDie>(&w, p), 0);

    // A cursed one is welded on, and a welded weapon cannot be hurled.
    let cursed = stash(&mut w, p, |w| spawn_weapon(w, "spear", NOWHERE));
    w.entity_mut(cursed).insert(Curse);
    toggle_equipped(&mut w, p, cursed);
    assert!(throw_refusal(&w, p, cursed).is_some());
}

// ---------------------------------------------------------------------------
// Stacking
// ---------------------------------------------------------------------------

#[test]
fn throwing_from_a_quiver_spends_one_and_keeps_the_slot() {
    let mut w = test_world(14);
    let p = player(&mut w);
    let spot = open_run(&mut w, 1)[0];
    empty_pack(&mut w, p);
    let arrows = quiver(&mut w, p, "arrow", 10);
    // Something else above it, so "kept its slot" means something.
    let ring = stash(&mut w, p, |w| spawn_ring(w, RingEffect::Adornment, NOWHERE));
    w.get_mut::<Backpack>(p).unwrap().items = vec![ring, arrows];

    let loosed = throw(&mut w, p, arrows, spot);

    assert_ne!(loosed, arrows, "the quiver itself never leaves your hand");
    assert_eq!(w.get::<Stack>(arrows).unwrap().count, 9);
    assert_eq!(w.get::<Backpack>(p).unwrap().items, vec![ring, arrows]);
}

#[test]
fn the_last_arrow_takes_the_slot_with_it() {
    let mut w = test_world(15);
    let p = player(&mut w);
    let spot = open_run(&mut w, 1)[0];
    empty_pack(&mut w, p);
    let arrows = quiver(&mut w, p, "arrow", 1);

    let loosed = throw(&mut w, p, arrows, spot);

    assert_eq!(loosed, arrows, "a stack of one is thrown whole");
    assert!(w.get::<Backpack>(p).unwrap().items.is_empty());
}

#[test]
fn arrows_off_the_floor_top_up_the_quiver_you_are_carrying() {
    let mut w = test_world(16);
    let p = player(&mut w);
    empty_pack(&mut w, p);
    let carried = quiver(&mut w, p, "arrow", 4);

    let pile = spawn_ammo(&mut w, "arrow", NOWHERE);
    w.get_mut::<Stack>(pile).unwrap().count = 6;
    let taken = stow(&mut w, p, pile).expect("they went in");

    assert_eq!(taken, "6 arrows");
    assert_eq!(w.get::<Stack>(carried).unwrap().count, 10);
    assert!(
        w.get_entity(pile).is_none(),
        "the pile is gone into the quiver"
    );
    assert_eq!(
        w.get::<Backpack>(p).unwrap().items,
        vec![carried],
        "still one slot"
    );
}

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
fn a_quiver_tops_out_at_twenty_six_and_the_rest_takes_a_slot_of_its_own() {
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

    // A second pile fills the partial slot before opening a third.
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
    quiver(&mut w, p, "arrow", 13);
    let bow = stash(&mut w, p, |w| spawn_launcher(w, "short bow", NOWHERE));
    w.entity_mut(bow).insert(ThrowBonus(2));
    stash(&mut w, p, |w| spawn_weapon(w, "spear", NOWHERE));

    let path = std::env::temp_dir().join("roog-missiles.sav");
    save_game(&mut w, path.to_str().unwrap()).unwrap();
    // A bare world: `load_game` builds the whole thing, player included.
    let mut loaded = World::new();
    loaded.insert_resource(GameRng(ChaCha12Rng::seed_from_u64(19)));
    loaded.insert_resource(RngSeed(19));
    loaded.init_resource::<GameLog>();
    loaded.insert_resource(PlayerName {
        what: "TESTER".into(),
    });
    load_game(&mut loaded, path.to_str().unwrap()).unwrap();
    let _ = std::fs::remove_file(&path);

    let find = |w: &mut World, what: &str| -> Entity {
        let what = what.to_string();
        w.iter_entities()
            .find(|e| e.get::<Name>().is_some_and(|n| n.what == what))
            .map(|e| e.id())
            .unwrap_or_else(|| panic!("no {what} survived the save"))
    };

    // The stack's count is saved; everything the catalog row already says is
    // read back from the row.
    let arrows = find(&mut loaded, "arrow");
    assert_eq!(loaded.get::<Stack>(arrows).unwrap().count, 13);
    assert_eq!(loaded.get::<ThrownDamage>(arrows), Some(&ThrownDamage(4)));
    assert!(loaded.get::<Projectile>(arrows).is_some());
    assert!(loaded.get::<LaunchedBy>(arrows).is_some());

    let bow = find(&mut loaded, "short bow");
    assert!(loaded.get::<Launcher>(bow).is_some());
    assert_eq!(loaded.get::<ThrowBonus>(bow), Some(&ThrowBonus(2)));

    let spear = find(&mut loaded, "spear");
    assert_eq!(loaded.get::<ThrownDamage>(spear), Some(&ThrownDamage(8)));
    assert!(loaded.get::<Piercing>(spear).is_some());
    assert!(loaded.get::<Projectile>(spear).is_some());

    // And a loaded bow still arms a loaded arrow.
    let hero = player(&mut loaded);
    toggle_equipped(&mut loaded, hero, bow);
    assert!(loaded.get::<FireArrow>(hero).is_some());
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
        // A ruinously good one, to prove the cap is a ceiling and not a die.
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
/// corridor still doubles an arrow's die when it is drawn.
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

/// A bow that is not in your hand caps nothing — the ceiling comes off with the
/// bow, and an unarmed punch is worth more than a swung one.
#[test]
fn the_cap_comes_off_with_the_bow() {
    let mut w = test_world(3);
    let p = player(&mut w);
    let target = punching_bag(&mut w, Position { x: 1, y: 1 });

    let bow = stash(&mut w, p, |w| spawn_launcher(w, "short bow", NOWHERE));
    toggle_equipped(&mut w, p, bow);
    let before = hp(&w, target);
    for _ in 0..300 {
        resolve_attack(&mut w, p, target);
    }
    let with_bow = before - hp(&w, target);

    toggle_equipped(&mut w, p, bow);
    let before = hp(&w, target);
    for _ in 0..300 {
        resolve_attack(&mut w, p, target);
    }
    let bare_handed = before - hp(&w, target);

    assert!(
        bare_handed > with_bow,
        "bare fists ({bare_handed}) should beat a swung bow ({with_bow})"
    );
}

/// The ceiling is a catalog row, so it comes back the way a bow's grant does.
#[test]
fn the_melee_cap_survives_a_save() {
    let path = std::env::temp_dir().join("roog_melee_cap.sav");
    let path = path.to_str().unwrap();

    let mut w = test_world(5);
    let p = player(&mut w);
    empty_pack(&mut w, p);
    let bow = stash(&mut w, p, |w| spawn_launcher(w, "short bow", NOWHERE));
    assert_eq!(w.get::<MeleeCap>(bow).copied(), Some(MeleeCap(1)));
    save_game(&mut w, path).unwrap();

    let mut w2 = World::new();
    w2.init_resource::<GameLog>();
    load_game(&mut w2, path).unwrap();

    let caps: Vec<MeleeCap> = w2.query::<&MeleeCap>().iter(&w2).copied().collect();
    assert_eq!(
        caps,
        vec![MeleeCap(1)],
        "the bow came back without its ceiling"
    );
    let _ = std::fs::remove_file(path);
}
