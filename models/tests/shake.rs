//! What actually shakes the screen.
//!
//! `models/src/shake.rs`'s own unit tests cover the resource's bookkeeping and
//! `particle-core` covers the arithmetic. This file covers the part neither
//! can: that the moments the feature was asked for are wired to the right
//! kinds, that nothing else is, and that a shake is still pure decoration —
//! absent from a world that never inserted the resource, and silent under
//! `-nshake`.
//!
//! Five kinds. Anything of the player's that gets through armour — a swing, a
//! throw or shot, a bolt that bit — takes the tick; a kill in sight is short;
//! an excellent hit and a blast in sight are heavy; crossing into the low-HP
//! warning is long; their own death is the longest, and the last thing the map
//! does. Plus the hits that deliberately arm nothing: a glancing blow and the
//! shot equivalent, which clink instead, and anything out of sight.

use bevy_ecs::prelude::*;
use bevy_ecs::schedule::Schedule;
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
    w.insert_resource(Shake::new());
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

/// A punching bag on the player's own tile-neighbourhood, with enough HP that
/// no single swing kills it and no armour to eat the blow.
fn spawn_dummy(w: &mut World, hp: i32) -> Entity {
    let pos = player_pos(w);
    w.spawn((
        Mob {
            movement_type: MovementType::Static,
        },
        Name {
            what: "dummy".into(),
        },
        Position {
            x: pos.x + 1,
            y: pos.y,
        },
        Fighter {
            hp,
            max_hp: hp,
            armor: 0,
            power: 1,
            max_power: 1,
            armor_bonus: 0,
            power_bonus: 0,
        },
    ))
    .id()
}

/// A punching bag standing one *open* tile from the player, plus that tile — a
/// throw needs a clear lane, which `spawn_dummy`'s blind `x + 1` cannot
/// promise. Which way the lane points is nobody's business.
fn dummy_down_the_lane(w: &mut World, hp: i32) -> (Entity, Position) {
    let p = player_pos(w);
    let map = w.resource::<Map>().clone();
    let at = [(1i32, 0i32), (-1, 0), (0, 1), (0, -1)]
        .into_iter()
        .map(|(dx, dy)| Position {
            x: (p.x as i32 + dx) as u16,
            y: (p.y as i32 + dy) as u16,
        })
        .find(|q| !map.blocks(q.x, q.y))
        .expect("the player is walled in on all four sides");
    let dummy = spawn_dummy(w, hp);
    *w.get_mut::<Position>(dummy).unwrap() = at;
    (dummy, at)
}

/// Hurl `item` at a tile: the engine's Throw action from the point where the
/// item has already left the pack, which is all a scripted throw needs.
fn hurl(w: &mut World, thrower: Entity, item: Entity, target: Position) {
    w.entity_mut(item).remove::<Position>();
    w.resource_mut::<ThrowQueue>().throws.push(WantsToThrow {
        thrower,
        item,
        target,
    });
    throw_system(w);
}

/// The opening glyphs of every mote queued so far — how a test tells a landed
/// hit's spark (`‼`) from a glancing one's clink (`+`).
fn spark_glyphs(w: &World) -> Vec<char> {
    w.resource::<Particles>()
        .live
        .iter()
        .map(|p| p.frames[0].0)
        .collect()
}

/// Refresh the player's viewshed, so the "did you see it?" gates on cosmetic
/// effects have something real to answer from.
fn look(w: &mut World) {
    let mut schedule = Schedule::default();
    schedule.add_systems(visibility_system);
    schedule.run(w);
}

/// Give the player a fresh, well-charged wand, already in the pack.
fn give_wand(w: &mut World, p: Entity, effect: WandEffect) -> Entity {
    let wand = spawn_wand(w, effect, Position { x: 0, y: 0 });
    w.entity_mut(wand).remove::<Position>();
    w.get_mut::<Battery>(wand).unwrap().charges = 9;
    w.get_mut::<Backpack>(p).unwrap().items.push(wand);
    wand
}

/// Zap `wand` at a tile — the engine's targeted "Use", and the only public way
/// in to `elemental_blast`. A wand of fire is the shortest path from a test to
/// a real explosion.
fn zap(w: &mut World, user: Entity, wand: Entity, target: Position) {
    let i = w
        .get_mut::<Backpack>(user)
        .unwrap()
        .items
        .iter()
        .position(|&e| e == wand);
    if let Some(i) = i {
        w.get_mut::<Backpack>(user).unwrap().items.remove(i);
        w.resource_mut::<UseQueue>().uses.push(WantsToUse {
            user,
            item: wand,
            target: Some(target),
            slot_idx: Some(i),
        });
    }
    item_system(w);
}

