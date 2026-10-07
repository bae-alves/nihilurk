//! Decks of cards: reading one card off the top, every card either way up, a
//! thrown deck played as a poker hand, and THE WORLD's stopped time.
//!
//! Every test builds its deck by hand and stands the player in a bare room, so
//! nothing the floor rolled can get in the way.

mod common;
#[path = "common/monster.rs"]
#[allow(dead_code)] // only the handless fixture is wanted here
mod monster;

use bevy_ecs::prelude::*;
use fixedbitset::FixedBitSet;
use models::constants::decks::{CARD_CHAIN_CAP, DECK_SIZE, REVERSED_MAX, REVERSED_MIN};
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
        inert_doors: FixedBitSet::with_capacity(MAP_TILE_COUNT),
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

fn up(face: CardFace) -> Card {
    Card {
        face,
        reversed: false,
    }
}

fn down(face: CardFace) -> Card {
    Card {
        face,
        reversed: true,
    }
}

/// A deck in the player's pack holding `cards`, bottom first.
fn deck_in_pack(w: &mut World, cards: &[Card]) -> Entity {
    let deck = spawn_named(w, "deck of cards", HERE).unwrap();
    w.entity_mut(deck).remove::<Position>().insert(Deck {
        cards: cards.to_vec(),
    });
    let p = player(w);
    w.get_mut::<Backpack>(p).unwrap().items.push(deck);
    deck
}

/// The engine's "Use": out of the pack, onto the queue, through the system.
fn use_item(w: &mut World, item: Entity) {
    let p = player(w);
    let mut bp = w.get_mut::<Backpack>(p).unwrap();
    let i = bp.items.iter().position(|&e| e == item).unwrap();
    bp.items.remove(i);
    w.resource_mut::<UseQueue>().uses.push(WantsToUse {
        user: p,
        item,
        target: None,
        slot_idx: Some(i),
    });
    item_system(w);
}

/// Reads a one-card deck.
fn read(w: &mut World, card: Card) {
    let deck = deck_in_pack(w, &[card]);
    use_item(w, deck);
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
    });
    throw_system(w);
}

fn throw_hand(w: &mut World, cards: &[Card]) {
    let deck = deck_in_pack(w, cards);
    throw(w, deck, Position { x: 25, y: 10 });
}

fn in_pack(w: &mut World, item: Entity) -> bool {
    let p = player(w);
    w.get::<Backpack>(p).unwrap().items.contains(&item)
}

fn log_has(w: &World, line: &str) -> bool {
    w.resource::<GameLog>().history.iter().any(|l| l == line)
}

fn draws(w: &World) -> usize {
    w.resource::<GameLog>()
        .history
        .iter()
        .filter(|l| l.starts_with("You draw "))
        .count()
}

fn wielded(w: &mut World) -> Entity {
    let p = player(w);
    w.get::<Backpack>(p)
        .unwrap()
        .items
        .iter()
        .copied()
        .find(|&e| {
            w.get::<Equipped>(e)
                .is_some_and(|q| q.slot == Slot::Hand && q.by == Some(p))
        })
        .unwrap()
}

fn plus(w: &World, item: Entity) -> i32 {
    w.get::<PowerBonus>(item).map_or(0, |b| b.0)
        + w.get::<ArmorBonus>(item).map_or(0, |b| b.0)
        + w.get::<ThrowBonus>(item).map_or(0, |b| b.0)
}

// ---------------------------------------------------------------------------
// The deck itself
// ---------------------------------------------------------------------------

#[test]
fn reading_plays_only_the_top_card() {
    let mut w = test_world(1);
    let deck = deck_in_pack(&mut w, &[up(CardFace::Eyes), up(CardFace::Fool)]);
    use_item(&mut w, deck);
    assert!(in_pack(&mut w, deck));
    assert_eq!(w.get::<Deck>(deck).unwrap().cards, vec![up(CardFace::Eyes)]);
    assert_eq!(draws(&w), 1);
}

