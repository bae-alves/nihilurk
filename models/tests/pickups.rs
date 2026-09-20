//! Coins: what stepping on one does, what refuses to be stepped on, what the
//! staircase pays out, and what happens when you shoot one instead.
//!
//! Nothing here asserts how *much* a coin is worth. The amounts are the
//! `amount` column of `catalog::COINS` and belong to whoever is balancing the
//! game; what belongs to a test is that the column is honoured, that a coin
//! which would do nothing is left alone, and that a promise is kept or broken
//! for the right reasons.

use bevy_ecs::prelude::*;
use models::*;

fn test_world(seed: u64) -> World {
    let mut w = World::new();
    w.insert_resource(GameRng(ChaCha12Rng::seed_from_u64(seed)));
    w.insert_resource(RngSeed(seed));
    w.init_resource::<GameLog>();
    w.init_resource::<UseQueue>();
    w.init_resource::<AttackQueue>();
    w.init_resource::<ThrowQueue>();
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

/// The row for a named coin, so a test can read the dial instead of copying it.
fn coin_row(name: &str) -> &'static CoinDef {
    COINS.iter().find(|c| c.name == name).expect(name)
}

/// Drops the named coin on the player's own tile and steps on it. Returns
/// whether it was taken.
fn step_on(w: &mut World, name: &str) -> bool {
    let p = player(w);
    let at = *w.get::<Position>(p).unwrap();
    let coin = spawn_named(w, name, at).expect("a coin by that name");
    let taken = pick_up(w, p, coin).is_some();
    assert_eq!(
        taken,
        w.get_entity(coin).is_none(),
        "a coin is spent exactly when it is taken"
    );
    taken
}

fn score(w: &mut World) -> i64 {
    let p = player(w);
    w.get::<Score>(p).unwrap().value
}

// ---------------------------------------------------------------------------
// Treasure
// ---------------------------------------------------------------------------

#[test]
fn treasure_pays_its_row_into_the_score() {
    let mut w = test_world(1);
    let before = score(&mut w);
    assert!(step_on(&mut w, "gold coin"));
    assert_eq!(
        score(&mut w) - before,
        i64::from(coin_row("gold coin").amount)
    );
}

#[test]
fn treasure_is_always_worth_taking() {
    let mut w = test_world(1);
    assert!(step_on(&mut w, "silver coin"));
    assert!(step_on(&mut w, "silver coin"), "and taking again");
}

#[test]
fn the_relic_pays_its_value_the_moment_it_is_in_hand() {
    let mut w = test_world(1);
    let p = player(&mut w);
    let at = *w.get::<Position>(p).unwrap();
    let before = score(&mut w);
    let relic = spawn_named(&mut w, ELEMENT_OF_YOORD, at).expect("the relic");
    let worth = w.get::<Value>(relic).unwrap().amount;

    assert!(pick_up(&mut w, p, relic).is_some());

    assert_eq!(score(&mut w) - before, i64::from(worth));
    assert!(
        w.get::<Backpack>(p).unwrap().items.contains(&relic),
        "and it is carried, not spent — it is the run"
    );

    // Once, though. The one thing in the game that can be paid for and then
    // set down again must not be a drop-and-take-again money press.
    let paid = score(&mut w);
    w.get_mut::<Backpack>(p)
        .unwrap()
        .items
        .retain(|&e| e != relic);
    w.entity_mut(relic).insert(at);
    assert!(pick_up(&mut w, p, relic).is_some());
    assert_eq!(score(&mut w), paid, "worth is paid once");
}

// ---------------------------------------------------------------------------
// The coins that mend something
// ---------------------------------------------------------------------------

#[test]
fn a_red_coin_is_left_alone_until_you_are_hurt() {
    let mut w = test_world(1);
    assert!(!step_on(&mut w, "red coin"), "at full health, no thank you");

    let p = player(&mut w);
    w.get_mut::<Fighter>(p).unwrap().hp = 1;
    assert!(step_on(&mut w, "red coin"), "hurt, it is worth taking");
    assert!(w.get::<Fighter>(p).unwrap().hp > 1);
}

#[test]
fn a_coin_never_heals_past_the_ceiling() {
    let mut w = test_world(1);
    let p = player(&mut w);
    w.get_mut::<Fighter>(p).unwrap().hp = 1;
    let max = w.get::<Fighter>(p).unwrap().max_hp;
    for _ in 0..20 {
        step_on(&mut w, "red coin");
    }
    assert_eq!(w.get::<Fighter>(p).unwrap().hp, max);
}