/// Leave the player one point above the low-HP warning's threshold, on a pool
/// deep enough that any single blow crosses it without killing them.
///
/// The deep pool is the point. On a starting 12 HP the threshold is 3, and a
/// blow big enough to cross it is usually big enough to finish the job — which
/// is a different trigger (there isn't one: see `a_killing_blow_shakes_nothing`).
/// Returns the threshold, so a caller that needs to stay under it can.
fn poise_on_the_threshold(w: &mut World, p: Entity) -> i32 {
    const POOL: i32 = 100;
    let threshold = (POOL as f32 * models::constants::player::LOW_HP_WARNING_FRACTION) as i32;
    let mut fighter = w.get_mut::<Fighter>(p).unwrap();
    fighter.max_hp = POOL;
    fighter.hp = threshold + 1;
    threshold
}

// --- The player's own swing: tick, or heavy ---------------------------------

#[test]
fn every_blow_the_player_lands_shakes_and_a_crit_shakes_harder() {
    // Sweeps seeds until both outcomes have been seen, then checks each one
    // armed exactly what it should have. `resolve_attack` is the only thing
    // that decides a hit is excellent, so this is also the only place to look.
    // The dummy has no armour, so no swing here can be a glancing one — that
    // case is `a_glancing_blow_clinks_instead_of_shaking` below.
    let (mut saw_excellent, mut saw_ordinary) = (false, false);
    for seed in 0..2000u64 {
        let mut w = test_world(seed);
        let attacker = player(&mut w);
        let target = spawn_dummy(&mut w, 100);
        resolve_attack(&mut w, attacker, target);

        let excellent = w
            .resource::<GameLog>()
            .history
            .iter()
            .any(|l| l.contains("excellent hit"));
        let expected = match excellent {
            true => ShakeKind::Heavy,
            false => ShakeKind::Hit,
        };
        assert_eq!(
            w.resource::<Shake>().remaining_ms(),
            expected.duration_ms(),
            "seed {seed}: a hit with excellent={excellent} armed the wrong kick"
        );

        saw_excellent |= excellent;
        saw_ordinary |= !excellent;
        if saw_excellent && saw_ordinary {
            return;
        }
    }
    panic!("never saw both an excellent and an ordinary hit");
}

#[test]
fn a_glancing_blow_clinks_instead_of_shaking() {
    // The one hit that draws blood and moves nothing: the armour turned the
    // swing aside and only the chip-damage floor got through. It says so with
    // a cold spark — checked by the glyph it opens on, since that is the whole
    // difference between it and a landed hit's — and with a map that holds
    // still. A crit skips the floor entirely, so keep swinging past those.
    let mut w = test_world(7);
    let attacker = player(&mut w);
    // Armour deep enough that the armour roll eats an ordinary weapon roll
    // almost every time.
    let target = spawn_dummy(&mut w, 100);
    w.get_mut::<Fighter>(target).unwrap().armor = 40;
    w.insert_resource(Particles::new());

    for _ in 0..200 {
        w.resource_mut::<Shake>().settle();
        w.resource_mut::<Particles>().clear();
        w.resource_mut::<GameLog>().history.clear();
        resolve_attack(&mut w, attacker, target);

        let glancing = w
            .resource::<GameLog>()
            .history
            .iter()
            .any(|l| l.contains("glancing blow"));
        if !glancing {
            continue;
        }
        assert!(
            !w.resource::<Shake>().active(),
            "a glancing blow shook the screen"
        );
        let opening = spark_glyphs(&w);
        assert!(
            opening.contains(&'+'),
            "a glancing blow queued no clink spark (glyphs: {opening:?})"
        );
        return;
    }
    panic!("never saw a glancing blow against armour 40");
}

// --- The player's own shot: the same tick -----------------------------------