#[test]
fn a_deck_is_gone_after_its_last_card() {
    let mut w = test_world(1);
    let deck = deck_in_pack(&mut w, &[up(CardFace::Fool)]);
    use_item(&mut w, deck);
    assert!(w.get_entity(deck).is_none());
}

#[test]
fn a_rolled_deck_is_full_with_one_or_two_reversed() {
    for seed in 0..50 {
        let deck = roll_deck(&mut ChaCha12Rng::seed_from_u64(seed));
        assert_eq!(deck.cards.len(), DECK_SIZE);
        let flipped = deck.cards.iter().filter(|c| c.reversed).count();
        assert!(
            (REVERSED_MIN..=REVERSED_MAX).contains(&flipped),
            "{flipped}"
        );
    }
}

#[test]
fn a_decks_cards_survive_a_save() {
    let mut w = test_world(2);
    let cards = [
        down(CardFace::Joker),
        up(CardFace::World),
        up(CardFace::Fool),
    ];
    deck_in_pack(&mut w, &cards);
    let save = common::SaveFile::new("deck");
    save_game(&mut w, save.path()).unwrap();

    let mut w2 = World::new();
    w2.insert_resource(GameRng(ChaCha12Rng::seed_from_u64(3)));
    w2.insert_resource(RngSeed(3));
    w2.init_resource::<GameLog>();
    w2.insert_resource(PlayerName { what: "X".into() });
    load_game(&mut w2, save.path()).unwrap();
    let loaded = w2.query::<&Deck>().single(&w2);
    assert_eq!(loaded.cards, cards.to_vec());
}

#[test]
fn a_thrown_deck_is_spent() {
    let mut w = test_world(1);
    let deck = deck_in_pack(&mut w, &[up(CardFace::Fool), up(CardFace::Eyes)]);
    throw(&mut w, deck, Position { x: 25, y: 10 });
    assert!(w.get_entity(deck).is_none());
}

// ---------------------------------------------------------------------------
// The cards
// ---------------------------------------------------------------------------

#[test]
fn the_prince_enchants_the_weapon_and_reversed_dulls_it() {
    let mut w = test_world(1);
    let mace = wielded(&mut w);
    let before = plus(&w, mace);
    read(&mut w, up(CardFace::PrinceOfSwords));
    assert!(plus(&w, mace) > before);
    let enchanted = plus(&w, mace);
    read(&mut w, down(CardFace::PrinceOfSwords));
    assert_eq!(plus(&w, mace), enchanted - 1);
}

#[test]
fn the_reversed_king_empties_wands_and_an_empty_wand_crumbles_unfired() {
    let mut w = test_world(1);
    let p = player(&mut w);
    let wand = spawn_named(&mut w, "wand of striking", HERE).unwrap();
    w.entity_mut(wand).remove::<Position>();
    w.get_mut::<Backpack>(p).unwrap().items.push(wand);
    read(&mut w, down(CardFace::KingOfClubs));
    assert_eq!(w.get::<Battery>(wand).unwrap().charges, 0);

    let foe = monster::plain_monster(&mut w, "dummy", Position { x: 21, y: 10 });
    w.get_mut::<Backpack>(p)
        .unwrap()
        .items
        .retain(|&e| e != wand);
    w.resource_mut::<UseQueue>().uses.push(WantsToUse {
        user: p,
        item: wand,
        target: Some(Position { x: 21, y: 10 }),
        slot_idx: None,
    });
    item_system(&mut w);
    assert!(w.get_entity(wand).is_none());
    assert_eq!(w.get::<Fighter>(foe).unwrap().hp, 100);

    let wand = spawn_named(&mut w, "wand of striking", HERE).unwrap();
    w.entity_mut(wand).remove::<Position>();
    w.get_mut::<Battery>(wand).unwrap().charges = 1;
    w.get_mut::<Backpack>(p).unwrap().items.push(wand);
    read(&mut w, up(CardFace::KingOfClubs));
    assert!(w.get::<Battery>(wand).unwrap().charges > 1);
}

