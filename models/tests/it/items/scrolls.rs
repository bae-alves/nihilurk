//! The scroll table: teleportation, aggravate monsters, scare monster, create
//! monster and vorpalize weapon (plus the rule that a glancing blow can never
//! be the killing one), then the two enchantments, monster confusion, hold
//! monster, sleep and food detection, and the three rare ones: recharging,
//! protection and justice.

#[allow(dead_code)] // only the handless fixture is wanted here
use crate::common::monster;

use crate::common::fixtures::{scroll_row, spawn_launcher, spawn_named, spawn_weapon};
use bevy_ecs::prelude::*;
use bevy_ecs::schedule::Schedule;
use models::*;

const NOWHERE: Position = Position { x: 0, y: 0 };

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

/// Queue `item` at its pack row, run the item system — the engine's
/// "Use" action.
fn use_item(w: &mut World, user: Entity, item: Entity) {
    let idx = w
        .get_mut::<Backpack>(user)
        .unwrap()
        .items
        .iter()
        .position(|&e| e == item);
    if let Some(i) = idx {
        w.resource_mut::<UseQueue>().uses.push(WantsToUse {
            user,
            item,
            target: None,
            slot_idx: Some(i),
        });
    }
    item_system(w);
}

fn stash(w: &mut World, user: Entity, item: Entity) {
    w.entity_mut(item).remove::<Position>();
    w.get_mut::<Backpack>(user).unwrap().items.push(item);
}

/// Put a fresh scroll of the row called `name` in `user`'s pack and read it.
fn read(w: &mut World, user: Entity, name: &str) {
    read_row(w, user, scroll_row(name).unwrap());
}

/// The same for a row in hand.
fn read_row(w: &mut World, user: Entity, row: &'static ScrollDef) {
    let scroll = row.spawn(w, Position { x: 0, y: 0 });
    stash(w, user, scroll);
    use_item(w, user, scroll);
}

/// Fixtures wired to the shipped detection handlers, so these tests follow
/// the behaviour and not whatever the rows are called.
static DETECT_SCROLL: ScrollDef = ScrollDef::new("detect scroll", scrolls::detect_mundane_items);
static DETECT_POTION: PotionDef =
    PotionDef::new("detect potion", Color::Yellow, potions::detect_magic);

fn logged(w: &World, needle: &str) -> bool {
    w.resource::<GameLog>()
        .history
        .iter()
        .any(|l| l.contains(needle))
}

fn spawn_dummy(w: &mut World, name: &str, x: u16, y: u16, hp: i32, mv: MovementType) -> Entity {
    w.spawn((
        Name { what: name.into() },
        Mob { movement_type: mv },
        Position { x, y },
        Fighter {
            hp,
            max_hp: hp,
            armor: 0,
            power: 1,
            max_power: 1,
            armor_bonus: 0,
            power_bonus: 0,
        },
        Faction::Monster,
        Blood,
    ))
    .id()
}

fn run_ai(w: &mut World) {
    let mut s = Schedule::default();
    s.add_systems(ai);
    s.run(w);
}

// ---------------------------------------------------------------------------
// Teleportation
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// Aggravate monsters
// ---------------------------------------------------------------------------

#[test]
fn aggravate_turns_every_monster_into_a_hunter_that_closes_in_unseen() {
    let mut w = test_world(1);
    let p = player(&mut w);
    let hero = *w.get::<Position>(p).unwrap();

    let mob_start = {
        let map = w.resource::<Map>();
        (2..8)
            .map(|d| (hero.x + d, hero.y))
            .find(|&(x, y)| !map.blocks(x, y))
            .unwrap()
    };
    let mob = spawn_dummy(
        &mut w,
        "orc",
        mob_start.0,
        mob_start.1,
        3,
        MovementType::Static,
    );
    w.get_mut::<Viewshed>(p).unwrap().visible_tiles = vec![(hero.x, hero.y)];

    let scroll = spawn_named(
        &mut w,
        "scroll of aggravate monsters",
        Position { x: 0, y: 0 },
    )
    .unwrap();
    stash(&mut w, p, scroll);
    use_item(&mut w, p, scroll);

    let ag = w
        .get::<Aggravated>(mob)
        .expect("aggravate should mark the mob Aggravated");
    assert_eq!((ag.tx, ag.ty), (hero.x, hero.y));

    let dist_before = mob_start.0 - hero.x;
    for _ in 0..3 {
        run_ai(&mut w);
    }
    let now = *w.get::<Position>(mob).unwrap();
    assert!(
        hero.x.abs_diff(now.x) < dist_before,
        "an aggravated monster should have closed the distance (was {dist_before}, now {})",
        hero.x.abs_diff(now.x)
    );
}

