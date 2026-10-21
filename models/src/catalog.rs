//! Every item in the game, one row each.
//!
//! This is the file you edit to add content. A row says what a thing is called,
//! how it draws, and which components it carries into the world — and that is
//! the whole of it. Nothing else in the codebase enumerates items: the loot
//! table rolls a category and picks a row ([`crate::map`]), and combat reads
//! the components the rows attached ([`crate::effects`]). Potions, scrolls,
//! wands and rings are always shown by their true name — there is no cosmetic
//! appearance or per-effect identification to keep in step with this table.
//! Only equipment (`crate::identify::KnownQuality`) hides anything: whether
//! its enchantment bonus and curse are known yet.
//!
//! Bows are the newest case of it. A bow is not a weapon with a special "fires
//! arrows" mode; it is an item that [`Grants`] [`FireArrow`], and an arrow is an
//! item that says it answers to [`FireArrow`] ([`AmmoDef::launched_by`]). Neither knows the
//! other exists, and a sling is one row in each table away.
//!
//! Rings are the clearest case of the design. A ring of protection is not an
//! enum variant that seven files have to recognise; it is an item
//! carrying [`ArmorBonus`]`(2)`, which combat already folds in for plate mail.
//! A ring of perception is an item that [`Grants`] [`SeesInvisible`] to whoever
//! wears it, which the visibility system already asks about. Adding "ring of
//! fire resistance" is a row here — and no behaviour code at all, anywhere.
//!
//! No row has a name of its own in Rust. Code that wants one particular row
//! reaches it through an `index_of!` const, `POTIONS[HEALING]`, which the
//! compiler works out from the row's name; a save stores the name itself.
//! A row that must never drop sits in its table at weight 0.
//!
//! The full recipes, per category, are in `docs/how-to/add-an-item.md`.

use crate::Color;
use bevy_ecs::prelude::*;
use rand::Rng;
use rand_chacha::ChaCha12Rng;

use crate::components::*;
use crate::conditions::{blind, hasten, paralyse};
use crate::effects::*;
use crate::equipment::{Equipped, Slot};
use crate::items::wands::dazzle;
use crate::items::{decks, pickups, potions, runes, scrolls, spells, throwing, wands};
use crate::monsters::{BESTIARY, SUMMONS};
use crate::particles::BlastPalette;

// --- Tuning constants -----------------------------------------------------
// The numbers a *drop* rolls that are not part of any one row: wand battery
// size, ammunition bundle size, and the enchantment odds / bonus ranges.
// All defined and documented in `constants.rs`.
use crate::constants::items::ESTOC_NUMBER_OF_ATTACKS;
use crate::constants::loot::{
    AMMO_BUNDLE_MAX, AMMO_BUNDLE_MIN, CURSED_BONUS_MAX, CURSED_BONUS_MIN, EXCEPTIONAL_BONUS_MAX,
    EXCEPTIONAL_BONUS_MIN, EXCEPTIONAL_QUALITY_PCT, IMPROVISED_THROW_DIE, MEGACURSED_BONUS_MAX,
    MEGACURSED_BONUS_MIN, MEGACURSED_PCT_OF_CURSED, NORMAL_QUALITY_PCT,
};
use crate::constants::rings::{
    CURSED_BONUS_MAX as RING_CURSED_BONUS_MAX, CURSED_BONUS_MIN as RING_CURSED_BONUS_MIN,
    EXCEPTIONAL_BONUS as RING_EXCEPTIONAL_BONUS, MEGACURSED_BONUS_MAX as RING_MEGACURSED_BONUS_MAX,
    MEGACURSED_BONUS_MIN as RING_MEGACURSED_BONUS_MIN, PLAIN_BONUS,
};
use crate::constants::spells::METEOR_STRIKE_CHAIN_SPREAD;
use crate::constants::wands::{BLAST_RADIUS, GRENADE_RADIUS, WAND_CHARGES};

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

    /// What the player sees this row called, in whatever language this binary
    /// was built for. [`ItemDef::name`] itself never changes meaning — see its
    /// own doc comment — so every item type gets this the same way, from the
    /// same id, rather than each redefining it.
    fn display_name(&self) -> &'static str {
        strings::content_name(self.name())
    }

    /// How often this row turns up relative to its table-mates. The default is
    /// the baseline, so a row at half of it is half as common and one at double
    /// it twice.
    ///
    /// Most rows sit at the default — within a category nihilurk picks evenly,
    /// on purpose. Overriding it is how a category earns per-row rarity: coins
    /// carry a `weight` field, scrolls match on their effect.
    fn weight(&self) -> u32 {
        crate::constants::monsters::DEFAULT_SPAWN_WEIGHT
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

/// The index of the row of `$table` called `$name`, worked out while compiling.
/// Code reaches a row through one of these, never a bare number: a name no row
/// has stops the build, and a row moved within its table keeps its callers.
///
/// ```ignore
/// const HEALING: usize = index_of!(POTIONS, "potion of healing");
/// POTIONS[HEALING].spawn(world, pos);
/// ```
macro_rules! index_of {
    ($table:expr, $name:literal) => {{
        let table = $table;
        let mut i = 0;
        loop {
            if i == table.len() {
                panic!(concat!("no row named ", $name));
            }
            if $crate::catalog::same_name(table[i].name, $name) {
                break i;
            }
            i += 1;
        }
    }};
}
pub(crate) use index_of;

/// Whether two row names are the same, in a form [`index_of`] can run while
/// compiling.
pub(crate) const fn same_name(a: &str, b: &str) -> bool {
    let (a, b) = (a.as_bytes(), b.as_bytes());
    if a.len() != b.len() {
        return false;
    }
    let mut i = 0;
    while i < a.len() {
        if a[i] != b[i] {
            return false;
        }
        i += 1;
    }
    true
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

/// A potion: quaffed once, then gone. The row is the potion: a [`Potion`]
/// points at its row, and drinking it calls the row's `apply`. The handlers
/// live in the `potions` submodule of `crate::items`.
pub struct PotionDef {
    /// The row's identity: the save file stores it and
    /// [`crate::spawn::spawn_named`] finds the row by it. The player reads
    /// [`ItemDef::display_name`] instead.
    pub name: &'static str,
    /// The colour it draws in.
    pub color: Color,
    /// What a dose does to whoever takes it, drunk or splashed. Returns
    /// whether it visibly took hold: a potion thrown at a monster gives itself
    /// away only then.
    pub apply: fn(&mut World, Entity) -> bool,
    /// How often the dungeon drops it against the other potions.
    pub weight: u32,
}

impl PotionDef {
    /// A potion that does `apply`, dropped as often as most.
    pub const fn new(
        name: &'static str,
        color: Color,
        apply: fn(&mut World, Entity) -> bool,
    ) -> Self {
        Self {
            name,
            color,
            apply,
            weight: crate::constants::monsters::DEFAULT_SPAWN_WEIGHT,
        }
    }

    /// Never dropped: only something else turns a potion into this one.
    const fn never_drops(mut self) -> Self {
        self.weight = 0;
        self
    }
}

impl ItemDef for &'static PotionDef {
    fn name(&self) -> &'static str {
        self.name
    }

    fn weight(&self) -> u32 {
        self.weight
    }

    fn spawn(&self, world: &mut World, pos: Position) -> Entity {
        world
            .spawn((
                Name {
                    what: strings::content_name(self.name).to_string(),
                },
                Renderable {
                    glyph: '!',
                    color: self.color,
                },
                pos,
                Item,
                Potion { row: self },
                Consume,
            ))
            .id()
    }
}

/// Every potion in the game, one row each.
#[rustfmt::skip]
pub static POTIONS: &[&PotionDef] = &[
    &PotionDef::new("potion of confusion",         Color::Magenta,     potions::confusion),
    &PotionDef::new("potion of paralysis",         Color::DarkGrey,    paralyse),
    &PotionDef::new("potion of poison",            Color::Green,       potions::poison),
    &PotionDef::new("potion of gain strength",     Color::Red,         potions::gain_strength),
    &PotionDef::new("potion of see invisible",     Color::Cyan,        potions::see_invisible),
    &PotionDef::new("potion of healing",           Color::Red,         potions::healing),
    &PotionDef::new("potion of monster detection", Color::Yellow,      potions::detect_monsters),
    &PotionDef::new("potion of magic detection",   Color::Yellow,      potions::detect_magic),
    &PotionDef::new("potion of raise level",       Color::White,       potions::raise_level),
    &PotionDef::new("potion of extra healing",     Color::Red,         potions::extra_healing),
    &PotionDef::new("potion of haste self",        Color::DarkYellow,  hasten),
    &PotionDef::new("potion of restore strength",  Color::Red,         potions::restore_strength),
    &PotionDef::new("potion of blindness",         Color::DarkGrey,    blind),
    &PotionDef::new("potion of fruit juice",       Color::DarkYellow,  potions::fruit_juice),
    &PotionDef::new("potion of magic",             Color::Blue,        potions::gain_magic),
    &PotionDef::new("potion of adjustment",        Color::DarkCyan,    potions::adjustment),
    &PotionDef::new("potion of polymorph",         Color::DarkMagenta, potions::polymorph),
    &PotionDef::new("potion of thirst quenching",  Color::Blue,        potions::water).never_drops(),
];

/// The potion row called `name`.
pub fn potion_row(name: &str) -> Option<&'static PotionDef> {
    POTIONS.iter().copied().find(|row| row.name == name)
}

