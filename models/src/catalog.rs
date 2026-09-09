//! Every item in the game, one row each.
//!
//! This is the file you edit to add content. A row says what a thing is called,
//! how it draws, and which components it carries into the world — and that is
//! the whole of it. Nothing else in the codebase enumerates items: the loot
//! table rolls a category and picks a row ([`crate::map`]), the identification
//! system shuffles appearances over the rows ([`crate::identify`]), and combat
//! reads the components the rows attached ([`crate::effects`]).
//!
//! Bows are the newest case of it. A bow is not a weapon with a special "fires
//! arrows" mode; it is an item that [`Grants`] [`FireArrow`], and an arrow is an
//! item that says it answers to [`FireArrow`] ([`LaunchedBy`]). Neither knows the
//! other exists, and a sling is one row in each table away.
//!
//! Rings are the clearest case of the design. A ring of protection is not a
//! `RingEffect::Protection` that seven files have to recognise; it is an item
//! carrying [`ArmorBonus`]`(2)`, which combat already folds in for plate mail.
//! A ring of perception is an item that [`Grants`] [`SeesInvisible`] to whoever
//! wears it, which the visibility system already asks about. Adding "ring of
//! fire resistance" is a row here plus a [`RingEffect`] variant to be identified
//! by — and no behaviour code at all, anywhere.
//!
//! The full recipes, per category, are in `docs/how-to/add-an-item.md`.

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
    /// What the row is called. A name is a row's identity everywhere it
    /// travels: the save file stores it instead of the row's contents, and
    /// [`crate::spawn::spawn_named`] finds the row again from it.
    fn name(&self) -> &'static str;

    /// How often this row turns up relative to its table-mates. Ten is the
    /// baseline, so a row at 5 is half as common and one at 20 twice.
    ///
    /// Every row in the game currently sits at the default — within a category
    /// roog picks evenly, on purpose. Overriding it is how a category earns
    /// per-row rarity: give the struct a `weight: u32` field and return it here.
    fn weight(&self) -> u32 {
        10
    }

    /// The shallowest floor this row may drop on. The default lets it appear
    /// anywhere; raise it to keep a thing out of the early dungeon.
    fn min_depth(&self) -> u8 {
        1
    }

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
    fn name(&self) -> &'static str {
        self.name
    }

    fn spawn(&self, world: &mut World, pos: Position) -> Entity {
        world
            .spawn((
                Name {
                    what: self.name.to_string(),
                },
                Renderable {
                    glyph: '!',
                    color: self.color,
                },
                pos,
                Item,
                Potion {
                    effect: self.effect,
                },
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
    fn name(&self) -> &'static str {
        self.name
    }

    fn spawn(&self, world: &mut World, pos: Position) -> Entity {
        world
            .spawn((
                Name {
                    what: self.name.to_string(),
                },
                Renderable {
                    glyph: '?',
                    color: Color::White,
                },
                pos,
                Item,
                Scroll {
                    effect: self.effect,
                },
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

/// A wand's battery: `2d6 + 1` charges, rolled when it enters the dungeon.
pub fn roll_wand_charges(rng: &mut ChaCha12Rng) -> i8 {
    (0..2).map(|_| rng.gen_range(1..=6)).sum::<i8>() + 1
}

impl ItemDef for WandDef {
    fn name(&self) -> &'static str {
        self.name
    }

    fn spawn(&self, world: &mut World, pos: Position) -> Entity {
        world
            .spawn((
                Name {
                    what: self.name.to_string(),
                },
                Renderable {
                    glyph: '/',
                    color: self.color,
                },
                pos,
                Item,
                Wand {
                    effect: self.effect,
                },
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
///
/// Every weapon can be hurled; what separates one built for it from one that
/// merely tolerates it is `.missile()`. A mace leaves your hand as an improvised
/// lump — worth its own class, blunted by the target's armour, and liable to be
/// plucked out of the air by anything with hands and used on you. A dagger or a
/// spear is balanced for the flight ([`Projectile`]) and, being balanced, does
/// not stop at the first body it finds ([`Piercing`]).
pub struct WeaponDef {
    pub name: &'static str,
    pub color: Color,
    pub power_die: i32,
    /// The die it rolls when thrown. Defaults to `power_die` — a weapon is as
    /// dangerous thrown as it is swung unless the row says otherwise.
    thrown_die: i32,
    /// Purpose-built for throwing: ignores armour, is spent on what it hits, and
    /// is never caught.
    projectile: bool,
    /// Whether the throw carries on through everything on its line (see
    /// [`Piercing`]).
    piercing: bool,
}

impl WeaponDef {
    const fn new(name: &'static str, color: Color, power_die: i32) -> Self {
        Self {
            name,
            color,
            power_die,
            thrown_die: power_die,
            projectile: false,
            piercing: false,
        }
    }

    /// A weapon shaped to fly: it rolls `die` on impact rather than its own
    /// class, and behaves as a [`Projectile`].
    const fn missile(mut self, die: i32) -> Self {
        self.thrown_die = die;
        self.projectile = true;
        self
    }

    /// The throw does not stop at the first creature: it runs the whole line.
    const fn piercing(mut self) -> Self {
        self.piercing = true;
        self
    }
}

impl ItemDef for WeaponDef {
    fn name(&self) -> &'static str {
        self.name
    }

    fn spawn(&self, world: &mut World, pos: Position) -> Entity {
        let mut e = world.spawn((
            Name {
                what: self.name.to_string(),
            },
            Renderable {
                glyph: ')',
                color: self.color,
            },
            pos,
            Item,
            Equipped::loose(Slot::Hand),
            PowerDie(self.power_die),
            ThrownDamage(self.thrown_die),
        ));
        attach_flight(&mut e, self.projectile, self.piercing);
        e.id()
    }

    fn spawn_as_loot(&self, world: &mut World, rng: &mut ChaCha12Rng, pos: Position) -> Entity {
        let e = self.spawn(world, pos);
        enchant_equipment(world, rng, e);
        e
    }
}

#[rustfmt::skip]
pub const WEAPONS: &[WeaponDef] = &[
    WeaponDef::new("dagger",           Color::Grey,     4).missile(4).piercing(),
    WeaponDef::new("spear",            Color::DarkGrey, 6).missile(8).piercing(),
    WeaponDef::new("mace",             Color::DarkGrey, 6),
    WeaponDef::new("long sword",       Color::White,    8),
    WeaponDef::new("two-handed sword", Color::Cyan,    10),
];

// ---------------------------------------------------------------------------
// Ammunition and launchers
// ---------------------------------------------------------------------------

/// Ammunition: an arrow, a quarrel. Useless in the hand — it carries no
/// [`PowerDie`] and no [`Equipped`], so there is nothing to wield and nothing to
/// wear — and it exists only to be thrown.
///
/// Two things set it apart from every other item. It **stacks**: one pack slot
/// holds up to [`STACK_LIMIT`] of them, and throwing spends one off the top. And
/// it **answers to a launcher**: whoever throws it with `launched_by` already on
/// them looses it properly, for double the die.
pub struct AmmoDef {
    pub name: &'static str,
    pub color: Color,
    /// The die one of these rolls, lobbed by hand. A launcher doubles it.
    pub die: i32,
    /// The effect that turns a lob into a shot (see [`LaunchedBy`]).
    pub launched_by: Grant,
}

impl AmmoDef {
    /// How many a floor drop arrives in. Never a lone arrow — finding one arrow
    /// is not finding ammunition.
    fn roll_bundle(rng: &mut ChaCha12Rng) -> u8 {
        rng.gen_range(3..=12)
    }
}

impl ItemDef for AmmoDef {
    fn name(&self) -> &'static str {
        self.name
    }

    fn spawn(&self, world: &mut World, pos: Position) -> Entity {
        world
            .spawn((
                Name {
                    what: self.name.to_string(),
                },
                Renderable {
                    glyph: ')',
                    color: self.color,
                },
                pos,
                Item,
                ThrownDamage(self.die),
                Projectile,
                LaunchedBy(self.launched_by),
                Stack { count: 1 },
            ))
            .id()
    }

    fn spawn_as_loot(&self, world: &mut World, rng: &mut ChaCha12Rng, pos: Position) -> Entity {
        let bundle = Self::roll_bundle(rng);
        let e = self.spawn(world, pos);
        if let Some(mut stack) = world.get_mut::<Stack>(e) {
            stack.count = bundle;
        }
        e
    }
}

#[rustfmt::skip]
pub const AMMO: &[AmmoDef] = &[
    AmmoDef { name: "arrow",   color: Color::DarkYellow, die: 4, launched_by: Grant::of::<FireArrow>()   },
    AmmoDef { name: "quarrel", color: Color::Grey,       die: 6, launched_by: Grant::of::<FireQuarrel>() },
];

/// A bow or a crossbow. Like a ring, and unlike every other thing you hold, it
/// is a pure grant: no attack die, no armour die, nothing to roll. What it does
/// is put [`FireArrow`] (or [`FireQuarrel`]) on whoever draws it, which is the
/// only thing an arrow ever asks about.
///
/// Its enchantment has no melee roll to land on, so it lands on
/// [`ThrowBonus`] — see [`enchant_equipment`]. The [`ThrowBonus(0)`] every
/// launcher spawns with is what gives the enchantment somewhere to go.
///
/// It is also the only gear that carries a [`MeleeCap`]: a hand holding a bow
/// is a hand not holding a sword, and swinging the bow is worth a bruise
/// whatever else the wielder has on. A +5 crossbow is still a stick in a
/// corridor.
///
/// [`ThrowBonus(0)`]: ThrowBonus
pub struct LauncherDef {
    pub name: &'static str,
    pub color: Color,
    pub grants: &'static [Grant],
    /// The most this is worth swung at something. A launcher occupies the hand
    /// a sword would have had, and this is the price of that: drawn it is the
    /// best thing in the dungeon, clubbed it is a stick. See
    /// [`crate::effects::MeleeCap`].
    pub melee_cap: i32,
}

impl ItemDef for LauncherDef {
    fn name(&self) -> &'static str {
        self.name
    }

    fn spawn(&self, world: &mut World, pos: Position) -> Entity {
        world
            .spawn((
                Name {
                    what: self.name.to_string(),
                },
                Renderable {
                    glyph: '}',
                    color: self.color,
                },
                pos,
                Item,
                Equipped::loose(Slot::Hand),
                Launcher,
                ThrowBonus(0),
                Grants(self.grants),
                MeleeCap(self.melee_cap),
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
pub const LAUNCHERS: &[LauncherDef] = &[
    LauncherDef { name: "bow",      color: Color::DarkYellow, grants: &[Grant::of::<FireArrow>()],   melee_cap: 1 },
    LauncherDef { name: "crossbow", color: Color::DarkGrey,   grants: &[Grant::of::<FireQuarrel>()], melee_cap: 1 },
];

/// Attaches the two markers that describe how a thing behaves in flight, and
/// only when they say something — a mace carries neither.
fn attach_flight(entity: &mut bevy_ecs::world::EntityWorldMut, projectile: bool, piercing: bool) {
    if projectile {
        entity.insert(Projectile);
    }
    if piercing {
        entity.insert(Piercing);
    }
}

/// A suit of armour. `armor_die` is what wearing it adds to the defence die.
pub struct ArmorDef {
    pub name: &'static str,
    pub color: Color,
    pub armor_die: i32,
}

impl ItemDef for ArmorDef {
    fn name(&self) -> &'static str {
        self.name
    }

    fn spawn(&self, world: &mut World, pos: Position) -> Entity {
        world
            .spawn((
                Name {
                    what: self.name.to_string(),
                },
                Renderable {
                    glyph: ']',
                    color: self.color,
                },
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
    throw_bonus: i32,
    /// The marker effects this ring lends its wearer. Read back on load, so a
    /// saved ring never has to store what its row already says.
    pub grants: &'static [Grant],
}

impl RingDef {
    const fn new(effect: RingEffect, name: &'static str) -> Self {
        Self {
            effect,
            name,
            power_die: 0,
            power_bonus: 0,
            armor_die: 0,
            armor_bonus: 0,
            throw_bonus: 0,
            grants: &[],
        }
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

    /// Flat modifier on everything the wearer throws.
    const fn throw_bonus(mut self, n: i32) -> Self {
        self.throw_bonus = n;
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
    fn name(&self) -> &'static str {
        self.name
    }

    fn spawn(&self, world: &mut World, pos: Position) -> Entity {
        let mut e = world.spawn((
            Name {
                what: self.name.to_string(),
            },
            Renderable {
                glyph: '=',
                color: Color::Yellow,
            },
            pos,
            Item,
            Ring {
                effect: self.effect,
            },
            Equipped::loose(Slot::Finger),
        ));
        insert_modifier(&mut e, PowerDie(self.power_die));
        insert_modifier(&mut e, PowerBonus(self.power_bonus));
        insert_modifier(&mut e, ArmorDie(self.armor_die));
        insert_modifier(&mut e, ArmorBonus(self.armor_bonus));
        insert_modifier(&mut e, ThrowBonus(self.throw_bonus));
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

/// The rings. Seven of the twelve are still inert — they have a name and an
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
    // A steady hand: worth as much on a hurled dagger as on a loosed arrow.
    RingDef::new(RingEffect::Dexterity, "ring of dexterity")
        .throw_bonus(2),
    RingDef::new(RingEffect::Adornment,      "ring of adornment"),
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
    fn name(&self) -> &'static str {
        self.name
    }

    fn spawn(&self, world: &mut World, pos: Position) -> Entity {
        world
            .spawn((
                Name {
                    what: self.name.to_string(),
                },
                Renderable {
                    glyph: '$',
                    color: self.color,
                },
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
            Name {
                what: String::from(crate::spawn::ELEMENT_OF_YOORD),
            },
            Renderable {
                glyph: '\"',
                color: Color::Magenta,
            },
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
/// The forgiving version, for callers holding a name they did not write
/// themselves, is [`crate::spawn::spawn_named`].
fn named<'a, D: ItemDef>(table: &'a [D], name: &str, kind: &str) -> &'a D {
    table
        .iter()
        .find(|d| d.name() == name)
        .unwrap_or_else(|| panic!("no {kind} named {name:?}"))
}

pub fn spawn_potion(world: &mut World, effect: PotionEffect, pos: Position) -> Entity {
    POTIONS
        .iter()
        .find(|d| d.effect == effect)
        .expect("potion row")
        .spawn(world, pos)
}

pub fn spawn_scroll(world: &mut World, effect: ScrollEffect, pos: Position) -> Entity {
    SCROLLS
        .iter()
        .find(|d| d.effect == effect)
        .expect("scroll row")
        .spawn(world, pos)
}

pub fn spawn_wand(world: &mut World, effect: WandEffect, pos: Position) -> Entity {
    WANDS
        .iter()
        .find(|d| d.effect == effect)
        .expect("wand row")
        .spawn(world, pos)
}

pub fn spawn_ring(world: &mut World, effect: RingEffect, pos: Position) -> Entity {
    RingDef::of(effect).spawn(world, pos)
}

pub fn spawn_weapon(world: &mut World, name: &str, pos: Position) -> Entity {
    named(WEAPONS, name, "weapon").spawn(world, pos)
}

pub fn spawn_armor(world: &mut World, name: &str, pos: Position) -> Entity {
    named(ARMORS, name, "armor").spawn(world, pos)
}

pub fn spawn_coin(world: &mut World, name: &str, pos: Position) -> Entity {
    named(COINS, name, "coin").spawn(world, pos)
}

/// One arrow or quarrel. Ammunition arrives in bundles from the dungeon floor
/// ([`AmmoDef::spawn_as_loot`]); this is the single unit tests and splits want.
pub fn spawn_ammo(world: &mut World, name: &str, pos: Position) -> Entity {
    named(AMMO, name, "ammo").spawn(world, pos)
}

pub fn spawn_launcher(world: &mut World, name: &str, pos: Position) -> Entity {
    named(LAUNCHERS, name, "launcher").spawn(world, pos)
}

/// A fresh single unit of whatever `item` is a stack of, spawned nowhere in
/// particular — the one arrow that leaves a quiver when you shoot it. `None` if
/// `item` is not something the catalog knows how to make more of.
///
/// Re-rolling the row rather than copying the entity is the same trick the save
/// file plays: a catalog row is the definition, so it is always cheaper to look
/// one up by name than to remember what it said.
pub fn split_one(world: &mut World, item: Entity) -> Option<Entity> {
    let name = world.get::<Name>(item)?.what.clone();
    let def = AMMO.iter().find(|d| d.name == name)?;
    let one = def.spawn(world, Position { x: 0, y: 0 });
    world.entity_mut(one).remove::<Position>();
    Some(one)
}

/// Re-attaches what a catalog row gives an item that the save file does not
/// store: how a weapon behaves in flight, what a bow lends its wielder, what a
/// missile answers to. Keyed by name, the same way a ring's grants come back
/// from [`RingDef::of`] — the row is the definition, so a save that stored these
/// would only be storing the table twice.
pub fn restore_from_catalog(entity: &mut bevy_ecs::world::EntityWorldMut, name: &str) {
    if let Some(def) = WEAPONS.iter().find(|d| d.name == name) {
        entity.insert(ThrownDamage(def.thrown_die));
        attach_flight(entity, def.projectile, def.piercing);
    }
    if let Some(def) = AMMO.iter().find(|d| d.name == name) {
        entity.insert((
            ThrownDamage(def.die),
            Projectile,
            LaunchedBy(def.launched_by),
        ));
    }
    if let Some(def) = LAUNCHERS.iter().find(|d| d.name == name) {
        entity.insert((Launcher, Grants(def.grants), MeleeCap(def.melee_cap)));
    }
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
/// | Cursed      | 65%  | -5 .. +5 (yes, a cursed item can roll positive) |
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
/// is a weapon, a thing with an [`ArmorDie`] is armour, a [`Launcher`] is a bow —
/// so a future item that is two of those gets both pluses, and a ring (which is
/// none of them) gets only the tag.
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
    // A bow rolls no die of its own — what it improves is the arrow — so its
    // plus lands on the throw instead. A +3 bow is +3 on everything it looses.
    if entity.contains::<Launcher>() {
        let base = entity.get::<ThrowBonus>().map(|b| b.0).unwrap_or(0);
        entity.insert(ThrowBonus(base + bonus));
    }
    if quality == Quality::Cursed {
        entity.insert(Curse);
    }
}