#[test]
fn a_blue_coin_answers_an_empty_magic_pool_and_nothing_else() {
    let mut w = test_world(1);
    assert!(!step_on(&mut w, "blue coin"), "a full pool needs nothing");

    let p = player(&mut w);
    w.get_mut::<Magic>(p).unwrap().points = 0;
    assert!(step_on(&mut w, "blue coin"));
    let m = *w.get::<Magic>(p).unwrap();
    assert!(m.points > 0 && m.points <= m.max_points);
}

#[test]
fn a_rose_coin_wants_something_to_clear() {
    let mut w = test_world(1);
    assert!(!step_on(&mut w, "rosé coin"), "nothing wrong with you");

    let p = player(&mut w);
    assert!(blind(&mut w, p));
    assert!(confuse(&mut w, p, "You are confused!", "reels"));
    assert!(step_on(&mut w, "rosé coin"));
    assert!(
        w.get::<Blind>(p).is_none() && w.get::<Confused>(p).is_none(),
        "one coin clears more than one thing"
    );
}

#[test]
fn a_green_coin_wants_a_drained_arm() {
    let mut w = test_world(1);
    assert!(!step_on(&mut w, "green coin"), "an unpoisoned arm is fine");

    let p = player(&mut w);
    w.get_mut::<Fighter>(p).unwrap().power -= 3;
    assert!(step_on(&mut w, "green coin"));
    let f = w.get::<Fighter>(p).unwrap();
    assert_eq!(
        f.power, f.max_power,
        "and it gives back more than one point"
    );
}

// ---------------------------------------------------------------------------
// The two that pay at the stairs
// ---------------------------------------------------------------------------

/// Has something hit the player for real. Their armour is stripped first so the
/// opposed roll cannot come out at nothing: what is being tested is what a
/// landed blow costs, not whether one lands.
fn hurt(w: &mut World, p: Entity) {
    {
        let mut f = w.get_mut::<Fighter>(p).unwrap();
        f.armor = 0;
        f.armor_bonus = 0;
        f.hp = f.max_hp;
    }
    let armour: Vec<Entity> = equipped_items(w, p);
    for piece in armour {
        force_unequip(w, piece);
    }
    let at = *w.get::<Position>(p).unwrap();
    let brute = spawn_monster(
        w,
        MonsterDef::named("dragon"),
        Position {
            x: at.x + 1,
            y: at.y,
        },
    );
    resolve_attack(w, brute, p);
    assert!(
        w.get::<Fighter>(p).unwrap().hp < w.get::<Fighter>(p).unwrap().max_hp,
        "the blow has to have landed for this test to mean anything"
    );
}

/// Walks the player onto the down-stair and takes it.
fn descend(w: &mut World) {
    let p = player(w);
    let down = w
        .resource::<Map>()
        .tiles
        .iter()
        .position(|&t| t == TileType::Downstairs)
        .unwrap();
    w.get_mut::<Position>(p).unwrap().x = (down % MAP_WIDTH as usize) as u16;
    w.get_mut::<Position>(p).unwrap().y = (down / MAP_WIDTH as usize) as u16;
    assert!(change_level(w, true));
}

#[test]
fn a_platinum_coin_pays_a_point_of_die_at_the_stairs() {
    let mut w = test_world(1);
    let p = player(&mut w);
    assert!(step_on(&mut w, "platinum coin"));
    assert!(w.get::<Plated>(p).is_some(), "PLAT");
    let (power, armor) = {
        let f = w.get::<Fighter>(p).unwrap();
        (f.power, f.armor)
    };

    descend(&mut w);

    let f = w.get::<Fighter>(p).unwrap();
    assert_eq!(
        (f.power + f.armor) - (power + armor),
        1,
        "one point, into one of the two"
    );
    assert!(w.get::<Plated>(p).is_none(), "and the promise is settled");
}

#[test]
fn a_forge_coin_pays_a_point_of_plus_at_the_stairs() {
    let mut w = test_world(1);
    let p = player(&mut w);
    assert!(step_on(&mut w, "forge coin"));
    let before = equipped_total::<PowerBonus>(&w, p) + equipped_total::<ArmorBonus>(&w, p);

    descend(&mut w);

    assert_eq!(
        equipped_total::<PowerBonus>(&w, p) + equipped_total::<ArmorBonus>(&w, p),
        before + 1,
        "one point of plus, onto the weapon or the armour"
    );
    assert!(w.get::<Forged>(p).is_none());
}

