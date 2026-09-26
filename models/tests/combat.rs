//! The damage rule itself, and `resolve_attack`'s excellent-hit floor.

use bevy_ecs::prelude::*;
use models::*;

fn combat_world(seed: u64) -> World {
    let mut w = World::new();
    w.insert_resource(GameRng(ChaCha12Rng::seed_from_u64(seed)));
    w.init_resource::<GameLog>();
    w
}

fn spawn_attacker(w: &mut World, power: i32) -> Entity {
    w.spawn((
        Player,
        Name { what: "you".into() },
        Fighter {
            hp: 20,
            max_hp: 20,
            armor: 0,
            power,
            max_power: power,
            armor_bonus: 0,
            power_bonus: 0,
        },
    ))
    .id()
}

fn spawn_target(w: &mut World, hp: i32, armor: i32) -> Entity {
    w.spawn((
        Name {
            what: "dummy".into(),
        },
        Fighter {
            hp,
            max_hp: hp,
            armor,
            power: 1,
            max_power: 1,
            armor_bonus: 0,
            power_bonus: 0,
        },
    ))
    .id()
}

/// A weak attacker (die size 1) against heavy armour, on a foe left at exactly
/// 1 HP: the combination that used to clamp an "excellent hit" down to 0
/// damage — the "a glancing blow can't take the last point" rule firing on a
/// swing that was never glancing in the first place. Sweeps enough seeds that
/// an excellent hit (15% chance) is certain to land at least once.
#[test]
fn an_excellent_hit_never_deals_or_reports_zero_damage() {
    let mut saw_excellent = false;
    for seed in 0..2000u64 {
        let mut w = combat_world(seed);
        let attacker = spawn_attacker(&mut w, 1);
        let target = spawn_target(&mut w, 1, 100);
        resolve_attack(&mut w, attacker, target);

        let log = w.resource::<GameLog>();
        let Some(line) = log.unread.iter().find(|l| l.contains("excellent hit")) else {
            continue;
        };
        saw_excellent = true;
        assert!(
            !line.contains("for 0 damage"),
            "seed {seed}: excellent hit reported 0 damage: {line}"
        );
    }
    assert!(
        saw_excellent,
        "no excellent hit landed across 2000 seeds; the sweep isn't exercising the path"
    );
}

/// The other half of the same fix: a plain chip/glancing blow can never take a
/// foe's last point of HP (see `scrolls.rs`'s
/// `a_glancing_blow_chips_a_foe_down_to_one_but_never_finishes_it`), but an
/// excellent hit is a real hit, not a whiff that got rounded up — so unlike a
/// glancing blow, it's allowed to land the killing blow even against a foe at
/// 1 HP behind heavy armour.
#[test]
fn an_excellent_hit_can_finish_a_foe_a_glancing_blow_could_not() {
    for seed in 0..2000u64 {
        let mut w = combat_world(seed);
        let attacker = spawn_attacker(&mut w, 1);
        let target = spawn_target(&mut w, 1, 100);
        resolve_attack(&mut w, attacker, target);

        let log = w.resource::<GameLog>();
        let excellent = log.unread.iter().any(|l| l.contains("excellent hit"));
        if excellent && !w.entities().contains(target) {
            return; // found one: an excellent hit finished a 1-HP target.
        }
    }
    panic!("no excellent hit finished a 1-HP target across 2000 seeds");
}

// ---------------------------------------------------------------------------
// The damage rule, with the dice taken out of it
// ---------------------------------------------------------------------------
// `roll_die` resolves `1d1` to exactly 1, so a fighter whose die is 1 rolls a
// known number and the whole exchange becomes arithmetic. That is deliberate:
// a test of the damage rule should not be able to fail because something
// upstream started drawing from the RNG in a different order.

/// A monster, so no excellent-hit roll and no chip-damage floor apply.
fn spawn_monster_attacker(w: &mut World, power: i32, power_bonus: i32) -> Entity {
    w.spawn((
        Name { what: "orc".into() },
        Fighter {
            hp: 10,
            max_hp: 10,
            armor: 0,
            power,
            max_power: power,
            armor_bonus: 0,
            power_bonus,
        },
    ))
    .id()
}

fn spawn_armored_target(w: &mut World, hp: i32, armor: i32, armor_bonus: i32) -> Entity {
    w.spawn((
        Name {
            what: "dummy".into(),
        },
        Fighter {
            hp,
            max_hp: hp,
            armor,
            power: 1,
            max_power: 1,
            armor_bonus,
            power_bonus: 0,
        },
    ))
    .id()
}

