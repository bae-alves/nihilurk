//! The bestiary, and everything that turns a [`MonsterDef`] row into a
//! creature standing on a tile.
//!
//! [`BESTIARY`] is the single table every species is described by;
//! [`spawn_monster`]/[`spawn_monster_with_rng`] are the only doors into the
//! world, so gear rolls, mimic disguises and innate magic are assembled once
//! regardless of whether the spawn came from floor population, a scroll or a
//! wand. [`wear_monster`] is the odd one out: it puts a species' body on the
//! *player* instead of spawning a new entity, for playing as a monster.

use bevy_ecs::prelude::*;
use crossterm::style::Color;
use rand::Rng;
use rand::seq::SliceRandom;
use rand_chacha::ChaCha12Rng;

use crate::catalog::ItemDef;
use crate::components::*;
use crate::constants::decks::CARD_CASTER_CASTS;
use crate::constants::spirits::{BARTER_STOCK_MAX, BARTER_STOCK_MIN};
use crate::effects::{
    AlwaysHelper, AlwaysTamed, Batty, Binds, ColdImmune, FaerieOnDeath, FireImmune, Flies,
    Freezing, Gorgon, Grant, Grants, GreenBlood, ItemUser, Lifetime, Phasing, PriorityHelper,
    Regenerates, RustsArmor, ScoreBounty, ShapeshiftOnKill, Splits, StealsAndFlees,
    StealsAndVanishes, Swims, Undead, Vampiric, Venomous, VorpalTarget, grant_all, lend,
};
use crate::equipment::equip_silently;
use crate::map::{Endless, FINAL_DEPTH, GameRng};
use crate::particles::{BlastPalette, Particles, on_map};
use crate::spawn::{pick_weighted, roll_item_except};
use crate::spirits::{
    angel_event, blue_demon_event, gnome_event, pink_demon_event, red_demon_event,
    salamander_event, sphynx_event, sylphid_event, undyne_event, yellow_demon_event,
};
use MovementType::{Ambush, Chase, Confused};

/// The spawn weight a bestiary row gets when it doesn't `.weight(n)` for itself.
/// Defined and documented in `constants.rs`.
use crate::constants::monsters::{
    DEFAULT_SPAWN_WEIGHT as DEFAULT_WEIGHT, HIGH_GEAR_CHANCE, NORMAL_GEAR_CHANCE,
    ULTIMATE_GEAR_CHANCE,
};
use crate::constants::spirits::SPAWN_WEIGHT as SPIRIT_WEIGHT;

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
    /// The species' identity: the save file stores it and
    /// [`crate::spawn::spawn_named`] finds the row by it. The player reads
    /// [`MonsterDef::display_name`] instead.
    pub name: &'static str,
    /// The character it draws as.
    pub glyph: char,
    /// The colour it draws in.
    pub color: Color,
    /// The tactic it thinks with, which picks its rule set.
    pub movement: MovementType,
    /// Starting hit points: [`Fighter::hp`] and [`Fighter::max_hp`] both.
    pub hp: i32,
    /// The attack die, [`Fighter::power`].
    pub power: i32,
    /// A flat modifier on every damage roll, [`Fighter::power_bonus`].
    pub power_bonus: i32,
    /// The defence die, [`Fighter::armor`].
    pub armor: i32,
    /// A flat modifier on every armour roll, [`Fighter::armor_bonus`].
    pub armor_bonus: i32,
    /// The shallowest floor this species appears on. A floor rolls from every
    /// row it has unlocked so far, so shallow letters keep turning up as fodder
    /// while deeper ones mix in. The bat is a baseline from the first floor; the
    /// mid tier holds off and the dragon waits much deeper.
    pub min_depth: u8,
    /// How often this species turns up relative to the rest of the eligible
    /// pool. [`DEFAULT_SPAWN_WEIGHT`](crate::constants::monsters::DEFAULT_SPAWN_WEIGHT) is the baseline: half of it is half as
    /// common, double it twice as common. See [`MonsterDef::pick`].
    pub weight: u32,
    /// The magic this species is born with, named the same way a ring names
    /// what it lends its wearer (see [`crate::effects::Grant`]). A dragon's
    /// `FireImmune` and a ring of fire resistance's `FireImmune` are the same
    /// component, so the wand of fire has one case to handle, not two.
    pub grants: &'static [Grant],
    /// The spells this species is born knowing: its [`Spellset`], the same
    /// [`SpellEffect`]s a player learns, at the same [`SpellDef::cost`]. A
    /// player wearing the row gets them in the spell bar.
    pub spells: &'static [SpellEffect],
    /// How many times a monster of this row can cast its dearest spell: two
    /// for the beefy, three for the weak. It is born with that many
    /// casts' worth of [`Magic`] ([`MonsterDef::magic_pool`]), and the pool
    /// never refills: a caster that runs dry is back to its claws.
    pub casts: u8,
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
    /// `Some` makes this a [`Faction::Spirits`] row instead of an ordinary
    /// [`Faction::Monster`] one, and says which way it pulls [`Alignment`]
    /// when it poofs. See [`crate::spirits`].
    pub spirit_kind: Option<SpiritKind>,
    /// What a peaceful melee against this row does instead of a normal
    /// attack. `None` is the generic "nothing happens" fallback. Only
    /// meaningful alongside `spirit_kind: Some(_)`.
    pub spirit_event: Option<SpiritEvent>,
    /// A pool to draw `n` distinct random [`Grant`]s from at spawn time, on
    /// top of the fixed `grants` list — every spirit's two boons (see
    /// [`MonsterDef::spirit`]). `None` for every ordinary row: a species'
    /// magic is the same magic every time.
    pub random_grants: Option<(&'static [Grant], u8)>,
    /// Born with a pack of floor loot to trade away — the yellow demon's
    /// stock, [`BARTER_STOCK_MIN`] to [`BARTER_STOCK_MAX`] drops. See
    /// [`roll_barter_stock`].
    pub stocks_barter: bool,
}

