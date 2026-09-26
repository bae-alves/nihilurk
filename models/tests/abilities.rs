//! The safety net under monster and gear abilities.
//!
//! Every ability in the game is a marker component on a creature plus a
//! mechanic that fires at some *moment* — a blow that landed, a turn that
//! passed, a step onto a tile. Two of those moments are tables
//! (rows in [`ABILITIES`]); the rest are hand-written
//! into `ai`, `combat`, `traps` and `helpers::took_damage`. Until this file
//! existed, none of them were covered: 315 tests and not one named `Gorgon`,
//! `FireBreath` or `Splits`.
//!
//! This is a *characterisation* net, written to hold the behaviour still
//! while the moments are generalised. Two rules shape it.
//!
//! **A test never asserts a constant**
//! (`docs/explanation/code-calisthenics.md`). Not "venom drains 1 power" —
//! that is `constants.rs` testing itself, and it goes red the morning
//! somebody rebalances. What each test claims is the *relation* that makes
//! the ability what it is: the victim's power is lower than it was, the slime
//! that lived is now two slimes, the player who looked at a medusa cannot
//! move. Chance-gated abilities are swept over seeds and assert "this happens
//! at least once, and never when the marker is absent".
//!
//! **A test never reaches for another file's names.** Nothing here spawns
//! `"aquator"` or `"ring mail"`. What these tests are about is *a creature
//! carrying `RustsArmor`* and *something worn in `Slot::Body`*, so they build
//! those out of components and owe the content tables nothing. Whether the
//! tables can build their own contents is a different question, already
//! answered by `tests/content.rs` walking `content_names()`.
//!
//! One exception, and it is a finding rather than a shortcut: `maybe_split`
//! (`monsters.rs`) looks its victim up in the bestiary by `Name` and panics
//! on a miss, so a fixture with a name this file invented cannot split. That
//! one test takes a name off the table at runtime instead of inventing one.

use bevy_ecs::prelude::*;
use bevy_ecs::schedule::Schedule;
use fixedbitset::FixedBitSet;
use models::*;

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

/// How far a fixture player can see. The test's own number, not the game's —
/// borrowing `SIGHT_RANGE` would tie these tests to a tuning knob.
const FIXTURE_SIGHT: u16 = 12;

/// An empty floor: every tile open, nothing standing on it, no content drawn
/// from any table.
///
/// `initialize_world` would bring a generated map, scattered monsters, coins
/// and traps — every one of them a variable these tests would then have to
/// control for. `Map` is a plain public struct, so an arena is cheaper and
/// says exactly what it is.
fn arena(seed: u64) -> World {
    let mut w = World::new();
    w.insert_resource(GameRng(ChaCha12Rng::seed_from_u64(seed)));
    w.insert_resource(RngSeed(seed));
    w.init_resource::<GameLog>();
    w.init_resource::<ExtraMonsterRound>();
    w.init_resource::<AttackQueue>();
    w.init_resource::<UseQueue>();
    w.init_resource::<ThrowQueue>();
    w.init_resource::<SpellQueue>();
    w.insert_resource(Map {
        tiles: vec![TileType::Room; MAP_TILE_COUNT],
        dark: FixedBitSet::with_capacity(MAP_TILE_COUNT),
        special: vec![None; MAP_TILE_COUNT],
        level: None,
    });
    // The overlays a mechanic writes to as it resolves — smoke where something
    // vanished, blood where something was hurt — plus the floor number the
    // trap formulas scale against. `initialize_world` would supply these;
    // reached through `resource_mut`, so they have to be here or a flourish
    // panics mid-test.
    w.init_resource::<Smoke>();
    w.init_resource::<BloodStains>();
    w.init_resource::<Corpses>();
    w.insert_resource(Depth { what: 1 });
    w
}

fn at(x: u16, y: u16) -> Position {
    Position { x, y }
}

fn body(hp: i32, power: i32) -> Fighter {
    Fighter {
        hp,
        max_hp: hp,
        armor: 0,
        power,
        max_power: power,
        armor_bonus: 0,
        power_bonus: 0,
    }
}

/// The player, as the tests need them: something that fights, stands
/// somewhere, sees, and has a pack to be robbed of.
fn hero(w: &mut World, pos: Position, hp: i32, power: i32) -> Entity {
    w.spawn((
        Player,
        Name { what: "you".into() },
        body(hp, power),
        pos,
        Faction::Player,
        Backpack { items: Vec::new() },
        Speed::new(SpeedKind::Normal),
        Viewshed {
            visible_tiles: Vec::new(),
            revealed_tiles: FixedBitSet::with_capacity(MAP_TILE_COUNT),
            range: FIXTURE_SIGHT,
            dirty: true,
        },
    ))
    .id()
}

/// A hostile creature with no magic of its own. Every ability test starts
/// here and inserts the one marker it is about.
fn creature(w: &mut World, pos: Position, hp: i32, power: i32) -> Entity {
    w.spawn((
        Name {
            what: "creature".into(),
        },
        Mob {
            movement_type: MovementType::Chase,
        },
        body(hp, power),
        pos,
        Faction::Monster,
        Speed::new(SpeedKind::Normal),
    ))
    .id()
}

/// A plain combatant with no faction, position politics or AI — the target
/// dummy the on-hit rows swing at.
fn dummy(w: &mut World, hp: i32, power: i32) -> Entity {
    w.spawn((
        Name {
            what: "dummy".into(),
        },
        body(hp, power),
        at(10, 10),
    ))
    .id()
}

/// What a fixture ring lends. `Grants` holds a `&'static [Grant]`, so the
/// slice has to outlive the test rather than be built inline.
const LENDS_SIGHT: &[Grant] = &[Grant::of::<SeesInvisible>()];

/// A wearable, worn. `Equipped.by` points at the wearer, which is how gear
/// records who has it (`equipment::equipped`).
fn worn(w: &mut World, wearer: Entity, slot: Slot) -> Entity {
    w.spawn((
        Name {
            what: "gear".into(),
        },
        Item,
        Equipped {
            by: Some(wearer),
            slot,
        },
    ))
    .id()
}

/// Something loose in a pack, for a thief to lift.
fn pack_item(w: &mut World, owner: Entity, name: &str) -> Entity {
    let item = w.spawn((Name { what: name.into() }, Item, at(1, 1))).id();
    if let Some(mut pack) = w.get_mut::<Backpack>(owner) {
        pack.items.push(item);
    }
    item
}

/// Recomputes what the player can see. `ai` only lets a mob act on what is in
/// the player's viewshed (`ai::notices`), so anything driving `ai` runs this
/// after placing its creatures. Same one-system schedule `tests/autofight.rs`
/// uses.
fn see(w: &mut World) {
    let mut s = Schedule::default();
    s.add_systems(visibility_system);
    s.run(w);
}

/// A blow that connected cleanly: neither turned by armour nor the last one.
/// The default case every on-hit row accepts.
const CLEAN: Blow = Blow {
    glancing: false,
    lethal: false,
};

const GLANCING: Blow = Blow {
    glancing: true,
    lethal: false,
};

const LETHAL: Blow = Blow {
    glancing: false,
    lethal: true,
};

fn power_of(w: &World, e: Entity) -> i32 {
    w.get::<Fighter>(e)
        .expect("a combatant has a Fighter")
        .power
}

fn hp_of(w: &World, e: Entity) -> i32 {
    w.get::<Fighter>(e).expect("a combatant has a Fighter").hp
}

fn max_hp_of(w: &World, e: Entity) -> i32 {
    w.get::<Fighter>(e)
        .expect("a combatant has a Fighter")
        .max_hp
}

fn pos_of(w: &World, e: Entity) -> Option<Position> {
    w.get::<Position>(e).copied()
}

fn armor_plus(w: &World, item: Entity) -> i32 {
    w.get::<ArmorBonus>(item).map_or(0, |b| b.0)
}

// ---------------------------------------------------------------------------
// The on-hit table: one test per row
// ---------------------------------------------------------------------------

/// The wiring test the other on-hit tests lean on: `resolve_attack` really
/// does fire the table, rather than the table being a list nothing reads.
/// Everything below goes through `fire_on_hit` directly, so combat's dice
/// cannot make a row's test flaky — this is the one test that proves the two
/// are connected at all.
#[test]
fn a_landed_blow_fires_the_on_hit_table() {
    // Swept because the swing has to actually connect: a big power die
    // against no armour, on a foe with enough HP that the blow is never
    // lethal.
    let drained = (0..64u64).any(|seed| {
        let mut w = arena(seed);
        let biter = dummy(&mut w, 10, 40);
        w.entity_mut(biter).insert(Venomous);
        let victim = dummy(&mut w, 10_000, 10);

        let before = power_of(&w, victim);
        resolve_attack(&mut w, biter, victim);
        power_of(&w, victim) < before
    });
    assert!(
        drained,
        "no swing in 64 seeds reached the ability table — combat is not firing it"
    );
}

#[test]
fn a_corroding_blow_eats_the_plus_off_what_the_victim_wears() {
    let mut w = arena(7);
    let victim = hero(&mut w, at(10, 10), 20, 8);
    let armor = worn(&mut w, victim, Slot::Body);

    let attacker = dummy(&mut w, 10, 4);
    w.entity_mut(attacker).insert(RustsArmor);

    let before = armor_plus(&w, armor);
    fire_on_hit(&mut w, attacker, victim, CLEAN);

    assert!(
        armor_plus(&w, armor) < before,
        "the armour came through the corrosion no weaker ({before} -> {})",
        armor_plus(&w, armor)
    );
}

