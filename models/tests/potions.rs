//! The potion table, now that every row does something: the four that change
//! the drinker's body for good, the three conditions, the two detections, the
//! floor-long second sight, and the one that is a staircase in a bottle.

use bevy_ecs::prelude::*;
use bevy_ecs::schedule::Schedule;
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

/// Drink (or read, or zap) `item` out of the pack — the engine's "Use" action.
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

/// Put a fresh potion of `effect` in `user`'s pack and drink it.
fn quaff(w: &mut World, user: Entity, effect: PotionEffect) {
    let potion = spawn_potion(w, effect, Position { x: 0, y: 0 });
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

fn run_visibility(w: &mut World) {
    w.query_filtered::<&mut Viewshed, With<Player>>()
        .single_mut(w)
        .dirty = true;
    let mut s = Schedule::default();
    s.add_systems(visibility_system);
    s.run(w);
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
fn healing_refills_and_raises_the_ceiling() {
    let mut w = test_world(3);
    let p = player(&mut w);
    let max = w.get::<Fighter>(p).unwrap().max_hp;
    w.get_mut::<Fighter>(p).unwrap().hp = 1;

    quaff(&mut w, p, PotionEffect::Healing);

    let f = w.get::<Fighter>(p).unwrap();
    assert!(f.max_hp > max, "a dose raises the ceiling");
    assert_eq!(f.hp, f.max_hp, "and fills you to it");
}

#[test]
fn extra_healing_raises_the_ceiling() {
    let mut w = test_world(3);
    let p = player(&mut w);
    let max = w.get::<Fighter>(p).unwrap().max_hp;

    // Drunk at full health, which is the point: the ceiling still moves.
    quaff(&mut w, p, PotionEffect::ExtraHealing);

    let f = w.get::<Fighter>(p).unwrap();
    assert!(f.max_hp > max);
    assert_eq!(f.hp, f.max_hp);
}

#[test]
fn strength_is_gained_poisoned_and_restored() {
    let mut w = test_world(3);
    let p = player(&mut w);
    let base = w.get::<Fighter>(p).unwrap().power;

    quaff(&mut w, p, PotionEffect::GainStrength);
    let f = w.get::<Fighter>(p).unwrap();
    assert_eq!((f.power, f.max_power), (base + 1, base + 1));

    quaff(&mut w, p, PotionEffect::Poison);
    let f = w.get::<Fighter>(p).unwrap();
    assert!(f.power < base + 1 && f.power >= 1, "poison lowers power safely");
    assert_eq!(f.max_power, base + 1, "the ceiling is untouched");

    quaff(&mut w, p, PotionEffect::RestoreStrength);
    let f = w.get::<Fighter>(p).unwrap();
    assert_eq!(
        f.power, f.max_power,
        "restored to the new peak, not the old"
    );
}

#[test]
fn poison_can_leave_you_feeble_but_never_weaponless() {
    let mut w = test_world(3);
    let p = player(&mut w);
    w.get_mut::<Fighter>(p).unwrap().power = 2;

    quaff(&mut w, p, PotionEffect::Poison);
    quaff(&mut w, p, PotionEffect::Poison);

    assert_eq!(w.get::<Fighter>(p).unwrap().power, 1, "the floor holds");
}

// ---------------------------------------------------------------------------
// Conditions
// ---------------------------------------------------------------------------

#[test]
fn haste_goes_straight_to_fast_and_the_stairs_wash_it_out() {
    let mut w = test_world(3);
    let p = player(&mut w);
    w.get_mut::<Speed>(p).unwrap().kind = SpeedKind::Slow;

    quaff(&mut w, p, PotionEffect::Haste);
    assert_eq!(
        w.get::<Speed>(p).unwrap().kind,
        SpeedKind::Fast,
        "a potion skips the notches a wand steps through"
    );

    descend(&mut w, p);
    assert_eq!(w.get::<Speed>(p).unwrap().kind, SpeedKind::Normal);
}

#[test]
fn paralysis_slows_you_and_costs_you_turns_until_a_staircase() {
    let mut w = test_world(3);
    let p = player(&mut w);

    quaff(&mut w, p, PotionEffect::Paralysis);
    assert!(w.get::<Paralyzed>(p).is_some());
    assert_eq!(w.get::<Speed>(p).unwrap().kind, SpeedKind::Slow);

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
    quaff(&mut w, p, PotionEffect::Confusion);
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

#[test]
fn blindness_cuts_the_player_to_arms_reach_and_hides_every_monster() {
    let mut w = test_world(3);
    let p = player(&mut w);
    let start = *w.get::<Position>(p).unwrap();

    run_visibility(&mut w);
    let sighted = w.get::<Viewshed>(p).unwrap().visible_tiles.len();

    quaff(&mut w, p, PotionEffect::Blindness);
    run_visibility(&mut w);

    let vs = w.get::<Viewshed>(p).unwrap();
    assert!(vs.visible_tiles.len() < sighted, "and less than eyes give");
    assert!(vs.visible_tiles.contains(&(start.x, start.y)));

    // Even a monster standing right next to you is not perceived.
    let next_door = spawn_dummy(&mut w, "orc", start.x + 1, start.y);
    run_visibility(&mut w);
    assert!(w.get::<Hidden>(next_door).is_some());

    descend(&mut w, p);
    assert!(
        w.get::<Blind>(p).is_none(),
        "the stairs give your eyes back"
    );
}

#[test]
fn a_blinded_player_is_not_a_hidden_player() {
    let mut w = test_world(3);
    let p = player(&mut w);
    let start = *w.get::<Position>(p).unwrap();

    // An orc four tiles away in the same lit room, blind player or not.
    let orc = spawn_dummy(&mut w, "orc", start.x + 4, start.y);
    let before = *w.get::<Position>(orc).unwrap();

    quaff(&mut w, p, PotionEffect::Blindness);
    run_visibility(&mut w);
    let mut s = Schedule::default();
    s.add_systems(ai);
    s.run(&mut w);

    assert_ne!(
        *w.get::<Position>(orc).unwrap(),
        before,
        "the monsters can still see you perfectly well"
    );
}

// ---------------------------------------------------------------------------
// The senses
// ---------------------------------------------------------------------------

#[test]
fn see_invisible_lasts_the_floor_and_no_longer() {
    let mut w = test_world(3);
    let p = player(&mut w);

    quaff(&mut w, p, PotionEffect::SeeInvisible);
    assert!(w.get::<SeesInvisible>(p).is_some());

    descend(&mut w, p);
    assert!(
        w.get::<SeesInvisible>(p).is_none(),
        "a potion's sight does not survive the stairs"
    );
    assert!(logged(&w, "You are no longer able to see the unseen."));
}

#[test]
fn a_worn_ring_keeps_its_sight_when_the_potion_expires() {
    let mut w = test_world(3);
    let p = player(&mut w);
    let ring = spawn_ring(&mut w, RingEffect::Perception, Position { x: 0, y: 0 });
    w.entity_mut(ring).remove::<Position>();
    w.get_mut::<Backpack>(p).unwrap().items.push(ring);
    toggle_equipped(&mut w, p, ring);

    quaff(&mut w, p, PotionEffect::SeeInvisible);
    descend(&mut w, p);

    assert!(
        w.get::<SeesInvisible>(p).is_some(),
        "the ring is still on a finger"
    );
}

#[test]
fn monster_detection_turns_up_every_creature_on_the_floor() {
    let mut w = test_world(3);
    let p = player(&mut w);
    let far = spawn_dummy(&mut w, "orc", 60, 18);
    w.entity_mut(far).insert((Hidden, Invisible));

    quaff(&mut w, p, PotionEffect::MonsterDetection);

    assert!(w.get::<Detected>(far).is_some());
    // A creature's tags are not a stash to be turned up: `visibility_system`
    // recomputes `Hidden` on every mob every turn, and a phantom's `Invisible`
    // is what it *is*. Detection senses where they are and changes neither.
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

    let wand = spawn_wand(&mut w, WandEffect::Striking, here);
    w.entity_mut(wand).insert((Hidden, Invisible));
    let plain = spawn_weapon(&mut w, "dagger", here);
    let cursed = spawn_weapon(&mut w, "mace", here);
    w.entity_mut(cursed).insert(Curse);
    let enchanted = spawn_armor(&mut w, "leather armor", here);
    w.entity_mut(enchanted).insert(ArmorBonus(2));

    quaff(&mut w, p, PotionEffect::MagicDetection);

    assert!(
        w.get::<Detected>(wand).is_some(),
        "a wand is magic outright"
    );
    // Detecting a stashed item turns it up for good — see
    // `scrolls.rs`'s `detection_turns_a_stashed_item_up_for_good`.
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

    quaff(&mut w, p, PotionEffect::RaiseLevel);

    assert_eq!(w.resource::<Depth>().what, 2, "up one, relic or no relic");
    assert!(!w.resource::<Ending>().player_won);
}

#[test]
fn raise_level_on_depth_one_needs_the_relic() {
    let mut w = test_world(3);
    let p = player(&mut w);

    quaff(&mut w, p, PotionEffect::RaiseLevel);

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

    quaff(&mut w, p, PotionEffect::RaiseLevel);

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

    quaff(&mut w, p, PotionEffect::FruitJuice);

    // Water is no longer a spawnable catalog row — the only way an item ever
    // becomes one is a wand of cancellation mutating it in place (see
    // `items::wands::cancel_entity`) — so it's built directly here instead of
    // through `spawn_potion`, which only knows real catalog rows.
    let water = w
        .spawn((
            Name {
                what: "potion of thirst quenching".into(),
            },
            Potion {
                effect: PotionEffect::Water,
            },
            Consume,
            Position { x: 0, y: 0 },
        ))
        .id();
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

    quaff(&mut w, p, PotionEffect::Magic);

    let m = *w.get::<Magic>(p).unwrap();
    assert_eq!(m.max_points, max + 1, "a dose is worth a point of ceiling");
    assert_eq!(m.points, m.max_points, "and fills you to it");
}
