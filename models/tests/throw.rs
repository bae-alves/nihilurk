//! Throwing: the third thing the pack screen can do with an item. A hurled
//! object flies to the spot you picked, stops at the first wall or creature in
//! the way, and then behaves like what it is — a missile, a bottle of glass, or
//! a page of instructions for anything literate enough to follow them.

mod common;
#[path = "common/monster.rs"]
mod monster;

use bevy_ecs::prelude::*;
use models::constants::wands::{GRENADE_DIE_PER_CHARGE, GRENADE_RADIUS};
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

/// Spawn something and put it in the player's pack, as picking it up would.
fn stash(w: &mut World, p: Entity, spawn: impl FnOnce(&mut World) -> Entity) -> Entity {
    let item = spawn(w);
    w.entity_mut(item).remove::<Position>();
    w.get_mut::<Backpack>(p).unwrap().items.push(item);
    item
}

/// The tile an item is spawned on before it is stashed — never looked at.
const NOWHERE: Position = Position { x: 0, y: 0 };

/// The engine's "Throw" action: the item leaves the pack for good, and
/// `throw_system` finds out where it ends up. A quiver is the exception — it
/// gives up one arrow and stays in its slot — so this goes through `draw_one`
/// exactly as the engine does, and returns whatever actually took flight.
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

/// A plain monster dropped onto a tile next to the player so the throw has a
/// clear one-step flight.
fn monster(w: &mut World, _name: &str, at: Position) -> Entity {
    monster::plain_monster(w, "test monster", at)
}

/// An open tile `dx` to the east of the player, and the tile beyond it.
fn east_of_player(w: &mut World, dx: u16) -> Position {
    let p = player_pos(w);
    Position {
        x: p.x + dx,
        y: p.y,
    }
}

/// `Position` has no `Debug`, so compare tiles as plain pairs.
fn pos_of(w: &World, e: Entity) -> (u16, u16) {
    let p = w.get::<Position>(e).expect("entity has no position");
    (p.x, p.y)
}

fn logged(w: &World, needle: &str) -> bool {
    w.resource::<GameLog>()
        .history
        .iter()
        .any(|l| l.contains(needle))
}

#[test]
fn a_thrown_weapon_is_caught_and_wielded_by_a_creature_with_hands() {
    let mut w = test_world(7);
    let p = player(&mut w);
    let spot = east_of_player(&mut w, 1);
    let troll = monster::monster(&mut w, "test monster", spot);
    w.get_mut::<Fighter>(troll).unwrap().hp = 20;
    let mace = stash(&mut w, p, |w| spawn_weapon(w, "mace", NOWHERE));

    throw(&mut w, p, mace, spot);

    assert_eq!(w.get::<Equipped>(mace).unwrap().by, Some(troll));
    assert!(w.get::<Position>(mace).is_none());
    assert!(logged(&w, "wields it"));
}

#[test]
fn only_an_item_with_thrown_damage_hurts_what_it_hits() {
    let mut w = test_world(7);
    let p = player(&mut w);
    let spot = east_of_player(&mut w, 1);
    let bat = monster(&mut w, "bat", spot);
    w.get_mut::<Fighter>(bat).unwrap().hp = 20;

    // A suit of armour carries no `ThrownDamage`: it bounces off and lands.
    let mail = stash(&mut w, p, |w| spawn_armor(w, "leather armor", NOWHERE));
    throw(&mut w, p, mail, spot);

    assert_eq!(
        w.get::<Fighter>(bat).unwrap().hp,
        20,
        "armour is not a weapon"
    );
    assert_eq!(pos_of(&w, mail), (spot.x, spot.y));
    assert!(logged(&w, "bounces off"));

    // A dagger does carry one.
    let dagger = stash(&mut w, p, |w| spawn_weapon(w, "dagger", NOWHERE));
    assert_eq!(w.get::<ThrownDamage>(dagger), Some(&ThrownDamage(4)));
    throw(&mut w, p, dagger, spot);
    assert!(w.get::<Fighter>(bat).unwrap().hp < 20, "a dagger is");
}

#[test]
fn a_thrown_wand_of_fire_goes_off_like_a_grenade() {
    let mut w = test_world(13);
    let p = player(&mut w);
    let spot = east_of_player(&mut w, 1);
    let orc = monster(&mut w, "orc", spot);
    w.get_mut::<Fighter>(orc).unwrap().hp = 40;
    w.get_mut::<Fighter>(p).unwrap().hp = 99;
    let wand = stash(&mut w, p, |w| spawn_wand(w, WandEffect::Fire, NOWHERE));
    w.get_mut::<Battery>(wand).unwrap().charges = 5;

    throw(&mut w, p, wand, spot);

    assert!(
        w.get_entity(wand).is_none(),
        "the wand went up with the flame"
    );
    assert!(w.get::<Fighter>(orc).unwrap().hp < 40);
    assert!(logged(&w, "gets out at once"));
}

