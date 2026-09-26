//! Item identification, equipment-only: a weapon, suit of armour or launcher
//! hides its enchantment plus and cursed status until [`KnownQuality`] says
//! otherwise — set the moment it's worn (see [`crate::equipment::toggle_equipped`])
//! or a scroll of identify singles it out. Potions, scrolls, rings and wands
//! are always shown by their true [`Name`]: nothing about *what kind* of item
//! something is stays hidden, only a piece of gear's own quality can be
//! unknown.

use bevy_ecs::prelude::*;

use crate::components::{Curse, KnownQuality, Name, Stack, Vorpal};
use crate::effects::{ArmorBonus, PowerBonus, ThrowBonus};

/// What the player actually sees for `item`: its true [`Name`], a [`Stack`]
/// count if it holds more than one, and — once [`known_quality`] says either
/// is actually known — a weapon, suit of armour or launcher additionally gets
/// an enchantment plus in front and a `(cursed)` tag after.
pub fn display_name(world: &World, item: Entity) -> String {
    let base = named_display(world.get::<Name>(item), world.get::<Stack>(item));
    annotate_quality(world, item, base)
}

/// Whether `item`'s own enchantment plus and curse status are visible yet.
/// Every kind of gear answers through its own [`KnownQuality`], set the
/// moment it's worn (see [`crate::equipment::toggle_equipped`]) or a scroll of
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
/// label — see [`crate::visibility::spotted_line`].
pub fn named_display(name: Option<&Name>, stack: Option<&Stack>) -> String {
    let name = name
        .map(|n| n.what.clone())
        .unwrap_or_else(|| "item".to_string());
    // A stack says how many it is: one pack slot reading "7 arrows".
    match stack.map(|s| s.count).filter(|&c| c > 1) {
        Some(count) => format!("{count} {name}s"),
        None => name,
    }
}

// Localization note: `counted`, `phrase_for` and `article_for` below encode
// English grammar rules -- a bare `-s` plural, an "a"/"an" article chosen off
// the first letter's sound -- as if they were universal. They aren't:
// Portuguese and Spanish need gender agreement on the article (`"un daga"` is
// wrong, `"una daga"` is right), and none of the three languages' plurals are
// a suffix in the general case. This is silently fine only because `pt`/`es`/
// `ht` currently re-export `en`'s text wholesale (see `strings/src/lib.rs`);
// a real translation calling into these functions will read as broken
// English grammar bolted onto translated words. Fixing it means each
// language deciding its own article/plural rule, not adding cases here --
// out of scope for the string-extraction pass that added this note.

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
/// Good enough for nihilurk's vocabulary — no "an hour" / "a unicorn" edge cases.
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
