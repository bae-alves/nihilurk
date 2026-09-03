//! Every item in the game, one row each.
//!
//! This is the file you edit to add content. A row says what a thing is called,
//! how it draws, and which components it carries into the world — and that is
//! the whole of it. Nothing else in the codebase enumerates items: the loot
//! table rolls a category and picks a row ([`crate::map`]), the identification
//! system shuffles appearances over the rows ([`crate::identify`]), and combat
//! reads the components the rows attached ([`crate::effects`]).
//!
//! Rings are the clearest case of the design. A ring of protection is not a
//! `RingEffect::Protection` that seven files have to recognise; it is an item
//! carrying [`ArmorBonus`]`(2)`, which combat already folds in for plate mail.
//! A ring of perception is an item that [`Grants`] [`SeesInvisible`] to whoever
//! wears it, which the visibility system already asks about. Adding "ring of
//! fire resistance" is one row here and no other edit anywhere.

use bevy_ecs::prelude::*;
use crossterm::style::Color;
use rand::Rng;
use rand_chacha::ChaCha12Rng;

use crate::components::*;
use crate::effects::*;
use crate::equipment::{Equipped, Slot};

// ---------------------------------------------------------------------------
// The shape every catalog row shares
// ---------------------------------------------------------------------------

/// One spawnable item kind. Implemented by every table row below so the loot
/// roller can treat all categories alike.
pub trait ItemDef {
    /// The item exactly as the table describes it — no random rolls. This is
    /// what tests and scripted spawns want.
    fn spawn(&self, world: &mut World, pos: Position) -> Entity;

    /// The item as a dungeon floor produces it: the same thing, plus whatever
    /// the floor rolls for it — an enchantment, a battery charge. Categories
    /// with nothing to roll inherit this as-is.
    fn spawn_as_loot(&self, world: &mut World, rng: &mut ChaCha12Rng, pos: Position) -> Entity {
        let _ = rng;
        self.spawn(world, pos)
    }
}

/// Attaches a modifier component only when it's worth attaching, so a plain
/// dagger doesn't drag an `ArmorBonus(0)` through every archetype.
fn insert_modifier<C: Modifier>(entity: &mut bevy_ecs::world::EntityWorldMut, value: C) {
    if value.amount() != 0 {
        entity.insert(value);
    }
}

// ---------------------------------------------------------------------------
// Potions
// ---------------------------------------------------------------------------

/// A potion: quaffed once, then gone. What it *does* lives in
/// [`crate::items::apply_potion_effect`], keyed by [`PotionDef::effect`].
pub struct PotionDef {
    pub effect: PotionEffect,
    pub name: &'static str,
    pub color: Color,
}

impl ItemDef for PotionDef {
    fn spawn(&self, world: &mut World, pos: Position) -> Entity {
        world
            .spawn((
                Name { what: self.name.to_string() },
                Renderable { glyph: '!', color: self.color },
                pos,
                Item,
                Potion { effect: self.effect },
                Consume,
            ))
            .id()
    }
}

#[rustfmt::skip]
pub const POTIONS: &[PotionDef] = &[
    PotionDef { effect: PotionEffect::Confusion,       name: "potion of confusion",        color: Color::Magenta },
    PotionDef { effect: PotionEffect::Paralysis,       name: "potion of paralysis",        color: Color::DarkGrey },
    PotionDef { effect: PotionEffect::Poison,          name: "potion of poison",           color: Color::Green },
    PotionDef { effect: PotionEffect::GainStrength,    name: "potion of gain strength",    color: Color::Red },
    PotionDef { effect: PotionEffect::SeeInvisible,    name: "potion of see invisible",    color: Color::Cyan },
    PotionDef { effect: PotionEffect::Healing,         name: "potion of healing",          color: Color::Red },
    PotionDef { effect: PotionEffect::MonsterDetection,name: "potion of monster detection",color: Color::Yellow },
    PotionDef { effect: PotionEffect::MagicDetection,  name: "potion of magic detection",  color: Color::Yellow },
    PotionDef { effect: PotionEffect::RaiseLevel,      name: "potion of raise level",      color: Color::White },
    PotionDef { effect: PotionEffect::ExtraHealing,    name: "potion of extra healing",    color: Color::Red },
    PotionDef { effect: PotionEffect::Haste,           name: "potion of haste self",       color: Color::DarkYellow },
    PotionDef { effect: PotionEffect::RestoreStrength, name: "potion of restore strength", color: Color::Red },
    PotionDef { effect: PotionEffect::Blindness,       name: "potion of blindness",        color: Color::DarkGrey },
    PotionDef { effect: PotionEffect::Water,           name: "potion of thirst quenching", color: Color::Blue },
];