#[test]
fn the_grenade_is_wider_and_hotter_than_the_beam() {
    fn burn(seed: u64, throw_it: bool) -> (usize, i32) {
        let mut w = test_world(seed);
        let p = player(&mut w);
        let traps: Vec<Entity> = w.query_filtered::<Entity, With<Trap>>().iter(&w).collect();
        for t in traps {
            w.entity_mut(t).despawn();
        }
        let pp = player_pos(&mut w);
        let orcs: Vec<Entity> = (1..=7)
            .map(|dx| {
                let m = monster(
                    &mut w,
                    "orc",
                    Position {
                        x: pp.x + dx,
                        y: pp.y,
                    },
                );
                w.get_mut::<Fighter>(m).unwrap().hp = 200;
                m
            })
            .collect();
        let target = Position {
            x: pp.x + 1,
            y: pp.y,
        };
        let wand = stash(&mut w, p, |w| spawn_wand(w, WandEffect::Fire, NOWHERE));
        w.get_mut::<Battery>(wand).unwrap().charges = 5;

        if throw_it {
            throw(&mut w, p, wand, target);
        } else {
            let idx = w
                .get::<Backpack>(p)
                .unwrap()
                .items
                .iter()
                .position(|&e| e == wand);
            w.get_mut::<Backpack>(p)
                .unwrap()
                .items
                .retain(|&e| e != wand);
            w.resource_mut::<UseQueue>().uses.push(WantsToUse {
                user: p,
                item: wand,
                target: Some(target),
                slot_idx: idx,
            });
            item_system(&mut w);
        }

        let burned = orcs
            .iter()
            .filter(|&&o| w.get::<Fighter>(o).unwrap().hp < 200)
            .count();
        let worst = orcs
            .iter()
            .map(|&o| 200 - w.get::<Fighter>(o).unwrap().hp)
            .max()
            .unwrap();
        (burned, worst)
    }

    let (zapped_count, _) = burn(21, false);
    let (thrown_count, _) = burn(21, true);
    assert!(
        thrown_count > zapped_count,
        "the grenade should catch more than the beam ({thrown_count} vs {zapped_count})"
    );

    const CHARGES: i32 = 5;
    let rolls: Vec<i32> = (0..60).map(|seed| burn(seed, true).1).collect();
    let floor = CHARGES;
    let ceiling = CHARGES * GRENADE_DIE_PER_CHARGE;
    assert!(
        rolls.iter().all(|&d| (floor..=ceiling).contains(&d)),
        "a grenade rolled outside {floor}..={ceiling}: {rolls:?}"
    );
    let mean = |v: &[i32]| v.iter().sum::<i32>() as f64 / v.len() as f64;
    let beam = mean(&(0..60).map(|seed| burn(seed, false).1).collect::<Vec<_>>());
    let grenade = mean(&rolls);
    assert!(
        grenade > beam,
        "the grenade should hit harder than the beam (beam {beam:.1}, grenade {grenade:.1})"
    );
}

#[test]
fn a_wand_lobbed_into_open_floor_lands_a_dud() {
    let mut w = test_world(3);
    let p = player(&mut w);
    let spot = east_of_player(&mut w, 2);
    let wand = stash(&mut w, p, |w| spawn_wand(w, WandEffect::Fire, NOWHERE));
    w.get_mut::<Battery>(wand).unwrap().charges = 7;
    throw(&mut w, p, wand, spot);

    assert_eq!(pos_of(&w, wand), (spot.x, spot.y), "it just lands");
    assert_eq!(w.get::<Battery>(wand).unwrap().charges, 7, "charges intact");
    assert!(logged(&w, "still bottled up"));
}

#[test]
fn a_thrown_effect_wand_works_its_effect_on_everyone_in_the_blast() {
    let mut w = test_world(3);
    let p = player(&mut w);
    let spot = east_of_player(&mut w, 1);
    let orc = monster(&mut w, "orc", spot);
    assert_eq!(w.get::<Speed>(orc).unwrap().kind, SpeedKind::Normal);

    let wand = stash(&mut w, p, |w| {
        spawn_wand(w, WandEffect::HasteMonster, NOWHERE)
    });
    w.get_mut::<Battery>(wand).unwrap().charges = 6;
    throw(&mut w, p, wand, spot);

    assert!(w.get_entity(wand).is_none(), "the wand burst");
    assert_eq!(
        w.get::<Speed>(orc).unwrap().kind,
        SpeedKind::Fast,
        "the blast hasted the orc it caught"
    );
}

