//! Every tuning knob in the model, in one place.
//!
//! roog's balance lives in a few dozen numbers scattered across a dozen
//! modules. This file gathers the ones worth turning — the ones a person
//! rebalancing the game reaches for — so that job is "read one file, edit one
//! file" instead of a treasure hunt.
//!
//! Each constant is defined here and re-exported from the module that uses it
//! (often under its historical name), so call sites and doc links elsewhere in
//! the tree keep working. Change the value here; nothing else needs to move
//! unless a doc comment below says so.
//!
//! ## What is *not* here
//!
//! * **Animation timing** (`particles.rs` — bolt/blast/ripple milliseconds) and
//!   **magic-map wave counts** (`magicmap.rs`). These are presentation feel,
//!   wound tightly around the algorithms that read them, and no one balances
//!   the game by touching them.
//! * **Content-table numbers** — a monster's HP, a weapon's die, a potion's
//!   colour. Those are data, one row per thing, and live in `catalog.rs` /
//!   `monsters.rs` / `traps.rs`. See `docs/reference/content-tables.md`.
//! * **Map-layout geometry** (room sizes, corridor carving), RNG salts, and the
//!   neighbour-offset tables. Structural, not balance.
//! * A couple of one-off rolls still inline where they fire. Called out in
//!   `docs/reference/constants.md`.

// ===========================================================================
// Combat
// ===========================================================================

/// Numbers that shape a single exchange of blows. Background:
/// `docs/explanation/combat-and-balance.md`.
pub mod combat {
    /// The player's chance, per swing, of landing an "excellent hit" that rolls
    /// [`EXCELLENT_HIT_DICE`] weapon dice instead of one. Monsters never get it.
    ///
    /// This is the player's escape hatch from a stalemate against good armour.
    /// Drop it toward 0 and a poorly-geared player facing plate has no path;
    /// raise it toward 1 and armour stops mattering.
    pub const EXCELLENT_HIT_CHANCE: f64 = 0.15;

    /// How many weapon dice an excellent hit rolls: `1d[power]` becomes
    /// `Nd[power]`, summed, *before* armour is subtracted. At 3 a lucky swing
    /// roughly triples its ceiling.
    pub const EXCELLENT_HIT_DICE: i32 = 3;

    /// The floor under a player's swing: even when the armour roll eats the
    /// whole blow, the player still scrapes off this much (logged as a
    /// "glancing blow"). It keeps a fight from never moving the number.
    ///
    /// A glancing blow can never be the killing one — it leaves a foe on 1 HP —
    /// so raising this does not let chip damage finish things, only speeds the
    /// grind. Monsters have no such floor: a monster that cannot beat your
    /// armour simply cannot hurt you, and that asymmetry is why armour is worth
    /// wearing. Leave at 1 unless you are re-teaching that lesson.
    pub const CHIP_DAMAGE: i32 = 1;

    /// The odds that any one piece of a dead creature's gear survives to be
    /// picked up; the rest is lost in the mess. Rolled once per equipped item.
    /// Lower this and thrown-weapon retrieval gets stingier; raise it toward 1
    /// and every orc becomes a vending machine for its own sword.
    pub const GEAR_SURVIVES_DEATH: f64 = 0.5;
}

// ===========================================================================
// The player
// ===========================================================================

/// The hero's opening line. These seed a *new* game only — a loaded save
/// carries its own numbers (see `saveload.rs`), so changing them never touches
/// a run in progress.
pub mod player {
    /// Starting (and maximum) hit points. The whole bestiary is tuned against a
    /// 12-HP player — see `combat-and-balance.md` — so raising this quietly
    /// makes the early floors safer across the board. There is no level-up:
    /// this is the number for the whole run, bar potions of raise level.
    pub const START_HP: i32 = 12;

    /// Starting armour die (`1d[armor]` on the defence roll), before the +1 ring
    /// mail the hero spawns wearing. Matches an unarmoured townsperson.
    pub const START_ARMOR: i32 = 2;

    /// Starting power die (`1d[power]` on the attack roll), before the +1 mace.
    pub const START_POWER: i32 = 4;

