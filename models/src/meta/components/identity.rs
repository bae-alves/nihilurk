//! Who and where a thing is: name, position, glyph, faction, and the spirit, barter and
//! helper vocabulary.

use super::*;

// ===========================================================================
// Identity, position, appearance
// ===========================================================================

/// The display name of anything the player can be told about — a monster, an
/// item on the floor, the hero. Item type-keys ([`Potion`] &c.) still carry
/// this: the appearance shown before identification is swapped in on top of it
/// by `crate::identify`.
#[derive(Component)]
pub struct Name {
    /// The name. `crate::identify` swaps another in for display where the thing is not yet identified.
    pub what: String,
}

impl Name {
    /// The indefinite article that reads correctly before this name:
    /// `"an"` before a vowel sound, `"a"` otherwise. Good enough for the
    /// bestiary and item list (no "an hour" / "a unicorn" edge cases here).
    pub fn article(&self) -> &'static str {
        crate::identify::article_for(&self.what)
    }
}

/// The one entity the keyboard drives and the camera follows. A marker: exactly
/// one entity in a run has it.
#[derive(Component)]
pub struct Player;

/// A tile coordinate. Every entity that exists *somewhere* on the current floor
/// has one; an item tucked into a [`Backpack`] has its `Position` removed until
/// it is dropped again.
#[derive(Component, Clone, Copy, PartialEq, Eq, Debug)]
pub struct Position {
    /// Column. With `y`, flattened into an array index by [`crate::map::tile_index`].
    pub x: u16,
    /// Row.
    pub y: u16,
}

/// How an entity draws: one glyph in one colour. The colour is packed to a byte
/// against a fixed 16-entry palette on save (see `crate::saveload`).
#[derive(Component)]
pub struct Renderable {
    /// The character it draws as.
    pub glyph: char,
    /// The colour it draws in.
    pub color: Color,
}

/// Whose side an actor is on. Monsters fight the player and (in principle) spare
/// each other; `Ally` fights the monsters. A [`Helper`] is an `Ally`.
///
/// `Spirits` is its own side: peaceful toward everyone until
/// [`SpiritsHostile`] flips, at which point it reads hostile toward the
/// player and their allies exactly like `Monster` does. See
/// `crate::ai::hostile`.
///
/// Appended, not filed under M: a save encodes a variant by its position
/// (`postcard`), so new variants only ever go at the end.
#[derive(Component, PartialEq, Eq, Clone, Copy, Debug, Serialize, Deserialize)]
pub enum Faction {
    /// The player, and the one side every [`Faction::Monster`] hunts.
    Player,
    /// Fights [`Faction::Player`] and [`Faction::Ally`], never another monster.
    Monster,
    /// On the player's side: fights [`Faction::Monster`].
    Ally,
    /// Peaceful toward everyone until [`SpiritsHostile`] is set.
    Spirits,
    /// Part of the scenery that happens to block a tile, like an
    /// [`IceCube`](crate::ice::IceCube): hostile to no one.
    Inert,
}

/// The hidden pull between the demons (cacodaemons) and the angels/sphynx/
/// elves (eudaemons): minus [`ALIGNMENT_POLE`](crate::constants::spirits::ALIGNMENT_POLE)
/// is fully cacodaemon-aligned, plus that fully eudaemon-aligned, `0` neutral.
/// Interacting with a cacodaemon spirit moves this toward its pole by
/// [`ALIGNMENT_STEP`](crate::constants::spirits::ALIGNMENT_STEP), a eudaemon
/// spirit toward the other. Gaining a Helper moves it
/// [`HELPER_GAINED_ALIGNMENT`](crate::constants::spirits::HELPER_GAINED_ALIGNMENT),
/// blowing one up (explosion or system shock)
/// [`HELPER_BLOWN_UP_ALIGNMENT`](crate::constants::spirits::HELPER_BLOWN_UP_ALIGNMENT).
/// Reaching either pole flips
/// [`SpiritsHostile`] for good. A player-only stat, so it lives on the
/// player entity the same way [`Fighter`]/[`Spellset`] do rather than as a
/// bare resource.
#[derive(Component, Clone, Copy, Debug, Default, Serialize, Deserialize)]
pub struct Alignment(pub i8);

/// Whether the spirits have turned on the player for good this run: either
/// [`Alignment`] reached a pole, or the player landed a direct hit (melee or
/// a fired/thrown shot — never an AoE blast or a trap) on a peaceful spirit.
/// Permanent once set; there is no path back down except starting a new run.
#[derive(Resource, Default, Clone, Copy, Debug, Serialize, Deserialize)]
pub struct SpiritsHostile(pub bool);