impl MonsterDef {
    /// The Ma this row is born with: `casts` casts of its dearest spell.
    pub fn magic_pool(&self) -> u8 {
        let dearest = self
            .spells
            .iter()
            .map(|&s| crate::catalog::SpellDef::of(s).cost)
            .max()
            .unwrap_or(0);
        self.casts * dearest
    }

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
            spells: &[],
            casts: 0,
            invisible: false,
            speed: SpeedKind::Normal,
            equip_rolls: &[],
            mimics: false,
            spirit_kind: None,
            spirit_event: None,
            random_grants: None,
            stocks_barter: false,
        }
    }

    /// Stock a bestiary row's pack with loot at spawn (the yellow demon).
    const fn stocks_barter(mut self) -> Self {
        self.stocks_barter = true;
        self
    }

    /// Attach innate magic to a bestiary row.
    const fn grants(mut self, grants: &'static [Grant]) -> Self {
        self.grants = grants;
        self
    }

    /// Teach a bestiary row `spells`, with Ma for `casts` casts of the dearest
    /// (the dragon).
    const fn casts(mut self, casts: u8, spells: &'static [SpellEffect]) -> Self {
        self.casts = casts;
        self.spells = spells;
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

    /// Mark a bestiary row as a spirit ([`Faction::Spirits`] instead of
    /// [`Faction::Monster`]), and give it the melee event a peaceful
    /// interaction fires. Every spirit gets [`SPIRIT_GRANTS`], plus
    /// [`SPIRIT_BOON_COUNT`] drawn from [`SPIRIT_BOONS`]. See
    /// `crate::spirits`.
    const fn spirit(mut self, kind: SpiritKind, event: SpiritEvent) -> Self {
        self.spirit_kind = Some(kind);
        self.spirit_event = Some(event);
        self.grants = SPIRIT_GRANTS;
        self.random_grants = Some((SPIRIT_BOONS, SPIRIT_BOON_COUNT));
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
        BESTIARY.iter().chain(SUMMONS).find(|m| m.name == name)
    }

    /// What the player sees this species called, in whatever language this
    /// binary was built for. `name` itself never changes — it is also the id
    /// `-am`, `NIHILURK_SPAWN` and save files match against (see
    /// `docs/reference/cli-and-env.md`) — so every place that prints a
    /// monster's name for the player to read calls this instead of `.name`.
    pub fn display_name(&self) -> &'static str {
        strings::content_name(self.name)
    }

    /// Picks a species appropriate for `depth`: a weighted draw from every row
    /// the floor has unlocked. This is the only place the dungeon decides what
    /// lives on a floor, so a new creature's rarity and debut are the two
    /// numbers on its row and nothing else.
    pub fn pick(depth: u8, rng: &mut ChaCha12Rng) -> &'static MonsterDef {
        Self::draw(rng, |m| !m.swims() && m.min_depth <= depth.max(1))
    }

    /// A weighted draw from the *whole* bestiary, depth gate and all. Once the
    /// Element of Yoord is in the pack the dungeon stops holding anything back:
    /// the climb out re-populates each floor through here, so a dragon can turn
    /// up on floor 1. See [`crate::map::holding_element_of_yoord`].
    pub fn pick_any(rng: &mut ChaCha12Rng) -> &'static MonsterDef {
        Self::draw(rng, |m| !m.swims())
    }

    /// A weighted draw from the rows that [`Swims`], and only those — what a
    /// water tile gets instead of [`pick`](Self::pick). No depth gate: water
    /// only turns up on floors deep enough for every swimmer. Neither
    /// ordinary draw ever hands a swimmer out, so this is the one door they
    /// come into a floor through.
    pub fn pick_aquatic(rng: &mut ChaCha12Rng) -> &'static MonsterDef {
        Self::draw(rng, MonsterDef::swims)
    }

    /// Whether this species lives in the water: its row grants [`Swims`].
    pub fn swims(&self) -> bool {
        self.grants
            .iter()
            .any(|g| g.effect_id() == Grant::of::<Swims>().effect_id())
    }

    /// The shared body of [`pick`](Self::pick), [`pick_any`](Self::pick_any)
    /// and [`pick_aquatic`](Self::pick_aquatic): a weighted draw from every
    /// bestiary row `eligible` accepts.
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

