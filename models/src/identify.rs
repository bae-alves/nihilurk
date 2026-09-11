//! Item identification: potions, scrolls, wands and rings hide their true
//! nature behind a cosmetic appearance until the player learns otherwise —
//! the classic roguelike "unidentified item" trick (see NetHack).
//!
//! A [`Potion`]/[`Scroll`]/[`Wand`]/[`Ring`] component always carries the
//! item's true effect; nothing about identification changes that. What
//! changes is how the item is *displayed*: [`ItemAppearances`] holds this
//! run's random, shuffled cosmetic label for every true type (assigned once,
//! at world creation, from the seeded RNG), and [`Identified`] is the
//! player's global knowledge registry — a per-category set of effects the
//! player has learned. [`display_name`] is the single place that reconciles
//! the two into what the player actually sees.
//!
//! Identifying one bubbly potion identifies *every* bubbly potion, anywhere:
//! knowledge lives on the effect, not the entity.

use bevy_ecs::prelude::*;
use rand::seq::SliceRandom;
use rand_chacha::ChaCha12Rng;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

use crate::catalog::{POTIONS, RINGS, SCROLLS, WANDS};
use crate::components::{
    Curse, KnownQuality, Name, PotionEffect, Ring, RingEffect, ScrollEffect, Stack, Vorpal,
    WandEffect,
};
use crate::effects::{ArmorBonus, PowerBonus, ThrowBonus};
use crate::helpers::item_label;

/// The true types in each category, taken straight from the catalog tables so a
/// new item is never missing an appearance. There is no second list to keep in
/// step with [`crate::catalog`].
fn all_potions() -> Vec<PotionEffect> {
    POTIONS.iter().map(|d| d.effect).collect()
}
fn all_scrolls() -> Vec<ScrollEffect> {
    SCROLLS.iter().map(|d| d.effect).collect()
}
fn all_wands() -> Vec<WandEffect> {
    WANDS.iter().map(|d| d.effect).collect()
}
fn all_rings() -> Vec<RingEffect> {
    RINGS.iter().map(|d| d.effect).collect()
}

/// Colours and consistencies, NetHack-style: pooled well beyond the potion
/// table's length so a shuffle always has room to spare.
const POTION_APPEARANCES: [&str; 20] = [
    "ruby",
    "pink",
    "orange",
    "amber",
    "emerald",
    "cyan",
    "violet",
    "brown",
    "grey",
    "yellow",
    "bubbly",
    "fizzy",
    "swirly",
    "milky",
    "murky",
    "cloudy",
    "smoky",
    "oily",
    "sparkling",
    "viscous",
];

/// Nonsense scroll titles, invented for roog rather than borrowed from
/// NetHack's own in-joke set.
const SCROLL_APPEARANCES: [&str; 20] = [
    "XLUM QUAZAR",
    "GNOR VEKTIL",
    "ZIMBO RASHT",
    "PLIN DRAVOK",
    "MORZ ELKATH",
    "YFEN CROSTIL",
    "WUBBA LENTHOR",
    "SKAR MUNDIL",
    "TAVROK ZIN",
    "ELDIC PHORN",
    "QUIX SABAROTH",
    "NELGO VASHT",
    "BRIN ZORATHIL",
    "HULK PENDRIL",
    "OMFA TRIXEL",
    "VASK ELDROON",
    "KYRIL BANTOX",
    "DREN QUOZAL",
    "SPLICK VORNAI",
    "TUM ELKHESH",
];

/// Wand materials, from mundane woods to oddities.
const WAND_APPEARANCES: [&str; 20] = [
    "oak",
    "balsa",
    "maple",
    "teak",
    "ebony",
    "iron",
    "brass",
    "copper",
    "zinc",
    "tin",
    "silver",
    "bronze",
    "steel",
    "glass",
    "crystal",
    "hexagonal",
    "marbled",
    "jeweled",
    "forked",
    "runed",
];

/// Ring gems and metals.
const RING_APPEARANCES: [&str; 20] = [
    "pearl",
    "iron",
    "twisted",
    "wire",
    "engagement",
    "diamond",
    "sapphire",
    "ruby",
    "wooden",
    "granite",
    "opal",
    "clay",
    "tiger-eye",
    "moonstone",
    "jade",
    "coral",
    "bronze",
    "spinel",
    "agate",
    "onyx",
];

/// This run's cosmetic appearance for every true item type, one shuffled
/// pool per category. Generated once at world creation from the seeded RNG
/// (so it's reproducible per-seed) and persisted in the save file thereafter
/// — a save always keeps the appearances it was created with.
#[derive(Resource, Default, Clone, Serialize, Deserialize)]
pub struct ItemAppearances {
    pub potions: HashMap<PotionEffect, String>,
    pub scrolls: HashMap<ScrollEffect, String>,
    pub wands: HashMap<WandEffect, String>,
    pub rings: HashMap<RingEffect, String>,
}

