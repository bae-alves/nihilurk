//! The ring table, now that every row does something — and the scoreboard the
//! two showiest of them play with.
//!
//! Most rings are checked by asking whether the component the row promised is
//! on the wearer, because that is the whole of what those rings are. The three
//! with verbs behind them (adornment, regeneration, teleportation) get the
//! mechanic exercised.

#[path = "common/monster.rs"]
mod monster;

use bevy_ecs::prelude::*;
use crossterm::style::Color;
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

/// The engine's "Use" action on a pack item — for a ring, putting it on.
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

/// Spawn a ring of `effect` into `user`'s pack and put it on. Uncursed, so the
/// test is never wearing something it can't take off again.
fn put_on(w: &mut World, user: Entity, effect: RingEffect) -> Entity {
    let ring = spawn_ring(w, effect, Position { x: 0, y: 0 });
    w.entity_mut(ring).remove::<Position>();
    w.entity_mut(ring).remove::<Curse>();
    w.get_mut::<Backpack>(user).unwrap().items.push(ring);
    use_item(w, user, ring);
    ring
}

fn score(w: &mut World) -> i64 {
    let p = player(w);
    w.get::<Score>(p).unwrap().value
}

fn logged(w: &World, needle: &str) -> bool {
    let log = w.resource::<GameLog>();
    log.history
        .iter()
        .map(String::as_str)
        .chain(log.unread.iter().map(|e| e.text.as_str()))
        .any(|l| l.contains(needle))
}

// ---------------------------------------------------------------------------
// The rings that are only a row
// ---------------------------------------------------------------------------

#[test]
fn increase_damage_is_two_points_on_every_swing() {
    let mut w = test_world(1);
    let p = player(&mut w);
    let before = equipped_total::<PowerBonus>(&w, p);

    put_on(&mut w, p, RingEffect::IncreaseDamage);

    assert_eq!(
        equipped_total::<PowerBonus>(&w, p),
        before + 2,
        "the ring folds into the same total a +2 weapon would"
    );
}

#[test]
fn slow_digestion_slows_everything_else_too() {
    let mut w = test_world(1);
    let p = player(&mut w);
    assert_eq!(tempo(&w, p), SpeedKind::Normal);

    let ring = put_on(&mut w, p, RingEffect::SlowDigestion);
    assert_eq!(tempo(&w, p), SpeedKind::Slow, "worn, you are slowed");

    use_item(&mut w, p, ring);
    assert_eq!(
        tempo(&w, p),
        SpeedKind::Normal,
        "and taking it off gives the notch straight back"
    );
}

#[test]
fn slow_digestion_never_touches_the_speed_a_potion_set() {
    let mut w = test_world(1);
    let p = player(&mut w);
    assert!(hasten(&mut w, p));

    let ring = put_on(&mut w, p, RingEffect::SlowDigestion);
    assert_eq!(tempo(&w, p), SpeedKind::Normal, "hasted, then weighed down");

    use_item(&mut w, p, ring);
    assert_eq!(
        tempo(&w, p),
        SpeedKind::Fast,
        "the haste was never spent, only masked"
    );
}

#[test]
fn stealth_and_teleportitis_and_regeneration_are_worn_properties() {
    let mut w = test_world(1);
    let p = player(&mut w);

    let stealth = put_on(&mut w, p, RingEffect::Stealth);
    assert!(w.get::<Stealthy>(p).is_some());
    use_item(&mut w, p, stealth);
    assert!(w.get::<Stealthy>(p).is_none(), "and it comes off again");

    put_on(&mut w, p, RingEffect::Teleportation);
    assert!(w.get::<Teleportitis>(p).is_some());

    put_on(&mut w, p, RingEffect::Regeneration);
    assert!(w.get::<Regenerates>(p).is_some());
}

