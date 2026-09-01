//! Item identification: potions, scrolls, wands and rings hide their true
//! nature behind a cosmetic appearance until the player learns otherwise —
//! the classic roguelike "unidentified item" trick (see NetHack).
//!
//! A [`Potion`]/[`Scroll`]/[`Wand`]/[`PutOn`] component always carries the
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

use crate::components::{Name, PotionEffect, PutOn, RingEffect, ScrollEffect, WandEffect};
use crate::items::item_label;

/// Every [`PotionEffect`] variant, used to build this run's appearance map.
const ALL_POTIONS: [PotionEffect; 15] = [
    PotionEffect::Blindness, PotionEffect::Confusion, PotionEffect::ExtraHealing,
    PotionEffect::FruitJuice, PotionEffect::GainStrength, PotionEffect::Haste,
    PotionEffect::Healing, PotionEffect::MagicDetection, PotionEffect::MonsterDetection,
    PotionEffect::Paralysis, PotionEffect::Poison, PotionEffect::RaiseLevel,
    PotionEffect::RestoreStrength, PotionEffect::SeeInvisible, PotionEffect::Water,
];

/// Every [`ScrollEffect`] variant, used to build this run's appearance map.
const ALL_SCROLLS: [ScrollEffect; 15] = [
    ScrollEffect::MonsterConfusion, ScrollEffect::MagicMapping, ScrollEffect::HoldMonster,
    ScrollEffect::Sleep, ScrollEffect::EnchantArmor, ScrollEffect::Identify,
    ScrollEffect::ScareMonster, ScrollEffect::FoodDetection, ScrollEffect::Teleportation,
    ScrollEffect::EnchantWeapon, ScrollEffect::CreateMonster, ScrollEffect::RemoveCurse,
    ScrollEffect::AggravateMonsters, ScrollEffect::BlankPaper, ScrollEffect::VorpalizeWeapon,
];

/// Every [`WandEffect`] variant, used to build this run's appearance map.
const ALL_WANDS: [WandEffect; 14] = [
    WandEffect::Light, WandEffect::Striking, WandEffect::Lightning, WandEffect::Fire,
    WandEffect::Cold, WandEffect::Polymorph, WandEffect::MagicMissile, WandEffect::HasteMonster,
    WandEffect::SlowMonster, WandEffect::DrainLife, WandEffect::Nothing, WandEffect::TeleportAway,
    WandEffect::TeleportTo, WandEffect::Cancellation,
];

/// Every [`RingEffect`] variant, used to build this run's appearance map.
const ALL_RINGS: [RingEffect; 12] = [
    RingEffect::Protection, RingEffect::Strength, RingEffect::Perception,
    RingEffect::Adornment, RingEffect::AggravateMonster, RingEffect::Dexterity,
    RingEffect::IncreaseDamage, RingEffect::Regeneration, RingEffect::SlowDigestion,
    RingEffect::Teleportation, RingEffect::Stealth, RingEffect::MaintainArmor,
];

/// Colours and consistencies, NetHack-style: pooled well beyond
/// [`ALL_POTIONS`]'s length so a shuffle always has room to spare.
const POTION_APPEARANCES: [&str; 20] = [
    "ruby", "pink", "orange", "amber", "emerald", "cyan", "violet", "brown", "grey", "yellow",
    "bubbly", "fizzy", "swirly", "milky", "murky", "cloudy", "smoky", "oily", "sparkling", "viscous",
];

/// Nonsense scroll titles, invented for roog rather than borrowed from
/// NetHack's own in-joke set.
const SCROLL_APPEARANCES: [&str; 20] = [
    "XLUM QUAZAR", "GNOR VEKTIL", "ZIMBO RASHT", "PLIN DRAVOK", "MORZ ELKATH",
    "YFEN CROSTIL", "WUBBA LENTHOR", "SKAR MUNDIL", "TAVROK ZIN", "ELDIC PHORN",
    "QUIX SABAROTH", "NELGO VASHT", "BRIN ZORATHIL", "HULK PENDRIL", "OMFA TRIXEL",
    "VASK ELDROON", "KYRIL BANTOX", "DREN QUOZAL", "SPLICK VORNAI", "TUM ELKHESH",
];

/// Wand materials, from mundane woods to oddities.
const WAND_APPEARANCES: [&str; 20] = [
    "oak", "balsa", "maple", "teak", "ebony", "iron", "brass", "copper", "zinc", "tin",
    "silver", "bronze", "steel", "glass", "crystal", "hexagonal", "marbled", "jeweled", "forked", "runed",
];

/// Ring gems and metals.
const RING_APPEARANCES: [&str; 20] = [
    "pearl", "iron", "twisted", "wire", "engagement", "diamond", "sapphire", "ruby", "wooden", "granite",
    "opal", "clay", "tiger-eye", "moonstone", "jade", "coral", "bronze", "spinel", "agate", "onyx",
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
    effects.iter().copied().zip(shuffled.into_iter().map(String::from)).collect()
}

impl ItemAppearances {
    /// Builds a fresh, randomised appearance map for a new run.
    pub fn generate(rng: &mut ChaCha12Rng) -> Self {
        Self {
            potions: assign(&ALL_POTIONS, &POTION_APPEARANCES, rng),
            scrolls: assign(&ALL_SCROLLS, &SCROLL_APPEARANCES, rng),
            wands: assign(&ALL_WANDS, &WAND_APPEARANCES, rng),
            rings: assign(&ALL_RINGS, &RING_APPEARANCES, rng),
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
/// (weapons, armor, gold, the amulet) always show their true [`Name`].
pub fn display_name(world: &World, item: Entity) -> String {
    if let Some(p) = world.get::<crate::components::Potion>(item) {
        if world.resource::<Identified>().potions.contains(&p.effect) {
            return item_label(world, item);
        }
        let appearance = world
            .resource::<ItemAppearances>()
            .potions
            .get(&p.effect)
            .cloned()
            .unwrap_or_else(|| "strange".to_string());
        return format!("{appearance} potion");
    }
    if let Some(s) = world.get::<crate::components::Scroll>(item) {
        if world.resource::<Identified>().scrolls.contains(&s.effect) {
            return item_label(world, item);
        }
        let appearance = world
            .resource::<ItemAppearances>()
            .scrolls
            .get(&s.effect)
            .cloned()
            .unwrap_or_else(|| "unreadable".to_string());
        return format!("scroll labeled {appearance}");
    }
    if let Some(w) = world.get::<crate::components::Wand>(item) {
        if world.resource::<Identified>().wands.contains(&w.effect) {
            return item_label(world, item);
        }
        let appearance = world
            .resource::<ItemAppearances>()
            .wands
            .get(&w.effect)
            .cloned()
            .unwrap_or_else(|| "strange".to_string());
        return format!("{appearance} wand");
    }
    if let Some(r) = world.get::<PutOn>(item) {
        if world.resource::<Identified>().rings.contains(&r.effect) {
            return item_label(world, item);
        }
        let appearance = world
            .resource::<ItemAppearances>()
            .rings
            .get(&r.effect)
            .cloned()
            .unwrap_or_else(|| "plain".to_string());
        return format!("{appearance} ring");
    }
    world.get::<Name>(item).map(|n| n.what.clone()).unwrap_or_else(|| "item".to_string())
}

/// The indefinite article that reads correctly before `s`: `"an"` before a
/// vowel sound, `"a"` otherwise. Same rule as [`Name::article`], but usable
/// on an arbitrary display string (an appearance is not a [`Name`]).
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
        name.to_string()
    } else {
        format!("the {name}")
    }
}
