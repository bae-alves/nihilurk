//! Decks. A stack of potion and scroll effects, stacked when the deck was
//! rolled. Reading the deck plays the top effect on the reader ([`play`]);
//! throwing it bursts it like a wand of light with one charge per effect left
//! ([`throw_deck`]), and the stacked effects are lost. The pool a deck rolls
//! from is [`crate::catalog::deck_pool`].

use bevy_ecs::{entity::Entity, world::World};

use crate::catalog::{Burst, WANDS, index_of};
use crate::components::*;
use crate::conduct::{Conduct, player_broke};

const WAND_OF_LIGHT: usize = index_of!(WANDS, "wand of light");

/// Plays `effect` on `user` the way drinking the potion or reading the scroll
/// would, conduct and all.
pub fn play(world: &mut World, user: Entity, effect: DeckEffect) {
    match effect {
        DeckEffect::Potion(row) => {
            player_broke(world, user, Conduct::Potions);
            (row.apply)(world, user);
        }
        DeckEffect::Scroll(row) => {
            player_broke(world, user, Conduct::Scrolls);
            (row.apply)(world, user);
        }
    }
}

/// A thrown deck breaks where it lands, bursting as a wand of light with a
/// charge for each effect it still held. The effects themselves are lost.
pub fn throw_deck(world: &mut World, thrower: Entity, deck: Entity, at: Position, seen_name: &str) {
    let charges = world
        .get::<Deck>(deck)
        .map_or(0, |d| i32::try_from(d.effects.len()).unwrap_or(i32::MAX));
    world.entity_mut(deck).despawn();
    let row = WANDS[WAND_OF_LIGHT];
    (row.thrown.burst)(
        world,
        Burst {
            row,
            thrower,
            at,
            charges,
            seen_name,
        },
    );
}
