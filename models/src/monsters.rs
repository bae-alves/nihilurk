use bevy_ecs::prelude::*;
use crossterm::style::Color;
use rand::Rng;
use rand_chacha::ChaCha12Rng;

use crate::catalog::ItemDef;
use crate::components::*;
use crate::effects::{
    Batty, Binds, CoinGreedy, ColdImmune, FireBreath, FireImmune, Flies, Freezing, Gorgon, Grant,
    Grants, ItemUser, Regenerates, RustsArmor, Splits, StealsAndFlees, StealsAndVanishes, Undead,
    Vampiric, Venomous, VorpalTarget, grant_all,
};
use crate::equipment::equip_silently;
use crate::map::{FINAL_DEPTH, GameRng};
use crate::spawn::pick_weighted;
use MovementType::{Ambush, Chase};

/// The spawn weight a bestiary row gets when it doesn't `.weight(n)` for itself.
/// Defined and documented in `constants.rs`.
use crate::constants::monsters::{
    CENTAUR_BOW_CHANCE, DEFAULT_SPAWN_WEIGHT as DEFAULT_WEIGHT, HOBGOBLIN_GEAR_CHANCE,
    MEDUSA_BOW_CHANCE, ORC_GEAR_CHANCE,
};

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
    /// while deeper ones mix in. The bat is a depth-1 baseline; the mid tier
    /// holds off until floor 5 and the dragon waits until floor 10.
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
    /// Born at this tempo rather than [`SpeedKind::Normal`] — the wraith's
    /// permanent haste.
    pub speed: SpeedKind,
    /// Odds, rolled independently, of turning up already carrying a piece of
    /// gear — a centaur's bow, a hobgoblin's chance at a weapon, an armour and
    /// a ring all at once. See [`roll_spawn_gear`].
    pub equip_rolls: &'static [EquipRoll],
    /// Born disguised as an item until the player is adjacent — the xeroc. See
    /// [`disguise_as_item`].
    pub mimics: bool,
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
            speed: SpeedKind::Normal,
            equip_rolls: &[],
            mimics: false,
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

    /// Mark a bestiary row as permanently hasted (the wraith).
    const fn fast(mut self) -> Self {
        self.speed = SpeedKind::Fast;
        self
    }

    /// Give a bestiary row a chance of arriving already geared up.
    const fn equip(mut self, rolls: &'static [EquipRoll]) -> Self {
        self.equip_rolls = rolls;
        self
    }

    /// Mark a bestiary row as a mimic, born disguised as an item (the xeroc).
    ///
    /// Never pair this with [`MonsterDef::invisible`]. A mimic hides by
    /// disguise; [`crate::monsters::reveal_mimics`] strips that the instant
    /// the player is adjacent. An invisible row hides by not being drawn at
    /// all. Combine them and the result is a creature that, having just been
    /// noticed, still cannot be seen -- a state neither mechanic resolves.
    /// `models/tests/content.rs`'s `a_mimic_is_never_also_invisible` holds
    /// this to the fire.
    const fn mimics(mut self) -> Self {
        self.mimics = true;
        self
    }

    /// Make a species rarer or commoner than its floor-mates.
    ///
    /// No row asks for it at present — within a tier nihilurk draws evenly on
    /// purpose — so it is kept as the extension point the docs teach
    /// (`docs/tutorial/add-your-first-monster.md`) rather than deleted as
    /// unused. The same is true of `ItemDef::weight` next door.
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
/// with steel — orc, hobgoblin, troll — and the cunning ones that covet it: the
/// centaur, the two thieves, the medusa, the ur-vile, the vampire. The mindless
/// humanoids are deliberately left out: a zombie has hands and no idea what to
/// do with them.
const ITEM_USER: &[Grant] = &[Grant::of::<ItemUser>()];

/// How often a bestiary row that carries [`EquipRoll`]s rolls each one, and what
/// it reaches for when it hits. Rolled independently at spawn by
/// [`roll_spawn_gear`] — a hobgoblin's three rolls (weapon, armour, ring) can
/// land none, one, two or all three.
#[derive(Clone, Copy)]
pub struct EquipRoll {
    pub chance: f64,
    pub kind: EquipKind,
}

/// What one [`EquipRoll`] reaches for.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum EquipKind {
    /// A random melee weapon off [`crate::catalog::WEAPONS`].
    Weapon,
    /// A random suit off [`crate::catalog::ARMORS`].
    Armor,
    /// A random ring off [`crate::catalog::RINGS`].
    Ring,
    /// A short bow, with a bundle of arrows left at its wearer's feet.
    Bow,
}