/// Drops a chaser onto a visible tile at least `away` tiles from the player and
/// gives the floor one turn. Reports whether it came for them.
///
/// The tile is picked so that the *only* thing that can stop the orc is not
/// noticing the player: both tiles are room floor and so is the step between
/// them, which clears `ai`'s wall, diagonal and room-leash rules. And the
/// candidates are sorted before one is taken, because `Viewshed::visible_tiles`
/// comes out of a `HashSet` and its order is not the same twice.
fn a_chaser_closes_in(w: &mut World, away: i32) -> bool {
    let p = player(w);
    let mut s = bevy_ecs::schedule::Schedule::default();
    s.add_systems(visibility_system);
    w.get_mut::<Viewshed>(p).unwrap().dirty = true;
    s.run(w);

    let at = *w.get::<Position>(p).unwrap();
    let map = w.resource::<Map>().clone();
    let room = |x: u16, y: u16| map.tile(x, y) == TileType::Room;
    let mut candidates: Vec<(u16, u16)> = w
        .get::<Viewshed>(p)
        .unwrap()
        .visible_tiles
        .iter()
        .copied()
        .filter(|&(x, y)| {
            let (dx, dy) = (x as i32 - at.x as i32, y as i32 - at.y as i32);
            let toward = (
                (x as i32 - dx.signum()) as u16,
                (y as i32 - dy.signum()) as u16,
            );
            dx.abs().max(dy.abs()) >= away && room(x, y) && room(toward.0, toward.1)
        })
        .collect();
    candidates.sort_unstable();
    let spot = candidates
        .first()
        .copied()
        .expect("a lit room with somewhere to stand in it");

    let mob = monster::monster(
        w,
        "test monster",
        Position {
            x: spot.0,
            y: spot.1,
        },
    );
    ai(w);
    *w.get::<Position>(mob).unwrap()
        != Position {
            x: spot.0,
            y: spot.1,
        }
}

#[test]
fn stealth_keeps_the_room_from_noticing_you() {
    let mut w = test_world(4);
    assert!(
        a_chaser_closes_in(&mut w, 4),
        "an orc that can see you comes for you"
    );

    let mut w = test_world(4);
    let p = player(&mut w);
    put_on(&mut w, p, RingEffect::Stealth);
    assert!(
        !a_chaser_closes_in(&mut w, 4),
        "wearing the ring, the same orc has no idea you are there"
    );
}

// ---------------------------------------------------------------------------
// Maintain armor, and the thing it protects you from
// ---------------------------------------------------------------------------

/// Wears the starting armour into an aquator's touch, with or without the ring.
fn corrode_the_player(seed: u64, with_ring: bool) -> i32 {
    let mut w = test_world(seed);
    let p = player(&mut w);
    if with_ring {
        put_on(&mut w, p, RingEffect::MaintainArmor);
    }
    corrode_armor(&mut w, p);
    equipped_total::<ArmorBonus>(&w, p)
}

#[test]
fn corrosion_eats_a_point_of_the_armours_plus() {
    let mut w = test_world(1);
    let p = player(&mut w);
    let before = equipped_total::<ArmorBonus>(&w, p);

    assert!(corrode_armor(&mut w, p), "there was armour to eat");

    assert_eq!(equipped_total::<ArmorBonus>(&w, p), before - 1);
    assert!(logged(&w, "corrodes"));
}

#[test]
fn maintain_armor_shrugs_the_corrosion_off() {
    let plain = corrode_the_player(1, false);
    let warded = corrode_the_player(1, true);
    assert_eq!(warded, plain + 1, "the ring is worth exactly the point");
}

#[test]
fn sustain_form_lends_its_marker_while_worn() {
    let mut w = test_world(1);
    let p = player(&mut w);
    assert!(w.get::<SustainsForm>(p).is_none());

    let ring = put_on(&mut w, p, RingEffect::SustainForm);
    assert!(w.get::<SustainsForm>(p).is_some());

    use_item(&mut w, p, ring);
    assert!(w.get::<SustainsForm>(p).is_none());
}

// ---------------------------------------------------------------------------
// Regeneration
// ---------------------------------------------------------------------------

#[test]
fn regeneration_lifts_a_condition_before_it_mends_an_arm() {
    let mut w = test_world(1);
    let p = player(&mut w);
    w.get_mut::<Fighter>(p).unwrap().power -= 2;
    assert!(blind(&mut w, p));

    assert!(cure_one_condition(&mut w, p), "the eyes come first");
    assert!(w.get::<Blind>(p).is_none());

    let before = w.get::<Fighter>(p).unwrap().power;
    assert!(!cure_one_condition(&mut w, p), "nothing left to cure");
    assert!(restore_one_power(&mut w, p));
    assert_eq!(w.get::<Fighter>(p).unwrap().power, before + 1);
}