#[test]
fn being_hurt_breaks_the_promise() {
    let mut w = test_world(1);
    let p = player(&mut w);
    assert!(step_on(&mut w, "platinum coin"));
    assert!(step_on(&mut w, "forge coin"));

    hurt(&mut w, p);

    assert!(w.get::<Plated>(p).is_none(), "one point of damage did it");
    assert!(w.get::<Forged>(p).is_none());
    let (power, armor) = {
        let f = w.get::<Fighter>(p).unwrap();
        (f.power, f.armor)
    };
    descend(&mut w);
    let f = w.get::<Fighter>(p).unwrap();
    assert_eq!(
        (f.power, f.armor),
        (power, armor),
        "and the stairs pay nothing"
    );
}

#[test]
fn a_promise_you_already_hold_leaves_the_coin_on_the_floor() {
    let mut w = test_world(1);
    assert!(step_on(&mut w, "platinum coin"));
    assert!(
        !step_on(&mut w, "platinum coin"),
        "no stacking perfection: the second one keeps"
    );
}

// ---------------------------------------------------------------------------
// Shooting them instead
// ---------------------------------------------------------------------------

/// Drops the named coin a few tiles east of the player and sets it off as
/// though they had shot it. Returns what went off.
fn shoot(w: &mut World, coin: &str) -> (Option<TrickShot>, Entity) {
    let p = player(w);
    let at = *w.get::<Position>(p).unwrap();
    let target = Position {
        x: at.x + 4,
        y: at.y,
    };
    let entity = spawn_named(w, coin, target).expect("a coin by that name");
    (detonate_at(w, target, Some(p)), entity)
}

#[test]
fn a_shot_coin_goes_off_and_is_gone() {
    let mut w = test_world(1);
    let (shot, coin) = shoot(&mut w, "gold coin");
    assert_eq!(shot, Some(TrickShot::Pickup));
    assert!(w.get_entity(coin).is_none(), "the shot spent it");
}

#[test]
fn shooting_a_coin_pays_the_shooter() {
    let mut w = test_world(1);
    let before = score(&mut w);
    shoot(&mut w, "gold coin");
    assert_eq!(
        score(&mut w) - before,
        i64::from(coin_row("gold coin").amount),
        "the coin gives itself up to whoever set it off"
    );
}

#[test]
fn a_shot_coin_reaches_across_the_room_with_its_effect() {
    let mut w = test_world(1);
    let p = player(&mut w);
    w.get_mut::<Fighter>(p).unwrap().hp = 1;

    shoot(&mut w, "red coin");

    // The coin's effect lands before its burst does, so the healing is not
    // something the player's own blast can take back off them.
    assert!(
        w.get::<Fighter>(p).unwrap().hp > 1,
        "a red coin heals whoever shot it"
    );
}

#[test]
fn a_shot_coin_does_not_ask_whether_you_needed_it() {
    // Stepping over a coin you cannot use leaves it for later; shooting one is
    // a decision, and a decision is allowed to be a waste.
    let mut w = test_world(1);
    let p = player(&mut w);
    let full = w.get::<Fighter>(p).unwrap().hp;
    assert_eq!(shoot(&mut w, "red coin").0, Some(TrickShot::Pickup));
    assert!(w.get::<Fighter>(p).unwrap().hp <= full);
}

#[test]
fn a_blast_sets_off_the_coins_it_covers_and_pays_the_zapper() {
    let mut w = test_world(1);
    let p = player(&mut w);
    let at = *w.get::<Position>(p).unwrap();
    let center = Position {
        x: at.x + 3,
        y: at.y,
    };
    spawn_named(&mut w, "gold coin", center).expect("a coin");
    let before = score(&mut w);
    let wand = spawn_wand(&mut w, WandEffect::Fire, Position { x: 0, y: 0 });
    w.entity_mut(wand).remove::<Position>();
    w.get_mut::<Backpack>(p).unwrap().items.push(wand);

    w.resource_mut::<UseQueue>().uses.push(WantsToUse {
        user: p,
        item: wand,
        target: Some(center),
        slot_idx: None,
    });
    w.get_mut::<Backpack>(p)
        .unwrap()
        .items
        .retain(|&e| e != wand);
    item_system(&mut w);

    assert_eq!(
        score(&mut w) - before,
        i64::from(coin_row("gold coin").amount),
        "a coin caught in your own blast is a coin you set off"
    );
}

