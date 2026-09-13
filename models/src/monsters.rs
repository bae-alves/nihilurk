use bevy_ecs::prelude::*;
use crossterm::style::Color;
use rand_chacha::ChaCha12Rng;

use crate::components::*;
use crate::effects::{
    ColdImmune, FireImmune, Grant, Grants, ItemUser, RustsArmor, Undead, VorpalTarget, grant_all,
};
use crate::spawn::pick_weighted;
use MovementType::{Chase, Confused, Flee, Static};

/// The spawn weight a bestiary row gets when it doesn't `.weight(n)` for itself.
/// Defined and documented in `constants.rs`.
use crate::constants::monsters::DEFAULT_SPAWN_WEIGHT as DEFAULT_WEIGHT;

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
    /// The shallowest floor this species appears on. A floor rolls from every
    /// row it has unlocked so far, so shallow letters keep turning up as fodder
    /// while deeper ones mix in. The goblin is the depth-1 baseline; the mid
    /// tier holds off until floor 5 and the dragon waits until floor 10.
    pub min_depth: u8,
    /// How often this species turns up relative to the rest of the eligible
    /// pool. Ten is the baseline: a row at 5 is half as common, one at 20 twice
    /// as common. See [`MonsterDef::pick`].
    pub weight: u32,
    /// The magic this species is born with, named the same way a ring names
    /// what it lends its wearer (see [`crate::effects::Grant`]). A dragon's
    /// `FireImmune` and a ring of fire resistance's `FireImmune` are the same
    /// component, so the wand of fire has one case to handle, not two.
    pub grants: &'static [Grant],
    /// Born [`Invisible`] — unseeable without a ring of perception (the phantom).
    pub invisible: bool,
}

impl MonsterDef {
    /// One bestiary row: the ten numbers every species needs. Anything past
    /// that — innate magic, invisibility, an unusual rarity — is chained on
    /// afterwards, so a plain creature stays one readable line.
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
        min_depth: u8,
    ) -> Self {
        Self {
            name,
            glyph,
            color,
            movement,
            hp,
            power,
            power_bonus,
            armor,
            armor_bonus,
            min_depth,
            weight: DEFAULT_WEIGHT,
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

    /// Make a species rarer or commoner than its floor-mates.
    #[allow(dead_code)]
    const fn weight(mut self, weight: u32) -> Self {
        self.weight = weight;
        self
    }

    /// Look up a species by name. Panics on an unknown name — callers pass
    /// string literals straight from the bestiary. Use [`MonsterDef::lookup`]
    /// for a name that came from outside the source, such as a save file.
    pub fn named(name: &str) -> &'static MonsterDef {
        Self::lookup(name).unwrap_or_else(|| panic!("no monster named {name:?}"))
    }

    /// Look up a species by name, or `None` if the bestiary has no such row.
    pub fn lookup(name: &str) -> Option<&'static MonsterDef> {
        BESTIARY.iter().find(|m| m.name == name)
    }

    /// Picks a species appropriate for `depth`: a weighted draw from every row
    /// the floor has unlocked. This is the only place the dungeon decides what
    /// lives on a floor, so a new creature's rarity and debut are the two
    /// numbers on its row and nothing else.
    pub fn pick(depth: u8, rng: &mut ChaCha12Rng) -> &'static MonsterDef {
        Self::draw(rng, |m| m.min_depth <= depth.max(1))
    }

    /// A weighted draw from the *whole* bestiary, depth gate and all. Once the
    /// Element of Yoord is in the pack the dungeon stops holding anything back:
    /// the climb out re-populates each floor through here, so a dragon can turn
    /// up on floor 1. See [`crate::map::holding_element_of_yoord`].
    pub fn pick_any(rng: &mut ChaCha12Rng) -> &'static MonsterDef {
        Self::draw(rng, |_| true)
    }

    /// The shared body of [`pick`](Self::pick) and [`pick_any`](Self::pick_any):
    /// a weighted draw from every bestiary row `eligible` accepts.
    fn draw(rng: &mut ChaCha12Rng, eligible: impl Fn(&MonsterDef) -> bool) -> &'static MonsterDef {
        let pool: Vec<&MonsterDef> = BESTIARY.iter().filter(|m| eligible(m)).collect();
        let weights: Vec<u32> = pool.iter().map(|m| m.weight).collect();
        pool[pick_weighted(&weights, rng).expect("the bestiary always has a depth-1 row")]
    }
}

/// The humanoids with the wits to use what they find: they catch thrown gear and
/// wear it, and they read thrown scrolls aloud. The brutes that already fight
/// with steel — goblin, orc, hobgoblin, troll — and the cunning ones that covet
/// it: the centaur, the two thieves, the medusa, the ur-vile, the vampire. The
/// mindless humanoids are deliberately left out: a zombie has hands and no idea
/// what to do with them.
const ITEM_USER: &[Grant] = &[Grant::of::<ItemUser>()];

