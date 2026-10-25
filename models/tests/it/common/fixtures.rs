//! Fixture rows for the tests: every item and trap row they spawn, copied from
//! the shipped tables and wired to the shipped handlers. A test finds a row
//! here by name, so renaming, reweighting or recolouring a shipped row changes
//! no test; a change to a handler still does, which is what the tests are for.
//!
//! The lookups carry the names of the library's own (`spawn_named`,
//! `spawn_weapon`, `scroll_row`, ...) and a test imports them in their place.
//! A name no fixture has falls through to the library, which is how monsters
//! come in.

use bevy_ecs::prelude::*;
use models::constants::loot::IMPROVISED_THROW_DIE;
use models::constants::monsters::DEFAULT_SPAWN_WEIGHT;
use models::constants::rings::PLAIN_BONUS;
use models::constants::wands::BLAST_RADIUS;
use models::*;

/// The potions the tests lean on, copied from the shipped table so no
/// test follows a row's name or tuning.
#[rustfmt::skip]
pub static FIXTURE_POTIONS: &[&PotionDef] = &[
    &PotionDef::new("potion of confusion",         Color::Magenta,     potions::confusion),
    &PotionDef::new("potion of paralysis",         Color::DarkGrey,    models::conditions::paralyse),
    &PotionDef::new("potion of poison",            Color::Green,       potions::poison),
    &PotionDef::new("potion of gain strength",     Color::Red,         potions::gain_strength),
    &PotionDef::new("potion of see invisible",     Color::Cyan,        potions::see_invisible),
    &PotionDef::new("potion of healing",           Color::Red,         potions::healing),
    &PotionDef::new("potion of monster detection", Color::Yellow,      potions::detect_monsters),
    &PotionDef::new("potion of magic detection",   Color::Yellow,      potions::detect_magic),
    &PotionDef::new("potion of raise level",       Color::White,       potions::raise_level),
    &PotionDef::new("potion of extra healing",     Color::Red,         potions::extra_healing),
    &PotionDef::new("potion of haste self",        Color::DarkYellow,  models::conditions::hasten),
    &PotionDef::new("potion of restore strength",  Color::Red,         potions::restore_strength),
    &PotionDef::new("potion of fruit juice",       Color::DarkYellow,  potions::fruit_juice),
    &PotionDef::new("potion of magic",             Color::Blue,        potions::gain_magic),
    &PotionDef::new("potion of adjustment",        Color::DarkCyan,    potions::adjustment),
    &PotionDef::new("potion of polymorph",         Color::DarkMagenta, potions::polymorph),
    &PotionDef::new("potion of thirst quenching",  Color::Blue,        potions::water).never_drops(),
];

/// The scrolls the tests lean on, copied from the shipped table so no
/// test follows a row's name or tuning.
#[rustfmt::skip]
pub static FIXTURE_SCROLLS: &[&ScrollDef] = &[
    &ScrollDef::new("scroll of monster confusion",  scrolls::charm_hands),
    &ScrollDef::new("scroll of magic mapping",      scrolls::map_the_floor),
    &ScrollDef::new("scroll of hold monster",       scrolls::hold_in_view),
    &ScrollDef::new("scroll of sleep",              scrolls::read_sleep),
    &ScrollDef::new("scroll of enchant armor",      scrolls::enchant_armor),
    &ScrollDef::new("scroll of identify",           scrolls::identify_everything_hidden).common(),
    &ScrollDef::new("scroll of scare monster",      scrolls::scare_monster),
    &ScrollDef::new("scroll of teleportation",      scrolls::teleport_reader),
    &ScrollDef::new("scroll of enchant weapon",     scrolls::enchant_weapon),
    &ScrollDef::new("scroll of create monster",     scrolls::create_monster),
    &ScrollDef::new("scroll of remove curse",       scrolls::remove_curse).common(),
    &ScrollDef::new("scroll of aggravate monsters", scrolls::aggravate_floor),
    &ScrollDef::new("scroll of vorpalize weapon",   scrolls::vorpalize_wielded_weapon).common(),
    &ScrollDef::new("scroll of amnesia",            scrolls::read_amnesia),
    &ScrollDef::new("scroll of charming",           scrolls::charming),
    &ScrollDef::new("scroll of atonement",          scrolls::atonement).rare(),
    &ScrollDef::new("scroll of recharging",         scrolls::recharging).rare(),
    &ScrollDef::new("scroll of protection",         scrolls::protection).rare(),
    &ScrollDef::new("scroll of justice",            scrolls::justice).rare(),
    &ScrollDef::new("scroll of teleport to stairs", scrolls::teleport_to_stairs).never_drops(),
    &ScrollDef::new("scroll of blank paper",        scrolls::blank_paper).never_drops(),
];