/// What every spirit is born with: flight, so no spirit springs a trap, and
/// immunity to fire and cold. Set by [`MonsterDef::spirit`].
const SPIRIT_GRANTS: &[Grant] = &[
    Grant::of::<Flies>(),
    Grant::of::<FireImmune>(),
    Grant::of::<ColdImmune>(),
];

/// How many boons every spirit draws from [`SPIRIT_BOONS`] at spawn.
const SPIRIT_BOON_COUNT: u8 = 2;

/// The spirits' boon pool: every passive grant one of Rogue's 26 lettered
/// creatures carries, spells never, and nothing from [`SPIRIT_GRANTS`],
/// which every spirit already has. No gorgon's gaze and no thieving: a
/// spirit you aim at or deal with should never stone you or rob you. Drawn distinct, without replacement, at
/// spawn ([`roll_random_grants`]).
const SPIRIT_BOONS: &[Grant] = &[
    Grant::of::<RustsArmor>(),   // A aquator
    Grant::of::<Batty>(),        // B bat, P phantom
    Grant::of::<ItemUser>(),     // C centaur and the rest with hands
    Grant::of::<Binds>(),        // F venus flytrap, X xeroc
    Grant::of::<Regenerates>(),  // G griffin, T troll
    Grant::of::<Freezing>(),     // I ice monster
    Grant::of::<VorpalTarget>(), // J jabberwock
    Grant::of::<Undead>(),       // P phantom, W wraith, Z zombie
    Grant::of::<Venomous>(),     // R rattlesnake
    Grant::of::<Splits>(),       // S slime
    Grant::of::<GreenBlood>(),   // S slime
    Grant::of::<Vampiric>(),     // V vampire
];

