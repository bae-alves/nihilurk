//! Fixture rows for the engine's tests, copied from the shipped tables: a test
//! that spawns one of these follows the handler, not whatever the row is called
//! or how the table weights it. A name no fixture has falls through to the
//! library (monsters, mostly).

use bevy_ecs::prelude::*;
use models::constants::wands::BLAST_RADIUS;
use models::*;

#[rustfmt::skip]
static WANDS: &[&WandDef] = &[
    &WandDef::new("wand of fire",          Color::Red,         8, BlastPalette::Fire,   wands::fire).attack().element(Element::Fire).bursts().wasted_on(wands::fireproof),
    &WandDef::new("wand of cold",          Color::Blue,        8, BlastPalette::Frost,  wands::cold).attack().element(Element::Cold).bursts().wasted_on(wands::coldproof),
    &WandDef::new("wand of slow monster",  Color::DarkCyan,    6, BlastPalette::Warp,   wands::slow_monster).wasted_on(wands::already_slow).thrown(Thrown::blast(wands::slow_entity)),
];

#[rustfmt::skip]
const WEAPONS: &[WeaponDef] = &[
    WeaponDef::new("dagger",           Color::Grey,       4).missile(8).piercing(),
    WeaponDef::new("boomerang",        Color::DarkYellow, 4).returning(),
    WeaponDef::new("war hammer",       Color::DarkGrey,   9).grants(&[Grant::of::<ShattersStone>()]),
];

#[rustfmt::skip]
static TREATS: &[&TreatDef] = &[
    &TreatDef { name: "snack",          color: Color::DarkYellow, takes: models::companion::has_no_hands },
];

#[rustfmt::skip]
static SPELLS: &[&SpellDef] = &[
    &SpellDef::new("Sting", 1, spells::sting).attack().aimed(8),
    &SpellDef::new("Thunderbolt", 1, spells::thunderbolt).attack().aimed(8),
    &SpellDef::new("Fireball", 2, spells::breathe_fire).attack().aimed(8).bursts(BLAST_RADIUS).wasted_on(wands::fireproof),
    &SpellDef::new("Force Lance", 2, spells::force_lance).attack().aimed(8).line(),
];

fn find<'a, T>(rows: &'a [T], name: &str, of: impl Fn(&T) -> &'static str) -> Option<&'a T> {
    rows.iter().find(|row| of(row) == name)
}

/// The fixture spell called `name`.
pub fn spell_row(name: &str) -> Option<&'static SpellDef> {
    find(SPELLS, name, |r| r.name).copied()
}

/// The fixture weapon called `name`, on the floor at `pos`.
pub fn spawn_weapon(world: &mut World, name: &str, pos: Position) -> Entity {
    find(WEAPONS, name, |r| r.name)
        .unwrap_or_else(|| panic!("no fixture weapon {name:?}"))
        .spawn(world, pos)
}

/// Whatever fixture row is called `name`, at `pos`; a name no fixture has is the
/// library's to answer.
pub fn spawn_named(world: &mut World, name: &str, pos: Position) -> Option<Entity> {
    if let Some(row) = find(WANDS, name, |r| r.name) {
        return Some(row.spawn(world, pos));
    }
    if let Some(row) = find(TREATS, name, |r| r.name) {
        return Some(row.spawn(world, pos));
    }
    if let Some(row) = find(WEAPONS, name, |r| r.name) {
        return Some(row.spawn(world, pos));
    }
    models::spawn_named(world, name, pos)
}
