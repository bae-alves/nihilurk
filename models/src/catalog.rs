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

// --- Tuning constants -----------------------------------------------------
// The numbers a *drop* rolls that are not part of any one row: wand battery
// size, ammunition bundle size, and the enchantment odds / bonus ranges.
// All defined and documented in `constants.rs`.
use crate::constants::loot::{
    AMMO_BUNDLE_MAX, AMMO_BUNDLE_MIN, CURSED_BONUS_MAX, CURSED_BONUS_MIN, EXCEPTIONAL_BONUS_MAX,
    EXCEPTIONAL_BONUS_MIN, EXCEPTIONAL_QUALITY_PCT, NORMAL_QUALITY_PCT,
};
use crate::constants::wands::{CHARGE_BONUS, CHARGE_DICE, CHARGE_SIDES};

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

/// A potion: quaffed once, then gone. What it *does* lives in the `potions`
/// submodule of `crate::items`, keyed by [`PotionDef::effect`].
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
    PotionDef { effect: PotionEffect::FruitJuice,      name: "potion of fruit juice",      color: Color::DarkYellow },
    PotionDef { effect: PotionEffect::Water,           name: "potion of thirst quenching", color: Color::Blue },
];

// ---------------------------------------------------------------------------
// Scrolls
// ---------------------------------------------------------------------------

/// A scroll: read once, then it crumbles. Its mechanic lives in the `scrolls`
/// submodule of `crate::items`, keyed by [`ScrollDef::effect`].
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
    ScrollDef { effect: ScrollEffect::Amnesia,           name: "scroll of amnesia" },
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

/// A wand's battery, rolled when it enters the dungeon:
/// `CHARGE_DICE d CHARGE_SIDES + CHARGE_BONUS`, all three from
/// [`crate::constants::wands`].
pub fn roll_wand_charges(rng: &mut ChaCha12Rng) -> i8 {
    (0..CHARGE_DICE)
        .map(|_| rng.gen_range(1..=CHARGE_SIDES) as i8)
        .sum::<i8>()
        + CHARGE_BONUS
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
    /// How far this weapon strikes in melee: 0 for a plain weapon (a walk into
    /// the target's tile is the whole of it), 2 or more for a reach weapon
    /// aimed with its own reticle (see [`Reach`]).
    reach: i32,
    /// A reach weapon's strike runs the whole line rather than stopping at the
    /// first body (see [`ReachPiercing`]).
    reach_piercing: bool,
    /// The tricks this weapon lends its wielder while equipped — a battle
    /// axe's cleave, an estoc's double time — exactly the way a ring lends its
    /// wearer an effect. See [`crate::effects::Grants`].
    grants: &'static [Grant],
    /// A one-shot fired the instant it's wielded — the staff's "You're a
    /// wizard!". See [`OnWear`].
    on_wear: Option<OnWear>,
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
            reach: 0,
            reach_piercing: false,
            grants: &[],
            on_wear: None,
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

    /// A reach weapon, aimed rather than walked into: a bardiche (2), a whip
    /// (5).
    const fn reach(mut self, tiles: i32) -> Self {
        self.reach = tiles;
        self
    }

    /// The reach strike runs the whole line instead of stopping at the first
    /// body — a bardiche's polearm sweep, not a whip's single crack.
    const fn reach_piercing(mut self) -> Self {
        self.reach_piercing = true;
        self
    }

    /// Marker effects the wielder gains while it's in hand.
    const fn grants(mut self, grants: &'static [Grant]) -> Self {
        self.grants = grants;
        self
    }

    /// A one-shot fired the instant it goes on (see [`OnWear`]).
    const fn on_wear(mut self, on_wear: OnWear) -> Self {
        self.on_wear = Some(on_wear);
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
        if self.reach > 0 {
            e.insert(Reach(self.reach));
        }
        if self.reach_piercing {
            e.insert(ReachPiercing);
        }
        if !self.grants.is_empty() {
            e.insert(Grants(self.grants));
        }
        if let Some(on_wear) = self.on_wear {
            e.insert(on_wear);
        }
        e.id()
    }

    fn spawn_as_loot(&self, world: &mut World, rng: &mut ChaCha12Rng, pos: Position) -> Entity {
        let e = self.spawn(world, pos);
        enchant_equipment(world, rng, e);
        e
    }
}

