use bevy_ecs::prelude::Bundle;
use crossterm::style::Color;
use crate::components::*;

#[derive(Bundle)]
pub struct MonsterBundle {
    pub name: Name,
    pub mob: Mob,
    pub fighter: Fighter,
    pub glyph: Renderable,
    pub position: Position,
    pub faction: Faction,
}

impl MonsterBundle {
    /// Shared constructor for the bestiary below.
    ///
    /// Micro-HP design: mobs live on a handful of HP and survive by winning the
    /// opposed armour roll, not by having a fat health pool. `power`/`armor` are
    /// die sizes (`1d[power]`, `1d[armor]`); `*_bonus` are flat modifiers added
    /// once to each roll (see [`crate::combat::resolve_attack`]).
    #[allow(clippy::too_many_arguments)]
    fn new(
        name: &str,
        glyph: char,
        color: Color,
        movement_type: MovementType,
        hp: i32,
        power: i32,
        power_bonus: i32,
        armor: i32,
        armor_bonus: i32,
        position: Position,
    ) -> Self {
        Self {
            name: Name { what: name.to_string() },
            mob: Mob { movement_type },
            fighter: Fighter { hp, max_hp: hp, power, power_bonus, armor, armor_bonus },
            glyph: Renderable { glyph, color },
            position,
            faction: Faction::Monster,
        }
    }

    /// Not part of the lettered bestiary, but kept as the classic early cannon
    /// fodder: attacks roll 1d4, defence rolls 1d6.
    pub fn goblin(position: Position) -> Self {
        Self::new("goblin", 'g', Color::Green, MovementType::Flee, 1, 4, 0, 6, 0, position)
    }

    // --- The lettered bestiary --------------------------------------------
    // Each row lists the Rogue-style Lvl/AC for reference; only Micro-HP and the
    // two opposed rolls (damage = power, armour) are modelled.

    /// A — Aquator · Lvl 5 / AC 2 · dmg 1d4-1 · armour 1d8+1
    pub fn aquator(position: Position) -> Self {
        Self::new("aquator", 'A', Color::Blue, MovementType::Chase, 3, 4, -1, 8, 1, position)
    }

    /// B — Bat · Lvl 1 / AC 3 · dmg 1d4 · armour 1d8
    pub fn bat(position: Position) -> Self {
        Self::new("bat", 'B', Color::DarkGrey, MovementType::Confused, 1, 4, 0, 8, 0, position)
    }

    /// C — Centaur · Lvl 4 / AC 4 · dmg 1d8 · armour 1d6+1
    pub fn centaur(position: Position) -> Self {
        Self::new("centaur", 'C', Color::DarkYellow, MovementType::Chase, 3, 8, 0, 6, 1, position)
    }

    /// D — Dragon · Lvl 10 / AC -1 · dmg 1d12+2 · armour 1d10+2
    pub fn dragon(position: Position) -> Self {
        Self::new("dragon", 'D', Color::Red, MovementType::Chase, 8, 12, 2, 10, 2, position)
    }

    /// E — Emu · Lvl 1 / AC 7 · dmg 1d4 · armour 1d4+1
    pub fn emu(position: Position) -> Self {
        Self::new("emu", 'E', Color::DarkGreen, MovementType::Chase, 1, 4, 0, 4, 1, position)
    }

    /// F — Venus Flytrap · Lvl 8 / AC 3 · dmg 1d10 · armour 1d8 · rooted in place
    pub fn venus_flytrap(position: Position) -> Self {
        Self::new("venus flytrap", 'F', Color::Green, MovementType::Static, 6, 10, 0, 8, 0, position)
    }

    /// G — Griffin · Lvl 13 / AC 2 · dmg 1d12+1 · armour 1d8+1
    pub fn griffin(position: Position) -> Self {
        Self::new("griffin", 'G', Color::DarkYellow, MovementType::Chase, 10, 12, 1, 8, 1, position)
    }

    /// H — Hobgoblin · Lvl 1 / AC 5 · dmg 1d8 · armour 1d6
    pub fn hobgoblin(position: Position) -> Self {
        Self::new("hobgoblin", 'H', Color::DarkRed, MovementType::Chase, 1, 8, 0, 6, 0, position)
    }

    /// I — Ice Monster · Lvl 1 / AC 9 · dmg 1d4 · armour 1d4-1 · lies in wait
    pub fn ice_monster(position: Position) -> Self {
        Self::new("ice monster", 'I', Color::Cyan, MovementType::Static, 1, 4, 0, 4, -1, position)
    }

    /// J — Jabberwock · Lvl 15 / AC 6 · dmg 2d8 (≈1d8+5) · armour 1d6
    pub fn jabberwock(position: Position) -> Self {
        Self::new("jabberwock", 'J', Color::Magenta, MovementType::Chase, 12, 8, 5, 6, 0, position)
    }

