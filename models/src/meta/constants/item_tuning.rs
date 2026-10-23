//! Tuning for items: `potions`, `scrolls`, `decks`, `runes`, `wands`, `loot`, `items`,
//! `rings`, `spells`.

// ===========================================================================
// Potions
// ===========================================================================

/// What a dose is worth. The potion *table* (which potion is which colour) is
/// data in `catalog.rs`; the mechanics keyed off each effect are
/// `crate::items`'s `potions` submodule.
///
/// Every number here is permanent: a potion of healing's point of max HP and a
/// potion of poison's lost power both outlive the floor they were drunk on. The
/// only way back up from poison is a potion of restore strength.
pub mod potions {
    /// A potion of healing refills the drinker and raises their ceiling by this
    /// much — the slow, reliable way the hero's HP pool grows over a run, since
    /// nothing else raises it.
    pub const HEALING_MAX_HP_GAIN: i32 = 1;

    /// A potion of extra healing does the same, for more.
    pub const EXTRA_HEALING_MAX_HP_GAIN: i32 = 3;

    /// A potion of gain strength adds this to the drinker's attack die, floor
    /// and ceiling both.
    pub const GAIN_STRENGTH_POWER: i32 = 1;

    /// A potion of magic adds this to the drinker's magic pool, floor and
    /// ceiling both — gain strength's twin, for the other bar on the HUD.
    pub const GAIN_MAGIC_POINTS: u8 = 1;

    /// A potion of poison takes this much off the drinker's attack die, never
    /// below [`POISON_POWER_FLOOR`]. Restore strength puts it all back.
    pub const POISON_POWER_LOSS: i32 = 2;
    /// See [`POISON_POWER_LOSS`]. Poison can leave you feeble but never
    /// weaponless.
    pub const POISON_POWER_FLOOR: i32 = 1;

    /// The chance a paralysed creature's turn is forfeited outright, the player's
    /// and a monster's alike. Rolled once per turn — see
    /// [`crate::conditions::paralysis_forfeits_turn`].
    pub const PARALYSIS_LOST_TURN_CHANCE: f64 = 0.5;

    /// A potion doesn't just dose whoever it hits: it breaks and spreads over
    /// this radius, the same delivery and the same reach as a thrown utility
    /// wand's blast (see [`crate::constants::wands::BLAST_RADIUS`]) — a potion
    /// is a mouthful of glass, but it spreads as far as a wand's small blast.
    pub const POTION_SPLASH_RADIUS: f32 = 1.0;
}

// ===========================================================================
// Scrolls
// ===========================================================================

/// What the words on a page are worth. The scroll *table* (which scroll is
/// which label) is data in `catalog.rs`; the mechanics keyed off each effect are
/// `crate::items`'s `scrolls` submodule.
///
/// Only the scrolls with a *number* in them appear here — the rest are shapes
/// (whatever is in view, whatever is in the pack) rather than magnitudes.
pub mod scrolls {
    /// What one reading of enchant weapon / enchant armor adds to the plus on
    /// the gear it is read over. A minus is not merely nudged by this: it is
    /// mended straight to `+0` (see `crate::items`'s `enchant_gear`), so a
    /// single scroll always redeems the worst cursed item in one go.
    pub const ENCHANT_BONUS: i32 = 1;

    /// Chance a scroll of sleep goes off in the reader's own face instead of
    /// rolling out over the room. The gamble is the point: it is a panic button
    /// that occasionally *is* the emergency.
    pub const SLEEP_BACKFIRE_CHANCE: f64 = 0.25;

    /// How many turns a scroll of sleep puts its victims under — the same
    /// helplessness a sleeping gas trap deals, whether it caught the room or
    /// the reader.
    pub const SLEEP_TURNS: u32 = 5;

    /// How many turns a scroll of hold monster roots what it catches. Longer
    /// than sleep, because a held monster is only pinned and can still fight:
    /// it buys distance, not a free kill.
    pub const HOLD_TURNS: u32 = 10;
}

// ===========================================================================
// Decks of cards
// ===========================================================================

/// The deck's size, its reversals, the chain cap and what each card is worth.
/// The cards themselves are `crate::catalog::CARDS`; what they do lives in the
/// `decks` submodule of `crate::items`.
pub mod decks {
    /// Cards stacked into a deck when it is rolled.
    pub const DECK_SIZE: usize = 5;