#[test]
fn a_shot_that_draws_blood_shakes_like_a_sword_hit() {
    // A missile hit is a hit. It arrives through `helpers::apply_damage`
    // rather than `resolve_attack`, which is exactly why it needs its own
    // test: the two damage paths have to agree about this.
    let mut w = test_world(3);
    look(&mut w);
    let p = player(&mut w);
    let (dummy, at) = dummy_down_the_lane(&mut w, 500);
    w.insert_resource(Particles::new());
    // An arrow ignores armour, so this cannot come out a glance.
    let arrow = spawn_ammo(&mut w, "arrow", Position { x: 0, y: 0 });

    hurl(&mut w, p, arrow, at);

    assert!(
        w.get::<Fighter>(dummy).unwrap().hp < 500,
        "the arrow missed the dummy it was aimed at"
    );
    assert_eq!(
        w.resource::<Shake>().remaining_ms(),
        ShakeKind::Hit.duration_ms(),
        "a shot that drew blood armed the wrong kick"
    );
}

#[test]
fn a_shot_the_armour_turns_aside_clinks_instead_of_shaking() {
    // The ranged glancing blow, and harsher than melee's: a non-projectile
    // throw is blunted by the armour *plus*, with no chip-damage floor under
    // it, so a mace against enough armour does nothing at all. It still says
    // where it struck, and the map still holds still.
    let mut w = test_world(3);
    look(&mut w);
    let p = player(&mut w);
    let (dummy, at) = dummy_down_the_lane(&mut w, 500);
    w.get_mut::<Fighter>(dummy).unwrap().armor_bonus = 20;
    w.insert_resource(Particles::new());
    let mace = spawn_weapon(&mut w, "mace", Position { x: 0, y: 0 });

    hurl(&mut w, p, mace, at);

    assert_eq!(
        w.get::<Fighter>(dummy).unwrap().hp,
        500,
        "the mace got through armour 20 — pick a heavier suit"
    );
    assert!(
        !w.resource::<Shake>().active(),
        "a shot that glanced off shook the screen"
    );
    let opening = spark_glyphs(&w);
    assert!(
        opening.contains(&'+'),
        "a glancing shot queued no clink spark (glyphs: {opening:?})"
    );
}

#[test]
fn a_monsters_shot_never_shakes_the_screen_either() {
    // The melee gate, at range: only the player's own hits move the map.
    let mut w = test_world(3);
    look(&mut w);
    let p = player(&mut w);
    let (shooter, _) = dummy_down_the_lane(&mut w, 500);
    let max_hp = w.get::<Fighter>(p).unwrap().max_hp;
    let target = player_pos(&mut w);

    for _ in 0..20 {
        // Healed back up each shot so the low-HP crossing can't be what fires.
        w.get_mut::<Fighter>(p).unwrap().hp = max_hp;
        let arrow = spawn_ammo(&mut w, "arrow", Position { x: 0, y: 0 });
        hurl(&mut w, shooter, arrow, target);
        assert!(
            !w.resource::<Shake>().active(),
            "a monster's shot shook the screen"
        );
    }
}

#[test]
fn a_monsters_own_hit_never_shakes_the_screen() {
    // `excellent` is player-only, so a monster swinging — even 2000 times —
    // must never arm one. The counterpart to the test above.
    for seed in 0..500u64 {
        let mut w = test_world(seed);
        let victim = player(&mut w);
        let attacker = spawn_dummy(&mut w, 20);
        // Heal past the low-HP threshold each swing so the *other* trigger
        // can't be what fires.
        let max_hp = w.get::<Fighter>(victim).unwrap().max_hp;
        for _ in 0..4 {
            w.get_mut::<Fighter>(victim).unwrap().hp = max_hp;
            resolve_attack(&mut w, attacker, victim);
            assert!(
                !w.resource::<Shake>().active(),
                "seed {seed}: a monster's swing shook the screen"
            );
        }
    }
}

// --- A kill: short ----------------------------------------------------------

#[test]
fn killing_something_in_sight_shakes_the_screen() {
    let mut w = test_world(13);
    look(&mut w);
    let p = player(&mut w);
    let victim = spawn_dummy(&mut w, 1);

    resolve_attack(&mut w, p, victim);

    assert!(
        w.get_entity(victim).is_none(),
        "the dummy survived a swing it had 1 HP for"
    );
    assert!(w.resource::<Shake>().active(), "a kill must land a kick");
    // A crit could have done the killing, and that is the heavier shake; either
    // way it is one of the two, and never nothing.
    let left = w.resource::<Shake>().remaining_ms();
    assert!(
        left == ShakeKind::Kill.duration_ms() || left == ShakeKind::Heavy.duration_ms(),
        "a kill armed neither its own kick nor a crit's thump ({left} ms)"
    );
}