/// The whole bestiary: 26 lettered creatures, in one table. Effects that pick a
/// creature at random (scrolls of create monster and vorpalize weapon) index
/// straight into it, and floor population draws from it through
/// [`MonsterDef::pick`].
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
/// F, I and X lie in wait; L and N steal and bolt — all expressed through
/// [`MonsterDef::movement`].
#[rustfmt::skip]
pub const BESTIARY: &[MonsterDef] = &[
    // A row is the whole species. Columns after `ab` are the two dials that
    // decide where and how often it shows up; `.grants(...)`, `.invisible()`,
    // `.fast()`, `.equip(...)`, `.mimics()` and `.weight(n)` are chained on
    // when a row wants more than the default.
    //              name             glyph  colour              move       hp  pow  pb   ar  ab  dep
    MonsterDef::row("aquator",       'A',   Color::Blue,        Chase,      3,   4,  -1,   8,  1,   5).grants(&[Grant::of::<RustsArmor>()]),
    MonsterDef::row("bat",           'B',   Color::DarkGrey,    Chase,      1,   4,   0,   8,  0,   1).grants(&[Grant::of::<Batty>()]),
    MonsterDef::row("centaur",       'C',   Color::DarkYellow,  Chase,      3,   8,   0,   6,  1,   5)
        .grants(ITEM_USER)
        .equip(&[EquipRoll { chance: CENTAUR_BOW_CHANCE, kind: EquipKind::Bow }]),
    MonsterDef::row("dragon",        'D',   Color::Red,         Chase,      8,  12,   2,  10,  2,  10).grants(&[Grant::of::<FireImmune>(), Grant::of::<Flies>(), Grant::of::<FireBreath>()]),
    MonsterDef::row("emu",           'E',   Color::DarkGreen,   Chase,      1,   4,   0,   4,  1,   1),
    MonsterDef::row("venus flytrap", 'F',   Color::Green,       Ambush,     6,  10,   0,   8,  0,   5).grants(&[Grant::of::<Binds>()]),
    MonsterDef::row("griffin",       'G',   Color::DarkYellow,  Chase,     10,  12,   1,   8,  1,  10).grants(&[Grant::of::<Flies>(), Grant::of::<Regenerates>()]),
    MonsterDef::row("hobgoblin",     'H',   Color::DarkRed,     Chase,      1,   8,   0,   6,  0,   1)
        .grants(ITEM_USER)
        .equip(&[
            EquipRoll { chance: HOBGOBLIN_GEAR_CHANCE, kind: EquipKind::Weapon },
            EquipRoll { chance: HOBGOBLIN_GEAR_CHANCE, kind: EquipKind::Armor },
            EquipRoll { chance: HOBGOBLIN_GEAR_CHANCE, kind: EquipKind::Ring },
        ]),
    MonsterDef::row("ice monster",   'I',   Color::Cyan,        Ambush,     1,   4,   0,   4, -1,   1).grants(&[Grant::of::<Freezing>()]),
    MonsterDef::row("jabberwock",    'J',   Color::Magenta,     Chase,     12,   8,   5,   6,  0,  10).grants(&[Grant::of::<VorpalTarget>(), Grant::of::<Flies>()]),
    MonsterDef::row("kestral",       'K',   Color::Grey,        Chase,      1,   4,   0,   4,  1,   1).grants(&[Grant::of::<Flies>()]),
    MonsterDef::row("leprechaun",    'L',   Color::Green,       Chase,      2,   4,   0,   4,  0,   5).grants(&[Grant::of::<ItemUser>(), Grant::of::<StealsAndFlees>()]),
    MonsterDef::row("medusa",        'M',   Color::DarkGreen,   Chase,      6,  10,   0,   8,  1,   5)
        .grants(&[Grant::of::<ItemUser>(), Grant::of::<Gorgon>()])
        .equip(&[EquipRoll { chance: MEDUSA_BOW_CHANCE, kind: EquipKind::Bow }]),
    MonsterDef::row("nymph",         'N',   Color::Magenta,     Chase,      2,   4,  -1,   4, -1,   5).grants(&[Grant::of::<ItemUser>(), Grant::of::<StealsAndVanishes>()]),
    MonsterDef::row("orc",           'O',   Color::Red,         Chase,      1,   8,   0,   6,  0,   1)
        .grants(&[Grant::of::<ItemUser>(), Grant::of::<CoinGreedy>()])
        .equip(&[
            EquipRoll { chance: ORC_GEAR_CHANCE, kind: EquipKind::Weapon },
            EquipRoll { chance: ORC_GEAR_CHANCE, kind: EquipKind::Armor },
            EquipRoll { chance: ORC_GEAR_CHANCE, kind: EquipKind::Ring },
        ]),
    MonsterDef::row("phantom",       'P',   Color::DarkGrey,    Chase,      6,  10,   0,   8,  0,   5).grants(&[Grant::of::<Undead>(), Grant::of::<Batty>()]).invisible(),
    MonsterDef::row("quagga",        'Q',   Color::DarkYellow,  Chase,      2,   6,   0,   8,  1,   5),
    MonsterDef::row("rattlesnake",   'R',   Color::DarkGreen,   Chase,      2,   6,   0,   8,  0,   5).grants(&[Grant::of::<Venomous>()]),
    MonsterDef::row("slime",         'S',   Color::DarkGreen,   Chase,      2,   4,   0,   4,  0,   5).grants(&[Grant::of::<Splits>()]),
    MonsterDef::row("troll",         'T',   Color::DarkGreen,   Chase,      4,  10,   0,   6,  1,   5).grants(&[Grant::of::<ItemUser>(), Grant::of::<Regenerates>()]),
    MonsterDef::row("ur-vile",       'U',   Color::DarkMagenta, Chase,      5,  10,   0,  12,  1,   5).grants(ITEM_USER),
    MonsterDef::row("vampire",       'V',   Color::DarkRed,     Chase,      6,  10,   0,  10,  1,  10).grants(&[Grant::of::<Undead>(), Grant::of::<ItemUser>(), Grant::of::<Regenerates>(), Grant::of::<Vampiric>()]),
    MonsterDef::row("wraith",        'W',   Color::DarkGrey,    Chase,      3,   6,   0,   6,  1,   5).grants(&[Grant::of::<Undead>()]).fast(),
    MonsterDef::row("xeroc",         'X',   Color::Yellow,      Ambush,     5,   8,   0,   4,  1,  13).grants(&[Grant::of::<Binds>()]).mimics(),
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
            speed: Speed::new(def.speed),
        }
    }
}