// ---------------------------------------------------------------------------
// Scrolls
// ---------------------------------------------------------------------------

/// A scroll: read once, then it crumbles. Its mechanic lives in
/// [`crate::items::apply_scroll_effect`], keyed by [`ScrollDef::effect`].
pub struct ScrollDef {
    pub effect: ScrollEffect,
    pub name: &'static str,
}

impl ItemDef for ScrollDef {
    fn spawn(&self, world: &mut World, pos: Position) -> Entity {
        world
            .spawn((
                Name { what: self.name.to_string() },
                Renderable { glyph: '?', color: Color::White },
                pos,
                Item,
                Scroll { effect: self.effect },
                Consume,
            ))
            .id()
    }
}

#[rustfmt::skip]
pub const SCROLLS: &[ScrollDef] = &[
    ScrollDef { effect: ScrollEffect::MonsterConfusion,  name: "scroll of monster confusion" },
    ScrollDef { effect: ScrollEffect::MagicMapping,      name: "scroll of magic mapping" },
    ScrollDef { effect: ScrollEffect::HoldMonster,       name: "scroll of hold monster" },
    ScrollDef { effect: ScrollEffect::Sleep,             name: "scroll of sleep" },
    ScrollDef { effect: ScrollEffect::EnchantArmor,      name: "scroll of enchant armor" },
    ScrollDef { effect: ScrollEffect::Identify,          name: "scroll of identify" },
    ScrollDef { effect: ScrollEffect::ScareMonster,      name: "scroll of scare monster" },
    ScrollDef { effect: ScrollEffect::FoodDetection,     name: "scroll of food detection" },
    ScrollDef { effect: ScrollEffect::Teleportation,     name: "scroll of teleportation" },
    ScrollDef { effect: ScrollEffect::EnchantWeapon,     name: "scroll of enchant weapon" },
    ScrollDef { effect: ScrollEffect::CreateMonster,     name: "scroll of create monster" },
    ScrollDef { effect: ScrollEffect::RemoveCurse,       name: "scroll of remove curse" },
    ScrollDef { effect: ScrollEffect::AggravateMonsters, name: "scroll of aggravate monsters" },
    ScrollDef { effect: ScrollEffect::BlankPaper,        name: "scroll of blank paper" },
    ScrollDef { effect: ScrollEffect::VorpalizeWeapon,   name: "scroll of vorpalize weapon" },
];

// ---------------------------------------------------------------------------
// Wands
// ---------------------------------------------------------------------------

/// A wand: zapped until its battery runs dry. `range` feeds the aiming reticle.
pub struct WandDef {
    pub effect: WandEffect,
    pub name: &'static str,
    pub color: Color,
    pub range: i32,
}

/// A wand's battery: `3d4` charges, rolled when it enters the dungeon.
pub fn roll_wand_charges(rng: &mut ChaCha12Rng) -> i8 {
    (0..3).map(|_| rng.gen_range(1..=4)).sum()
}