/// Which way a spirit's row pulls [`Alignment`] when the player interacts
/// with it peacefully: a cacodaemon (the demons) one way, a eudaemon (the
/// angel, sphynx, sylphid, salamander, undyne, gnome) the other, each by
/// [`ALIGNMENT_STEP`](crate::constants::spirits::ALIGNMENT_STEP). Set from
/// `crate::monsters::MonsterDef::spirit_kind`, not saved — cheap to rebuild
/// from the bestiary row on load, the same way `Grants` is.
#[derive(Component, Clone, Copy, PartialEq, Eq, Debug)]
pub enum SpiritKind {
    /// A demon: interacting with it moves [`Alignment`] toward the cacodaemon
    /// pole.
    Cacodaemon,
    /// An angel or kin: interacting with it moves [`Alignment`] toward the
    /// eudaemon pole.
    Eudaemon,
}

/// What melee does to a peaceful [`Faction::Spirits`] mob instead of a normal
/// attack: opens a menu, teaches a spell, unmakes the player's gear —
/// whatever that species' row says. `None` (no component at all) falls back
/// to a generic "nothing happens" line — see `crate::spirits::trigger_event`.
/// Set from `crate::monsters::MonsterDef::spirit_event`, not saved, for the
/// same reason as [`SpiritKind`].
#[derive(Component, Clone, Copy)]
pub struct SpiritEvent(pub fn(&mut World, Entity, Entity));

/// The angel's blessing, pending: set the instant its "Tests your faith!"
/// event fires, cleared the next time the player takes any staircase. See
/// [`crate::spirits::apply_test_of_faith`].
#[derive(Component, Default)]
pub struct TestOfFaithLedger;

