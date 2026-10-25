//! Items: what lies on the floor and in the pack, the type keys that name which
//! potion, scroll or wand a thing is, and the throwing and launcher components.

use super::*;

// ===========================================================================
// Items: on the floor and in the pack
// ===========================================================================

/// Marker for anything that can sit on the floor and be picked up. The display
/// name lives on the [`Name`] component, same as monsters.
#[derive(Component)]
pub struct Item;

/// What an item is worth in score, paid the moment it is picked up. Coins and
/// the relic carry it; see [`crate::score::award`] and
/// `crate::items::pickups::pick_up`.
#[derive(Component)]
pub struct Value {
    /// The score paid on pickup.
    pub amount: i32,
}

/// An item that is never carried: it works the instant you step on it and is
/// gone. Every coin is one.
///
/// Three rules follow from "never carried", and together they are what makes a
/// pickup a different kind of thing from an item you stow:
///
/// * **A full pack is no obstacle.** There is nothing to find room for.
/// * **It is left alone when it would do nothing.** Walk over a red coin at
///   full health and it stays on the floor waiting for the day you need it
///   (its [`crate::catalog::CoinDef`] row's `helps`), and auto-explore does not detour
///   for one it cannot use either.
/// * **It can be shot.** A missile that comes down on one sets it off like a
///   trap, in a wider burst
///   ([`PICKUP_TRICK_SHOT_RADIUS`](crate::constants::traps::PICKUP_TRICK_SHOT_RADIUS)
///   against [`TRICK_SHOT_RADIUS`](crate::constants::traps::TRICK_SHOT_RADIUS)) — see
///   [`crate::traps::detonate_pickup`].
#[derive(Component)]
pub struct Pickup {
    /// Which coin this is: what taking it does, and the number it does it with.
    pub row: &'static crate::catalog::CoinDef,
}

/// An actor's carried items, in inventory-letter order. An item in here has had
/// its [`Position`] removed; dropping or throwing puts one back.
#[derive(Component)]
pub struct Backpack {
    /// The items carried, in inventory-letter order.
    pub items: Vec<Entity>,
}

/// Used up on use: a potion or a scroll. [`item_system`](crate::items::item_system)
/// despawns anything carrying this once its effect has been applied.
#[derive(Component)]
pub struct Consume;

/// A wand's remaining charges. Each zap spends one; at zero the wand crumbles.
/// Every wand spawns with [`crate::constants::wands::WAND_CHARGES`], full.
#[derive(Component)]
pub struct Battery {
    /// Zaps left. At zero the wand crumbles.
    pub charges: i8,
}

/// How many identical items share one pack slot. Only ammunition stacks: a
/// quiver of arrows is one entity carrying a number, not thirty entities
/// crowding thirty inventory letters. Throwing spends one; picking more up tops
/// the stack back up to at most [`STACK_LIMIT`].
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct Stack {
    /// How many are in the stack.
    pub count: u8,
}

/// An item with a firing range of its own: a wand. Feeds the aiming reticle when
/// the item is zapped (a thrown item uses [`crate::items::THROW_RANGE`] instead).
#[derive(Component)]
pub struct Ranged {
    /// How far the reticle reaches when the item is zapped.
    pub range: i32,
}

/// Marker for the Element of Yoord — the relic each run must carry up from the
/// depths. While the player's pack holds an entity with this component the
/// staircases invert: the up-stair works and the down-stair is dead.
#[derive(Component)]
pub struct Amulet;

// ===========================================================================
// Items: type keys
//
// One component per identifiable kind, holding its catalog row or an enum
// value: the item's identity for `crate::identify` and the save file. A row
// names its own handler; an enum value is what the mechanic in `crate::items`
// matches on.
// ===========================================================================

/// Type-key for a potion: its row. Drinking it calls the row's `apply`.
#[derive(Component)]
pub struct Potion {
    /// Which potion this is.
    pub row: &'static PotionDef,
}

/// Type-key for a scroll: its row. Reading it calls the row's `apply`.
#[derive(Component)]
pub struct Scroll {
    /// Which scroll this is.
    pub row: &'static ScrollDef,
}

/// A deck: a stack of potion and scroll effects, stacked when the deck was
/// rolled, read one at a time off the top. Mechanic: the `decks` submodule of
/// `crate::items`. The top effect is the *last* one, so a read is a `pop`.
#[derive(Component, Clone, Debug, Default)]
pub struct Deck {
    /// What is left, bottom first.
    pub effects: Vec<DeckEffect>,
}

/// One effect in a [`Deck`]: a potion's or a scroll's row, played on the reader
/// the way drinking or reading it would.
#[derive(Clone, Copy)]
pub enum DeckEffect {
    /// Plays the potion row's `apply`.
    Potion(&'static crate::catalog::PotionDef),
    /// Plays the scroll row's `apply`.
    Scroll(&'static crate::catalog::ScrollDef),
}

impl PartialEq for DeckEffect {
    fn eq(&self, other: &Self) -> bool {
        self.name() == other.name()
    }
}

impl Eq for DeckEffect {}

impl std::fmt::Debug for DeckEffect {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.name())
    }
}