    /// Starting (and maximum) magic points — the pool abilities draw on, shown
    /// as `Ma X/Y` on the HUD. Refilled in full by every staircase
    /// ([`crate::constants::progression::DESCENT_RESTORES_MAGIC`]).
    pub const START_MAGIC: u8 = 4;

    /// Sight radius, in tiles, for the player's Viewshed. Rooms flood-fill on
    /// top of this; corridors and dark rooms are cut back to the always-on 3x3.
    ///
    /// A loaded save keeps the range it was saved with (`saveload.rs` restores
    /// it), so this too only affects new games.
    pub const SIGHT_RANGE: u16 = 12;

    /// Fraction of max HP at or below which the player gets a one-time "badly
    /// wounded" warning as they cross down into it. See
    /// [`crate::helpers::apply_damage`].
    pub const LOW_HP_WARNING_FRACTION: f32 = 0.25;
}

// ===========================================================================
// Progression / the descent
// ===========================================================================

/// Depth, the Dungeon Lord's patience, and what a staircase does for you.
pub mod progression {
    /// The deepest floor of a run. It has no down-stair — the Element of Yoord
    /// sits where the stair would be, and carrying it back out is the game.
    ///
    /// **If you change this:** the Element-of-Yoord placement and the
    /// staircase-inversion logic key off it automatically, but
    /// `docs/reference/content-tables.md` and `gdd.md` name "depth 13" in prose
    /// and would need a pass. Monster/trap `min_depth` values in the catalog
    /// are relative to 1, not to this, so deep content still appears — just
    /// with fewer floors to spread over.
    pub const FINAL_DEPTH: u8 = 13;

    /// Turns the player may dawdle on one level before the Dungeon Lord loses
    /// patience and portals them onward — down on the way in, up once they
    /// carry the Element. Reset by every level change. Lower is crueller; this
    /// is the clock the whole "one room past the Lord's patience" death runs
    /// on.
    pub const DUNGEON_LORD_PATIENCE: u32 = 260;

    /// A staircase is a rest: arriving on a new floor heals
    /// `max_hp / DESCENT_HEAL_DIVISOR` and refills the magic pool in full. A
    /// trapdoor plunge is *not* a rest and skips both. Set the divisor to 1 for
    /// a full heal on every floor (a much easier run); a larger divisor makes
    /// attrition bite sooner.
    pub const DESCENT_HEAL_DIVISOR: i32 = 2;

    /// The last floor of each floor-crowding tier below the deepest one. The
    /// monster and trap *budgets* (`constants::population`, spent in `map.rs`)
    /// gain a slot and widen their fill odds at each of these depths. `[3, 6,
    /// 9, 12]` against a 13-floor dungeon gives five tiers spanning depths 1-3,
    /// 4-6, 7-9, 10-12, and 13 alone — the deepest floor its own hardest band.
    ///
    /// The damage traps scale on their own, coarser bands
    /// ([`crate::constants::traps::TRAP_DAMAGE_TIER_LAST_DEPTH`]).
    ///
    /// **If you change this:** `map::difficulty_tier` reads it, and `gdd.md`
    /// and `docs/how-to/tune-rarity-and-depth.md` name the boundaries.
    pub const DIFFICULTY_TIER_LAST_DEPTH: [u8; 4] = [3, 6, 9, 12];
}

// ===========================================================================
// Map
// ===========================================================================

/// The grid itself.
pub mod map {
    /// Playfield width in tiles. The HUD log is sized to match
    /// ([`crate::constants::hud::LOG_WIDTH`]) and `determinism.rs` pins the
    /// layout for existing seeds — changing either dimension moves every wall
    /// on every seed and that test will (correctly) fail. Only change these
    /// together with a deliberate "all old seeds are void" decision.
    pub const WIDTH: u16 = 80;

    /// Playfield height in tiles. The terminal must fit this plus the HUD; see
    /// the portability note in `gdd.md`. Same determinism caveat as [`WIDTH`].
    pub const HEIGHT: u16 = 22;

    /// Chance that a given room past the starting one spawns unlit. A dark room
    /// behaves like a corridor (sight cut to the 3x3) until a wand of light
    /// goes off in it. Raise for a darker, more wand-of-light-dependent game.
    pub const DARK_ROOM_CHANCE: f64 = 0.15;
}

