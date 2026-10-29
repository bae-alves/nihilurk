//! The safety net under monster and gear abilities.
//!
//! Every ability in the game is a marker component on a creature plus a
//! mechanic that fires at some *moment* — a blow that landed, a turn that
//! passed, a step onto a tile. Two of those moments are tables
//! (rows in [`ABILITIES`]); the rest are hand-written
//! into `ai`, `combat`, `traps` and `helpers::took_damage`. Until this file
//! existed, none of them were covered: 315 tests and not one named `Gorgon`,
//! a spell or `Splits`.
//!
//! This is a *characterisation* net, written to hold the behaviour still
//! while the moments are generalised. Two rules shape it.
//!
//! **A test never asserts a constant**
//! (`docs/explanation/code-calisthenics.md`). Not "venom drains 1 power" —
//! that is `constants/` testing itself, and it goes red the morning
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
//! answered by `tests/it/content.rs` walking `content_names()`.
//!
//! One exception: `maybe_split` (`monsters.rs`) buds a copy from the bestiary
//! row its victim shows as, so a fixture with a name this file invented has no
//! row to bud from. The tests that need a split take a name off the table at
//! runtime instead of inventing one.

use bevy_ecs::prelude::*;
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
    let victim = dummy(&mut w, 20, 8);

    fire_on_hit(&mut w, attacker, victim, CLEAN);

    let said = &w.resource::<GameLog>().history;
    assert!(
        said.iter().any(|l| l.contains("dummy")),
        "a drain the player landed on a monster said nothing about it: {said:?}"
    );
}

#[test]
fn a_binding_bite_clamps_the_victim() {
    let mut w = arena(1);
    let attacker = dummy(&mut w, 10, 4);
    w.entity_mut(attacker).insert(Binds);
    let victim = dummy(&mut w, 10, 8);

    fire_on_hit(&mut w, attacker, victim, CLEAN);

    assert!(
        w.get::<Clamped>(victim).is_some(),
        "the bite should have clamped the victim, not pinned it like a bear trap"
    );
    assert!(
        turns_left(&w, victim, Grant::of::<Clamped>()).is_some_and(|n| n > 0),
        "clamped for no turns at all"
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

/// A monster's paralysis earns a log line of its own, but only when the player
/// could actually watch it happen, the same rule `conditions::report_cure`
/// already holds a mending monster to.
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
        1,
        "a paralysis landing in plain sight should say so: {:?}",
        w.resource::<GameLog>().history
    );
}

/// Left on whoever set the fixture trap off.
#[derive(Component)]
struct Sprung;

static HOP_TRAP: TrapDef = TrapDef {
    name: "trap of the test",
    glyph: '^',
    color: Color::White,
    weight: 0,
    min_depth: 1,
    snare_turns: 0,
    always_spent: true,
    apply: |world, spring| {
        world.entity_mut(spring.victim).insert(Sprung);
    },
    flourish: |_, _, _| {},
};

static KILLING_TRAP: TrapDef = TrapDef {
    apply: |world, spring| {
        world.get_mut::<Fighter>(spring.victim).unwrap().hp = 0;
    },
    ..HOP_TRAP
};

/// The reaper has already swept by the time the hop lands, so a trap that
/// kills the hopper finishes it there, not a turn later.
#[test]
fn an_erratic_striker_its_trap_kills_dies_the_same_turn() {
    let mut w = arena(3);
    w.init_resource::<Ending>();
    let victim = hero(&mut w, at(10, 10), 20, 8);
    let hopper = creature(&mut w, at(11, 10), 5, 4);
    w.entity_mut(hopper).insert(Batty);
    for x in 10..=12 {
        for y in 9..=11 {
            w.spawn(TrapBundle::from_def(
                &KILLING_TRAP,
                TrapReveal::Sight,
                at(x, y),
            ));
        }
    }

    fire_on_hit(&mut w, hopper, victim, CLEAN);

    assert!(
        w.get::<Fighter>(hopper).is_none(),
        "the trap killed it and it is still standing"
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

/// The two gating booleans on a row, checked as a pair: acid does not care
/// that the armour turned the blow, and there is no point charming a corpse.
#[test]
fn the_row_gates_decide_which_blows_count() {
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

// ---------------------------------------------------------------------------
// The markers that reach no table at all
// ---------------------------------------------------------------------------

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
/// `maybe_split` buds the copy from the bestiary row the victim shows as, so a
/// splitter must be named something the table knows. The name is taken off the
/// table at runtime rather than written in here, so this test still names
/// nothing.
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

/// A splitter boxed in by walls on every side but one, with the player
/// standing in that one gap — the shape of a passage, where a corridor
/// leaves a wounded slime nowhere to bud a copy except the tile the player
/// is already standing on.
///
/// The player is not a [`Mob`], so a copy must not land there: it has
/// nowhere to go and should stay a single slime, not silently duplicate
/// itself onto the hero.
#[test]
fn a_boxed_in_splitter_does_not_bud_onto_the_player() {
    let species = any_species();
    let mut w = arena(42);
    let center = at(20, 20);
    for dy in -1i32..=1 {
        for dx in -1i32..=1 {
            if dx == 0 && dy == 0 || (dx == 1 && dy == 0) {
                continue;
            }
            let (x, y) = ((center.x as i32 + dx) as u16, (center.y as i32 + dy) as u16);
            w.resource_mut::<Map>().tiles[tile_index(x, y)] = TileType::Wall;
        }
    }
    let hero_pos = at(center.x + 1, center.y);
    let attacker = hero(&mut w, hero_pos, 20, 10);
    let splitter = splitter_named(&mut w, species, center, 10_000);

    let before = count_named(&mut w, species);
    resolve_attack(&mut w, attacker, splitter);
    let after = count_named(&mut w, species);

    assert_eq!(
        before, after,
        "a boxed-in splitter budded a copy with nowhere free to put it"
    );
    let occupied_hero_tile = w
        .query::<(&Position, &Mob)>()
        .iter(&w)
        .any(|(p, _)| *p == hero_pos);
    assert!(
        !occupied_hero_tile,
        "the splitter's copy landed on the player's own tile"
    );
}

/// A splitter no bestiary row shows as, the way every slime looks to a lookup
/// by row id in a translated build: it is wounded like any other, and simply
/// has no row to bud from.
#[test]
fn a_splitter_no_row_shows_as_survives_the_wound_whole() {
    let mut w = arena(41);
    let attacker = hero(&mut w, at(10, 10), 20, 10);
    let splitter = splitter_named(&mut w, "splitter of the test", at(11, 10), 10_000);

    resolve_attack(&mut w, attacker, splitter);

    assert_eq!(count_named(&mut w, "splitter of the test"), 1);
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

// ---------------------------------------------------------------------------
// The tables as data
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// The effects ledger
// ---------------------------------------------------------------------------

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
            count: 0,
        },
        Held {
            id: "a_row_this_build_does_not_have",
            lifetime: Lifetime::Permanent,
            count: 0,
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
// The moments
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// The damage funnel
// ---------------------------------------------------------------------------

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