#[test]
fn a_venomous_bite_drains_the_victims_power() {
    let mut w = arena(1);
    let biter = dummy(&mut w, 10, 4);
    w.entity_mut(biter).insert(Venomous);
    let victim = dummy(&mut w, 10, 8);

    let before = power_of(&w, victim);
    fire_on_hit(&mut w, biter, victim, CLEAN);

    assert!(
        power_of(&w, victim) < before,
        "the venom left the victim as strong as it found them"
    );
}

/// A bite the player *lands* used to drain in total silence — the log line
/// was gated on the victim being the player, never on the victim existing.
/// Fixed to match `stagger`/`blind`'s split: second person for the player,
/// the victim's own name in the third otherwise.
#[test]
fn a_venomous_bite_announces_itself_whoever_it_bites() {
    let mut w = arena(1);
    let biter = dummy(&mut w, 10, 4);
    w.entity_mut(biter).insert(Venomous);
    let victim = dummy(&mut w, 10, 8); // Name: "dummy", not the player

    fire_on_hit(&mut w, biter, victim, CLEAN);

    let said = &w.resource::<GameLog>().history;
    assert!(
        said.iter().any(|l| l.contains("dummy")),
        "a bite the player landed on a monster said nothing about it: {said:?}"
    );

    let mut w = arena(1);
    let biter = dummy(&mut w, 10, 4);
    w.entity_mut(biter).insert(Venomous);
    let victim = hero(&mut w, at(11, 10), 10, 8);

    fire_on_hit(&mut w, biter, victim, CLEAN);

    assert!(
        w.resource::<GameLog>()
            .history
            .contains(&"Venom courses through you — your strength ebbs away.".to_string()),
        "the player-victim line changed shape"
    );
}

/// The `SustainsStrength` guard, which is currently written out three times
/// (`abilities.rs`, `traps.rs`, `spells.rs`). This covers the venom copy; the
/// other two belong to the trap and spell suites.
#[test]
fn sustained_strength_holds_against_venom() {
    let mut w = arena(1);
    let biter = dummy(&mut w, 10, 4);
    w.entity_mut(biter).insert(Venomous);
    let victim = dummy(&mut w, 10, 8);
    w.entity_mut(victim).insert(SustainsStrength);

    let before = power_of(&w, victim);
    fire_on_hit(&mut w, biter, victim, CLEAN);

    assert_eq!(
        power_of(&w, victim),
        before,
        "a sustained strength gave ground to venom"
    );
}

#[test]
fn a_draining_touch_lowers_the_victims_ceiling() {
    let mut w = arena(1);
    let attacker = dummy(&mut w, 10, 4);
    w.entity_mut(attacker).insert(Vampiric);
    let victim = dummy(&mut w, 20, 8);

    let before = max_hp_of(&w, victim);
    fire_on_hit(&mut w, attacker, victim, CLEAN);
    let after = max_hp_of(&w, victim);

    assert!(
        after < before,
        "the drain left the ceiling where it was ({before} -> {after})"
    );
    assert!(
        hp_of(&w, victim) <= after,
        "current HP was left above the new ceiling"
    );
}

/// Same fix, same shape, for the vampire's touch.
#[test]
fn a_draining_touch_announces_itself_whoever_it_drains() {
    let mut w = arena(1);
    let attacker = dummy(&mut w, 10, 4);
    w.entity_mut(attacker).insert(Vampiric);
    let victim = dummy(&mut w, 20, 8); // Name: "dummy", not the player

    fire_on_hit(&mut w, attacker, victim, CLEAN);

    let said = &w.resource::<GameLog>().history;
    assert!(
        said.iter().any(|l| l.contains("dummy")),
        "a drain the player landed on a monster said nothing about it: {said:?}"
    );
}

#[test]
fn a_binding_bite_pins_the_victim() {
    let mut w = arena(1);
    let attacker = dummy(&mut w, 10, 4);
    w.entity_mut(attacker).insert(Binds);
    let victim = dummy(&mut w, 10, 8);

    fire_on_hit(&mut w, attacker, victim, CLEAN);

    assert!(
        w.get::<Pinned>(victim).is_some(),
        "the bite should have pinned, and with steel rather than words"
    );
    assert!(
        turns_left(&w, victim, Grant::of::<Pinned>()).is_some_and(|n| n > 0),
        "pinned for no turns at all"
    );
}

/// Chance-gated: swept rather than pinned to the odds, and checked against a
/// marker-free control so the sweep cannot pass on some other paralysis.
#[test]
fn a_freezing_touch_can_paralyse_and_nothing_else_does() {
    let froze = (0..64u64).any(|seed| {
        let mut w = arena(seed);
        let attacker = dummy(&mut w, 10, 4);
        w.entity_mut(attacker).insert(Freezing);
        let victim = dummy(&mut w, 10, 8);
        fire_on_hit(&mut w, attacker, victim, CLEAN);
        w.get::<Paralyzed>(victim).is_some()
    });
    assert!(froze, "no touch in 64 seeds ever froze anyone");

    let froze_without = (0..64u64).any(|seed| {
        let mut w = arena(seed);
        let attacker = dummy(&mut w, 10, 4);
        let victim = dummy(&mut w, 10, 8);
        fire_on_hit(&mut w, attacker, victim, CLEAN);
        w.get::<Paralyzed>(victim).is_some()
    });
    assert!(
        !froze_without,
        "something with no Freezing marker paralysed its victim"
    );
}

/// A monster's `Paralyzed` marker is tint-only in the HUD, but the log should
/// not stay just as silent: `conditions::set_speed` already prints a generic
/// slow-down line for *any* monster regardless of what caused it (that part is
/// unconditional, on purpose — a wand of slow monster wants exactly that
/// line). Paralysis earns a second, dedicated line on top of it — but only
/// when the player could actually watch it happen, the same rule
/// `conditions::report_cure` already holds a mending monster to.
#[test]
fn a_visible_monsters_paralysis_gets_a_line_of_its_own() {
    let mut w = arena(1);
    let p = hero(&mut w, at(5, 5), 20, 4);
    let victim = creature(&mut w, at(6, 5), 10, 3);
    w.get_mut::<Viewshed>(p).unwrap().visible_tiles = vec![(6, 5)];

    paralyse(&mut w, victim);

    let name = w.get::<Name>(victim).unwrap().what.clone();
    let mentions = w
        .resource::<GameLog>()
        .history
        .iter()
        .filter(|l| l.contains(&name))
        .count();
    assert_eq!(
        mentions,
        2,
        "a paralysis landing in plain sight should say so, on top of the \
         generic slow-down: {:?}",
        w.resource::<GameLog>().history
    );
}

/// The other half: nothing new is said about a monster paralysed out of
/// sight — it still gets the generic slow-down line `set_speed` always
/// prints, but not the dedicated paralysis line, which would leak that
/// something happened in a room the player has never seen.
#[test]
fn an_unseen_monsters_paralysis_only_gets_the_generic_slow_line() {
    let mut w = arena(1);
    let p = hero(&mut w, at(5, 5), 20, 4);
    let victim = creature(&mut w, at(50, 50), 10, 3);
    w.get_mut::<Viewshed>(p).unwrap().visible_tiles = vec![(6, 5)];

    paralyse(&mut w, victim);

    let name = w.get::<Name>(victim).unwrap().what.clone();
    let mentions = w
        .resource::<GameLog>()
        .history
        .iter()
        .filter(|l| l.contains(&name))
        .count();
    assert_eq!(
        mentions,
        1,
        "a paralysis nobody could see should stay as quiet as it already \
         was: {:?}",
        w.resource::<GameLog>().history
    );
}

#[test]
fn an_erratic_striker_hops_after_every_blow_it_lands() {
    let mut w = arena(3);
    let victim = hero(&mut w, at(10, 10), 20, 8);
    let hopper = creature(&mut w, at(10, 10), 5, 4);
    w.entity_mut(hopper).insert(Batty);

    let before = pos_of(&w, hopper).expect("the hopper stands somewhere");
    fire_on_hit(&mut w, hopper, victim, CLEAN);
    let after = pos_of(&w, hopper).expect("the hopper still stands somewhere");

    assert_ne!(before, after, "it landed a blow and stayed put");
    assert!(
        w.get::<EntityMoved>(hopper).is_some(),
        "the hop left no EntityMoved, so it could hop onto a trap without springing it"
    );
}

#[test]
fn a_thief_that_flees_takes_something_loose_and_goes() {
    let mut w = arena(11);
    let victim = hero(&mut w, at(10, 10), 20, 8);
    pack_item(&mut w, victim, "loose thing");

    let thief = creature(&mut w, at(10, 10), 5, 4);
    w.entity_mut(thief).insert(StealsAndFlees);
    let stood = pos_of(&w, thief).expect("the thief stands somewhere");

    fire_on_hit(&mut w, thief, victim, CLEAN);

    assert!(
        w.get::<Backpack>(victim)
            .is_some_and(|b| b.items.is_empty()),
        "the thief left the pack alone"
    );
    assert_ne!(
        Some(stood),
        pos_of(&w, thief),
        "it robbed the victim and then stood there waiting"
    );
}

