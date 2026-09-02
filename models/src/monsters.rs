use bevy_ecs::prelude::*;
use crossterm::style::Color;
use crate::components::*;
use crate::effects::{grant_all, ColdImmune, FireImmune, Grant, Grants, Undead, VorpalTarget};
use MovementType::{Chase, Confused, Flee, Static};

/// Static, per-species description: everything about a monster that does not vary
/// between individuals of the same kind. The whole bestiary is one table of
/// these ([`BESTIARY`]), and every monster in the game — level population, the
/// create-monster scroll, the polymorph wand — is born by handing one to
/// [`spawn_monster`], so a species is never described in more than one place.
///
/// Micro-HP design: mobs live on a handful of HP and survive by winning the
/// opposed armour roll, not by having a fat health pool. `power`/`armor` are die
/// sizes (`1d[power]`, `1d[armor]`); `*_bonus` are flat modifiers added once to
/// each roll (see [`crate::combat::resolve_attack`]).
#[derive(Clone, Copy)]
pub struct MonsterDef {
    pub name: &'static str,
    pub glyph: char,
    pub color: Color,
    pub movement: MovementType,
    pub hp: i32,
    pub power: i32,
    pub power_bonus: i32,
    pub armor: i32,
    pub armor_bonus: i32,
    /// Danger tier `0..=3`. Each dungeon floor rolls from every tier it has
    /// unlocked so far, so low tiers keep turning up as fodder while deeper ones
    /// mix in (see [`crate::map`]). The goblin is the tier-0 baseline.
    pub tier: u8,
    /// The magic this species is born with, named the same way a ring names
    /// what it lends its wearer (see [`crate::effects::Grant`]). A dragon's
    /// `FireImmune` and a ring of fire resistance's `FireImmune` are the same
    /// component, so the wand of fire has one case to handle, not two.
    pub grants: &'static [Grant],
    /// Born [`Invisible`] — unseeable without a ring of perception (the phantom).
    pub invisible: bool,
}

impl MonsterDef {
    /// One bestiary row. A species with innate magic chains
    /// [`MonsterDef::grants`].
    #[allow(clippy::too_many_arguments)]
    const fn row(
        name: &'static str,
        glyph: char,
        color: Color,
        movement: MovementType,
        hp: i32,
        power: i32,
        power_bonus: i32,
        armor: i32,
        armor_bonus: i32,
        tier: u8,
    ) -> Self {
        Self {
            name, glyph, color, movement,
            hp, power, power_bonus, armor, armor_bonus,
            tier,
            grants: &[],
            invisible: false,
        }
    }

    /// Attach innate magic to a bestiary row.
    const fn grants(mut self, grants: &'static [Grant]) -> Self {
        self.grants = grants;
        self
    }

    /// Mark a bestiary row as born invisible (the phantom).
    const fn invisible(mut self) -> Self {
        self.invisible = true;
        self
    }

    /// Look up a species by name. Panics on an unknown name — callers pass
    /// string literals straight from the bestiary.
    pub fn named(name: &str) -> &'static MonsterDef {
        BESTIARY
            .iter()
            .find(|m| m.name == name)
            .unwrap_or_else(|| panic!("no monster named {name:?}"))
    }
}