// ---------------------------------------------------------------------------
// Scrolls
// ---------------------------------------------------------------------------

/// A scroll: read once, then it crumbles. The row is the scroll: a [`Scroll`]
/// points at its row, and reading it calls the row's `apply`. The handlers
/// live in the `scrolls` submodule of `crate::items`.
pub struct ScrollDef {
    /// The row's identity: the save file stores it and
    /// [`crate::spawn::spawn_named`] finds the row by it. The player reads
    /// [`ItemDef::display_name`] instead.
    pub name: &'static str,
    /// What reading it does to its `reader`, the player or a monster it was
    /// thrown at.
    pub apply: fn(&mut World, Entity),
    /// How often the dungeon drops it against the other scrolls.
    pub weight: u32,
}

impl ScrollDef {
    /// A scroll that reads through `apply`, dropped as often as most.
    pub const fn new(name: &'static str, apply: fn(&mut World, Entity)) -> Self {
        Self {
            name,
            apply,
            weight: crate::constants::monsters::DEFAULT_SPAWN_WEIGHT,
        }
    }

    /// Dropped twice as often as most.
    const fn common(mut self) -> Self {
        self.weight *= 2;
        self
    }

    /// Never dropped: only something else turns a scroll into this one.
    const fn never_drops(mut self) -> Self {
        self.weight = 0;
        self
    }
}

impl ItemDef for &'static ScrollDef {
    fn name(&self) -> &'static str {
        self.name
    }

    fn weight(&self) -> u32 {
        self.weight
    }

    fn spawn(&self, world: &mut World, pos: Position) -> Entity {
        world
            .spawn((
                Name {
                    what: strings::content_name(self.name).to_string(),
                },
                Renderable {
                    glyph: '?',
                    color: Color::White,
                },
                pos,
                Item,
                Scroll { row: self },
                Consume,
            ))
            .id()
    }
}

/// Every scroll in the game, one row each. Identify and remove curse are the
/// two every run needs, so they are `.common()`.
#[rustfmt::skip]
pub static SCROLLS: &[&ScrollDef] = &[
    &ScrollDef::new("scroll of monster confusion",  scrolls::charm_hands),
    &ScrollDef::new("scroll of magic mapping",      scrolls::map_the_floor),
    &ScrollDef::new("scroll of hold monster",       scrolls::hold_in_view),
    &ScrollDef::new("scroll of sleep",              scrolls::read_sleep),
    &ScrollDef::new("scroll of enchant armor",      scrolls::enchant_armor),
    &ScrollDef::new("scroll of identify",           scrolls::identify_everything_hidden).common(),
    &ScrollDef::new("scroll of scare monster",      scrolls::scare_monster),
    &ScrollDef::new("scroll of food detection",     scrolls::detect_mundane_items),
    &ScrollDef::new("scroll of teleportation",      scrolls::teleport_reader),
    &ScrollDef::new("scroll of enchant weapon",     scrolls::enchant_weapon),
    &ScrollDef::new("scroll of create monster",     scrolls::create_monster),
    &ScrollDef::new("scroll of remove curse",       scrolls::remove_curse).common(),
    &ScrollDef::new("scroll of aggravate monsters", scrolls::aggravate_floor),
    &ScrollDef::new("scroll of vorpalize weapon",   scrolls::vorpalize_wielded_weapon),
    &ScrollDef::new("scroll of amnesia",            scrolls::read_amnesia),
    &ScrollDef::new("scroll of charming",           scrolls::charming),
    &ScrollDef::new("scroll of pitfall",            scrolls::pitfall),
    &ScrollDef::new("scroll of atonement",          scrolls::atonement),
    &ScrollDef::new("scroll of teleport to stairs", scrolls::teleport_to_stairs).never_drops(),
    &ScrollDef::new("scroll of blank paper",        scrolls::blank_paper).never_drops(),
];

/// The scroll row called `name`.
pub fn scroll_row(name: &str) -> Option<&'static ScrollDef> {
    SCROLLS.iter().copied().find(|row| row.name == name)
}

// ---------------------------------------------------------------------------
// Decks of cards
// ---------------------------------------------------------------------------

/// One card: the name the log calls it, and what drawing it does.
pub struct CardDef {
    /// The row's identity: the save file stores it, and the log calls it this
    /// through [`strings::content_name`].
    pub name: &'static str,
    /// What the card does when it plays, either way up: `card` is the card
    /// itself, so `card.reversed` says which way it lay. `deal` is the draw in
    /// progress: who plays it, and the deck it came from.
    pub play: fn(&mut World, &mut decks::Deal, Card),
}

impl PartialEq for CardDef {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name
    }
}

impl Eq for CardDef {}

impl std::fmt::Debug for CardDef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.name)
    }
}

/// Every card a deck can hold, one row each. A rolled deck draws from these
/// evenly, with replacement.
#[rustfmt::skip]
pub static CARDS: &[&CardDef] = &[
    &CardDef { name: "The Joker",             play: decks::joker },
    &CardDef { name: "King of Clubs",         play: decks::king_of_clubs },
    &CardDef { name: "Prince of Swords",      play: decks::prince_of_swords },
    &CardDef { name: "Queen of Cups",         play: decks::queen_of_cups },
    &CardDef { name: "Princess of Diamonds",  play: decks::princess_of_diamonds },
    &CardDef { name: "The Balance",           play: decks::balance },
    &CardDef { name: "The Bole",              play: decks::bole },
    &CardDef { name: "The +4",                play: decks::plus_four },
    &CardDef { name: "THE SKULL KING!",       play: decks::skull_king },
    &CardDef { name: "Pot of Sin",            play: decks::pot_of_sin },
    &CardDef { name: "THE BLACK MAGE",        play: decks::black_mage },
    &CardDef { name: "The Child",             play: decks::child },
    &CardDef { name: "The Crone",             play: decks::crone },
    &CardDef { name: "The Eyes Never Lie",    play: decks::eyes },
    &CardDef { name: "FOOL",                  play: decks::fool },
    &CardDef { name: "THE EXCUSE",           play: decks::excuse },
    &CardDef { name: "JESTER",                play: decks::jester },
    &CardDef { name: "VII THE CHARIOT",       play: decks::chariot },
    &CardDef { name: "XXII THE WORLD",        play: decks::the_world },
    &CardDef { name: "XXIII GOLDEN WIND",     play: decks::golden_wind },
];

/// The card row named `name`, for a save to find its card by.
pub fn card_row(name: &str) -> Option<&'static CardDef> {
    CARDS.iter().copied().find(|row| row.name == name)
}

/// A deck of cards: read off the top one card at a time, or thrown and played
/// as a poker hand. Its mechanic lives in the `decks` submodule of
/// `crate::items`.
pub struct DeckDef {
    /// The row's identity, as [`ScrollDef::name`].
    pub name: &'static str,
}

impl ItemDef for DeckDef {
    fn name(&self) -> &'static str {
        self.name
    }

    /// The deck as the table prints it: the first [`DECK_SIZE`] rows of
    /// [`CARDS`], all upright, the last of them on top.
    ///
    /// [`DECK_SIZE`]: crate::constants::decks::DECK_SIZE
    fn spawn(&self, world: &mut World, pos: Position) -> Entity {
        use crate::constants::decks::DECK_SIZE;
        let cards = CARDS[..DECK_SIZE]
            .iter()
            .map(|&row| Card {
                row,
                reversed: false,
            })
            .collect();
        world
            .spawn((
                Name {
                    what: strings::content_name(self.name).to_string(),
                },
                Renderable {
                    glyph: '?',
                    color: Color::Magenta,
                },
                pos,
                Item,
                Deck { cards },
            ))
            .id()
    }

    fn spawn_as_loot(&self, world: &mut World, rng: &mut ChaCha12Rng, pos: Position) -> Entity {
        let e = self.spawn(world, pos);
        world.entity_mut(e).insert(roll_deck(rng));
        e
    }
}

/// A freshly rolled deck: [`DECK_SIZE`] faces drawn evenly from [`CARDS`] with
/// replacement, then [`REVERSED_MIN`] to [`REVERSED_MAX`] of them turned over.
///
/// [`DECK_SIZE`]: crate::constants::decks::DECK_SIZE
/// [`REVERSED_MIN`]: crate::constants::decks::REVERSED_MIN
/// [`REVERSED_MAX`]: crate::constants::decks::REVERSED_MAX
pub fn roll_deck(rng: &mut ChaCha12Rng) -> Deck {
    use crate::constants::decks::{DECK_SIZE, REVERSED_MAX, REVERSED_MIN};
    use rand::seq::SliceRandom;
    let mut cards: Vec<Card> = (0..DECK_SIZE)
        .map(|_| Card {
            row: CARDS[rng.gen_range(0..CARDS.len())],
            reversed: false,
        })
        .collect();
    let flips = rng.gen_range(REVERSED_MIN..=REVERSED_MAX).min(DECK_SIZE);
    let mut slots: Vec<usize> = (0..DECK_SIZE).collect();
    slots.shuffle(rng);
    for &i in &slots[..flips] {
        cards[i].reversed = true;
    }
    Deck { cards }
}

/// Every kind of deck: there is one.
pub const DECKS: &[DeckDef] = &[DeckDef {
    name: "deck of cards",
}];

// ---------------------------------------------------------------------------
// Runes
// ---------------------------------------------------------------------------