/// Worn gear lives in the pack too, the way `initialize_world` stows the
/// starting kit before putting it on. A thief that flees takes only what is
/// loose: stripping the wearer is the other thief's trick.
#[test]
fn a_thief_that_flees_never_lifts_worn_gear() {
    for seed in 0..32u64 {
        let mut w = arena(seed);
        let victim = hero(&mut w, at(10, 10), 20, 8);
        let armour = worn(&mut w, victim, Slot::Body);
        w.get_mut::<Backpack>(victim).unwrap().items.push(armour);
        pack_item(&mut w, victim, "loose thing");

        let thief = creature(&mut w, at(10, 10), 5, 4);
        w.entity_mut(thief).insert(StealsAndFlees);
        fire_on_hit(&mut w, thief, victim, CLEAN);

        assert_eq!(
            w.get::<Backpack>(victim).unwrap().items,
            vec![armour],
            "seed {seed}: the thief went for the armour on the victim's back"
        );
    }
}

/// And a thief whose hand closes on the Element of Yoord blows apart in gore
/// on the spot, leaving the Element where it was.
#[test]
fn a_thief_that_reaches_for_the_element_blows_apart() {
    let mut w = arena(11);
    let victim = hero(&mut w, at(10, 10), 20, 8);
    let element = spawn_element_of_yoord(&mut w, at(0, 0));
    w.entity_mut(element).remove::<Position>();
    w.get_mut::<Backpack>(victim).unwrap().items.push(element);

    let thief = creature(&mut w, at(11, 10), 5, 4);
    w.entity_mut(thief).insert(StealsAndFlees);
    fire_on_hit(&mut w, thief, victim, CLEAN);

    assert!(
        w.get_entity(thief).is_none(),
        "the thief is still in one piece"
    );
    assert_eq!(
        w.get::<Backpack>(victim).unwrap().items,
        vec![element],
        "the Element stays where it was"
    );
    assert!(
        w.resource::<GameLog>()
            .history
            .iter()
            .any(|l| l == &strings::element_bursts_thief("creature")),
        "nothing said the thief burst"
    );
}

#[test]
fn a_thief_that_vanishes_strips_worn_gear_and_takes_it_with_them() {
    let mut w = arena(13);
    let victim = hero(&mut w, at(10, 10), 20, 8);
    worn(&mut w, victim, Slot::Body);
    assert!(
        equipped_in(&w, victim, Slot::Body).is_some(),
        "the test never got the gear on"
    );

    let thief = creature(&mut w, at(10, 10), 5, 4);
    w.entity_mut(thief).insert(StealsAndVanishes);

    fire_on_hit(&mut w, thief, victim, CLEAN);

    assert!(
        equipped_in(&w, victim, Slot::Body).is_none(),
        "the thief left the gear on its owner"
    );
    assert!(
        w.get_entity(thief).is_none(),
        "the thief took the gear and stayed on the floor"
    );
}

#[test]
fn a_charged_touch_is_spent_on_the_first_blow_it_lands() {
    let mut w = arena(5);
    let striker = hero(&mut w, at(10, 10), 20, 10);
    lend(
        &mut w,
        striker,
        Grant::of::<ConfusingTouch>(),
        Lifetime::Permanent,
    );
    let victim = creature(&mut w, at(11, 10), 10, 4);

    fire_on_hit(&mut w, striker, victim, CLEAN);

    assert!(
        w.get::<ConfusingTouch>(striker).is_none(),
        "the charge survived the blow it was supposed to be spent on"
    );
}

#[test]
fn a_heavy_swing_staggers_what_it_hits_and_costs_its_wielder_a_beat() {
    let mut w = arena(1);
    let wielder = hero(&mut w, at(10, 10), 20, 10);
    w.entity_mut(wielder).insert(HeavySwing);
    let victim = dummy(&mut w, 10, 4);

    fire_on_hit(&mut w, wielder, victim, CLEAN);

    assert!(
        w.get::<Asleep>(victim).is_some(),
        "the weight left the victim free to answer"
    );
    assert!(
        w.resource::<ExtraMonsterRound>().0,
        "the swing cost its wielder nothing"
    );
}

/// The player-only gate (`abilities::is_player`) that four rows re-check in
/// their own bodies. One test for the rule, on the row where it bites hardest.
#[test]
fn a_monster_swinging_the_same_weapon_gets_none_of_its_weight() {
    let mut w = arena(1);
    let wielder = dummy(&mut w, 20, 10);
    w.entity_mut(wielder).insert(HeavySwing);
    let victim = dummy(&mut w, 10, 4);

    fire_on_hit(&mut w, wielder, victim, CLEAN);

    assert!(
        w.get::<Asleep>(victim).is_none(),
        "a monster's heavy swing staggered its victim — the player-only gate is gone"
    );
}

#[test]
fn a_recoiling_weapon_bites_the_hand_that_swings_it() {
    let mut w = arena(1);
    let wielder = hero(&mut w, at(10, 10), 20, 10);
    w.entity_mut(wielder).insert(SelfDamageOnHit);
    let victim = dummy(&mut w, 10, 4);

    let before = hp_of(&w, wielder);
    fire_on_hit(&mut w, wielder, victim, CLEAN);

    assert!(
        hp_of(&w, wielder) < before,
        "the recoil cost its wielder nothing"
    );
}

#[test]
fn a_momentum_weapon_builds_on_every_blow_that_lands() {
    let mut w = arena(17);
    let wielder = hero(&mut w, at(10, 10), 20, 10);
    let weapon = worn(&mut w, wielder, Slot::Hand);
    w.entity_mut(wielder).insert(BuildsMomentum);
    let victim = dummy(&mut w, 10, 4);

    let before = w.get::<Momentum>(weapon).map_or(0, |m| m.0);
    fire_on_hit(&mut w, wielder, victim, LETHAL);
    let after = w.get::<Momentum>(weapon).map_or(0, |m| m.0);

    assert!(
        after > before,
        "the landing blow built nothing ({before} -> {after})"
    );
}

/// The two gating booleans on a row, checked as a pair: acid does not care
/// that the armour turned the blow, and there is no point charming a corpse.
#[test]
fn the_row_gates_decide_which_blows_count() {
    // Venom needs skin: a glancing scrape carries none of it.
    let mut w = arena(1);
    let biter = dummy(&mut w, 10, 4);
    w.entity_mut(biter).insert(Venomous);
    let victim = dummy(&mut w, 10, 8);
    let before = power_of(&w, victim);
    fire_on_hit(&mut w, biter, victim, GLANCING);
    assert_eq!(
        power_of(&w, victim),
        before,
        "a glancing scrape carried venom through armour"
    );

    // Rust does not care — it landed on the armour, which is the point.
    let mut w = arena(19);
    let wearer = hero(&mut w, at(10, 10), 20, 8);
    let armor = worn(&mut w, wearer, Slot::Body);
    let attacker = dummy(&mut w, 10, 4);
    w.entity_mut(attacker).insert(RustsArmor);
    let before = armor_plus(&w, armor);
    fire_on_hit(&mut w, attacker, wearer, GLANCING);
    assert!(
        armor_plus(&w, armor) < before,
        "a glancing blow spared the armour it landed on"
    );

    // There is no point charming a corpse.
    let mut w = arena(1);
    let striker = hero(&mut w, at(10, 10), 20, 10);
    lend(
        &mut w,
        striker,
        Grant::of::<ConfusingTouch>(),
        Lifetime::Permanent,
    );
    let victim = dummy(&mut w, 10, 4);
    fire_on_hit(&mut w, striker, victim, LETHAL);
    assert!(
        w.get::<ConfusingTouch>(striker).is_some(),
        "the charge was spent on a killing blow"
    );
}

/// `fire_on_hit`'s blanket guard: a ward turns aside everything riding on a
/// blow, whatever the row. Phase 3 moves this check, so it needs holding down
/// first.
#[test]
fn a_magic_ward_turns_aside_what_rides_on_a_blow() {
    let mut w = arena(1);
    let biter = dummy(&mut w, 10, 4);
    w.entity_mut(biter).insert(Venomous);
    let victim = dummy(&mut w, 10, 8);
    w.entity_mut(victim).insert(MagicWard);

    let before = power_of(&w, victim);
    fire_on_hit(&mut w, biter, victim, CLEAN);

    assert_eq!(
        power_of(&w, victim),
        before,
        "venom reached a warded victim"
    );
}

// ---------------------------------------------------------------------------
// The passive table
// ---------------------------------------------------------------------------

/// Note what regeneration actually mends: `rings::regenerate` lifts a
/// condition, or failing that puts back a point of drained *power*. It is not
/// an HP drip, and a test that assumed one would be asserting a mechanic the
/// game does not have.
#[test]
fn regeneration_puts_back_what_was_drained() {
    let mut w = arena(23);
    let bearer = hero(&mut w, at(10, 10), 20, 8);
    w.entity_mut(bearer).insert(Regenerates);
    if let Some(mut f) = w.get_mut::<Fighter>(bearer) {
        f.power = 1;
    }

    let before = power_of(&w, bearer);
    for _ in 0..64 {
        ability_system(&mut w);
    }
    let mended = power_of(&w, bearer);

    assert!(
        mended > before,
        "64 turns of regeneration mended nothing ({before} -> {mended})"
    );
    assert!(
        mended <= w.get::<Fighter>(bearer).unwrap().max_power,
        "regeneration mended past what was ever lost"
    );
}