/// The wands the tests lean on, copied from the shipped table so no
/// test follows a row's name or tuning.
#[rustfmt::skip]
pub static FIXTURE_WANDS: &[&WandDef] = &[
    &WandDef::new("wand of light",         Color::Yellow,      8, BlastPalette::Glam,   wands::light).unaimed().thrown(Thrown::FLASH),
    &WandDef::new("wand of striking",      Color::White,       6, BlastPalette::Force,  wands::striking).attack(),
    &WandDef::new("wand of lightning",     Color::Cyan,        8, BlastPalette::Spark,  wands::lightning).attack(),
    &WandDef::new("wand of fire",          Color::Red,         8, BlastPalette::Fire,   wands::fire).attack().element(Element::Fire).bursts().wasted_on(wands::fireproof),
    &WandDef::new("wand of cold",          Color::Blue,        8, BlastPalette::Frost,  wands::cold).attack().element(Element::Cold).bursts().wasted_on(wands::coldproof),
    &WandDef::new("wand of polymorph",     Color::Magenta,     6, BlastPalette::Warp,   wands::polymorph).thrown(Thrown::blast(wands::polymorph_entity)),
    &WandDef::new("wand of magic missile", Color::Cyan,        6, BlastPalette::Arcane, wands::magic_missile).attack(),
    &WandDef::new("wand of haste monster", Color::DarkYellow,  6, BlastPalette::Warp,   wands::haste_monster).thrown(Thrown::blast(wands::haste_entity)),
    &WandDef::new("wand of slow monster",  Color::DarkCyan,    6, BlastPalette::Warp,   wands::slow_monster).wasted_on(wands::already_slow).thrown(Thrown::blast(wands::slow_entity)),
    &WandDef::new("wand of drain life",    Color::DarkRed,     6, BlastPalette::Death,  wands::drain_life).attack().element(Element::Drain),
    &WandDef::new("wand of teleport away", Color::Green,       8, BlastPalette::Warp,   wands::teleport_away).thrown(Thrown::blast(wands::teleport_entity_away)),
    &WandDef::new("wand of teleport to",   Color::Green,       8, BlastPalette::Warp,   wands::teleport_to).thrown(Thrown::blast(wands::teleport_entity_to_self)),
    &WandDef::new("wand of cancellation",  Color::DarkMagenta, 6, BlastPalette::Void,   wands::cancellation).thrown(Thrown::blast(wands::cancel_entity)),
    &WandDef::new("wand of charming",      Color::Magenta,     6, BlastPalette::Warp,   wands::charming).wasted_on(wands::already_an_ally),
    &WandDef::new("wand of digging",       Color::DarkYellow,  8, BlastPalette::Warp,   wands::digging).thrown(Thrown::CRATER),
    &WandDef::new("wand of swapping",      Color::DarkGreen,   8, BlastPalette::Warp,   wands::swapping).thrown(Thrown::SHUFFLE),
    &WandDef::new("wand of nothing",       Color::Grey,        6, BlastPalette::Warp,   wands::nothing).wasted_on(|_, _| true).thrown(Thrown::CONFETTI).never_drops(),
];

/// The weapons the tests lean on, copied from the shipped table so no
/// test follows a row's name or tuning.
#[rustfmt::skip]
pub const FIXTURE_WEAPONS: &[WeaponDef] = &[
    WeaponDef::new("dagger",           Color::Grey,       4).missile(8).piercing(),
    WeaponDef::new("boomerang",        Color::DarkYellow, 4).returning(),
    WeaponDef::new("moon blade",       Color::Cyan,       3).returning().hits(3),
    WeaponDef::new("mace",             Color::DarkGrey,   6),
    WeaponDef::new("long sword",       Color::White,      8),
    WeaponDef::new("battle axe",       Color::DarkYellow, 7).grants(&[Grant::of::<Cleaves>()]),
    WeaponDef::new("estoc",            Color::White,      5).grants(&[Grant::of::<Lunges>()]),
    WeaponDef::new("chain-sickle",     Color::DarkGreen,  5).grants(&[Grant::of::<WhirlOnMove>()]),
    WeaponDef::new("wizard staff",     Color::Yellow,     5).grants(&[Grant::of::<TurboMagic>()]).on_wear(OnWear(models::catalog::announce_wizard)).on_doff(OnDoff(models::catalog::announce_not_wizard)),
    WeaponDef::new("war hammer",       Color::DarkGrey,   9).grants(&[Grant::of::<ShattersStone>()]),
];