impl ItemDef for WandDef {
    fn spawn(&self, world: &mut World, pos: Position) -> Entity {
        world
            .spawn((
                Name { what: self.name.to_string() },
                Renderable { glyph: '/', color: self.color },
                pos,
                Item,
                Wand { effect: self.effect },
                Ranged { range: self.range },
                Battery { charges: 0 },
            ))
            .id()
    }

    fn spawn_as_loot(&self, world: &mut World, rng: &mut ChaCha12Rng, pos: Position) -> Entity {
        let wand = self.spawn(world, pos);
        let charges = roll_wand_charges(rng);
        if let Some(mut battery) = world.get_mut::<Battery>(wand) {
            battery.charges = charges;
        }
        wand
    }
}

#[rustfmt::skip]
pub const WANDS: &[WandDef] = &[
    WandDef { effect: WandEffect::Light,        name: "wand of light",         color: Color::Yellow,      range: 8 },
    WandDef { effect: WandEffect::Striking,     name: "wand of striking",      color: Color::White,       range: 6 },
    WandDef { effect: WandEffect::Lightning,    name: "wand of lightning",     color: Color::Cyan,        range: 8 },
    WandDef { effect: WandEffect::Fire,         name: "wand of fire",          color: Color::Red,         range: 8 },
    WandDef { effect: WandEffect::Cold,         name: "wand of cold",          color: Color::Blue,        range: 8 },
    WandDef { effect: WandEffect::Polymorph,    name: "wand of polymorph",     color: Color::Magenta,     range: 6 },
    WandDef { effect: WandEffect::MagicMissile, name: "wand of magic missile", color: Color::Cyan,        range: 6 },
    WandDef { effect: WandEffect::HasteMonster, name: "wand of haste monster", color: Color::DarkYellow,  range: 6 },
    WandDef { effect: WandEffect::SlowMonster,  name: "wand of slow monster",  color: Color::DarkCyan,    range: 6 },
    WandDef { effect: WandEffect::DrainLife,    name: "wand of drain life",    color: Color::DarkRed,     range: 6 },
    WandDef { effect: WandEffect::Nothing,      name: "wand of nothing",       color: Color::DarkGrey,    range: 6 },
    WandDef { effect: WandEffect::TeleportAway, name: "wand of teleport away", color: Color::Green,       range: 8 },
    WandDef { effect: WandEffect::TeleportTo,   name: "wand of teleport to",   color: Color::Green,       range: 8 },
    WandDef { effect: WandEffect::Cancellation, name: "wand of cancellation",  color: Color::DarkMagenta, range: 6 },
];

// ---------------------------------------------------------------------------
// Weapons and armour
// ---------------------------------------------------------------------------

/// A weapon. `power_die` is what wielding it adds to the attack die — the
/// classic weapon class, expressed as the [`PowerDie`] modifier combat already
/// knows how to fold in.
pub struct WeaponDef {
    pub name: &'static str,
    pub color: Color,
    pub power_die: i32,
}

impl ItemDef for WeaponDef {
    fn spawn(&self, world: &mut World, pos: Position) -> Entity {
        world
            .spawn((
                Name { what: self.name.to_string() },
                Renderable { glyph: ')', color: self.color },
                pos,
                Item,
                Equipped::loose(Slot::Hand),
                PowerDie(self.power_die),
                // A weapon is as dangerous thrown as it is swung — its class is
                // the die either way.
                ThrownDamage(self.power_die),
            ))
            .id()
    }

    fn spawn_as_loot(&self, world: &mut World, rng: &mut ChaCha12Rng, pos: Position) -> Entity {
        let e = self.spawn(world, pos);
        enchant_equipment(world, rng, e);
        e
    }
}

#[rustfmt::skip]
pub const WEAPONS: &[WeaponDef] = &[
    WeaponDef { name: "dagger",           color: Color::Grey,     power_die:  4 },
    WeaponDef { name: "mace",             color: Color::DarkGrey, power_die:  6 },
    WeaponDef { name: "long sword",       color: Color::White,    power_die:  8 },
    WeaponDef { name: "two-handed sword", color: Color::Cyan,     power_die: 10 },
];