#[test]
fn nothing_regenerates_without_the_marker() {
    let mut w = arena(23);
    let bearer = hero(&mut w, at(10, 10), 20, 8);
    if let Some(mut f) = w.get_mut::<Fighter>(bearer) {
        f.power = 1;
    }

    for _ in 0..64 {
        ability_system(&mut w);
    }

    assert_eq!(
        power_of(&w, bearer),
        1,
        "an unmarked bearer mended themselves on their own"
    );
}

#[test]
fn teleportitis_eventually_moves_its_bearer() {
    let mut w = arena(29);
    let bearer = hero(&mut w, at(10, 10), 20, 8);
    w.entity_mut(bearer).insert(Teleportitis);
    let start = pos_of(&w, bearer).expect("the bearer stands somewhere");

    // The odds are long on purpose, so this sweeps turns rather than seeds.
    // The claim is "eventually", not "at this rate".
    let jumped = (0..4000).any(|_| {
        ability_system(&mut w);
        pos_of(&w, bearer) != Some(start)
    });

    assert!(jumped, "4000 turns of teleportitis never moved anyone");
}

#[test]
fn an_aggravating_bearer_is_heard_across_the_floor() {
    let mut w = arena(31);
    let bearer = hero(&mut w, at(10, 10), 20, 8);
    w.entity_mut(bearer).insert(AggravatesMonsters);
    for n in 1..=4u16 {
        creature(&mut w, at(10 + n, 10), 5, 4);
    }

    let before = aggravated_count(&mut w);
    for _ in 0..64 {
        ability_system(&mut w);
    }
    let after = aggravated_count(&mut w);

    assert!(
        after > before,
        "64 turns of yipping and not one creature turned around ({before} -> {after})"
    );
}

/// How many creatures are homing in on a tile the bearer once stood on.
/// Aggravation works by switching a mob to a fifth `MovementType`, so a change
/// in this count is the ability landing. (`MovementType` derives no `Debug`,
/// hence a count rather than a snapshot.)
fn aggravated_count(w: &mut World) -> usize {
    w.query::<&Mob>()
        .iter(w)
        .filter(|m| matches!(m.movement_type, MovementType::Aggravated { .. }))
        .count()
}

// ---------------------------------------------------------------------------
// The markers that reach no table at all
// ---------------------------------------------------------------------------

/// `Gorgon` — hand-called from four sites (`combat.rs` twice, `wands.rs`,
/// `throwing.rs`). This covers the melee one.
#[test]
fn looking_upon_a_gorgon_turns_the_player_to_stone() {
    let mut w = arena(37);
    let looker = hero(&mut w, at(10, 10), 20, 8);
    let seen = creature(&mut w, at(11, 10), 10, 4);
    w.entity_mut(seen).insert(Gorgon);

    resolve_attack(&mut w, looker, seen);

    assert!(
        w.get::<Petrified>(looker).is_some(),
        "meeting a gorgon's eyes should turn the looker to stone"
    );
    assert!(
        w.get::<Asleep>(looker).is_none(),
        "the gaze put the looker to sleep — stone is its own hold"
    );
}

#[test]
fn a_gorgons_own_kind_is_unmoved_by_the_gaze() {
    let mut w = arena(37);
    let bystander = hero(&mut w, at(10, 10), 20, 8);
    let seen = creature(&mut w, at(11, 10), 10, 4);
    w.entity_mut(seen).insert(Gorgon);
    let other = creature(&mut w, at(12, 10), 10, 4);

    resolve_attack(&mut w, other, seen);

    assert!(
        w.get::<Petrified>(other).is_none(),
        "a monster was petrified by a gaze only a person's eyes can meet"
    );
    assert!(
        w.get::<Petrified>(bystander).is_none(),
        "the player was petrified by a gaze they never met"
    );
}

/// The bug this hold was split out of `Asleep` for: stone let go with the
/// sleeper's line, and the player who had just been turned to stone was told
/// they had shaken off their drowsiness. The lines are read off the rows
/// rather than written out here — what this holds still is *which row speaks*.
#[test]
fn stone_lets_go_speaking_of_stone_and_never_of_sleep() {
    let stone = Effect::by_id("petrified")
        .and_then(|e| e.ends)
        .expect("petrification says something when it lets go");
    let slumber = Effect::by_id("asleep")
        .and_then(|e| e.ends)
        .expect("sleep says something when it lets go");

    let mut w = arena(37);
    let looker = hero(&mut w, at(10, 10), 20, 8);
    let seen = creature(&mut w, at(11, 10), 10, 4);
    w.entity_mut(seen).insert(Gorgon);
    fire_on_targeted(&mut w, looker, seen);

    // Long enough that any hold this could have applied has run out.
    for _ in 0..64 {
        tick_effects(&mut w);
    }

    let said: Vec<String> = w.resource::<GameLog>().history.clone();
    assert!(
        said.iter().any(|l| l == stone),
        "coming out of stone never said so: {said:?}"
    );
    assert!(
        !said.iter().any(|l| l == slumber),
        "coming out of stone was reported as waking up: {said:?}"
    );
}

/// Stone costs you your turns, exactly as sleeping gas does — that half of the
/// gaze is unchanged, and it is the half the player feels first.
#[test]
fn a_petrified_player_can_do_nothing_at_all() {
    let mut w = arena(37);
    let looker = hero(&mut w, at(10, 10), 20, 8);
    let seen = creature(&mut w, at(11, 10), 10, 4);
    w.entity_mut(seen).insert(Gorgon);

    assert!(
        !models::traps::player_incapacitated(&mut w),
        "the player had lost their turn before they ever looked"
    );
    fire_on_targeted(&mut w, looker, seen);
    assert!(
        models::traps::player_incapacitated(&mut w),
        "a player turned to stone was still allowed to act"
    );
}

/// `Splits` — reached through `helpers::took_damage`'s hardcoded dispatch,
/// which Phase 2 turns into a moment.
///
/// **The one test here that touches a content table, and it is forced.**
/// `maybe_split` looks the victim up in the bestiary by its own `Name` and
/// panics on a miss, so a splitter must be named something the table knows.
/// The name is taken off the table at runtime rather than written in here, so
/// this test still names nothing — but the coupling belongs to the mechanic,
/// not to the test, and it is logged as Phase 2 work.
#[test]
fn a_wounded_splitter_becomes_two() {
    let species = any_species();
    let mut w = arena(41);
    let attacker = hero(&mut w, at(10, 10), 20, 10);
    let splitter = splitter_named(&mut w, species, at(11, 10), 10_000);

    let before = count_named(&mut w, species);
    resolve_attack(&mut w, attacker, splitter);
    let after = count_named(&mut w, species);

    assert!(
        after > before,
        "a wounded splitter did not split ({before} -> {after})"
    );
}

#[test]
fn a_slain_splitter_leaves_nothing_behind() {
    let species = any_species();
    let mut w = arena(41);
    let attacker = hero(&mut w, at(10, 10), 20, 10_000);
    let splitter = splitter_named(&mut w, species, at(11, 10), 1);

    resolve_attack(&mut w, attacker, splitter);

    // `settle_the_dead` may already have taken it off the floor, so "dead"
    // means gone *or* out of HP.
    let dead = w
        .get::<Fighter>(splitter)
        .is_none_or(|f: &Fighter| f.hp <= 0);
    assert!(dead, "the test never actually killed the splitter");
    assert!(
        count_named(&mut w, species) <= 1,
        "it split on the blow that killed it"
    );
}

/// Any species the bestiary knows, discovered rather than named. Only
/// `maybe_split`'s own name lookup needs this.
fn any_species() -> &'static str {
    content_names()
        .into_iter()
        .find(|&(category, _)| category == "monster")
        .map(|(_, name)| name)
        .expect("the bestiary has at least one row")
}

fn splitter_named(w: &mut World, species: &str, pos: Position, hp: i32) -> Entity {
    let e = creature(w, pos, hp, 4);
    w.entity_mut(e).insert(Name {
        what: species.to_string(),
    });
    w.entity_mut(e).insert(Splits);
    e
}

fn count_named(w: &mut World, what: &str) -> usize {
    w.query::<&Name>()
        .iter(w)
        .filter(|n| n.what == what)
        .count()
}

/// `Flies` — a bare `continue` in `trap_system`.
#[test]
fn a_flier_crosses_a_trap_untouched_and_a_walker_does_not() {
    assert!(
        trap_sprang(43, false),
        "the test's walker never triggered the trap it stood on"
    );
    assert!(!trap_sprang(43, true), "a flier set off a trap");
}

/// Stands a creature on a freshly planted trap and runs the trap system.
/// Reports whether the trap went off.
fn trap_sprang(seed: u64, flies: bool) -> bool {
    let mut w = arena(seed);
    let here = at(10, 10);
    hero(&mut w, at(20, 20), 20, 8);

    w.spawn((
        Name {
            what: "trap".into(),
        },
        Trap {
            effect: TrapEffect::Dart,
            reveal: TrapReveal::Sight,
            revealed: true,
        },
        here,
    ));

    let mover = creature(&mut w, here, 20, 8);
    if flies {
        w.entity_mut(mover).insert(Flies);
    }
    w.entity_mut(mover).insert(EntityMoved);

    let (power_before, hp_before) = (power_of(&w, mover), hp_of(&w, mover));
    trap_system(&mut w);
    power_of(&w, mover) < power_before || hp_of(&w, mover) < hp_before
}