#[test]
fn regeneration_stops_at_full_health_and_says_nothing() {
    let mut w = test_world(1);
    let p = player(&mut w);
    assert!(
        !cure_one_condition(&mut w, p) && !restore_one_power(&mut w, p),
        "an unhurt player gives the ring nothing to do"
    );
}

// ---------------------------------------------------------------------------
// Adornment, and the score it plays with
// ---------------------------------------------------------------------------

#[test]
fn adornment_doubles_the_score_and_burns_itself_out() {
    let mut w = test_world(1);
    let p = player(&mut w);
    award(&mut w, 1200);

    let ring = put_on(&mut w, p, RingEffect::Adornment);

    assert_eq!(score(&mut w), 2400, "worn once, worth everything twice");
    assert!(logged(&w, "And you do it with style!"));
    assert!(w.get_entity(ring).is_none(), "and the ring is gone");
    assert!(
        !w.get::<Backpack>(p).unwrap().items.contains(&ring),
        "gone from the pack, not left as a dangling id"
    );
}

#[test]
fn adornment_throws_sixteen_fireworks_in_three_colours() {
    let mut w = test_world(1);
    w.init_resource::<Particles>();
    let p = player(&mut w);

    put_on(&mut w, p, RingEffect::Adornment);

    let fx = w.resource::<Particles>();
    assert!(fx.pending, "the flourish is the whole point of the ring");
    let fireworks: Vec<Color> = fx
        .live
        .iter()
        .filter(|m| m.frames.len() == 5 && m.frames.iter().all(|f| f.1 == m.frames[0].1))
        .map(|m| m.frames[0].1)
        .collect();
    assert_eq!(fireworks.len(), 16, "two rings of them, not one");
    for c in fireworks {
        assert!(
            matches!(c, Color::Magenta | Color::Cyan | Color::Yellow),
            "a flourish is magenta, cyan and yellow, never {c:?}"
        );
    }
}

/// One whole turn of killing: lays `n` already-dead bodies on the floor, each
/// worth `max_hp`, lets the reaper sweep them up, then runs the schedule's tail
/// — which is where a turn's dead are actually paid for.
fn corpses(w: &mut World, n: usize, max_hp: i32) {
    for i in 0..n {
        w.spawn((
            Name {
                what: "target dummy".into(),
            },
            Mob {
                movement_type: MovementType::Static,
            },
            Position {
                x: 1 + i as u16,
                y: 1,
            },
            Fighter {
                hp: 0,
                max_hp,
                armor: 0,
                power: 1,
                max_power: 1,
                armor_bonus: 0,
                power_bonus: 0,
            },
            Faction::Monster,
        ));
    }
    reaper_system(w);
    score_turn_system(w);
}

/// What one turn's killing paid, on a fresh world: `n` corpses of `max_hp` each.
fn paid_for(n: usize, max_hp: i32) -> i64 {
    let mut w = test_world(1);
    let before = score(&mut w);
    corpses(&mut w, n, max_hp);
    score(&mut w) - before
}

#[test]
fn a_corpse_is_worth_its_hit_points() {
    assert_eq!(paid_for(1, 7), i64::from(7 * KILL_PER_MAX_HP));
    assert_eq!(paid_for(1, 2) * 3, paid_for(1, 6), "worth is linear in HP");
}

#[test]
fn killing_two_in_one_turn_beats_killing_them_one_at_a_time() {
    let pair = paid_for(2, 4);
    let singly = paid_for(1, 4) * 2;
    assert!(
        pair > singly,
        "two at once ({pair}) should beat two in a row ({singly})"
    );
    assert_eq!(
        pair,
        (singly as f32 * (1.0 + COMBO_BONUS_PER_KILL)) as i64,
        "and beat it by exactly one corpse's worth of bonus"
    );
}

#[test]
fn a_bigger_pile_is_worth_more_still() {
    let two = paid_for(2, 4);
    let three = paid_for(3, 4);
    assert!(three > two, "three at once ({three}) beats two ({two})");
}

#[test]
fn the_combo_does_not_carry_into_the_next_turn() {
    let mut w = test_world(1);
    corpses(&mut w, 1, 4);
    let after_first = score(&mut w);

    corpses(&mut w, 1, 4);
    assert_eq!(
        score(&mut w) - after_first,
        paid_for(1, 4),
        "a fresh turn starts the count again"
    );
}