#[test]
fn the_child_and_the_crone_lead_to_the_stairs() {
    let mut w = test_world(1);
    let p = player(&mut w);
    let at = |w: &World| {
        let pos = w.get::<Position>(p).unwrap();
        (pos.x, pos.y)
    };
    read(&mut w, up(CardFace::Child));
    assert_eq!(at(&w), UP);
    read(&mut w, up(CardFace::Crone));
    assert_eq!(at(&w), DOWN);
    read(&mut w, down(CardFace::Crone));
    assert_eq!(at(&w), UP);
    read(&mut w, down(CardFace::Child));
    assert_eq!(at(&w), DOWN);
}

#[test]
fn the_excuse_confuses_foes_and_spares_allies() {
    let mut w = test_world(1);
    let foe = monster::plain_monster(&mut w, "foe", Position { x: 25, y: 10 });
    let friend = monster::plain_monster(&mut w, "friend", Position { x: 25, y: 12 });
    charm(&mut w, friend);
    read(&mut w, up(CardFace::Excuse));
    let reeling = |w: &World, e: Entity| {
        matches!(
            w.get::<Mob>(e).unwrap().movement_type,
            MovementType::Confused
        )
    };
    assert!(reeling(&w, foe));
    assert!(!reeling(&w, friend));

    read(&mut w, down(CardFace::Excuse));
    let p = player(&mut w);
    assert!(w.get::<Confused>(p).is_some());
}

#[test]
fn the_eyes_show_whats_left_top_first() {
    let mut w = test_world(1);
    let deck = deck_in_pack(
        &mut w,
        &[down(CardFace::Bole), up(CardFace::Fool), up(CardFace::Eyes)],
    );
    use_item(&mut w, deck);
    assert!(log_has(
        &w,
        "The deck holds, top first: FOOL, The Bole (reversed)."
    ));
}

#[test]
fn the_balance_raises_the_swing() {
    let swing = |bala: bool| {
        let mut total = 0;
        for seed in 0..40 {
            let mut w = test_world(seed);
            let p = player(&mut w);
            if bala {
                read(&mut w, up(CardFace::Balance));
            }
            let foe = monster::plain_monster(&mut w, "dummy", Position { x: 21, y: 10 });
            w.insert_resource(GameRng(ChaCha12Rng::seed_from_u64(seed)));
            resolve_attack(&mut w, p, foe);
            total += 100 - w.get::<Fighter>(foe).unwrap().hp;
        }
        total
    };
    assert!(swing(true) > swing(false));
}

#[test]
fn the_bole_raises_the_guard() {
    let taken = |bole: bool| {
        let mut total = 0;
        for seed in 0..40 {
            let mut w = test_world(seed);
            let p = player(&mut w);
            if bole {
                read(&mut w, up(CardFace::Bole));
            }
            let foe = monster::plain_monster(&mut w, "brute", Position { x: 21, y: 10 });
            w.get_mut::<Fighter>(foe).unwrap().power = 12;
            w.get_mut::<Fighter>(p).unwrap().hp = 1000;
            w.insert_resource(GameRng(ChaCha12Rng::seed_from_u64(seed)));
            resolve_attack(&mut w, foe, p);
            total += 1000 - w.get::<Fighter>(p).unwrap().hp;
        }
        total
    };
    assert!(taken(true) < taken(false));
}