/// A rune: a scroll that goes inert when read instead of crumbling, and wakes
/// again on the stairs. The row is the rune: a [`Rune`] points at its row, and
/// invoking it calls the row's `apply`. The handlers live in the `runes`
/// submodule of `crate::items`.
pub struct RuneDef {
    /// The row's identity: the save file stores it and
    /// [`crate::spawn::spawn_named`] finds the row by it. The player reads
    /// [`ItemDef::display_name`] instead.
    pub name: &'static str,
    /// What invoking a charged rune does to its `user`.
    pub apply: fn(&mut World, Entity),
    /// How often the dungeon drops it against the other runes.
    pub weight: u32,
}

impl RuneDef {
    /// A rune that does `apply`, dropped as often as most.
    pub const fn new(name: &'static str, apply: fn(&mut World, Entity)) -> Self {
        Self {
            name,
            apply,
            weight: crate::constants::monsters::DEFAULT_SPAWN_WEIGHT,
        }
    }

    /// Never dropped: only something else turns a rune into this one.
    const fn never_drops(mut self) -> Self {
        self.weight = 0;
        self
    }
}

impl ItemDef for &'static RuneDef {
    fn name(&self) -> &'static str {
        self.name
    }

    fn weight(&self) -> u32 {
        self.weight
    }

    fn spawn(&self, world: &mut World, pos: Position) -> Entity {
        world
            .spawn((
                Name {
                    what: strings::content_name(self.name).to_string(),
                },
                Renderable {
                    glyph: '\'',
                    color: Color::White,
                },
                pos,
                Item,
                Rune {
                    row: self,
                    charged: true,
                },
            ))
            .id()
    }
}

/// Every rune in the game, one row each.
#[rustfmt::skip]
pub static RUNES: &[&RuneDef] = &[
    &RuneDef::new("rune of recharging",   runes::recharging),
    &RuneDef::new("rune of displacement", runes::displacement),
    &RuneDef::new("rune of justice",      runes::justice),
    &RuneDef::new("rune of chaos",        runes::chaos),
    &RuneDef::new("rune of ice",          runes::ice),
    &RuneDef::new("rune of protection",   runes::protection),
    // What a wand of cancellation leaves of any rune.
    &RuneDef::new("blank rune",           runes::blank).never_drops(),
];

/// The rune row called `name`.
pub fn rune_row(name: &str) -> Option<&'static RuneDef> {
    RUNES.iter().copied().find(|row| row.name == name)
}

// ---------------------------------------------------------------------------
// Wands
// ---------------------------------------------------------------------------

/// A wand: zapped until its battery runs dry. The row is the wand: a [`Wand`]
/// points at its row, zapping calls the row's `zap`, and the rest of the row
/// says what the reticle, a throw and a [`DualZap`] read off it. The handlers
/// live in the `wands` submodule of `crate::items`.
pub struct WandDef {
    /// The row's identity: the save file stores it and
    /// [`crate::spawn::spawn_named`] finds the row by it. The player reads
    /// [`ItemDef::display_name`] instead.
    pub name: &'static str,
    /// The colour it draws in.
    pub color: Color,
    /// How far the aiming reticle reaches.
    pub range: i32,
    /// The palette its blasts burn in, zapped or thrown, and the tint a thrown
    /// one flies in.
    pub palette: BlastPalette,
    /// What zapping it does.
    pub zap: fn(&mut World, Zap),
    /// Whether it deals damage. Thrown, it goes off as the grenade; a
    /// [`DualZap`] wearer fires it twice.
    pub attack: bool,
    /// The element its damage carries, if any ([`Element`]).
    pub element: Option<Element>,
    /// Whether zapping opens the aiming reticle. Only a wand that acts on the
    /// zapper's own room does not.
    pub needs_target: bool,
    /// The disc a zap bursts in on the aimed tile, for the reticle to paint.
    pub zap_radius: Option<f32>,
    /// Whether a zap at this creature would change nothing, so the reticle
    /// passes it over.
    pub wasted_on: fn(&World, Entity) -> bool,
    /// What it does thrown.
    pub thrown: Thrown,
    /// How often the dungeon drops it against the other wands.
    pub weight: u32,
}

/// One zap, as a row's handler receives it.
#[derive(Clone, Copy)]
pub struct Zap {
    /// The wand's row.
    pub row: &'static WandDef,
    /// Whoever zaps.
    pub user: Entity,
    /// Where they stand.
    pub from: Position,
    /// The aimed tile; the zapper's own for a wand that takes no aim.
    pub at: Position,
}

/// What a thrown wand does where it breaks. The reticle paints `radius` and
/// the throw calls `burst`, which reads the radius back from here, so the two
/// cannot disagree. A new wand picks one of the consts below or
/// [`Thrown::blast`]; a new way to burst is a new const and a handler in the
/// `throwing` submodule of `crate::items`.
#[derive(Clone, Copy)]
pub struct Thrown {
    /// The disc it bursts in, for the reticle to paint; `None` paints nothing.
    pub radius: Option<f32>,
    /// What breaking does.
    pub burst: fn(&mut World, Burst),
    /// What happens to each creature the burst catches, if anything.
    pub each: Option<fn(&mut World, Entity)>,
}

/// One thrown wand breaking, as a [`Thrown::burst`] handler receives it.
#[derive(Clone, Copy)]
pub struct Burst<'a> {
    /// The wand's row.
    pub row: &'static WandDef,
    /// Whoever threw it.
    pub thrower: Entity,
    /// Where it broke.
    pub at: Position,
    /// The charges it had left, all spent at once.
    pub charges: i32,
    /// The wand's name as the thrower saw it.
    pub seen_name: &'a str,
}

impl Thrown {
    /// The wide, hot grenade, elemental where the wand is.
    pub const GRENADE: Thrown = Thrown {
        radius: Some(GRENADE_RADIUS),
        burst: throwing::grenade,
        each: None,
    };
    /// The grenade, and every creature it catches is dazzled.
    pub const FLASH: Thrown = Thrown {
        each: Some(dazzle),
        ..Thrown::GRENADE
    };
    /// The small blast that hurts nobody; the creatures it catches trade
    /// places.
    pub const SHUFFLE: Thrown = Thrown {
        radius: Some(BLAST_RADIUS),
        burst: throwing::shuffle,
        each: None,
    };
    /// Confetti, and nothing else.
    pub const CONFETTI: Thrown = Thrown {
        radius: None,
        burst: throwing::confetti,
        each: None,
    };
    /// A crater, line of sight or not, so the reticle paints none.
    pub const CRATER: Thrown = Thrown {
        radius: None,
        burst: throwing::crater,
        each: None,
    };

    /// The small blast that hurts nobody, `each` worked on every creature it
    /// catches.
    pub const fn blast(each: fn(&mut World, Entity)) -> Thrown {
        Thrown {
            radius: Some(BLAST_RADIUS),
            burst: throwing::blast,
            each: Some(each),
        }
    }
}

impl WandDef {
    /// A wand that zaps through `zap`, wasted on nobody, thrown as the plain
    /// small blast.
    pub const fn new(
        name: &'static str,
        color: Color,
        range: i32,
        palette: BlastPalette,
        zap: fn(&mut World, Zap),
    ) -> Self {
        Self {
            name,
            color,
            range,
            palette,
            zap,
            attack: false,
            element: None,
            needs_target: true,
            zap_radius: None,
            wasted_on: |_, _| false,
            thrown: Thrown::blast(|_, _| {}),
            weight: crate::constants::monsters::DEFAULT_SPAWN_WEIGHT,
        }
    }

    /// It deals damage, and thrown it is the grenade.
    const fn attack(mut self) -> Self {
        self.attack = true;
        self.thrown = Thrown::GRENADE;
        self
    }

    /// Its damage carries `element`.
    const fn element(mut self, element: Element) -> Self {
        self.element = Some(element);
        self
    }

    /// A zap bursts over [`BLAST_RADIUS`] where it is aimed.
    const fn bursts(mut self) -> Self {
        self.zap_radius = Some(BLAST_RADIUS);
        self
    }

    /// It takes no aim: zapping acts on the spot.
    const fn unaimed(mut self) -> Self {
        self.needs_target = false;
        self
    }

    /// The reticle passes over a creature `wasted` says it would not change.
    const fn wasted_on(mut self, wasted: fn(&World, Entity) -> bool) -> Self {
        self.wasted_on = wasted;
        self
    }

    /// What it does thrown.
    pub const fn thrown(mut self, thrown: Thrown) -> Self {
        self.thrown = thrown;
        self
    }

    /// Never dropped: no floor ever rolls it.
    const fn never_drops(mut self) -> Self {
        self.weight = 0;
        self
    }
}

impl ItemDef for &'static WandDef {
    fn name(&self) -> &'static str {
        self.name
    }

    fn weight(&self) -> u32 {
        self.weight
    }

    fn spawn(&self, world: &mut World, pos: Position) -> Entity {
        world
            .spawn((
                Name {
                    what: strings::content_name(self.name).to_string(),
                },
                Renderable {
                    glyph: '/',
                    color: self.color,
                },
                pos,
                Item,
                Wand { row: self },
                Ranged { range: self.range },
                Battery {
                    charges: WAND_CHARGES,
                },
            ))
            .id()
    }
}

/// Every wand in the game, one row each.
#[rustfmt::skip]
pub static WANDS: &[&WandDef] = &[
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
    // The wand that does nothing, and says so.
    &WandDef::new("wand of nothing",       Color::Grey,        6, BlastPalette::Warp,   wands::nothing).wasted_on(|_, _| true).thrown(Thrown::CONFETTI).never_drops(),
];