    /// The fewest cards a rolled deck holds reversed.
    pub const REVERSED_MIN: usize = 1;

    /// The most cards a rolled deck holds reversed.
    pub const REVERSED_MAX: usize = 2;

    /// The most cards one draw may play, chains included (the Joker, Pot of
    /// Sin, The +4). A chain dies out on its own almost always; this is the
    /// guarantee that it does.
    pub const CARD_CHAIN_CAP: u32 = 32;

    /// What `BALA` adds to its holder's damage roll.
    pub const BALA_POWER: i32 = 4;

    /// What `BOLE` adds to its holder's armour roll.
    pub const BOLE_ARMOR: i32 = 4;

    /// How many turns THE WORLD stops time for.
    pub const WORLD_TURNS: u32 = 5;

    /// How many casts of their spell the Skull King and the Black Mage are
    /// born with. They spam it.
    pub const CARD_CASTER_CASTS: u8 = 13;

    /// How many monsters a reversed GOLDEN WIND conjures.
    pub const GOLDEN_WIND_SUMMONS: usize = 8;

    /// Score a hand pays for each card that took part in it: both of a pair,
    /// all five of a full house, the one card an anti-flush plays. Cards a
    /// chain plays (the Joker, Pot of Sin, The +4) were never in the hand and
    /// pay nothing.
    pub const CARD_POINTS: i32 = 5_000;

    /// The plus a Five Flush sets every piece of worn gear to, curse burnt off.
    pub const FIVE_FLUSH_PLUS: i32 = 5;
}

// ===========================================================================
// Runes
// ===========================================================================

/// The numbers a rune's effect reads. Which runes exist is data in
/// `catalog.rs`; the mechanics are `crate::items`'s `runes` submodule.
pub mod runes {
    /// How many turns a rune of protection stops all damage.
    pub const PROTECTION_TURNS: u32 = 6;

    /// What a rune of recharging adds to each wand's battery.
    pub const RECHARGE_STEP: i8 = 1;

    /// The most charges a rune of recharging leaves in a wand: a fresh wand's
    /// own.
    pub const RECHARGE_CAP: i8 = super::wands::WAND_CHARGES;
}

// ===========================================================================
// Wands and their blasts
// ===========================================================================

/// Charges, damage dice, and the two blast radii. The wand *table* (which wand
/// is which colour, which range, which effect) is data in `catalog.rs`.
pub mod wands {
    /// The odds that polymorphing something already [`Polymorphed`] is a system
    /// shock instead: it comes apart (a monster dies in a burst of gore; the
    /// player is left on 1 HP). Otherwise it settles into a chimeric form.
    ///
    /// [`Polymorphed`]: crate::effects::Polymorphed
    pub const SYSTEM_SHOCK_CHANCE: f64 = 0.25;

    /// How many times a system shock splashes the gore, and how hard each
    /// splash is: `spill_blood`'s droplets and reach stop growing at a cap, and
    /// this sits at or above it, so each splash is the heaviest there is.
    pub const SHOCK_SPLASHES: u32 = 3;
    /// The damage each splash is drawn at; see [`SHOCK_SPLASHES`].
    pub const SHOCK_GORE_DAMAGE: i32 = 40;

    /// Every wand enters the dungeon fully charged. Charges are never shown
    /// to the player, so the number is pure gameplay balance, not a hidden
    /// roll to identify.
    pub const WAND_CHARGES: i8 = 6;

    /// How far, in tiles, a wand of digging bores along the aim, however
    /// near the tile the reticle is on.
    pub const DIG_RANGE: i32 = 8;

    /// A *zapped* attack wand deals `DAMAGE_DICE d DAMAGE_SIDES`,
    /// armour-ignoring, rolled once and applied whole to everyone it touches.
    pub const DAMAGE_DICE: i32 = 2;
    /// See [`DAMAGE_DICE`].
    pub const DAMAGE_SIDES: i32 = 4;

    /// Radius, in tiles, of a zapped fire/cold wand's blast disc, and of a
    /// *thrown* utility wand's (damage-free) effect disc.
    ///
    /// **Relationship to keep in mind:** for years the thrown "grenade" was
    /// defined as exactly twice this. It no longer is — [`GRENADE_RADIUS`] is
    /// set independently — but the two are still meant to read as
    /// "small blast" vs. "room-clearing blast". Keep grenade > blast.
    pub const BLAST_RADIUS: f32 = 1.0;