#[test]
fn a_thrown_wand_of_cancellation_devastates_the_player_it_catches() {
    let mut w = test_world(4);
    let p = player(&mut w);

    let blade = stash(&mut w, p, |w| spawn_weapon(w, "long sword", NOWHERE));
    w.entity_mut(blade).insert((PowerBonus(3), Curse));
    // A bow wears its plus on the throw, not on a melee roll — the third kind
    // of gear, and the one a "weapons and armour" reading of the rule misses.
    let bow = stash(&mut w, p, |w| spawn_launcher(w, "short bow", NOWHERE));
    w.entity_mut(bow).insert(ThrowBonus(3));
    let scroll = stash(&mut w, p, |w| {
        spawn_scroll(w, ScrollEffect::Teleportation, NOWHERE)
    });
    let potion = stash(&mut w, p, |w| {
        spawn_potion(w, PotionEffect::Poison, NOWHERE)
    });

    let spot = east_of_player(&mut w, 1);
    monster(&mut w, "bat", spot);
    let wand = stash(&mut w, p, |w| {
        spawn_wand(w, WandEffect::Cancellation, NOWHERE)
    });
    w.get_mut::<Battery>(wand).unwrap().charges = 4;
    throw(&mut w, p, wand, spot);

    assert_eq!(
        w.get::<PowerBonus>(blade).map(|b| b.0),
        Some(0),
        "the plus is wiped"
    );
    assert!(
        w.get::<Curse>(blade).is_none(),
        "but the curse lifts, blade intact"
    );
    assert_eq!(
        w.get::<ThrowBonus>(bow).map(|b| b.0),
        Some(0),
        "a bow's plus is an enchantment like any other, and goes the same way"
    );
    assert_eq!(
        w.get::<Scroll>(scroll).unwrap().effect,
        ScrollEffect::BlankPaper
    );
    assert_eq!(w.get::<Potion>(potion).unwrap().effect, PotionEffect::Water);
}

#[test]
fn cancellation_blanks_the_runes_the_player_carries() {
    let mut w = test_world(4);
    let p = player(&mut w);
    let rune = stash(&mut w, p, |w| spawn_rune(w, RuneEffect::Justice, NOWHERE));

    let spot = east_of_player(&mut w, 1);
    monster(&mut w, "bat", spot);
    let wand = stash(&mut w, p, |w| {
        spawn_wand(w, WandEffect::Cancellation, NOWHERE)
    });
    w.get_mut::<Battery>(wand).unwrap().charges = 4;
    throw(&mut w, p, wand, spot);

    let blanked = w.get::<Rune>(rune).unwrap();
    assert_eq!(blanked.effect, RuneEffect::Blank);
    assert!(!blanked.charged, "nothing left in it to wake");
}

#[test]
fn cancellation_clears_the_players_conditions() {
    let mut w = test_world(4);
    let p = player(&mut w);
    w.entity_mut(p).insert(Confused);
    w.get_mut::<Speed>(p).unwrap().kind = SpeedKind::Fast;

    let spot = east_of_player(&mut w, 1);
    monster(&mut w, "bat", spot);
    let wand = stash(&mut w, p, |w| {
        spawn_wand(w, WandEffect::Cancellation, NOWHERE)
    });
    w.get_mut::<Battery>(wand).unwrap().charges = 4;
    throw(&mut w, p, wand, spot);

    assert!(w.get::<Confused>(p).is_none(), "confusion lifts");
    assert_eq!(
        w.get::<Speed>(p).unwrap().kind,
        SpeedKind::Normal,
        "haste lifts"
    );
    assert!(logged(&w, "You are no longer confused."));
    assert!(logged(&w, "You are no longer hasted."));
}

#[test]
fn a_thrown_wand_of_light_dazzles_and_burns_everyone_in_the_blast() {
    let mut w = test_world(6);
    let p = player(&mut w);
    let spot = east_of_player(&mut w, 1);
    let orc = monster(&mut w, "orc", spot);
    w.get_mut::<Fighter>(orc).unwrap().hp = 40;

    let wand = stash(&mut w, p, |w| spawn_wand(w, WandEffect::Light, NOWHERE));
    w.get_mut::<Battery>(wand).unwrap().charges = 5;
    throw(&mut w, p, wand, spot);

    assert!(w.get_entity(wand).is_none(), "the wand burst");
    assert!(
        w.get::<Fighter>(orc).unwrap().hp < 40,
        "the flash still burns like an attack wand"
    );
    assert!(
        matches!(
            w.get::<Mob>(orc).unwrap().movement_type,
            MovementType::Confused
        ),
        "and it dazzles what it catches"
    );
    assert!(logged(&w, "dazzled"));
}

