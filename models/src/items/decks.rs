//! Decks of cards. Five cards stacked when the deck was rolled, one to two of
//! them reversed. Reading the deck plays the top card ([`draw`]); throwing it,
//! or drawing JESTER, plays what is left as a poker hand ([`play_hand`]), which
//! never reverses anything. The cards are [`crate::catalog::CARDS`].
//!
//! Every card plays on whoever drew or threw it. Some cards play other cards
//! (the Joker, Pot of Sin, The +4); one draw may play at most
//! [`CARD_CHAIN_CAP`] cards in all, which is what ends a chain that would not
//! end by itself.

use bevy_ecs::{entity::Entity, prelude::With, world::World};
use crossterm::style::Color;
use rand::seq::SliceRandom;

use crate::catalog::{CARDS, CardDef, RingDef};
use crate::components::*;
use crate::constants::decks::*;
use crate::effects::{Bala, Bole, Crit, Grant, Lifetime, Oof, TimeStopped, lend};
use crate::equipment::{Slot, destroy_worn, sync_equipment_effects};
use crate::helpers::{free_adjacent_tile, hostiles_in_view, item_label, mark_conditions};
use crate::identify::{article_for, display_name};
use crate::map::{GameRng, Map, TileType};
use crate::monsters::{MonsterDef, SUMMONS, spawn_monster};

use super::scrolls::{
    create_monster, enchant_gear, lower_equipped, missing_gear_line, move_reader_to, summon_spot,
};

/// One draw in progress: who plays, the deck they hold (`None` once it is gone
/// or on the table), and how many more cards this draw may still play.
struct Deal {
    user: Entity,
    deck: Option<Entity>,
    budget: u32,
}

/// Reading a deck: `card` is the one just taken off the top. `deck` is the deck
/// itself while it still holds cards, `None` once that was its last.
pub(super) fn draw(world: &mut World, user: Entity, deck: Option<Entity>, card: Card) {
    let mut deal = Deal {
        user,
        deck,
        budget: CARD_CHAIN_CAP,
    };
    play(world, &mut deal, card);
}

/// A thrown deck: it hits the floor and everything left in it plays as a hand,
/// on the thrower.
pub(super) fn throw_deck(world: &mut World, thrower: Entity, deck: Entity) {
    let cards = world
        .get::<Deck>(deck)
        .map(|d| d.cards.clone())
        .unwrap_or_default();
    world.entity_mut(deck).despawn();
    let mut deal = Deal {
        user: thrower,
        deck: None,
        budget: CARD_CHAIN_CAP,
    };
    play_hand(world, &mut deal, &cards);
}

/// Plays one card. Exhaustive over [`CardFace`] with no catch-all, the way
/// `apply_scroll_effect` is: a new face fails the build until it does
/// something.
fn play(world: &mut World, deal: &mut Deal, card: Card) {
    if deal.budget == 0 {
        say(world, strings::card_chain_spent());
        return;
    }
    deal.budget -= 1;
    let name = card_name(card.face);
    say(world, &strings::card_drawn(&name, card.reversed));

    let user = deal.user;
    let r = card.reversed;
    match card.face {
        CardFace::Joker => {
            if let Some(&face) = other_faces(world, card.face, 1).first() {
                play(world, deal, Card { face, reversed: r });
            }
        }
        CardFace::PotOfSin => play_others(world, deal, card, 2),
        CardFace::PlusFour => play_others(world, deal, card, 4),
        CardFace::KingOfClubs => recharge_wands(world, user, r),
        CardFace::PrinceOfSwords => enchant_or_dull(world, user, Slot::Hand, r),
        CardFace::QueenOfCups => enchant_or_dull(world, user, Slot::Body, r),
        CardFace::PrincessOfDiamonds => princess(world, user, r),
        CardFace::Balance => ledger(world, user, Grant::of::<Bala>(), strings::card_bala()),
        CardFace::Bole => ledger(world, user, Grant::of::<Bole>(), strings::card_bole()),
        CardFace::Chariot => match r {
            false => ledger(world, user, Grant::of::<Crit>(), strings::card_crit()),
            true => ledger(world, user, Grant::of::<Oof>(), strings::card_oof()),
        },
        CardFace::SkullKing => summon(world, user, &SUMMONS[0], r, false),
        CardFace::BlackMage => summon(world, user, &SUMMONS[1], r, true),
        CardFace::Child => to_stair(world, user, !r),
        CardFace::Crone => to_stair(world, user, r),
        CardFace::Eyes => eyes(world, deal, r),
        CardFace::Fool => say(world, strings::card_fool()),
        CardFace::Excuse => excuse(world, user, r),
        CardFace::Jester => jester(world, deal),
        CardFace::World => match r {
            false => {
                lend(
                    world,
                    user,
                    Grant::of::<TimeStopped>(),
                    Lifetime::Turns(WORLD_TURNS),
                );
                say(world, strings::card_world());
            }
            true => {
                crate::conditions::paralyse(world, user);
            }
        },
        CardFace::GoldenWind => match r {
            false => golden_wind(world),
            true => {
                for _ in 0..GOLDEN_WIND_SUMMONS {
                    create_monster(world, user);
                }
            }
        },
    }
}