/// The whole bestiary: the classic goblin plus the 26 lettered creatures, in one
/// table. Effects that pick a creature at random (scrolls of create monster and
/// vorpalize weapon) index straight into it, and floor population draws from it
/// through [`MonsterDef::pick`].
///
/// **This table is the bestiary.** Adding a creature is adding a line here; no
/// other file in the game enumerates species. See `docs/how-to/add-a-monster.md`.
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
    // A row is the whole species. Columns after `ab` are the two dials that
    // decide where and how often it shows up; `.grants(...)`, `.invisible()`
    // and `.weight(n)` are chained on when a row wants more than the default.
    //              name             glyph  colour              move       hp  pow  pb   ar  ab  dep
    MonsterDef::row("goblin",        'g',   Color::Green,       Flee,       1,   4,   0,   6,  0,   1).grants(ITEM_USER),
    MonsterDef::row("aquator",       'A',   Color::Blue,        Chase,      3,   4,  -1,   8,  1,   5).grants(&[Grant::of::<RustsArmor>()]),
    MonsterDef::row("bat",           'B',   Color::DarkGrey,    Confused,   1,   4,   0,   8,  0,   1),
    MonsterDef::row("centaur",       'C',   Color::DarkYellow,  Chase,      3,   8,   0,   6,  1,   5).grants(ITEM_USER),
    MonsterDef::row("dragon",        'D',   Color::Red,         Chase,      8,  12,   2,  10,  2,  10).grants(&[Grant::of::<FireImmune>()]),
    MonsterDef::row("emu",           'E',   Color::DarkGreen,   Chase,      1,   4,   0,   4,  1,   1),
    MonsterDef::row("venus flytrap", 'F',   Color::Green,       Static,     6,  10,   0,   8,  0,   5),
    MonsterDef::row("griffin",       'G',   Color::DarkYellow,  Chase,     10,  12,   1,   8,  1,  10),
    MonsterDef::row("hobgoblin",     'H',   Color::DarkRed,     Chase,      1,   8,   0,   6,  0,   1).grants(ITEM_USER),
    MonsterDef::row("ice monster",   'I',   Color::Cyan,        Static,     1,   4,   0,   4, -1,   1),
    MonsterDef::row("jabberwock",    'J',   Color::Magenta,     Chase,     12,   8,   5,   6,  0,  10).grants(&[Grant::of::<VorpalTarget>()]),
    MonsterDef::row("kestral",       'K',   Color::Grey,        Chase,      1,   4,   0,   4,  1,   1),
    MonsterDef::row("leprechaun",    'L',   Color::Green,       Flee,       2,   4,   0,   4,  0,   5).grants(ITEM_USER),
    MonsterDef::row("medusa",        'M',   Color::DarkGreen,   Chase,      6,  10,   0,   8,  1,   5).grants(ITEM_USER),
    MonsterDef::row("nymph",         'N',   Color::Magenta,     Flee,       2,   4,  -1,   4, -1,   5).grants(ITEM_USER),
    MonsterDef::row("orc",           'O',   Color::Red,         Chase,      1,   8,   0,   6,  0,   1).grants(ITEM_USER),
    MonsterDef::row("phantom",       'P',   Color::DarkGrey,    Chase,      6,  10,   0,   8,  0,   5).grants(&[Grant::of::<Undead>()]).invisible(),
    MonsterDef::row("quagga",        'Q',   Color::DarkYellow,  Chase,      2,   6,   0,   8,  1,   5),
    MonsterDef::row("rattlesnake",   'R',   Color::DarkGreen,   Chase,      2,   6,   0,   8,  0,   5),
    MonsterDef::row("slime",         'S',   Color::DarkGreen,   Chase,      2,   4,   0,   4,  0,   5),
    MonsterDef::row("troll",         'T',   Color::DarkGreen,   Chase,      4,  10,   0,   6,  1,   5).grants(ITEM_USER),
    MonsterDef::row("ur-vile",       'U',   Color::DarkMagenta, Chase,      5,  10,   0,  12,  1,   5).grants(ITEM_USER),
    MonsterDef::row("vampire",       'V',   Color::DarkRed,     Chase,      6,  10,   0,  10,  1,  10).grants(&[Grant::of::<Undead>(), Grant::of::<ItemUser>()]),
    MonsterDef::row("wraith",        'W',   Color::DarkGrey,    Chase,      3,   6,   0,   6,  1,   5).grants(&[Grant::of::<Undead>()]),
    MonsterDef::row("xeroc",         'X',   Color::Yellow,      Static,     5,   8,   0,   4,  1,   5),
    MonsterDef::row("yeti",          'Y',   Color::White,       Chase,      3,   8,   0,   6,  0,   5).grants(&[Grant::of::<ColdImmune>()]),
    MonsterDef::row("zombie",        'Z',   Color::DarkGrey,    Chase,      2,   8,   0,   4,  0,   5).grants(&[Grant::of::<Undead>()]),
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
            name: Name {
                what: def.name.to_string(),
            },
            mob: Mob {
                movement_type: def.movement,
            },
            fighter: Fighter {
                hp: def.hp,
                max_hp: def.hp,
                power: def.power,
                max_power: def.power,
                power_bonus: def.power_bonus,
                armor: def.armor,
                armor_bonus: def.armor_bonus,
            },
            glyph: Renderable {
                glyph: def.glyph,
                color: def.color,
            },
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