/// The ammo the tests lean on, copied from the shipped table so no
/// test follows a row's name or tuning.
#[rustfmt::skip]
pub static FIXTURE_AMMO: &[&AmmoDef] = &[
    &AmmoDef { name: "arrow",    color: Color::DarkYellow, die: IMPROVISED_THROW_DIE, launched_die: 4, launched_by: Grant::of::<FireArrow>(),   on_blood: |_, _| {} },
    &AmmoDef { name: "quarrel",  color: Color::Grey,       die: IMPROVISED_THROW_DIE, launched_die: 5, launched_by: Grant::of::<FireQuarrel>(), on_blood: |_, _| {} },
    &AmmoDef { name: "blowdart", color: Color::Green,      die: IMPROVISED_THROW_DIE, launched_die: 3, launched_by: Grant::of::<FireDart>(),    on_blood: throwing::sap },
];

/// The launchers the tests lean on, copied from the shipped table so no
/// test follows a row's name or tuning.
#[rustfmt::skip]
pub const FIXTURE_LAUNCHERS: &[LauncherDef] = &[
    LauncherDef { name: "short bow", color: Color::DarkYellow, grants: &[Grant::of::<FireArrow>()],   melee_cap: 1 },
    LauncherDef { name: "crossbow", color: Color::DarkGrey,   grants: &[Grant::of::<FireQuarrel>()], melee_cap: 1 },
    LauncherDef { name: "blowgun",  color: Color::DarkGreen, grants: &[Grant::of::<FireDart>()],    melee_cap: 1 },
];

/// The armors the tests lean on, copied from the shipped table so no
/// test follows a row's name or tuning.
#[rustfmt::skip]
pub const FIXTURE_ARMORS: &[ArmorDef] = &[
    ArmorDef { name: "leather armor",          color: Color::DarkYellow, armor_die: 2, grants: &[] },
    ArmorDef { name: "ring mail",              color: Color::Grey,       armor_die: 3, grants: &[] },
    ArmorDef { name: "plate mail",             color: Color::Cyan,       armor_die: 6, grants: &[] },
    ArmorDef { name: "spikemail",              color: Color::DarkGrey,   armor_die: 4, grants: &[Grant::of::<Spiked>()] },
];

/// The rings the tests lean on, copied from the shipped table so no
/// test follows a row's name or tuning.
#[rustfmt::skip]
pub static FIXTURE_RINGS: &[&RingDef] = &[
    &RingDef::new("ring of protection")
        .armor_bonus(PLAIN_BONUS),
    &RingDef::new("ring of strength")
        .power_bonus(PLAIN_BONUS)
        .grants(&[Grant::of::<SustainsStrength>()]),
    &RingDef::new("ring of perception")
        .grants(&[Grant::of::<SeesInvisible>()]),
    &RingDef::new("ring of sharpshooting")
        .throw_bonus(PLAIN_BONUS),
    &RingDef::new("ring of adornment")
        .on_wear(models::rings::ADORNMENT),
    &RingDef::new("ring of stealth")
        .grants(&[Grant::of::<Stealthy>()]),
    &RingDef::new("ring of maintain armor")
        .grants(&[Grant::of::<SustainsArmor>()]),
    &RingDef::new("ring of dualzap")
        .grants(&[Grant::of::<DualZap>()]),
    &RingDef::new("ring of health")
        .max_hp_bonus(PLAIN_BONUS),
];

