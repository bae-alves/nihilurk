use bevy_ecs::prelude::*;
use models::*;

fn test_world(seed: u64) -> World {
    let mut w = World::new();
    w.insert_resource(GameRng(ChaCha12Rng::seed_from_u64(seed)));
    w.insert_resource(RngSeed(seed));
    w.init_resource::<GameLog>();
    w.init_resource::<UseQueue>();
    w.insert_resource(PlayerName {
        what: "TESTER".into(),
    });
    initialize_world(&mut w);
    w
}

fn player(w: &mut World) -> Entity {
    w.query_filtered::<Entity, With<Player>>().single(w)
}

/// Empties the player's starting kit and despawns it — the starting short bow
/// carries its own hidden +1 (never worn, so never identified), which would
/// otherwise compete with whatever a test stashes as a candidate for a scroll
/// of identify's random pick.
fn empty_pack(w: &mut World, p: Entity) {
    let items = std::mem::take(&mut w.get_mut::<Backpack>(p).unwrap().items);
    for item in items {
        w.despawn(item);
    }
}

/// Simulate the inventory "Use" action, exactly like the engine does.
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

/// Puts `item` in `user`'s pack at floor position `(0, 0)`.
fn stash(w: &mut World, user: Entity, item: Entity) {
    w.entity_mut(item).remove::<Position>();
    w.get_mut::<Backpack>(user).unwrap().items.push(item);
}

#[test]
fn unidentified_potion_shows_its_appearance_not_its_true_name() {
    let mut w = test_world(1);
    let p = player(&mut w);
    let potion = spawn_potion(
        &mut w,
        PotionEffect::MonsterDetection,
        Position { x: 0, y: 0 },
    );
    stash(&mut w, p, potion);

    let seen = display_name(&w, potion);
    assert_ne!(seen, "potion of monster detection");
    assert!(
        seen.ends_with(" potion"),
        "expected an appearance-based label, got {seen:?}"
    );

    let appearance =
        w.resource::<ItemAppearances>().potions[&PotionEffect::MonsterDetection].clone();
    assert_eq!(seen, format!("{appearance} potion"));
}

#[test]
fn quaffing_a_potion_identifies_every_potion_of_that_type() {
    let mut w = test_world(1);
    let p = player(&mut w);
    let drunk = spawn_potion(
        &mut w,
        PotionEffect::MonsterDetection,
        Position { x: 0, y: 0 },
    );
    let other = spawn_potion(
        &mut w,
        PotionEffect::MonsterDetection,
        Position { x: 0, y: 0 },
    );
    stash(&mut w, p, drunk);
    stash(&mut w, p, other);

    assert!(
        !w.resource::<Identified>()
            .potions
            .contains(&PotionEffect::MonsterDetection)
    );

    use_item(&mut w, p, drunk);

    // Knowledge is global: the untouched sister potion is revealed too.
    assert!(
        w.resource::<Identified>()
            .potions
            .contains(&PotionEffect::MonsterDetection)
    );
    assert_eq!(display_name(&w, other), "potion of monster detection");

    let log = w.resource::<GameLog>();
    assert!(
        log.history
            .iter()
            .any(|m| m.contains("That was a potion of monster detection!"))
    );
}

#[test]
fn zapping_a_wand_identifies_it() {
    let mut w = test_world(3);
    let p = player(&mut w);
    let wand = spawn_wand(&mut w, WandEffect::Fire, Position { x: 0, y: 0 });
    stash(&mut w, p, wand);

    assert_ne!(display_name(&w, wand), "wand of fire");
    use_item(&mut w, p, wand);
    assert!(w.resource::<Identified>().wands.contains(&WandEffect::Fire));
}

#[test]
fn wearing_a_ring_identifies_it_and_toggles_like_gear() {
    let mut w = test_world(2);
    let p = player(&mut w);
    let ring = spawn_ring(&mut w, RingEffect::Regeneration, Position { x: 0, y: 0 });
    stash(&mut w, p, ring);

    let unseen = display_name(&w, ring);
    assert_ne!(unseen, "ring of regeneration");
    assert!(unseen.ends_with(" ring"));

    use_item(&mut w, p, ring);
    assert!(
        w.resource::<Identified>()
            .rings
            .contains(&RingEffect::Regeneration)
    );
    assert_eq!(display_name(&w, ring), "ring of regeneration");
    assert_eq!(w.get::<Equipped>(ring).unwrap().by, Some(p));
    // Still in the pack, just worn.
    assert!(w.get::<Backpack>(p).unwrap().items.contains(&ring));

    // Using it again takes it off.
    use_item(&mut w, p, ring);
    assert_eq!(w.get::<Equipped>(ring).unwrap().by, None);
}

#[test]
fn scroll_of_identify_reveals_an_unknown_item_without_using_it() {
    let mut w = test_world(5);
    let p = player(&mut w);
    empty_pack(&mut w, p);
    let potion = spawn_potion(&mut w, PotionEffect::Poison, Position { x: 0, y: 0 });
    let scroll = spawn_scroll(&mut w, ScrollEffect::Identify, Position { x: 0, y: 0 });
    stash(&mut w, p, potion);
    stash(&mut w, p, scroll);

    use_item(&mut w, p, scroll);

    // The potion was never drunk, but its true type is now known.
    assert!(
        w.resource::<Identified>()
            .potions
            .contains(&PotionEffect::Poison)
    );
    assert_eq!(display_name(&w, potion), "potion of poison");
    assert!(
        w.get::<Potion>(potion).is_some(),
        "identify must not consume the target item"
    );

    // Scroll of Identify identifies itself too, on the same read.
    assert!(
        w.resource::<Identified>()
            .scrolls
            .contains(&ScrollEffect::Identify)
    );
}
