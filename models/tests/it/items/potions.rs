//! The potion table, now that every row does something: the four that change
//! the drinker's body for good, the three conditions, the two detections, the
//! floor-long second sight, and the one that is a staircase in a bottle.

use crate::common::monster;

use bevy_ecs::prelude::*;
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

/// Drink (or read, or zap) `item` from the pack — the engine's "Use" action.
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

/// Put a fresh potion of the row called `name` in `user`'s pack and drink it.
fn quaff(w: &mut World, user: Entity, name: &str) {
    let potion = spawn_named(w, name, Position { x: 0, y: 0 }).unwrap();
    w.entity_mut(potion).remove::<Position>();
    w.get_mut::<Backpack>(user).unwrap().items.push(potion);
    use_item(w, user, potion);
}

fn logged(w: &World, needle: &str) -> bool {
    w.resource::<GameLog>()
        .history
        .iter()
        .any(|l| l.contains(needle))
}

fn spawn_dummy(w: &mut World, name: &str, x: u16, y: u16) -> Entity {
    w.spawn((
        Name { what: name.into() },
        Mob {
            movement_type: MovementType::Chase,
        },
        Position { x, y },
        Fighter {
            hp: 5,
            max_hp: 5,
            armor: 0,
            power: 1,
            max_power: 1,
            armor_bonus: 0,
            power_bonus: 0,
        },
        Faction::Monster,
        Speed::new(SpeedKind::Normal),
        Blood,
    ))
    .id()
}