#[test]
fn a_thrown_utility_wand_blast_deals_no_damage() {
    let mut w = test_world(8);
    let p = player(&mut w);
    let spot = east_of_player(&mut w, 1);
    let orc = monster(&mut w, "orc", spot);
    w.get_mut::<Fighter>(orc).unwrap().hp = 30;

    let wand = stash(&mut w, p, |w| {
        spawn_wand(w, WandEffect::SlowMonster, NOWHERE)
    });
    w.get_mut::<Battery>(wand).unwrap().charges = 9;
    throw(&mut w, p, wand, spot);

    assert_eq!(
        w.get::<Fighter>(orc).unwrap().hp,
        30,
        "the payload is the effect, not damage"
    );
    assert_eq!(w.get::<Speed>(orc).unwrap().kind, SpeedKind::Slow);
}

#[test]
fn a_thrown_wand_of_teleport_to_with_no_target_sends_a_victim_to_itself() {
    let mut w = test_world(5);
    let p = player(&mut w);
    let spot = east_of_player(&mut w, 1);
    let orc = monster(&mut w, "orc", spot);
    let before = pos_of(&w, orc);

    let wand = stash(&mut w, p, |w| {
        spawn_wand(w, WandEffect::TeleportTo, NOWHERE)
    });
    w.get_mut::<Battery>(wand).unwrap().charges = 5;
    throw(&mut w, p, wand, spot);

    assert_eq!(pos_of(&w, orc), before, "it arrives exactly where it stood");
    assert!(logged(&w, "teleports directly to themselves"));
}

#[test]
fn a_thrown_wand_of_swapping_shuffles_everyone_it_catches() {
    let mut w = test_world(3);
    let p = player(&mut w);
    let near = east_of_player(&mut w, 1);
    let far = east_of_player(&mut w, 2);
    let a = monster(&mut w, "orc", near);
    let b = monster(&mut w, "orc", far);
    let before = [p, a, b].map(|e| pos_of(&w, e));

    let wand = stash(&mut w, p, |w| spawn_wand(w, WandEffect::Swapping, NOWHERE));
    w.get_mut::<Battery>(wand).unwrap().charges = 5;
    throw(&mut w, p, wand, near);

    let after = [p, a, b].map(|e| pos_of(&w, e));
    for (was, now) in before.iter().zip(&after) {
        assert_ne!(was, now, "nobody keeps their own tile");
    }
    let (mut was, mut now) = (before, after);
    was.sort();
    now.sort();
    assert_eq!(was, now, "and every tile still holds one of them");
}

#[test]
fn a_creature_without_hands_just_takes_the_hit_and_the_weapon_falls() {
    let mut w = test_world(7);
    let p = player(&mut w);
    let spot = east_of_player(&mut w, 1);
    let bat = monster(&mut w, "bat", spot);
    w.get_mut::<Fighter>(bat).unwrap().hp = 20;
    let mace = stash(&mut w, p, |w| spawn_weapon(w, "mace", NOWHERE));

    throw(&mut w, p, mace, spot);

    assert!(w.get::<Equipped>(mace).unwrap().by.is_none());
    assert_eq!(pos_of(&w, mace), (spot.x, spot.y));
}

#[test]
fn caught_gear_arms_the_monster_that_caught_it() {
    let mut w = test_world(3);
    let p = player(&mut w);
    let spot = east_of_player(&mut w, 1);
    let hobgoblin = monster::monster(&mut w, "test monster", spot);
    w.get_mut::<Fighter>(hobgoblin).unwrap().hp = 30;
    let mail = stash(&mut w, p, |w| spawn_armor(w, "plate mail", NOWHERE));
    let mail_armor = w.get::<ArmorDie>(mail).unwrap().0;

    throw(&mut w, p, mail, spot);

    assert_eq!(equipped_total::<ArmorDie>(&w, hobgoblin), mail_armor);
}

#[test]
fn a_thrown_weapon_hurts_what_it_hits() {
    let mut w = test_world(11);
    let p = player(&mut w);
    let spot = east_of_player(&mut w, 1);
    let bat = monster(&mut w, "bat", spot);
    w.get_mut::<Fighter>(bat).unwrap().hp = 20;
    let sword = stash(&mut w, p, |w| spawn_weapon(w, "two-handed sword", NOWHERE));

    throw(&mut w, p, sword, spot);

    assert!(
        w.get::<Fighter>(bat).unwrap().hp < 20,
        "the sword should have drawn blood"
    );
}