/// The wand row called `name`.
pub fn wand_row(name: &str) -> Option<&'static WandDef> {
    WANDS.iter().copied().find(|row| row.name == name)
}

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
/// not stop at the first body it finds ([`Piercing`]). A boomerang is neither:
/// it strikes the first creature in its way and flies home ([`Returns`]).
pub struct WeaponDef {
    /// The row's identity: the save file stores it and
    /// [`crate::spawn::spawn_named`] finds the row by it. The player reads
    /// [`ItemDef::display_name`] instead.
    pub name: &'static str,
    /// The colour it draws in.
    pub color: Color,
    /// The size of the damage die a blow with it rolls.
    pub power_die: i32,
    /// The die it rolls when thrown. Defaults to `power_die` — a weapon is as
    /// dangerous thrown as it is swung unless the row says otherwise.
    thrown_die: i32,
    /// Purpose-built for throwing: is spent on what it hits, and is never
    /// caught.
    projectile: bool,
    /// Whether the throw carries on through everything on its line (see
    /// [`Piercing`]).
    piercing: bool,
    /// The throw ends back with the thrower (see [`Returns`]).
    returns: bool,
    /// How many creatures one throw strikes in all before it flies home (see
    /// [`ChainHits`]); 0 or 1 for a plain throw.
    hits: u8,
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
    /// wizard now!". See [`OnWear`].
    on_wear: Option<OnWear>,
    /// A one-shot fired the instant it is deliberately put away again — the
    /// other half of the staff's announcement. See [`OnDoff`].
    on_doff: Option<OnDoff>,
}