    /// Radius, in tiles, of the blast a *thrown* attack wand (or the wand of
    /// light) makes when it bursts on impact — the grenade. Wider and hotter
    /// than a zap, and it does not care who set it off: lob one too close and
    /// it burns you too.
    pub const GRENADE_RADIUS: f32 = 3.0;

    /// A thrown wand spends *every* remaining charge at once. An attack-wand
    /// grenade rolls one die of this many sides per charge — at 1, a flat
    /// point a charge.
    pub const GRENADE_DIE_PER_CHARGE: i32 = 1;

    /// How many turns a fire blast's smoke lingers on the tiles it covered,
    /// purely cosmetic, never blocks movement or sight. Zapped
    /// or thrown, makes no difference; see `elemental_blast`.
    pub const SMOKE_LINGER_TURNS: u8 = 4;
}

// ===========================================================================
// Loot: enchantment odds and bundle sizes
// ===========================================================================

/// What a weapon / armour / ring drop rolls when it spawns, and how much
/// ammunition arrives in a bundle. Background: the "Why most gear is cursed"
/// section of `combat-and-balance.md`.
pub mod loot {
    /// Percent chance a gear drop is plain (`+0`, no curse). The remainder
    /// after this and [`EXCEPTIONAL_QUALITY_PCT`] is *cursed*.
    ///
    /// **If you change these two:** they are read as `0..NORMAL` and
    /// `NORMAL..NORMAL+EXCEPTIONAL` against a `0..100` roll in
    /// `catalog::Quality::roll`. Keep `NORMAL + EXCEPTIONAL <= 100`. The odds
    /// table in that function's doc comment is prose — update it too.
    pub const NORMAL_QUALITY_PCT: i32 = 25;

    /// Percent chance a gear drop is exceptional — a clean bonus in
    /// [`EXCEPTIONAL_BONUS_MIN`]..=[`EXCEPTIONAL_BONUS_MAX`]. See
    /// [`NORMAL_QUALITY_PCT`].
    pub const EXCEPTIONAL_QUALITY_PCT: i32 = 10;

    /// Exceptional items roll a flat bonus uniformly in this inclusive range.
    pub const EXCEPTIONAL_BONUS_MIN: i32 = 1;
    /// See [`EXCEPTIONAL_BONUS_MIN`].
    pub const EXCEPTIONAL_BONUS_MAX: i32 = 3;

    /// Cursed items roll a flat bonus uniformly in this inclusive range — yes,
    /// it can come up positive. What you gamble on a cursed item is not the
    /// number, it is that it welds on until a scroll of remove curse (which
    /// destroys it). Widen the negative end to make curses scarier.
    pub const CURSED_BONUS_MIN: i32 = -5;
    /// See [`CURSED_BONUS_MIN`].
    pub const CURSED_BONUS_MAX: i32 = 5;

    /// Percent of cursed drops that come up MEGACURSED instead: stuck on for
    /// good, see [`crate::components::MegaCurse`]. Rolled after the cursed
    /// share above, so a gear drop is MEGACURSED about
    /// `(100 - NORMAL - EXCEPTIONAL) * this / 100` percent of the time.
    pub const MEGACURSED_PCT_OF_CURSED: i32 = 1;

    /// MEGACURSED gear rolls a flat bonus uniformly in this inclusive range:
    /// never better than plain. A ring rolls its own, see
    /// [`crate::constants::rings::MEGACURSED_BONUS_MAX`].
    pub const MEGACURSED_BONUS_MIN: i32 = -3;
    /// See [`MEGACURSED_BONUS_MIN`].
    pub const MEGACURSED_BONUS_MAX: i32 = 0;

    /// Percent chance a curse merge (equipping a known cursed item over a worn
    /// cursed one in the same slot) leaves the worn item's form.
    pub const MERGE_RECIPIENT_PCT: i32 = 45;
    /// Percent chance a curse merge leaves the newly equipped item's form. The
    /// remainder after this and [`MERGE_RECIPIENT_PCT`] is both items breaking.
    pub const MERGE_DONOR_PCT: i32 = 45;