#[test]
fn a_combo_shouts_once_for_the_whole_turn_and_a_single_kill_not_at_all() {
    let mut w = test_world(1);
    corpses(&mut w, 1, 4);
    assert_eq!(
        w.resource::<ScoreFlash>().text,
        format!("+{}", paid_for(1, 4))
    );
    assert!(w.resource::<ScoreFlash>().lit());
    assert!(!logged(&w, "With style.") && !logged(&w, "With pride."));

    corpses(&mut w, 3, 4);
    assert!(
        w.resource::<ScoreFlash>().text.starts_with("COMBO! +"),
        "one shout for the pile, not one per corpse: {}",
        w.resource::<ScoreFlash>().text
    );
    let stripes = pride::stripes(&w).to_vec();
    let flash = w.resource::<ScoreFlash>();
    assert_eq!(
        (0..stripes.len())
            .map(|i| flash.color_at(i))
            .collect::<Vec<_>>(),
        stripes,
        "one stripe of the run's flag per letter of the word"
    );
    let lines = w
        .resource::<GameLog>()
        .history
        .iter()
        .filter(|l| l.contains("With style.") || l.contains("With pride."))
        .count();
    assert_eq!(lines, 1, "and one line in the log for it");
}

#[test]
fn a_doubling_shouts_the_word_instead_of_a_number() {
    let mut w = test_world(1);
    award(&mut w, 100);
    double(&mut w);
    assert_eq!(w.resource::<ScoreFlash>().text, "DOUBLE");
}

#[test]
fn the_flash_goes_dark_after_one_frame() {
    let mut w = test_world(1);
    award(&mut w, 100);
    score_turn_system(&mut w);
    assert!(w.resource::<ScoreFlash>().lit(), "lit for its own turn");
    score_turn_system(&mut w);
    assert!(!w.resource::<ScoreFlash>().lit(), "and dark by the next");
}

#[test]
fn a_staircase_pays_by_difficulty_tier() {
    let mut w = test_world(1);
    let p = player(&mut w);
    let before = score(&mut w);
    let down = w
        .resource::<Map>()
        .tiles
        .iter()
        .position(|&t| t == TileType::Downstairs)
        .unwrap();
    w.get_mut::<Position>(p).unwrap().x = (down % MAP_WIDTH as usize) as u16;
    w.get_mut::<Position>(p).unwrap().y = (down / MAP_WIDTH as usize) as u16;

    assert!(change_level(&mut w, true));

    assert_eq!(
        score(&mut w) - before,
        i64::from(STAIR_PER_TIER),
        "depth 1 is the first tier, and the first tier still pays"
    );
}

#[test]
fn a_run_cannot_be_doubled_into_nothing() {
    let mut w = test_world(1);
    award(&mut w, 100);
    for _ in 0..80 {
        double(&mut w);
    }
    assert_eq!(score(&mut w), i64::MAX, "the score pins at the ceiling");
}

#[test]
fn polymorphitis_eventually_polymorphs_its_bearer() {
    let mut w = test_world(1);
    let p = player(&mut w);
    put_on(&mut w, p, RingEffect::Polymorph);
    assert!(w.get::<Polymorphitis>(p).is_some());

    // 1 in 83 a turn: the claim is "eventually", so sweep turns, not seeds.
    let turned = (0..4000).any(|_| {
        ability_system(&mut w);
        w.get::<Polymorphed>(p).is_some()
    });

    assert!(turned, "4000 turns of polymorphitis never changed anyone");
}

// ---------------------------------------------------------------------------
// What a ring rolls when the dungeon makes one
// ---------------------------------------------------------------------------

/// The flat plus a ring carries, whichever roll its row feeds.
fn plus(w: &World, ring: Entity) -> i32 {
    w.get::<ArmorBonus>(ring).map_or(0, |b| b.0)
        + w.get::<PowerBonus>(ring).map_or(0, |b| b.0)
        + w.get::<ThrowBonus>(ring).map_or(0, |b| b.0)
}