impl WeaponDef {
    const fn new(name: &'static str, color: Color, power_die: i32) -> Self {
        Self {
            name,
            color,
            power_die,
            thrown_die: IMPROVISED_THROW_DIE,
            projectile: false,
            piercing: false,
            returns: false,
            hits: 0,
            reach: 0,
            reach_piercing: false,
            grants: &[],
            on_wear: None,
            on_doff: None,
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

    /// The throw comes back to the thrower, hit or miss.
    const fn returning(mut self) -> Self {
        self.returns = true;
        self
    }

    /// One throw strikes `n` creatures in all: the first in its way, then the
    /// nearest other one the thrower can see, and so on (see [`ChainHits`]).
    const fn hits(mut self, n: u8) -> Self {
        self.hits = n;
        self
    }

    /// A reach weapon, aimed rather than walked into: a bardiche, a whip.
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

    /// A one-shot fired the instant it is deliberately taken off again (see
    /// [`OnDoff`]).
    const fn on_doff(mut self, on_doff: OnDoff) -> Self {
        self.on_doff = Some(on_doff);
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
                what: strings::content_name(self.name).to_string(),
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
        attach_flight(
            &mut e,
            self.projectile,
            self.piercing,
            self.returns,
            self.hits,
        );
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
        if let Some(on_doff) = self.on_doff {
            e.insert(on_doff);
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
///
/// The line carries real information, which is why it is worth one at all: the
/// staff multiplies what every attacking spell costs *and* what it does
/// ([`crate::constants::spells::TURBO_MAGIC_COST_MULT`] and
/// `TURBO_MAGIC_POWER_MULT`), and neither multiplier appears anywhere on the
/// HUD. See [`announce_not_wizard`] for the other end of it.
fn announce_wizard(world: &mut World, wearer: Entity, _item: Entity) {
    if world.get::<Player>(wearer).is_none() {
        return;
    }
    world.resource_mut::<GameLog>().add(strings::wizard_now());
}

/// [`announce_wizard`] in reverse, the moment the staff is deliberately put
/// away ([`OnDoff`]). The multipliers leave with it, nothing on screen would
/// otherwise say so, and a player about to cast at the price they got used to
/// is owed the warning.
fn announce_not_wizard(world: &mut World, wearer: Entity, _item: Entity) {
    if world.get::<Player>(wearer).is_none() {
        return;
    }
    world
        .resource_mut::<GameLog>()
        .add(strings::wizard_no_more());
}

/// Every melee weapon in the game, one row each.
#[rustfmt::skip]
pub const WEAPONS: &[WeaponDef] = &[
    WeaponDef::new("dagger",           Color::Grey,       4).missile(4).piercing(),
    WeaponDef::new("spear",            Color::DarkGrey,   6).missile(8).piercing(),
    WeaponDef::new("boomerang",        Color::DarkYellow, 5).returning(),
    WeaponDef::new("moon blade",       Color::Cyan,       3).returning().hits(3),
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
    // Thin, fast steel: every attack goes out ESTOC_NUMBER_OF_ATTACKS times in
    // the time a plainer blade manages once, and closing the last stride of a
    // run lands a lunge.
    WeaponDef::new("estoc",            Color::White,      5)
        .grants(&[Grant::counted::<Fencer>(ESTOC_NUMBER_OF_ATTACKS), Grant::of::<Lunges>()]),
    // Whirled at the end of its chain in step with your feet: moving between
    // two tiles both next to the same enemy lands a free cut on it.
    WeaponDef::new("chain-sickle",     Color::DarkGreen,  5)
        .grants(&[Grant::of::<WhirlOnMove>()]),
    // No edge to speak of — it doesn't need one against something that can't
    // fight back.
    WeaponDef::new("garrote",          Color::DarkGrey,   0)
        .grants(&[Grant::of::<VorpalOnCondition>()]),
    // Doubles the toll of every spell cast through it, and triples the fury.
    // The only weapon that says so at both ends: neither multiplier shows on
    // the HUD, so the two log lines are the whole of the player's notice.
    WeaponDef::new("staff",            Color::Yellow,     5)
        .grants(&[Grant::of::<TurboMagic>()])
        .on_wear(OnWear(announce_wizard))
        .on_doff(OnDoff(announce_not_wizard)),
    // A blade with an edge in the world it half-belongs to: every hit that
    // lands there also lands a little on you.
    WeaponDef::new("chaos blade",      Color::Magenta,    12)
        .grants(&[Grant::of::<SelfDamageOnHit>()]),
    // Momentum, not mass: every hit that lands makes the next one hit harder,
    // for as long as you keep swinging it.
    WeaponDef::new("rapier",           Color::Red,        3)
        .grants(&[Grant::of::<BuildsMomentum>()]),
    // Mass, not momentum. The one weapon a petrified creature is no safer
    // from than any other: stone turns steel aside and a hammer goes through
    // it whole.
    WeaponDef::new("war hammer",       Color::DarkGrey,   9)
        .grants(&[Grant::of::<ShattersStone>()]),
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
    /// The row's identity: the save file stores it and
    /// [`crate::spawn::spawn_named`] finds the row by it. The player reads
    /// [`ItemDef::display_name`] instead.
    pub name: &'static str,
    /// The colour it draws in.
    pub color: Color,
    /// The die one of these rolls, lobbed by hand.
    pub die: i32,
    /// The die one of these rolls loosed from its launcher instead. A quarrel's
    /// is the full step up a crossbow earns; an arrow's is short of that — the
    /// bow is the best thing in the dungeon drawn, and this keeps it from also
    /// being the hardest-hitting.
    pub launched_die: i32,
    /// The effect that turns a lob into a shot.
    pub launched_by: Grant,
    /// What a hit that draws blood does to the victim, after the hit line.
    pub on_blood: fn(&mut World, Entity),
}

impl AmmoDef {
    /// How many a floor drop arrives in. Never a lone arrow — finding one arrow
    /// is not finding ammunition.
    fn roll_bundle(rng: &mut ChaCha12Rng) -> u8 {
        rng.gen_range(AMMO_BUNDLE_MIN..=AMMO_BUNDLE_MAX) as u8
    }
}

impl ItemDef for &'static AmmoDef {
    fn name(&self) -> &'static str {
        self.name
    }

    fn spawn(&self, world: &mut World, pos: Position) -> Entity {
        world
            .spawn((
                Name {
                    what: strings::content_name(self.name).to_string(),
                },
                Renderable {
                    glyph: ')',
                    color: self.color,
                },
                pos,
                Item,
                ThrownDamage(self.die),
                Projectile,
                Ammo { row: self },
                Stack { count: 1 },
            ))
            .id()
    }

    fn spawn_as_loot(&self, world: &mut World, rng: &mut ChaCha12Rng, pos: Position) -> Entity {
        let bundle = AmmoDef::roll_bundle(rng);
        let e = self.spawn(world, pos);
        if let Some(mut stack) = world.get_mut::<Stack>(e) {
            stack.count = bundle;
        }
        e
    }
}

/// Every kind of ammunition in the game, one row each.
///
/// To add one: name a fn in `crate::items::throwing` for `on_blood` if a hit
/// does more than damage, and add a row here.
#[rustfmt::skip]
pub static AMMO: &[&AmmoDef] = &[
    // The arrow a bow looses.
    &AmmoDef { name: "arrow",    color: Color::DarkYellow, die: IMPROVISED_THROW_DIE, launched_die: 3, launched_by: Grant::of::<FireArrow>(),   on_blood: |_, _| {} },
    // The quarrel a crossbow looses.
    &AmmoDef { name: "quarrel",  color: Color::Grey,       die: IMPROVISED_THROW_DIE, launched_die: 5, launched_by: Grant::of::<FireQuarrel>(), on_blood: |_, _| {} },
    // The blowdart a blowgun looses. It saps a victim it bleeds.
    &AmmoDef { name: "blowdart", color: Color::Green,      die: IMPROVISED_THROW_DIE, launched_die: 2, launched_by: Grant::of::<FireDart>(),    on_blood: throwing::sap },
];

/// The row a monster's shot is loosed from: the ammunition answering to the
/// effect its launcher lends, so a monster's shot and the player's agree on
/// the same dial. Monsters keep no quiver to check an `Ammo` row against, so
/// `crate::items::monster_ranged_attack` picks the row by effect instead.
/// `None` for a shooter with no launcher effect at all.
pub fn ammo_for(world: &World, shooter: Entity) -> Option<&'static AmmoDef> {
    AMMO.iter()
        .copied()
        .find(|def| def.launched_by.probe(world, shooter))
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
    /// The row's identity: the save file stores it and
    /// [`crate::spawn::spawn_named`] finds the row by it. The player reads
    /// [`ItemDef::display_name`] instead.
    pub name: &'static str,
    /// The colour it draws in.
    pub color: Color,
    /// What it lends its wielder while equipped: the effect ([`FireArrow`],
    /// [`FireQuarrel`]) that lets it loose the matching ammunition.
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
                    what: strings::content_name(self.name).to_string(),
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

/// Every launcher in the game, one row each.
#[rustfmt::skip]
pub const LAUNCHERS: &[LauncherDef] = &[
    LauncherDef { name: "short bow", color: Color::DarkYellow, grants: &[Grant::of::<FireArrow>()],   melee_cap: 1 },
    LauncherDef { name: "crossbow", color: Color::DarkGrey,   grants: &[Grant::of::<FireQuarrel>()], melee_cap: 1 },
    LauncherDef { name: "blowgun",  color: Color::DarkGreen, grants: &[Grant::of::<FireDart>()],    melee_cap: 1 },
];

/// Attaches the markers that describe how a thing behaves in flight, and
/// only when they say something — a mace carries none.
fn attach_flight(
    entity: &mut bevy_ecs::world::EntityWorldMut,
    projectile: bool,
    piercing: bool,
    returns: bool,
    hits: u8,
) {
    if hits > 1 {
        entity.insert(ChainHits(hits));
    }
    if projectile {
        entity.insert(Projectile);
    }
    if piercing {
        entity.insert(Piercing);
    }
    if returns {
        entity.insert(Returns);
    }
}

/// A suit of armour. `armor_die` is what wearing it adds to the defence die.
pub struct ArmorDef {
    /// The row's identity: the save file stores it and
    /// [`crate::spawn::spawn_named`] finds the row by it. The player reads
    /// [`ItemDef::display_name`] instead.
    pub name: &'static str,
    /// The colour it draws in.
    pub color: Color,
    /// What wearing it adds to the defence die.
    pub armor_die: i32,
    /// Marker effects the wearer gains while it's on.
    pub grants: &'static [Grant],
}

impl ItemDef for ArmorDef {
    fn name(&self) -> &'static str {
        self.name
    }

    fn spawn(&self, world: &mut World, pos: Position) -> Entity {
        let mut e = world.spawn((
            Name {
                what: strings::content_name(self.name).to_string(),
            },
            Renderable {
                glyph: ']',
                color: self.color,
            },
            pos,
            Item,
            Equipped::loose(Slot::Body),
            ArmorDie(self.armor_die),
        ));
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

/// Every suit of armour in the game, one row each.
#[rustfmt::skip]
pub const ARMORS: &[ArmorDef] = &[
    ArmorDef { name: "leather armor",          color: Color::DarkYellow, armor_die: 2, grants: &[] },
    ArmorDef { name: "ring mail",              color: Color::Grey,       armor_die: 3, grants: &[] },
    ArmorDef { name: "studded leather armor",  color: Color::DarkYellow, armor_die: 4, grants: &[] },
    ArmorDef { name: "scale mail",             color: Color::Grey,       armor_die: 5, grants: &[] },
    ArmorDef { name: "chain mail",             color: Color::Grey,       armor_die: 6, grants: &[] },
    ArmorDef { name: "splint mail",            color: Color::White,      armor_die: 7, grants: &[] },
    ArmorDef { name: "banded mail",            color: Color::White,      armor_die: 8, grants: &[] },
    ArmorDef { name: "plate mail",             color: Color::Cyan,       armor_die: 9, grants: &[] },
    // Barbed all over: whoever lands a blow on the wearer is pricked for it.
    ArmorDef { name: "spikemail",              color: Color::DarkGrey,   armor_die: 4,
               grants: &[Grant::of::<Spiked>()] },
];

// ---------------------------------------------------------------------------
// Rings
// ---------------------------------------------------------------------------

/// A ring: a modifier item, exactly like a sword or a suit of armour. It stacks
/// numbers through the same [`Modifier`] components combat already folds, and
/// lends marker effects to its wearer through [`Grants`]. There is no such thing
/// as "ring logic" anywhere else in the codebase.
pub struct RingDef {
    /// The row's identity: the save file stores it and
    /// [`crate::spawn::spawn_named`] finds the row by it. The player reads
    /// [`ItemDef::display_name`] instead.
    pub name: &'static str,
    power_die: i32,
    power_bonus: i32,
    armor_die: i32,
    armor_bonus: i32,
    throw_bonus: i32,
    max_hp_bonus: i32,
    /// The marker effects this ring lends its wearer. Read back on load, so a
    /// saved ring never has to store what its row already says.
    pub grants: &'static [Grant],
    /// What happens the *moment* it goes on, for the one ring whose effect is
    /// an event rather than a property. Restored from the row on load, exactly
    /// like `RingDef::grants`.
    pub on_wear: Option<OnWear>,
}

impl RingDef {
    /// A ring with no bonus, no grants and no `on_wear`: the builders below
    /// add them.
    pub const fn new(name: &'static str) -> Self {
        Self {
            name,
            power_die: 0,
            power_bonus: 0,
            armor_die: 0,
            armor_bonus: 0,
            throw_bonus: 0,
            max_hp_bonus: 0,
            grants: &[],
            on_wear: None,
        }
    }

    /// Flat modifier on the wearer's damage roll.
    pub const fn power_bonus(mut self, n: i32) -> Self {
        self.power_bonus = n;
        self
    }

    /// Flat modifier on the wearer's armour roll.
    pub const fn armor_bonus(mut self, n: i32) -> Self {
        self.armor_bonus = n;
        self
    }

    /// Flat modifier on everything the wearer throws.
    pub const fn throw_bonus(mut self, n: i32) -> Self {
        self.throw_bonus = n;
        self
    }

    /// Flat modifier on the wearer's maximum hit points.
    pub const fn max_hp_bonus(mut self, n: i32) -> Self {
        self.max_hp_bonus = n;
        self
    }

    /// Marker effects the wearer gains while it's on.
    pub const fn grants(mut self, grants: &'static [Grant]) -> Self {
        self.grants = grants;
        self
    }

    /// A one-shot fired the instant it goes on (see [`OnWear`]).
    pub const fn on_wear(mut self, on_wear: OnWear) -> Self {
        self.on_wear = Some(on_wear);
        self
    }

    /// The row's flat modifiers: `(power, armor, throw, max hp)`.
    pub fn bonuses(&self) -> (i32, i32, i32, i32) {
        (
            self.power_bonus,
            self.armor_bonus,
            self.throw_bonus,
            self.max_hp_bonus,
        )
    }

    /// Whether this ring is a number at all: protection, strength, increase
    /// damage, sharpshooting, health. The Princess of Diamonds only enchants these.
    pub fn is_numeric(&self) -> bool {
        self.bonuses() != (0, 0, 0, 0)
    }
}

impl ItemDef for &'static RingDef {
    fn name(&self) -> &'static str {
        self.name
    }

    fn spawn(&self, world: &mut World, pos: Position) -> Entity {
        let mut e = world.spawn((
            Name {
                what: strings::content_name(self.name).to_string(),
            },
            Renderable {
                glyph: '=',
                color: Color::Yellow,
            },
            pos,
            Item,
            Ring { row: self },
            Equipped::loose(Slot::Finger),
        ));
        insert_modifier(&mut e, PowerDie(self.power_die));
        insert_modifier(&mut e, PowerBonus(self.power_bonus));
        insert_modifier(&mut e, ArmorDie(self.armor_die));
        insert_modifier(&mut e, ArmorBonus(self.armor_bonus));
        insert_modifier(&mut e, ThrowBonus(self.throw_bonus));
        insert_modifier(&mut e, MaxHpBonus(self.max_hp_bonus));
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
        enchant_ring(world, rng, e, self.is_numeric());
        e
    }
}

/// The rings, all twelve live. Eleven of them are pure table: a number that
/// combat already folds, or a marker some system already asks about. Only
/// adornment needed a verb written for it, and it is named here the same way
/// everything else is — see `crate::items::rings`.
#[rustfmt::skip]
pub static RINGS: &[&RingDef] = &[
    &RingDef::new("ring of protection")
        .armor_bonus(PLAIN_BONUS),
    // Rogue's separate add-strength and sustain-strength rings, merged.
    &RingDef::new("ring of strength")
        .power_bonus(PLAIN_BONUS)
        .grants(&[Grant::of::<SustainsStrength>()]),
    // Rogue's separate searching and see-invisible rings, merged.
    &RingDef::new("ring of perception")
        .grants(&[Grant::of::<SeesInvisible>()]),
    &RingDef::new("ring of aggravate monster")
        .grants(&[Grant::of::<AggravatesMonsters>()]),
    // A steady hand: worth as much on a hurled dagger as on a loosed arrow.
    &RingDef::new("ring of sharpshooting")
        .throw_bonus(PLAIN_BONUS),
    // Rogue's useless ring, made the most valuable thing in the dungeon: worn
    // once, for one action, it doubles the run's score and is gone.
    &RingDef::new("ring of adornment")
        .on_wear(crate::items::rings::ADORNMENT),
    // The ring of strength's plain twin: the same two points on the damage
    // roll, without the arm behind it that a dart can't drain.
    &RingDef::new("ring of increase damage")
        .power_bonus(PLAIN_BONUS),
    &RingDef::new("ring of regeneration")
        .grants(&[Grant::of::<Regenerates>()]),
    // The joke it has always been, taken literally: it slows your digestion by
    // slowing *you*.
    &RingDef::new("ring of slow digestion")
        .grants(&[Grant::of::<Sluggish>()]),
    &RingDef::new("ring of teleportation")
        .grants(&[Grant::of::<Teleportitis>()]),
    &RingDef::new("ring of stealth")
        .grants(&[Grant::of::<Stealthy>()]),
    // The ring of strength's other twin: that one keeps the poison out of your
    // arm, this one keeps the corrosion off your plate.
    &RingDef::new("ring of maintain armor")
        .grants(&[Grant::of::<SustainsArmor>()]),
    &RingDef::new("ring of polymorph")
        .grants(&[Grant::of::<Polymorphitis>()]),
    &RingDef::new("ring of sustain form")
        .grants(&[Grant::of::<SustainsForm>()]),
    // The staff's bargain, for wands: two charges, two casts, one turn.
    &RingDef::new("ring of dualzap")
        .grants(&[Grant::of::<DualZap>()]),
    // Hit points on the way on, none taken back on the way off: swapping it
    // washes health, and a cursed one can kill.
    &RingDef::new("ring of health")
        .max_hp_bonus(PLAIN_BONUS),
];

// ---------------------------------------------------------------------------
// Active spells
// ---------------------------------------------------------------------------

/// An active spell. Unlike every other row in this file, a spell is never
/// spawned — it is not an [`ItemDef`], has no [`Position`] on the floor and no
/// pack slot. A [`Spellset`] holds the row itself, and a save stores its name.
///
/// To add one: write its handler in `crate::items::spells`, then add a row to
/// [`SPELLS`] naming it. `cast` is required, so a row with no handler does not
/// build.
pub struct SpellDef {
    /// The spell's identity, as [`ItemDef::name`] is an item's: the save file
    /// stores it. The player reads [`SpellDef::display_name`] instead.
    pub name: &'static str,
    /// [`Magic`] points one use costs.
    pub cost: u8,
    /// How far the aiming reticle reaches; `0` for a spell that takes no aim
    /// and fires on its slot press.
    pub range: i32,
    /// Attack or skill — the dial a staff's [`TurboMagic`] checks before
    /// multiplying the cost and the damage of the cast. See
    /// [`crate::items::spell_system`].
    pub kind: SpellKind,
    /// What casting it does.
    pub cast: fn(&mut World, SpellCast),
    /// The disc it bursts in on the aimed tile, for the reticle to paint and
    /// for a monster to keep its friends out of.
    pub blast: Option<f32>,
    /// How far past the blast a chained impact can land.
    pub spread: i32,
    /// Whether it flies along the line and stops at the first wall, as a lance
    /// does. Every other aimed spell lands on the aimed tile itself.
    pub line: bool,
    /// Whether a cast at this creature would change nothing, so the reticle
    /// passes it over.
    pub wasted_on: fn(&World, Entity) -> bool,
}

/// One cast, as a row's handler receives it.
#[derive(Clone, Copy)]
pub struct SpellCast {
    /// The spell's row.
    pub row: &'static SpellDef,
    /// Whoever casts.
    pub user: Entity,
    /// The aimed tile; the caster's own for a spell that takes no aim.
    pub target: Position,
    /// What the damage is multiplied by: a staff's
    /// [`TURBO_MAGIC_POWER_MULT`](crate::constants::spells::TURBO_MAGIC_POWER_MULT)
    /// on an attack, `1` otherwise.
    pub power_mult: i32,
}

impl SpellDef {
    /// A skill that takes no aim, wasted on nobody.
    pub const fn new(name: &'static str, cost: u8, cast: fn(&mut World, SpellCast)) -> Self {
        Self {
            name,
            cost,
            range: 0,
            kind: SpellKind::Skill,
            cast,
            blast: None,
            spread: 0,
            line: false,
            wasted_on: |_, _| false,
        }
    }

    /// It deals damage.
    const fn attack(mut self) -> Self {
        self.kind = SpellKind::Attack;
        self
    }

    /// It opens the reticle, which reaches `range` tiles.
    const fn aimed(mut self, range: i32) -> Self {
        self.range = range;
        self
    }

    /// It bursts over `radius` where it is aimed.
    const fn bursts(mut self, radius: f32) -> Self {
        self.blast = Some(radius);
        self
    }

    /// Its impacts chain up to `spread` tiles past the blast.
    const fn spreads(mut self, spread: i32) -> Self {
        self.spread = spread;
        self
    }

    /// It flies the line and stops at the first wall.
    const fn line(mut self) -> Self {
        self.line = true;
        self
    }

    /// The reticle passes over a creature `wasted` says it would not change.
    const fn wasted_on(mut self, wasted: fn(&World, Entity) -> bool) -> Self {
        self.wasted_on = wasted;
        self
    }

    /// Whether triggering it opens the aiming reticle at all.
    pub fn needs_target(&self) -> bool {
        self.range > 0
    }

    /// What the player sees this spell called, in whatever language this
    /// binary was built for. `name` itself never changes — see
    /// [`crate::monsters::MonsterDef::display_name`]'s doc comment for why.
    pub fn display_name(&self) -> &'static str {
        strings::content_name(self.name)
    }
}

impl PartialEq for SpellDef {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name
    }
}