// ---------------------------------------------------------------------------
// Scare monster
// ---------------------------------------------------------------------------

#[test]
fn scare_monster_routs_what_you_can_see_and_leaves_the_rest_alone() {
    let mut w = test_world(3);
    let p = player(&mut w);
    let hero = *w.get::<Position>(p).unwrap();

    let seen = spawn_dummy(&mut w, "troll", hero.x + 1, hero.y, 4, MovementType::Chase);
    let unseen = spawn_dummy(&mut w, "troll", hero.x + 2, hero.y, 4, MovementType::Chase);
    w.get_mut::<Viewshed>(p).unwrap().visible_tiles = vec![(hero.x, hero.y), (hero.x + 1, hero.y)];

    let scroll = spawn_named(&mut w, "scroll of scare monster", Position { x: 0, y: 0 }).unwrap();
    stash(&mut w, p, scroll);
    use_item(&mut w, p, scroll);

    assert!(
        matches!(
            w.get::<Mob>(seen).unwrap().movement_type,
            MovementType::Flee
        ),
        "a monster in view is scared off"
    );
    assert!(
        matches!(
            w.get::<Mob>(unseen).unwrap().movement_type,
            MovementType::Chase
        ),
        "a monster out of view is untouched"
    );
}

// ---------------------------------------------------------------------------
// Create monster
// ---------------------------------------------------------------------------

#[test]
fn create_monster_sometimes_conjures_it_already_tamed() {
    let (mut monster, mut ally, mut helper) = (0, 0, 0);
    for seed in 0..60 {
        let mut w = test_world(seed);
        let p = player(&mut w);
        let before: std::collections::HashSet<Entity> =
            w.query_filtered::<Entity, With<Mob>>().iter(&w).collect();

        let scroll =
            spawn_named(&mut w, "scroll of create monster", Position { x: 0, y: 0 }).unwrap();
        stash(&mut w, p, scroll);
        use_item(&mut w, p, scroll);

        let mut q = w.query_filtered::<(Entity, &Faction, Option<&Helper>), With<Mob>>();
        let (_, faction, helper_row) = q
            .iter(&w)
            .find(|(e, _, _)| !before.contains(e))
            .expect("the scroll conjured exactly one new creature");
        match (faction, helper_row.is_some()) {
            (Faction::Monster, _) => monster += 1,
            (Faction::Ally, true) => helper += 1,
            (Faction::Ally, false) => ally += 1,
            _ => unreachable!(),
        }
    }
    assert!(
        monster > 0 && ally > 0 && helper > 0,
        "monster {monster}, ally {ally}, helper {helper}"
    );
}

// ---------------------------------------------------------------------------
// Charming
// ---------------------------------------------------------------------------

#[test]
fn charming_tames_every_monster_in_view_and_leaves_the_rest_alone() {
    let mut w = test_world(3);
    let p = player(&mut w);
    let hero = *w.get::<Position>(p).unwrap();

    let seen = spawn_dummy(&mut w, "troll", hero.x + 1, hero.y, 4, MovementType::Chase);
    let unseen = spawn_dummy(&mut w, "troll", hero.x + 2, hero.y, 4, MovementType::Chase);
    w.get_mut::<Viewshed>(p).unwrap().visible_tiles = vec![(hero.x, hero.y), (hero.x + 1, hero.y)];

    let scroll = spawn_named(&mut w, "scroll of charming", Position { x: 0, y: 0 }).unwrap();
    stash(&mut w, p, scroll);
    use_item(&mut w, p, scroll);

    assert_eq!(
        w.get::<Faction>(seen),
        Some(&Faction::Ally),
        "in view: charmed"
    );
    assert!(w.get::<Helper>(seen).is_none(), "charmed, not recruited");
    assert_eq!(
        w.get::<Faction>(unseen),
        Some(&Faction::Monster),
        "out of view: untouched"
    );
}