/// How often a bestiary row that carries [`EquipRoll`]s rolls each one, and what
/// it reaches for when it hits. Rolled independently at spawn by
/// [`roll_spawn_gear`] — a hobgoblin's three rolls (weapon, armour, ring) can
/// land none, one, two or all three.
#[derive(Clone, Copy)]
pub struct EquipRoll {
    /// The probability the roll succeeds, as a fraction.
    pub chance: f64,
    /// What it reaches for when it does.
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

/// What a dog is, one grant per behaviour: any treat tames it, a charm or a
/// conjuring makes it the Helper, it outranks the ordinary Helper, a kill
/// sometimes turns it into something else, and its death reveals a faerie
/// shapeshifter. Nothing cancels them. A creature that holds any of them
/// keeps them through a change of shape ([`reshape`]).
pub const DOG_GRANTS: &[Grant] = &[
    Grant::of::<AlwaysTamed>(),
    Grant::of::<AlwaysHelper>(),
    Grant::of::<PriorityHelper>(),
    Grant::of::<ShapeshiftOnKill>(),
    Grant::of::<FaerieOnDeath>(),
];

/// The whole bestiary: Rogue's 26 lettered creatures and the few nihilurk
/// added, in one table. Effects that pick a
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
    // A rattlesnake's numbers verbatim, plus a bounty: the guardian a
    // treasure hive forces in (`SpecialRoom::TreasureHive`), fast where the
    // snake is not.
    MonsterDef::row("apis", 'a', Color::Yellow, Chase, 6, 6, 0, 8, 0, 5)
        .grants(&[Grant::of::<Venomous>(), Grant::of::<ScoreBounty>()])
        .fast(),
    MonsterDef::row("aquator", 'A', Color::Blue, Chase, 9, 4, -1, 8, 1, 5)
        .grants(&[Grant::of::<RustsArmor>()]),
    MonsterDef::row("foxbat", 'B', Color::DarkGrey, Chase, 6, 8, 0, 8, 0, 5)
        .grants(&[Grant::of::<Batty>()]),
    MonsterDef::row("centaur", 'C', Color::DarkYellow, Chase, 9, 8, 0, 6, 1, 5)
        .grants(ITEM_USER)
        .equip(&[EquipRoll {
            chance: ULTIMATE_GEAR_CHANCE,
            kind: EquipKind::Bow,
        }]),
    // The centaur's own numbers and wits, in the water.
    MonsterDef::row("ichthyocentaur", 'C', Color::Cyan, Chase, 9, 8, 0, 6, 1, 5)
        .grants(&[Grant::of::<ItemUser>(), Grant::of::<Swims>()])
        .equip(&[EquipRoll {
            chance: ULTIMATE_GEAR_CHANCE,
            kind: EquipKind::Bow,
        }]),
    // Rare, and the dragon's own wits (it chases) with none of its fire. What
    // makes it a dog is `DOG_GRANTS`, four separate behaviours.
    MonsterDef::row("dog", 'd', Color::DarkYellow, Chase, 8, 6, 0, 7, 0, 3)
        .grants(DOG_GRANTS)
        .weight(2),
    MonsterDef::row("dragon", 'D', Color::Red, Chase, 13, 12, 2, 10, 2, 10)
        .grants(&[Grant::of::<FireImmune>(), Grant::of::<Flies>()])
        .casts(2, &[SpellEffect::DragonBreath]),
    MonsterDef::row("emu", 'E', Color::DarkGreen, Chase, 6, 4, 0, 4, 1, 1),
    MonsterDef::row("eel", 'e', Color::Cyan, Chase, 6, 6, 0, 6, 0, 6)
        .grants(&[Grant::of::<Swims>()])
        .casts(3, &[SpellEffect::Thunderbolt]),
    MonsterDef::row(
        "venus flytrap",
        'f',
        Color::Green,
        Ambush,
        9,
        10,
        0,
        8,
        0,
        5,
    )
    .grants(&[Grant::of::<Binds>()]),
    MonsterDef::row(
        "griffin",
        'G',
        Color::DarkYellow,
        Chase,
        13,
        12,
        1,
        8,
        1,
        10,
    )
    .grants(&[Grant::of::<Flies>(), Grant::of::<Regenerates>()]),
    MonsterDef::row("hobgoblin", 'h', Color::DarkRed, Chase, 6, 4, 0, 6, 0, 1)
        .grants(ITEM_USER)
        .equip(&[
            EquipRoll {
                chance: NORMAL_GEAR_CHANCE,
                kind: EquipKind::Weapon,
            },
            EquipRoll {
                chance: HIGH_GEAR_CHANCE,
                kind: EquipKind::Armor,
            },
            EquipRoll {
                chance: NORMAL_GEAR_CHANCE,
                kind: EquipKind::Ring,
            },
        ]),
    MonsterDef::row("ice monster", 'I', Color::Cyan, Ambush, 3, 4, 0, 4, -1, 1)
        .grants(&[Grant::of::<Freezing>()]),
    MonsterDef::row("jabberwock", 'J', Color::Magenta, Chase, 13, 8, 5, 6, 0, 10)
        .grants(&[Grant::of::<VorpalTarget>(), Grant::of::<Flies>()]),
    MonsterDef::row("kestral", 'K', Color::Grey, Chase, 3, 4, 0, 4, 1, 1)
        .grants(&[Grant::of::<Flies>()]),
    MonsterDef::row("leprechaun", 'L', Color::Green, Chase, 3, 4, 0, 4, 0, 5)
        .grants(&[Grant::of::<ItemUser>(), Grant::of::<StealsAndFlees>()]),
    MonsterDef::row("medusa", 'M', Color::DarkGreen, Chase, 9, 5, 0, 8, 1, 5)
        .grants(&[Grant::of::<ItemUser>(), Grant::of::<Gorgon>()])
        .equip(&[EquipRoll {
            chance: NORMAL_GEAR_CHANCE,
            kind: EquipKind::Bow,
        }]),
    MonsterDef::row("nymph", 'N', Color::Magenta, Chase, 3, 4, -1, 4, -1, 5)
        .grants(&[Grant::of::<ItemUser>(), Grant::of::<StealsAndVanishes>()]),
    MonsterDef::row("orc", 'o', Color::Red, Chase, 4, 6, 0, 4, 0, 1)
        .grants(ITEM_USER)
        .equip(&[
            EquipRoll {
                chance: HIGH_GEAR_CHANCE,
                kind: EquipKind::Weapon,
            },
            EquipRoll {
                chance: NORMAL_GEAR_CHANCE,
                kind: EquipKind::Armor,
            },
            EquipRoll {
                chance: NORMAL_GEAR_CHANCE,
                kind: EquipKind::Ring,
            },
        ]),
    MonsterDef::row("phantom", 'P', Color::DarkGrey, Chase, 9, 10, 0, 8, 0, 5)
        .grants(&[Grant::of::<Undead>(), Grant::of::<Batty>()])
        .invisible(),
    MonsterDef::row("quagga", 'Q', Color::DarkYellow, Chase, 6, 6, 0, 12, 4, 5),
    MonsterDef::row(
        "rattlesnake",
        'R',
        Color::DarkGreen,
        Chase,
        6,
        6,
        0,
        8,
        0,
        5,
    )
    .grants(&[Grant::of::<Venomous>()]),
    MonsterDef::row("slime", 'S', Color::DarkGreen, Chase, 6, 4, 0, 4, 0, 5)
        .grants(&[Grant::of::<Splits>(), Grant::of::<GreenBlood>()]),
    MonsterDef::row("troll", 'T', Color::DarkGreen, Chase, 8, 10, 0, 6, 1, 5)
        .grants(&[Grant::of::<ItemUser>(), Grant::of::<Regenerates>()]),
    MonsterDef::row(
        "ur-vile",
        'U',
        Color::DarkMagenta,
        Chase,
        10,
        10,
        0,
        12,
        1,
        5,
    )
    .grants(ITEM_USER),
    MonsterDef::row("vampire", 'V', Color::DarkRed, Chase, 10, 10, 0, 8, 1, 5)
        .grants(&[Grant::of::<Vampiric>(), Grant::of::<ItemUser>()]),
    MonsterDef::row("wraith", 'W', Color::DarkGrey, Chase, 9, 6, 0, 6, 1, 5)
        .grants(&[Grant::of::<Undead>(), Grant::of::<ItemUser>()])
        .fast()
        .equip(&[
            EquipRoll {
                chance: HIGH_GEAR_CHANCE,
                kind: EquipKind::Weapon,
            },
            EquipRoll {
                chance: NORMAL_GEAR_CHANCE,
                kind: EquipKind::Armor,
            },
            EquipRoll {
                chance: ULTIMATE_GEAR_CHANCE,
                kind: EquipKind::Ring,
            },
        ]),
    MonsterDef::row("xeroc", 'X', Color::Yellow, Ambush, 6, 8, 0, 4, 1, 5)
        .grants(&[Grant::of::<Binds>()])
        .mimics(),
    MonsterDef::row("yeti", 'Y', Color::White, Chase, 9, 8, 0, 6, 0, 5)
        .grants(&[Grant::of::<ColdImmune>()]),
    MonsterDef::row("zombie", 'Z', Color::DarkGrey, Chase, 8, 8, 0, 4, 0, 5)
        .grants(&[Grant::of::<Undead>()]),
    // Spirits (gdd.md "Spirits!"). An eighth of the ordinary spawn weight,
    // always alone. Peaceful (`Confused`, a random wander) until
    // `SpiritsHostile` sends them all to `Chase` — see `crate::spirits`.
    // HP/power/armor 13/13/13, the pink demon 7/7/7.
    MonsterDef::row(
        "yellow demon",
        '&',
        Color::Yellow,
        Confused,
        13,
        13,
        0,
        13,
        0,
        1,
    )
    .weight(SPIRIT_WEIGHT)
    .spirit(SpiritKind::Cacodaemon, SpiritEvent(yellow_demon_event))
    .stocks_barter(),
    MonsterDef::row("red demon", '&', Color::Red, Confused, 13, 13, 0, 13, 0, 1)
        .weight(SPIRIT_WEIGHT)
        .spirit(SpiritKind::Cacodaemon, SpiritEvent(red_demon_event))
        .equip(&[EquipRoll {
            chance: ULTIMATE_GEAR_CHANCE,
            kind: EquipKind::Weapon,
        }]),
    MonsterDef::row(
        "blue demon",
        '&',
        Color::Cyan,
        Confused,
        13,
        13,
        0,
        13,
        0,
        1,
    )
    .weight(SPIRIT_WEIGHT)
    .spirit(SpiritKind::Cacodaemon, SpiritEvent(blue_demon_event)),
    MonsterDef::row(
        "pink demon",
        '&',
        Color::Magenta,
        Confused,
        7,
        7,
        0,
        7,
        0,
        1,
    )
    .weight(SPIRIT_WEIGHT)
    .spirit(SpiritKind::Cacodaemon, SpiritEvent(pink_demon_event)),
    MonsterDef::row("angel", '&', Color::White, Confused, 13, 13, 0, 13, 0, 1)
        .weight(SPIRIT_WEIGHT)
        .spirit(SpiritKind::Eudaemon, SpiritEvent(angel_event)),
    MonsterDef::row(
        "sphynx",
        '&',
        Color::DarkYellow,
        Confused,
        13,
        13,
        0,
        13,
        0,
        1,
    )
    .weight(SPIRIT_WEIGHT)
    .spirit(SpiritKind::Eudaemon, SpiritEvent(sphynx_event)),
    MonsterDef::row(
        "sylphid",
        '&',
        Color::DarkBlue,
        Confused,
        13,
        13,
        0,
        13,
        0,
        1,
    )
    .weight(SPIRIT_WEIGHT)
    .spirit(SpiritKind::Eudaemon, SpiritEvent(sylphid_event)),
    MonsterDef::row(
        "salamander",
        '&',
        Color::DarkRed,
        Confused,
        13,
        13,
        0,
        13,
        0,
        1,
    )
    .weight(SPIRIT_WEIGHT)
    .spirit(SpiritKind::Eudaemon, SpiritEvent(salamander_event)),
    MonsterDef::row(
        "undyne",
        '&',
        Color::DarkMagenta,
        Confused,
        13,
        13,
        0,
        13,
        0,
        1,
    )
    .weight(SPIRIT_WEIGHT)
    .spirit(SpiritKind::Eudaemon, SpiritEvent(undyne_event)),
    MonsterDef::row("gnome", '&', Color::Grey, Confused, 13, 13, 0, 13, 0, 1)
        .weight(SPIRIT_WEIGHT)
        .spirit(SpiritKind::Eudaemon, SpiritEvent(gnome_event)),
];