/// Shuffles `pool` with `rng` and zips it against `effects`, one appearance
/// per true type. `pool` must have at least as many entries as `effects`.
fn assign<E: Copy + Eq + std::hash::Hash>(
    effects: &[E],
    pool: &[&str],
    rng: &mut ChaCha12Rng,
) -> HashMap<E, String> {
    let mut shuffled: Vec<&str> = pool.to_vec();
    shuffled.shuffle(rng);
    effects
        .iter()
        .copied()
        .zip(shuffled.into_iter().map(String::from))
        .collect()
}

impl ItemAppearances {
    /// Builds a fresh, randomised appearance map for a new run.
    pub fn generate(rng: &mut ChaCha12Rng) -> Self {
        Self {
            potions: assign(&all_potions(), &POTION_APPEARANCES, rng),
            scrolls: assign(&all_scrolls(), &SCROLL_APPEARANCES, rng),
            wands: assign(&all_wands(), &WAND_APPEARANCES, rng),
            rings: assign(&all_rings(), &RING_APPEARANCES, rng),
        }
    }
}

/// The player's global identification knowledge: which true item types have
/// been learned, per category. Identifying an effect identifies it
/// everywhere — every item sharing that effect displays its true name from
/// then on, matching every roguelike's shared-knowledge convention.
#[derive(Resource, Default, Clone, Serialize, Deserialize)]
pub struct Identified {
    pub potions: HashSet<PotionEffect>,
    pub scrolls: HashSet<ScrollEffect>,
    pub wands: HashSet<WandEffect>,
    pub rings: HashSet<RingEffect>,
}

/// What the player actually sees for `item`: its true name if the type has
/// been identified, otherwise this run's cosmetic appearance (or a generic
/// fallback if somehow no appearance was assigned). Non-identifiable items
/// (weapons, armor, gold, the amulet) always show their true [`Name`], and a
/// [`Stack`] of them shows how many it holds. A weapon, suit of armour or
/// launcher additionally gets an enchantment plus in front and a `(cursed)`
/// tag after, once [`known_quality`] says either is actually known.
pub fn display_name(world: &World, item: Entity) -> String {
    let base = named_display(
        world.get::<crate::components::Potion>(item),
        world.get::<crate::components::Scroll>(item),
        world.get::<crate::components::Wand>(item),
        world.get::<Ring>(item),
        world.get::<Name>(item),
        world.get::<Stack>(item),
        world.resource::<Identified>(),
        world.resource::<ItemAppearances>(),
    );
    annotate_quality(world, item, base)
}

/// Whether `item`'s own enchantment plus and curse status are visible yet.
/// Deliberately *not* the same question as whether its type is identified: two
/// rings of protection share one [`Identified`] entry the moment either is
/// worn, but each is its own roll of the curse dice, so knowing one is a ring
/// of protection must not leak whether some *other* one is cursed. Every kind
/// of gear therefore answers through its own [`KnownQuality`], set the moment
/// it's worn (see [`crate::equipment::toggle_equipped`]) or a scroll of
/// identify singles it out.
pub fn known_quality(world: &World, item: Entity) -> bool {
    world.get::<KnownQuality>(item).is_some()
}

/// The enchantment plus `item` carries — whichever bonus its kind rolled
/// (a weapon's [`PowerBonus`], a suit of armour's [`ArmorBonus`], a launcher's
/// [`ThrowBonus`]) — or `0` for anything [`crate::catalog::enchant_equipment`]
/// never touched. Never more than one of the three is actually nonzero; adding
/// them saves asking which kind of gear this is.
fn enchantment_plus(world: &World, item: Entity) -> i32 {
    world.get::<PowerBonus>(item).map(|b| b.0).unwrap_or(0)
        + world.get::<ArmorBonus>(item).map(|b| b.0).unwrap_or(0)
        + world.get::<ThrowBonus>(item).map(|b| b.0).unwrap_or(0)
}

/// Adds `item`'s enchantment plus, curse status and vorpal bane to `base`,
/// once [`known_quality`] says they're visible — `"+1 ring mail"`, `"-2
/// dagger (cursed)"`, `"long sword (vorpal vs. orc)"`. Silently returns `base`
/// untouched beforehand, and for anything with none of the three to show.
fn annotate_quality(world: &World, item: Entity, base: String) -> String {
    if !known_quality(world, item) {
        return base;
    }
    let plus = enchantment_plus(world, item);
    let name = if plus != 0 {
        format!("{plus:+} {base}")
    } else {
        base
    };
    let name = if world.get::<Curse>(item).is_some() {
        format!("{name} (cursed)")
    } else {
        name
    };
    match world.get::<Vorpal>(item) {
        Some(v) => format!("{name} (vorpal vs. {})", v.bane),
        None => name,
    }
}