#[test]
fn a_kill_the_player_cannot_see_shakes_nothing() {
    // Same information-leak gate the blast has: something dying in a room the
    // player has never been in must not announce itself through the floor.
    let mut w = test_world(13);
    look(&mut w);
    let p = player(&mut w);

    let unseen = {
        let seen: Vec<(u16, u16)> = w.get::<Viewshed>(p).unwrap().visible_tiles.clone();
        (1..MAP_WIDTH - 1)
            .flat_map(|x| (1..MAP_HEIGHT - 1).map(move |y| (x, y)))
            .find(|t| !seen.contains(t))
            .map(|(x, y)| Position { x, y })
            .expect("the whole floor cannot be visible at once")
    };

    // A dummy off in the dark, killed outright. `resolve_attack` doesn't care
    // that the two are nowhere near each other — only the sight gate does. A
    // crit would arm the *heavy* shake on its own account, whatever the sight
    // gate says, so keep swinging until one lands that isn't one.
    for _ in 0..40 {
        let victim = spawn_dummy(&mut w, 1);
        *w.get_mut::<Position>(victim).unwrap() = unseen;
        resolve_attack(&mut w, p, victim);
        let crit = w
            .resource::<GameLog>()
            .history
            .iter()
            .any(|l| l.contains("excellent hit"));
        if crit {
            w.resource_mut::<Shake>().settle();
            w.resource_mut::<GameLog>().history.clear();
            continue;
        }
        assert!(
            !w.resource::<Shake>().active(),
            "a kill out of sight shook the screen"
        );
        return;
    }
    panic!("every swing in the sweep was an excellent hit");
}

// --- Explosion: heavy -------------------------------------------------------

#[test]
fn a_blast_in_sight_shakes_the_screen() {
    let mut w = test_world(7);
    look(&mut w);
    let p = player(&mut w);
    let at = player_pos(&mut w);
    let wand = give_wand(&mut w, p, WandEffect::Fire);

    zap(&mut w, p, wand, at);

    assert!(
        w.resource::<Shake>().active(),
        "a blast underfoot must thump"
    );
    assert_eq!(
        w.resource::<Shake>().remaining_ms(),
        ShakeKind::Heavy.duration_ms()
    );
}

#[test]
fn a_blast_the_player_cannot_see_shakes_nothing() {
    // The information leak this gate exists to prevent: shaking for a blast in
    // a room the player has never been in tells them something the renderer is
    // careful never to draw.
    let mut w = test_world(7);
    look(&mut w);
    let p = player(&mut w);

    let unseen = {
        let seen: Vec<(u16, u16)> = w.get::<Viewshed>(p).unwrap().visible_tiles.clone();
        (1..MAP_WIDTH - 1)
            .flat_map(|x| (1..MAP_HEIGHT - 1).map(move |y| (x, y)))
            .find(|t| !seen.contains(t))
            .map(|(x, y)| Position { x, y })
            .expect("the whole floor cannot be visible at once")
    };

    let wand = give_wand(&mut w, p, WandEffect::Fire);
    zap(&mut w, p, wand, unseen);

    assert!(
        !w.resource::<Shake>().active(),
        "a blast out of sight shook the screen"
    );
}

// --- A bolt: the same tick as a hit -----------------------------------------

#[test]
fn a_bolt_that_bites_something_in_sight_shakes_like_a_hit() {
    // A bolt wand is the player's weapon at range, and lands like one. Striking
    // is the plainest of the four — no element, so nothing in the bestiary can
    // shrug it off and leave this testing an empty beam.
    let mut w = test_world(4);
    let (dummy, at) = dummy_down_the_lane(&mut w, 500);
    look(&mut w);
    let p = player(&mut w);
    let wand = give_wand(&mut w, p, WandEffect::Striking);

    zap(&mut w, p, wand, at);

    assert!(
        w.get::<Fighter>(dummy).unwrap().hp < 500,
        "the bolt went past the dummy standing in it"
    );
    assert_eq!(
        w.resource::<Shake>().remaining_ms(),
        ShakeKind::Hit.duration_ms(),
        "a bolt that drew blood armed the wrong kick"
    );
}

#[test]
fn a_bolt_that_hits_nothing_shakes_nothing() {
    // Down an empty lane it is just a light show. The kick is for landing on
    // something, not for zapping.
    let mut w = test_world(4);
    let (dummy, at) = dummy_down_the_lane(&mut w, 500);
    w.despawn(dummy);
    look(&mut w);
    let p = player(&mut w);
    let wand = give_wand(&mut w, p, WandEffect::Striking);

    zap(&mut w, p, wand, at);

    assert!(
        !w.resource::<Shake>().active(),
        "a bolt through thin air shook the screen"
    );
}