/// The one place a monster is brought into the world: builds the full
/// [`MonsterBundle`] from `def`, spawns it at `pos`, attaches the effect
/// components its row grants, and tacks on the [`Invisible`] marker for the
/// species that need it. Every spawn site — level population, the
/// create-monster scroll, the polymorph wand — goes through here so nothing
/// about a creature is assembled twice.
///
/// This is the whole of what a rng-less caller (a bare test fixture) gets: no
/// gear roll, no mimic disguise, since both need dice. [`spawn_monster`] and
/// [`spawn_monster_with_rng`] are the two callers that hand those a source of
/// randomness and get the rest of the creature besides.
fn base_spawn(world: &mut World, def: &MonsterDef, pos: Position) -> Entity {
    let e = world.spawn(MonsterBundle::from_def(def, pos)).id();
    grant_all(world, e, def.grants);
    if def.invisible {
        world.entity_mut(e).insert(Invisible);
    }
    e
}

/// [`base_spawn`] plus whatever `def` rolls dice for: its [`EquipRoll`]s
/// ([`roll_spawn_gear`]) and its mimic disguise ([`disguise_as_item`]), off
/// the shared [`GameRng`] — the ordinary way to spawn a monster at runtime (a
/// scroll of create monster, a wand of polymorph), where borrowing a few
/// rolls from the shared stream is exactly what the rest of that effect
/// already does.
///
/// A world with no [`GameRng`] resource (a bare test fixture) falls back to
/// [`base_spawn`] alone — no gear, no disguise — rather than panicking.
pub fn spawn_monster(world: &mut World, def: &MonsterDef, pos: Position) -> Entity {
    let e = base_spawn(world, def, pos);
    let Some(GameRng(mut rng)) = world.remove_resource::<GameRng>() else {
        return e;
    };
    roll_spawn_gear(world, e, def, &mut rng);
    if def.mimics {
        disguise_as_item(world, e, &mut rng);
    }
    world.insert_resource(GameRng(rng));
    e
}

/// [`spawn_monster`], but drawing its dice from `rng` instead of the shared
/// [`GameRng`] resource. This is what floor population
/// ([`crate::map::population::populate_level`]) calls: a floor's *contents*
/// are a pure function of `(seed, depth, staircases taken)`, rolled off their
/// own dedicated stream, and reaching into the shared one here — even for
/// something as small as a hobgoblin's chance at a sword — would let whatever
/// the player did on an *earlier* floor perturb what a later one is stocked
/// with.
pub fn spawn_monster_with_rng(
    world: &mut World,
    def: &MonsterDef,
    pos: Position,
    rng: &mut ChaCha12Rng,
) -> Entity {
    let e = base_spawn(world, def, pos);
    roll_spawn_gear(world, e, def, rng);
    if def.mimics {
        disguise_as_item(world, e, rng);
    }
    e
}