/// A suit of armour. `armor_die` is what wearing it adds to the defence die.
pub struct ArmorDef {
    pub name: &'static str,
    pub color: Color,
    pub armor_die: i32,
}

impl ItemDef for ArmorDef {
    fn spawn(&self, world: &mut World, pos: Position) -> Entity {
        world
            .spawn((
                Name { what: self.name.to_string() },
                Renderable { glyph: ']', color: self.color },
                pos,
                Item,
                Equipped::loose(Slot::Body),
                ArmorDie(self.armor_die),
            ))
            .id()
    }

    fn spawn_as_loot(&self, world: &mut World, rng: &mut ChaCha12Rng, pos: Position) -> Entity {
        let e = self.spawn(world, pos);
        enchant_equipment(world, rng, e);
        e
    }
}

#[rustfmt::skip]
pub const ARMORS: &[ArmorDef] = &[
    ArmorDef { name: "leather armor",          color: Color::DarkYellow, armor_die: 2 },
    ArmorDef { name: "ring mail",              color: Color::Grey,       armor_die: 3 },
    ArmorDef { name: "studded leather armor",  color: Color::DarkYellow, armor_die: 4 },
    ArmorDef { name: "scale mail",             color: Color::Grey,       armor_die: 5 },
    ArmorDef { name: "chain mail",             color: Color::Grey,       armor_die: 6 },
    ArmorDef { name: "splint mail",            color: Color::White,      armor_die: 7 },
    ArmorDef { name: "banded mail",            color: Color::White,      armor_die: 8 },
    ArmorDef { name: "plate mail",             color: Color::Cyan,       armor_die: 9 },
];

// ---------------------------------------------------------------------------
// Rings
// ---------------------------------------------------------------------------

/// A ring: a modifier item, exactly like a sword or a suit of armour. It stacks
/// numbers through the same [`Modifier`] components combat already folds, and
/// lends marker effects to its wearer through [`Grants`]. There is no such thing
/// as "ring logic" anywhere else in the codebase.
pub struct RingDef {
    pub effect: RingEffect,
    pub name: &'static str,
    power_die: i32,
    power_bonus: i32,
    armor_die: i32,
    armor_bonus: i32,
    /// The marker effects this ring lends its wearer. Read back on load, so a
    /// saved ring never has to store what its row already says.
    pub grants: &'static [Grant],
}

impl RingDef {
    const fn new(effect: RingEffect, name: &'static str) -> Self {
        Self { effect, name, power_die: 0, power_bonus: 0, armor_die: 0, armor_bonus: 0, grants: &[] }
    }

    /// Flat modifier on the wearer's damage roll.
    const fn power_bonus(mut self, n: i32) -> Self {
        self.power_bonus = n;
        self
    }

    /// Flat modifier on the wearer's armour roll.
    const fn armor_bonus(mut self, n: i32) -> Self {
        self.armor_bonus = n;
        self
    }

    /// Marker effects the wearer gains while it's on.
    const fn grants(mut self, grants: &'static [Grant]) -> Self {
        self.grants = grants;
        self
    }

    /// The catalog row for `effect`. Panics on a ring that isn't in the table —
    /// which would mean a [`RingEffect`] variant nobody ever gave a row.
    pub fn of(effect: RingEffect) -> &'static RingDef {
        RINGS
            .iter()
            .find(|r| r.effect == effect)
            .unwrap_or_else(|| panic!("no ring row for {effect:?}"))
    }
}