// ---------------------------------------------------------------------------
// Vorpalize weapon
// ---------------------------------------------------------------------------

/// Give the player a wielded weapon and return its entity.
fn wield_a_blade(w: &mut World, p: Entity) -> Entity {
    for item in equipped_items(w, p) {
        force_unequip(w, item);
    }
    let sword = spawn_weapon(w, "long sword", Position { x: 0, y: 0 });
    w.entity_mut(sword).remove::<Position>();
    w.get_mut::<Backpack>(p).unwrap().items.push(sword);
    w.get_mut::<Equipped>(sword).unwrap().by = Some(p);
    sword
}

#[test]
fn vorpalize_brands_the_wielded_blade_and_names_a_bane() {
    let mut w = test_world(11);
    let p = player(&mut w);
    let sword = wield_a_blade(&mut w, p);

    let scroll = spawn_named(
        &mut w,
        "scroll of vorpalize weapon",
        Position { x: 0, y: 0 },
    )
    .unwrap();
    stash(&mut w, p, scroll);
    use_item(&mut w, p, scroll);

    let bane = w
        .get::<Vorpal>(sword)
        .expect("the blade is now vorpal")
        .bane
        .clone();
    assert!(!bane.is_empty());
}

#[test]
fn vorpalizing_an_already_vorpal_weapon_crumbles_it() {
    let mut w = test_world(11);
    let p = player(&mut w);
    let sword = wield_a_blade(&mut w, p);
    w.entity_mut(sword).insert(Vorpal { bane: "orc".into() });

    let scroll = spawn_named(
        &mut w,
        "scroll of vorpalize weapon",
        Position { x: 0, y: 0 },
    )
    .unwrap();
    stash(&mut w, p, scroll);
    use_item(&mut w, p, scroll);

    assert!(
        !w.entities().contains(sword),
        "a twice-vorpalized blade is destroyed"
    );
    assert!(
        !w.get::<Backpack>(p).unwrap().items.contains(&sword),
        "…and gone from the pack"
    );
    assert!(
        w.resource::<GameLog>()
            .history
            .iter()
            .any(|l| l.contains("crumbles to dust"))
    );
}

#[test]
fn vorpalize_with_a_bow_in_hand_also_just_fizzles() {
    let mut w = test_world(11);
    let p = player(&mut w);
    for item in equipped_items(&w, p) {
        force_unequip(&mut w, item);
    }
    let bow = spawn_launcher(&mut w, "short bow", Position { x: 0, y: 0 });
    stash(&mut w, p, bow);
    toggle_equipped(&mut w, p, bow);

    let scroll = spawn_named(
        &mut w,
        "scroll of vorpalize weapon",
        Position { x: 0, y: 0 },
    )
    .unwrap();
    stash(&mut w, p, scroll);
    use_item(&mut w, p, scroll);

    assert_eq!(w.query::<&Vorpal>().iter(&w).count(), 0);
    assert!(
        w.resource::<GameLog>()
            .history
            .iter()
            .any(|l| l.contains("gutters out"))
    );
}

