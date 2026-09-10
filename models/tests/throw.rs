//! Throwing: the third thing the pack screen can do with an item. A hurled
//! object flies to the spot you picked, stops at the first wall or creature in
//! the way, and then behaves like what it is — a missile, a bottle of glass, or
//! a page of instructions for anything literate enough to follow them.

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

/// A monster of the given species, dropped onto a tile next to the player so the
/// throw has a clear one-step flight.
fn monster(w: &mut World, species: &str, at: Position) -> Entity {
    spawn_monster(w, MonsterDef::named(species), at)
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
    let orc = monster(&mut w, "orc", spot);
    // Enough HP that the mace can't kill it before it can catch it.
    w.get_mut::<Fighter>(orc).unwrap().hp = 20;
    let mace = stash(&mut w, p, |w| spawn_weapon(w, "mace", NOWHERE));

    throw(&mut w, p, mace, spot);

    assert_eq!(w.get::<Equipped>(mace).unwrap().by, Some(orc));
    // Caught, not dropped: it is nobody's floor item now.
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
    // Well out of the blast, so this measures the grenade and not a suicide.
    w.get_mut::<Fighter>(p).unwrap().hp = 99;
    let wand = stash(&mut w, p, |w| spawn_wand(w, WandEffect::Fire, NOWHERE));
    w.get_mut::<Battery>(wand).unwrap().charges = 5;

    throw(&mut w, p, wand, spot);

    assert!(
        w.get_entity(wand).is_none(),
        "the wand went up with the flame"
    );
    assert!(w.get::<Fighter>(orc).unwrap().hp < 40);
    // Unmistakable: you know exactly what that was.
    assert!(w.resource::<Identified>().wands.contains(&WandEffect::Fire));
    assert!(logged(&w, "gets out at once"));
}

#[test]
fn the_grenade_is_wider_and_hotter_than_the_beam() {
    // Line orcs up east of the player, one per tile, and compare who burns when
    // the wand is zapped against who burns when it is thrown.
    fn burn(seed: u64, throw_it: bool) -> (usize, i32) {
        let mut w = test_world(seed);
        let p = player(&mut w);
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

    // A thrown wand spends every charge: 5 charges is 5d4 against the beam's
    // 3d3. Every roll inside the honest range, and about twice the beam on
    // average.
    let rolls: Vec<i32> = (0..60).map(|seed| burn(seed, true).1).collect();
    assert!(
        rolls.iter().all(|&d| (5..=20).contains(&d)),
        "a 5d4 grenade rolled outside 5..=20: {rolls:?}"
    );
    let mean = |v: &[i32]| v.iter().sum::<i32>() as f64 / v.len() as f64;
    let beam = mean(&(0..60).map(|seed| burn(seed, false).1).collect::<Vec<_>>());
    let grenade = mean(&rolls);
    assert!(
        grenade > beam * 1.6 && grenade < beam * 2.4,
        "the grenade should average about twice the beam (beam {beam:.1}, grenade {grenade:.1})"
    );
}

#[test]
fn a_wand_lobbed_into_open_floor_lands_a_dud() {
    let mut w = test_world(3);
    let p = player(&mut w);
    // Two tiles east, nothing in the way, well inside the throw leash.
    let spot = east_of_player(&mut w, 2);
    let wand = stash(&mut w, p, |w| spawn_wand(w, WandEffect::Fire, NOWHERE));
    w.get_mut::<Battery>(wand).unwrap().charges = 7;
    throw(&mut w, p, wand, spot);

    assert_eq!(pos_of(&w, wand), (spot.x, spot.y), "it just lands");
    assert_eq!(w.get::<Battery>(wand).unwrap().charges, 7, "charges intact");
    assert!(
        !w.resource::<Identified>().wands.contains(&WandEffect::Fire),
        "secret intact"
    );
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
    let scroll = stash(&mut w, p, |w| {
        spawn_scroll(w, ScrollEffect::Teleportation, NOWHERE)
    });
    let potion = stash(&mut w, p, |w| {
        spawn_potion(w, PotionEffect::Poison, NOWHERE)
    });

    // Lobbed at a rat one tile away — the wand bursts on it, and the blast disc
    // washes back over the thrower.
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
        w.get::<Scroll>(scroll).unwrap().effect,
        ScrollEffect::BlankPaper
    );
    assert_eq!(w.get::<Potion>(potion).unwrap().effect, PotionEffect::Water);
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
    let hobgoblin = monster(&mut w, "hobgoblin", spot);
    w.get_mut::<Fighter>(hobgoblin).unwrap().hp = 30;
    let mail = stash(&mut w, p, |w| spawn_armor(w, "plate mail", NOWHERE));

    throw(&mut w, p, mail, spot);

    // The armour die it just pulled on now counts towards its defence, exactly
    // as it would for the player.
    assert_eq!(equipped_total::<ArmorDie>(&w, hobgoblin), 9);
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
    let orc = monster(&mut w, "orc", spot);
    {
        let mut f = w.get_mut::<Fighter>(orc).unwrap();
        f.max_hp = 10;
        f.hp = 1;
    }
    let potion = stash(&mut w, p, |w| {
        spawn_potion(w, PotionEffect::Healing, NOWHERE)
    });

    throw(&mut w, p, potion, spot);

    assert_eq!(
        w.get::<Fighter>(orc).unwrap().hp,
        10,
        "the orc drank the healing and went to full HP"
    );
    assert!(w.get_entity(potion).is_none(), "the bottle broke");
    assert!(
        w.resource::<Identified>()
            .potions
            .contains(&PotionEffect::Healing)
    );
}