impl ItemDef for RingDef {
    fn spawn(&self, world: &mut World, pos: Position) -> Entity {
        let mut e = world.spawn((
            Name { what: self.name.to_string() },
            Renderable { glyph: '=', color: Color::Yellow },
            pos,
            Item,
            Ring { effect: self.effect },
            Equipped::loose(Slot::Finger),
        ));
        insert_modifier(&mut e, PowerDie(self.power_die));
        insert_modifier(&mut e, PowerBonus(self.power_bonus));
        insert_modifier(&mut e, ArmorDie(self.armor_die));
        insert_modifier(&mut e, ArmorBonus(self.armor_bonus));
        if !self.grants.is_empty() {
            e.insert(Grants(self.grants));
        }
        e.id()
    }

    fn spawn_as_loot(&self, world: &mut World, rng: &mut ChaCha12Rng, pos: Position) -> Entity {
        let e = self.spawn(world, pos);
        enchant_equipment(world, rng, e);
        e
    }
}

/// The rings. Eight of the twelve are still inert — they have a name and an
/// appearance but no row content yet, which is exactly what "unwired" now looks
/// like: give one a `.power_bonus(2)` or a `.grants(...)` and it works, with no
/// other file touched.
#[rustfmt::skip]
pub const RINGS: &[RingDef] = &[
    RingDef::new(RingEffect::Protection, "ring of protection")
        .armor_bonus(2),
    // Rogue's separate add-strength and sustain-strength rings, merged.
    RingDef::new(RingEffect::Strength, "ring of strength")
        .power_bonus(2)
        .grants(&[Grant::of::<SustainsStrength>()]),
    // Rogue's separate searching and see-invisible rings, merged.
    RingDef::new(RingEffect::Perception, "ring of perception")
        .grants(&[Grant::of::<SeesInvisible>()]),
    RingDef::new(RingEffect::AggravateMonster, "ring of aggravate monster")
        .grants(&[Grant::of::<AggravatesMonsters>()]),
    RingDef::new(RingEffect::Adornment,      "ring of adornment"),
    RingDef::new(RingEffect::Dexterity,      "ring of dexterity"),
    RingDef::new(RingEffect::IncreaseDamage, "ring of increase damage"),
    RingDef::new(RingEffect::Regeneration,   "ring of regeneration"),
    RingDef::new(RingEffect::SlowDigestion,  "ring of slow digestion"),
    RingDef::new(RingEffect::Teleportation,  "ring of teleportation"),
    RingDef::new(RingEffect::Stealth,        "ring of stealth"),
    RingDef::new(RingEffect::MaintainArmor,  "ring of maintain armor"),
];

// ---------------------------------------------------------------------------
// Coins and the relic
// ---------------------------------------------------------------------------

/// Loose treasure. Rogue's food slot; here it's what you cash in at the end.
pub struct CoinDef {
    pub name: &'static str,
    pub color: Color,
    pub value: i32,
}

impl ItemDef for CoinDef {
    fn spawn(&self, world: &mut World, pos: Position) -> Entity {
        world
            .spawn((
                Name { what: self.name.to_string() },
                Renderable { glyph: '$', color: self.color },
                pos,
                Value { amount: self.value },
                Item,
            ))
            .id()
    }
}

#[rustfmt::skip]
pub const COINS: &[CoinDef] = &[
    CoinDef { name: "gold coin",   color: Color::Yellow, value: 1000 },
    CoinDef { name: "silver coin", color: Color::Grey,   value:  100 },
];

/// The Element of Yoord: the relic each run retrieves from the deepest floor,
/// spawned in place of that floor's down-stair. Carrying it flips the staircase
/// rules (see [`crate::map::change_level`]) so the player can climb back out.
pub fn spawn_element_of_yoord(world: &mut World, pos: Position) -> Entity {
    world
        .spawn((
            Name { what: String::from("The Element of Yoord") },
            Renderable { glyph: '\"', color: Color::Magenta },
            pos,
            Value { amount: 25000 },
            Item,
            Amulet,
        ))
        .id()
}

// ---------------------------------------------------------------------------
// Spawning by name or effect
// ---------------------------------------------------------------------------

