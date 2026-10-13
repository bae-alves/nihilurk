//! Conducts: the things a run can go without, and what a won run is paid for
//! keeping them.
//!
//! Each [`Conduct`] is broken by exactly one act, at exactly one place in the
//! code (the table is in `docs/reference/`). A broken conduct stays broken: the
//! tally is one `u16` of bits in the [`Conducts`] resource, saved with the run.
//!
//! Only [`Conduct::Lurk`] is not tracked. It is a fact about the body, read off
//! the player's marker when the mask is asked for.
//!
//! The order of [`Conduct::ALL`] is a file format. The leaderboard stores the
//! kept mask by bit index, so a variant goes on the end and none are reordered.

use bevy_ecs::prelude::*;
use crossterm::style::Color;

use crate::components::Player;
use crate::effects::Lurk;

/// What a kept conduct adds to a won run's score, in percent of the score.
pub const PERCENT_PER_CONDUCT: i64 = 10;

/// One thing a run can go without.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Conduct {
    /// Never drank a potion.
    Potions,
    /// Never read a scroll.
    Scrolls,
    /// Never zapped (or threw) a wand.
    Wands,
    /// Never invoked a rune.
    Runes,
    /// Never cast a spell.
    Spells,
    /// Never took a coin.
    Coins,
    /// Never threw or fired anything.
    Ranged,
    /// Never swung at anything.
    Attacking,
    /// Never charmed a creature.
    Charming,
    /// Never took a boon companion.
    Companion,
    /// HP never dropped.
    Damage,
    /// Never put on gear.
    Gear,
    /// Armor never took a killing blow.
    Armor,
    /// Played as the lurk.
    Lurk,
}

impl Conduct {
    /// Every conduct, in bit order.
    pub const ALL: [Conduct; 14] = [
        Conduct::Potions,
        Conduct::Scrolls,
        Conduct::Wands,
        Conduct::Runes,
        Conduct::Spells,
        Conduct::Coins,
        Conduct::Ranged,
        Conduct::Attacking,
        Conduct::Charming,
        Conduct::Companion,
        Conduct::Damage,
        Conduct::Gear,
        Conduct::Armor,
        Conduct::Lurk,
    ];

    /// This conduct's bit in a mask.
    pub const fn bit(self) -> u16 {
        1 << self as u16
    }
}

/// The conducts this run has broken, one bit each.
#[derive(Resource, Default)]
pub struct Conducts {
    /// Set bit = broken. [`Conduct::Lurk`]'s bit is never set here.
    pub broken: u16,
}

/// Marks `conduct` broken. Does nothing in a world with no tally.
pub fn broke(world: &mut World, conduct: Conduct) {
    if let Some(mut conducts) = world.get_resource_mut::<Conducts>() {
        conducts.broken |= conduct.bit();
    }
}

/// The conducts the run has kept, one bit each. A world with no tally has kept
/// every one it can show.
pub fn kept_mask(world: &mut World) -> u16 {
    let all = Conduct::ALL.iter().fold(0, |mask, c| mask | c.bit());
    let broken = world.get_resource::<Conducts>().map_or(0, |c| c.broken);
    let lurk = world
        .query_filtered::<Entity, (With<Player>, With<Lurk>)>()
        .iter(world)
        .next()
        .is_some();
    let mut kept = all & !broken;
    if !lurk {
        kept &= !Conduct::Lurk.bit();
    }
    kept
}

/// What a won run's score grows by, in percent, for the conducts in `mask`.
pub fn bonus_percent(mask: u16) -> i64 {
    PERCENT_PER_CONDUCT * i64::from(mask.count_ones())
}

/// [`broke`], but only when `who` is the player: the sites that break a
/// conduct are shared with monsters, who have no conducts to keep.
pub fn player_broke(world: &mut World, who: Entity, conduct: Conduct) {
    if world.get::<Player>(who).is_some() {
        broke(world, conduct);
    }
}

/// What one kept conduct looks like on the win screen and the leaderboard.
pub const MARK: char = '◆';

/// The lurk conduct's mark colour: the lurk's own, the same as the `@` the
/// player is drawn as (`wear_lurk` in `body.rs`).
pub const LURK_COLOR: Color = Color::Magenta;

/// One [`MARK`] per conduct in `mask`, in [`Conduct::ALL`] order. Colours cycle
/// through `stripes` by the conduct's place in that order, so a conduct keeps
/// its colour whichever others are broken; the lurk conduct is [`LURK_COLOR`]
/// whatever flag the run flies. An empty `stripes` paints everything white.
pub fn marks(mask: u16, stripes: &[Color]) -> Vec<(char, Color)> {
    Conduct::ALL
        .iter()
        .enumerate()
        .filter(|(_, c)| mask & c.bit() != 0)
        .map(|(i, &c)| {
            let colour = match (c, stripes.is_empty()) {
                (Conduct::Lurk, _) => LURK_COLOR,
                (_, true) => Color::White,
                (_, false) => stripes[i % stripes.len()],
            };
            (MARK, colour)
        })
        .collect()
}