impl DeckEffect {
    /// The row's name: the save stores it.
    pub fn name(&self) -> &'static str {
        match self {
            DeckEffect::Potion(row) => row.name,
            DeckEffect::Scroll(row) => row.name,
        }
    }

    /// How often the dungeon drops the row; `0` is never.
    pub fn weight(&self) -> u32 {
        match self {
            DeckEffect::Potion(row) => row.weight,
            DeckEffect::Scroll(row) => row.weight,
        }
    }

    /// The potion or scroll row called `name`, for a save to find it by.
    pub fn named(name: &str) -> Option<Self> {
        crate::catalog::potion_row(name)
            .map(DeckEffect::Potion)
            .or_else(|| crate::catalog::scroll_row(name).map(DeckEffect::Scroll))
    }
}

/// Type-key for a wand. Mechanic: the `wands` submodule of `crate::items`
/// (zapped), and `throwing` (hurled).
#[derive(Component)]
pub struct Wand {
    /// Which wand this is, and what zapping or throwing it does. Identity for
    /// saves by its name — see [`crate::catalog::WANDS`].
    pub row: &'static crate::catalog::WandDef,
}

/// The two shapes an active spell comes in — an attack wand's own split
/// (`WandDef::attack`), drawn again here because a spell
/// answers to it too: a staff's [`crate::effects::TurboMagic`] multiplies the
/// cost ([`TURBO_MAGIC_COST_MULT`](crate::constants::spells::TURBO_MAGIC_COST_MULT))
/// and the damage ([`TURBO_MAGIC_POWER_MULT`](crate::constants::spells::TURBO_MAGIC_POWER_MULT))
/// of an [`Attack`](SpellKind::Attack), and leaves a
/// [`Skill`](SpellKind::Skill) — the utility half, potions and scrolls play the
/// same way — alone.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SpellKind {
    /// Deals damage — [`crate::effects::TurboMagic`]'s business.
    Attack,
    /// Everything else a spell can do.
    Skill,
}

/// The player's active-ability bar: up to four spells, each picked by its row
/// letter from the `Z` menu.
///
/// A spell is coded the way a potion, scroll or wand is — one catalog row
/// ([`crate::catalog::SpellDef`]) that names its own handler in
/// `crate::items::spells` — because it is exactly as *active* as any of
/// those. It differs from every item in the game in what it is not: it
/// carries no [`Item`] marker, is never spawned with a [`Position`], holds no
/// pack slot, and cannot be dropped or thrown. It lives here, permanently, and
/// costs [`Magic`] per use instead of a battery running dry.
#[derive(Component, Default, Clone)]
pub struct Spellset {
    /// The spells known, in slot order. Saved by row name.
    pub slots: Vec<&'static crate::catalog::SpellDef>,
}

/// Type-key for a ring, the twin of [`Potion`] / [`Scroll`] / [`Wand`]. What the
/// ring *does* is not read from here — it rides along as modifier components and
/// [`crate::effects::Grants`], attached by its [`crate::catalog::RingDef`] row.
/// This tag exists so the ring can be identified and saved.
#[derive(Component)]
pub struct Ring {
    /// Which ring this is.
    pub row: &'static RingDef,
}

/// Tag for a cursed piece of equipment. Rolled on at spawn for the majority of
/// weapon/armour/ring drops (see [`crate::catalog::enchant_equipment`]). Once a
/// cursed item is equipped it can't be taken off again until the curse is lifted
/// by a scroll of remove curse.
#[derive(Component)]
pub struct Curse;

/// Rides alongside [`Curse`] on the rare MEGACURSED item (see
/// [`crate::constants::loot::MEGACURSED_PCT_OF_CURSED`]). Nothing lifts it:
/// not remove curse, enchanting, cancellation, a test of faith or a Five
/// Flush, and nothing destroys the item either — no remove curse, pink demon,
/// curse merge or shivering ring. The one exception is a suit taking the
/// killing blow (`helpers::armor_takes_last_hit`).
#[derive(Component)]
pub struct MegaCurse;

/// A weapon, suit of armour or launcher whose enchantment plus and curse status
/// the player has actually learned — by wearing it or by a scroll of identify
/// singling it out (see [`crate::equipment::toggle_equipped`] and
/// `crate::items::scrolls`). Until then [`crate::identify::display_name`]
/// keeps both hidden, the same way a potion hides its effect. A potion, scroll,
/// wand or ring never needs this — their own `crate::identify::Identified`
/// registry already answers the question.
#[derive(Component)]
pub struct KnownQuality;

/// A weapon that has been vorpalized (scroll of vorpalize weapon). Any hit from
/// it that draws blood slays a creature named `bane` outright — as it does any
/// creature carrying [`crate::effects::VorpalTarget`], regardless of `bane`. See
/// [`crate::combat::resolve_attack`].
#[derive(Component)]
pub struct Vorpal {
    /// The name of the species it slays outright.
    pub bane: String,
}