impl Eq for SpellDef {}

impl std::fmt::Debug for SpellDef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.name)
    }
}

/// The row named `name`, as a save stores it.
pub fn spell_row(name: &str) -> Option<&'static SpellDef> {
    SPELLS.iter().copied().find(|row| row.name == name)
}

/// Every active spell in the game: tiers priced by [`Magic`]
/// cost, the same way a floor's danger is priced by depth. A
/// monster pays the same price out of its own
/// [`Magic`] (`crate::monsters::MonsterDef::magic`). These are the spells a
/// coin or a spirit may teach.
#[rustfmt::skip]
pub static SPELLS: &[&SpellDef] = &[
    // --- 1 Ma ---------------------------------------------------------
    &SpellDef::new("Sting", 1, spells::sting).attack().aimed(8),
    // The eel's bolt.
    &SpellDef::new("Thunderbolt", 1, spells::thunderbolt).attack().aimed(8),
    &SpellDef::new("Cure", 1, spells::cure_self),
    // What a lurk is born knowing.
    &SpellDef::new("Bide", 1, spells::bide),
    // --- 2 Ma ---------------------------------------------------------
    // The fire blast a dragon breathes, and the player casts.
    &SpellDef::new("Fireball", 2, spells::breathe_fire).attack().aimed(8).bursts(BLAST_RADIUS).wasted_on(wands::fireproof),
    // The card caster's lance.
    &SpellDef::new("Force Lance", 2, spells::force_lance).attack().aimed(8).line(),
    &SpellDef::new("Identify", 2, spells::identify),
    &SpellDef::new("Setup", 2, spells::setup),
    &SpellDef::new("Polymorph Self", 2, spells::polymorph_self),
    // --- 3 Ma ---------------------------------------------------------
    &SpellDef::new("Lux", 3, spells::lux).attack().aimed(8).bursts(GRENADE_RADIUS),
    &SpellDef::new("Circle of Death", 3, spells::circle_of_death).attack(),
    &SpellDef::new("Magic Ward", 3, spells::magic_ward),
    &SpellDef::new("Heal", 3, spells::heal_self),
    // An aimed skill: it does no damage, so a staff has nothing to double.
    &SpellDef::new("Polymorph Other", 3, spells::polymorph_other).aimed(8),
    // --- 4 Ma ---------------------------------------------------------
    &SpellDef::new("Meteor Strike", 4, spells::meteor_strike).attack().aimed(8).bursts(GRENADE_RADIUS).spreads(METEOR_STRIKE_CHAIN_SPREAD).wasted_on(wands::fireproof),
    // The rune of ice's spell.
    &SpellDef::new("Frost Nova", 4, spells::frost_nova).attack(),
    &SpellDef::new("Magic Mapping", 4, spells::magic_mapping),
    // The rune of chaos's spell.
    &SpellDef::new("Haste Self", 4, spells::haste_self),
    &SpellDef::new("Gate Down", 4, spells::gate_down),
];

// ---------------------------------------------------------------------------
// Coins and the relic
// ---------------------------------------------------------------------------