#[test]
fn a_thrown_potion_is_drunk_by_its_target_and_names_itself_when_it_works() {
    let mut w = test_world(5);
    let p = player(&mut w);
    let spot = east_of_player(&mut w, 1);
    let orc = monster::plain_monster(&mut w, "test monster", spot);
    {
        let mut f = w.get_mut::<Fighter>(orc).unwrap();
        f.max_hp = 10;
        f.hp = 1;
    }
    let old_max_hp = w.get::<Fighter>(orc).unwrap().max_hp;
    let potion = stash(&mut w, p, |w| {
        spawn_potion(w, PotionEffect::Healing, NOWHERE)
    });

    throw(&mut w, p, potion, spot);

    let healed = w.get::<Fighter>(orc).unwrap();
    assert_eq!(healed.hp, healed.max_hp);
    assert!(
        healed.max_hp > old_max_hp,
        "healing raised the target's ceiling"
    );
    assert!(w.get_entity(potion).is_none(), "the bottle broke");
}

#[test]
fn a_potion_that_does_nothing_visible_still_lands_on_its_target() {
    let mut w = test_world(5);
    let p = player(&mut w);
    let spot = east_of_player(&mut w, 1);
    let orc = monster::plain_monster(&mut w, "test monster", spot);
    let hp = w.get::<Fighter>(orc).unwrap().max_hp;
    w.get_mut::<Fighter>(orc).unwrap().hp = hp;
    let potion = stash(&mut w, p, |w| {
        spawn_potion(w, PotionEffect::RestoreStrength, NOWHERE)
    });

    throw(&mut w, p, potion, spot);

    assert!(w.get_entity(potion).is_none(), "the bottle broke anyway");
}

#[test]
fn only_a_creature_that_understands_items_reads_a_thrown_scroll() {
    let mut w = test_world(9);
    let p = player(&mut w);
    let spot = east_of_player(&mut w, 1);
    let bat = monster(&mut w, "bat", spot);
    w.get_mut::<Fighter>(bat).unwrap().hp = 20;
    let scroll = stash(&mut w, p, |w| {
        spawn_scroll(w, ScrollEffect::FoodDetection, NOWHERE)
    });

    throw(&mut w, p, scroll, spot);

    assert_eq!(pos_of(&w, scroll), (spot.x, spot.y));

    let scroll = stash(&mut w, p, |w| {
        spawn_scroll(w, ScrollEffect::FoodDetection, NOWHERE)
    });
    w.despawn(bat);
    let orc = monster::monster(&mut w, "test monster", spot);
    w.get_mut::<Fighter>(orc).unwrap().hp = 20;

    throw(&mut w, p, scroll, spot);

    assert!(
        w.get_entity(scroll).is_none(),
        "the scroll crumbled as it was read"
    );
    assert!(logged(&w, "reads it aloud"));
}

#[test]
fn the_item_users_are_the_humanoids_with_wits() {
    let mut w = test_world(1);
    let users: Vec<&str> = BESTIARY
        .iter()
        .filter(|def| def.spirit_kind.is_none())
        .filter(|def| {
            let m = spawn_monster(&mut w, def, Position { x: 1, y: 1 });
            let uses_items = w.get::<ItemUser>(m).is_some();
            w.despawn(m);
            uses_items
        })
        .map(|def| def.name)
        .collect();
    assert_eq!(
        users,
        vec![
            "centaur",
            "ichthyocentaur",
            "hobgoblin",
            "leprechaun",
            "medusa",
            "nymph",
            "orc",
            "troll",
            "ur-vile",
            "vampire",
            "wraith",
        ]
    );
}

#[test]
fn a_throw_with_nothing_in_the_way_lands_on_the_aimed_tile() {
    let mut w = test_world(2);
    let p = player(&mut w);
    let spot = east_of_player(&mut w, 1);
    let dagger = stash(&mut w, p, |w| spawn_weapon(w, "dagger", NOWHERE));

    throw(&mut w, p, dagger, spot);

    assert_eq!(pos_of(&w, dagger), (spot.x, spot.y));
    assert!(!w.get::<Backpack>(p).unwrap().items.contains(&dagger));
}

#[test]
fn cursed_gear_you_are_wearing_cannot_be_thrown_or_dropped() {
    let mut w = test_world(4);
    let p = player(&mut w);
    let sword = stash(&mut w, p, |w| spawn_weapon(w, "long sword", NOWHERE));
    w.entity_mut(sword).insert(Curse);
    toggle_equipped(&mut w, p, sword);

    assert!(throw_refusal(&w, p, sword).is_some());
    assert!(drop_refusal(&w, p, sword).is_some());

    force_unequip(&mut w, sword);
    assert!(throw_refusal(&w, p, sword).is_none());
}

#[test]
fn the_element_of_yoord_never_leaves_your_hand() {
    let mut w = test_world(4);
    let p = player(&mut w);
    let element = stash(&mut w, p, |w| spawn_element_of_yoord(w, NOWHERE));

    assert!(throw_refusal(&w, p, element).is_some());
    assert!(drop_refusal(&w, p, element).is_some());
}