// ===========================================================================
// How crowded a floor is
// ===========================================================================

/// The monster, trap and item budgets a floor spends when it is populated.
/// Everything here scales with `map::difficulty_tier(depth)` — `0` through `4`,
/// stepping at [`crate::constants::progression::DIFFICULTY_TIER_LAST_DEPTH`] —
/// so the dungeon gets nastier in bands as you descend.
pub mod population {
    /// Monster slots on floor 1, before the per-tier bonus. The first slot
    /// always fills; the rest roll [`MONSTER_FILL_CHANCE_BASE`].
    pub const MONSTER_SLOTS_BASE: usize = 3;

    /// Chance a non-first monster slot actually spawns something, at tier 0.
    pub const MONSTER_FILL_CHANCE_BASE: f64 = 0.60;

    /// Added to the monster fill chance per tier.
    pub const MONSTER_FILL_CHANCE_PER_TIER: f64 = 0.12;

    /// Ceiling on the monster fill chance, so a slot is never quite certain.
    pub const MONSTER_FILL_CHANCE_CAP: f64 = 0.95;

    /// Trap slots on floor 1, before the per-tier bonus.
    pub const TRAP_SLOTS_BASE: usize = 4;

    /// Chance a trap slot produces a trap, at tier 0.
    pub const TRAP_FILL_CHANCE_BASE: f64 = 0.12;

    /// Added to the trap fill chance per tier.
    pub const TRAP_FILL_CHANCE_PER_TIER: f64 = 0.13;

    /// Ceiling on the trap fill chance.
    pub const TRAP_FILL_CHANCE_CAP: f64 = 0.75;

    /// From this depth on, every corridor centre has a small chance
    /// ([`CORRIDOR_LURKER_CHANCE`]) of hiding a monster right in the traveller's
    /// path.
    pub const CORRIDOR_LURKER_MIN_DEPTH: u8 = 7;

    /// Per-corridor chance of a mid-corridor lurker, once past
    /// [`CORRIDOR_LURKER_MIN_DEPTH`].
    pub const CORRIDOR_LURKER_CHANCE: f64 = 0.05;

    /// Ordinary item drops attempted on floor 1, before the per-tier bonus.
    /// Like the monster and trap budgets, the item budget gains one attempt per
    /// `map::difficulty_tier(depth)` — `ITEM_SLOTS_BASE + tier` tries, each of
    /// which still needs a free tile. Every attempt that lands a tile drops an
    /// item (no fill roll — deeper floors are simply richer).
    pub const ITEM_SLOTS_BASE: usize = 3;

    /// Chance a floor also hides one extra item — no glyph, no announcement —
    /// until a ring of perception turns it up or the player walks onto it. One
    /// in five.
    pub const HIDDEN_ITEM_CHANCE: f64 = 0.20;
}

// ===========================================================================
// Traps
// ===========================================================================

/// The bite of the two damage traps (arrow, dart), and how it grows with
/// depth. The trap *table* — which trap is which glyph, how often the dungeon
/// lays one, how long a snare holds — is data in `traps.rs`; the mechanics are
/// `arrow_effect` / `dart_effect` there. Background: the "Traps that scale"
/// section of `docs/explanation/combat-and-balance.md`.
pub mod traps {
    /// The last floor of each damage tier below the deepest. Coarser than the
    /// floor-crowding bands
    /// ([`crate::constants::progression::DIFFICULTY_TIER_LAST_DEPTH`]): `[4,
    /// 8]` gives three tiers — depths 1-4, 5-8, 9-13 — so the arrow and dart
    /// step up three times over a run, not four. Read by
    /// `traps::trap_damage_tier`.
    pub const TRAP_DAMAGE_TIER_LAST_DEPTH: [u8; 2] = [4, 8];

    /// An arrow trap's hit rolls `ARROW_DAMAGE_DICE d ARROW_DAMAGE_SIDES +
    /// ARROW_DAMAGE_BONUS` before armour-plus. Today: `1d4 + 1`.
    pub const ARROW_DAMAGE_DICE: i32 = 1;
    /// See [`ARROW_DAMAGE_DICE`].
    pub const ARROW_DAMAGE_SIDES: i32 = 4;
    /// See [`ARROW_DAMAGE_DICE`].
    pub const ARROW_DAMAGE_BONUS: i32 = 1;