/// A coin: Rogue's food slot, grown into the whole pickup category. Every one
/// of them works the instant you step on it and is never carried — the two
/// treasure coins go straight into the score, the rest into you.
///
/// `amount` is the row's one dial and what it means is `apply`'s business:
/// points for a treasure coin, hit points for the red one, afflictions lifted
/// for the rosé. A row that needs no number leaves it at zero.
pub struct CoinDef {
    /// The row's identity: the save file stores it and
    /// [`crate::spawn::spawn_named`] finds the row by it. The player reads
    /// [`ItemDef::display_name`] instead.
    pub name: &'static str,
    /// The colour it draws in.
    pub color: Color,
    /// What taking it does to the taker, given `amount`. Returns the line the
    /// log adds after the coin's name.
    pub apply: fn(&mut World, Entity, i32) -> String,
    /// Whether taking it would do anything for the taker now. A coin that
    /// would not is left on the floor, and auto-explore does not detour for it.
    pub helps: fn(&World, Entity) -> bool,
    /// The dial `apply` reads. Zero for a row that needs none.
    pub amount: i32,
    /// Treasure: `amount` also goes into the score as the coin is taken.
    pub treasure: bool,
    /// This row's share of the coin table against its table-mates. Most sit at
    /// the default; the hero coin sits well under it — an uncommon find.
    pub weight: u32,
}

impl ItemDef for &'static CoinDef {
    fn name(&self) -> &'static str {
        self.name
    }

    fn weight(&self) -> u32 {
        self.weight
    }

    fn spawn(&self, world: &mut World, pos: Position) -> Entity {
        let mut e = world.spawn((
            Name {
                what: strings::content_name(self.name).to_string(),
            },
            Renderable {
                glyph: '$',
                color: self.color,
            },
            pos,
            Item,
            Pickup { row: self },
        ));
        if self.treasure {
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
pub static COINS: &[&CoinDef] = &[
    &CoinDef { name: "gold coin",     color: Color::Yellow,      apply: pickups::ledger,           helps: |_, _| true,               amount: 5000, treasure: true,  weight: 10 },
    &CoinDef { name: "silver coin",   color: Color::Grey,        apply: pickups::ledger,           helps: |_, _| true,               amount: 1000, treasure: true,  weight: 10 },
    &CoinDef { name: "red coin",      color: Color::Red,         apply: pickups::heal,             helps: pickups::hurt,             amount:    4, treasure: false, weight: 10 },
    &CoinDef { name: "blue coin",     color: Color::Blue,        apply: pickups::refill_magic,     helps: pickups::drained,          amount:    4, treasure: false, weight: 10 },
    &CoinDef { name: "rosé coin",     color: Color::Magenta,     apply: pickups::cleanse,          helps: crate::conditions::afflicted, amount: 4, treasure: false, weight: 10 },
    &CoinDef { name: "green coin",    color: Color::Green,       apply: pickups::restore_strength, helps: pickups::weakened,         amount:    4, treasure: false, weight: 10 },
    &CoinDef { name: "platine coin",  color: Color::White,       apply: pickups::platinum,         helps: pickups::owed_platinum,    amount:    0, treasure: false, weight: 10 },
    &CoinDef { name: "forge coin",    color: Color::DarkYellow,  apply: pickups::forge,            helps: pickups::owed_forge,       amount:    0, treasure: false, weight: 10 },
    // Teaches a random spell, and a shot one goes off in the widest burst in
    // the game (`crate::traps::detonate_pickup`). Uncommon, and never
    // disguised: no colour name, it is always just "a hero coin".
    &CoinDef { name: "hero coin",     color: Color::Magenta,     apply: pickups::learn_spell,      helps: pickups::can_learn_spell,  amount:    0, treasure: false, weight: 3 },
];

/// The coin row named `name`, for a save to find its coin by.
pub fn coin_row(name: &str) -> Option<&'static CoinDef> {
    COINS.iter().copied().find(|row| row.name == name)
}

// ---------------------------------------------------------------------------
// Treats
// ---------------------------------------------------------------------------

/// A treat: thrown at a creature as an offer of loyalty, never used. What
/// accepting one means lives in [`crate::companion`]. Treats stack like ammo.
pub struct TreatDef {
    /// The row's identity: the save file stores it and
    /// [`crate::spawn::spawn_named`] finds the row by it. The player reads
    /// [`ItemDef::display_name`] instead.
    pub name: &'static str,
    /// The colour it draws in.
    pub color: Color,
    /// Whether a creature takes this one when it is offered.
    pub takes: fn(&World, Entity) -> bool,
}

impl ItemDef for &'static TreatDef {
    fn name(&self) -> &'static str {
        self.name
    }

    fn spawn(&self, world: &mut World, pos: Position) -> Entity {
        world
            .spawn((
                Name {
                    what: strings::content_name(self.name).to_string(),
                },
                Renderable {
                    glyph: '%',
                    color: self.color,
                },
                pos,
                Item,
                Treat { row: self },
                Stack { count: 1 },
            ))
            .id()
    }
}

/// The treats. They share the coins' slice of the drop table: Rogue's food
/// slot, which coins stand in for.
///
/// To add one: name a fn in `crate::companion` that says who takes it, and add
/// a row here.
#[rustfmt::skip]
pub static TREATS: &[&TreatDef] = &[
    // For a creature without hands.
    &TreatDef { name: "snack",          color: Color::DarkYellow, takes: crate::companion::has_no_hands },
    // For a creature with hands.
    &TreatDef { name: "fancy of peace", color: Color::Cyan,       takes: crate::companion::has_hands },
];

/// The pool a floor at the end of a difficulty tier draws one extra find from,
/// over and above its budgeted loot: the nine things that leave the hero
/// permanently stronger. Two coins whose reward is a promise, the coin that
/// teaches a move, the three potions that raise a ceiling, and the three
/// scrolls that permanently sharpen what the hero is already carrying.
///
/// Names rather than rows, because it reaches across three tables — the dungeon
/// spawns them through [`crate::spawn::spawn_named`], for which a name is the
/// key. Placed by `map::population`.
pub const PROGRESSION_ITEMS: &[&str] = &[
    "platine coin",
    "forge coin",
    "hero coin",
    "potion of healing",
    "potion of magic",
    "potion of gain strength",
    "scroll of enchant weapon",
    "scroll of enchant armor",
    "scroll of vorpalize weapon",
];