/// `FireBreath` — hand-wired into `ai::step_one_mob`. The claim is that a
/// breather sometimes answers with fire, which reaches tiles a claw cannot: a
/// second creature standing beside the player gets hurt too.
#[test]
fn a_breather_sometimes_answers_with_fire_rather_than_claws() {
    let splashed = (0..48u64).any(|seed| breath_round(seed, true));
    assert!(
        splashed,
        "no breather in 48 seeds ever breathed — FireBreath is not reaching the AI"
    );

    let splashed_without = (0..48u64).any(|seed| breath_round(seed, false));
    assert!(
        !splashed_without,
        "something with no FireBreath marker still breathed fire"
    );
}

/// Puts a breather next to the player with a bystander beside them, runs one
/// AI round, and reports whether the bystander was caught in a blast — which
/// only a breath, never a claw, can do.
fn breath_round(seed: u64, breathes: bool) -> bool {
    let mut w = arena(seed);
    let here = at(10, 10);
    hero(&mut w, here, 10_000, 8);

    let breather = creature(&mut w, at(11, 10), 20, 8);
    if breathes {
        w.entity_mut(breather).insert(FireBreath);
    }

    let bystander = creature(&mut w, at(10, 11), 10_000, 8);
    let before = hp_of(&w, bystander);

    // A mob only acts on what is in the player's viewshed (`ai::notices`), so
    // the floor has to be looked at once after everything is placed.
    see(&mut w);
    ai(&mut w);

    hp_of(&w, bystander) < before
}

// ---------------------------------------------------------------------------
// The tables as data
// ---------------------------------------------------------------------------

/// Every ability's marker is in `EFFECTS`, with no exceptions. A marker that
/// is not half-works: it attaches and runs for the session, then vanishes on
/// load and cannot be cancelled — "the quiet failure to look for when an
/// effect works until you reload" (`docs/how-to/add-an-effect.md`).
///
/// `ConfusingTouch` was the one exemption until it and `Bided` were made
/// effects proper. There is no longer a legitimate reason for a row's marker
/// to sit outside the list, so this asserts none do.
#[test]
fn every_ability_marker_is_in_the_save_format() {
    let exempt = ABILITIES
        .iter()
        .filter(|a| a.effect.effect_id().is_none())
        .count();

    assert_eq!(
        exempt, 0,
        "an ability row's marker is missing from EFFECTS — it will work until \
         the next reload and then quietly stop"
    );
}

/// No two rows may be armed by the same marker in the same table — two rows
/// sharing one `Grant` fire together forever, which is never what was meant.
#[test]
fn no_two_ability_rows_share_a_marker() {
    // One marker may arm rows at *different* moments — that is the point of
    // the `when` field. Two rows at the same moment fire together forever,
    // which is never what was meant.
    let mut seen: Vec<(&str, Moment)> = Vec::new();
    for ability in ABILITIES {
        let id = ability
            .effect
            .effect_id()
            .expect("a row's marker is registered");
        assert!(
            !seen.contains(&(id, ability.when)),
            "two ABILITIES rows are armed by {id:?} at the same moment — both fire, always"
        );
        seen.push((id, ability.when));
    }
}

/// A passive that can never fire is a dead row, and one that always fires is a
/// rule wearing a probability. Both are mistakes a rebalance can introduce.
#[test]
fn every_passive_has_odds_it_can_lose_and_win() {
    for ability in ABILITIES {
        let Moment::EachTurn(chance) = ability.when else {
            continue;
        };
        assert!(
            chance > 0.0 && chance <= 1.0,
            "a passive's chance is outside the range where it means anything"
        );
        assert!(
            ability.flavour.is_some_and(|f| !f.is_empty()),
            "a passive with no flavour line fires in silence"
        );
    }
}

/// The ids are what a save file stores, so two rows sharing one would make
/// two effects indistinguishable on disk, and an empty one would store
/// nothing at all. Neither is reachable by accident today; both become
/// reachable the moment somebody copies a row to write a new one.
#[test]
fn every_effect_id_is_unique_and_says_something() {
    let mut seen: Vec<&str> = Vec::new();
    for effect in EFFECTS {
        assert!(
            !effect.id.is_empty(),
            "an EFFECTS row has an empty id, so it would save as nothing"
        );
        assert!(
            !seen.contains(&effect.id),
            "two EFFECTS rows share the id {:?} — on disk they are the same effect",
            effect.id
        );
        seen.push(effect.id);
    }
}

/// An id is the identity, and it is written out rather than derived from the
/// type name so that renaming the struct cannot silently rename the save key.
/// That freedom cuts both ways: a copied row keeps the id it was copied from,
/// and nothing about it looks wrong.
///
/// `Grant::effect_id` resolves a component back to the row that registers it,
/// so a row whose component answers to a *different* id is a row that was
/// copied and half-edited — two rows claiming one component, with the second
/// one dead on disk.
#[test]
fn every_row_is_the_one_its_component_answers_to() {
    for effect in EFFECTS {
        let resolved = effect
            .grant
            .effect_id()
            .expect("a registered effect resolves to a row");
        assert_eq!(
            resolved, effect.id,
            "the row {:?} carries a component that already belongs to {:?} — \
             one of the two is dead on disk",
            effect.id, resolved
        );
    }
}

// ---------------------------------------------------------------------------
// The effects ledger
// ---------------------------------------------------------------------------

/// The invariant the three overlapping bitsets used to buy by intersecting
/// each other: two sources lending one effect are two claims, and losing one
/// must not strip what the other still lends.
#[test]
fn losing_one_source_leaves_what_another_still_lends() {
    let mut w = arena(1);
    let bearer = hero(&mut w, at(10, 10), 20, 8);

    lend(
        &mut w,
        bearer,
        Grant::of::<FireImmune>(),
        Lifetime::Permanent,
    );
    lend(&mut w, bearer, Grant::of::<FireImmune>(), Lifetime::Floor);
    assert!(
        w.get::<FireImmune>(bearer).is_some(),
        "lending did not attach"
    );

    // A staircase takes the floor-lent copy and nothing else.
    clear_floor_grants(&mut w, bearer);

    assert!(
        w.get::<FireImmune>(bearer).is_some(),
        "the staircase stripped an effect the creature was born with, \
         because something else happened to lend the same one"
    );
}

#[test]
fn the_last_source_going_does_take_the_effect_away() {
    let mut w = arena(1);
    let bearer = hero(&mut w, at(10, 10), 20, 8);

    lend(&mut w, bearer, Grant::of::<FireImmune>(), Lifetime::Floor);
    clear_floor_grants(&mut w, bearer);

    assert!(
        w.get::<FireImmune>(bearer).is_none(),
        "nothing was lending it any more and it stayed anyway"
    );
}

/// Gear-lent effects are not written to a save: the gear itself is saved, and
/// `sync_equipment_effects` lends them again on load. `SavedLifetime` has no
/// variant that could express one, so this is belt to that braces — it checks
/// the entry never reaches the code that would have to leave it out.
#[test]
fn what_gear_lends_is_left_out_of_the_save() {
    let mut w = arena(1);
    let bearer = hero(&mut w, at(10, 10), 20, 8);
    let ring = worn(&mut w, bearer, Slot::Finger);

    lend(
        &mut w,
        bearer,
        Grant::of::<FireImmune>(),
        Lifetime::Permanent,
    );
    lend(
        &mut w,
        bearer,
        Grant::of::<SeesInvisible>(),
        Lifetime::WhileEquipped(ring),
    );

    let saved = effects_of(&w, bearer);
    let ids: Vec<&str> = saved.iter().map(|h| h.id).collect();

    assert!(
        ids.contains(&"fire_immune"),
        "what the creature owns outright was left out of the save"
    );
    assert!(
        !ids.contains(&"sees_invisible"),
        "a borrowed ring's effect was saved as if the creature owned it"
    );
}

/// The payoff for saving effects by name rather than by position: an id this
/// build has no row for is one lost property, reported, rather than a
/// misread one. An index-based format could not have told the difference,
/// because every index would still have been a valid index.
#[test]
fn a_save_naming_an_effect_this_build_lacks_loses_only_that_one() {
    let mut w = arena(1);
    let bearer = hero(&mut w, at(10, 10), 20, 8);

    let held = [
        Held {
            id: "fire_immune",
            lifetime: Lifetime::Permanent,
        },
        Held {
            id: "a_row_this_build_does_not_have",
            lifetime: Lifetime::Permanent,
        },
    ];
    let unknown = attach_effects(&mut w.entity_mut(bearer), &held);

    assert_eq!(
        unknown,
        vec!["a_row_this_build_does_not_have"],
        "the unknown id was not reported, so a load would drop it in silence"
    );
    assert!(
        w.get::<FireImmune>(bearer).is_some(),
        "one unknown id cost the entity the effects that were fine"
    );
}

/// Taking a ring off lifts what that ring lent and nothing else — the job
/// `sync_equipment_effects` used to do by diffing bitsets.
#[test]
fn unequipping_lifts_only_what_that_item_lent() {
    let mut w = arena(1);
    let bearer = hero(&mut w, at(10, 10), 20, 8);
    let ring = worn(&mut w, bearer, Slot::Finger);
    w.entity_mut(ring).insert(Grants(LENDS_SIGHT));

    lend(
        &mut w,
        bearer,
        Grant::of::<FireImmune>(),
        Lifetime::Permanent,
    );
    sync_equipment_effects(&mut w, bearer);
    assert!(
        w.get::<SeesInvisible>(bearer).is_some(),
        "the ring never lent what it grants"
    );

    force_unequip(&mut w, ring);
    sync_equipment_effects(&mut w, bearer);

    assert!(
        w.get::<SeesInvisible>(bearer).is_none(),
        "the ring came off and kept lending"
    );
    assert!(
        w.get::<FireImmune>(bearer).is_some(),
        "taking a ring off stripped innate magic"
    );
}

