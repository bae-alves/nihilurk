use bevy_ecs::prelude::*;
use models::*;

fn test_world(seed: u64) -> World {
    let mut w = World::new();
    w.insert_resource(GameRng(ChaCha12Rng::seed_from_u64(seed)));
    w.insert_resource(RngSeed(seed));
    w.init_resource::<GameLog>();
    w.init_resource::<UseQueue>();
    w.insert_resource(PlayerName { what: "TESTER".into() });
    initialize_world(&mut w);
    w
}

fn player(w: &mut World) -> Entity {
    w.query_filtered::<Entity, With<Player>>().single(w)
}

/// Simulate the inventory "Use" action, exactly like the engine does.
fn use_item(w: &mut World, user: Entity, item: Entity) {
    let idx = w.get_mut::<Backpack>(user).unwrap().items.iter().position(|&e| e == item);
    if let Some(i) = idx {
        w.get_mut::<Backpack>(user).unwrap().items.remove(i);
        w.resource_mut::<UseQueue>().uses.push(WantsToUse { user, item, target: None, slot_idx: Some(i) });
    }
    item_system(w);
}

/// Puts `item` in `user`'s pack at floor position `(0, 0)`.
fn stash(w: &mut World, user: Entity, item: Entity) {
    w.entity_mut(item).remove::<Position>();
    w.get_mut::<Backpack>(user).unwrap().items.push(item);
}

#[test]
fn unidentified_potion_shows_its_appearance_not_its_true_name() {
    let mut w = test_world(1);
    let p = player(&mut w);
    let potion = w.spawn(PotionBundle::healing(Position { x: 0, y: 0 })).id();
    stash(&mut w, p, potion);

    let seen = display_name(&w, potion);
    assert_ne!(seen, "potion of healing");
    assert!(seen.ends_with(" potion"), "expected an appearance-based label, got {seen:?}");

    let appearance = w.resource::<ItemAppearances>().potions[&PotionEffect::Healing].clone();
    assert_eq!(seen, format!("{appearance} potion"));
}

#[test]
fn quaffing_a_potion_identifies_every_potion_of_that_type() {
    let mut w = test_world(1);
    let p = player(&mut w);
    let drunk = w.spawn(PotionBundle::healing(Position { x: 0, y: 0 })).id();
    let other = w.spawn(PotionBundle::healing(Position { x: 0, y: 0 })).id();
    stash(&mut w, p, drunk);
    stash(&mut w, p, other);

    assert!(!w.resource::<Identified>().potions.contains(&PotionEffect::Healing));

    use_item(&mut w, p, drunk);

    // Knowledge is global: the untouched sister potion is revealed too.
    assert!(w.resource::<Identified>().potions.contains(&PotionEffect::Healing));
    assert_eq!(display_name(&w, other), "potion of healing");

    let log = w.resource::<GameLog>();
    assert!(log.history.iter().any(|m| m.contains("That was a potion of healing!")));
}

#[test]
fn zapping_a_wand_identifies_it() {
    let mut w = test_world(3);
    let p = player(&mut w);
    let wand = w.spawn(WandBundle::fire(Position { x: 0, y: 0 })).id();
    stash(&mut w, p, wand);

    assert_ne!(display_name(&w, wand), "wand of fire");
    use_item(&mut w, p, wand);
    assert!(w.resource::<Identified>().wands.contains(&WandEffect::Fire));
}

#[test]
fn wearing_a_ring_identifies_it_and_toggles_like_gear() {
    let mut w = test_world(2);
    let p = player(&mut w);
    let ring = w.spawn(RingBundle::new(RingEffect::Regeneration, Position { x: 0, y: 0 })).id();
    stash(&mut w, p, ring);

    let unseen = display_name(&w, ring);
    assert_ne!(unseen, "ring of regeneration");
    assert!(unseen.ends_with(" ring"));

    use_item(&mut w, p, ring);
    assert!(w.resource::<Identified>().rings.contains(&RingEffect::Regeneration));
    assert_eq!(display_name(&w, ring), "ring of regeneration");
    assert_eq!(w.get::<PutOn>(ring).unwrap().bearer, Some(p));
    // Still in the pack, just worn.
    assert!(w.get::<Backpack>(p).unwrap().items.contains(&ring));

    // Using it again takes it off.
    use_item(&mut w, p, ring);
    assert_eq!(w.get::<PutOn>(ring).unwrap().bearer, None);
}

#[test]
fn scroll_of_identify_reveals_an_unknown_item_without_using_it() {
    let mut w = test_world(5);
    let p = player(&mut w);
    let potion = w.spawn(PotionBundle::poison(Position { x: 0, y: 0 })).id();
    let scroll = w.spawn(ScrollBundle::identify(Position { x: 0, y: 0 })).id();
    stash(&mut w, p, potion);
    stash(&mut w, p, scroll);

    // The player starts with an unidentified wand too (see `initialize_world`);
    // mark it known so it can't be the scroll's random pick instead of the potion.
    w.resource_mut::<Identified>().wands.insert(WandEffect::MagicMissile);

    use_item(&mut w, p, scroll);

    // The potion was never drunk, but its true type is now known.
    assert!(w.resource::<Identified>().potions.contains(&PotionEffect::Poison));
    assert_eq!(display_name(&w, potion), "potion of poison");
    assert!(w.get::<Potion>(potion).is_some(), "identify must not consume the target item");

    // Scroll of Identify identifies itself too, on the same read.
    assert!(w.resource::<Identified>().scrolls.contains(&ScrollEffect::Identify));
}