/// Looks a row up by name in `table`, panicking on a miss — callers pass string
/// literals straight from the catalog, same as [`crate::MonsterDef::named`].
fn named<'a, D>(table: &'a [D], name: &str, name_of: fn(&D) -> &'static str, kind: &str) -> &'a D {
    table
        .iter()
        .find(|d| name_of(d) == name)
        .unwrap_or_else(|| panic!("no {kind} named {name:?}"))
}

pub fn spawn_potion(world: &mut World, effect: PotionEffect, pos: Position) -> Entity {
    POTIONS.iter().find(|d| d.effect == effect).expect("potion row").spawn(world, pos)
}

pub fn spawn_scroll(world: &mut World, effect: ScrollEffect, pos: Position) -> Entity {
    SCROLLS.iter().find(|d| d.effect == effect).expect("scroll row").spawn(world, pos)
}

pub fn spawn_wand(world: &mut World, effect: WandEffect, pos: Position) -> Entity {
    WANDS.iter().find(|d| d.effect == effect).expect("wand row").spawn(world, pos)
}

pub fn spawn_ring(world: &mut World, effect: RingEffect, pos: Position) -> Entity {
    RingDef::of(effect).spawn(world, pos)
}

pub fn spawn_weapon(world: &mut World, name: &str, pos: Position) -> Entity {
    named(WEAPONS, name, |d| d.name, "weapon").spawn(world, pos)
}

pub fn spawn_armor(world: &mut World, name: &str, pos: Position) -> Entity {
    named(ARMORS, name, |d| d.name, "armor").spawn(world, pos)
}

pub fn spawn_coin(world: &mut World, name: &str, pos: Position) -> Entity {
    named(COINS, name, |d| d.name, "coin").spawn(world, pos)
}

// ---------------------------------------------------------------------------
// Enchantment
// ---------------------------------------------------------------------------

/// The quality every weapon, armour and ring drop rolls when it spawns.
///
/// | Quality     | Odds | Bonus (equal-probability integer) |
/// |-------------|------|-----------------------------------|
/// | Normal      | 25%  | +0                                |
/// | Exceptional | 10%  | +1 .. +3                          |
/// | Cursed      | 65%  | -6 .. +4 (yes, a cursed item can roll positive) |
///
/// The bonus lands on the flat modifier, never the die size.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Quality {
    Normal,
    Exceptional,
    Cursed,
}

impl Quality {
    fn roll(rng: &mut ChaCha12Rng) -> Self {
        match rng.gen_range(0..100) {
            0..=24 => Quality::Normal,
            25..=34 => Quality::Exceptional,
            _ => Quality::Cursed,
        }
    }
}

/// Rolls quality for a freshly spawned piece of gear and stamps the result on:
/// the flat bonus onto whichever roll the item contributes to, and a [`Curse`]
/// tag if it came up cursed.
///
/// Which bonus applies is read off the item itself — a thing with a [`PowerDie`]
/// is a weapon, a thing with an [`ArmorDie`] is armour — so a future item that
/// does both gets both, and a ring (which has neither) gets only the tag.
pub fn enchant_equipment(world: &mut World, rng: &mut ChaCha12Rng, item: Entity) {
    let quality = Quality::roll(rng);
    let bonus: i32 = match quality {
        Quality::Normal => 0,
        Quality::Exceptional => rng.gen_range(1..=3),
        Quality::Cursed => rng.gen_range(-5..=5),
    };

    let mut entity = world.entity_mut(item);
    if entity.get::<PowerDie>().is_some() {
        let base = entity.get::<PowerBonus>().map(|b| b.0).unwrap_or(0);
        entity.insert(PowerBonus(base + bonus));
    }
    if entity.get::<ArmorDie>().is_some() {
        let base = entity.get::<ArmorBonus>().map(|b| b.0).unwrap_or(0);
        entity.insert(ArmorBonus(base + bonus));
    }
    if quality == Quality::Cursed {
        entity.insert(Curse);
    }
}