/// The staff's flourish: logged the moment it's wielded, exactly the ceremony
/// a ring's [`OnWear`] gets, minus the ring — nothing about the wearer
/// changes, it's just the one line.
fn announce_wizard(world: &mut World, wearer: Entity, _item: Entity) {
    if world.get::<Player>(wearer).is_none() {
        return;
    }
    world
        .resource_mut::<GameLog>()
        .add("You're a wizard!".to_string());
}

#[rustfmt::skip]
pub const WEAPONS: &[WeaponDef] = &[
    WeaponDef::new("dagger",           Color::Grey,       4).missile(4).piercing(),
    WeaponDef::new("spear",            Color::DarkGrey,   6).missile(8).piercing(),
    WeaponDef::new("mace",             Color::DarkGrey,   6),
    WeaponDef::new("long sword",       Color::White,      8),
    WeaponDef::new("two-handed sword", Color::Cyan,       10),
    // The arc of a battle axe's swing catches everything standing next to you.
    WeaponDef::new("battle axe",       Color::DarkYellow, 7)
        .grants(&[Grant::of::<Cleaves>()]),
    // Colossal and slow: a hit staggers its victim outright, and the effort
    // costs the wielder a beat of their own.
    WeaponDef::new("greatclub",        Color::DarkRed,    12)
        .grants(&[Grant::of::<HeavySwing>()]),
    // A polearm: strikes two tiles out, and runs clean through anything in the
    // way.
    WeaponDef::new("bardiche",         Color::Grey,       7)
        .reach(2).reach_piercing(),
    // The longest reach in the dungeon, and the least behind it.
    WeaponDef::new("whip",             Color::DarkMagenta, 3)
        .reach(5),
    // Thin, fast steel: every attack goes out twice as quick, and closing the
    // last stride of a run lands a lunge.
    WeaponDef::new("estoc",            Color::White,      5)
        .grants(&[Grant::of::<Fencer>()]),
    // Whirled at the end of its chain in step with your feet: moving between
    // two tiles both next to the same enemy lands a free cut on it.
    WeaponDef::new("chain-sickle",     Color::DarkGreen,  5)
        .grants(&[Grant::of::<WhirlOnMove>()]),
    // No edge to speak of — it doesn't need one against something that can't
    // fight back.
    WeaponDef::new("garrote",          Color::DarkGrey,   0)
        .grants(&[Grant::of::<VorpalOnCondition>()]),
    // Doubles the toll and the fury of every spell cast through it.
    WeaponDef::new("staff",            Color::Yellow,     5)
        .grants(&[Grant::of::<TurboMagic>()])
        .on_wear(OnWear(announce_wizard)),
    // A blade with an edge in the world it half-belongs to: every hit that
    // lands there also lands a little on you.
    WeaponDef::new("chaos blade",      Color::Magenta,    12)
        .grants(&[Grant::of::<SelfDamageOnHit>()]),
    // Momentum, not mass: every hit that lands makes the next one hit harder,
    // for as long as you keep swinging it.
    WeaponDef::new("rapier",           Color::Red,        3)
        .grants(&[Grant::of::<BuildsMomentum>()]),
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
/// them looses it properly, on `launched_die` instead of `die`.
pub struct AmmoDef {
    pub name: &'static str,
    pub color: Color,
    /// The die one of these rolls, lobbed by hand.
    pub die: i32,
    /// The die one of these rolls loosed from its launcher instead. A quarrel's
    /// is the plain double a crossbow earns; an arrow's is short of that — the
    /// bow is the best thing in the dungeon drawn, and this keeps it from also
    /// being the hardest-hitting.
    pub launched_die: i32,
    /// The effect that turns a lob into a shot (see [`LaunchedBy`]).
    pub launched_by: Grant,
}

impl AmmoDef {
    /// How many a floor drop arrives in. Never a lone arrow — finding one arrow
    /// is not finding ammunition.
    fn roll_bundle(rng: &mut ChaCha12Rng) -> u8 {
        rng.gen_range(AMMO_BUNDLE_MIN..=AMMO_BUNDLE_MAX) as u8
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
                LaunchedDamage(self.launched_die),
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
    AmmoDef { name: "arrow",   color: Color::DarkYellow, die: 4, launched_die: 6,  launched_by: Grant::of::<FireArrow>()   },
    AmmoDef { name: "quarrel", color: Color::Grey,       die: 6, launched_die: 12, launched_by: Grant::of::<FireQuarrel>() },
];

/// The die and name a monster's shot rolls once loosed — `AMMO`'s own
/// `launched_die`, so a monster's shot and the player's agree on the same
/// dial. Monsters keep no quiver to check `LaunchedBy` against, so
/// [`crate::items::monster_ranged_attack`] picks the row by name instead.
pub fn ammo_launched_die(fires_quarrel: bool) -> (i32, &'static str) {
    let name = if fires_quarrel { "quarrel" } else { "arrow" };
    let def = AMMO
        .iter()
        .find(|def| def.name == name)
        .expect("arrow and quarrel are both rows in AMMO");
    (def.launched_die, def.name)
}

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
    LauncherDef { name: "short bow", color: Color::DarkYellow, grants: &[Grant::of::<FireArrow>()],   melee_cap: 1 },
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
    /// What happens the *moment* it goes on, for the one ring whose effect is
    /// an event rather than a property. Restored from the row on load, exactly
    /// like [`RingDef::grants`].
    pub on_wear: Option<OnWear>,
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
            on_wear: None,
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

    /// A one-shot fired the instant it goes on (see [`OnWear`]).
    const fn on_wear(mut self, on_wear: OnWear) -> Self {
        self.on_wear = Some(on_wear);
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
        if let Some(on_wear) = self.on_wear {
            e.insert(on_wear);
        }
        e.id()
    }

    fn spawn_as_loot(&self, world: &mut World, rng: &mut ChaCha12Rng, pos: Position) -> Entity {
        let e = self.spawn(world, pos);
        enchant_equipment(world, rng, e);
        e
    }
}

/// The rings, all twelve live. Eleven of them are pure table: a number that
/// combat already folds, or a marker some system already asks about. Only
/// adornment needed a verb written for it, and it is named here the same way
/// everything else is — see [`crate::items::rings`].
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
    // Rogue's useless ring, made the most valuable thing in the dungeon: worn
    // once, for one action, it doubles the run's score and is gone.
    RingDef::new(RingEffect::Adornment, "ring of adornment")
        .on_wear(crate::items::rings::ADORNMENT),
    // The ring of strength's plain twin: the same two points on the damage
    // roll, without the arm behind it that a dart can't drain.
    RingDef::new(RingEffect::IncreaseDamage, "ring of increase damage")
        .power_bonus(2),
    RingDef::new(RingEffect::Regeneration, "ring of regeneration")
        .grants(&[Grant::of::<Regenerates>()]),
    // The joke it has always been, taken literally: it slows your digestion by
    // slowing *you*.
    RingDef::new(RingEffect::SlowDigestion, "ring of slow digestion")
        .grants(&[Grant::of::<Sluggish>()]),
    RingDef::new(RingEffect::Teleportation, "ring of teleportation")
        .grants(&[Grant::of::<Teleportitis>()]),
    RingDef::new(RingEffect::Stealth, "ring of stealth")
        .grants(&[Grant::of::<Stealthy>()]),
    // The ring of strength's other twin: that one keeps the poison out of your
    // arm, this one keeps the corrosion off your plate.
    RingDef::new(RingEffect::MaintainArmor, "ring of maintain armor")
        .grants(&[Grant::of::<SustainsArmor>()]),
];