/// `n` floor rolls of the ring `effect`, as `(plus, cursed)` pairs.
fn rolled(effect: RingEffect, n: usize) -> Vec<(i32, bool)> {
    let mut w = World::new();
    let mut rng = ChaCha12Rng::seed_from_u64(7);
    (0..n)
        .map(|_| {
            let ring = RingDef::of(effect).spawn_as_loot(&mut w, &mut rng, Position { x: 0, y: 0 });
            (plus(&w, ring), w.get::<Curse>(ring).is_some())
        })
        .collect()
}

#[test]
fn a_numeric_ring_is_plus_two_plain_plus_three_exceptional_and_cursed_from_minus_three_to_plus_two()
{
    for effect in [
        RingEffect::Protection,
        RingEffect::Strength,
        RingEffect::IncreaseDamage,
        RingEffect::Sharpshooting,
    ] {
        let rolls = rolled(effect, 3000);
        for &(plus, cursed) in &rolls {
            match (plus, cursed) {
                (2, false) | (3, false) => {}
                (-3..=2, true) => {}
                other => panic!("{effect:?} rolled {other:?}"),
            }
        }
        for want in [3, -3, 2] {
            assert!(
                rolls.iter().any(|&(p, _)| p == want),
                "{effect:?} never rolled a {want:+} in 3000 tries"
            );
        }
        assert!(
            rolls.iter().any(|&(p, c)| p < 0 && c),
            "{effect:?} never rolled a negative one"
        );
    }
}

#[test]
fn a_ring_with_no_number_gets_no_number_however_it_rolls() {
    for effect in [
        RingEffect::Perception,
        RingEffect::Stealth,
        RingEffect::Regeneration,
        RingEffect::Adornment,
    ] {
        let rolls = rolled(effect, 1000);
        assert!(
            rolls.iter().all(|&(p, _)| p == 0),
            "{effect:?} picked up a plus"
        );
        assert!(
            rolls.iter().any(|&(_, c)| c) && rolls.iter().any(|&(_, c)| !c),
            "{effect:?} should still roll both cursed and clean"
        );
    }
}

// ---------------------------------------------------------------------------
// The ring of dualzap
// ---------------------------------------------------------------------------

/// A well-charged `effect` wand in `user`'s pack, zapped at `at`.
fn zap_with(w: &mut World, user: Entity, effect: WandEffect, charges: i8, at: Position) -> Entity {
    let wand = spawn_wand(w, effect, Position { x: 0, y: 0 });
    w.entity_mut(wand).remove::<Position>();
    w.get_mut::<Battery>(wand).unwrap().charges = charges;
    w.get_mut::<Backpack>(user).unwrap().items.push(wand);
    w.resource_mut::<UseQueue>().uses.push(WantsToUse {
        user,
        item: wand,
        target: Some(at),
        slot_idx: Some(0),
    });
    item_system(w);
    wand
}

fn bolts_logged(w: &World) -> usize {
    w.resource::<GameLog>()
        .history
        .iter()
        .filter(|l| l.contains(strings::bolt_magic_missile()))
        .count()
}

fn aim_at_open_tile(w: &World, p: Entity) -> Position {
    let here = *w.get::<Position>(p).unwrap();
    let map = w.resource::<Map>();
    [(1i32, 0i32), (-1, 0), (0, 1), (0, -1)]
        .into_iter()
        .map(|(dx, dy)| (here.x as i32 + dx, here.y as i32 + dy))
        .find(|&(x, y)| x >= 0 && y >= 0 && !map.blocks(x as u16, y as u16))
        .map(|(x, y)| Position {
            x: x as u16,
            y: y as u16,
        })
        .expect("an open tile beside the player")
}

#[test]
fn dualzap_spends_two_charges_for_two_casts() {
    let mut w = test_world(1);
    let p = player(&mut w);
    put_on(&mut w, p, RingEffect::DualZap);
    let at = aim_at_open_tile(&w, p);

    let wand = zap_with(&mut w, p, WandEffect::MagicMissile, 5, at);

    assert_eq!(w.get::<Battery>(wand).unwrap().charges, 3);
    assert_eq!(bolts_logged(&w), 2);
}