// ---------------------------------------------------------------------------
// Gear rolled at spawn
// ---------------------------------------------------------------------------

/// Rolls a bestiary row's [`EquipRoll`]s: each fires independently, and a hit
/// finds the gear and puts it on in silence ([`equip_silently`]), announced
/// exactly the way any other freshly worn item is — but only when the player
/// can actually see the spot, so a hobgoblin geared up on the far side of the
/// floor doesn't spoil itself in the log before it's ever met.
fn roll_spawn_gear(world: &mut World, mob: Entity, def: &MonsterDef, rng: &mut ChaCha12Rng) {
    if def.equip_rolls.is_empty() {
        return;
    }
    let Some(pos) = world.get::<Position>(mob).copied() else {
        return;
    };
    for roll in def.equip_rolls {
        if rng.gen_bool(roll.chance) {
            equip_one(world, mob, roll.kind, pos, rng);
        }
    }
}

/// One [`EquipRoll`] that hit.
fn equip_one(
    world: &mut World,
    mob: Entity,
    kind: EquipKind,
    pos: Position,
    rng: &mut ChaCha12Rng,
) {
    use crate::catalog::{ARMORS, LAUNCHERS, RINGS, WEAPONS};
    match kind {
        EquipKind::Weapon => equip_random(world, mob, WEAPONS, pos, rng),
        EquipKind::Armor => equip_random(world, mob, ARMORS, pos, rng),
        EquipKind::Ring => equip_random(world, mob, RINGS, pos, rng),
        EquipKind::Bow => equip_launcher(world, mob, &LAUNCHERS[0], pos, rng),
    }
}

/// A uniformly random row of `table`, rolled as a floor drop (enchantment and
/// all) and put on `mob` in silence.
fn equip_random<D: crate::catalog::ItemDef>(
    world: &mut World,
    mob: Entity,
    table: &'static [D],
    pos: Position,
    rng: &mut ChaCha12Rng,
) {
    let idx = rng.gen_range(0..table.len());
    let item = table[idx].spawn_as_loot(world, rng, pos);
    equip_and_announce(world, mob, item);
}

/// A bow or crossbow, put on `mob`, with a bundle of the ammunition it fires
/// left at its feet — nothing here gives a monster a pack to carry it in, so
/// the bundle is loot from the moment it drops, exactly as if it had died on
/// the spot.
fn equip_launcher(
    world: &mut World,
    mob: Entity,
    def: &crate::catalog::LauncherDef,
    pos: Position,
    rng: &mut ChaCha12Rng,
) {
    let launcher = def.spawn_as_loot(world, rng, pos);
    equip_and_announce(world, mob, launcher);
    let ammo_name = if def.name == "crossbow" {
        "quarrel"
    } else {
        "arrow"
    };
    if let Some(ammo_def) = crate::catalog::AMMO.iter().find(|a| a.name == ammo_name) {
        ammo_def.spawn_as_loot(world, rng, pos);
    }
}

/// Puts `item` on `mob` and, if it actually went on and the player can see the
/// tile, announces it: `"It is wielding a long sword."` / `"It is wearing a
/// suit of banded mail."`
fn equip_and_announce(world: &mut World, mob: Entity, item: Entity) {
    if !equip_silently(world, mob, item) {
        return;
    }
    let Some(pos) = world.get::<Position>(mob).copied() else {
        return;
    };
    if !crate::helpers::player_sees(world, pos.x, pos.y) {
        return;
    }
    let slot = world
        .get::<crate::equipment::Equipped>(item)
        .map(|e| e.slot);
    let verb = match slot {
        Some(crate::equipment::Slot::Hand) => "wielding",
        _ => "wearing",
    };
    let name = crate::identify::with_article(world, item);
    world
        .resource_mut::<GameLog>()
        .add(format!("It is {verb} {name}."));
}

// ---------------------------------------------------------------------------
// The xeroc's disguise
// ---------------------------------------------------------------------------

/// The plain-item look-alikes a xeroc can wear on an ordinary floor: a name,
/// a glyph and a colour, off the same appearances the real things spawn with.
const MIMIC_LOOKS: &[(&str, char, Color)] = &[
    ("scroll", '?', Color::White),
    ("potion", '!', Color::Magenta),
    ("wand", '/', Color::Yellow),
    ("gold coin", '$', Color::Yellow),
    ("ring", '=', Color::Yellow),
    ("suit of armor", ']', Color::Grey),
    ("weapon", ')', Color::Grey),
];

