//! Decks: an ordered stack of potion and scroll effects. Reading one plays the
//! top effect on the reader; throwing it bursts like a wand of light.
//!
//! Every test builds its deck by hand and stands the player in a bare room, so
//! nothing the floor rolled can get in the way.

use crate::common;
use crate::common::fixtures::{potion_row, scroll_row};
use crate::common::monster::monster;

use bevy_ecs::prelude::*;
use fixedbitset::FixedBitSet;
use models::constants::decks::DECK_SIZE;
use models::constants::potions::GAIN_STRENGTH_POWER;
use models::*;

const UP: (u16, u16) = (11, 6);
const DOWN: (u16, u16) = (38, 14);
const HERE: Position = Position { x: 20, y: 10 };

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
    bare_room(&mut w);
    w
}

/// One lit room, x 10..=39 by y 5..=15, with a stair at each end, nothing in
/// it but the player at [`HERE`], who sees all of it.
fn bare_room(w: &mut World) {
    let doomed: Vec<Entity> = w
        .iter_entities()
        .filter(|e| !e.contains::<Player>() && e.contains::<Position>())
        .map(|e| e.id())
        .collect();
    for e in doomed {
        w.despawn(e);
    }
    let mut map = Map {
        tiles: vec![TileType::Wall; MAP_TILE_COUNT],
        dark: FixedBitSet::with_capacity(MAP_TILE_COUNT),
        special: vec![None; MAP_TILE_COUNT],
        level: None,
    };
    let mut seen = Vec::new();
    for y in 5..=15 {
        for x in 10..=39 {
            map.tiles[tile_index(x, y)] = TileType::Room;
            seen.push((x, y));
        }
    }
    map.tiles[tile_index(UP.0, UP.1)] = TileType::Upstairs;
    map.tiles[tile_index(DOWN.0, DOWN.1)] = TileType::Downstairs;
    w.insert_resource(map);
    let p = player(w);
    *w.get_mut::<Position>(p).unwrap() = HERE;
    w.get_mut::<Viewshed>(p).unwrap().visible_tiles = seen;
}

fn player(w: &mut World) -> Entity {
    w.query_filtered::<Entity, With<Player>>().single(w)
}

/// The effect a row name stands for.
fn fx(name: &str) -> DeckEffect {
    potion_row(name)
        .map(DeckEffect::Potion)
        .or_else(|| scroll_row(name).map(DeckEffect::Scroll))
        .expect(name)
}

/// A deck in the player's pack holding `effects`, bottom first.
fn deck_in_pack(w: &mut World, effects: &[DeckEffect]) -> Entity {
    let deck = DECKS[0].spawn(w, HERE);
    w.entity_mut(deck).remove::<Position>().insert(Deck {
        effects: effects.to_vec(),
    });
    let p = player(w);
    w.get_mut::<Backpack>(p).unwrap().items.push(deck);
    deck
}

/// The engine's "Use": onto the queue, through the system.
fn use_item(w: &mut World, item: Entity) {
    let p = player(w);
    let bp = w.get::<Backpack>(p).unwrap();
    let i = bp.items.iter().position(|&e| e == item).unwrap();
    w.resource_mut::<UseQueue>().uses.push(WantsToUse {
        user: p,
        item,
        target: None,
        slot_idx: Some(i),
    });
    item_system(w);
}

/// The engine's "Throw": out of the pack, at `target`.
fn throw(w: &mut World, item: Entity, target: Position) {
    let p = player(w);
    w.get_mut::<Backpack>(p)
        .unwrap()
        .items
        .retain(|&e| e != item);
    w.resource_mut::<ThrowQueue>().throws.push(WantsToThrow {
        thrower: p,
        item,
        target,
        slot_idx: None,
    });
    throw_system(w);
}

fn in_pack(w: &mut World, item: Entity) -> bool {
    let p = player(w);
    w.get::<Backpack>(p).unwrap().items.contains(&item)
}

fn power(w: &mut World) -> i32 {
    let p = player(w);
    w.get::<Fighter>(p).unwrap().power
}