#[test]
fn the_chariot_makes_every_blow_excellent_either_way_up() {
    for seed in 0..10 {
        let mut w = test_world(seed);
        let p = player(&mut w);
        read(&mut w, up(CardFace::Chariot));
        let foe = monster::plain_monster(&mut w, "dummy", Position { x: 21, y: 10 });
        resolve_attack(&mut w, p, foe);
        let last = w.resource::<GameLog>().history.last().unwrap().clone();
        assert!(last.contains("excellent"), "{last}");
    }
    for seed in 0..10 {
        let mut w = test_world(seed);
        let p = player(&mut w);
        let foe = monster::plain_monster(&mut w, "dummy", Position { x: 21, y: 10 });
        lend(&mut w, foe, Grant::of::<Oof>(), Lifetime::Floor);
        resolve_attack(&mut w, p, foe);
        let last = w.resource::<GameLog>().history.last().unwrap().clone();
        assert!(last.contains("excellent"), "{last}");
    }
}

#[test]
fn the_ledgers_survive_a_save() {
    let mut w = test_world(4);
    for face in [CardFace::Balance, CardFace::Bole, CardFace::Chariot] {
        read(&mut w, up(face));
    }
    let save = common::SaveFile::new("ledgers");
    save_game(&mut w, save.path()).unwrap();
    let mut w2 = World::new();
    w2.insert_resource(GameRng(ChaCha12Rng::seed_from_u64(3)));
    w2.insert_resource(RngSeed(3));
    w2.init_resource::<GameLog>();
    w2.insert_resource(PlayerName { what: "X".into() });
    load_game(&mut w2, save.path()).unwrap();
    let p = player(&mut w2);
    assert!(w2.get::<Bala>(p).is_some());
    assert!(w2.get::<Bole>(p).is_some());
    assert!(w2.get::<Crit>(p).is_some());
}

/// A worn ring of protection, or of stealth: one with a number, one without.
fn worn_ring(w: &mut World, name: &str) -> Entity {
    let p = player(w);
    let ring = spawn_named(w, name, HERE).unwrap();
    w.entity_mut(ring).remove::<Position>();
    w.get_mut::<Backpack>(p).unwrap().items.push(ring);
    assert!(equip_silently(w, p, ring));
    ring
}

#[test]
fn the_princess_enchants_a_numeric_ring() {
    let mut w = test_world(1);
    let ring = worn_ring(&mut w, "ring of protection");
    let before = plus(&w, ring);
    read(&mut w, up(CardFace::PrincessOfDiamonds));
    assert_eq!(plus(&w, ring), before + 1);
    read(&mut w, down(CardFace::PrincessOfDiamonds));
    assert_eq!(plus(&w, ring), before);
}

#[test]
fn the_princess_passes_over_a_ring_with_no_number() {
    let mut w = test_world(1);
    let ring = worn_ring(&mut w, "ring of stealth");
    read(&mut w, up(CardFace::PrincessOfDiamonds));
    assert!(w.get_entity(ring).is_some());
    read(&mut w, down(CardFace::PrincessOfDiamonds));
    assert!(w.get_entity(ring).is_none());
}

#[test]
fn the_reversed_princess_breaks_a_ring_at_zero() {
    let mut w = test_world(1);
    let ring = worn_ring(&mut w, "ring of protection");
    w.entity_mut(ring).insert(ArmorBonus(0));
    read(&mut w, down(CardFace::PrincessOfDiamonds));
    assert!(w.get_entity(ring).is_none());
    assert!(!in_pack(&mut w, ring));
}

// ---------------------------------------------------------------------------
// Summons and the golden wind
// ---------------------------------------------------------------------------

fn summoned(w: &mut World, name: &str) -> Entity {
    let found: Vec<Entity> = w
        .query::<(Entity, &Name)>()
        .iter(w)
        .filter(|(_, n)| n.what == name)
        .map(|(e, _)| e)
        .collect();
    assert_eq!(found.len(), 1, "{name}");
    found[0]
}

#[test]
fn the_skull_king_comes_as_an_ally_and_the_black_mage_as_the_helper() {
    let mut w = test_world(1);
    read(&mut w, up(CardFace::SkullKing));
    let king = summoned(&mut w, "skull king");
    assert_eq!(*w.get::<Faction>(king).unwrap(), Faction::Ally);
    assert!(w.get::<Helper>(king).is_none());

    read(&mut w, up(CardFace::BlackMage));
    let mage = summoned(&mut w, "black mage");
    assert!(w.get::<Helper>(mage).is_some());
}

