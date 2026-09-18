//! Identification, narrowed to equipment: a weapon, suit of armour or
//! launcher hides its enchantment plus and cursed status until
//! [`KnownQuality`] says otherwise. Potions, scrolls, wands and rings carry no
//! hidden type at all — they are always shown by their true name.

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
/// of identify's reveal.
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
fn a_potion_shows_its_true_name_before_it_is_ever_used() {
    let mut w = test_world(1);
    let p = player(&mut w);
    let potion = spawn_potion(
        &mut w,
        PotionEffect::MonsterDetection,
        Position { x: 0, y: 0 },
    );
    stash(&mut w, p, potion);

    assert_eq!(display_name(&w, potion), "potion of monster detection");
}

#[test]
fn a_wand_shows_its_true_name_before_and_after_a_zap() {
    let mut w = test_world(3);
    let p = player(&mut w);
    let wand = spawn_wand(&mut w, WandEffect::Fire, Position { x: 0, y: 0 });
    stash(&mut w, p, wand);

    assert_eq!(display_name(&w, wand), "wand of fire");
    use_item(&mut w, p, wand);
    assert_eq!(display_name(&w, wand), "wand of fire");
}

#[test]
fn wearing_a_ring_toggles_it_like_any_other_gear() {
    let mut w = test_world(2);
    let p = player(&mut w);
    let ring = spawn_ring(&mut w, RingEffect::Regeneration, Position { x: 0, y: 0 });
    stash(&mut w, p, ring);

    assert_eq!(display_name(&w, ring), "ring of regeneration");

    use_item(&mut w, p, ring);
    assert_eq!(display_name(&w, ring), "ring of regeneration");
    assert_eq!(w.get::<Equipped>(ring).unwrap().by, Some(p));
    // Still in the pack, just worn.
    assert!(w.get::<Backpack>(p).unwrap().items.contains(&ring));

    // Using it again takes it off.
    use_item(&mut w, p, ring);
    assert_eq!(w.get::<Equipped>(ring).unwrap().by, None);
}

#[test]
fn a_cursed_weapon_hides_its_curse_until_worn_or_identified() {
    let mut w = test_world(4);
    let p = player(&mut w);
    let dagger = spawn_weapon(&mut w, "dagger", Position { x: 0, y: 0 });
    w.entity_mut(dagger).insert((PowerBonus(-2), Curse));
    stash(&mut w, p, dagger);

    assert_eq!(
        display_name(&w, dagger),
        "dagger",
        "the plus and curse must not leak before quality is known"
    );

    use_item(&mut w, p, dagger); // wielding it
    assert_eq!(display_name(&w, dagger), "-2 dagger (cursed)");
}

#[test]
fn scroll_of_identify_reveals_every_hidden_piece_of_gear_in_one_read() {
    let mut w = test_world(5);
    let p = player(&mut w);
    empty_pack(&mut w, p);

    let dagger = spawn_weapon(&mut w, "dagger", Position { x: 0, y: 0 });
    w.entity_mut(dagger).insert(Curse);
    let armor = spawn_armor(&mut w, "leather armor", Position { x: 0, y: 0 });
    w.entity_mut(armor).insert(ArmorBonus(2));
    let scroll = spawn_scroll(&mut w, ScrollEffect::Identify, Position { x: 0, y: 0 });
    stash(&mut w, p, dagger);
    stash(&mut w, p, armor);
    stash(&mut w, p, scroll);

    assert_eq!(display_name(&w, dagger), "dagger");
    assert_eq!(display_name(&w, armor), "leather armor");

    use_item(&mut w, p, scroll);

    // Both pieces of gear were revealed by the one read — not just one of them.
    assert_eq!(display_name(&w, dagger), "dagger (cursed)");
    assert_eq!(display_name(&w, armor), "+2 leather armor");
    assert!(
        w.get::<Name>(dagger).is_some(),
        "identify must not consume the target item"
    );

    let log = w.resource::<GameLog>();
    assert!(
        log.history
            .iter()
            .any(|m| m.contains("identifies everything in your pack"))
    );
}

#[test]
fn scroll_of_identify_says_so_when_the_pack_holds_nothing_hidden() {
    let mut w = test_world(6);
    let p = player(&mut w);
    empty_pack(&mut w, p);
    let scroll = spawn_scroll(&mut w, ScrollEffect::Identify, Position { x: 0, y: 0 });
    stash(&mut w, p, scroll);

    use_item(&mut w, p, scroll);

    let log = w.resource::<GameLog>();
    assert!(
        log.history
            .iter()
            .any(|m| m.contains("already recognise everything"))
    );
}