/// One thing a spirit's "choose one of three" menu is offering — the blue
/// demon's spells, the sylphid's weapons, the salamander's armor, undyne's
/// rings, the red demon's gear, the gnome's magic. See [`crate::spirits`].
#[derive(Clone, Copy)]
pub enum OfferOption {
    /// A spell to learn. Free even in a priced menu: see [`OfferOption::price`].
    Spell(&'static crate::catalog::SpellDef),
    /// A weapon to take. Costs Max HP in a priced menu.
    Weapon(&'static WeaponDef),
    /// A suit of armour to take. Costs Max HP in a priced menu.
    Armor(&'static ArmorDef),
    /// A ring to take. Costs Max HP in a priced menu.
    Ring(&'static RingDef),
    /// A scroll to take. Costs Max Ma in a priced menu.
    Scroll(&'static ScrollDef),
    /// A potion to take. Costs Max Ma in a priced menu.
    Potion(&'static PotionDef),
    /// A wand to take. Costs Max Ma in a priced menu.
    Wand(&'static WandDef),
}

/// What a priced offer costs: Max HP for the red demon's gear, Max Ma for the
/// gnome's magic. See [`OfferMenu::priced`].
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Price {
    /// This much off the player's max HP, for good. A price equal to the whole of
    /// it is still payable, and paying it ends the run.
    MaxHp(i32),
    /// This much off the player's max Ma, for good.
    MaxMa(u8),
}

impl OfferOption {
    /// The name the menu lists this row under.
    pub fn display_name(&self) -> &'static str {
        match self {
            OfferOption::Spell(def) => def.display_name(),
            OfferOption::Weapon(def) => def.display_name(),
            OfferOption::Armor(def) => def.display_name(),
            OfferOption::Ring(def) => def.display_name(),
            OfferOption::Scroll(def) => def.display_name(),
            OfferOption::Potion(def) => def.display_name(),
            OfferOption::Wand(def) => def.display_name(),
        }
    }

    /// What this row costs when the menu is [`OfferMenu::priced`]. The kind
    /// of thing sets the price, so the red demon and the gnome need no table
    /// of their own. `None` for a spell: no spirit sells one.
    pub fn price(&self) -> Option<Price> {
        use crate::constants::spirits::*;
        match self {
            OfferOption::Spell(_) => None,
            OfferOption::Weapon(_) | OfferOption::Armor(_) | OfferOption::Ring(_) => {
                Some(Price::MaxHp(RED_DEMON_GEAR_PRICE))
            }
            OfferOption::Scroll(_) => Some(Price::MaxMa(GNOME_SCROLL_PRICE)),
            OfferOption::Potion(_) => Some(Price::MaxMa(GNOME_POTION_PRICE)),
            OfferOption::Wand(_) => Some(Price::MaxMa(GNOME_WAND_PRICE)),
        }
    }
}

/// The "choose one of three" menu: open, which row the cursor sits on, and
/// exactly what was rolled to offer — rolled once, when the spirit's event
/// opens it, so backing out and reopening the menu is not how this is done
/// (a spirit only ever gets one interaction). See
/// [`crate::spirits::open_offer_menu`], and the `Z` [`SpellsMenu`] this is
/// modeled on.
#[derive(Resource, Default)]
pub struct OfferMenu {
    /// Whether the menu is up and taking the keyboard.
    pub open: bool,
    /// The row the cursor sits on, an index into `options`.
    pub selected: usize,
    /// What was rolled to offer, in the order the rows are drawn.
    pub options: Vec<OfferOption>,
    /// The spirit whose event opened this menu, if a spirit's did — it
    /// poofs once the player confirms a choice (not on cancel, so a
    /// player who backs out can just melee it again). `None` for anything
    /// that reaches this menu some other way.
    pub source: Option<Entity>,
    /// Whether picking a row costs its [`OfferOption::price`] (the red demon,
    /// the gnome) or comes free (every other spirit).
    pub priced: bool,
}

/// One thing that can sit on either side of a [`BarterMenu`]: an entity in a
/// pack (the yellow demon's trade) or a spell either side already knows or
/// offers (the sphynx's). See [`crate::spirits`].
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Tradeable {
    /// An item in a pack, by entity.
    Item(Entity),
    /// A spell, by row. It moves between spellsets rather than between packs.
    Spell(&'static crate::catalog::SpellDef),
}

/// Which column the cursor is in.
#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub enum BarterColumn {
    /// The pool of what the player holds.
    #[default]
    Player,
    /// The pool of what the demon or sphynx holds.
    Demon,
}

/// The barter menu: the yellow demon's item trade, or the sphynx's spell
/// trade — same shape either way. `player_side`/`demon_side` are the full
/// pools each column lists (rolled or read once, when the menu opens);
/// `player_selected`/`demon_selected` are what's currently staged to change
/// hands. Confirming moves every selected [`Tradeable`] the way it says to
/// and poofs the demon; cancelling moves nothing. See
/// [`crate::spirits::confirm_barter`].
#[derive(Resource, Default)]
pub struct BarterMenu {
    /// Whether the menu is up and taking the keyboard.
    pub open: bool,
    /// The trader across the table. It poofs once a trade is confirmed.
    pub demon: Option<Entity>,
    /// Which pool the cursor is in.
    pub column: BarterColumn,
    /// The row the cursor sits on, within the pool `column` names.
    pub cursor: usize,
    /// Everything the player could put up.
    pub player_side: Vec<Tradeable>,
    /// Everything the trader could put up. Read it through
    /// [`BarterMenu::demon_visible`], which hides the rows the player cannot
    /// match.
    pub demon_side: Vec<Tradeable>,
    /// What the player has staged to give away. Nothing moves until the trade
    /// is confirmed.
    pub player_selected: Vec<Tradeable>,
    /// What the player has staged to receive.
    pub demon_selected: Vec<Tradeable>,
}

impl BarterMenu {
    /// The demon rows the player can actually reach: one per thing they hold
    /// to put against it, so a short pack or spellset hides the demon's
    /// tail. Rendering, cursor travel and staging all read this, never
    /// `demon_side` directly.
    ///
    /// ```
    /// use models::{BarterMenu, Tradeable};
    /// use models::catalog::spell_row;
    ///
    /// let spell = Tradeable::Spell(spell_row("Bide").unwrap());
    /// let menu = BarterMenu {
    ///     player_side: vec![spell],
    ///     demon_side: vec![spell, spell, spell],
    ///     ..Default::default()
    /// };
    /// // One thing to put against three: only one demon row is in reach.
    /// assert_eq!(menu.demon_visible().len(), 1);
    /// ```
    pub fn demon_visible(&self) -> &[Tradeable] {
        let reach = self.player_side.len().min(self.demon_side.len());
        &self.demon_side[..reach]
    }
}

/// The player's boon companion: the one [`Faction::Ally`] that follows them
/// between floors. There is only ever one. Loyalty is not magic, so this is a
/// plain component and not an effect row a wand of cancellation could strip.
/// See [`crate::companion`].
#[derive(Component)]
pub struct Helper;

/// A thing thrown as an offer of loyalty: a snack for a creature without hands,
/// a fancy of peace for one with them. See [`crate::companion`].
#[derive(Component, Clone, Copy)]
pub struct Treat {
    /// Which treat this is, and who takes it. Identity for saves by its name —
    /// see [`crate::catalog::TREATS`].
    pub row: &'static crate::catalog::TreatDef,
}