/// Walk the player onto the down-stair and take it.
fn descend(w: &mut World, p: Entity) {
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

// ---------------------------------------------------------------------------
// The body
// ---------------------------------------------------------------------------

#[test]
fn extra_healing_raises_the_ceiling() {
    let mut w = test_world(3);
    let p = player(&mut w);
    let max = w.get::<Fighter>(p).unwrap().max_hp;

    quaff(&mut w, p, "potion of extra healing");

    let f = w.get::<Fighter>(p).unwrap();
    assert!(f.max_hp > max);
    assert_eq!(f.hp, f.max_hp);
}

#[test]
fn strength_is_gained_poisoned_and_restored() {
    let mut w = test_world(3);
    let p = player(&mut w);
    let base = w.get::<Fighter>(p).unwrap().power;

    quaff(&mut w, p, "potion of gain strength");
    let f = w.get::<Fighter>(p).unwrap();
    assert_eq!((f.power, f.max_power), (base + 1, base + 1));

    quaff(&mut w, p, "potion of poison");
    let f = w.get::<Fighter>(p).unwrap();
    assert!(
        f.power < base + 1 && f.power >= 1,
        "poison lowers power safely"
    );
    assert_eq!(f.max_power, base + 1, "the ceiling is untouched");

    quaff(&mut w, p, "potion of restore strength");
    let f = w.get::<Fighter>(p).unwrap();
    assert_eq!(
        f.power, f.max_power,
        "restored to the new peak, not the old"
    );
}

// ---------------------------------------------------------------------------
// Conditions
// ---------------------------------------------------------------------------

#[test]
fn haste_overflows_and_the_stairs_wash_it_out() {
    let mut w = test_world(3);
    let p = player(&mut w);

    quaff(&mut w, p, "potion of haste self");
    assert_eq!(w.get::<Speed>(p).unwrap().kind, SpeedKind::Fast);

    quaff(&mut w, p, "potion of haste self");
    assert_eq!(
        w.get::<Speed>(p).unwrap().kind,
        SpeedKind::Slow,
        "a second dose overflows"
    );

    descend(&mut w, p);
    assert_eq!(w.get::<Speed>(p).unwrap().kind, SpeedKind::Normal);
}

#[test]
fn paralysis_costs_you_turns_but_not_tempo_until_a_staircase() {
    let mut w = test_world(3);
    let p = player(&mut w);

    quaff(&mut w, p, "potion of paralysis");
    assert!(w.get::<Paralyzed>(p).is_some());
    assert_eq!(w.get::<Speed>(p).unwrap().kind, SpeedKind::Normal);

    // Over many turns the coin flip eats some of them but not all.
    let lost = (0..200).filter(|_| paralysis_forfeits_turn(&mut w)).count();
    assert!(
        (40..160).contains(&lost),
        "about half of 200 turns should go (got {lost})"
    );

    descend(&mut w, p);
    assert!(w.get::<Paralyzed>(p).is_none());
    assert_eq!(w.get::<Speed>(p).unwrap().kind, SpeedKind::Normal);
    assert!(
        !paralysis_forfeits_turn(&mut w),
        "and no more turns are lost"
    );
}

#[test]
fn confusion_confuses_the_player_and_staggers_a_monster() {
    let mut w = test_world(3);
    let p = player(&mut w);
    quaff(&mut w, p, "potion of confusion");
    assert!(w.get::<Confused>(p).is_some());

    let orc = spawn_dummy(&mut w, "orc", 5, 5);
    assert!(confuse(&mut w, orc, "you", LogCategory::Plain, "reels"));
    assert!(matches!(
        w.get::<Mob>(orc).unwrap().movement_type,
        MovementType::Confused
    ));
}

#[test]
fn blindness_on_a_monster_is_just_a_random_walk() {
    let mut w = test_world(3);
    let orc = spawn_dummy(&mut w, "orc", 5, 5);

    blind(&mut w, orc);

    assert!(
        w.get::<Blind>(orc).is_none(),
        "a monster has no viewshed to put out"
    );
    assert!(matches!(
        w.get::<Mob>(orc).unwrap().movement_type,
        MovementType::Confused
    ));
}

// ---------------------------------------------------------------------------
// The senses
// ---------------------------------------------------------------------------

#[test]
fn see_invisible_lasts_the_floor_and_no_longer() {
    let mut w = test_world(3);
    let p = player(&mut w);

    quaff(&mut w, p, "potion of see invisible");
    assert!(w.get::<SeesInvisible>(p).is_some());

    descend(&mut w, p);
    assert!(
        w.get::<SeesInvisible>(p).is_none(),
        "a potion's sight does not survive the stairs"
    );
    assert!(logged(&w, "You are no longer able to see the unseen."));
}

#[test]
fn monster_detection_turns_up_every_creature_on_the_floor() {
    let mut w = test_world(3);
    let p = player(&mut w);
    let far = spawn_dummy(&mut w, "orc", 60, 18);
    w.entity_mut(far).insert((Hidden, Invisible));

    quaff(&mut w, p, "potion of monster detection");

    assert!(w.get::<Detected>(far).is_some());
    assert!(w.get::<Hidden>(far).is_some());
    assert!(w.get::<Invisible>(far).is_some());
    let undetected = w
        .query_filtered::<Entity, (With<Mob>, Without<Detected>)>()
        .iter(&w)
        .count();
    assert_eq!(undetected, 0, "every last one of them");
}

#[test]
fn magic_detection_skips_ordinary_gear() {
    let mut w = test_world(3);
    let p = player(&mut w);
    let here = Position { x: 40, y: 10 };

    let wand = spawn_named(&mut w, "wand of striking", here).unwrap();
    w.entity_mut(wand).insert((Hidden, Invisible));
    let plain = spawn_weapon(&mut w, "dagger", here);
    let cursed = spawn_weapon(&mut w, "mace", here);
    w.entity_mut(cursed).insert(Curse);
    let enchanted = spawn_armor(&mut w, "leather armor", here);
    w.entity_mut(enchanted).insert(ArmorBonus(2));

    quaff(&mut w, p, "potion of magic detection");

    assert!(
        w.get::<Detected>(wand).is_some(),
        "a wand is magic outright"
    );
    assert!(w.get::<Hidden>(wand).is_none());
    assert!(w.get::<Invisible>(wand).is_none());
    assert!(w.get::<Detected>(cursed).is_some(), "so is a curse");
    assert!(w.get::<Detected>(enchanted).is_some(), "so is a plus");
    assert!(
        w.get::<Detected>(plain).is_none(),
        "a plain dagger is just metal"
    );
}

// ---------------------------------------------------------------------------
// Raise level
// ---------------------------------------------------------------------------

#[test]
fn raise_level_climbs_a_floor_without_the_element() {
    let mut w = test_world(3);
    let p = player(&mut w);
    descend(&mut w, p);
    descend(&mut w, p);
    assert_eq!(w.resource::<Depth>().what, 3);

    quaff(&mut w, p, "potion of raise level");

    assert_eq!(w.resource::<Depth>().what, 2, "up one, relic or no relic");
    assert!(!w.resource::<Ending>().player_won);
}

#[test]
fn raise_level_on_depth_one_needs_the_relic() {
    let mut w = test_world(3);
    let p = player(&mut w);

    quaff(&mut w, p, "potion of raise level");

    assert!(!w.resource::<Ending>().player_won, "nothing to rise to");
    assert_eq!(w.resource::<Depth>().what, 1);
    assert!(logged(&w, "You hear distant laughter."));
}

#[test]
fn raise_level_on_depth_one_with_the_element_wins_the_run() {
    let mut w = test_world(3);
    let p = player(&mut w);
    let relic = spawn_element_of_yoord(&mut w, Position { x: 0, y: 0 });
    w.entity_mut(relic).remove::<Position>();
    w.get_mut::<Backpack>(p).unwrap().items.push(relic);

    quaff(&mut w, p, "potion of raise level");

    assert!(
        w.resource::<Ending>().player_won,
        "the potion takes the last stair for you"
    );
}

// ---------------------------------------------------------------------------
// The ones that are only a taste
// ---------------------------------------------------------------------------

#[test]
fn fruit_juice_and_water_are_flavour_and_nothing_else() {
    let mut w = test_world(3);
    let p = player(&mut w);
    let before = {
        let f = w.get::<Fighter>(p).unwrap();
        (f.hp, f.max_hp, f.power)
    };

    quaff(&mut w, p, "potion of fruit juice");

    let water =
        (potion_row("potion of thirst quenching").unwrap()).spawn(&mut w, Position { x: 0, y: 0 });
    w.entity_mut(water).remove::<Position>();
    w.get_mut::<Backpack>(p).unwrap().items.push(water);
    use_item(&mut w, p, water);

    let after = w.get::<Fighter>(p).unwrap();
    assert_eq!((after.hp, after.max_hp, after.power), before);
    assert!(logged(&w, "Yummy!"));
    assert!(logged(&w, "It is water."));
}

#[test]
fn magic_refills_the_pool_and_raises_its_ceiling() {
    let mut w = test_world(3);
    let p = player(&mut w);
    let max = w.get::<Magic>(p).unwrap().max_points;
    w.get_mut::<Magic>(p).unwrap().points = 0;

    quaff(&mut w, p, "potion of magic");

    let m = *w.get::<Magic>(p).unwrap();
    assert_eq!(m.max_points, max + 1, "a dose is worth a point of ceiling");
    assert_eq!(m.points, m.max_points, "and fills you to it");
}

#[test]
fn adjustment_resets_alignment_to_neutral() {
    let mut w = test_world(3);
    let p = player(&mut w);
    w.get_mut::<Alignment>(p).unwrap().0 = -3;

    quaff(&mut w, p, "potion of adjustment");

    assert_eq!(w.get::<Alignment>(p).unwrap().0, 0);
    assert!(logged(&w, "calm"));
}

#[test]
fn a_potion_of_polymorph_polymorphs_the_drinker_for_the_floor() {
    let mut w = test_world(3);
    let p = player(&mut w);

    quaff(&mut w, p, "potion of polymorph");
    assert!(w.get::<Polymorphed>(p).is_some());

    descend(&mut w, p);
    assert!(w.get::<Polymorphed>(p).is_none());
    assert!(logged(&w, "You are no longer polymorphed."));
}

#[test]
fn an_unseen_monsters_second_sight_says_nothing() {
    let mut w = test_world(1);
    let p = player(&mut w);
    let orc = monster::plain_monster(&mut w, "orc", Position { x: 50, y: 15 });
    w.entity_mut(orc).insert(Backpack { items: vec![] });
    w.get_mut::<Viewshed>(p).unwrap().visible_tiles = vec![(1, 1)];

    quaff(&mut w, orc, "potion of see invisible");

    assert!(w.get::<SeesInvisible>(orc).is_some());
    assert!(!logged(&w, "orc"), "{:?}", w.resource::<GameLog>().history);
}