#[test]
fn throwing_equipped_gear_takes_it_off_first() {
    let mut w = test_world(6);
    let p = player(&mut w);
    let spot = east_of_player(&mut w, 1);
    let sword = stash(&mut w, p, |w| spawn_weapon(w, "long sword", NOWHERE));
    toggle_equipped(&mut w, p, sword);
    assert_eq!(equipped_total::<PowerDie>(&w, p), 8);

    throw(&mut w, p, sword, spot);

    assert_eq!(
        equipped_total::<PowerDie>(&w, p),
        0,
        "the sword is not in your hand any more"
    );
}

/// Arm a troll with a thrown mace, kill it, and report whether the mace
/// survived the death roll.
fn kill_an_armed_troll(seed: u64) -> bool {
    let mut w = test_world(seed);
    let p = player(&mut w);
    let spot = east_of_player(&mut w, 1);
    let troll = monster::monster(&mut w, "test monster", spot);
    w.get_mut::<Fighter>(troll).unwrap().hp = 20;
    let mace = stash(&mut w, p, |w| spawn_weapon(w, "mace", NOWHERE));
    throw(&mut w, p, mace, spot);
    assert_eq!(w.get::<Equipped>(mace).unwrap().by, Some(troll));

    w.get_mut::<Fighter>(troll).unwrap().hp = 0;
    reaper_system(&mut w);
    assert!(w.get_entity(troll).is_none());

    match w.get_entity(mace) {
        Some(_) => {
            assert_eq!(pos_of(&w, mace), (spot.x, spot.y));
            assert!(w.get::<Equipped>(mace).unwrap().by.is_none());
            assert!(logged(&w, "clatters to the floor"));
            true
        }
        None => {
            assert!(!logged(&w, "clatters to the floor"));
            false
        }
    }
}

#[test]
fn a_slain_catcher_leaves_its_gear_or_takes_it_with_it() {
    let survivals = (0..40).filter(|&seed| kill_an_armed_troll(seed)).count();
    assert!(survivals > 0, "no gear ever survived a death");
    assert!(survivals < 40, "gear always survived a death");
}

#[test]
fn throw_is_the_middle_row_of_the_browse_modal_and_has_a_key_of_its_own() {
    assert_eq!(
        ItemAction::MENU,
        [ItemAction::Use, ItemAction::Throw, ItemAction::Drop]
    );
    assert_eq!(ItemAction::at(1), ItemAction::Throw);
    assert_eq!(PackMode::Throw.action(), Some(ItemAction::Throw));
}

/// A monster holds what it caught across a save. The file used to record a
/// slot, never a wearer, so a monster had to put its gear down before the save
/// — an item in flight's last position with no tile of its own would otherwise
/// come back adrift. Now the wearer is in the file as an index remapped on
/// load, the save never touches the live world to write itself, and the mace
/// comes back in the same hand.
#[test]
fn a_monster_keeps_what_it_is_holding_across_a_save() {
    let mut w = test_world(12);
    let p = player(&mut w);
    let spot = east_of_player(&mut w, 1);
    let orc = monster::monster(&mut w, "test monster", spot);
    w.get_mut::<Fighter>(orc).unwrap().hp = 20;
    let mace = stash(&mut w, p, |w| spawn_weapon(w, "mace", NOWHERE));
    throw(&mut w, p, mace, spot);
    assert_eq!(w.get::<Equipped>(mace).unwrap().by, Some(orc));

    let save = common::SaveFile::new("throw-save");
    save_game(&mut w, save.path()).unwrap();

    assert_eq!(w.get::<Equipped>(mace).unwrap().by, Some(orc));

    let mut loaded = test_world(12);
    load_game(&mut loaded, save.path()).unwrap();
    let orc2 = mob_at(&mut loaded, spot).expect("the orc is where it was");
    let held =
        equipped_in(&loaded, orc2, Slot::Hand).expect("the mace came back in the orc's hand");
    assert_eq!(
        loaded.get::<Name>(held).map(|n| n.what.as_str()),
        Some("mace")
    );
}

#[test]
fn a_weapon_keeps_its_thrown_damage_across_a_save() {
    let mut w = test_world(2);
    let p = player(&mut w);
    let sword = stash(&mut w, p, |w| spawn_weapon(w, "long sword", NOWHERE));
    assert_eq!(w.get::<ThrownDamage>(sword), Some(&ThrownDamage(8)));

    let save = common::SaveFile::new("thrown-damage");
    save_game(&mut w, save.path()).unwrap();
    let mut loaded = test_world(2);
    load_game(&mut loaded, save.path()).unwrap();

    let dice: Vec<i32> = loaded
        .iter_entities()
        .filter(|e| e.get::<Name>().is_some_and(|n| n.what == "long sword"))
        .filter_map(|e| e.get::<ThrownDamage>().map(|t| t.0))
        .collect();
    assert_eq!(dice, vec![8]);
}