#[test]
fn dualzap_on_a_last_charge_casts_once() {
    let mut w = test_world(1);
    let p = player(&mut w);
    put_on(&mut w, p, RingEffect::DualZap);
    let at = aim_at_open_tile(&w, p);

    let wand = zap_with(&mut w, p, WandEffect::MagicMissile, 1, at);

    assert!(w.get_entity(wand).is_none(), "the wand crumbles");
    assert_eq!(bolts_logged(&w), 1, "one charge buys one cast");
}

#[test]
fn dualzap_leaves_utility_wands_alone() {
    let mut w = test_world(1);
    let p = player(&mut w);
    put_on(&mut w, p, RingEffect::DualZap);
    let at = aim_at_open_tile(&w, p);

    let wand = zap_with(&mut w, p, WandEffect::SlowMonster, 5, at);

    assert_eq!(w.get::<Battery>(wand).unwrap().charges, 4);
}

#[test]
fn a_bare_hand_zaps_one_charge_one_cast() {
    let mut w = test_world(1);
    let p = player(&mut w);
    let at = aim_at_open_tile(&w, p);

    let wand = zap_with(&mut w, p, WandEffect::MagicMissile, 5, at);

    assert_eq!(w.get::<Battery>(wand).unwrap().charges, 4);
    assert_eq!(bolts_logged(&w), 1);
}

// ---------------------------------------------------------------------------
// The ring of health
// ---------------------------------------------------------------------------

fn hp(w: &World, e: Entity) -> (i32, i32) {
    let f = w.get::<Fighter>(e).unwrap();
    (f.hp, f.max_hp)
}

fn set_hp(w: &mut World, e: Entity, hp: i32) {
    w.get_mut::<Fighter>(e).unwrap().hp = hp;
}

#[test]
fn health_adds_two_to_max_and_current() {
    let mut w = test_world(1);
    let p = player(&mut w);
    let (cur, max) = hp(&w, p);

    put_on(&mut w, p, RingEffect::Health);

    assert_eq!(hp(&w, p), (cur + 2, max + 2));
}

#[test]
fn health_washes_hp_on_every_swap() {
    let mut w = test_world(1);
    let p = player(&mut w);
    let (_, max) = hp(&w, p);
    set_hp(&mut w, p, 1);

    let ring = put_on(&mut w, p, RingEffect::Health);
    assert_eq!(hp(&w, p), (3, max + 2));
    use_item(&mut w, p, ring);
    assert_eq!(hp(&w, p), (3, max), "taking it off keeps the hit points");
    use_item(&mut w, p, ring);
    assert_eq!(hp(&w, p), (5, max + 2), "and putting it back on pays again");
}

#[test]
fn health_taken_off_at_full_clamps_to_the_old_max() {
    let mut w = test_world(1);
    let p = player(&mut w);
    let (_, max) = hp(&w, p);

    let ring = put_on(&mut w, p, RingEffect::Health);
    use_item(&mut w, p, ring);

    assert_eq!(hp(&w, p), (max, max));
}

#[test]
fn health_leaves_with_a_ring_that_was_dropped() {
    let mut w = test_world(1);
    let p = player(&mut w);
    let (_, max) = hp(&w, p);

    let ring = put_on(&mut w, p, RingEffect::Health);
    force_unequip(&mut w, ring);
    equipment_effects_system(&mut w);

    assert_eq!(hp(&w, p), (max, max));
}

#[test]
fn health_changes_nothing_twice() {
    let mut w = test_world(1);
    let p = player(&mut w);
    let (cur, max) = hp(&w, p);

    put_on(&mut w, p, RingEffect::Health);
    equipment_effects_system(&mut w);
    equipment_effects_system(&mut w);

    assert_eq!(hp(&w, p), (cur + 2, max + 2));
}

#[test]
fn a_cursed_ring_of_health_can_kill() {
    let mut w = test_world(1);
    let p = player(&mut w);
    let (_, base_max) = hp(&w, p);
    set_hp(&mut w, p, 2);
    let ring = spawn_ring(&mut w, RingEffect::Health, Position { x: 0, y: 0 });
    w.entity_mut(ring).remove::<Position>();
    set_plus(&mut w, ring, -3);
    w.get_mut::<Backpack>(p).unwrap().items.push(ring);

    use_item(&mut w, p, ring);

    let (cur, max) = hp(&w, p);
    assert!(cur <= 0, "2 hp and a -3 ring leaves {cur}");
    assert_eq!(max, base_max - 3);
}