// ---------------------------------------------------------------------------
// Active moves
// ---------------------------------------------------------------------------

/// An active move: the identity and numbers behind a [`MoveEffect`], the way
/// [`WandDef`] is behind a [`WandEffect`]. Unlike every other row in this
/// file, a move is never spawned — it is not an [`ItemDef`], has no
/// [`Position`] on the floor and no pack slot. Its mechanic lives in
/// `crate::items::moves`, keyed by [`MoveDef::effect`].
pub struct MoveDef {
    pub effect: MoveEffect,
    pub name: &'static str,
    /// [`Magic`] points one use costs.
    pub cost: u8,
    /// How far the aiming reticle reaches.
    pub range: i32,
    /// Attack or skill — the dial a staff's [`TurboMagic`] checks before
    /// doubling both the cost and the fury of the cast. See
    /// [`crate::items::move_system`].
    pub kind: MoveKind,
}

impl MoveDef {
    /// The catalog row for `effect`. Panics on a move that isn't in the table
    /// — which would mean a [`MoveEffect`] variant nobody ever gave a row.
    pub fn of(effect: MoveEffect) -> &'static MoveDef {
        MOVES
            .iter()
            .find(|m| m.effect == effect)
            .unwrap_or_else(|| panic!("no move row for {effect:?}"))
    }
}