/// A shot that comes down on a trap the player has found sets it off — and a
/// trap nobody is standing on bursts instead of biting. The dagger never
/// touched the orc beside the trap; the trap did.
#[test]
fn a_shot_that_lands_on_a_trap_sets_it_off() {
    let mut w = test_world(7);
    let p = player(&mut w);
    let spot = east_of_player(&mut w, 2);
    assert!(!w.resource::<Map>().blocks(spot.x, spot.y), "open floor");

    let trap = w.spawn(TrapBundle::sleep(spot)).id();
    w.entity_mut(trap).remove::<Hidden>();
    let beside_it = east_of_player(&mut w, 3);
    let bystander = monster(&mut w, "orc", beside_it);
    w.get_mut::<Fighter>(bystander).unwrap().hp = 30;
    let dagger = stash(&mut w, p, |w| spawn_weapon(w, "dagger", NOWHERE));

    throw(&mut w, p, dagger, spot);

    assert!(
        w.get_entity(trap).is_none(),
        "the trap went off with the shot"
    );
    assert!(
        w.get::<Fighter>(bystander).unwrap().hp < 30,
        "the burst caught the orc standing next to the trap"
    );
    assert_eq!(
        w.get::<Asleep>(bystander).is_some(),
        true,
        "and so did the gas the trap was holding"
    );
}

/// The same shot at a trap nobody has found: the dagger lands, and that is all
/// it does. You cannot line one of these up on a mechanism you have not seen.
#[test]
fn a_shot_that_lands_on_a_hidden_trap_just_lands() {
    let mut w = test_world(7);
    let p = player(&mut w);
    let spot = east_of_player(&mut w, 2);
    let trap = w.spawn(TrapBundle::sleep(spot)).id();
    let beside_it = east_of_player(&mut w, 3);
    let bystander = monster(&mut w, "orc", beside_it);
    w.get_mut::<Fighter>(bystander).unwrap().hp = 30;
    let dagger = stash(&mut w, p, |w| spawn_weapon(w, "dagger", NOWHERE));

    throw(&mut w, p, dagger, spot);

    assert!(w.get_entity(trap).is_some(), "still armed, still hidden");
    assert!(w.get::<Hidden>(trap).is_some());
    assert_eq!(w.get::<Fighter>(bystander).unwrap().hp, 30);
}

/// Shooting the monster *is* shooting the tile it is standing on. A missile
/// that stops on a creature stops on whatever is under it, so a monster caught
/// standing over a found trap takes the trick shot with the blow.
#[test]
fn hitting_a_monster_standing_on_a_trap_sets_the_trap_off() {
    let mut w = test_world(7);
    let p = player(&mut w);
    let spot = east_of_player(&mut w, 1);
    let trap = w.spawn(TrapBundle::sleep(spot)).id();
    w.entity_mut(trap).remove::<Hidden>();
    let orc = monster(&mut w, "orc", spot);
    w.get_mut::<Fighter>(orc).unwrap().hp = 30;
    let dagger = stash(&mut w, p, |w| spawn_weapon(w, "dagger", NOWHERE));

    throw(&mut w, p, dagger, spot);

    assert!(w.get_entity(trap).is_none(), "the blow set it off");
    assert_eq!(
        w.get::<Asleep>(orc).is_some(),
        true,
        "and the gas it was holding went into the one standing on it"
    );
}

/// A thrown potion doesn't just dose whoever it hits: it bursts where it
/// lands and spreads over the same small splash a thrown utility wand's
/// blast covers, just narrower.
#[test]
fn a_thrown_potion_splashes_everyone_in_the_small_burst() {
    let mut w = test_world(5);
    let p = player(&mut w);
    let spot = east_of_player(&mut w, 1);
    let struck = monster(&mut w, "test monster", spot);
    let beside = monster(
        &mut w,
        "test monster",
        Position {
            x: spot.x + 1,
            y: spot.y,
        },
    );
    for orc in [struck, beside] {
        let mut f = w.get_mut::<Fighter>(orc).unwrap();
        f.max_hp = 10;
        f.hp = 1;
    }
    let potion = stash(&mut w, p, |w| {
        spawn_potion(w, PotionEffect::Healing, NOWHERE)
    });

    throw(&mut w, p, potion, spot);

    for orc in [struck, beside] {
        let f = w.get::<Fighter>(orc).unwrap();
        assert_eq!(f.hp, f.max_hp, "everyone in the splash got a share");
    }
    assert!(w.get_entity(potion).is_none(), "the bottle broke");
}