#[test]
fn a_blow_is_exactly_the_attack_total_minus_the_armour_total() {
    let mut w = combat_world(1);
    // Attacking: 1d1 + 14 = 15. Defending: 1d1 + 9 = 10. Five gets through.
    let foe = spawn_monster_attacker(&mut w, 1, 14);
    let target = spawn_armored_target(&mut w, 40, 1, 9);

    resolve_attack(&mut w, foe, target);
    assert_eq!(w.get::<Fighter>(target).unwrap().hp, 35, "40 - 5");

    // Again, to show it is the rule rather than one lucky roll.
    resolve_attack(&mut w, foe, target);
    assert_eq!(w.get::<Fighter>(target).unwrap().hp, 30, "40 - 5 - 5");
}

/// The chip-damage floor is the hero's alone. Armour that outrolls a monster's
/// swing leaves the defender completely untouched.
#[test]
fn armour_that_outrolls_a_monsters_blow_lets_nothing_through() {
    let mut w = combat_world(1);
    let foe = spawn_monster_attacker(&mut w, 1, 0); // 1
    let target = spawn_armored_target(&mut w, 40, 1, 20); // 21
    resolve_attack(&mut w, foe, target);
    assert_eq!(
        w.get::<Fighter>(target).unwrap().hp,
        40,
        "a monster gets no chip through armour"
    );
}

// ---------------------------------------------------------------------------
// Stone
// ---------------------------------------------------------------------------
//
// A petrified creature is not a helpless one: nothing gets more than a chip
// through stone, and nothing takes its last point. The rule has to hold on
// both damage paths — `resolve_attack`, which applies its own HP, and
// `helpers::apply_hit`, which every other source of harm goes through — so it
// is tested on both.

/// The same arithmetic as `a_blow_is_exactly_the_attack_total_minus_the_armour_total`,
/// with the target turned to stone: the five that got through becomes a chip.
#[test]
fn a_blow_that_would_wound_a_petrified_creature_only_chips_it() {
    let mut w = combat_world(1);
    let foe = spawn_monster_attacker(&mut w, 1, 14); // 15
    let target = spawn_armored_target(&mut w, 40, 1, 9); // 10, so five gets through
    hold(&mut w, target, Grant::of::<Petrified>(), 5);

    resolve_attack(&mut w, foe, target);

    let hp = w.get::<Fighter>(target).unwrap().hp;
    assert!(hp > 35, "the blow went through stone whole: {hp}");
    assert!(hp < 40, "stone turned the blow aside entirely: {hp}");
    assert!(
        w.resource::<GameLog>()
            .history
            .iter()
            .any(|l| l.contains("is chipped for")),
        "the chip was never reported: {:?}",
        w.resource::<GameLog>().history
    );
}

/// The second half of the rule, and the reason the medusa is a hard stop
/// rather than a death: a creature on its last point stays on it.
#[test]
fn nothing_takes_a_petrified_creatures_last_point() {
    let mut w = combat_world(1);
    let foe = spawn_monster_attacker(&mut w, 1, 14); // 15
    let target = spawn_armored_target(&mut w, 1, 1, 9); // 10, five would be lethal
    hold(&mut w, target, Grant::of::<Petrified>(), 5);

    resolve_attack(&mut w, foe, target);

    assert_eq!(
        w.get::<Fighter>(target).unwrap().hp,
        1,
        "a blow finished off something made of stone"
    );
    assert!(
        w.resource::<GameLog>()
            .history
            .iter()
            .any(|l| l.contains("chipped for no damage")),
        "a blow stone stopped dead was reported as doing something: {:?}",
        w.resource::<GameLog>().history
    );
}

/// Stone is not armour against steel alone. Every other source of harm — a
/// dragon's breath, a wand's ray, a dart — goes through `apply_hit`, and the
/// cap lives there too.
#[test]
fn stone_caps_every_other_source_of_harm_as_well() {
    let mut w = combat_world(1);
    let target = spawn_armored_target(&mut w, 40, 1, 0);
    hold(&mut w, target, Grant::of::<Petrified>(), 5);

    let flesh = spawn_armored_target(&mut w, 40, 1, 0);

    let dealt = apply_hit(&mut w, target, Hit::magic(30), None);
    let whole = apply_hit(&mut w, flesh, Hit::magic(30), None);

    assert!(
        dealt < whole,
        "a ray burned through stone exactly as it burned through flesh: {dealt} of {whole}"
    );
    assert!(
        w.get::<Fighter>(target).unwrap().hp > w.get::<Fighter>(flesh).unwrap().hp,
        "stone was no better than flesh to stand in a blast in"
    );
    assert!(
        w.resource::<GameLog>()
            .history
            .iter()
            .any(|l| l.contains("is chipped for")),
        "the chip was never reported: {:?}",
        w.resource::<GameLog>().history
    );
}