#[test]
fn a_bolt_landing_out_of_sight_shakes_nothing() {
    // The blast's information leak, at bolt scale — and worse, because nothing
    // logs a bolt's damage: without this gate a zap down a dark corridor would
    // announce, through the floor, that there is something standing in it.
    let mut w = test_world(4);
    look(&mut w);
    let p = player(&mut w);

    let unseen = {
        let seen: Vec<(u16, u16)> = w.get::<Viewshed>(p).unwrap().visible_tiles.clone();
        (1..MAP_WIDTH - 1)
            .flat_map(|x| (1..MAP_HEIGHT - 1).map(move |y| (x, y)))
            .find(|t| !seen.contains(t))
            .map(|(x, y)| Position { x, y })
            .expect("the whole floor cannot be visible at once")
    };
    // A dummy out in the dark, and a bolt aimed straight at it. `trace_bolt`
    // walks the line whatever the lighting; only the sight gate cares.
    let dummy = spawn_dummy(&mut w, 500);
    *w.get_mut::<Position>(dummy).unwrap() = unseen;
    let wand = give_wand(&mut w, p, WandEffect::Striking);

    zap(&mut w, p, wand, unseen);

    assert!(
        !w.resource::<Shake>().active(),
        "a bolt landing out of sight shook the screen"
    );
}

// --- Low HP: long -----------------------------------------------------------

#[test]
fn crossing_into_the_low_hp_warning_shakes_the_screen_once() {
    // Driven by setting off a wand of fire at the player's own feet. A blast
    // catches everyone in it, its caster included (a *bolt* pointedly does not
    // — `trace_bolt` skips the zapper), and it damages through
    // `helpers::apply_damage`, which is the path every trap, dart and bolt in
    // the game shares.
    //
    // That makes this the harder case, not the easier one: the blast arms the
    // medium shake too, so it is also the check that the long one wins.
    let mut w = test_world(11);
    look(&mut w);
    let p = player(&mut w);
    let at = player_pos(&mut w);

    let threshold = poise_on_the_threshold(&mut w, p);
    assert!(!w.resource::<Shake>().active());

    let wand = give_wand(&mut w, p, WandEffect::Fire);
    zap(&mut w, p, wand, at);
    assert!(
        w.resource::<Shake>().active(),
        "crossing into the red did not shake"
    );
    assert_eq!(
        w.resource::<Shake>().remaining_ms(),
        ShakeKind::Wounded.duration_ms(),
        "the low-HP lurch is the long one, and it outranks the blast's own thump"
    );

    // Still down there, hurt again: the warning is a one-shot on the crossing,
    // and so is its shake. What is left rocking is the second blast's own
    // thump, which is a different (and shorter) thing.
    w.get_mut::<Fighter>(p).unwrap().hp = threshold - 1;
    w.resource_mut::<Shake>().settle();
    let again = give_wand(&mut w, p, WandEffect::Fire);
    zap(&mut w, p, again, at);
    assert_eq!(
        w.resource::<Shake>().remaining_ms(),
        ShakeKind::Heavy.duration_ms(),
        "the lurch repeated while already wounded"
    );
}

#[test]
fn a_melee_blow_into_the_red_warns_and_shakes_like_any_other_damage() {
    // Melee is the one damage path that does not run through `apply_damage`;
    // before the shake landed it was also the one path that never reported
    // "You are badly wounded!". Being clubbed into the red is the single most
    // likely way to get there, so this is the case that matters most.
    let mut w = test_world(3);
    let victim = player(&mut w);
    let attacker = spawn_dummy(&mut w, 20);

    poise_on_the_threshold(&mut w, victim);
    w.get_mut::<Fighter>(attacker).unwrap().power = 8;

    // Swing until something lands — a monster's blow can be absorbed outright.
    for _ in 0..40 {
        resolve_attack(&mut w, attacker, victim);
        if w.resource::<Shake>().active() {
            break;
        }
    }

    assert!(
        w.resource::<GameLog>()
            .history
            .iter()
            .any(|l| l.contains("badly wounded")),
        "melee never reported the low-HP warning"
    );
    assert_eq!(
        w.resource::<Shake>().remaining_ms(),
        ShakeKind::Wounded.duration_ms()
    );
}