#[test]
fn a_vorpal_blade_beheads_its_bane_in_one_blow() {
    let mut w = test_world(1);
    let p = player(&mut w);
    let sword = wield_a_blade(&mut w, p);
    w.entity_mut(sword).insert(Vorpal { bane: "orc".into() });
    w.get_mut::<Fighter>(p).unwrap().power = 100;

    let hero = *w.get::<Position>(p).unwrap();
    let orc = spawn_dummy(&mut w, "orc", hero.x + 1, hero.y, 999, MovementType::Static);

    resolve_attack(&mut w, p, orc);

    assert!(
        !w.entities().contains(orc),
        "the bane is slain outright despite its 999 HP"
    );
    assert!(
        w.resource::<GameLog>()
            .history
            .iter()
            .any(|l| l.contains("Snicker-snack"))
    );
}

// ---------------------------------------------------------------------------
// Glancing blows can never kill
// ---------------------------------------------------------------------------

#[test]
fn a_glancing_blow_chips_a_foe_down_to_one_but_never_finishes_it() {
    let mut w = test_world(151);
    let p = player(&mut w);
    for item in equipped_items(&w, p) {
        force_unequip(&mut w, item);
    }
    w.get_mut::<Fighter>(p).unwrap().power = 1;
    w.get_mut::<Fighter>(p).unwrap().power_bonus = 0;

    let hero = *w.get::<Position>(p).unwrap();
    let foe = w
        .spawn((
            Name {
                what: "wall of a monster".into(),
            },
            Mob {
                movement_type: MovementType::Static,
            },
            Position {
                x: hero.x + 1,
                y: hero.y,
            },
            Fighter {
                hp: 5,
                max_hp: 5,
                armor: 2,
                power: 1,
                max_power: 1,
                armor_bonus: 10,
                power_bonus: 0,
            },
            Faction::Monster,
            Blood,
        ))
        .id();

    for _ in 0..40 {
        resolve_attack(&mut w, p, foe);
    }

    assert!(
        w.entities().contains(foe),
        "a string of glancing blows never kills"
    );
    assert_eq!(
        w.get::<Fighter>(foe).unwrap().hp,
        1,
        "they chip it to 1 HP and no further"
    );
    assert!(
        w.resource::<GameLog>()
            .history
            .iter()
            .any(|l| l.contains("glancing blow"))
    );
}

// ---------------------------------------------------------------------------
// Enchant weapon / enchant armor
// ---------------------------------------------------------------------------

#[test]
fn enchant_weapon_adds_a_plus_to_the_blade_in_hand() {
    let mut w = test_world(2);
    let p = player(&mut w);
    let sword = wield_a_blade(&mut w, p);

    read(&mut w, p, "scroll of enchant weapon");

    assert_eq!(
        w.get::<PowerBonus>(sword).map(|b| b.0),
        Some(1),
        "a plain blade comes out +1"
    );
    assert!(
        w.get::<KnownQuality>(sword).is_some(),
        "you watched it take — there is nothing left to be coy about"
    );
    assert!(logged(&w, "orange sparks"));
}

#[test]
fn enchant_weapon_mends_a_minus_all_the_way_to_plus_zero_and_burns_the_curse_off() {
    let mut w = test_world(2);
    let p = player(&mut w);
    let sword = wield_a_blade(&mut w, p);
    w.entity_mut(sword).insert(PowerBonus(-4));
    w.entity_mut(sword).insert(Curse);

    read(&mut w, p, "scroll of enchant weapon");

    assert_eq!(
        w.get::<PowerBonus>(sword).map(|b| b.0),
        Some(0),
        "a minus is mended whole, not nudged one point"
    );
    assert!(
        w.get::<Curse>(sword).is_none(),
        "…and the curse goes with it, without destroying the blade"
    );
    assert!(logged(&w, "curse on the long sword burns away"));
}

#[test]
fn enchanting_a_bow_lands_on_the_throw_the_way_the_dungeon_rolls_one() {
    let mut w = test_world(2);
    let p = player(&mut w);
    for item in equipped_items(&w, p) {
        force_unequip(&mut w, item);
    }
    let bow = spawn_launcher(&mut w, "short bow", Position { x: 0, y: 0 });
    stash(&mut w, p, bow);
    toggle_equipped(&mut w, p, bow);

    read(&mut w, p, "scroll of enchant weapon");

    assert_eq!(
        w.get::<ThrowBonus>(bow).map(|b| b.0),
        Some(1),
        "a bow's plus steadies its aim"
    );
    assert!(
        w.get::<PowerBonus>(bow).is_none(),
        "a bow has no melee roll for a plus to land on"
    );
}