/// Running the sync twice must not pile up duplicate claims — it runs every
/// turn for every pack-carrying creature.
#[test]
fn syncing_twice_lends_once() {
    let mut w = arena(1);
    let bearer = hero(&mut w, at(10, 10), 20, 8);
    let ring = worn(&mut w, bearer, Slot::Finger);
    w.entity_mut(ring).insert(Grants(LENDS_SIGHT));

    sync_equipment_effects(&mut w, bearer);
    let after_one = w.get::<Effects>(bearer).map(|l| l.0.len()).unwrap_or(0);
    for _ in 0..8 {
        sync_equipment_effects(&mut w, bearer);
    }
    let after_many = w.get::<Effects>(bearer).map(|l| l.0.len()).unwrap_or(0);

    assert_eq!(
        after_one, after_many,
        "the ledger grew an entry per turn for gear that never moved"
    );
}

// ---------------------------------------------------------------------------
// The clock
// ---------------------------------------------------------------------------

/// The general tick that replaced `snare_system`. One system now ages every
/// timed effect there will ever be, where the old one knew about exactly one
/// component and could not have handled a second without being copied.
#[test]
fn a_timed_effect_runs_down_one_turn_at_a_time_and_then_ends() {
    let mut w = arena(1);
    let victim = hero(&mut w, at(10, 10), 20, 8);
    hold(&mut w, victim, Grant::of::<Asleep>(), 3);

    for left in [2, 1] {
        tick_effects(&mut w);
        assert_eq!(
            turns_left(&w, victim, Grant::of::<Asleep>()),
            Some(left),
            "the clock did not lose exactly one turn"
        );
        assert!(
            w.get::<Asleep>(victim).is_some(),
            "it let go while turns were still on the clock"
        );
    }

    tick_effects(&mut w);
    assert!(
        w.get::<Asleep>(victim).is_none(),
        "the clock ran out and it held on anyway"
    );
}

/// A permanent effect has no clock, and the tick must leave it alone — it
/// walks every ledger on the floor, so an immunity is the thing most likely
/// to be aged by accident.
#[test]
fn the_clock_never_touches_an_effect_that_has_none() {
    let mut w = arena(1);
    let bearer = hero(&mut w, at(10, 10), 20, 8);
    lend(
        &mut w,
        bearer,
        Grant::of::<FireImmune>(),
        Lifetime::Permanent,
    );

    for _ in 0..64 {
        tick_effects(&mut w);
    }

    assert!(
        w.get::<FireImmune>(bearer).is_some(),
        "64 turns aged away something that was never on a clock"
    );
}

/// **A behaviour change, recorded deliberately.** The single `Snare` component
/// could only hold one kind at a time, so a bear trap closing on a sleeping
/// creature replaced the sleep. Three effects with three clocks means both are
/// true at once, and each ends when its own turns run out.
#[test]
fn two_holds_at_once_run_on_their_own_clocks() {
    let mut w = arena(1);
    let victim = hero(&mut w, at(10, 10), 20, 8);
    hold(&mut w, victim, Grant::of::<Asleep>(), 1);
    hold(&mut w, victim, Grant::of::<Pinned>(), 3);

    assert!(
        w.get::<Asleep>(victim).is_some() && w.get::<Pinned>(victim).is_some(),
        "one hold clobbered the other, the way the old Snare component did"
    );

    tick_effects(&mut w);

    assert!(
        w.get::<Asleep>(victim).is_none(),
        "the shorter hold outlived its own clock"
    );
    assert!(
        w.get::<Pinned>(victim).is_some(),
        "the shorter hold ending took the longer one with it"
    );
}

/// A second dose of the same thing is a longer hold, never two holds running
/// down side by side — and it never shortens one already running.
#[test]
fn a_second_dose_lengthens_a_hold_and_never_shortens_it() {
    let mut w = arena(1);
    let victim = hero(&mut w, at(10, 10), 20, 8);

    assert!(hold(&mut w, victim, Grant::of::<Asleep>(), 5));
    assert!(
        !hold(&mut w, victim, Grant::of::<Asleep>(), 2),
        "a weaker dose reported that it did something"
    );
    assert_eq!(
        turns_left(&w, victim, Grant::of::<Asleep>()),
        Some(5),
        "a weaker dose cut the sentence short"
    );

    assert!(hold(&mut w, victim, Grant::of::<Asleep>(), 9));
    assert_eq!(
        turns_left(&w, victim, Grant::of::<Asleep>()),
        Some(9),
        "a stronger dose did not extend the hold"
    );

    let entries = w
        .get::<Effects>(victim)
        .map(|l| l.0.iter().filter(|h| h.id == "asleep").count())
        .unwrap_or(0);
    assert_eq!(
        entries, 1,
        "doses stacked as separate clocks instead of one"
    );
}

// ---------------------------------------------------------------------------
// The afflictions, as one table
// ---------------------------------------------------------------------------

/// The guard that makes one table safe to have: every row names an effect the
/// registry knows, so a condition cannot be curable without being saveable.
#[test]
fn every_affliction_is_a_registered_effect() {
    for affliction in AFFLICTIONS {
        assert!(
            affliction.effect.effect_id().is_some(),
            "an AFFLICTIONS row names a component that is not in EFFECTS — \
             it would cure fine and vanish on reload"
        );
    }
}

/// Three lists in three orders became one, and the order in it is load-bearing:
/// `cure_one_condition` takes the first row it finds, so the table is sorted
/// worst-first. Blindness costs you the floor; confusion costs you half your
/// steps.
#[test]
fn a_cure_lifts_the_worst_affliction_first() {
    let mut w = arena(1);
    let victim = hero(&mut w, at(10, 10), 20, 8);
    lend(&mut w, victim, Grant::of::<Confused>(), Lifetime::Floor);
    lend(&mut w, victim, Grant::of::<Blind>(), Lifetime::Floor);

    assert!(cure_one_condition(&mut w, victim), "nothing was lifted");

    assert!(
        w.get::<Blind>(victim).is_none(),
        "the cure went for the lesser affliction first"
    );
    assert!(
        w.get::<Confused>(victim).is_some(),
        "one cure lifted two afflictions"
    );
}

/// `afflicted` and `cure_one_condition` used to be two hand-written lists that
/// could disagree. They walk one table now, so anything the first reports must
/// be something the second can actually lift.
#[test]
fn anything_reported_as_afflicted_can_be_cured() {
    for affliction in AFFLICTIONS {
        let mut w = arena(1);
        let victim = hero(&mut w, at(10, 10), 20, 8);
        lend(&mut w, victim, affliction.effect, Lifetime::Floor);

        assert!(
            afflicted(&w, victim),
            "an AFFLICTIONS row is not recognised as an affliction"
        );
        assert!(
            cure_one_condition(&mut w, victim),
            "something reported as afflicted could not be cured"
        );
        assert!(
            !affliction.effect.probe(&w, victim),
            "the cure reported success and left the affliction on"
        );
    }
}

/// A staircase takes every affliction, and says so once per affliction — the
/// third of the three lists, now the same table as the other two.
#[test]
fn a_staircase_lifts_every_affliction_and_names_each() {
    let mut w = arena(1);
    let player_e = hero(&mut w, at(10, 10), 20, 8);
    for affliction in AFFLICTIONS {
        lend(&mut w, player_e, affliction.effect, Lifetime::Floor);
    }

    clear_player_conditions(&mut w, player_e);

    for affliction in AFFLICTIONS {
        assert!(
            !affliction.effect.probe(&w, player_e),
            "a staircase left an affliction on"
        );
        let line = format!("You are no longer {}.", affliction.lifted_adjective);
        assert!(
            w.resource::<GameLog>().history.contains(&line),
            "the staircase lifted an affliction without saying so: {line:?}"
        );
    }
}

/// **The hazard the ledger introduces.** A condition attached with a bare
/// `insert` has no ledger entry and therefore no lifetime, so a staircase that
/// trusted the ledger alone would leave it on for the rest of the run. The
/// afflictions are floor-scoped by definition, so the staircase lifts them
/// whether or not anything recorded that it had.
#[test]
fn a_staircase_lifts_an_affliction_that_never_went_through_the_ledger() {
    let mut w = arena(1);
    let player_e = hero(&mut w, at(10, 10), 20, 8);
    w.entity_mut(player_e).insert(Blind);

    clear_player_conditions(&mut w, player_e);

    assert!(
        w.get::<Blind>(player_e).is_none(),
        "a condition attached without `lend` became permanent"
    );
}