/// The coins the tests lean on, copied from the shipped table so no
/// test follows a row's name or tuning.
#[rustfmt::skip]
pub static FIXTURE_COINS: &[&CoinDef] = &[
    &CoinDef { name: "red coin",      color: Color::Red,         apply: pickups::heal,             helps: pickups::hurt,             amount:    4, treasure: false, weight: 10 },
    &CoinDef { name: "blue coin",     color: Color::Blue,        apply: pickups::refill_magic,     helps: pickups::drained,          amount:    4, treasure: false, weight: 10 },
    &CoinDef { name: "rosé coin",     color: Color::Magenta,     apply: pickups::cleanse,          helps: models::conditions::afflicted, amount: 4, treasure: false, weight: 10 },
    &CoinDef { name: "green coin",    color: Color::Green,       apply: pickups::restore_strength, helps: pickups::weakened,         amount:    4, treasure: false, weight: 10 },
    &CoinDef { name: "platine coin",  color: Color::White,       apply: pickups::platinum,         helps: pickups::owed_platinum,    amount:    0, treasure: false, weight: 10 },
    &CoinDef { name: "forge coin",    color: Color::DarkYellow,  apply: pickups::forge,            helps: pickups::owed_forge,       amount:    0, treasure: false, weight: 10 },
    &CoinDef { name: "hero coin",     color: Color::Magenta,     apply: pickups::learn_spell,      helps: pickups::can_learn_spell,  amount:    0, treasure: false, weight: 3 },
];

/// The treats the tests lean on, copied from the shipped table so no
/// test follows a row's name or tuning.
#[rustfmt::skip]
pub static FIXTURE_TREATS: &[&TreatDef] = &[
    &TreatDef { name: "snack",          color: Color::DarkYellow, takes: models::companion::has_no_hands },
    &TreatDef { name: "fancy of peace", color: Color::Cyan,       takes: models::companion::has_hands },
];

/// The traps the tests lean on, copied from the shipped table so no
/// test follows a row's name or tuning.
#[rustfmt::skip]
pub static FIXTURE_TRAPS: &[&TrapDef] = &[
    &TrapDef { name: "trapdoor",          glyph: '^', color: Color::Green,       weight: DEFAULT_SPAWN_WEIGHT, min_depth: 1, snare_turns: 0, always_spent: false, apply: models::traps::trapdoor, flourish: models::traps::no_flourish },
    &TrapDef { name: "bear trap",         glyph: '^', color: Color::DarkGreen,   weight: DEFAULT_SPAWN_WEIGHT, min_depth: 1, snare_turns: 3, always_spent: true,  apply: models::traps::bear,     flourish: models::traps::jaws },
    &TrapDef { name: "sleeping gas trap", glyph: '^', color: Color::Blue,        weight: DEFAULT_SPAWN_WEIGHT, min_depth: 1, snare_turns: 5, always_spent: false, apply: models::traps::sleep,    flourish: models::traps::gas },
    &TrapDef { name: "teleport trap",     glyph: '^', color: Color::DarkMagenta, weight: DEFAULT_SPAWN_WEIGHT, min_depth: 1, snare_turns: 0, always_spent: false, apply: models::traps::teleport, flourish: models::traps::no_flourish },
    &TrapDef { name: "arrow trap",        glyph: '^', color: Color::DarkCyan,    weight: DEFAULT_SPAWN_WEIGHT, min_depth: 1, snare_turns: 0, always_spent: false, apply: models::traps::arrow,    flourish: models::traps::missile },
    &TrapDef { name: "dart trap",         glyph: '^', color: Color::Cyan,        weight: DEFAULT_SPAWN_WEIGHT, min_depth: 1, snare_turns: 0, always_spent: false, apply: models::traps::dart,     flourish: models::traps::missile },
];

/// The spells the tests lean on, copied from the shipped table.
#[rustfmt::skip]
pub static FIXTURE_SPELLS: &[&SpellDef] = &[
    &SpellDef::new("Sting", 1, spells::sting).attack().aimed(8),
    &SpellDef::new("Thunderbolt", 1, spells::thunderbolt).attack().aimed(8),
    &SpellDef::new("Cure", 1, spells::cure_self),
    &SpellDef::new("Bide", 1, spells::bide),
    &SpellDef::new("Fireball", 2, spells::breathe_fire).attack().aimed(8).bursts(BLAST_RADIUS).wasted_on(wands::fireproof),
    &SpellDef::new("Force Lance", 2, spells::force_lance).attack().aimed(8).line(),
    &SpellDef::new("Polymorph Self", 2, spells::polymorph_self),
    &SpellDef::new("Heal", 3, spells::heal_self),
    &SpellDef::new("Polymorph Other", 3, spells::polymorph_other).aimed(8),
    &SpellDef::new("Haste Self", 4, spells::haste_self),
];

fn find<'a, T>(rows: &'a [T], name: &str, of: impl Fn(&T) -> &'static str) -> Option<&'a T> {
    rows.iter().find(|row| of(row) == name)
}

/// The fixture potion called `name`.
pub fn potion_row(name: &str) -> Option<&'static PotionDef> {
    find(FIXTURE_POTIONS, name, |r| r.name).copied()
}