    /// The die a thrown thing rolls when it was not made for throwing: a mace,
    /// or an arrow lobbed by hand. Only a purpose-built missile
    /// ([`crate::components::Projectile`]) or a launcher does better.
    pub const IMPROVISED_THROW_DIE: i32 = 2;

    /// A dropped ammunition bundle holds this many, uniformly — never a lone
    /// arrow, because finding one arrow is not finding ammunition. Capped by
    /// [`crate::constants::items::STACK_LIMIT`] once it lands in a pack slot.
    pub const AMMO_BUNDLE_MIN: i32 = 4;
    /// See [`AMMO_BUNDLE_MIN`].
    pub const AMMO_BUNDLE_MAX: i32 = 13;
}

// ===========================================================================
// Items in the hand and in the pack
// ===========================================================================

/// Throw range and stack size.
pub mod items {
    /// How many times an estoc throws each attack: the total, so `2` is the
    /// blow and one more.
    pub const ESTOC_NUMBER_OF_ATTACKS: u8 = 2;

    /// How far a heavy thing can be hurled, in tiles — the throw reticle's
    /// default leash. A wand overrides this with its own `range` when zapped,
    /// but a *thrown* wand obeys a leash like anything else.
    pub const THROW_RANGE: i32 = 4;

    /// The same leash for the small stuff — a potion, a scroll, a wand, a ring.
    /// Little enough to get a wrist behind, so it carries further than a spear
    /// or a fistful of arrows.
    pub const LIGHT_THROW_RANGE: i32 = 6;

    /// How far ammunition carries when it is *loosed* rather than lobbed — an
    /// arrow from a bow, a quarrel from a crossbow. Further than the arm alone
    /// carries it, which is the whole reason to carry the stick.
    pub const LAUNCHER_RANGE: i32 = 8;

    /// The most one pack slot will hold before the overflow spills into a
    /// second slot. A round, generous number — nothing in the engine forces a
    /// particular value.
    ///
    /// **If you change it:** `models/tests/it/missiles.rs` names the current value and would need
    /// a pass.
    pub const STACK_LIMIT: u8 = 13;

    /// The most inventory slots a pack will hold at once. It has to stop short of
    /// the `j` and `k` rows the pack menu reserves for down and up: raise it
    /// past that and those two rows become unreachable by letter, since
    /// `navigate_pack` reads the direction first.
    pub const PACK_CAPACITY: usize = 9;
}

// ===========================================================================
// Rings
// ===========================================================================

/// What the rings whose effect is a *verb* are worth. The eleven rings that are
/// only a number or a marker have no knobs here — their whole content is the
/// row in `catalog.rs`. See `models/src/items/rings.rs`.
pub mod rings {
    /// How close a stealthy player has to be before anything on the floor
    /// notices them, in tiles (Chebyshev — a diagonal counts as one). Set it
    /// small and a ring of stealth makes you effectively untouchable outside
    /// melee; set it large and it stops changing how a room plays.
    pub const STEALTH_RANGE: i32 = 2;

    /// Magic points one deliberate teleport costs a wearer of the ring of
    /// teleportation.
    pub const TELEPORT_MAGIC_COST: u8 = 2;

    /// A MEGACURSED numeric ring rolls its plus uniformly in this inclusive
    /// range: built different, it can far outclass any other ring, and still
    /// never comes off.
    pub const MEGACURSED_BONUS_MIN: i32 = 0;
    /// See [`MEGACURSED_BONUS_MIN`].
    pub const MEGACURSED_BONUS_MAX: i32 = 13;

    /// The plus a *numeric* ring — protection, strength, increase damage,
    /// sharpshooting — carries when it rolls plain. An absolute number, not a
    /// bonus on top of the row: the rows read it too, so the table and the
    /// roll cannot disagree.
    pub const PLAIN_BONUS: i32 = 2;

    /// The plus a numeric ring carries when it rolls exceptional, always.
    pub const EXCEPTIONAL_BONUS: i32 = 3;

    /// A cursed numeric ring rolls its plus uniformly in this inclusive range.
    /// It tops out at [`PLAIN_BONUS`], so a curse is never the better ring,
    /// only the one that will not come off.
    pub const CURSED_BONUS_MIN: i32 = -3;
    /// See [`CURSED_BONUS_MIN`].
    pub const CURSED_BONUS_MAX: i32 = PLAIN_BONUS;
}

