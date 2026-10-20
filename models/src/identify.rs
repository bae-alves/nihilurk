//! Item identification, equipment-only: a weapon, suit of armour or launcher
//! hides its enchantment plus and cursed status until [`KnownQuality`] says
//! otherwise — set the moment it's worn (see [`crate::equipment::toggle_equipped`])
//! or a scroll of identify singles it out. Potions, scrolls, rings and wands
//! are always shown by their true [`Name`]: nothing about *what kind* of item
//! something is stays hidden, only a piece of gear's own quality can be
//! unknown.

use bevy_ecs::prelude::*;

use crate::combat::mean_blow;
use crate::components::{
    Ammo, Curse, Fighter, Item, KnownQuality, MegaCurse, Mob, Name, Player, Reach, Rune, Speed,
    SpeedKind, Stack, ThrownDamage, Trap, Vorpal,
};
use crate::effects::{ArmorBonus, ArmorDie, MaxHpBonus, PowerBonus, PowerDie, ThrowBonus};

/// What the player actually sees for `item`: its true [`Name`], a [`Stack`]
/// count if it holds more than one, and — once [`known_quality`] says either
/// is actually known — a weapon, suit of armour or launcher additionally gets
/// an enchantment plus in front and a `(cursed)` (`(MEGACURSED)`) tag after. A rune with
/// nothing left in it is tagged `(inert)`.
pub fn display_name(world: &World, item: Entity) -> String {
    let base = named_display(world.get::<Name>(item), world.get::<Stack>(item));
    let name = annotate_quality(world, item, base);
    match world.get::<Rune>(item) {
        Some(rune) if !rune.charged => format!("{name} ({})", strings::rune_inert_suffix()),
        _ => name,
    }
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
pub(crate) fn enchantment_plus(world: &World, item: Entity) -> i32 {
    world.get::<PowerBonus>(item).map(|b| b.0).unwrap_or(0)
        + world.get::<ArmorBonus>(item).map(|b| b.0).unwrap_or(0)
        + world.get::<ThrowBonus>(item).map(|b| b.0).unwrap_or(0)
        + world.get::<MaxHpBonus>(item).map(|b| b.0).unwrap_or(0)
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
    let name = if world.get::<MegaCurse>(item).is_some() {
        format!("{name} (MEGACURSED)")
    } else if world.get::<Curse>(item).is_some() {
        format!("{name} (cursed)")
    } else {
        name
    };
    match world.get::<Vorpal>(item) {
        Some(v) => format!("{name} (vorpal vs. {})", v.bane),
        None => name,
    }
}

/// The lines the pack's Info row shows for `item`: its name, the numbers its
/// components carry, one authored line for the row if it has one, and — only
/// once [`known_quality`] says so — its enchantment plus and what a curse
/// means. Everything but the last two is always visible, so nothing here can
/// give away what [`known_quality`] is still hiding.
pub fn item_info(world: &World, item: Entity) -> Vec<String> {
    let mut lines = vec![display_name(world, item)];
    if let Some(d) = world.get::<PowerDie>(item) {
        lines.push(strings::info_melee(d.0));
    }
    if let Some(d) = world.get::<ArmorDie>(item) {
        lines.push(strings::info_armor(d.0));
    }
    if let Some(d) = world.get::<ThrownDamage>(item) {
        lines.push(strings::info_thrown(d.0));
    }
    if let Some(a) = world.get::<Ammo>(item) {
        lines.push(strings::info_launched(a.row.launched_die));
    }
    if let Some(r) = world.get::<Reach>(item) {
        lines.push(strings::info_reach(r.0));
    }
    if let Some(blurb) = strings::item_blurb(&crate::helpers::item_label(world, item)) {
        lines.push(blurb.to_string());
    }
    if known_quality(world, item) {
        let plus = enchantment_plus(world, item);
        if plus != 0 {
            lines.push(strings::info_enchantment(plus));
        }
        if world.get::<MegaCurse>(item).is_some() {
            lines.push(strings::megacursed_explained().to_string());
        } else if world.get::<Curse>(item).is_some() {
            lines.push(strings::cursed_explained().to_string());
        }
    }
    lines
}

/// The lines `?` on the look reticle (and a look click) add under the sighting
/// line for `entity`: the pack's Info lines for an item (minus its name, the
/// sighting already gave that), a creature's numbers, or what a trap does.
/// Empty for anything else. A trap not yet [`Trap::revealed`] is the caller's
/// to pass over, as for every other purpose.
pub fn tile_info(world: &World, entity: Entity) -> Vec<String> {
    if world.get::<Item>(entity).is_some() {
        return item_info(world, entity).split_off(1);
    }
    if let Some(f) = world.get::<Fighter>(entity) {
        let mut lines = vec![
            strings::info_health(f.hp, f.max_hp),
            strings::info_attack(f.power, f.power_bonus),
            strings::info_defence(f.armor, f.armor_bonus),
        ];
        if let Some(s) = world.get::<Speed>(entity) {
            lines.push(strings::info_speed(match s.kind {
                SpeedKind::Slow => strings::speed_slow(),
                SpeedKind::Normal => strings::speed_normal(),
                SpeedKind::Quick => strings::speed_quick(),
                SpeedKind::Fast => strings::speed_fast(),
            }));
        }
        if world.get::<Mob>(entity).is_some() {
            lines.extend(odds_against_player(world, entity));
        }
        return lines;
    }
    match world.get::<Trap>(entity) {
        Some(t) => {
            vec![strings::trap_explained(t.row.name, t.row.snare_turns)]
        }
        None => Vec::new(),
    }
}

/// "They will likely kill you in 4 hits" and its mirror, from
/// [`mean_blow`] each way against the hero's current health. Nothing when
/// there is no hero.
fn odds_against_player(world: &World, mob: Entity) -> Vec<String> {
    let Some(player) = world
        .iter_entities()
        .find(|e| e.contains::<Player>())
        .map(|e| e.id())
    else {
        return Vec::new();
    };
    let hits = |hp: i32, mean: f64| (mean > 0.0).then(|| (f64::from(hp) / mean).ceil() as u32);
    let hp_of = |e| world.get::<Fighter>(e).map_or(0, |f| f.hp);
    vec![
        strings::info_they_kill_you(hits(hp_of(player), mean_blow(world, mob, player))),
        strings::info_you_kill_them(hits(hp_of(mob), mean_blow(world, player, mob))),
    ]
}

/// The [`display_name`] base: the plain [`Name`], counted when stacked.
fn named_display(name: Option<&Name>, stack: Option<&Stack>) -> String {
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
/// and [`crate::TrapDef::display_article`] both defer here, and it works on any
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