#[test]
fn reversed_they_come_as_foes() {
    let mut w = test_world(1);
    read(&mut w, down(CardFace::SkullKing));
    read(&mut w, down(CardFace::BlackMage));
    let king = summoned(&mut w, "skull king");
    let mage = summoned(&mut w, "black mage");
    assert_eq!(*w.get::<Faction>(king).unwrap(), Faction::Monster);
    assert_eq!(*w.get::<Faction>(mage).unwrap(), Faction::Monster);
}

#[test]
fn summons_never_roam_but_are_known_by_name() {
    for name in ["skull king", "black mage"] {
        assert!(MonsterDef::lookup(name).is_some());
        assert!(MonsterDef::is_species_name(name));
        assert!(BESTIARY.iter().all(|m| m.name != name));
    }
}

#[test]
fn the_golden_wind_wakes_the_floor_and_leaves_the_element() {
    let mut w = test_world(1);
    let dagger = spawn_named(&mut w, "dagger", Position { x: 30, y: 8 }).unwrap();
    let trap = spawn_named(&mut w, "bear trap", Position { x: 32, y: 12 }).unwrap();
    let element = spawn_named(&mut w, "Element of Yoord", Position { x: 34, y: 8 })
        .or_else(|| Some(spawn_element_of_yoord(&mut w, Position { x: 34, y: 8 })))
        .unwrap();
    read(&mut w, up(CardFace::GoldenWind));
    assert!(w.get_entity(dagger).is_none());
    assert!(w.get_entity(trap).is_none());
    assert!(w.get_entity(element).is_some());
    let allies: Vec<Position> = w
        .query::<(&Position, &Faction, &Mob)>()
        .iter(&w)
        .filter(|(_, f, _)| **f == Faction::Ally)
        .map(|(p, _, _)| *p)
        .collect();
    assert!(allies.contains(&Position { x: 30, y: 8 }));
    assert!(allies.contains(&Position { x: 32, y: 12 }));
}

// ---------------------------------------------------------------------------
// Hands
// ---------------------------------------------------------------------------

fn hand(faces: &[CardFace]) -> Option<Hand> {
    let cards: Vec<Card> = faces.iter().map(|&f| up(f)).collect();
    score_hand(&cards)
}

#[test]
fn hands_score_like_poker_with_the_fool_wild() {
    use CardFace::*;
    let rank = |faces: &[CardFace]| hand(faces).unwrap().rank;
    assert_eq!(rank(&[Joker, Bole, Balance, Eyes, Child]), Rank::AntiFlush);
    assert_eq!(rank(&[Bole, Bole, Balance, Eyes, Child]), Rank::Pair);
    assert_eq!(rank(&[Bole, Bole, Eyes, Eyes, Child]), Rank::TwoPair);
    assert_eq!(rank(&[Bole, Bole, Bole, Eyes, Child]), Rank::ThreeOfAKind);
    assert_eq!(rank(&[Bole, Bole, Bole, Eyes, Eyes]), Rank::FullHouse);
    assert_eq!(rank(&[Bole, Bole, Bole, Bole, Eyes]), Rank::FourOfAKind);
    assert_eq!(rank(&[Bole, Bole, Bole, Bole, Bole]), Rank::FiveFlush);
    assert_eq!(rank(&[Fool, Bole, Balance, Eyes, Child]), Rank::Pair);
    assert_eq!(rank(&[Fool, Bole, Bole, Eyes, Eyes]), Rank::FullHouse);
    assert_eq!(rank(&[Fool, Fool, Bole, Eyes, Child]), Rank::ThreeOfAKind);
    assert_eq!(rank(&[Fool, Fool, Fool, Fool, Fool]), Rank::FiveFlush);
    assert_eq!(rank(&[Fool, Fool, Fool, Fool, Eyes]), Rank::FiveFlush);
    assert_eq!(rank(&[Bole]), Rank::AntiFlush);
    assert_eq!(rank(&[Bole, Fool]), Rank::Pair);
    assert!(hand(&[]).is_none());
    assert_eq!(
        hand(&[Eyes, Bole, Bole, Eyes, Child]).unwrap().plays,
        vec![(Eyes, 2), (Bole, 2)]
    );
}