#[test]
fn enchanting_with_nothing_in_the_slot_just_gutters_out() {
    let mut w = test_world(2);
    let p = player(&mut w);
    for item in equipped_items(&w, p) {
        force_unequip(&mut w, item);
    }

    read(&mut w, p, "scroll of enchant armor");

    assert!(logged(&w, "bare skin"));
}

// ---------------------------------------------------------------------------
// Monster confusion
// ---------------------------------------------------------------------------

#[test]
fn monster_confusion_charges_the_hands_and_the_next_landed_blow_spends_them() {
    let mut w = test_world(1);
    let p = player(&mut w);

    read(&mut w, p, "scroll of monster confusion");
    assert!(
        w.get::<ConfusingTouch>(p).is_some(),
        "the charm waits on the hands rather than going off now"
    );

    w.get_mut::<Fighter>(p).unwrap().power = 20;
    let hero = *w.get::<Position>(p).unwrap();
    let orc = spawn_dummy(&mut w, "orc", hero.x + 1, hero.y, 999, MovementType::Chase);

    resolve_attack(&mut w, p, orc);

    assert!(
        matches!(
            w.get::<Mob>(orc).unwrap().movement_type,
            MovementType::Confused
        ),
        "what you hit reels"
    );
    assert!(
        w.get::<ConfusingTouch>(p).is_none(),
        "and the charge is spent doing it"
    );
}

// ---------------------------------------------------------------------------
// Hold monster
// ---------------------------------------------------------------------------

#[test]
fn a_held_monster_cannot_step_but_still_bites_what_comes_in_reach() {
    let mut w = test_world(3);
    let p = player(&mut w);
    let hero = *w.get::<Position>(p).unwrap();

    let far = spawn_dummy(&mut w, "troll", hero.x + 2, hero.y, 4, MovementType::Chase);
    let near = spawn_dummy(&mut w, "troll", hero.x, hero.y + 1, 4, MovementType::Chase);
    w.get_mut::<Viewshed>(p).unwrap().visible_tiles =
        vec![(hero.x, hero.y), (hero.x + 2, hero.y), (hero.x, hero.y + 1)];

    read(&mut w, p, "scroll of hold monster");
    let stood = *w.get::<Position>(far).unwrap();
    run_ai(&mut w);

    assert_eq!(
        *w.get::<Position>(far).unwrap(),
        stood,
        "rooted where it stands"
    );
    assert!(
        w.resource::<AttackQueue>()
            .attacks
            .iter()
            .any(|a| a.attacker == near),
        "…but a held monster is pinned, not helpless: in reach, it still bites"
    );
}

// ---------------------------------------------------------------------------
// Sleep
// ---------------------------------------------------------------------------

/// Reads a scroll of sleep with one monster in view, and reports whether the
/// room went down (`false`) or the reader did (`true`).
fn read_sleep_once(seed: u64) -> bool {
    let mut w = test_world(seed);
    let p = player(&mut w);
    let hero = *w.get::<Position>(p).unwrap();
    let mob = spawn_dummy(&mut w, "rat", hero.x + 1, hero.y, 4, MovementType::Chase);
    w.get_mut::<Viewshed>(p).unwrap().visible_tiles = vec![(hero.x, hero.y), (hero.x + 1, hero.y)];

    read(&mut w, p, "scroll of sleep");

    let reader_out = w.get::<Asleep>(p).is_some();
    let room_out = w.get::<Asleep>(mob).is_some();
    assert!(
        reader_out != room_out,
        "seed {seed}: the scroll takes the room or the reader, never both and never neither"
    );
    reader_out
}