#[test]
fn reading_plays_only_the_top_effect() {
    let mut w = test_world(1);
    let before = power(&mut w);
    let deck = deck_in_pack(
        &mut w,
        &[fx("potion of gain strength"), fx("potion of fruit juice")],
    );
    use_item(&mut w, deck);
    assert!(in_pack(&mut w, deck));
    assert_eq!(
        w.get::<Deck>(deck).unwrap().effects,
        vec![fx("potion of gain strength")]
    );
    assert_eq!(power(&mut w), before, "the strength is still underneath");
}

#[test]
fn the_effect_plays_on_the_reader_and_the_last_one_spends_the_deck() {
    let mut w = test_world(2);
    let before = power(&mut w);
    let deck = deck_in_pack(&mut w, &[fx("potion of gain strength")]);
    use_item(&mut w, deck);
    assert_eq!(power(&mut w), before + GAIN_STRENGTH_POWER);
    assert!(w.get_entity(deck).is_none(), "an empty deck is gone");
    assert!(!in_pack(&mut w, deck));
}

#[test]
fn a_scroll_effect_plays_like_the_scroll() {
    let mut w = test_world(3);
    let p = player(&mut w);
    let weapon = w
        .get::<Backpack>(p)
        .unwrap()
        .items
        .iter()
        .copied()
        .find(|&e| w.get::<Equipped>(e).is_some_and(|q| q.slot == Slot::Hand))
        .unwrap();
    let bonus = |w: &World| w.get::<PowerBonus>(weapon).map_or(0, |b| b.0);
    let before = bonus(&w);
    let deck = deck_in_pack(&mut w, &[fx("scroll of enchant weapon")]);
    use_item(&mut w, deck);
    assert_eq!(bonus(&w), before + 1);
}

#[test]
fn a_rolled_deck_holds_deck_size_effects_that_drop() {
    let mut rng = ChaCha12Rng::seed_from_u64(7);
    for _ in 0..50 {
        let deck = roll_deck(&mut rng);
        assert_eq!(deck.effects.len(), DECK_SIZE);
        assert!(deck.effects.iter().all(|e| e.weight() > 0));
    }
}

#[test]
fn a_thrown_deck_bursts_like_a_wand_of_light_and_plays_nothing() {
    let mut w = test_world(6);
    let before = power(&mut w);
    let orc = monster(&mut w, "orc", Position { x: 21, y: 10 });
    w.get_mut::<Fighter>(orc).unwrap().hp = 40;
    let deck = deck_in_pack(&mut w, &[fx("potion of gain strength"); DECK_SIZE]);
    throw(&mut w, deck, Position { x: 21, y: 10 });

    assert!(w.get_entity(deck).is_none(), "the deck is spent");
    assert!(w.get::<Fighter>(orc).unwrap().hp < 40, "the flash burns");
    assert!(matches!(
        w.get::<Mob>(orc).unwrap().movement_type,
        MovementType::Confused
    ));
    assert_eq!(power(&mut w), before, "the stacked effects are lost");
}

#[test]
fn a_deck_survives_a_save() {
    let mut w = test_world(4);
    let pool = deck_pool();
    let effects = [pool[0], pool[pool.len() - 1]];
    let deck = deck_in_pack(&mut w, &effects);
    let save = common::SaveFile::new("deck");
    save_game(&mut w, save.path()).unwrap();
    let mut w2 = World::new();
    w2.insert_resource(GameRng(ChaCha12Rng::seed_from_u64(3)));
    w2.insert_resource(RngSeed(3));
    w2.init_resource::<GameLog>();
    w2.insert_resource(PlayerName { what: "X".into() });
    load_game(&mut w2, save.path()).unwrap();
    let _ = deck;
    let loaded: Vec<Vec<DeckEffect>> = w2
        .query::<&Deck>()
        .iter(&w2)
        .map(|d| d.effects.clone())
        .collect();
    assert!(loaded.contains(&effects.to_vec()));
}