#[test]
fn a_potion_that_does_nothing_visible_keeps_its_secret() {
    let mut w = test_world(5);
    let p = player(&mut w);
    let spot = east_of_player(&mut w, 1);
    let orc = monster(&mut w, "orc", spot);
    // Strength was never drained: restore strength has nothing to show for itself.
    let hp = w.get::<Fighter>(orc).unwrap().max_hp;
    w.get_mut::<Fighter>(orc).unwrap().hp = hp;
    let potion = stash(&mut w, p, |w| {
        spawn_potion(w, PotionEffect::RestoreStrength, NOWHERE)
    });

    throw(&mut w, p, potion, spot);

    assert!(
        !w.resource::<Identified>()
            .potions
            .contains(&PotionEffect::RestoreStrength)
    );
}

#[test]
fn only_a_creature_that_understands_items_reads_a_thrown_scroll() {
    // The orc reads it — anything with the wits to swing a sword can follow a
    // page. The bat cannot.
    let mut w = test_world(9);
    let p = player(&mut w);
    let spot = east_of_player(&mut w, 1);
    let bat = monster(&mut w, "bat", spot);
    w.get_mut::<Fighter>(bat).unwrap().hp = 20;
    let scroll = stash(&mut w, p, |w| {
        spawn_scroll(w, ScrollEffect::BlankPaper, NOWHERE)
    });

    throw(&mut w, p, scroll, spot);

    // Bounced off and landed, still a mystery.
    assert_eq!(pos_of(&w, scroll), (spot.x, spot.y));
    assert!(
        !w.resource::<Identified>()
            .scrolls
            .contains(&ScrollEffect::BlankPaper)
    );

    let scroll = stash(&mut w, p, |w| {
        spawn_scroll(w, ScrollEffect::BlankPaper, NOWHERE)
    });
    w.despawn(bat);
    let orc = monster(&mut w, "orc", spot);
    w.get_mut::<Fighter>(orc).unwrap().hp = 20;

    throw(&mut w, p, scroll, spot);

    assert!(
        w.get_entity(scroll).is_none(),
        "the scroll crumbled as it was read"
    );
    assert!(logged(&w, "reads it aloud"));
    assert!(
        w.resource::<Identified>()
            .scrolls
            .contains(&ScrollEffect::BlankPaper)
    );
}

#[test]
fn the_item_users_are_the_humanoids_with_wits() {
    // Wielding and reading are one flag, so this roster is both lists at once —
    // the mindless humanoids (zombie, wraith, phantom) are deliberately absent.
    let mut w = test_world(1);
    let users: Vec<&str> = BESTIARY
        .iter()
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
            "goblin",
            "centaur",
            "hobgoblin",
            "leprechaun",
            "medusa",
            "nymph",
            "orc",
            "troll",
            "ur-vile",
            "vampire",
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

    // The same sword loose in the pack is throwable.
    force_unequip(&mut w, sword);
    assert!(throw_refusal(&w, p, sword).is_none());
}