#[test]
fn sleep_usually_takes_the_room_and_sometimes_the_reader() {
    let backfires = (0..40u64).filter(|&s| read_sleep_once(s)).count();
    assert!(
        backfires > 0,
        "the backfire is the whole gamble — it has to actually happen"
    );
    assert!(
        backfires * 2 < 40,
        "…but it is the minority case (backfired {backfires} times in 40)"
    );
}

#[test]
fn a_sleeping_monster_forfeits_its_turn_outright() {
    let mut w = test_world(3);
    let p = player(&mut w);
    let hero = *w.get::<Position>(p).unwrap();
    let mob = spawn_dummy(&mut w, "rat", hero.x, hero.y + 1, 4, MovementType::Chase);
    hold(&mut w, mob, Grant::of::<Asleep>(), 3);

    run_ai(&mut w);

    assert!(
        w.resource::<AttackQueue>().attacks.is_empty(),
        "asleep in reach of the player and it still does nothing"
    );
}

// ---------------------------------------------------------------------------
// Food detection, and the one thing both detections find
// ---------------------------------------------------------------------------

/// A detection turns an invisibly-stashed item up for good, exactly the way a
/// ring of perception does. Half a reveal would be worse than none: the
/// `Detected` render pass only paints tiles the player *cannot* see, so a stash
/// left wearing `Hidden` would glow magenta from across the floor and then wink
/// out the moment the player walked into the room.
#[test]
fn detection_turns_a_stashed_item_up_for_good() {
    let spot = Position { x: 1, y: 1 };

    let mut w = test_world(2);
    let p = player(&mut w);
    let plain = spawn_weapon(&mut w, "dagger", spot);
    w.entity_mut(plain).insert((Hidden, Invisible));
    read_row(&mut w, p, &DETECT_SCROLL);
    assert!(w.get::<Detected>(plain).is_some());
    assert!(w.get::<Hidden>(plain).is_none());
    assert!(w.get::<Invisible>(plain).is_none());

    let mut w = test_world(2);
    let p = player(&mut w);
    let potion = spawn_named(&mut w, "potion of healing", spot).unwrap();
    w.entity_mut(potion).insert((Hidden, Invisible));
    let detection = (&DETECT_POTION).spawn(&mut w, spot);
    stash(&mut w, p, detection);
    use_item(&mut w, p, detection);
    assert!(w.get::<Detected>(potion).is_some());
    assert!(w.get::<Hidden>(potion).is_none());
    assert!(w.get::<Invisible>(potion).is_none());
}

#[test]
fn both_detections_find_the_element_of_yoord() {
    let spot = Position { x: 1, y: 1 };

    let mut w = test_world(2);
    let p = player(&mut w);
    let relic = spawn_element_of_yoord(&mut w, spot);
    read_row(&mut w, p, &DETECT_SCROLL);
    assert!(
        w.get::<Detected>(relic).is_some(),
        "the scroll finds the relic"
    );

    let mut w = test_world(2);
    let p = player(&mut w);
    let relic = spawn_element_of_yoord(&mut w, spot);
    let potion = (&DETECT_POTION).spawn(&mut w, Position { x: 0, y: 0 });
    stash(&mut w, p, potion);
    use_item(&mut w, p, potion);
    assert!(
        w.get::<Detected>(relic).is_some(),
        "and so does the potion — it is the run, and neither sense misses it"
    );
}

// ---------------------------------------------------------------------------
// The juice
// ---------------------------------------------------------------------------

/// The effect layer is optional — every test above runs without one, the way a
/// headless caller does. With one present the new scrolls have to actually
/// queue their flourish, which is the half those tests can't see.
#[test]
fn the_new_scrolls_queue_their_flourish_when_there_is_an_effect_layer() {
    let cases: Vec<&'static ScrollDef> = [
        "scroll of enchant weapon",
        "scroll of enchant armor",
        "scroll of monster confusion",
        "scroll of hold monster",
        "scroll of sleep",
        "scroll of teleportation",
    ]
    .into_iter()
    .map(|name| scroll_row(name).unwrap())
    .chain([&DETECT_SCROLL])
    .collect();
    for row in cases {
        let name = row.name;
        let mut w = test_world(6);
        w.insert_resource(Particles::new());
        let p = player(&mut w);
        let hero = *w.get::<Position>(p).unwrap();
        spawn_dummy(&mut w, "rat", hero.x + 1, hero.y, 4, MovementType::Chase);
        w.get_mut::<Viewshed>(p).unwrap().visible_tiles =
            vec![(hero.x, hero.y), (hero.x + 1, hero.y)];

        read_row(&mut w, p, row);

        let fx = w.resource::<Particles>();
        assert!(
            fx.pending && fx.any_alive(),
            "{name} should have queued something to look at"
        );
    }
}