    /// Added to an arrow trap's damage roll for each depth tier past the first
    /// (tier 0: +0, tier 1: +1, tier 2: +2).
    pub const ARROW_DAMAGE_PER_TIER: i32 = 1;

    /// A dart trap's hit rolls `DART_DAMAGE_DICE d DART_DAMAGE_SIDES` before
    /// armour-plus. Today: `1d2`.
    pub const DART_DAMAGE_DICE: i32 = 1;
    /// See [`DART_DAMAGE_DICE`].
    pub const DART_DAMAGE_SIDES: i32 = 2;

    /// Permanent melee power a dart trap drains on a hit in the first depth
    /// tier; each deeper tier drains [`DART_POWER_DRAIN_PER_TIER`] more (tier 0:
    /// 1, tier 1: 2, tier 2: 3). Floored so `power` never drops below 1, and
    /// negated entirely by a ring of strength.
    pub const DART_POWER_DRAIN_BASE: i32 = 1;
    /// See [`DART_POWER_DRAIN_BASE`].
    pub const DART_POWER_DRAIN_PER_TIER: i32 = 1;

    /// Damage the player takes for thrashing against a sprung bear trap — one
    /// wasted turn and this much blood ("As you stumble drunkenly, the trap
    /// flays your leg"). A bear trap does not otherwise deal damage.
    pub const BEAR_TRAP_THRASH_DAMAGE: i32 = 1;

    /// Cosmetic only: the wound the thrash *reads* as when
    /// `traps::bear_trap_thrash` splatters blood. Far above
    /// [`BEAR_TRAP_THRASH_DAMAGE`] on purpose — the leg tears against the steel
    /// and the tile should show it — but it costs no extra HP.
    pub const BEAR_TRAP_THRASH_GORE: i32 = 32;
}

// ===========================================================================
// Wands and their blasts
// ===========================================================================

/// Charges, damage dice, and the two blast radii. The wand *table* (which wand
/// is which colour, which range, which effect) is data in `catalog.rs`.
pub mod wands {
    /// A fresh wand's battery is `CHARGE_DICE d CHARGE_SIDES + CHARGE_BONUS`
    /// charges, rolled when it enters the dungeon. Today: `2d6 + 1`, i.e. 3..13.
    pub const CHARGE_DICE: i32 = 2;
    /// See [`CHARGE_DICE`].
    pub const CHARGE_SIDES: i32 = 6;
    /// See [`CHARGE_DICE`].
    pub const CHARGE_BONUS: i8 = 1;

    /// A *zapped* attack wand deals `DAMAGE_DICE d DAMAGE_SIDES`, armour-ignoring,
    /// rolled once and applied whole to everyone it touches. Today: `3d3`.
    pub const DAMAGE_DICE: i32 = 3;
    /// See [`DAMAGE_DICE`].
    pub const DAMAGE_SIDES: i32 = 3;

    /// Radius, in tiles, of a zapped fire/cold wand's blast disc, and of a
    /// *thrown* utility wand's (damage-free) effect disc.
    ///
    /// **Relationship to keep in mind:** for years the thrown "grenade" was
    /// defined as exactly twice this. It no longer is — [`GRENADE_RADIUS`] is
    /// set independently — but the two are still meant to read as
    /// "small blast" vs. "room-clearing blast". Keep grenade > blast.
    pub const BLAST_RADIUS: f32 = 2.0;

    /// Radius, in tiles, of the blast a *thrown* attack wand (or the wand of
    /// light) makes when it bursts on impact — the grenade. Wider and hotter
    /// than a zap, and it does not care who set it off: lob one too close and
    /// it burns you too.
    pub const GRENADE_RADIUS: f32 = 3.0;

    /// A thrown wand spends *every* remaining charge at once. An attack-wand
    /// grenade rolls this many sides per charge...
    pub const GRENADE_DIE_PER_CHARGE: i32 = 4;
    /// ...and a thrown utility wand's blast rolls this many (it deals no damage,
    /// but the roll still drives the animation's reach). See
    /// [`crate::items`]`::resolve_wand_throw`.
    pub const EFFECT_DIE_PER_CHARGE: i32 = 3;