#[test]
fn a_thrown_pair_plays_its_card_twice_and_never_reversed() {
    let mut w = test_world(1);
    let mace = wielded(&mut w);
    let before = plus(&w, mace);
    throw_hand(
        &mut w,
        &[
            down(CardFace::PrinceOfSwords),
            down(CardFace::PrinceOfSwords),
            up(CardFace::Bole),
            up(CardFace::Child),
            up(CardFace::Excuse),
        ],
    );
    assert!(plus(&w, mace) >= before + 2);
    assert!(log_has(&w, "A Pair (1000 points)"));
}

#[test]
fn a_hand_pays_its_points_and_never_doubles_the_score() {
    let mut w = test_world(1);
    let p = player(&mut w);
    w.get_mut::<Score>(p).unwrap().value = 1000;
    throw_hand(&mut w, &[up(CardFace::Fool); 5]);
    assert_eq!(
        w.get::<Score>(p).unwrap().value,
        1000 + Rank::FiveFlush.points() as i64
    );
}

#[test]
fn a_five_flush_puts_the_element_in_the_pack() {
    let mut w = test_world(1);
    throw_hand(&mut w, &[up(CardFace::Fool); 5]);
    assert!(holding_element_of_yoord(&mut w));
    let p = player(&mut w);
    let kept = w.get::<Backpack>(p).unwrap().items.len();
    throw_hand(&mut w, &[up(CardFace::Fool); 5]);
    assert_eq!(w.get::<Backpack>(p).unwrap().items.len(), kept);
}

#[test]
fn a_five_flush_into_a_full_pack_leaves_only_the_element() {
    let mut w = test_world(1);
    let p = player(&mut w);
    while w.get::<Backpack>(p).unwrap().items.len() < models::constants::items::PACK_CAPACITY {
        let potion = spawn_named(&mut w, "potion of healing", HERE).unwrap();
        w.entity_mut(potion).remove::<Position>();
        w.get_mut::<Backpack>(p).unwrap().items.push(potion);
    }
    throw_hand(&mut w, &[up(CardFace::Fool); 5]);
    let pack = w.get::<Backpack>(p).unwrap().items.clone();
    assert_eq!(pack.len(), 1);
    assert!(w.get::<Amulet>(pack[0]).is_some());
}

#[test]
fn a_jester_in_a_hand_finds_nothing_to_play() {
    let mut w = test_world(1);
    throw_hand(
        &mut w,
        &[
            up(CardFace::Jester),
            up(CardFace::Jester),
            up(CardFace::Jester),
            up(CardFace::Eyes),
            up(CardFace::Child),
        ],
    );
    assert!(log_has(&w, "Three of a Kind (100000 points)"));
}

#[test]
fn a_read_jester_plays_the_rest_of_the_deck_as_a_hand() {
    let mut w = test_world(1);
    let deck = deck_in_pack(
        &mut w,
        &[up(CardFace::Fool), up(CardFace::Fool), up(CardFace::Jester)],
    );
    use_item(&mut w, deck);
    assert!(w.get_entity(deck).is_none());
    assert!(log_has(&w, "A Pair (1000 points)"));
}

#[test]
fn the_joker_never_plays_itself_and_chains_stop_at_the_cap() {
    for seed in 0..30 {
        let mut w = test_world(seed);
        read(&mut w, up(CardFace::Joker));
        let lines = w.resource::<GameLog>().history.clone();
        let drawn: Vec<&String> = lines
            .iter()
            .filter(|l| l.starts_with("You draw "))
            .collect();
        assert!(drawn.len() >= 2);
        assert!(!drawn[1].contains("Joker"));

        let mut w = test_world(seed);
        read(&mut w, up(CardFace::PlusFour));
        assert!(draws(&w) <= CARD_CHAIN_CAP as usize);
    }
}