/// The other side of that: the staircase must not take what something else is
/// still lending. This is the gear path — a ring of perception outlives the
/// potion, and outlives the floor.
#[test]
fn a_staircase_leaves_what_gear_is_still_lending() {
    let mut w = arena(1);
    let player_e = hero(&mut w, at(10, 10), 20, 8);
    let ring = worn(&mut w, player_e, Slot::Finger);

    lend(
        &mut w,
        player_e,
        Grant::of::<SeesInvisible>(),
        Lifetime::Floor,
    );
    lend(
        &mut w,
        player_e,
        Grant::of::<SeesInvisible>(),
        Lifetime::WhileEquipped(ring),
    );

    clear_player_conditions(&mut w, player_e);

    assert!(
        w.get::<SeesInvisible>(player_e).is_some(),
        "the staircase took the sight a worn ring was still lending"
    );
}

/// And the sharp edge of the blanket lift: the afflictions are lifted whether
/// or not the ledger recorded them, so the one thing that could go wrong is
/// lifting one a *worn item* is still lending.
///
/// Nothing in the game lends an affliction from gear today. This is the test
/// that keeps the guard honest anyway, because the first cursed ring of
/// blindness would otherwise be cured by walking downstairs.
#[test]
fn a_staircase_leaves_an_affliction_a_worn_item_is_lending() {
    let mut w = arena(1);
    let player_e = hero(&mut w, at(10, 10), 20, 8);
    let cursed = worn(&mut w, player_e, Slot::Finger);

    lend(
        &mut w,
        player_e,
        Grant::of::<Blind>(),
        Lifetime::WhileEquipped(cursed),
    );

    clear_player_conditions(&mut w, player_e);

    assert!(
        w.get::<Blind>(player_e).is_some(),
        "walking downstairs cured a blindness the ring is still causing"
    );
}

// ---------------------------------------------------------------------------
// What Look warns about
// ---------------------------------------------------------------------------

/// The test the old arrangement could not have: every ability a *monster* can
/// carry has words for `Look` to warn with.
///
/// The engine used to keep its own 15-row list of these, in another crate,
/// with nothing tying it to the ability tables. Adding a thirteenth on-hit
/// ability and forgetting that list meant `Look` quietly stopped warning about
/// it. Now the phrase is a field on the effect row, and this walks the
/// bestiary to check that anything a creature is actually born with, and that
/// actually does something, says so.
#[test]
fn every_monster_ability_has_words_for_look() {
    let armed: Vec<&str> = ABILITIES
        .iter()
        .filter_map(|a| a.effect.effect_id())
        .collect();

    for def in BESTIARY {
        for grant in def.grants {
            let Some(id) = grant.effect_id() else {
                continue;
            };
            if !armed.contains(&id) {
                continue;
            }
            let effect = Effect::by_id(id).expect("a registered effect");
            assert!(
                effect.beware.is_some(),
                "{:?} arms an ability on a creature in the bestiary but has no \
                 `beware` line, so Look will not warn about it",
                id
            );
        }
    }
}

/// And the other direction: `dangers_of` reports what a creature carries and
/// nothing it does not.
#[test]
fn dangers_are_read_off_the_creature_not_guessed() {
    let mut w = arena(1);
    let plain = creature(&mut w, at(10, 10), 10, 4);
    assert!(
        dangers_of(&w, plain).is_empty(),
        "a creature with no magic was reported as dangerous"
    );

    let nasty = creature(&mut w, at(11, 10), 10, 4);
    lend(&mut w, nasty, Grant::of::<Venomous>(), Lifetime::Permanent);
    lend(&mut w, nasty, Grant::of::<Gorgon>(), Lifetime::Permanent);
    // Carried, but nothing a Look should warn about: the player cannot be hurt
    // by a creature's own immunity.
    lend(
        &mut w,
        nasty,
        Grant::of::<FireImmune>(),
        Lifetime::Permanent,
    );

    let warned = dangers_of(&w, nasty);
    assert!(warned.contains(&"venomous bite"), "{warned:?}");
    assert!(warned.contains(&"petrifying gaze"), "{warned:?}");
    assert_eq!(
        warned.len(),
        2,
        "an immunity was reported as a danger: {warned:?}"
    );
}

/// `Gorgon`, `FireBreath` and `Splits` reach no ability table at all — they are
/// three of the six markers hand-wired into `ai`, `combat` and `took_damage`.
/// Putting the phrase on the effect row rather than on an ability row is what
/// lets Look warn about them anyway.
#[test]
fn look_warns_about_abilities_that_have_no_table_row() {
    for id in ["gorgon", "fire_breath", "splits"] {
        let effect = Effect::by_id(id).expect("a registered effect");
        assert!(
            effect.beware.is_some(),
            "{id:?} has no `beware` line, so Look is silent about it"
        );
    }
}

// ---------------------------------------------------------------------------
// The moments
// ---------------------------------------------------------------------------

/// `Moment::OnTargeted` replaced four hand-written `medusa_gaze` call sites,
/// and the fifth — the look reticle — was one line once the moment existed.
/// This is the moment itself: turning your attention on a gorgon petrifies
/// you, whatever you were pointing at it.
#[test]
fn turning_your_attention_on_a_gorgon_petrifies_you() {
    let mut w = arena(1);
    let looker = hero(&mut w, at(10, 10), 20, 8);
    let seen = creature(&mut w, at(11, 10), 10, 4);
    lend(&mut w, seen, Grant::of::<Gorgon>(), Lifetime::Permanent);

    assert!(
        fire_on_targeted(&mut w, looker, seen),
        "the gaze reported that nothing happened"
    );
    assert!(
        w.get::<Petrified>(looker).is_some(),
        "meeting a gorgon's eyes left the looker standing"
    );
}

/// The look reticle asks this before it fires anything, because for a *look*
/// the answer is the whole event: it decides whether pointing the cursor costs
/// the player their turn.
#[test]
fn a_creature_that_answers_being_looked_at_says_so_in_advance() {
    let mut w = arena(1);
    let harmless = creature(&mut w, at(10, 10), 10, 4);
    let gorgon = creature(&mut w, at(11, 10), 10, 4);
    lend(&mut w, gorgon, Grant::of::<Gorgon>(), Lifetime::Permanent);

    assert!(!answers_being_looked_at(&w, harmless));
    assert!(answers_being_looked_at(&w, gorgon));
}

/// A monster looking at a gorgon is unmoved — only a person's eyes turn to
/// stone. The row cannot express that, so the mechanic checks it, and this is
/// the test that keeps the check.
#[test]
fn only_a_persons_eyes_meet_a_gorgons() {
    let mut w = arena(1);
    let other = creature(&mut w, at(12, 10), 10, 4);
    let seen = creature(&mut w, at(11, 10), 10, 4);
    lend(&mut w, seen, Grant::of::<Gorgon>(), Lifetime::Permanent);

    assert!(!fire_on_targeted(&mut w, other, seen));
    assert!(w.get::<Petrified>(other).is_none());
}

/// `Moment::OnDamaged`. The slime's split used to be a hardcoded line in
/// `helpers::took_damage`, beside two things that are not abilities at all.
#[test]
fn being_hurt_and_living_is_a_moment_of_its_own() {
    let species = any_species();
    let mut w = arena(1);
    let splitter = splitter_named(&mut w, species, at(11, 10), 10_000);

    let before = count_named(&mut w, species);
    fire_on_damaged(&mut w, splitter);

    assert!(
        count_named(&mut w, species) > before,
        "the on-damaged moment did not reach the splitter"
    );
}

/// The `player_only` field replaced the same `is_player` guard written out in
/// four different mechanic bodies. A gate written four times is a gate that
/// can be forgotten the fifth.
#[test]
fn a_player_only_row_does_nothing_for_a_monster() {
    let mut w = arena(1);
    let monster = creature(&mut w, at(10, 10), 20, 10);
    lend(
        &mut w,
        monster,
        Grant::of::<HeavySwing>(),
        Lifetime::Permanent,
    );
    let victim = dummy(&mut w, 10, 4);

    fire_on_hit(&mut w, monster, victim, CLEAN);

    assert!(
        w.get::<Asleep>(victim).is_none(),
        "a monster used a trick the table marks as the player's alone"
    );
}

/// And the gate is on the row, so every player-only row is gated whether or
/// not its body remembers to check.
#[test]
fn every_player_only_row_is_gated_by_the_table() {
    let mut w = arena(1);
    let monster = creature(&mut w, at(10, 10), 20, 10);

    for ability in ABILITIES {
        if !ability.player_only {
            continue;
        }
        lend(&mut w, monster, ability.effect, Lifetime::Permanent);
        assert!(
            !ability.armed(&w, monster),
            "a player-only row armed itself on a monster"
        );
    }
}

/// A moment fires its own rows and nobody else's.
///
/// Easy to lose: with one table and a `when` field, a driver that stopped
/// matching on the moment would still pass every test that only asks whether
/// the *right* thing happened. This asks whether the wrong things stayed put.
#[test]
fn one_moment_never_fires_another_moments_rows() {
    let mut w = arena(1);
    let bearer = hero(&mut w, at(10, 10), 20, 8);
    // An on-hit row and an each-turn row, on a creature about to be hurt.
    lend(&mut w, bearer, Grant::of::<Batty>(), Lifetime::Permanent);
    lend(
        &mut w,
        bearer,
        Grant::of::<Teleportitis>(),
        Lifetime::Permanent,
    );
    let stood = pos_of(&w, bearer).expect("the bearer stands somewhere");

    fire_on_damaged(&mut w, bearer);

    assert_eq!(
        pos_of(&w, bearer),
        Some(stood),
        "being hurt fired a row belonging to another moment — the bearer moved, \
         which only Batty's hop or teleportitis could have done"
    );
}