/// A potion is one of the things a blast can find lying underfoot: caught in
/// somebody else's burst it goes off just the same as a trap or a coin does.
#[test]
fn a_potion_caught_in_a_wands_grenade_goes_off_too() {
    let mut w = test_world(5);
    let p = player(&mut w);
    let spot = east_of_player(&mut w, 1);
    let orc = monster(&mut w, "test monster", spot);
    w.get_mut::<Fighter>(orc).unwrap().power -= 2;
    let starved_power = w.get::<Fighter>(orc).unwrap().power;
    let potion = spawn_potion(&mut w, PotionEffect::RestoreStrength, spot);

    let wand = stash(&mut w, p, |w| spawn_wand(w, WandEffect::Fire, NOWHERE));
    w.get_mut::<Battery>(wand).unwrap().charges = 4;
    throw(&mut w, p, wand, spot);

    assert!(w.get_entity(potion).is_none(), "the blast broke it");
    assert!(
        w.get::<Fighter>(orc).unwrap().power > starved_power,
        "and its effect landed on whoever the blast caught"
    );
}

/// A chaser three tiles east of a player who stands on a doorway, and where
/// the player stands. `moved` is whether the player's last turn was a step.
fn doorway_standoff(w: &mut World, moved: bool) -> (Entity, Position, Position) {
    let p = player(w);
    let here = player_pos(w);
    for dx in 1..=3 {
        w.resource_mut::<Map>().tiles[tile_index(here.x + dx, here.y)] = TileType::Room;
    }
    w.resource_mut::<Map>().tiles[tile_index(here.x, here.y)] = TileType::Door;
    let perch = east_of_player(w, 3);
    let chaser = monster(w, "chaser", perch);
    w.get_mut::<Mob>(chaser).unwrap().movement_type = MovementType::Chase;
    if moved {
        w.entity_mut(p).insert(EntityMoved);
    }
    let mut s = Schedule::default();
    s.add_systems(visibility_system);
    s.run(w);
    (chaser, here, perch)
}

/// Step onto a doorway and everything that can see you stands still.
#[test]
fn a_creature_that_sees_you_step_onto_a_doorway_does_nothing() {
    let mut w = test_world(11);
    let (chaser, _, perch) = doorway_standoff(&mut w, true);

    ai(&mut w);

    assert_eq!(pos_of(&w, chaser), (perch.x, perch.y), "it held its ground");
}

/// Do anything but move while you stand there and the ward breaks for good.
#[test]
fn acting_on_a_doorway_breaks_the_ward_and_greys_the_door() {
    let mut w = test_world(11);
    let (chaser, here, perch) = doorway_standoff(&mut w, false);

    ai(&mut w);

    assert!(w.resource::<Map>().is_inert_door(here.x, here.y));
    assert_ne!(pos_of(&w, chaser), (perch.x, perch.y), "it came for you");

    let p = player(&mut w);
    w.entity_mut(p).insert(EntityMoved);
    let before = pos_of(&w, chaser);
    ai(&mut w);
    assert_ne!(pos_of(&w, chaser), before, "an inert doorway is no ward");
}

/// A wand of digging that bursts on impact leaves a crater: every wall inside
/// the grenade's radius goes, in line of sight or not, and none outside it.
#[test]
fn a_thrown_wand_of_digging_makes_a_crater() {
    let mut w = test_world(5);
    let p = player(&mut w);
    {
        let mut map = w.resource_mut::<Map>();
        map.tiles.fill(TileType::Wall);
        for x in 20..=22 {
            map.tiles[tile_index(x, 10)] = TileType::Room;
        }
    }
    w.get_mut::<Position>(p).unwrap().x = 20;
    w.get_mut::<Position>(p).unwrap().y = 10;
    let wand = stash(&mut w, p, |w| spawn_wand(w, WandEffect::Digging, NOWHERE));
    throw(&mut w, p, wand, Position { x: 23, y: 10 });

    let r = GRENADE_RADIUS;
    let map = w.resource::<Map>();
    for y in 1..MAP_HEIGHT - 1 {
        for x in 1..MAP_WIDTH - 1 {
            let (dx, dy) = (x as f32 - 22.0, y as f32 - 10.0);
            let inside = (dx * dx + dy * dy).sqrt() <= r;
            let was_rock = !(20..=22).contains(&x) || y != 10;
            if was_rock {
                assert_eq!(
                    map.tile(x, y) != TileType::Wall,
                    inside,
                    "({x},{y}) {}",
                    if inside {
                        "survived the crater"
                    } else {
                        "was dug outside it"
                    }
                );
            }
        }
    }
}