/// A miss is still a miss. Stone reports a chip only when something actually
/// struck it — otherwise every swing that never connected would read as one.
#[test]
fn a_blow_that_misses_a_petrified_creature_is_still_a_miss() {
    let mut w = combat_world(1);
    let foe = spawn_monster_attacker(&mut w, 1, 0); // 1
    let target = spawn_armored_target(&mut w, 40, 1, 20); // 21
    hold(&mut w, target, Grant::of::<Petrified>(), 5);

    resolve_attack(&mut w, foe, target);

    let said = w.resource::<GameLog>().history.clone();
    assert_eq!(w.get::<Fighter>(target).unwrap().hp, 40);
    assert!(
        said.iter().any(|l| l.contains("misses")),
        "a swing that never landed was not reported as a miss: {said:?}"
    );
    assert!(
        !said.iter().any(|l| l.contains("chipped")),
        "a swing that never landed was reported as a chip: {said:?}"
    );
}

/// The one thing stone does not stop. A war hammer is mass rather than edge:
/// its blow lands whole on a petrified creature, last point included, and is
/// reported as the hit it was rather than as a chip.
#[test]
fn a_war_hammer_goes_through_stone_whole() {
    let mut w = combat_world(1);
    let foe = spawn_monster_attacker(&mut w, 1, 14); // 15
    let target = spawn_armored_target(&mut w, 40, 1, 9); // 10, so five gets through
    hold(&mut w, target, Grant::of::<Petrified>(), 5);
    lend(
        &mut w,
        foe,
        Grant::of::<ShattersStone>(),
        Lifetime::Permanent,
    );

    resolve_attack(&mut w, foe, target);

    assert_eq!(
        w.get::<Fighter>(target).unwrap().hp,
        35,
        "stone turned aside a war hammer"
    );
    assert!(
        !w.resource::<GameLog>()
            .history
            .iter()
            .any(|l| l.contains("chipped")),
        "a hammer blow that landed whole was reported as a chip: {:?}",
        w.resource::<GameLog>().history
    );
}

/// And it can finish one, which is the whole point of carrying it: stone is a
/// hard stop for everything else in the dungeon.
#[test]
fn a_war_hammer_can_take_a_petrified_creatures_last_point() {
    let mut w = combat_world(1);
    let foe = spawn_monster_attacker(&mut w, 1, 14); // 15
    let target = spawn_armored_target(&mut w, 1, 1, 9); // 10, five is lethal
    hold(&mut w, target, Grant::of::<Petrified>(), 5);
    lend(
        &mut w,
        foe,
        Grant::of::<ShattersStone>(),
        Lifetime::Permanent,
    );

    resolve_attack(&mut w, foe, target);

    assert!(
        w.get::<Fighter>(target).is_none_or(|f| f.hp <= 0),
        "a war hammer left a statue standing"
    );
}

/// The garrote's newest throat: a monster caught mid-flight. A plain 1-point
/// nick would leave 49 of this target's 50 HP standing — so if the garrote
/// fires, only the vorpal shear (which zeroes `hp` outright) explains a kill.
#[test]
fn a_garrote_finds_a_fleeing_monster_as_helpless_as_a_sleeping_one() {
    let mut w = combat_world(1);
    let hero = spawn_attacker(&mut w, 1); // 1d1: a deterministic 1-point nick
    lend(
        &mut w,
        hero,
        Grant::of::<VorpalOnCondition>(),
        Lifetime::Permanent,
    );
    let target = spawn_target(&mut w, 50, 0);
    w.entity_mut(target).insert(Mob {
        movement_type: MovementType::Flee,
    });

    resolve_attack(&mut w, hero, target);

    assert!(
        w.get::<Fighter>(target).is_none_or(|f| f.hp <= 0),
        "a fleeing monster survived a garroted hit that should have zeroed it"
    );
}