/// The three flavours of elemental damage a wand or a blast can carry. A
/// creature can be immune to one — and the immunity is a plain component, so a
/// dragon's innate [`FireImmune`] and a future ring of fire resistance's are the
/// same thing to the code that checks. Lives here rather than in
/// `crate::items` because both the wand mechanic and the throwing mechanic
/// reach for it.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Element {
    /// Resisted by [`FireImmune`].
    Fire,
    /// Resisted by [`ColdImmune`].
    Cold,
    /// Life drain. Resisted by [`Undead`], which has no
    /// life to take.
    Drain,
}

impl Element {
    /// The marker effect that shrugs this element off.
    pub fn immunity(self) -> Grant {
        match self {
            Element::Fire => Grant::of::<FireImmune>(),
            Element::Cold => Grant::of::<ColdImmune>(),
            Element::Drain => Grant::of::<Undead>(),
        }
    }

    /// The word for this element in an "unharmed by the ___" log line.
    pub fn noun(self) -> &'static str {
        match self {
            Element::Fire => strings::element_fire_noun(),
            Element::Cold => strings::element_cold_noun(),
            Element::Drain => strings::element_drain_noun(),
        }
    }
}

// ===========================================================================
// Items: throwing and launchers
//
// A missile and a launcher never name each other — they meet at an effect (see
// `crate::effects::FireArrow`). Resolution is `crate::items::throwing`.
// ===========================================================================

/// What this item does to a creature it is thrown into: the die rolled on
/// impact (see [`crate::items::throw_system`]). Only things meant to hurt when
/// they land carry it — a dagger does, a wand does not, and an item without one
/// simply bounces off and falls at the target's feet.
///
/// No throw is measured against the target's armour.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct ThrownDamage(pub i32);

/// Made to be thrown: an arrow, a quarrel, a dagger, a spear. Two things
/// follow from it, the same two for all four — the missile is spent on the creature it strikes rather
/// than clattering to the floor, and nothing ever catches one out of the air.
///
/// A projectile that finds no target is not spent: it lands where it fell and
/// can be picked up again.
#[derive(Component, Clone, Copy)]
pub struct Projectile;

/// This missile does not stop at the first thing it hits. A hurled dagger or
/// spear runs the whole line you aimed down, spending itself on every creature
/// standing in it — the NecroDancer trick, and the reason a corridor full of
/// kobolds is worth one spear.
///
/// Everything without it resolves on the first creature in the way, which is why
/// aiming past a monster does not work.
#[derive(Component, Clone, Copy)]
pub struct Piercing;

/// Thrown, this comes back to whoever threw it: a hit or a miss, it flies home
/// afterwards, into the hand it left if that hand was wielding it and into the
/// pack if not. It strikes the first creature in its way like any
/// non-[`Piercing`] throw, and nothing catches it.
#[derive(Component, Clone, Copy)]
pub struct Returns;

/// How many creatures one throw strikes in all, each for full damage: the first
/// in its way, then each time the nearest other living hostile in its thrower's
/// view. Pairs with [`Returns`]: it flies home after the last. A thrower with no
/// sight of its own (a monster) leaves the finding to the blade.
#[derive(Component, Clone, Copy)]
pub struct ChainHits(pub u8);

/// Ammunition: an arrow, a quarrel, a blowdart. Its row says which launcher
/// effect looses it ([`crate::catalog::AmmoDef::launched_by`]), what it rolls
/// then, and what it does to a victim it draws blood from. Neither missile
/// knows a launcher exists, and no launcher knows what ammunition is — they
/// meet at the effect, like everything else here. Identity for saves by its
/// name — see [`crate::catalog::AMMO`].
#[derive(Component, Clone, Copy)]
pub struct Ammo {
    /// Which ammunition this is.
    pub row: &'static crate::catalog::AmmoDef,
}

/// A bow or a crossbow: gear that is worth nothing swung and everything drawn.
/// It contributes no attack die, so its enchantment has no melee roll to land
/// on and lands on [`crate::effects::ThrowBonus`] instead — a +2 bow puts +2 on
/// every arrow it looses. See [`crate::catalog::enchant_equipment`].
#[derive(Component, Clone, Copy)]
pub struct Launcher;

/// A melee weapon that reaches past adjacency: a bardiche strikes two tiles
/// out, a whip five. Aimed with its own reticle (`v`) rather than a walk into
/// the target's tile — see `crate::combat::resolve_reach_attack` and
/// `engine`'s `begin_reach_attack`.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct Reach(pub i32);

/// A reach weapon's strike does not stop at the first body in its line — a
/// bardiche runs clean through, the way a piercing thrown spear does (see
/// [`Piercing`]). A whip, with no polearm's length behind it, lacks this and
/// stops at the first thing it finds.
#[derive(Component, Clone, Copy)]
pub struct ReachPiercing;

// ===========================================================================
// Player conditions
// ===========================================================================