/// Every active move in the game: four tiers of four, priced by [`Magic`]
/// cost — 1 through 4 — the same way a floor's danger is priced by depth. A
/// monster's own copy of a shared trick ([`crate::items::dragon_breath`], the
/// dragon's innate attack) spends no [`Magic`] at all; the cost here is the
/// price of the player borrowing it, not a property of the trick itself.
///
/// `range` is meaningless for a move whose [`MoveEffect::needs_target`] is
/// `false` — it fires on its slot press with no reticle at all — and is left
/// at `0` for those rows.
#[rustfmt::skip]
pub const MOVES: &[MoveDef] = &[
    // --- 1 Ma ---------------------------------------------------------
    MoveDef { effect: MoveEffect::Sting,       name: "Sting",       cost: 1, range: 8, kind: MoveKind::Attack },
    MoveDef { effect: MoveEffect::Thunderbolt, name: "Thunderbolt", cost: 1, range: 8, kind: MoveKind::Attack },
    MoveDef { effect: MoveEffect::Cure,        name: "Cure",        cost: 1, range: 0, kind: MoveKind::Skill },
    MoveDef { effect: MoveEffect::Bide,        name: "Bide",        cost: 1, range: 0, kind: MoveKind::Skill },
    // --- 2 Ma ---------------------------------------------------------
    MoveDef { effect: MoveEffect::DragonBreath, name: "Fireball",   cost: 2, range: 8, kind: MoveKind::Attack },
    MoveDef { effect: MoveEffect::ForceLance,  name: "Force Lance", cost: 2, range: 8, kind: MoveKind::Attack },
    MoveDef { effect: MoveEffect::Identify,    name: "Identify",    cost: 2, range: 0, kind: MoveKind::Skill },
    MoveDef { effect: MoveEffect::Setup,       name: "Setup",       cost: 2, range: 0, kind: MoveKind::Skill },
    // --- 3 Ma ---------------------------------------------------------
    MoveDef { effect: MoveEffect::Lux,          name: "Lux",           cost: 3, range: 8, kind: MoveKind::Attack },
    MoveDef { effect: MoveEffect::CircleOfDeath, name: "Circle of Death", cost: 3, range: 0, kind: MoveKind::Attack },
    MoveDef { effect: MoveEffect::MagicWard,    name: "Magic Ward",    cost: 3, range: 0, kind: MoveKind::Skill },
    MoveDef { effect: MoveEffect::Heal,         name: "Heal",          cost: 3, range: 0, kind: MoveKind::Skill },
    // --- 4 Ma ---------------------------------------------------------
    MoveDef { effect: MoveEffect::MeteorStrike, name: "Meteor Strike", cost: 4, range: 8, kind: MoveKind::Attack },
    MoveDef { effect: MoveEffect::FrostNova,    name: "Frost Nova",    cost: 4, range: 0, kind: MoveKind::Attack },
    MoveDef { effect: MoveEffect::MagicMapping, name: "Magic Mapping", cost: 4, range: 0, kind: MoveKind::Skill },
    MoveDef { effect: MoveEffect::HasteSelf,    name: "Haste Self",    cost: 4, range: 0, kind: MoveKind::Skill },
];

// ---------------------------------------------------------------------------
// Coins and the relic
// ---------------------------------------------------------------------------