/// The creatures only a deck of cards brings: THE SKULL KING! and THE BLACK
/// MAGE. Not in [`BESTIARY`], so no floor, conjuring or polymorph ever rolls
/// one; [`MonsterDef::lookup`] still finds them, so a save, `NIHILURK_SPAWN`
/// and `-am` do. The deck reads them by position: the Skull King first.
#[rustfmt::skip]
pub const SUMMONS: &[MonsterDef] = &[
    //              name          glyph  colour          move   hp  pow  pb  ar  ab  dep
    MonsterDef::row("skull king", 'S', Color::Magenta, Chase, 12,  6,  0,  6,  0,  1)
        .casts(CARD_CASTER_CASTS, &[SpellEffect::Thunderbolt]),
    MonsterDef::row("black mage", '&', Color::Magenta, Chase, 12,  5,  0,  8,  0,  1)
        .casts(CARD_CASTER_CASTS, &[SpellEffect::ForceLance]),
];

/// A bones ghost — a past character come back angry (see `crate::bones` and
/// `crate::map::levels::spawn_bones_ghost`). Deliberately *not* in
/// [`BESTIARY`]: it never turns up in the ordinary weighted population roll,
/// only ever spawned by name. Its glyph is a blank space, the same trick
/// NetHack's own ghost uses, and its stats are nihil's own starting numbers —
/// fixed and the same for every ghost, whoever they used to be.
pub(crate) const GHOST: MonsterDef = MonsterDef::row(
    "ghost",
    ' ',
    Color::DarkGrey,
    Chase,
    crate::constants::player::START_HP,
    crate::constants::player::START_POWER,
    0,
    crate::constants::player::START_ARMOR,
    0,
    1,
)
.grants(&[Grant::of::<Phasing>()]);

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
                what: def.display_name().to_string(),
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
            faction: match def.spirit_kind {
                Some(_) => Faction::Spirits,
                None => Faction::Monster,
            },
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
    if !def.spells.is_empty() {
        world.entity_mut(e).insert((
            Magic {
                points: def.magic_pool(),
                max_points: def.magic_pool(),
            },
            Spellset {
                slots: def.spells.to_vec(),
            },
        ));
    }
    if let Some(kind) = def.spirit_kind {
        world.entity_mut(e).insert(kind);
    }
    if let Some(event) = def.spirit_event {
        world.entity_mut(e).insert(event);
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
    roll_random_grants(world, e, def, &mut rng);
    roll_barter_stock(world, e, def, &mut rng);
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
    roll_random_grants(world, e, def, rng);
    roll_barter_stock(world, e, def, rng);
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
/// [`MonsterDef::random_grants`] paying off: `n` distinct grants off `pool`,
/// lent the same way [`MonsterDef::grants`]'s fixed list is
/// ([`grant_all`]). A no-op for every row that doesn't ask for one.
fn roll_random_grants(world: &mut World, mob: Entity, def: &MonsterDef, rng: &mut ChaCha12Rng) {
    let Some((pool, n)) = def.random_grants else {
        return;
    };
    for &grant in pool.choose_multiple(rng, n as usize) {
        crate::effects::lend(world, mob, grant, crate::effects::Lifetime::Permanent);
    }
}

/// [`MonsterDef::stocks_barter`] paying off: floor drops for this depth
/// ([`roll_item_except`], coins struck out), tucked straight into a fresh [`Backpack`] one entity
/// each — never merged, so the count is the count. A no-op for every row that
/// doesn't ask for one.
fn roll_barter_stock(world: &mut World, mob: Entity, def: &MonsterDef, rng: &mut ChaCha12Rng) {
    if !def.stocks_barter {
        return;
    }
    let Some(pos) = world.get::<Position>(mob).copied() else {
        return;
    };
    let depth = world.get_resource::<Depth>().map_or(1, |d| d.what);
    let mut items = Vec::new();
    for _ in 0..rng.gen_range(BARTER_STOCK_MIN..=BARTER_STOCK_MAX) {
        let item = roll_item_except(world, rng, depth, pos, &["coin"]);
        world.entity_mut(item).remove::<Position>();
        items.push(item);
    }
    world.entity_mut(mob).insert(Backpack { items });
}

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
/// tile, announces it: `"They are wielding a long sword."` / `"They are
/// wearing a suit of banded mail."`
///
/// Every creature in the dungeon is a they. The game does not know what lives
/// down there and has no business guessing.
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
        .add(strings::wearing_gear(verb, &name));
}