// ===========================================================================
// Active spells
// ===========================================================================

/// The dice and odds behind the spells whose damage isn't simply "as the
/// wand/trap it borrows from" (Fireball, Sting) — see `crate::items::spells`.
/// Each spell's cost, range and kind are on its own catalog row
/// ([`crate::catalog::SpellDef`]); these are the numbers a rebalance actually
/// reaches for.
pub mod spells {
    /// The most spells a [`crate::components::Spellset`] may ever hold — the
    /// rows of the `Z` menu, and no further row to reach for. A hero
    /// coin stops teaching once this is full.
    pub const SPELLSET_CAP: usize = 4;

    /// The most platinum or forge coins the stairs will owe you at once; a
    /// coin past it stays on the floor.
    pub const PROMISE_CAP: u8 = 3;

    /// A staff's [`crate::effects::TurboMagic`]: what an
    /// [`Attack`](crate::components::SpellKind::Attack)'s
    /// [`SpellDef::cost`](crate::catalog::SpellDef::cost) is multiplied by
    /// before it is charged. A [`Skill`](crate::components::SpellKind::Skill)
    /// is never touched — the staff buys fury, and a Skill has none to buy.
    pub const TURBO_MAGIC_COST_MULT: u8 = 2;

    /// What that same staff multiplies the Attack's *damage* by, reaching
    /// every mechanic as `power_mult`. Deliberately above
    /// [`TURBO_MAGIC_COST_MULT`]: a staff eats most of a starting
    /// [`crate::constants::player::START_MAGIC`] pool per cast, so it has to
    /// give back more than it takes or nobody would wield one. Bring the two
    /// level and the staff becomes a strictly worse wand.
    pub const TURBO_MAGIC_POWER_MULT: i32 = 3;

    /// Thunderbolt: `DICE d SIDES` armour-ignoring damage, and `PARALYZE_CHANCE`
    /// to lock the target up on top of it.
    pub const THUNDERBOLT_DAMAGE_DICE: i32 = 2;
    /// See [`THUNDERBOLT_DAMAGE_DICE`].
    pub const THUNDERBOLT_DAMAGE_SIDES: i32 = 2;
    /// See [`THUNDERBOLT_DAMAGE_DICE`].
    pub const THUNDERBOLT_PARALYZE_CHANCE: f64 = 0.30;

    /// Force Lance: a line of `DICE d SIDES` armour-ignoring damage, exactly
    /// like a wand of striking's own bolt.
    pub const FORCE_LANCE_DAMAGE_DICE: i32 = 2;
    /// See [`FORCE_LANCE_DAMAGE_DICE`].
    pub const FORCE_LANCE_DAMAGE_SIDES: i32 = 3;

    /// Circle of Death: `DICE d SIDES` armour-ignoring drain, rolled once per
    /// creature in view and given back to the caster as HP.
    pub const CIRCLE_OF_DEATH_DAMAGE_DICE: i32 = 3;
    /// See [`CIRCLE_OF_DEATH_DAMAGE_DICE`].
    pub const CIRCLE_OF_DEATH_DAMAGE_SIDES: i32 = 3;

    /// Frost Nova: `DICE d SIDES` armour-ignoring cold damage to everything in
    /// view, each on top paralysed if it survives.
    pub const FROST_NOVA_DAMAGE_DICE: i32 = 4;
    /// See [`FROST_NOVA_DAMAGE_DICE`].
    pub const FROST_NOVA_DAMAGE_SIDES: i32 = 3;

    /// How many "charges" Lux throws itself as — a wand of light has no
    /// battery of its own to spend here, so this stands in for one.
    pub const LUX_CHARGES: i32 = 3;
    /// See [`LUX_CHARGES`], Meteor Strike's own stand-in battery.
    pub const METEOR_STRIKE_CHARGES: i32 = 3;
    /// Meteor Strike's odds, after every impact, of tearing the sky open for
    /// another one.
    pub const METEOR_STRIKE_CHAIN_CHANCE: f64 = 0.50;
    /// How far, in tiles, a chained meteor may land from the one before it.
    pub const METEOR_STRIKE_CHAIN_SPREAD: i32 = 3;
}