// ---------------------------------------------------------------------------
// THE WORLD
// ---------------------------------------------------------------------------

fn turn(w: &mut World) {
    tick_effects(w);
    item_system(w);
    throw_system(w);
    ai(w);
}

#[test]
fn the_world_holds_the_floor_still_for_its_turns() {
    let mut w = test_world(1);
    let foe = monster::plain_monster(&mut w, "foe", Position { x: 26, y: 10 });
    let deck = deck_in_pack(&mut w, &[up(CardFace::World)]);
    let p = player(&mut w);
    let bp_i = w
        .get::<Backpack>(p)
        .unwrap()
        .items
        .iter()
        .position(|&e| e == deck);
    w.get_mut::<Backpack>(p)
        .unwrap()
        .items
        .retain(|&e| e != deck);
    w.resource_mut::<UseQueue>().uses.push(WantsToUse {
        user: p,
        item: deck,
        target: None,
        slot_idx: bp_i,
    });
    let start = *w.get::<Position>(foe).unwrap();
    for _ in 0..models::constants::decks::WORLD_TURNS {
        turn(&mut w);
        assert_eq!(*w.get::<Position>(foe).unwrap(), start);
    }
    turn(&mut w);
    assert_ne!(*w.get::<Position>(foe).unwrap(), start);
}

#[test]
fn a_throw_in_stopped_time_flies_from_where_it_left_the_hand() {
    let mut w = test_world(1);
    let foe = monster::plain_monster(&mut w, "foe", Position { x: 23, y: 10 });
    w.get_mut::<Fighter>(foe).unwrap().armor = 0;
    read(&mut w, up(CardFace::World));
    let p = player(&mut w);
    let dagger = spawn_named(&mut w, "dagger", HERE).unwrap();
    w.entity_mut(dagger).remove::<Position>();
    w.get_mut::<Backpack>(p).unwrap().items.push(dagger);
    throw(&mut w, dagger, Position { x: 23, y: 10 });
    assert!(w.get::<Position>(dagger).is_none());
    assert_eq!(w.get::<Fighter>(foe).unwrap().hp, 100);

    *w.get_mut::<Position>(p).unwrap() = Position { x: 20, y: 14 };
    for _ in 0..=models::constants::decks::WORLD_TURNS {
        turn(&mut w);
    }
    assert!(w.get::<Fighter>(foe).unwrap().hp < 100);
}

#[test]
fn a_save_in_stopped_time_puts_the_throw_back_in_the_pack() {
    let mut w = test_world(1);
    read(&mut w, up(CardFace::World));
    let p = player(&mut w);
    let dagger = spawn_named(&mut w, "dagger", HERE).unwrap();
    w.entity_mut(dagger).remove::<Position>();
    w.get_mut::<Backpack>(p).unwrap().items.push(dagger);
    throw(&mut w, dagger, Position { x: 25, y: 10 });
    assert!(!in_pack(&mut w, dagger));
    let save = common::SaveFile::new("world");
    save_game(&mut w, save.path()).unwrap();
    assert!(in_pack(&mut w, dagger));
    assert!(w.get::<TimeStopped>(p).is_some());
}

#[test]
fn the_world_reversed_paralyses_the_reader() {
    let mut w = test_world(1);
    read(&mut w, down(CardFace::World));
    let p = player(&mut w);
    assert!(w.get::<Paralyzed>(p).is_some());
}

#[test]
fn the_read_menu_lists_a_deck() {
    let mut w = test_world(1);
    let deck = deck_in_pack(&mut w, &[up(CardFace::Fool)]);
    assert!(PackMode::Read.admits(&w, deck));
}