// ---------------------------------------------------------------------------
// The xeroc's disguise
// ---------------------------------------------------------------------------

/// The plain-item look-alikes a xeroc can wear on an ordinary floor: a name,
/// a glyph and a colour, off the same appearances the real things spawn with.
const MIMIC_LOOKS: &[(&str, char, Color)] = &[
    (strings::mimic_look_scroll(), '?', Color::White),
    (strings::mimic_look_potion(), '!', Color::Magenta),
    (strings::mimic_look_wand(), '/', Color::Yellow),
    (strings::mimic_look_gold_coin(), '$', Color::Yellow),
    (strings::mimic_look_ring(), '=', Color::Yellow),
    (strings::mimic_look_suit_of_armor(), ']', Color::Grey),
    (strings::mimic_look_weapon(), ')', Color::Grey),
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
    let endless = world.get_resource::<Endless>().is_some_and(|e| e.enabled);
    let (name, glyph, color): (&str, char, Color) = if depth >= FINAL_DEPTH && !endless {
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
            n.what = def.display_name().to_string();
        }
        if let Some(mut r) = world.get_mut::<Renderable>(xeroc) {
            r.glyph = def.glyph;
            r.color = def.color;
        }
        world.entity_mut(xeroc).remove::<Mimic>();
        world.entity_mut(xeroc).remove::<Spotted>();
        world
            .resource_mut::<GameLog>()
            .add(strings::xeroc_disguise_falls());
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
        if let Some(mut fx) = world.get_resource_mut::<Particles>() {
            let mut cells = Vec::new();
            for dy in -1i32..=1 {
                for dx in -1i32..=1 {
                    let dist = ((dx * dx + dy * dy) as f32).sqrt();
                    if let Some((x, y)) = on_map(pos.x as i32 + dx, pos.y as i32 + dy) {
                        cells.push((x, y, dist));
                    }
                }
            }
            fx.explosion(&cells, BlastPalette::Warp);
        }
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

// ---------------------------------------------------------------------------
// Changing shape
// ---------------------------------------------------------------------------

/// Replaces `victim` with a fresh `def` on its tile, and carries over what a
/// change of shape keeps: any of [`DOG_GRANTS`] it held, and its side — an
/// ally stays an ally, a Helper stays the Helper.
pub(crate) fn reshape(world: &mut World, victim: Entity, def: &MonsterDef) -> Entity {
    let pos = world.get::<Position>(victim).copied();
    let kept: Vec<Grant> = DOG_GRANTS
        .iter()
        .copied()
        .filter(|g| g.probe(world, victim))
        .collect();
    let helper = world.get::<Helper>(victim).is_some();
    let ally = world.get::<Faction>(victim) == Some(&Faction::Ally);
    crate::spirits::poof(world, victim);

    let new = spawn_monster(world, def, pos.unwrap_or(Position { x: 0, y: 0 }));
    for grant in kept {
        lend(world, new, grant, Lifetime::Permanent);
    }
    if ally {
        crate::companion::stand_with_the_player(world, new, helper);
    }
    new
}

/// `who` becomes another random monster (never a spirit, never its own
/// species) and says so: "It was never a dog, but a dragon!". `None` when it
/// has no tile to change on.
pub fn shapeshift(world: &mut World, who: Entity) -> Option<Entity> {
    world.get::<Mob>(who)?;
    let pos = world.get::<Position>(who).copied()?;
    let old = crate::helpers::item_label(world, who);
    let pool: Vec<&MonsterDef> = BESTIARY
        .iter()
        .filter(|m| m.spirit_kind.is_none() && m.display_name() != old)
        .collect();
    let idx = world.resource_mut::<GameRng>().0.gen_range(0..pool.len());
    let def = pool[idx];
    let new = def.display_name().to_string();
    let grown = reshape(world, who, def);
    world
        .resource_mut::<GameLog>()
        .add(strings::shapeshift_reveal(
            crate::identify::article_for(&old),
            &old,
            crate::identify::article_for(&new),
            &new,
        ));
    crate::items::leave_smoke_ring(world, pos);
    Some(grown)
}

/// What `killer` just did, if it can shapeshift: a [`SHAPESHIFT_CHANCE`] roll
/// on [`ShapeshiftOnKill`]. Called by the melee kill funnel in `crate::combat`.
pub(crate) fn maybe_shapeshift(world: &mut World, killer: Entity) {
    if world.get::<ShapeshiftOnKill>(killer).is_some()
        && world
            .resource_mut::<GameRng>()
            .0
            .gen_bool(crate::constants::helpers::SHAPESHIFT_CHANCE)
    {
        shapeshift(world, killer);
    }
}

// ---------------------------------------------------------------------------
// Playing as a monster
// ---------------------------------------------------------------------------

/// Puts `def`'s body on `player`: its stats, its glyph, its tempo, the magic
/// it was born with, and whatever of that magic is a spell.
///
/// The player *entity* survives — [`Player`], [`Faction::Player`],
/// [`Viewshed`], [`Backpack`], [`Score`], [`Magic`] and [`Spellset`] are what
/// make them the one the game is about, and none of them are things a species
/// has an opinion on. What changes is everything [`MonsterBundle::from_def`]
/// would have set, minus the two fields that would hand the player to the
/// enemy: [`Mob`] (which is what `ai` steers) and [`Faction::Monster`].
///
/// Not a re-spawn: a species is a *costume*, and swapping the entity out
/// would drop the run — the score, the map already seen, the pack — on the
/// floor with it.
pub fn wear_monster(world: &mut World, player: Entity, def: &'static MonsterDef) {
    let mut e = world.entity_mut(player);
    e.insert((
        crate::body::MonsterBody(def),
        Name {
            what: def.display_name().to_string(),
        },
        Renderable {
            glyph: def.glyph,
            color: def.color,
        },
        Fighter {
            hp: def.hp,
            max_hp: def.hp,
            power: def.power,
            max_power: def.power,
            power_bonus: def.power_bonus,
            armor: def.armor,
            armor_bonus: def.armor_bonus,
        },
        Speed::new(def.speed),
        Grants(def.grants),
    ));
    if def.invisible {
        e.insert(Invisible);
    }
    grant_all(world, player, def.grants);

    if let Some(mut spellset) = world.get_mut::<Spellset>(player) {
        spellset.slots.extend(def.spells);
    }

    if let Some(GameRng(mut rng)) = world.remove_resource::<GameRng>() {
        roll_random_grants(world, player, def, &mut rng);
        world.insert_resource(GameRng(rng));
    }

    if def.name == "apis"
        && let Some(mut log) = world.get_resource_mut::<GameLog>()
    {
        log.add(strings::you_monster());
    }
    feel_what_you_were_born_with(world, player);
}

/// One "you feel" line per grant `player` was born with, in the order they
/// were lent: what a worn body is, said once at the start of the run.
fn feel_what_you_were_born_with(world: &mut World, player: Entity) {
    let feels: Vec<&'static str> = world
        .get::<crate::effects::Effects>(player)
        .into_iter()
        .flat_map(|e| e.0.iter())
        .filter_map(|h| crate::effects::Effect::by_id(h.id).and_then(|e| e.feel))
        .collect();
    if let Some(mut log) = world.get_resource_mut::<GameLog>() {
        for line in feels {
            log.add(line);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_ghost_phases() {
        let mut w = World::new();
        let ghost = spawn_monster(&mut w, &GHOST, Position { x: 1, y: 1 });
        let phases = Grant::of::<Phasing>().probe(&w, ghost);
        assert!(phases);
    }
}