/// The Element of Yoord: the relic each run retrieves from the deepest floor,
/// spawned in place of that floor's down-stair. Carrying it flips the staircase
/// rules (see [`crate::map::change_level`]) so the player can climb back out.
pub fn spawn_element_of_yoord(world: &mut World, pos: Position) -> Entity {
    world
        .spawn((
            Name {
                what: strings::content_name(crate::spawn::ELEMENT_OF_YOORD).to_string(),
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

/// The ring called `name` on the floor at `pos`. Panics if [`RINGS`] has no
/// such row; see [`spawn_weapon`].
pub fn spawn_ring(world: &mut World, name: &str, pos: Position) -> Entity {
    named(RINGS, name, "ring").spawn(world, pos)
}

/// The ring row named `name`, for a save to find its ring by.
pub fn ring_row(name: &str) -> Option<&'static RingDef> {
    RINGS.iter().copied().find(|row| row.name == name)
}

/// The weapon called `name` on the floor at `pos`. Panics if [`WEAPONS`] has no
/// such row: pass a literal from the catalog, or go through
/// [`crate::spawn::spawn_named`] for a name you did not write.
pub fn spawn_weapon(world: &mut World, name: &str, pos: Position) -> Entity {
    named(WEAPONS, name, "weapon").spawn(world, pos)
}

/// The armour called `name` on the floor at `pos`. Panics if [`ARMORS`] has no
/// such row; see [`spawn_weapon`].
pub fn spawn_armor(world: &mut World, name: &str, pos: Position) -> Entity {
    named(ARMORS, name, "armor").spawn(world, pos)
}

/// One arrow or quarrel. Ammunition arrives in bundles from the dungeon floor
/// ([`ItemDef::spawn_as_loot`]); this is the single unit tests and splits want.
pub fn spawn_ammo(world: &mut World, name: &str, pos: Position) -> Entity {
    named(AMMO, name, "ammo").spawn(world, pos)
}

/// The bow or crossbow called `name` on the floor at `pos`. Panics if [`LAUNCHERS`]
/// has no such row; see [`spawn_weapon`].
pub fn spawn_launcher(world: &mut World, name: &str, pos: Position) -> Entity {
    named(LAUNCHERS, name, "launcher").spawn(world, pos)
}

/// A fresh single unit of whatever `item` is a stack of, spawned nowhere in
/// particular — the one arrow that leaves a quiver when you shoot it, the one
/// snack that leaves the bag when you throw it. `None` if
/// `item` is not something the catalog knows how to make more of.
///
/// Re-rolling the row rather than copying the entity is the same trick the save
/// file plays: a catalog row is the definition, so it is always cheaper to look
/// one up by name than to remember what it said.
pub fn split_one(world: &mut World, item: Entity) -> Option<Entity> {
    let name = world.get::<Name>(item)?.what.clone();
    let one = match AMMO.iter().find(|d| d.display_name() == name) {
        Some(def) => def.spawn(world, Position { x: 0, y: 0 }),
        None => TREATS
            .iter()
            .find(|d| d.display_name() == name)?
            .spawn(world, Position { x: 0, y: 0 }),
    };
    world.entity_mut(one).remove::<Position>();
    Some(one)
}

/// The id of the row whose display name is `display` — the name a save stores
/// next to an entity, so a load can find the row again whatever language it is
/// read in. Covers the rows [`restore_from_catalog`] and the loader read back:
/// weapons, ammunition, launchers, treats and species. `None` for anything
/// else, and for a name no row shows.
pub(crate) fn content_id_of(display: &str) -> Option<&'static str> {
    let ids = WEAPONS
        .iter()
        .map(|d| (d.display_name(), d.name))
        .chain(AMMO.iter().map(|d| (d.display_name(), d.name)))
        .chain(LAUNCHERS.iter().map(|d| (d.display_name(), d.name)))
        .chain(TREATS.iter().map(|d| (d.display_name(), d.name)))
        .chain(
            BESTIARY
                .iter()
                .chain(SUMMONS)
                .map(|m| (m.display_name(), m.name)),
        );
    ids.into_iter()
        .find(|(shown, _)| *shown == display)
        .map(|(_, id)| id)
}

/// Re-attaches what a catalog row gives an item that the save file does not
/// store: how a weapon behaves in flight, what a bow lends its wielder, what a
/// missile answers to. Keyed by the row's id (see `content_id_of`), the same
/// way a ring's grants come back from its [`Ring`] row — the row is the
/// definition, so a save that stored these would only be storing the table
/// twice.
pub fn restore_from_catalog(entity: &mut bevy_ecs::world::EntityWorldMut, id: &str) {
    if let Some(def) = WEAPONS.iter().find(|d| d.name == id) {
        entity.insert(ThrownDamage(def.thrown_die));
        attach_flight(entity, def.projectile, def.piercing, def.returns, def.hits);
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
        if let Some(on_doff) = def.on_doff {
            entity.insert(on_doff);
        }
    }
    if let Some(def) = AMMO.iter().copied().find(|d| d.name == id) {
        entity.insert((ThrownDamage(def.die), Projectile, Ammo { row: def }));
    }
    if let Some(def) = LAUNCHERS.iter().find(|d| d.name == id) {
        entity.insert((Launcher, Grants(def.grants), MeleeCap(def.melee_cap)));
    }
    if let Some(def) = TREATS.iter().copied().find(|d| d.name == id) {
        entity.insert(Treat { row: def });
    }
}

// ---------------------------------------------------------------------------
// Enchantment
// ---------------------------------------------------------------------------

/// The quality every weapon, armour, launcher and ring drop rolls when it spawns.
///
/// | Quality     | Odds                       | Bonus (equal-probability integer)              |
/// |-------------|----------------------------|-------------------------------------------------|
/// | Normal      | [`NORMAL_QUALITY_PCT`]     | +0                                              |
/// | Exceptional | [`EXCEPTIONAL_QUALITY_PCT`]| [`EXCEPTIONAL_BONUS_MIN`]..[`EXCEPTIONAL_BONUS_MAX`] |
/// | Cursed      | whatever is left           | [`CURSED_BONUS_MIN`]..[`CURSED_BONUS_MAX`] (yes, a cursed item can roll positive) |
/// | MEGACURSED  | [`MEGACURSED_PCT_OF_CURSED`]% of the cursed | [`MEGACURSED_BONUS_MIN`]..[`MEGACURSED_BONUS_MAX`], and a [`MegaCurse`] |
///
/// The bonus lands on the flat modifier, never the die size. A ring rolls the
/// same three qualities but its own numbers: see [`enchant_ring`].
#[derive(Clone, Copy, PartialEq, Eq)]
enum Quality {
    Normal,
    Exceptional,
    Cursed,
    MegaCursed,
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
        if rng.gen_range(0..100) < MEGACURSED_PCT_OF_CURSED {
            return Quality::MegaCursed;
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
/// so a future item that is two of those gets both pluses. A ring is none of
/// them and has its own roll, `enchant_ring`.
pub fn enchant_equipment(world: &mut World, rng: &mut ChaCha12Rng, item: Entity) {
    let quality = Quality::roll(rng);
    let bonus: i32 = match quality {
        Quality::Normal => 0,
        Quality::Exceptional => rng.gen_range(EXCEPTIONAL_BONUS_MIN..=EXCEPTIONAL_BONUS_MAX),
        Quality::Cursed => rng.gen_range(CURSED_BONUS_MIN..=CURSED_BONUS_MAX),
        Quality::MegaCursed => rng.gen_range(MEGACURSED_BONUS_MIN..=MEGACURSED_BONUS_MAX),
    };

    apply_bonus(world, item, bonus);
    brand(world, item, quality);
}

/// [`enchant_equipment`], or `enchant_ring` for a ring, with the quality
/// fixed at `Quality::Exceptional`. A ring with no number stays as it was.
pub fn make_exceptional(world: &mut World, rng: &mut ChaCha12Rng, item: Entity) {
    if world.entity(item).contains::<Ring>() {
        set_ring_plus(&mut world.entity_mut(item), RING_EXCEPTIONAL_BONUS);
        return;
    }
    let bonus = rng.gen_range(EXCEPTIONAL_BONUS_MIN..=EXCEPTIONAL_BONUS_MAX);
    apply_bonus(world, item, bonus);
}

/// Rolls quality for a freshly spawned ring.
///
/// | Quality     | Numeric ring (protection, strength, increase damage, sharpshooting) | Any other ring |
/// |-------------|-----------------------------------|-------------------|
/// | Normal      | [`PLAIN_BONUS`] (its row)         | nothing           |
/// | Exceptional | [`RING_EXCEPTIONAL_BONUS`]        | nothing           |
/// | Cursed      | [`RING_CURSED_BONUS_MIN`]..[`RING_CURSED_BONUS_MAX`], and a [`Curse`] | a [`Curse`] |
/// | MEGACURSED  | [`RING_MEGACURSED_BONUS_MIN`]..[`RING_MEGACURSED_BONUS_MAX`], and a [`MegaCurse`] | a [`MegaCurse`] |
///
/// The odds are the gear odds ([`Quality::roll`]). The plus is an absolute
/// number that replaces the row's, not a bonus on top of it.
fn enchant_ring(world: &mut World, rng: &mut ChaCha12Rng, ring: Entity, numeric: bool) {
    let quality = Quality::roll(rng);
    let plus = match quality {
        Quality::Normal => PLAIN_BONUS,
        Quality::Exceptional => RING_EXCEPTIONAL_BONUS,
        Quality::Cursed => rng.gen_range(RING_CURSED_BONUS_MIN..=RING_CURSED_BONUS_MAX),
        Quality::MegaCursed => rng.gen_range(RING_MEGACURSED_BONUS_MIN..=RING_MEGACURSED_BONUS_MAX),
    };
    if numeric {
        set_ring_plus(&mut world.entity_mut(ring), plus);
    }
    brand(world, ring, quality);
}

/// Stamps the curse tags `quality` calls for onto `item`.
fn brand(world: &mut World, item: Entity, quality: Quality) {
    match quality {
        Quality::Cursed => {
            world.entity_mut(item).insert(Curse);
        }
        Quality::MegaCursed => {
            world.entity_mut(item).insert((Curse, MegaCurse));
        }
        _ => {}
    }
}

/// Makes `plus` the number a numeric ring carries, on whichever roll its row
/// feeds. A ring with no number has none of the three components and is left
/// alone.
fn set_ring_plus(entity: &mut bevy_ecs::world::EntityWorldMut, plus: i32) {
    if entity.contains::<PowerBonus>() {
        entity.insert(PowerBonus(plus));
    }
    if entity.contains::<ArmorBonus>() {
        entity.insert(ArmorBonus(plus));
    }
    if entity.contains::<ThrowBonus>() {
        entity.insert(ThrowBonus(plus));
    }
    if entity.contains::<MaxHpBonus>() {
        entity.insert(MaxHpBonus(plus));
    }
}

/// Makes `plus` the item's whole enchantment, replacing what it had, on
/// whichever roll it contributes to (see [`apply_bonus`] for the additive
/// version). A ring with no number ignores it.
pub fn set_plus(world: &mut World, item: Entity, plus: i32) {
    let mut entity = world.entity_mut(item);
    if entity.contains::<Ring>() {
        set_ring_plus(&mut entity, plus);
        return;
    }
    if entity.contains::<PowerDie>() {
        entity.insert(PowerBonus(plus));
    }
    if entity.contains::<ArmorDie>() {
        entity.insert(ArmorBonus(plus));
    }
    if entity.contains::<Launcher>() {
        entity.insert(ThrowBonus(plus));
    }
}

/// Adds `bonus` to whichever roll the item contributes to: the weapon's hit, the
/// armour's guard, or a launcher's throw. A ring is the exception: `bonus` *is*
/// its number (see `enchant_ring`), so `+3` makes a +3 ring of protection and
/// not a +5 one, and a ring with no number ignores it. Shared by
/// [`enchant_equipment`] and the `NIHILURK_SPAWN` `+N` prefix.
pub fn apply_bonus(world: &mut World, item: Entity, bonus: i32) {
    let mut entity = world.entity_mut(item);
    if entity.contains::<Ring>() {
        set_ring_plus(&mut entity, bonus);
        return;
    }
    if entity.get::<PowerDie>().is_some() {
        let base = entity.get::<PowerBonus>().map(|b| b.0).unwrap_or(0);
        entity.insert(PowerBonus(base + bonus));
    }
    if entity.get::<ArmorDie>().is_some() {
        let base = entity.get::<ArmorBonus>().map(|b| b.0).unwrap_or(0);
        entity.insert(ArmorBonus(base + bonus));
    }
    if entity.contains::<Launcher>() {
        let base = entity.get::<ThrowBonus>().map(|b| b.0).unwrap_or(0);
        entity.insert(ThrowBonus(base + bonus));
    }
}