#[test]
fn a_shot_coin_reaches_further_than_a_shot_trap() {
    // Both bursts are centred the same distance away; the wider one catches a
    // creature the narrower one cannot. Which radius is which is
    // `constants::traps`, and the point here is only that they differ this way.
    fn caught(what: &str, reach: u16) -> bool {
        let mut w = test_world(5);
        let p = player(&mut w);
        let at = *w.get::<Position>(p).unwrap();
        let center = Position {
            x: at.x + 6,
            y: at.y,
        };
        let victim_at = Position {
            x: center.x + reach,
            y: center.y,
        };
        // Clear the floor so nothing else is standing in the way.
        let mobs: Vec<Entity> = w.query_filtered::<Entity, With<Mob>>().iter(&w).collect();
        for m in mobs {
            if m != p {
                w.despawn(m);
            }
        }
        let victim = spawn_monster(&mut w, MonsterDef::named("troll"), victim_at);
        w.get_mut::<Fighter>(victim).unwrap().hp = 500;
        let thing = spawn_named(&mut w, what, center).expect(what);
        // A trap has to have been found before a shot can be aimed at it.
        w.entity_mut(thing).remove::<Hidden>();
        detonate_at(&mut w, center, Some(p));
        w.get::<Fighter>(victim).is_some_and(|f| f.hp < 500)
    }

    assert!(caught("gold coin", 2), "a coin's burst reaches two tiles");
    assert!(!caught("bear trap", 2), "a trap's does not");
}

#[test]
fn the_element_answers_a_shot_and_survives_it() {
    let mut w = test_world(1);
    let p = player(&mut w);
    let at = *w.get::<Position>(p).unwrap();
    let target = Position {
        x: at.x + 2,
        y: at.y,
    };
    let relic = spawn_named(&mut w, ELEMENT_OF_YOORD, target).expect("the relic");

    assert_eq!(
        detonate_at(&mut w, target, Some(p)),
        Some(TrickShot::Ultimate)
    );

    assert!(
        w.get::<Position>(relic).is_some(),
        "nothing the player does to the relic can cost them it"
    );
}

#[test]
fn a_shot_hero_coin_gives_up_everything_it_knows() {
    let mut w = test_world(1);
    // Somebody for the second burst to echo off — the shout only goes up once
    // the first burst has caught a body to centre the next one on.
    let p = player(&mut w);
    let at = *w.get::<Position>(p).unwrap();
    let beside = Position {
        x: at.x + 5,
        y: at.y,
    };
    let troll = spawn_monster(&mut w, MonsterDef::named("troll"), beside);
    w.get_mut::<Fighter>(troll).unwrap().hp = 500;
    let (shot, coin) = shoot(&mut w, "hero coin");

    assert_eq!(shot, Some(TrickShot::Pickup));
    assert!(
        w.get_entity(coin).is_none(),
        "unlike the relic, it is spent"
    );
    assert!(
        w.resource::<GameLog>()
            .history
            .iter()
            .any(|l| l.contains("ULTIMATE TRICK SHOT!")),
        "it answers the way the Element of Yoord does"
    );
}

#[test]
fn a_chained_coin_still_pays_whoever_started_the_chain() {
    let mut w = test_world(3);
    let p = player(&mut w);
    let at = *w.get::<Position>(p).unwrap();
    let first = Position {
        x: at.x + 4,
        y: at.y,
    };
    let second = Position {
        x: at.x + 5,
        y: at.y,
    };
    spawn_named(&mut w, "silver coin", first).expect("a coin");
    let chained = spawn_named(&mut w, "gold coin", second).expect("a coin");
    let before = score(&mut w);

    detonate_at(&mut w, first, Some(p));

    assert!(w.get_entity(chained).is_none(), "the chain spent it");
    assert_eq!(
        score(&mut w) - before,
        i64::from(coin_row("silver coin").amount) + i64::from(coin_row("gold coin").amount),
        "a chain reaction is still your shot, and it still pays you"
    );
}

#[test]
fn a_coin_whose_shooter_died_first_is_simply_spent() {
    // A chain runs on past the blast that started it, and the author of a shot
    // is as catchable as anyone else. A platinum coin reached by a chain whose
    // shooter is already gone used to attach its promise to a despawned
    // entity.
    let mut w = test_world(1);
    let p = player(&mut w);
    let at = *w.get::<Position>(p).unwrap();
    let spot = Position {
        x: at.x + 4,
        y: at.y,
    };
    let ghost = spawn_monster(&mut w, MonsterDef::named("orc"), spot);
    let coin = spawn_named(&mut w, "platinum coin", spot).expect("a coin");
    w.despawn(ghost);

    assert!(detonate_pickup(&mut w, coin, Some(ghost)));
    assert!(w.get_entity(coin).is_none(), "spent, and paid to nobody");
}