/// The same, the other way round: turns passing must not fire an on-hit row.
///
/// The row used here is the chaos blade's recoil, which is deliberate — it
/// ignores its target and bites its own wielder, so nothing but the moment
/// filter stands between "a turn passed" and the player bleeding. A row that
/// needs a target would pass this test for the wrong reason, because an
/// each-turn firing has none to give it.
#[test]
fn turns_passing_never_fire_an_on_hit_row() {
    let mut w = arena(1);
    let bearer = hero(&mut w, at(10, 10), 20, 8);
    lend(
        &mut w,
        bearer,
        Grant::of::<SelfDamageOnHit>(),
        Lifetime::Permanent,
    );
    let before = hp_of(&w, bearer);

    for _ in 0..64 {
        ability_system(&mut w);
    }

    assert_eq!(
        hp_of(&w, bearer),
        before,
        "64 turns of standing still cost the wielder HP that only a landed \
         blow should cost"
    );
}

// ---------------------------------------------------------------------------
// The damage funnel
// ---------------------------------------------------------------------------

/// The latent bug this funnel closes. Nothing in the game was elemental *and*
/// outside the one path that checked immunity, so nothing actually burned a
/// dragon — but the first elemental trap would have, silently, because the
/// check lived at the call site and twelve of thirteen call sites had none.
///
/// Now it is one branch, and this is a hit of a kind the game does not have
/// yet: elemental damage from a source that is not a wand.
#[test]
fn an_immune_creature_shrugs_off_an_element_whatever_the_source() {
    let mut w = arena(1);
    let dragon = creature(&mut w, at(10, 10), 100, 4);
    lend(
        &mut w,
        dragon,
        Grant::of::<FireImmune>(),
        Lifetime::Permanent,
    );
    let before = hp_of(&w, dragon);

    let dealt = apply_hit(&mut w, dragon, Hit::elemental(20, Element::Fire), None);

    assert_eq!(dealt, 0, "fire hurt a fire-immune creature");
    assert_eq!(hp_of(&w, dragon), before, "and took HP off doing it");
}

/// The immunity is to the element, not to being hurt.
#[test]
fn immunity_to_one_element_is_not_immunity_to_the_rest() {
    let mut w = arena(1);
    let dragon = creature(&mut w, at(10, 10), 100, 4);
    lend(
        &mut w,
        dragon,
        Grant::of::<FireImmune>(),
        Lifetime::Permanent,
    );

    assert!(
        apply_hit(&mut w, dragon, Hit::elemental(10, Element::Cold), None) > 0,
        "fire immunity turned aside the cold"
    );
    assert!(
        apply_hit(&mut w, dragon, Hit::physical(10), None) > 0,
        "fire immunity turned aside a dart"
    );
}

/// A ward stops magic. It does not stop a falling rock, and the funnel is
/// where that distinction is kept.
#[test]
fn a_ward_turns_aside_magic_and_lets_steel_through() {
    let mut w = arena(1);
    let warded = creature(&mut w, at(10, 10), 100, 4);
    lend(&mut w, warded, Grant::of::<MagicWard>(), Lifetime::Floor);

    assert_eq!(
        apply_hit(&mut w, warded, Hit::magic(10), None),
        0,
        "a ward let magic through"
    );
    assert!(
        apply_hit(&mut w, warded, Hit::physical(10), None) > 0,
        "a ward stopped a thrown dagger, which is not magic"
    );
}

/// The ordering that made the flavour line the funnel's business: a hit that
/// is turned aside must not announce damage it never did.
#[test]
fn a_hit_that_never_lands_never_announces_itself() {
    let mut w = arena(1);
    let warded = hero(&mut w, at(10, 10), 100, 4);
    lend(&mut w, warded, Grant::of::<MagicWard>(), Lifetime::Floor);

    apply_hit(
        &mut w,
        warded,
        Hit::magic(10),
        Some("The bolt slams home for 10 damage!"),
    );

    assert!(
        !w.resource::<GameLog>()
            .history
            .iter()
            .any(|l| l.contains("slams home")),
        "a warded hit announced damage it never did"
    );
}

/// And the other half of that ordering: a hit that lands says so *before*
/// anything that answers it. "You are badly wounded!" reading before the blow
/// that wounded you is the bug this shape prevents.
#[test]
fn a_landed_hit_announces_itself_before_its_consequences() {
    let mut w = arena(1);
    let victim = hero(&mut w, at(10, 10), 100, 4);

    apply_hit(
        &mut w,
        victim,
        Hit::physical(90),
        Some("The bolt slams home!"),
    );

    let log = w.resource::<GameLog>();
    let announced = log.history.iter().position(|l| l.contains("slams home"));
    let wounded = log.history.iter().position(|l| l.contains("badly wounded"));
    assert!(announced.is_some(), "the landed hit never announced itself");
    assert!(
        wounded.is_some_and(|w| announced.is_some_and(|a| a < w)),
        "the wound was reported before the blow that caused it: {:?}",
        log.history
    );
}

/// `SustainsStrength` was the same guard in three places. One rule now, and
/// the prose stays with whoever is inflicting it.
#[test]
fn sustained_strength_turns_aside_every_drain() {
    let mut w = arena(1);
    let victim = creature(&mut w, at(10, 10), 20, 8);
    lend(
        &mut w,
        victim,
        Grant::of::<SustainsStrength>(),
        Lifetime::Permanent,
    );
    let before = power_of(&w, victim);

    assert!(matches!(
        drain_power(&mut w, victim, 4, Some(1)),
        Drain::Resisted
    ));
    assert_eq!(power_of(&w, victim), before, "a resisted drain still bit");
}

/// The floor is the caller's to set, because the snake's bite has none: a long
/// enough fight drives a victim's power negative, and a dart never can.
#[test]
fn a_drain_respects_the_floor_its_source_asked_for() {
    let mut w = arena(1);
    let floored = creature(&mut w, at(10, 10), 20, 8);
    drain_power(&mut w, floored, 1_000, Some(1));
    assert_eq!(power_of(&w, floored), 1, "a floored drain went through it");

    let unfloored = creature(&mut w, at(11, 10), 20, 8);
    drain_power(&mut w, unfloored, 1_000, None);
    assert!(
        power_of(&w, unfloored) < 0,
        "a drain with no floor stopped at one anyway"
    );
}

/// `Moment::InsteadOfAttacking` — the one moment that is a decision rather
/// than a reaction. Nothing has happened yet; the row is bidding for the turn.
///
/// This is what closed §2.3: the dragon's fireball was a probe, a dice roll
/// and a two-armed `match` sitting in the pathing code, so the ability spanned
/// five files with nothing naming it. One row names it now, and `ai` never
/// learns that dragons exist.
#[test]
fn a_breather_can_take_the_turn_instead_of_swinging() {
    // Swept because the row carries odds, not because the wiring is uncertain.
    let breathed = (0..64u64).any(|seed| {
        let mut w = arena(seed);
        let mob = creature(&mut w, at(10, 10), 20, 8);
        lend(&mut w, mob, Grant::of::<FireBreath>(), Lifetime::Permanent);
        let victim = creature(&mut w, at(11, 10), 10_000, 8);
        fire_instead_of_attacking(&mut w, mob, victim)
    });
    assert!(
        breathed,
        "no breather in 64 seeds ever took the turn — the row is not wired"
    );
}

/// The eel's lightning is the same bid with the Thunderbolt in it — and the
/// bolt has to land on the player, which a spell written for the player to
/// cast never had to manage.
#[test]
fn an_eel_can_answer_with_lightning_that_finds_the_player() {
    let struck = (0..64u64).any(|seed| {
        let mut w = arena(seed);
        let eel = creature(&mut w, at(10, 10), 20, 8);
        lend(
            &mut w,
            eel,
            Grant::of::<LightningBreath>(),
            Lifetime::Permanent,
        );
        let you = hero(&mut w, at(11, 10), 10_000, 8);
        let before = hp_of(&w, you);
        fire_instead_of_attacking(&mut w, eel, you) && hp_of(&w, you) < before
    });
    assert!(
        struck,
        "no eel in 64 seeds ever struck the player with lightning"
    );
}

/// And a creature with nothing to say swings, every time. This is the branch
/// `ai` relies on: when nothing takes the turn, the ordinary blow queues.
#[test]
fn a_plain_creature_never_takes_the_turn_instead() {
    for seed in 0..64u64 {
        let mut w = arena(seed);
        let mob = creature(&mut w, at(10, 10), 20, 8);
        let victim = creature(&mut w, at(11, 10), 10_000, 8);
        assert!(
            !fire_instead_of_attacking(&mut w, mob, victim),
            "seed {seed}: a creature with no such ability stole its own turn"
        );
    }
}

/// The bid is the attacker's, not the victim's — the marker is read off
/// whoever is swinging.
#[test]
fn the_bid_is_read_off_the_attacker() {
    for seed in 0..64u64 {
        let mut w = arena(seed);
        let mob = creature(&mut w, at(10, 10), 20, 8);
        let victim = creature(&mut w, at(11, 10), 10_000, 8);
        lend(
            &mut w,
            victim,
            Grant::of::<FireBreath>(),
            Lifetime::Permanent,
        );
        assert!(
            !fire_instead_of_attacking(&mut w, mob, victim),
            "seed {seed}: the victim's own fire breath decided the attacker's turn"
        );
    }
}