    /// How many turns a fire blast's smoke lingers on the tiles it covered,
    /// DCSS-style — purely cosmetic, never blocks movement or sight. Zapped
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
    /// table in that function's doc comment and in
    /// `docs/reference/content-tables.md` is prose — update it too.
    pub const NORMAL_QUALITY_PCT: i32 = 25;

    /// Percent chance a gear drop is exceptional — a clean `+1..+3`. See
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

    /// A dropped ammunition bundle holds this many, uniformly — never a lone
    /// arrow, because finding one arrow is not finding ammunition. Capped by
    /// [`crate::constants::items::STACK_LIMIT`] once it lands in a pack slot.
    pub const AMMO_BUNDLE_MIN: i32 = 3;
    /// See [`AMMO_BUNDLE_MIN`].
    pub const AMMO_BUNDLE_MAX: i32 = 12;

    /// A launcher (bow, crossbow) multiplies the die of the ammunition it looses.
    /// This is the whole point of the bow — see the "price of a hand" section of
    /// `combat-and-balance.md`. At 1 a bow does nothing; above 2 it eclipses
    /// every melee weapon.
    pub const LAUNCHER_DIE_MULTIPLIER: i32 = 2;
}

// ===========================================================================
// Items in the hand and in the pack
// ===========================================================================

/// Throw range and stack size.
pub mod items {
    /// How far any item can be hurled, in tiles — the throw reticle's leash. A
    /// wand overrides this with its own `range` when zapped, but a *thrown*
    /// wand still obeys this leash.
    pub const THROW_RANGE: i32 = 7;

    /// The most one pack slot will hold before the overflow spills into a
    /// second slot. A round, generous number — nothing in the engine forces a
    /// particular value.
    ///
    /// **If you change it:** `models/tests/missiles.rs` and
    /// `docs/reference/content-tables.md` name the current value and would need
    /// a pass.
    pub const STACK_LIMIT: u8 = 26;

    /// The most inventory slots a pack will hold at once. Kept below 26 so
    /// every slot's letter stays inside `w`/`s`, which the pack menu already
    /// reserves for up/down navigation and so can never reach a letter row.
    pub const PACK_CAPACITY: usize = 13;
}

// ===========================================================================
// Monsters
// ===========================================================================

/// Bestiary-wide defaults. Per-creature stats are rows in `monsters.rs`.
pub mod monsters {
    /// The spawn weight a bestiary row gets when it doesn't `.weight(n)` for
    /// itself. Every default-weight creature is equally likely; a row asking
    /// for less is rarer, more is more common. Relative only — the absolute
    /// value just sets the granularity.
    pub const DEFAULT_SPAWN_WEIGHT: u32 = 10;
}

// ===========================================================================
// Travel assists (autoexplore / fast-move)
// ===========================================================================

/// Safety valves on the "keep walking" commands, so a pathfinding bug loops a
/// bounded number of times instead of forever.
pub mod travel {
    /// Hard stop on a single autoexplore invocation.
    pub const AUTO_EXPLORE_STEP_CAP: u32 = 5000;

    /// Hard stop on a single fast-move (travel-to-cursor / run) invocation.
    pub const FAST_MOVE_STEP_CAP: u32 = 1000;
}

// ===========================================================================
// HUD / message log
// ===========================================================================

/// Sizing for the on-screen log. These are display, not balance, but they live
/// here because they are the kind of number people fiddle with and they are
/// coupled to [`crate::constants::map::WIDTH`].
pub mod hud {
    /// Message-log lines shown at the bottom of the screen at once. More lines
    /// means less playfield unless the terminal is tall.
    pub const LOG_LINES: usize = 3;

    /// Wrap width for a log line in the normal view. Kept equal to the map
    /// width so the log spans the playfield exactly.
    pub const LOG_WIDTH: usize = 80;

    /// Wrap width for a log line in the "-- more --" backlog pager, which is
    /// inset from the edges.
    pub const LOG_MORE_WIDTH: usize = 56;
}