/// A coin: Rogue's food slot, grown into the whole pickup category. Every one
/// of them works the instant you step on it and is never carried — the two
/// treasure coins go straight into the score, the rest into you.
///
/// `amount` is the row's one dial and what it means is `effect`'s business:
/// points for a treasure coin, hit points for the red one, afflictions lifted
/// for the rosé. A row that needs no number leaves it at zero.
pub struct CoinDef {
    pub name: &'static str,
    pub color: Color,
    pub effect: PickupEffect,
    pub amount: i32,
    /// This row's share of the coin table against its table-mates. Ten is the
    /// baseline; heroic mana sits well under it — an uncommon find, not a
    /// coin.
    pub weight: u32,
}

impl ItemDef for CoinDef {
    fn name(&self) -> &'static str {
        self.name
    }

    fn weight(&self) -> u32 {
        self.weight
    }

    fn spawn(&self, world: &mut World, pos: Position) -> Entity {
        let mut e = world.spawn((
            Name {
                what: self.name.to_string(),
            },
            Renderable {
                glyph: '$',
                color: self.color,
            },
            pos,
            Item,
            Pickup {
                effect: self.effect,
                amount: self.amount,
            },
        ));
        // A treasure coin also carries the one component the score reads, the
        // same one the relic carries. Everything else on this table is worth
        // what it does to you, which the scoreboard never hears about.
        if self.effect == PickupEffect::Coin {
            e.insert(Value {
                amount: self.amount,
            });
        }
        e.id()
    }
}

/// The coins. Two are treasure and six are a small mercy, and the dungeon
/// scatters them alike — which is why a `$` on the floor is worth walking to
/// even when your pack is full.
#[rustfmt::skip]
pub const COINS: &[CoinDef] = &[
    CoinDef { name: "gold coin",     color: Color::Yellow,      effect: PickupEffect::Coin,     amount: 5000, weight: 10 },
    CoinDef { name: "silver coin",   color: Color::Grey,        effect: PickupEffect::Coin,     amount: 1000, weight: 10 },
    CoinDef { name: "red coin",      color: Color::Red,         effect: PickupEffect::Health,   amount:    4, weight: 10 },
    CoinDef { name: "blue coin",     color: Color::Blue,        effect: PickupEffect::Power,    amount:    4, weight: 10 },
    CoinDef { name: "rosé coin",     color: Color::Magenta,     effect: PickupEffect::Cleanse,  amount:    4, weight: 10 },
    CoinDef { name: "green coin",    color: Color::Green,       effect: PickupEffect::Strength, amount:    4, weight: 10 },
    CoinDef { name: "platinum coin", color: Color::White,      effect: PickupEffect::Platinum, amount:    0, weight: 10 },
    CoinDef { name: "forge coin",    color: Color::DarkYellow,  effect: PickupEffect::Forge,    amount:    0, weight: 10 },
    // Uncommon, and never disguised: no colour name, no adjective — it is
    // always just "heroic mana".
    CoinDef { name: "heroic mana",   color: Color::Magenta,     effect: PickupEffect::Mana,     amount:    0, weight:  3 },
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
        if def.reach > 0 {
            entity.insert(Reach(def.reach));
        }
        if def.reach_piercing {
            entity.insert(ReachPiercing);
        }
        if !def.grants.is_empty() {
            entity.insert(Grants(def.grants));
        }
        if let Some(on_wear) = def.on_wear {
            entity.insert(on_wear);
        }
    }
    if let Some(def) = AMMO.iter().find(|d| d.name == name) {
        entity.insert((
            ThrownDamage(def.die),
            LaunchedDamage(def.launched_die),
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
        let roll = rng.gen_range(0..100);
        if roll < NORMAL_QUALITY_PCT {
            return Quality::Normal;
        }
        if roll < NORMAL_QUALITY_PCT + EXCEPTIONAL_QUALITY_PCT {
            return Quality::Exceptional;
        }
        Quality::Cursed
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
        Quality::Exceptional => rng.gen_range(EXCEPTIONAL_BONUS_MIN..=EXCEPTIONAL_BONUS_MAX),
        Quality::Cursed => rng.gen_range(CURSED_BONUS_MIN..=CURSED_BONUS_MAX),
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