fn say(world: &mut World, line: &str) {
    world.resource_mut::<GameLog>().add(line.to_string());
}

fn card_name(face: CardFace) -> String {
    strings::content_name(CardDef::of(face).name).to_string()
}

/// `n` faces, all different, none of them `not` — what the Joker, Pot of Sin
/// and The +4 conjure.
fn other_faces(world: &mut World, not: CardFace, n: usize) -> Vec<CardFace> {
    let mut pool: Vec<CardFace> = CARDS.iter().map(|c| c.face).filter(|&f| f != not).collect();
    pool.shuffle(&mut world.resource_mut::<GameRng>().0);
    pool.truncate(n);
    pool
}

/// Pot of Sin and The +4: `n` different cards, each played the same way up as
/// the card that made them.
fn play_others(world: &mut World, deal: &mut Deal, card: Card, n: usize) {
    for face in other_faces(world, card.face, n) {
        play(
            world,
            deal,
            Card {
                face,
                reversed: card.reversed,
            },
        );
    }
}

/// King of Clubs: every wand in the pack full again. Reversed: every one empty.
fn recharge_wands(world: &mut World, user: Entity, reversed: bool) {
    let pack = world
        .get::<Backpack>(user)
        .map(|b| b.items.clone())
        .unwrap_or_default();
    let charges = match reversed {
        false => crate::constants::wands::WAND_CHARGES,
        true => 0,
    };
    for item in pack {
        if let Some(mut battery) = world.get_mut::<Battery>(item) {
            battery.charges = charges;
        }
    }
    say(
        world,
        match reversed {
            false => strings::card_wands_full(),
            true => strings::card_wands_empty(),
        },
    );
}

/// Prince of Swords and Queen of Cups: the scroll's enchantment on the gear in
/// `slot`. Reversed: one point off it instead.
fn enchant_or_dull(world: &mut World, user: Entity, slot: Slot, reversed: bool) {
    if !reversed {
        enchant_gear(world, user, slot);
        return;
    }
    if !lower_equipped(world, user, slot) {
        say(world, missing_gear_line(slot));
    }
}

/// Princess of Diamonds: a point onto a worn ring that has a number to it (a
/// ring of protection, strength, increase damage, sharpshooting). Reversed: a
/// worn ring loses a point, and one at `+0` or with no number at all breaks.
fn princess(world: &mut World, user: Entity, reversed: bool) {
    let worn: Vec<Entity> = crate::equipment::equipped_items(world, user)
        .into_iter()
        .filter(|&e| world.get::<Ring>(e).is_some())
        .collect();
    let numeric: Vec<Entity> = worn
        .iter()
        .copied()
        .filter(|&e| ring_is_numeric(world, e))
        .collect();
    let pool = if reversed { &worn } else { &numeric };
    let Some(&ring) = pool.choose(&mut world.resource_mut::<GameRng>().0) else {
        say(world, strings::card_no_ring());
        return;
    };
    let plus = ring_plus(world, ring);
    if reversed && (plus <= 0 || !ring_is_numeric(world, ring)) {
        let name = display_name(world, ring);
        destroy_worn(world, user, &[ring]);
        world
            .resource_mut::<GameLog>()
            .add(strings::ring_shivers_apart(&name));
        return;
    }
    shift_ring(world, ring, if reversed { -1 } else { 1 });
    world.entity_mut(ring).insert(KnownQuality);
    let name = display_name(world, ring);
    crate::helpers::spark_burst_at(world, user, Color::DarkYellow);
    world.resource_mut::<GameLog>().add(match reversed {
        false => strings::enchant_sparks(&name),
        true => strings::card_dulls(&name),
    });
}

/// Whether `ring`'s row has a number on it at all.
fn ring_is_numeric(world: &World, ring: Entity) -> bool {
    world
        .get::<Ring>(ring)
        .is_some_and(|r| RingDef::of(r.effect).is_numeric())
}