/// Disguises a freshly spawned xeroc as an ordinary item: its [`Name`] and
/// [`Renderable`] are swapped for a look-alike's and [`Mimic`] goes on, which
/// is what excludes it from every "a monster is nearby" check —
/// [`crate::autoexplore::monster_in_sight`] and
/// [`crate::autofight::visible_enemies`] — so auto-explore and auto-fight are
/// fooled right along with the player, and the sighting line in the log reads
/// as spotting the fake item rather than the monster underneath it.
///
/// On the floor that holds the Element of Yoord, a xeroc always disguises as
/// the relic itself instead of drawing a random look-alike — the one bait a
/// player hunting the real thing cannot help but walk up to.
fn disguise_as_item(world: &mut World, mob: Entity, rng: &mut ChaCha12Rng) {
    let depth = world.get_resource::<Depth>().map_or(1, |d| d.what);
    let (name, glyph, color): (&str, char, Color) = if depth >= FINAL_DEPTH {
        (crate::spawn::ELEMENT_OF_YOORD, '"', Color::Magenta)
    } else {
        MIMIC_LOOKS[rng.gen_range(0..MIMIC_LOOKS.len())]
    };
    if let Some(mut n) = world.get_mut::<Name>(mob) {
        n.what = name.to_string();
    }
    if let Some(mut r) = world.get_mut::<Renderable>(mob) {
        r.glyph = glyph;
        r.color = color;
    }
    world.entity_mut(mob).insert(Mimic);
}

/// Strips a xeroc's disguise the instant the player is standing next to it:
/// restores its true name and glyph and removes [`Mimic`]. Scheduled just
/// before [`crate::ai`], so the very turn the player draws alongside one it
/// also gets to lash out — see [`MovementType::Ambush`].
///
/// Assumes the player's [`Position`] already reflects this turn's move — true
/// because `schedule.run` only fires once `player_step` has already resolved
/// it (`engine/src/main.rs`), and before `ai` reads the same position to
/// decide what a monster can see.
pub fn reveal_mimics(world: &mut World) {
    let Some(player_pos) = world
        .query_filtered::<&Position, With<Player>>()
        .iter(world)
        .next()
        .copied()
    else {
        return;
    };
    let disguised: Vec<Entity> = world
        .query_filtered::<(Entity, &Position), With<Mimic>>()
        .iter(world)
        .filter(|(_, p)| {
            (p.x as i32 - player_pos.x as i32).abs() <= 1
                && (p.y as i32 - player_pos.y as i32).abs() <= 1
        })
        .map(|(e, _)| e)
        .collect();

    for xeroc in disguised {
        let def = MonsterDef::named("xeroc");
        if let Some(mut n) = world.get_mut::<Name>(xeroc) {
            n.what = def.name.to_string();
        }
        if let Some(mut r) = world.get_mut::<Renderable>(xeroc) {
            r.glyph = def.glyph;
            r.color = def.color;
        }
        world.entity_mut(xeroc).remove::<Mimic>();
        world.entity_mut(xeroc).remove::<Spotted>();
        world
            .resource_mut::<GameLog>()
            .add("The disguise falls away — it was a xeroc all along!".to_string());
    }
}

// ---------------------------------------------------------------------------
// The slime's split
// ---------------------------------------------------------------------------

/// A slime that survived a wound buds a fresh copy of itself at its current
/// HP, if there is somewhere for the copy to stand. Called from
/// [`crate::helpers::took_damage`], which every damage path in the game —
/// melee included — already runs through.
pub fn maybe_split(world: &mut World, victim: Entity) {
    if world.get::<crate::effects::Splits>(victim).is_none() {
        return;
    }
    let Some(hp) = world
        .get::<Fighter>(victim)
        .map(|f| f.hp)
        .filter(|&hp| hp > 0)
    else {
        return;
    };
    let Some(pos) = world.get::<Position>(victim).copied() else {
        return;
    };
    let Some((x, y)) = crate::helpers::free_adjacent_tile(world, pos) else {
        return;
    };
    let Some(name) = world.get::<Name>(victim).map(|n| n.what.clone()) else {
        return;
    };
    let def = MonsterDef::named(&name);
    let clone = spawn_monster(world, def, Position { x, y });
    if let Some(mut f) = world.get_mut::<Fighter>(clone) {
        f.hp = hp;
        f.max_hp = hp;
    }
}