// --- Death: the longest -----------------------------------------------------

#[test]
fn the_killing_blow_arms_the_longest_shake_there_is() {
    // Dying is not "being wounded" — it is the end of the run, and the last
    // thing the map does before the death screen slides in over it.
    let mut w = test_world(5);
    let p = player(&mut w);
    let attacker = spawn_dummy(&mut w, 20);
    w.get_mut::<Fighter>(p).unwrap().hp = 1;
    w.get_mut::<Fighter>(attacker).unwrap().power = 40;

    for _ in 0..40 {
        resolve_attack(&mut w, attacker, p);
        if w.resource::<Ending>().player_dead {
            break;
        }
    }
    assert!(
        w.resource::<Ending>().player_dead,
        "the dummy never landed a blow"
    );
    assert_eq!(
        w.resource::<Shake>().remaining_ms(),
        ShakeKind::Death.duration_ms(),
        "the last shake of a run outlasts every other one"
    );
}

#[test]
fn dying_to_something_with_no_killer_shakes_just_as_hard() {
    // The other death path: `finish_indirect_kill`, reached here by setting off
    // a wand of fire underfoot with no HP to spare. It flags the same `Ending`
    // and must arm the same shake — the player is no less dead for the blast
    // having no name.
    let mut w = test_world(11);
    look(&mut w);
    let p = player(&mut w);
    let at = player_pos(&mut w);
    w.get_mut::<Fighter>(p).unwrap().hp = 1;

    let wand = give_wand(&mut w, p, WandEffect::Fire);
    zap(&mut w, p, wand, at);

    assert!(w.resource::<Ending>().player_dead, "the blast spared them");
    assert_eq!(
        w.resource::<Shake>().remaining_ms(),
        ShakeKind::Death.duration_ms()
    );
}

// --- It stays decoration ----------------------------------------------------

#[test]
fn nothing_shakes_with_the_feature_switched_off() {
    // `-nshake`, over every trigger at once.
    let mut w = test_world(7);
    w.resource_mut::<Shake>().enabled = false;
    look(&mut w);

    let p = player(&mut w);
    let at = player_pos(&mut w);
    poise_on_the_threshold(&mut w, p);

    // One zap underfoot is both triggers at once: a blast in plain sight, and
    // its own damage knocking the caster into the red.
    let wand = give_wand(&mut w, p, WandEffect::Fire);
    zap(&mut w, p, wand, at);
    assert!(
        !w.resource::<Shake>().active(),
        "a blast, or the wound it dealt, shook with -nshake"
    );

    w.get_mut::<Fighter>(p).unwrap().hp = 50;
    let (target, lane) = dummy_down_the_lane(&mut w, 100);
    for _ in 0..200 {
        w.get_mut::<Fighter>(target).unwrap().hp = 100;
        resolve_attack(&mut w, p, target);
        assert!(
            !w.resource::<Shake>().active(),
            "a swing shook with -nshake"
        );
        let arrow = spawn_ammo(&mut w, "arrow", Position { x: 0, y: 0 });
        hurl(&mut w, p, arrow, lane);
        assert!(!w.resource::<Shake>().active(), "a shot shook with -nshake");
    }
}

#[test]
fn a_world_with_no_shake_resource_still_fights_and_explodes() {
    // The headless case: every test world in this suite that predates the
    // feature, and every one written after it that does not care. Arming a
    // shake on a world without the resource must be a silent no-op, exactly as
    // it is for the particle layer.
    let mut w = World::new();
    w.insert_resource(GameRng(ChaCha12Rng::seed_from_u64(1)));
    w.insert_resource(RngSeed(1));
    w.init_resource::<GameLog>();
    w.init_resource::<UseQueue>();
    w.init_resource::<AttackQueue>();
    w.init_resource::<Ending>();
    w.init_resource::<PlayerTempo>();
    w.insert_resource(PlayerName {
        what: "TESTER".into(),
    });
    initialize_world(&mut w);
    assert!(w.get_resource::<Shake>().is_none());

    look(&mut w);
    let p = player(&mut w);
    let target = spawn_dummy(&mut w, 100);
    for _ in 0..200 {
        resolve_attack(&mut w, p, target);
    }

    let at = player_pos(&mut w);
    poise_on_the_threshold(&mut w, p);
    let wand = give_wand(&mut w, p, WandEffect::Fire);
    zap(&mut w, p, wand, at);
}