/// The number a worn ring adds, whichever roll it lands on.
fn ring_plus(world: &World, ring: Entity) -> i32 {
    use crate::effects::{ArmorBonus, PowerBonus, ThrowBonus};
    world.get::<PowerBonus>(ring).map_or(0, |b| b.0)
        + world.get::<ArmorBonus>(ring).map_or(0, |b| b.0)
        + world.get::<ThrowBonus>(ring).map_or(0, |b| b.0)
}

/// Moves a numeric ring's number by `by`, on whichever roll its row puts it.
fn shift_ring(world: &mut World, ring: Entity, by: i32) {
    use crate::effects::{ArmorBonus, PowerBonus, ThrowBonus};
    let Some(def) = world.get::<Ring>(ring).map(|r| RingDef::of(r.effect)) else {
        return;
    };
    let (power, armor, throw) = def.bonuses();
    let mut e = world.entity_mut(ring);
    if power != 0 {
        let base = e.get::<PowerBonus>().map_or(0, |b| b.0);
        e.insert(PowerBonus(base + by));
    }
    if armor != 0 {
        let base = e.get::<ArmorBonus>().map_or(0, |b| b.0);
        e.insert(ArmorBonus(base + by));
    }
    if throw != 0 {
        let base = e.get::<ThrowBonus>().map_or(0, |b| b.0);
        e.insert(ThrowBonus(base + by));
    }
}

/// The Balance, The Bole and the Chariot: a mark on the ledger for the floor.
fn ledger(world: &mut World, user: Entity, grant: Grant, line: &str) {
    lend(world, user, grant, Lifetime::Floor);
    say(world, line);
}

/// THE SKULL KING! and THE BLACK MAGE: they arrive beside the reader, as an
/// ally (the Skull King) or the Helper (the Black Mage). Reversed, as a foe.
fn summon(world: &mut World, user: Entity, def: &MonsterDef, reversed: bool, helper: bool) {
    let Some((x, y)) = summon_spot(world, user) else {
        say(world, strings::create_monster_nowhere());
        return;
    };
    let e = spawn_monster(world, def, Position { x, y });
    let name = item_label(world, e);
    world
        .resource_mut::<GameLog>()
        .add(strings::create_monster_line(article_for(&name), &name));
    match (reversed, helper) {
        (true, _) => crate::effects::revoke(world, e, Grant::of::<crate::effects::Asleep>()),
        (false, true) => crate::companion::recruit(world, e),
        (false, false) => {
            crate::companion::charm(world, e);
            world
                .resource_mut::<GameLog>()
                .add(strings::charm_target_line(&name));
        }
    }
}

/// The Child and The Crone: to this floor's up-stair (`up`) or down-stair, or
/// beside it when something stands on it.
fn to_stair(world: &mut World, user: Entity, up: bool) {
    let want = if up {
        TileType::Upstairs
    } else {
        TileType::Downstairs
    };
    let Some(stair) = crate::map::find_tile(&world.resource::<Map>().tiles, want) else {
        say(world, strings::card_no_stair());
        return;
    };
    let at = Position {
        x: stair.0,
        y: stair.1,
    };
    let tile = match occupied(world, at) {
        false => Some(stair),
        true => free_adjacent_tile(world, at),
    };
    let Some(tile) = tile else {
        say(world, strings::card_no_stair());
        return;
    };
    move_reader_to(world, user, tile);
    say(world, strings::card_to_stair(up));
}

/// Whether a creature, the player included, stands on `at`.
fn occupied(world: &mut World, at: Position) -> bool {
    crate::helpers::mob_at(world, at).is_some()
        || world
            .query_filtered::<&Position, With<Player>>()
            .iter(world)
            .any(|p| *p == at)
}

/// The Eyes Never Lie: what is left in the deck, top card first.
fn eyes(world: &mut World, deal: &Deal, reversed: bool) {
    let cards = deal
        .deck
        .and_then(|d| world.get::<Deck>(d))
        .map(|d| d.cards.clone());
    match cards {
        Some(cards) if !cards.is_empty() => {
            let names: Vec<String> = cards
                .iter()
                .rev()
                .map(|c| strings::card_seen(&card_name(c.face), c.reversed))
                .collect();
            say(world, &strings::card_eyes(&names));
        }
        _ => say(world, strings::card_eyes_nothing()),
    }
    if reversed {
        say(world, strings::card_eyes_never_lie());
    }
}