/// The fixture scroll called `name`.
pub fn scroll_row(name: &str) -> Option<&'static ScrollDef> {
    find(FIXTURE_SCROLLS, name, |r| r.name).copied()
}

/// The fixture wand called `name`.
pub fn wand_row(name: &str) -> Option<&'static WandDef> {
    find(FIXTURE_WANDS, name, |r| r.name).copied()
}

/// The fixture ring called `name`.
pub fn ring_row(name: &str) -> Option<&'static RingDef> {
    find(FIXTURE_RINGS, name, |r| r.name).copied()
}

/// The fixture coin called `name`.
pub fn coin_row(name: &str) -> Option<&'static CoinDef> {
    find(FIXTURE_COINS, name, |r| r.name).copied()
}

/// The fixture spell called `name`.
pub fn spell_row(name: &str) -> Option<&'static SpellDef> {
    find(FIXTURE_SPELLS, name, |r| r.name).copied()
}

/// The fixture trap called `name`.
pub fn trap_row(name: &str) -> Option<&'static TrapDef> {
    find(FIXTURE_TRAPS, name, |r| r.name).copied()
}

/// The fixture weapon called `name`, on the floor at `pos`.
pub fn spawn_weapon(world: &mut World, name: &str, pos: Position) -> Entity {
    find(FIXTURE_WEAPONS, name, |r| r.name)
        .unwrap_or_else(|| panic!("no fixture weapon {name:?}"))
        .spawn(world, pos)
}

/// The fixture armour called `name`, on the floor at `pos`.
pub fn spawn_armor(world: &mut World, name: &str, pos: Position) -> Entity {
    find(FIXTURE_ARMORS, name, |r| r.name)
        .unwrap_or_else(|| panic!("no fixture armour {name:?}"))
        .spawn(world, pos)
}

/// The fixture launcher called `name`, on the floor at `pos`.
pub fn spawn_launcher(world: &mut World, name: &str, pos: Position) -> Entity {
    find(FIXTURE_LAUNCHERS, name, |r| r.name)
        .unwrap_or_else(|| panic!("no fixture launcher {name:?}"))
        .spawn(world, pos)
}

/// One fixture arrow or quarrel on the floor at `pos`.
pub fn spawn_ammo(world: &mut World, name: &str, pos: Position) -> Entity {
    find(FIXTURE_AMMO, name, |r| r.name)
        .unwrap_or_else(|| panic!("no fixture ammo {name:?}"))
        .spawn(world, pos)
}

/// The fixture ring called `name`, on the floor at `pos`.
pub fn spawn_ring(world: &mut World, name: &str, pos: Position) -> Entity {
    ring_row(name)
        .unwrap_or_else(|| panic!("no fixture ring {name:?}"))
        .spawn(world, pos)
}

/// Whatever fixture row is called `name`, at `pos`; a name no fixture has is the
/// library's to answer (monsters, the relic).
pub fn spawn_named(world: &mut World, name: &str, pos: Position) -> Option<Entity> {
    if let Some(row) = potion_row(name) {
        return Some(row.spawn(world, pos));
    }
    if let Some(row) = scroll_row(name) {
        return Some(row.spawn(world, pos));
    }
    if let Some(row) = wand_row(name) {
        return Some(row.spawn(world, pos));
    }
    if let Some(row) = ring_row(name) {
        return Some(row.spawn(world, pos));
    }
    if let Some(row) = coin_row(name) {
        return Some(row.spawn(world, pos));
    }
    if let Some(row) = find(FIXTURE_TREATS, name, |r| r.name) {
        return Some(row.spawn(world, pos));
    }
    if let Some(row) = find(FIXTURE_WEAPONS, name, |r| r.name) {
        return Some(row.spawn(world, pos));
    }
    if let Some(row) = find(FIXTURE_ARMORS, name, |r| r.name) {
        return Some(row.spawn(world, pos));
    }
    if let Some(row) = find(FIXTURE_AMMO, name, |r| r.name) {
        return Some(row.spawn(world, pos));
    }
    if let Some(row) = find(FIXTURE_LAUNCHERS, name, |r| r.name) {
        return Some(row.spawn(world, pos));
    }
    if let Some(row) = trap_row(name) {
        return Some(
            world
                .spawn(TrapBundle::from_def(row, TrapReveal::Sight, pos))
                .id(),
        );
    }
    models::spawn_named(world, name, pos)
}