#[test]
fn the_element_of_yoord_never_leaves_your_hand() {
    let mut w = test_world(4);
    let p = player(&mut w);
    let element = stash(&mut w, p, |w| spawn_element_of_yoord(w, NOWHERE));

    assert!(throw_refusal(&w, p, element).is_some());
    // It can still be set down, though — that much is the player's business.
    assert!(drop_refusal(&w, p, element).is_none());
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

/// Arm a goblin with a thrown mace, kill it, and report whether the mace
/// survived the death roll.
fn kill_an_armed_goblin(seed: u64) -> bool {
    let mut w = test_world(seed);
    let p = player(&mut w);
    let spot = east_of_player(&mut w, 1);
    let goblin = monster(&mut w, "goblin", spot);
    w.get_mut::<Fighter>(goblin).unwrap().hp = 20;
    let mace = stash(&mut w, p, |w| spawn_weapon(w, "mace", NOWHERE));
    throw(&mut w, p, mace, spot);
    assert_eq!(w.get::<Equipped>(mace).unwrap().by, Some(goblin));

    w.get_mut::<Fighter>(goblin).unwrap().hp = 0;
    reaper_system(&mut w);
    assert!(w.get_entity(goblin).is_none());

    match w.get_entity(mace) {
        // Survived: on the floor where it fell, owned by nobody, announced.
        Some(_) => {
            assert_eq!(pos_of(&w, mace), (spot.x, spot.y));
            assert!(w.get::<Equipped>(mace).unwrap().by.is_none());
            assert!(logged(&w, "clatters to the floor"));
            true
        }
        // Destroyed with its owner, and never mentioned.
        None => {
            assert!(!logged(&w, "clatters to the floor"));
            false
        }
    }
}

#[test]
fn a_slain_catcher_leaves_its_gear_or_takes_it_with_it() {
    let survivals = (0..40).filter(|&seed| kill_an_armed_goblin(seed)).count();
    // A coin flip per item: over forty deaths, both outcomes have to show up.
    assert!(survivals > 0, "no gear ever survived a death");
    assert!(survivals < 40, "gear always survived a death");
}

#[test]
fn the_action_menu_order_flips_with_dropthrow() {
    let default = ActionMenu { drop_first: false };
    assert_eq!(
        default.actions(),
        [ItemAction::Use, ItemAction::Throw, ItemAction::Drop]
    );

    let swapped = ActionMenu { drop_first: true };
    assert_eq!(
        swapped.actions(),
        [ItemAction::Use, ItemAction::Drop, ItemAction::Throw]
    );
}

#[test]
fn a_monster_lays_down_what_it_is_holding_before_a_save() {
    let mut w = test_world(12);
    let p = player(&mut w);
    let spot = east_of_player(&mut w, 1);
    let orc = monster(&mut w, "orc", spot);
    w.get_mut::<Fighter>(orc).unwrap().hp = 20;
    let mace = stash(&mut w, p, |w| spawn_weapon(w, "mace", NOWHERE));
    throw(&mut w, p, mace, spot);
    assert_eq!(w.get::<Equipped>(mace).unwrap().by, Some(orc));

    // A save records a slot, never a wearer — so the orc puts the mace down on
    // its own tile first, rather than leaving it adrift with no owner and no
    // square to be found on.
    let path = std::env::temp_dir().join("roog-throw-save.sav");
    save_game(&mut w, path.to_str().unwrap()).unwrap();
    let _ = std::fs::remove_file(&path);

    assert!(w.get::<Equipped>(mace).unwrap().by.is_none());
    assert_eq!(pos_of(&w, mace), (spot.x, spot.y));
}

#[test]
fn a_weapon_keeps_its_thrown_damage_across_a_save() {
    // Like a ring's grants, what a weapon does when hurled is read back from the
    // catalog rather than stored in every save file.
    let mut w = test_world(2);
    let p = player(&mut w);
    let sword = stash(&mut w, p, |w| spawn_weapon(w, "long sword", NOWHERE));
    assert_eq!(w.get::<ThrownDamage>(sword), Some(&ThrownDamage(8)));

    let path = std::env::temp_dir().join("roog-thrown-damage.sav");
    save_game(&mut w, path.to_str().unwrap()).unwrap();
    let mut loaded = test_world(2);
    load_game(&mut loaded, path.to_str().unwrap()).unwrap();
    let _ = std::fs::remove_file(&path);

    let dice: Vec<i32> = loaded
        .iter_entities()
        .filter(|e| e.get::<Name>().is_some_and(|n| n.what == "long sword"))
        .filter_map(|e| e.get::<ThrownDamage>().map(|t| t.0))
        .collect();
    assert_eq!(dice, vec![8]);
}