// ---------------------------------------------------------------------------
// Pitfall
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// Atonement
// ---------------------------------------------------------------------------

#[test]
fn atonement_makes_angered_spirits_neutral_again() {
    let mut w = test_world(3);
    let p = player(&mut w);
    let hero = *w.get::<Position>(p).unwrap();
    let spirit = spawn_monster(
        &mut w,
        MonsterDef::named("angel"),
        Position {
            x: hero.x + 3,
            y: hero.y,
        },
    );
    for _ in 0..3 {
        spirits::shift_alignment(&mut w, p, 1);
    }
    assert!(w.resource::<SpiritsHostile>().0);
    assert!(matches!(
        w.get::<Mob>(spirit).unwrap().movement_type,
        MovementType::Chase
    ));

    read(&mut w, p, "scroll of atonement");

    assert!(!w.resource::<SpiritsHostile>().0);
    assert_eq!(w.get::<Alignment>(p).unwrap().0, 0);
    assert!(matches!(
        w.get::<Mob>(spirit).unwrap().movement_type,
        MovementType::Confused
    ));
}

// ---------------------------------------------------------------------------
// Teleport to stairs
// ---------------------------------------------------------------------------

fn within_one_of(w: &World, p: Entity, tile: TileType) -> bool {
    let i = w
        .resource::<Map>()
        .tiles
        .iter()
        .position(|&t| t == tile)
        .unwrap();
    let (sx, sy) = (
        (i % MAP_WIDTH as usize) as u16,
        (i / MAP_WIDTH as usize) as u16,
    );
    let at = w.get::<Position>(p).unwrap();
    at.x.abs_diff(sx) <= 1 && at.y.abs_diff(sy) <= 1
}

#[test]
fn teleport_to_stairs_goes_down_without_the_element_and_up_with_it() {
    let mut w = test_world(3);
    let p = player(&mut w);
    assert!(!within_one_of(&w, p, TileType::Downstairs));

    read(&mut w, p, "scroll of teleport to stairs");
    assert!(within_one_of(&w, p, TileType::Downstairs));

    let element = spawn_element_of_yoord(&mut w, Position { x: 0, y: 0 });
    stash(&mut w, p, element);
    assert!(!within_one_of(&w, p, TileType::Upstairs));

    read(&mut w, p, "scroll of teleport to stairs");
    assert!(within_one_of(&w, p, TileType::Upstairs));
}

/// Two open tiles side by side on the hero's row, east of them, nearest first.
fn two_open_east(w: &mut World, hero: Position) -> (Position, Position) {
    let map = w.resource::<Map>();
    (1..12)
        .map(|d| (hero.x + d, hero.y))
        .find(|&(x, y)| !map.blocks(x, y) && !map.blocks(x + 1, y))
        .map(|(x, y)| (Position { x, y }, Position { x: x + 1, y }))
        .expect("two open tiles east of the start")
}

fn make_tough(w: &mut World, e: Entity) {
    let mut f = w.get_mut::<Fighter>(e).unwrap();
    f.hp = 1000;
    f.max_hp = 1000;
}

// ---------------------------------------------------------------------------
// Recharging
// ---------------------------------------------------------------------------