    /// K — Kestral · Lvl 1 / AC 7 · dmg 1d4 · armour 1d4+1
    pub fn kestral(position: Position) -> Self {
        Self::new("kestral", 'K', Color::Grey, MovementType::Chase, 1, 4, 0, 4, 1, position)
    }

    /// L — Leprechaun · Lvl 3 / AC 8 · dmg 1d4 · armour 1d4 · steals and bolts
    pub fn leprechaun(position: Position) -> Self {
        Self::new("leprechaun", 'L', Color::Green, MovementType::Flee, 2, 4, 0, 4, 0, position)
    }

    /// M — Medusa · Lvl 8 / AC 2 · dmg 1d10 · armour 1d8+1
    pub fn medusa(position: Position) -> Self {
        Self::new("medusa", 'M', Color::DarkGreen, MovementType::Chase, 6, 10, 0, 8, 1, position)
    }

    /// N — Nymph · Lvl 3 / AC 9 · dmg 1d4-1 · armour 1d4-1 · steals and bolts
    pub fn nymph(position: Position) -> Self {
        Self::new("nymph", 'N', Color::Magenta, MovementType::Flee, 2, 4, -1, 4, -1, position)
    }

    /// O — Orc · Lvl 1 / AC 6 · dmg 1d8 · armour 1d6
    pub fn orc(position: Position) -> Self {
        Self::new("orc", 'O', Color::Red, MovementType::Chase, 1, 8, 0, 6, 0, position)
    }

    /// P — Phantom · Lvl 8 / AC 3 · dmg 1d10 · armour 1d8
    pub fn phantom(position: Position) -> Self {
        Self::new("phantom", 'P', Color::DarkGrey, MovementType::Chase, 6, 10, 0, 8, 0, position)
    }

    /// Q — Quagga · Lvl 3 / AC 2 · dmg 1d6 · armour 1d8+1
    pub fn quagga(position: Position) -> Self {
        Self::new("quagga", 'Q', Color::DarkYellow, MovementType::Chase, 2, 6, 0, 8, 1, position)
    }

    /// R — Rattlesnake · Lvl 2 / AC 3 · dmg 1d6 · armour 1d8
    pub fn rattlesnake(position: Position) -> Self {
        Self::new("rattlesnake", 'R', Color::DarkGreen, MovementType::Chase, 2, 6, 0, 8, 0, position)
    }

    /// S — Slime · Lvl 2 / AC 8 · dmg 1d4 · armour 1d4
    pub fn slime(position: Position) -> Self {
        Self::new("slime", 'S', Color::DarkGreen, MovementType::Chase, 2, 4, 0, 4, 0, position)
    }

    /// T — Troll · Lvl 6 / AC 4 · dmg 1d10 · armour 1d6+1
    pub fn troll(position: Position) -> Self {
        Self::new("troll", 'T', Color::DarkGreen, MovementType::Chase, 4, 10, 0, 6, 1, position)
    }

    /// U — Ur-vile · Lvl 7 / AC -2 · dmg 1d10 · armour 1d12+1
    pub fn ur_vile(position: Position) -> Self {
        Self::new("ur-vile", 'U', Color::DarkMagenta, MovementType::Chase, 5, 10, 0, 12, 1, position)
    }

    /// V — Vampire · Lvl 8 / AC 1 · dmg 1d10 · armour 1d10+1
    pub fn vampire(position: Position) -> Self {
        Self::new("vampire", 'V', Color::DarkRed, MovementType::Chase, 6, 10, 0, 10, 1, position)
    }

    /// W — Wraith · Lvl 5 / AC 4 · dmg 1d6 · armour 1d6+1
    pub fn wraith(position: Position) -> Self {
        Self::new("wraith", 'W', Color::DarkGrey, MovementType::Chase, 3, 6, 0, 6, 1, position)
    }

    /// X — Xeroc · Lvl 7 / AC 7 · dmg 1d8 · armour 1d4+1 · mimics an object
    pub fn xeroc(position: Position) -> Self {
        Self::new("xeroc", 'X', Color::Yellow, MovementType::Static, 5, 8, 0, 4, 1, position)
    }

    /// Y — Yeti · Lvl 4 / AC 6 · dmg 1d8 · armour 1d6
    pub fn yeti(position: Position) -> Self {
        Self::new("yeti", 'Y', Color::White, MovementType::Chase, 3, 8, 0, 6, 0, position)
    }

    /// Z — Zombie · Lvl 2 / AC 8 · dmg 1d8 · armour 1d4
    pub fn zombie(position: Position) -> Self {
        Self::new("zombie", 'Z', Color::DarkGrey, MovementType::Chase, 2, 8, 0, 4, 0, position)
    }
}