/// The whole bestiary: the classic goblin plus the 26 lettered creatures, in one
/// table. Effects that pick a creature at random (scrolls of create monster and
/// vorpalize weapon) index straight into it.
///
/// Rogue reference — only Micro-HP and the two opposed rolls are modelled, but
/// each creature's original Lvl / AC is kept here as a design anchor:
///
/// | Species | Lvl/AC | | Species | Lvl/AC | | Species | Lvl/AC |
/// |---|---|---|---|---|---|---|---|
/// | A Aquator | 5 / 2 | | J Jabberwock | 15 / 6 | | S Slime | 2 / 8 |
/// | B Bat | 1 / 3 | | K Kestral | 1 / 7 | | T Troll | 6 / 4 |
/// | C Centaur | 4 / 4 | | L Leprechaun | 3 / 8 | | U Ur-vile | 7 / -2 |
/// | D Dragon | 10 / -1 | | M Medusa | 8 / 2 | | V Vampire | 8 / 1 |
/// | E Emu | 1 / 7 | | N Nymph | 3 / 9 | | W Wraith | 5 / 4 |
/// | F Venus Flytrap | 8 / 3 | | O Orc | 1 / 6 | | X Xeroc | 7 / 7 |
/// | G Griffin | 13 / 2 | | P Phantom | 8 / 3 | | Y Yeti | 4 / 6 |
/// | H Hobgoblin | 1 / 5 | | Q Quagga | 3 / 2 | | Z Zombie | 2 / 8 |
/// | I Ice Monster | 1 / 9 | | R Rattlesnake | 2 / 3 | | | |
///
/// F is rooted in place; I and X lie in wait; L and N steal and bolt — all
/// expressed through [`MonsterDef::movement`].
#[rustfmt::skip]
pub const BESTIARY: &[MonsterDef] = &[
    //              name             glyph  colour              move       hp  pow  pb   ar  ab  tier
    MonsterDef::row("goblin",        'g',   Color::Green,       Flee,       1,   4,   0,   6,  0,   0),
    MonsterDef::row("aquator",       'A',   Color::Blue,        Chase,      3,   4,  -1,   8,  1,   1),
    MonsterDef::row("bat",           'B',   Color::DarkGrey,    Confused,   1,   4,   0,   8,  0,   0),
    MonsterDef::row("centaur",       'C',   Color::DarkYellow,  Chase,      3,   8,   0,   6,  1,   1),
    MonsterDef::row("dragon",        'D',   Color::Red,         Chase,      8,  12,   2,  10,  2,   3).grants(&[Grant::of::<FireImmune>()]),
    MonsterDef::row("emu",           'E',   Color::DarkGreen,   Chase,      1,   4,   0,   4,  1,   0),
    MonsterDef::row("venus flytrap", 'F',   Color::Green,       Static,     6,  10,   0,   8,  0,   2),
    MonsterDef::row("griffin",       'G',   Color::DarkYellow,  Chase,     10,  12,   1,   8,  1,   3),
    MonsterDef::row("hobgoblin",     'H',   Color::DarkRed,     Chase,      1,   8,   0,   6,  0,   0),
    MonsterDef::row("ice monster",   'I',   Color::Cyan,        Static,     1,   4,   0,   4, -1,   0),
    MonsterDef::row("jabberwock",    'J',   Color::Magenta,     Chase,     12,   8,   5,   6,  0,   3).grants(&[Grant::of::<VorpalTarget>()]),
    MonsterDef::row("kestral",       'K',   Color::Grey,        Chase,      1,   4,   0,   4,  1,   0),
    MonsterDef::row("leprechaun",    'L',   Color::Green,       Flee,       2,   4,   0,   4,  0,   1),
    MonsterDef::row("medusa",        'M',   Color::DarkGreen,   Chase,      6,  10,   0,   8,  1,   2),
    MonsterDef::row("nymph",         'N',   Color::Magenta,     Flee,       2,   4,  -1,   4, -1,   1),
    MonsterDef::row("orc",           'O',   Color::Red,         Chase,      1,   8,   0,   6,  0,   0),
    MonsterDef::row("phantom",       'P',   Color::DarkGrey,    Chase,      6,  10,   0,   8,  0,   2).grants(&[Grant::of::<Undead>()]).invisible(),
    MonsterDef::row("quagga",        'Q',   Color::DarkYellow,  Chase,      2,   6,   0,   8,  1,   1),
    MonsterDef::row("rattlesnake",   'R',   Color::DarkGreen,   Chase,      2,   6,   0,   8,  0,   1),
    MonsterDef::row("slime",         'S',   Color::DarkGreen,   Chase,      2,   4,   0,   4,  0,   1),
    MonsterDef::row("troll",         'T',   Color::DarkGreen,   Chase,      4,  10,   0,   6,  1,   2),
    MonsterDef::row("ur-vile",       'U',   Color::DarkMagenta, Chase,      5,  10,   0,  12,  1,   2),
    MonsterDef::row("vampire",       'V',   Color::DarkRed,     Chase,      6,  10,   0,  10,  1,   3).grants(&[Grant::of::<Undead>()]),
    MonsterDef::row("wraith",        'W',   Color::DarkGrey,    Chase,      3,   6,   0,   6,  1,   2).grants(&[Grant::of::<Undead>()]),
    MonsterDef::row("xeroc",         'X',   Color::Yellow,      Static,     5,   8,   0,   4,  1,   2),
    MonsterDef::row("yeti",          'Y',   Color::White,       Chase,      3,   8,   0,   6,  0,   1).grants(&[Grant::of::<ColdImmune>()]),
    MonsterDef::row("zombie",        'Z',   Color::DarkGrey,    Chase,      2,   8,   0,   4,  0,   1).grants(&[Grant::of::<Undead>()]),
];

/// Every component a monster is spawned with, built wholesale from a
/// [`MonsterDef`] — including a record of the magic it was born with and a
/// `Normal` [`Speed`], so the entity is complete the moment it lands in the
/// world.
#[derive(Bundle)]
struct MonsterBundle {
    name: Name,
    mob: Mob,
    fighter: Fighter,
    glyph: Renderable,
    position: Position,
    faction: Faction,
    blood: Blood,
    /// What this creature was born with — read by the wand of cancellation, and
    /// by [`crate::equipment`] so a removed ring can never strip innate magic.
    grants: Grants,
    speed: Speed,
}

impl MonsterBundle {
    fn from_def(def: &MonsterDef, position: Position) -> Self {
        Self {
            name: Name { what: def.name.to_string() },
            mob: Mob { movement_type: def.movement },
            fighter: Fighter {
                hp: def.hp,
                max_hp: def.hp,
                power: def.power,
                max_power: def.power,
                power_bonus: def.power_bonus,
                armor: def.armor,
                armor_bonus: def.armor_bonus,
            },
            glyph: Renderable { glyph: def.glyph, color: def.color },
            position,
            faction: Faction::Monster,
            blood: Blood,
            grants: Grants(def.grants),
            speed: Speed::new(SpeedKind::Normal),
        }
    }
}

/// The one place a monster is brought into the world: builds the full
/// [`MonsterBundle`] from `def`, spawns it at `pos`, attaches the effect
/// components its row grants, and tacks on the [`Invisible`] marker for the
/// species that need it. Every spawn site — level population, the
/// create-monster scroll, the polymorph wand — goes through here so nothing
/// about a creature is assembled twice.
pub fn spawn_monster(world: &mut World, def: &MonsterDef, pos: Position) -> Entity {
    let e = world.spawn(MonsterBundle::from_def(def, pos)).id();
    grant_all(world, e, def.grants);
    if def.invisible {
        world.entity_mut(e).insert(Invisible);
    }
    e
}