#[test]
fn recharging_adds_one_charge_to_every_wand_up_to_the_cap() {
    let mut w = test_world(7);
    let p = player(&mut w);
    let charges = [3, 5, constants::wands::WAND_CHARGES];
    let wands: Vec<Entity> = charges
        .iter()
        .map(|&c| {
            let wand = spawn_named(&mut w, "wand of light", NOWHERE).unwrap();
            w.get_mut::<Battery>(wand).unwrap().charges = c;
            stash(&mut w, p, wand);
            wand
        })
        .collect();

    read(&mut w, p, "scroll of recharging");

    let now: Vec<i8> = wands
        .iter()
        .map(|&e| w.get::<Battery>(e).unwrap().charges)
        .collect();
    let cap = constants::wands::WAND_CHARGES;
    assert_eq!(now, vec![4, cap, cap]);
}

// ---------------------------------------------------------------------------
// Protection
// ---------------------------------------------------------------------------

#[test]
fn protection_stops_all_damage_for_six_turns_and_then_stops_stopping_it() {
    let mut w = test_world(7);
    let p = player(&mut w);
    make_tough(&mut w, p);
    let start = w.get::<Fighter>(p).unwrap().hp;
    let hero = *w.get::<Position>(p).unwrap();
    let (near, _) = two_open_east(&mut w, hero);
    let orc = monster::plain_monster(&mut w, "orc", near);
    w.get_mut::<Fighter>(orc).unwrap().power = 8;

    read(&mut w, p, "scroll of protection");
    assert!(w.get::<Protected>(p).is_some());

    assert_eq!(apply_hit(&mut w, p, Hit::physical(9), None), 0);
    assert_eq!(
        apply_hit(
            &mut w,
            p,
            Hit {
                amount: 9,
                element: Some(Element::Fire),
            },
            None
        ),
        0
    );
    for _ in 0..100 {
        resolve_attack(&mut w, orc, p);
    }
    assert_eq!(w.get::<Fighter>(p).unwrap().hp, start, "untouched");

    for _ in 0..constants::scrolls::PROTECTION_TURNS {
        assert!(w.get::<Protected>(p).is_some());
        tick_effects(&mut w);
    }
    assert!(w.get::<Protected>(p).is_none(), "six turns, then it ends");

    assert!(apply_hit(&mut w, p, Hit::physical(9), None) > 0);
    for _ in 0..100 {
        resolve_attack(&mut w, orc, p);
    }
    assert!(
        w.get::<Fighter>(p).unwrap().hp < start - 9,
        "blades bite again"
    );
}

// ---------------------------------------------------------------------------
// Justice
// ---------------------------------------------------------------------------

/// The hero, a marked monster `a` beside them, and a one-HP bystander `b` next
/// to `a`, out of the hero's sight so the scroll never marked it. `a`'s power die
/// is what the burst rolls.
fn justice_scene(w: &mut World) -> (Entity, Entity, Entity) {
    let p = player(w);
    make_tough(w, p);
    let hero = *w.get::<Position>(p).unwrap();
    let (at_a, at_b) = two_open_east(w, hero);
    let a = monster::plain_monster(w, "orc", at_a);
    let b = monster::plain_monster(w, "bat", at_b);
    {
        let mut f = w.get_mut::<Fighter>(a).unwrap();
        f.hp = 1;
        f.max_hp = 1;
        f.power = 6;
        f.max_power = 6;
    }
    {
        let mut f = w.get_mut::<Fighter>(b).unwrap();
        f.hp = 1;
        f.max_hp = 1;
    }
    w.get_mut::<Viewshed>(p).unwrap().visible_tiles = vec![(hero.x, hero.y), (at_a.x, at_a.y)];

    read(w, p, "scroll of justice");
    (p, a, b)
}

#[test]
fn a_marked_monster_cut_down_in_melee_bursts_too() {
    let mut w = test_world(7);
    let (p, a, b) = justice_scene(&mut w);

    for _ in 0..200 {
        if w.get_entity(a).is_none() {
            break;
        }
        resolve_attack(&mut w, p, a);
    }

    assert!(w.get_entity(a).is_none(), "the orc fell");
    assert!(w.get_entity(b).is_none(), "and the burst took the bat");
}