/// The [`display_name`] logic, decoupled from `&World` so a `Query`-based
/// system (which never holds a whole-`World` reference) can render the same
/// identification-aware label — see [`crate::visibility::spotted_line`].
#[allow(clippy::too_many_arguments)]
pub fn named_display(
    potion: Option<&crate::components::Potion>,
    scroll: Option<&crate::components::Scroll>,
    wand: Option<&crate::components::Wand>,
    ring: Option<&Ring>,
    name: Option<&Name>,
    stack: Option<&Stack>,
    identified: &Identified,
    appearances: &ItemAppearances,
) -> String {
    let true_name = || {
        name.map(|n| n.what.clone())
            .unwrap_or_else(|| "item".to_string())
    };
    if let Some(p) = potion {
        if identified.potions.contains(&p.effect) {
            return true_name();
        }
        let appearance = appearances
            .potions
            .get(&p.effect)
            .cloned()
            .unwrap_or_else(|| "strange".to_string());
        return format!("{appearance} potion");
    }
    if let Some(s) = scroll {
        if identified.scrolls.contains(&s.effect) {
            return true_name();
        }
        let appearance = appearances
            .scrolls
            .get(&s.effect)
            .cloned()
            .unwrap_or_else(|| "unreadable".to_string());
        return format!("scroll labeled {appearance}");
    }
    if let Some(w) = wand {
        if identified.wands.contains(&w.effect) {
            return true_name();
        }
        let appearance = appearances
            .wands
            .get(&w.effect)
            .cloned()
            .unwrap_or_else(|| "strange".to_string());
        return format!("{appearance} wand");
    }
    if let Some(r) = ring {
        if identified.rings.contains(&r.effect) {
            return true_name();
        }
        let appearance = appearances
            .rings
            .get(&r.effect)
            .cloned()
            .unwrap_or_else(|| "plain".to_string());
        return format!("{appearance} ring");
    }
    let name = true_name();
    // A stack says how many it is: one pack slot reading "7 arrows".
    match stack.map(|s| s.count).filter(|&c| c > 1) {
        Some(count) => format!("{count} {name}s"),
        None => name,
    }
}

/// `name` in a quantity, as it reads in a sentence: `"a dagger"`, `"7 arrows"`.
/// Plurals are a bare `-s`, which is all the catalog ever needs.
pub fn counted(name: &str, count: u8) -> String {
    if count <= 1 {
        return format!("{} {name}", article_for(name));
    }
    format!("{count} {name}s")
}

/// `name` as it reads in a sentence, prefixed with an article unless it
/// already carries its own count or built-in definite article — `"a dagger"`,
/// but `"7 arrows"` and `"The Element of Yoord"` stand as they are.
pub fn phrase_for(name: &str) -> String {
    if name.starts_with(|c: char| c.is_ascii_digit()) || name.starts_with("The ") {
        return name.to_string();
    }
    format!("{} {name}", article_for(name))
}

/// `item` as it reads in a sentence — `"a dagger"`, `"7 arrows"`, `"The Element
/// of Yoord"`. A stack counts itself and the relic carries its own article, so
/// neither takes an "a".
pub fn with_article(world: &World, item: Entity) -> String {
    phrase_for(&display_name(world, item))
}

/// The indefinite article that reads correctly before `s`: `"an"` before a
/// vowel sound, `"a"` otherwise. The one place this rule lives — [`Name::article`]
/// and [`crate::TrapEffect::label_article`] both defer here, and it works on any
/// display string, not just a [`Name`] (an appearance is not a [`Name`]).
/// Good enough for roog's vocabulary — no "an hour" / "a unicorn" edge cases.
pub fn article_for(s: &str) -> &'static str {
    match s.chars().next() {
        Some(c) if matches!(c.to_ascii_lowercase(), 'a' | 'e' | 'i' | 'o' | 'u') => "an",
        _ => "a",
    }
}

/// Prefixes `name` with "the", unless `name` already carries its own built-in
/// definite article — true only of the Element of Yoord, whose display name
/// is "The Element of Yoord" rather than a generic countable item. Avoids
/// "You can't use the The Element of Yoord right now."
pub fn with_the(name: &str) -> String {
    if name.starts_with("The ") {
        return name.to_string();
    }
    format!("the {name}")
}

/// Putting a ring on tells you what it is — a ring's only "use" is wearing it,
/// so that is where it gets identified. Called by
/// [`crate::equipment::toggle_equipped`] for every item; only rings answer.
pub fn learn_by_wearing(world: &mut World, item: Entity) {
    let Some(effect) = world.get::<Ring>(item).map(|r| r.effect) else {
        return;
    };
    let true_name = item_label(world, item);
    if world.resource_mut::<Identified>().rings.insert(effect) {
        world
            .resource_mut::<crate::components::GameLog>()
            .add(format!("That was {} {true_name}!", article_for(&true_name)));
    }
}