/// THE EXCUSE: every foe in view reels. Reversed: the reader does.
fn excuse(world: &mut World, user: Entity, reversed: bool) {
    if reversed {
        crate::conditions::confuse(
            world,
            user,
            strings::potion_confusion_player(),
            LogCategory::Plain,
            strings::potion_confusion_mob(),
        );
        return;
    }
    let caught = hostiles_in_view(world, user);
    for &t in &caught {
        crate::conditions::stagger(world, t, strings::potion_confusion_mob());
    }
    mark_conditions(world, &caught, '?', Color::Magenta);
    if caught.is_empty() {
        say(world, strings::card_excuse_nobody());
    }
}

/// JESTER: the rest of the deck, played as a hand. A JESTER in a hand finds
/// the deck already on the table and does nothing.
fn jester(world: &mut World, deal: &mut Deal) {
    let Some(deck) = deal.deck.take() else {
        say(world, strings::card_jester_nothing());
        return;
    };
    let cards = world
        .get::<Deck>(deck)
        .map(|d| d.cards.clone())
        .unwrap_or_default();
    if let Some(mut bp) = world.get_mut::<Backpack>(deal.user) {
        bp.items.retain(|&i| i != deck);
    }
    world.entity_mut(deck).despawn();
    play_hand(world, deal, &cards);
}

/// XXIII GOLDEN WIND: every item lying on the floor and every trap stands up as
/// a creature of this floor, on your side. The Element stays what it is.
fn golden_wind(world: &mut World) {
    let mut things: Vec<(Entity, Position)> = world
        .query_filtered::<(Entity, &Position), With<Item>>()
        .iter(world)
        .map(|(e, p)| (e, *p))
        .collect();
    things.extend(
        world
            .query_filtered::<(Entity, &Position), With<Trap>>()
            .iter(world)
            .map(|(e, p)| (e, *p)),
    );
    let depth = world.resource::<Depth>().what;
    let mut woke = 0;
    for (thing, at) in things {
        if world.get::<Amulet>(thing).is_some() {
            continue;
        }
        let spot = match occupied(world, at) {
            false => Some((at.x, at.y)),
            true => free_adjacent_tile(world, at),
        };
        let Some((x, y)) = spot else {
            continue;
        };
        world.entity_mut(thing).despawn();
        let def = MonsterDef::pick(depth, &mut world.resource_mut::<GameRng>().0);
        let e = spawn_monster(world, def, Position { x, y });
        crate::companion::charm(world, e);
        woke += 1;
    }
    say(
        world,
        match woke {
            0 => strings::card_golden_wind_nothing(),
            _ => strings::card_golden_wind(),
        },
    );
}

// ---------------------------------------------------------------------------
// Hands
// ---------------------------------------------------------------------------

/// What a hand of cards is worth, worst first.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Rank {
    /// Every card different: one of them plays, at random.
    AntiFlush,
    /// Two alike: that card plays twice.
    Pair,
    /// Two pairs: each plays twice.
    TwoPair,
    /// Three alike: that card plays thrice.
    ThreeOfAKind,
    /// Three alike and two alike: thrice and twice.
    FullHouse,
    /// Four alike: that card plays four times.
    FourOfAKind,
    /// Five alike: the Element of Yoord.
    FiveFlush,
}

impl Rank {
    /// The score this hand pays.
    pub fn points(self) -> i32 {
        match self {
            Rank::AntiFlush => ANTI_FLUSH_POINTS,
            Rank::Pair => PAIR_POINTS,
            Rank::TwoPair => TWO_PAIR_POINTS,
            Rank::ThreeOfAKind => THREE_OF_A_KIND_POINTS,
            Rank::FullHouse => FULL_HOUSE_POINTS,
            Rank::FourOfAKind => FOUR_OF_A_KIND_POINTS,
            Rank::FiveFlush => FIVE_FLUSH_POINTS,
        }
    }
}

/// A scored hand: its rank, and which cards play how many times. An
/// [`Rank::AntiFlush`] and a [`Rank::FiveFlush`] list no plays: the first
/// picks its card at random, the second plays none.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Hand {
    /// What the hand is.
    pub rank: Rank,
    /// Each grouped face and how many times it plays.
    pub plays: Vec<(CardFace, usize)>,
}

/// Scores `cards` as a poker hand. FOOL is wild: every FOOL joins the biggest
/// group, which is always the best use of it. Reversal is ignored; a hand never
/// plays reversed. `None` for no cards at all.
///
/// ```
/// # use models::{Card, CardFace, Rank, score_hand};
/// let up = |face| Card { face, reversed: false };
/// let hand = score_hand(&[up(CardFace::Bole), up(CardFace::Fool), up(CardFace::Joker)]).unwrap();
/// assert_eq!(hand.rank, Rank::Pair);
/// assert_eq!(hand.plays, vec![(CardFace::Bole, 2)]);
/// ```
pub fn score_hand(cards: &[Card]) -> Option<Hand> {
    if cards.is_empty() {
        return None;
    }
    let wild = cards.iter().filter(|c| c.face == CardFace::Fool).count();
    let mut groups: Vec<(CardFace, usize)> = Vec::new();
    for c in cards.iter().filter(|c| c.face != CardFace::Fool) {
        match groups.iter_mut().find(|(f, _)| *f == c.face) {
            Some((_, n)) => *n += 1,
            None => groups.push((c.face, 1)),
        }
    }
    // Stable, so equal groups keep the order they were dealt in.
    groups.sort_by_key(|g| std::cmp::Reverse(g.1));
    match groups.first_mut() {
        Some(top) => top.1 += wild,
        None => groups.push((CardFace::Fool, wild)),
    }
    let first = groups[0];
    let second = groups.get(1).map_or(0, |g| g.1);
    let (rank, plays) = match (first.1, second) {
        (5.., _) => (Rank::FiveFlush, vec![]),
        (4, _) => (Rank::FourOfAKind, vec![first]),
        (3, 2) => (Rank::FullHouse, vec![first, groups[1]]),
        (3, _) => (Rank::ThreeOfAKind, vec![first]),
        (2, 2) => (Rank::TwoPair, vec![first, groups[1]]),
        (2, _) => (Rank::Pair, vec![first]),
        _ => (Rank::AntiFlush, vec![]),
    };
    Some(Hand { rank, plays })
}

/// Plays `cards` as a hand on `deal.user`: the score, then each group's card
/// upright, as many times as the hand says.
fn play_hand(world: &mut World, deal: &mut Deal, cards: &[Card]) {
    let Some(hand) = score_hand(cards) else {
        say(world, strings::card_jester_nothing());
        return;
    };
    crate::score::award(world, hand.rank.points());
    world.resource_mut::<GameLog>().add_colored(
        strings::card_hand(hand_name(hand.rank), hand.rank.points()),
        LogCategory::Combo,
    );
    let plays = match hand.rank {
        Rank::FiveFlush => {
            five_flush(world, deal.user);
            return;
        }
        Rank::AntiFlush => {
            let faces: Vec<CardFace> = cards
                .iter()
                .map(|c| c.face)
                .filter(|&f| f != CardFace::Fool)
                .collect();
            let face = *faces
                .choose(&mut world.resource_mut::<GameRng>().0)
                .expect("an anti-flush holds a card that is not FOOL");
            vec![(face, 1)]
        }
        _ => hand.plays,
    };
    for (face, times) in plays {
        for _ in 0..times {
            play(
                world,
                deal,
                Card {
                    face,
                    reversed: false,
                },
            );
        }
    }
}

fn hand_name(rank: Rank) -> &'static str {
    match rank {
        Rank::AntiFlush => strings::hand_anti_flush(),
        Rank::Pair => strings::hand_pair(),
        Rank::TwoPair => strings::hand_two_pair(),
        Rank::ThreeOfAKind => strings::hand_three_of_a_kind(),
        Rank::FullHouse => strings::hand_full_house(),
        Rank::FourOfAKind => strings::hand_four_of_a_kind(),
        Rank::FiveFlush => strings::hand_five_flush(),
    }
}

/// Five Flush!: the Element of Yoord, into the pack. A full pack is wiped to
/// make room. Already holding it, or in an endless run with no Element to win,
/// the hand pays its score and nothing more. The ring of adornment's flourish
/// goes off either way, without its doubling.
fn five_flush(world: &mut World, user: Entity) {
    let endless = world
        .get_resource::<crate::map::Endless>()
        .is_some_and(|e| e.enabled);
    if !endless && !crate::map::holding_element_of_yoord(world) {
        give_element(world, user);
    }
    super::rings::flourish(world);
}

fn give_element(world: &mut World, user: Entity) {
    let pack = world
        .get::<Backpack>(user)
        .map(|b| b.items.clone())
        .unwrap_or_default();
    if pack.len() >= crate::constants::items::PACK_CAPACITY {
        destroy_worn(world, user, &pack);
        say(world, strings::card_element_wipes_pack());
    }
    let at = world
        .get::<Position>(user)
        .copied()
        .unwrap_or(Position { x: 0, y: 0 });
    let element = crate::catalog::spawn_element_of_yoord(world, at);
    world.entity_mut(element).remove::<Position>();
    if let Some(mut bp) = world.get_mut::<Backpack>(user) {
        bp.items.push(element);
    }
    sync_equipment_effects(world, user);
    say(world, strings::card_element());
}
