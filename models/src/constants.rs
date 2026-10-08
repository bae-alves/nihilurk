//! Every tuning knob in the model, in one place.
//!
//! nihilurk's balance lives in a few dozen numbers scattered across a dozen
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
//!   the game by touching them. The shake table is the exception: [`shake`].
//! * **Content-table numbers** — a monster's HP, a weapon's die, a potion's
//!   colour. Those are data, one row per thing, and live in `catalog.rs` /
//!   `monsters.rs` / `traps.rs`. See `docs/reference/content-tables.md`.
//! * **RNG salts** and the neighbour-offset tables. Structural, not balance.
//!   The grid and special-level dials are here ([`layout`], [`special_levels`]),
//!   with the caveat that moving one moves every layout on every seed.
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
    /// `Nd[power]`, summed, *before* armour is subtracted. The more dice, the
    /// higher a lucky swing's ceiling.
    pub const EXCELLENT_HIT_DICE: i32 = 3;

    /// The floor under a player's swing: even when the armour roll eats the
    /// whole blow, the player still scrapes off this much (logged as a
    /// "glancing blow"). It keeps a fight from never moving the number.
    ///
    /// A glancing blow can never be the killing one — it leaves a foe on 1 HP —
    /// so raising this does not let chip damage finish things, only speeds the
    /// grind. Monsters have no such floor: a monster that cannot beat your
    /// armour simply cannot hurt you, and that asymmetry is why armour is worth
    /// wearing. Leave it at its minimum unless you are re-teaching that lesson.
    pub const CHIP_DAMAGE: i32 = 1;

    /// The odds that any one piece of a dead creature's gear survives to be
    /// picked up; the rest is lost in the mess. Rolled once per equipped item.
    /// Lower this and thrown-weapon retrieval gets stingier; raise it toward 1
    /// and every orc becomes a vending machine for its own sword.
    pub const GEAR_SURVIVES_DEATH: f64 = 0.5;

    /// The flat bonus the spell Bide adds to the attack roll of the very next
    /// blow its caster lands — see [`crate::effects::Bided`].
    pub const BIDE_ATTACK_BONUS: i32 = 4;

    /// How much harder a monster's melee blow lands on a creature that is
    /// [`crate::effects::Vuln`], in percent of the damage the dice settled on.
    pub const VULN_DAMAGE_PERCENT: i32 = 25;

    /// How many dice a normal attack or armour roll averages together —
    /// see `crate::combat::roll_die_bell`. At 1 it is a flat `1d[sides]`
    /// like an excellent hit; raising it narrows the spread further without
    /// moving the mean or the min/max off the plain die's.
    pub const BELL_CURVE_DICE: i32 = 2;

    /// How far the splinters of a cursed suit fly when an excellent hit
    /// shatters it, in tiles (Chebyshev). See [`crate::combat::resolve_attack`].
    pub const CURSED_SPLINTER_RADIUS: i32 = 2;

    /// The die each splinter rolls against every foe of the hero it reaches:
    /// `1d[this]`, as magic, so a ward turns it aside.
    pub const CURSED_SPLINTER_DIE: i32 = 6;
}

// ===========================================================================
// The player
// ===========================================================================

/// The hero's opening line. These seed a *new* game only — a loaded save
/// carries its own numbers (see `saveload.rs`), so changing them never touches
/// a run in progress.
pub mod player {
    /// Starting (and maximum) hit points. The whole bestiary is tuned against a
    /// player on this much HP — see `combat-and-balance.md` — so raising this quietly
    /// makes the early floors safer across the board. There is no level-up:
    /// this is the number for the whole run, bar potions of raise level.
    pub const START_HP: i32 = 12;

    /// Starting armour die (`1d[armor]` on the defence roll), before the +1 ring
    /// mail the hero spawns wearing. Matches an unarmoured townsperson.
    pub const START_ARMOR: i32 = 3;

    /// Starting power die (`1d[power]` on the attack roll), before the +1 mace.
    pub const START_POWER: i32 = 3;

    /// Starting (and maximum) magic points — the pool abilities draw on, shown
    /// as `Ma X/Y` on the HUD. Refilled in full by every staircase, alongside
    /// the arrival heal ([`crate::constants::progression::DESCENT_HEAL_DIVISOR`]).
    pub const START_MAGIC: u8 = 4;

    /// Sight radius, in tiles, for the player's Viewshed. Rooms flood-fill on
    /// top of this; corridors and dark rooms are cut back to the always-on 3x3.
    ///
    /// A loaded save keeps the range it was saved with (`saveload.rs` restores
    /// it), so this too only affects new games.
    pub const SIGHT_RANGE: u16 = 12;

    /// Fraction of max HP at or below which the player gets a one-time "badly
    /// wounded" warning as they cross down into it. See
    /// `crate::helpers::apply_damage`.
    pub const LOW_HP_WARNING_FRACTION: f32 = 0.3;
}

/// The lurk: the other playable species. Quadruped, fanged, clawed, furred,
/// and carrying nothing it did not grow itself.
///
/// Its two dice start exactly where nihil's *bare* ones do
/// ([`player::START_POWER`], [`player::START_ARMOR`]): the difference is that
/// nihil walks out of the gate holding a mace and wearing ring mail and the
/// lurk never will, so where nihil's numbers are bought, the lurk's are
/// eaten. Less meat and less magic than nihil to pay for the technique it is
/// born with (see `crate::body::wear_lurk`).
pub mod lurk {
    /// Starting (and maximum) hit points. Well under [`super::player::START_HP`].
    pub const START_HP: i32 = 7;

    /// Starting (and maximum) magic points. Little to spend until it grows.
    pub const START_MAGIC: u8 = 2;

    /// Starting attack die (`1d[power]`) — its claws, and no weapon will ever
    /// add to them.
    pub const START_POWER: i32 = 2;

    /// Starting defence die (`1d[armor]`) — its fur, and no armour will ever
    /// add to it.
    pub const START_ARMOR: i32 = 2;

    /// Odds that one corpse feeds the lurk, rolled per death. Rolled rather
    /// than counted, so a growth is something
    /// that *happens* to a hunt and never something to count down to.
    pub const GROWTH_CHANCE: f64 = 0.15;

    /// What one growth is worth, on whichever of the four numbers it lands.
    pub const GROWTH_STEP: i32 = 1;

    /// How many times the lurk throws each attack: the bite and the second
    /// snap of the jaws.
    pub const NUMBER_OF_ATTACKS: u8 = 2;
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
    /// `docs/reference/content-tables.md` and `gdd.md` name the final depth in
    /// prose and would need a pass. Monster/trap `min_depth` values in the catalog
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
    pub const DESCENT_HEAL_DIVISOR: i32 = 3;

    /// The last floor of each floor-crowding tier below the deepest one. The
    /// monster and trap *budgets* (`constants::population`, spent in `map.rs`)
    /// gain a slot and widen their fill odds at each of these depths. Each entry
    /// closes a tier, and the floors after the last entry, up to
    /// [`FINAL_DEPTH`], are the hardest band.
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
    /// Playfield width in tiles. `determinism.rs` pins the layout for existing
    /// seeds — changing either dimension moves every wall on every seed and
    /// that test will (correctly) fail. Only change these
    /// together with a deliberate "all old seeds are void" decision.
    pub const WIDTH: u16 = 80;

    /// Playfield height in tiles. The terminal must fit this plus the HUD; see
    /// the portability note in `gdd.md`. Same determinism caveat as [`WIDTH`].
    pub const HEIGHT: u16 = 22;

    /// Chance that a given room past the starting one spawns unlit. A dark room
    /// behaves like a corridor (sight cut to the 3x3) until a wand of light
    /// goes off in it. Raise for a darker, more wand-of-light-dependent game.
    pub const DARK_ROOM_CHANCE: f64 = 0.15;

    /// Chance a room past the starting one is entirely coin, guarded by a
    /// forced dragon or two. See [`crate::map::SpecialRoom::DragonHoard`].
    pub const DRAGON_HOARD_CHANCE: f64 = 0.0125;

    /// Chance a room past the starting one is packed wall to wall with
    /// monsters. See [`crate::map::SpecialRoom::MonsterZoo`].
    pub const MONSTER_ZOO_CHANCE: f64 = 0.0125;

    /// Chance a room past the starting one is entirely coin, guarded by one
    /// apis. See [`crate::map::SpecialRoom::TreasureHive`].
    pub const TREASURE_HIVE_CHANCE: f64 = 0.0125;

    /// Chance a room past the starting one is a hive when the player is an
    /// apis (`-am apis`), the only special room such a run has.
    pub const BEE_RUN_HIVE_CHANCE: f64 = 0.10;

    /// Chance a room past the starting one holds a normal item budget that
    /// vanishes down to one the moment any of it is picked up. See
    /// [`crate::map::SpecialRoom::RedRoom`].
    pub const RED_ROOM_CHANCE: f64 = 0.05;

    /// The shallowest floor a special level can replace (see
    /// [`crate::map::SpecialLevel`]). The deepest floor never is one: it is
    /// where the Element of Yoord waits.
    pub const SPECIAL_LEVEL_MIN_DEPTH: u8 = 6;

    /// Chance an eligible floor is one open room, wall to wall. See
    /// [`crate::map::SpecialLevel::Battlefield`].
    pub const BATTLEFIELD_CHANCE: f64 = 0.05;

    /// Chance an eligible floor is a maze of passages. See
    /// [`crate::map::SpecialLevel::Labyrinth`].
    pub const LABYRINTH_CHANCE: f64 = 0.01;

    /// Chance an eligible floor is a treasure vault of doored cells. See
    /// [`crate::map::SpecialLevel::Vault`].
    pub const VAULT_CHANCE: f64 = 0.01;

    /// Chance an eligible floor is one yellow cave full of apis. See
    /// [`crate::map::SpecialLevel::BeeWorld`].
    pub const BEE_WORLD_CHANCE: f64 = 0.01;

    /// Chance an eligible floor has a castle in the middle of it. See
    /// [`crate::map::SpecialLevel::Castle`].
    pub const CASTLE_CHANCE: f64 = 0.01;

    /// Chance an eligible floor is an island in deep water. See
    /// [`crate::map::SpecialLevel::Island`].
    pub const ISLAND_CHANCE: f64 = 0.01;
}

// ===========================================================================
// How crowded a floor is
// ===========================================================================

/// The monster, trap and item budgets a floor spends when it is populated.
/// Everything here scales with `map::difficulty_tier(depth)` — `0` up to the
/// hardest band, stepping at [`crate::constants::progression::DIFFICULTY_TIER_LAST_DEPTH`] —
/// so the dungeon gets nastier in bands as you descend.
pub mod population {
    /// Monster slots on floor 1, before the per-tier bonus. The first slot
    /// always fills; the rest roll [`MONSTER_FILL_CHANCE_BASE`].
    pub const MONSTER_SLOTS_BASE: usize = 4;

    /// Chance a non-first monster slot actually spawns something, at tier 0.
    pub const MONSTER_FILL_CHANCE_BASE: f64 = 0.80;

    /// Added to the monster fill chance per tier.
    pub const MONSTER_FILL_CHANCE_PER_TIER: f64 = 0.05;

    /// Ceiling on the monster fill chance, so a slot is never quite certain.
    pub const MONSTER_FILL_CHANCE_CAP: f64 = 0.95;

    /// Trap slots on floor 1, before the per-tier bonus.
    pub const TRAP_SLOTS_BASE: usize = 4;

    /// Chance a trap slot produces a trap, at tier 0.
    pub const TRAP_FILL_CHANCE_BASE: f64 = 0.40;

    /// Added to the trap fill chance per tier.
    pub const TRAP_FILL_CHANCE_PER_TIER: f64 = 0.10;

    /// Ceiling on the trap fill chance.
    pub const TRAP_FILL_CHANCE_CAP: f64 = 0.95;

    /// From this depth on, every corridor centre has a small chance
    /// ([`CORRIDOR_LURKER_CHANCE`]) of hiding a monster right in the traveller's
    /// path.
    pub const CORRIDOR_LURKER_MIN_DEPTH: u8 = 7;

    /// Per-corridor chance of a mid-corridor lurker, once past
    /// [`CORRIDOR_LURKER_MIN_DEPTH`].
    pub const CORRIDOR_LURKER_CHANCE: f64 = 0.15;

    /// Ordinary item drops attempted on floor 1, before the per-tier bonus.
    /// Like the monster and trap budgets, the item budget gains one attempt per
    /// `map::difficulty_tier(depth)` — `ITEM_SLOTS_BASE + tier` tries, each of
    /// which still needs a free tile. Every attempt that lands a tile drops an
    /// item (no fill roll — deeper floors are simply richer).
    pub const ITEM_SLOTS_BASE: usize = 4;

    /// How often a floor also hides one extra item — no glyph, no
    /// announcement — until a ring of perception turns it up, a detection
    /// finds it, or the player walks onto it. At `1.0` every floor does; lower
    /// it to make a stash something worth hoping for rather than something to
    /// sweep for.
    pub const HIDDEN_ITEM_CHANCE: f64 = 0.75;

    /// On a floor that is one room and nothing else — a battlefield, a
    /// labyrinth, a bee world, an island — every tile within this many steps
    /// of where the player lands is kept clear of monsters and loot: the
    /// start room every other floor already has. Raise it for a gentler
    /// arrival.
    pub const START_CLEARING_RADIUS: u16 = 4;

    /// How many times a floor with no special level (and the labyrinth, castle
    /// and island, which promise no crowd) runs each of the monster and item
    /// budgets: once, the ordinary floor.
    pub const ORDINARY_BUDGET_RUNS: usize = 1;

    /// A battlefield runs the monster budget this many times: one lit room, all
    /// of it in view, so it needs the crowd to be a fight at all.
    pub const BATTLEFIELD_MONSTER_RUNS: usize = 2;

    /// A battlefield runs the item budget this many times.
    pub const BATTLEFIELD_ITEM_RUNS: usize = 2;

    /// A vault runs the monster budget this many times.
    pub const VAULT_MONSTER_RUNS: usize = 2;

    /// A vault runs the item budget this many times: the haul is the point.
    pub const VAULT_ITEM_RUNS: usize = 3;

    /// A bee world runs the item budget this many times (its monster budget
    /// runs once per difficulty tier instead).
    pub const BEE_WORLD_ITEM_RUNS: usize = 3;

    /// How many times a placement will re-roll before giving the slot up.
    ///
    /// Ten, not a hundred. A floor is a few hundred open tiles holding a dozen
    /// things, so the first draw nearly always lands somewhere free and the budget
    /// is never spent; the only floors that reach the end of it are ones so
    /// crowded that the eleventh try would not have helped either. Spending a
    /// hundred draws to find that out costs the same seeded RNG stream everything
    /// else on the floor draws from, for a slot the dungeon is happy to skip.
    pub const PLACEMENT_TRIES: usize = 10;
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
    /// ([`crate::constants::progression::DIFFICULTY_TIER_LAST_DEPTH`]), so the
    /// arrow and dart step up fewer times over a run. Read by
    /// `traps::trap_damage_tier`.
    pub const TRAP_DAMAGE_TIER_LAST_DEPTH: [u8; 2] = [4, 8];

    /// An arrow trap's hit rolls `ARROW_DAMAGE_DICE d ARROW_DAMAGE_SIDES +
    /// ARROW_DAMAGE_BONUS` before armour-plus.
    pub const ARROW_DAMAGE_DICE: i32 = 1;
    /// See [`ARROW_DAMAGE_DICE`].
    pub const ARROW_DAMAGE_SIDES: i32 = 2;
    /// See [`ARROW_DAMAGE_DICE`].
    pub const ARROW_DAMAGE_BONUS: i32 = 1;

    /// Added to an arrow trap's damage roll for each depth tier past the first.
    pub const ARROW_DAMAGE_PER_TIER: i32 = 1;

    /// A dart trap's hit rolls `DART_DAMAGE_DICE d DART_DAMAGE_SIDES` before
    /// armour-plus.
    pub const DART_DAMAGE_DICE: i32 = 1;
    /// See [`DART_DAMAGE_DICE`].
    pub const DART_DAMAGE_SIDES: i32 = 1;

    /// Permanent melee power a dart trap drains on a hit in the first depth
    /// tier; each deeper tier drains [`DART_POWER_DRAIN_PER_TIER`] more. Floored so `power` never drops to nothing, and
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

    /// How often a trap that has just sprung gives out — the mechanism is spent
    /// and the tile is clear again ("The dart trap breaks!"). The bear trap is
    /// exempt: it always bites once and is done. Raise it to make a floor's
    /// traps a one-time toll, lower it to make the same tile a lasting hazard.
    pub const TRAP_BREAK_CHANCE: f64 = 0.25;

    /// Radius, in tiles, of the burst a trap makes when something sets it off
    /// from a distance — `1` is the 3×3 around it. See `traps::detonate_trap`.
    pub const TRICK_SHOT_RADIUS: i32 = 1;

    /// The reach of a trick shot set off on something that is not a trap — a
    /// coin, or the Element of Yoord itself. Wider than a trap's on purpose:
    /// a trap has a mechanism to let go and this has only the shot, so the
    /// spectacle is all there is to it. `2` is the 5x5 around the tile.
    pub const PICKUP_TRICK_SHOT_RADIUS: i32 = 2;

    /// A trick shot's burst deals `TRICK_SHOT_DAMAGE_DICE d
    /// TRICK_SHOT_DAMAGE_SIDES`, rolled once and applied whole to everything
    /// caught — no armour of any kind is subtracted, not even the armour plus
    /// that a trap's own damage still allows.
    pub const TRICK_SHOT_DAMAGE_DICE: i32 = 2;
    /// See [`TRICK_SHOT_DAMAGE_DICE`].
    pub const TRICK_SHOT_DAMAGE_SIDES: i32 = 3;
}

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

    /// The chance a paralysed player's turn is forfeited outright, on top of the
    /// slowing paralysis already imposes. Rolled once per turn — see
    /// [`crate::conditions::paralysis_forfeits_turn`].
    pub const PARALYSIS_LOST_TURN_CHANCE: f64 = 0.5;

    /// A potion doesn't just dose whoever it hits: it breaks and spreads over
    /// this radius, the same delivery a thrown utility wand's blast uses (see
    /// [`crate::constants::wands::BLAST_RADIUS`]), just narrower — a potion is
    /// a mouthful of glass, not a wand's whole battery.
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
    pub const HOLD_TURNS: u32 = 8;
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
    pub const BALA_POWER: i32 = 5;

    /// What `BOLE` adds to its holder's armour roll.
    pub const BOLE_ARMOR: i32 = 5;

    /// How many turns THE WORLD stops time for.
    pub const WORLD_TURNS: u32 = 5;

    /// How many casts of their spell the Skull King and the Black Mage are
    /// born with. They spam it.
    pub const CARD_CASTER_CASTS: u8 = 12;

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
    pub const SYSTEM_SHOCK_CHANCE: f64 = 0.5;

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
    pub const BLAST_RADIUS: f32 = 2.0;

    /// Radius, in tiles, of the blast a *thrown* attack wand (or the wand of
    /// light) makes when it bursts on impact — the grenade. Wider and hotter
    /// than a zap, and it does not care who set it off: lob one too close and
    /// it burns you too.
    pub const GRENADE_RADIUS: f32 = 3.0;

    /// A thrown wand spends *every* remaining charge at once. An attack-wand
    /// grenade rolls one die of this many sides per charge — at 1, a flat
    /// point a charge...
    pub const GRENADE_DIE_PER_CHARGE: i32 = 1;
    /// ...and a thrown utility wand's blast rolls this many (it deals no damage,
    /// but the roll still drives the animation's reach). See
    /// `crate::items``::resolve_wand_throw`.
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

    /// Percent chance a curse merge (equipping a known cursed item over a worn
    /// cursed one in the same slot) leaves the worn item's form.
    pub const MERGE_RECIPIENT_PCT: i32 = 45;
    /// Percent chance a curse merge leaves the newly equipped item's form. The
    /// remainder after this and [`MERGE_RECIPIENT_PCT`] is both items breaking.
    pub const MERGE_DONOR_PCT: i32 = 45;

    /// The die a thrown thing rolls when it was not made for throwing: a mace,
    /// or an arrow lobbed by hand. Only a purpose-built missile
    /// ([`crate::components::Projectile`]) or a launcher does better.
    pub const IMPROVISED_THROW_DIE: i32 = 3;

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
    /// **If you change it:** `models/tests/missiles.rs` and
    /// `docs/reference/content-tables.md` name the current value and would need
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
    pub const STEALTH_RANGE: i32 = 3;

    /// Magic points one deliberate teleport costs a wearer of the ring of
    /// teleportation.
    pub const TELEPORT_MAGIC_COST: u8 = 2;

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
// Score
// ===========================================================================

/// What the number on the HUD is made of. The verbs are in `score.rs`, which
/// documents the whole table in one place.
pub mod score {
    /// Score paid per point of a slain creature's `max_hp`. A kestral is a
    /// rounding error next to a griffin, which is the intent: the scoreboard
    /// rewards fighting things that could have killed you.
    pub const KILL_PER_MAX_HP: i32 = 100;

    /// What each corpse past the first adds to a turn's kill score, as a
    /// fraction of the whole pile: the turn's kills are multiplied by one plus
    /// this for each extra corpse, applied to the kills together, not to the
    /// last one alone. This is the dial that decides whether a thrown wand is worth more
    /// than the same six kills one at a time.
    pub const COMBO_BONUS_PER_KILL: f32 = 0.5;

    /// How often a combo is logged as done "With pride." instead of the usual
    /// "With style." Rare on purpose: a line that shows up every other fight
    /// stops being one.
    pub const COMBO_PRIDE_CHANCE: f64 = 0.05;

    /// Score paid per difficulty tier every time a staircase is used, counting
    /// the shallowest band as tier one so the first flight still pays. Raise it
    /// and diving outscores clearing; lower it and the reverse.
    pub const STAIR_PER_TIER: i32 = 500;

    /// Turns a payment stays lit on the scorekeeper. The default is one full
    /// frame of screen time: the flash is aged at the tail of the turn it was armed in,
    /// shown by that turn's render, and dark by the player's next action.
    pub const SCORE_FLASH_TURNS: u8 = 2;

    /// The kill-score multiplier a creature carrying
    /// [`crate::effects::ScoreBounty`] pays out — the apis guarding a
    /// treasure hive is worth several ordinary rattlesnakes.
    pub const BOUNTY_SCORE_MULTIPLIER: i32 = 5;
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
    pub const DEFAULT_SPAWN_WEIGHT: u32 = 20;

    /// How many turns a medusa's gaze leaves the player standing as stone.
    /// Long enough to be the fight's whole shape and short enough to live
    /// through — nothing can kill a petrified player but a war hammer, so this
    /// is a toll in turns rather than in HP. See
    /// `crate::abilities::medusa_gaze`.
    pub const PETRIFY_TURNS: u32 = 5;

    /// The ice monster's odds, on a blow that lands, of paralysing what it hit.
    pub const ICE_MONSTER_PARALYZE_CHANCE: f64 = 3.0 / 6.0;

    /// The lowest of the three chances a bestiary row hands its
    /// [`EquipRoll`](crate::monsters::EquipRoll)s.
    pub const NORMAL_GEAR_CHANCE: f64 = 0.30;
    /// The middle gear chance, for a species that is more often armed.
    pub const HIGH_GEAR_CHANCE: f64 = 0.60;
    /// The highest gear chance, for a species that is armed nearly every time.
    pub const ULTIMATE_GEAR_CHANCE: f64 = 0.90;

    /// How many points of base power a rattlesnake's bite drains — permanently,
    /// and unlike the dart trap's, with no floor: a rattlesnake can drive a
    /// victim's power negative.
    pub const RATTLESNAKE_POWER_DRAIN: i32 = 1;

    /// How many points of max HP a vampire's touch drains per hit.
    pub const VAMPIRE_MAX_HP_DRAIN: i32 = 2;

    /// How far a launcher-wielding monster (a centaur, a medusa) can loose a
    /// shot. Shares the player's own launcher reach.
    pub use crate::constants::items::LAUNCHER_RANGE as MONSTER_SHOT_RANGE;
}

// ===========================================================================
// Spirits
// ===========================================================================

/// The neutral faction. See [`crate::spirits`].
pub mod spirits {
    /// How far [`crate::components::Alignment`] can drift toward either
    /// pole. Reaching it flips [`crate::components::SpiritsHostile`] for
    /// good.
    pub const ALIGNMENT_POLE: i8 = 3;

    /// The odds, per piece of gear the pink demon destroyed, that they join
    /// you as your Helper. Enough pieces make it certain.
    pub const PINK_DEMON_ODDS_PER_PIECE: f64 = 0.25;

    /// What the red demon charges for one piece of gear, in Max HP. Paying
    /// your last Max HP kills you.
    pub const RED_DEMON_GEAR_PRICE: i32 = 3;

    /// What the gnome charges, in Max Ma: a scroll, a potion, a wand.
    pub const GNOME_SCROLL_PRICE: u8 = 1;
    /// What the gnome charges for a potion, in Max Ma.
    pub const GNOME_POTION_PRICE: u8 = 1;
    /// What the gnome charges for a wand, in Max Ma.
    pub const GNOME_WAND_PRICE: u8 = 2;

    /// The spawn weight every spirit row gets: a twentieth of an ordinary
    /// monster's default, and the smallest weight there is. Rarer means
    /// raising [`crate::constants::monsters::DEFAULT_SPAWN_WEIGHT`] and every
    /// explicit `.weight(n)` with it.
    pub const SPAWN_WEIGHT: u32 = 1;

    /// How many things a barterer lays on the table: the yellow demon's
    /// pack, the sphynx's spells. Inclusive on both ends.
    pub const BARTER_STOCK_MIN: usize = 2;
    /// The most things a barterer lays out; see [`BARTER_STOCK_MIN`].
    pub const BARTER_STOCK_MAX: usize = 4;

    /// How far one spirit's poof moves [`crate::components::Alignment`]: toward
    /// the cacodaemon pole for a demon, toward the eudaemon pole for an angel
    /// or kin.
    pub const ALIGNMENT_STEP: i8 = 1;

    /// What gaining a Helper does to [`crate::components::Alignment`].
    pub const HELPER_GAINED_ALIGNMENT: i8 = 1;

    /// What losing a Helper to an explosion or a system shock does to
    /// [`crate::components::Alignment`].
    pub const HELPER_BLOWN_UP_ALIGNMENT: i8 = -2;

    /// The angel's test of faith takes the player's current HP down by this
    /// divisor (never below one point), on top of cancelling every effect.
    pub const TEST_OF_FAITH_HP_DIVISOR: i32 = 2;

    /// What the test of faith pays off with on the weapon and the armour: a
    /// flat enchantment, and the curse lifted.
    pub const TEST_OF_FAITH_GEAR_BONUS: i32 = 3;

    /// What a dud equipped item (no enchantment at all) rerolls to when the test
    /// of faith pays off.
    pub const TEST_OF_FAITH_DUD_BONUS: i32 = 1;
}

// ===========================================================================
// Helpers
// ===========================================================================

/// The boon companion. See [`crate::companion`].
pub mod helpers {
    /// The odds a creature accepts the right treat and becomes your Helper.
    /// The treat is eaten either way.
    pub const ACCEPT_CHANCE: f64 = 0.5;

    /// The odds a kill turns a [`crate::effects::ShapeshiftOnKill`] creature
    /// into another monster, or a [`crate::effects::MirrorOnKill`] one into
    /// what it killed. Either way it happens once.
    pub const SHAPESHIFT_CHANCE: f64 = 0.25;

    /// Scroll of create monster: the odds the creature conjured arrives
    /// already charmed, as a plain ally. Rolled against the same draw as
    /// [`CREATE_HELPER_CHANCE`] — the two never both land on one summon.
    pub const CREATE_ALLY_CHANCE: f64 = 0.13;

    /// Scroll of create monster: the odds the creature conjured arrives as
    /// your Helper outright, rather than a plain ally or a plain monster.
    pub const CREATE_HELPER_CHANCE: f64 = 0.07;
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

// ===========================================================================
// Travel assists (autoexplore / fast-move)
// ===========================================================================

/// Safety valves on the "keep walking" commands, so a pathfinding bug loops a
/// bounded number of times instead of forever.
pub mod travel {
    /// Hard stop on a single autoexplore invocation.
    pub const AUTO_EXPLORE_STEP_CAP: u32 = 260;

    /// Hard stop on a single fast-move (travel-to-cursor / run) invocation.
    pub const FAST_MOVE_STEP_CAP: u32 = 260;

    /// How far off, in tiles, a creature can be and still be charged.
    pub const CHARGE_RANGE: i32 = 6;

    /// Auto-fight refuses once the player's HP is at or below `max_hp` divided
    /// by this. Kept as a divisor so the check stays in integer maths.
    pub const AUTO_FIGHT_MIN_HP_DIVISOR: i32 = 4;
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
    pub const LOG_LINES: usize = 2;

    /// Column the command bar's text starts at. The bar fills the screen's
    /// bottom three rows and leaves two blank columns before [`LOG_X`], so its
    /// lines hold up to `LOG_X - 3` characters (the longest, Spanish, is 23).
    pub const BAR_X: u16 = 1;

    /// Column the log and the player line start at: right of the command bar
    /// and its divider. Everything to its left on the bottom three rows is
    /// the bar.
    pub const LOG_X: u16 = 26;

    /// Wrap width for a log line once the command bar is hidden: the whole
    /// screen, which is the map's width.
    pub const LOG_FULL_WIDTH: usize = super::map::WIDTH as usize;

    /// Wrap width for a log line in the normal view: the screen's width less
    /// the command bar.
    pub const LOG_WIDTH: usize = LOG_FULL_WIDTH - LOG_X as usize;

    /// Columns the "-- more --" prompt takes from the last log line: its 22
    /// plus a space. The pager wraps that line this much narrower.
    pub const MORE_PROMPT_WIDTH: usize = 23;

    /// How many past message-log lines the scrollback keeps before the oldest
    /// falls off.
    pub const LOG_HISTORY_CAP: usize = 50;
}

// ===========================================================================
// Passive abilities
// ===========================================================================

/// The odds and the step behind the abilities that act by themselves (the
/// `ABILITIES` table in `crate::abilities`). The effect markers carry no
/// numbers; the table row names one of these.
pub mod abilities {
    /// A ring of aggravate monster: the odds, on each turn its bearer acts, that
    /// everything on the floor learns where they are.
    pub const AGGRAVATES_MONSTERS_CHANCE: f64 = 0.10;

    /// A ring of regeneration: the odds, on each turn its bearer acts, that it
    /// tries to mend something.
    pub const REGENERATES_CHANCE: f64 = 0.50;

    /// Rogue's teleportitis, at NetHack's odds, per turn the bearer acts. The
    /// jump lands at the top of their next turn.
    pub const TELEPORTITIS_CHANCE: f64 = 1.0 / 85.0;

    /// A ring of polymorph: the odds, per turn the bearer acts, that it turns
    /// them into something else.
    pub const POLYMORPHITIS_CHANCE: f64 = 1.0 / 83.0;

    /// What each landed hit of a rapier (or a lurk's own technique) adds to
    /// [`crate::effects::Momentum`].
    pub const MOMENTUM_PER_HIT: i32 = 2;
}

// ===========================================================================
// Conditions
// ===========================================================================

/// What the afflictions cost. Paralysis has its own dial in
/// `potions`; confusion's lives here.
pub mod conditions {
    /// While the player is confused, the odds that any one step goes off in a
    /// random direction instead of the intended one.
    pub const CONFUSION_STUMBLE_CHANCE: f64 = 0.5;
}

// ===========================================================================
// Speed
// ===========================================================================

/// The tempo scale: energy banked per player turn by each
/// [`SpeedKind`](crate::components::SpeedKind), and what one action costs. How
/// often a creature acts is its rate against the cost.
pub mod speed {
    /// Energy a `Slow` creature banks per player turn.
    pub const SLOW_RATE: i32 = 1;

    /// Energy a `Normal` creature banks per player turn. The reference tempo.
    pub const NORMAL_RATE: i32 = 2;

    /// Energy a `Quick` creature banks per player turn: the lurk's tempo.
    pub const QUICK_RATE: i32 = 3;

    /// Energy a `Fast` creature banks per player turn.
    pub const FAST_RATE: i32 = 4;

    /// What one action costs, in the same units.
    pub const ACTION_COST: i32 = 2;
}

// ===========================================================================
// How a floor is laid out
// ===========================================================================

/// The grid the ordinary floors are laid out on: how many cells, how much
/// space between them, how small a room may be. Geometry the generator is
/// built around, so a value here moves every layout on every seed, and
/// `determinism.rs` will say so.
pub mod layout {
    /// Cells to a side. Three by three is Rogue's own: enough rooms for a floor to
    /// have a shape, few enough that every one of them is worth visiting.
    pub const SECTIONS: u16 = 3;

    /// Blank tiles between neighbouring cells. Three is what guarantees two rooms
    /// can never share a wall however they are placed inside their cells — which
    /// is what lets this file skip an overlap test entirely.
    pub const GUTTER: u16 = 3;

    /// Blank tiles around the whole playfield, so no room is flush with the edge.
    pub const PADDING: u16 = 1;

    /// The smallest room the generator will place. A room narrower than this reads
    /// as a wide corridor rather than a place.
    pub const MIN_ROOM_W: u16 = 4;

    /// The smallest room height. Three, not four: a 4-high room occupies five
    /// tiles once its walls are counted, which overflows a cell.
    pub const MIN_ROOM_H: u16 = 3;

    /// How many answers the "how many cells are left empty?" roll has: none, one,
    /// two or three. A floor with every cell filled is a floor with no shape, and
    /// one with four missing is barely a floor.
    pub const EMPTY_SECTION_CHOICES: usize = 4;
}

// ===========================================================================
// How the special levels are carved
// ===========================================================================

/// The dials of each special level's carving: the labyrinth's loops, the
/// vault's lattice, the bee world's caves, the castle's keep, the island's
/// shore. Same caveat as [`layout`]: a value here moves that level's layout on
/// every seed.
pub mod special_levels {
    /// Chance each wall still standing between two maze cells is knocked through
    /// once the dig is done. A perfect maze has exactly one way anywhere; this is
    /// what gives it a few more.
    pub const LABYRINTH_LOOP_CHANCE: f64 = 0.1;

    /// Rows of cells in a vault's honeycomb, and cells across each unshifted row.
    /// Every other row sits half a cell over and carries one more, cut in half
    /// by the wall at either end, which is what makes the rows interlock like a
    /// hive's.
    pub const VAULT_ROWS: i32 = 3;

    /// Cells across each unshifted row: see [`VAULT_ROWS`].
    pub const VAULT_COLS: i32 = 6;

    /// How far a cell's centre may wander off the lattice, in tiles — sideways,
    /// then up or down. Enough that no two vaults are the same hive.
    pub const VAULT_JITTER_X: i32 = 2;

    /// How far a cell's centre may wander up or down: see [`VAULT_JITTER_X`].
    pub const VAULT_JITTER_Y: i32 = 1;

    /// What a tile of vertical distance counts for against a tile of horizontal,
    /// when deciding which cell a tile belongs to. A terminal cell is about twice
    /// as tall as it is wide; two keeps the cells hex-shaped on screen rather
    /// than tall slivers.
    pub const VAULT_STRETCH: i32 = 2;

    /// Share of a bee world's interior that starts out as rock, before smoothing
    /// turns the noise into caves.
    pub const BEE_ROCK_CHANCE: f64 = 0.43;

    /// Smoothing passes: each one makes a tile rock when five or more of the nine
    /// tiles around and including it are rock, and floor otherwise.
    pub const BEE_SMOOTHING_PASSES: usize = 5;

    /// The keep's top-left floor tile. The castle takes the middle column of
    /// Rogue's grid, and this stands the keep in the middle of that column with
    /// room above and below it for the towers.
    pub const KEEP_X: i32 = 37;

    /// The keep's top-left floor row: see [`KEEP_X`].
    pub const KEEP_Y: i32 = 7;

    /// The keep's floor, and each tower's, to a side. A tower that small still
    /// has room for a whole floor's stock.
    pub const KEEP_SIZE: i32 = 7;

    /// Each tower's floor, to a side: see [`KEEP_SIZE`].
    pub const TOWER_SIZE: i32 = 5;

    /// The island's size: the half-width and half-height of its ellipse, each
    /// rolled from its range, in tiles.
    pub const ISLAND_HALF_WIDTH: std::ops::RangeInclusive<i32> = 14..=22;

    /// The island's half-height range, in tiles: see [`ISLAND_HALF_WIDTH`].
    pub const ISLAND_HALF_HEIGHT: std::ops::RangeInclusive<i32> = 5..=7;

    /// How far each row of shore may reach past the true ellipse, or fall short
    /// of it, so the coast never comes out as a clean curve.
    pub const ISLAND_SHORE_JITTER: i32 = 2;
}

// ===========================================================================
// Screen shake
// ===========================================================================

/// How long each kind of shake rocks the map, and how far its first frame
/// throws it. See [`crate::shake::ShakeKind`] for what arms each one, and for
/// why none may last less than two animation frames.
pub mod shake {
    /// How long the player's own landed blow shakes the map, in ms. The floor:
    /// two shaken frames is the least a shake can be and still be one.
    pub const HIT_MS: f32 = 80.0;

    /// Cells the map is thrown on a hit's opening frame.
    pub const HIT_AMPLITUDE: i8 = 1;

    /// How long a death in sight shakes the map, in ms.
    pub const KILL_MS: f32 = 120.0;

    /// Cells the map is thrown on a kill's opening frame.
    pub const KILL_AMPLITUDE: i8 = 1;

    /// How long a blast in sight, or the player's own excellent hit, shakes
    /// the map, in ms.
    pub const HEAVY_MS: f32 = 260.0;

    /// Cells the map is thrown on a heavy shake's opening frame.
    pub const HEAVY_AMPLITUDE: i8 = 2;

    /// How long the room reels when the player is knocked down through the
    /// low-HP warning, in ms.
    pub const WOUNDED_MS: f32 = 460.0;

    /// Cells the map is thrown on a wounded shake's opening frame.
    pub const WOUNDED_AMPLITUDE: i8 = 2;
}

/// The ice cube a cold kill leaves behind, and the kick that sends it flying.
pub mod ice {
    /// A kicked cube's cold damage: `DICE d SIDES`, armour-ignoring magic.
    pub const DAMAGE_DICE: i32 = 2;
    /// See [`DAMAGE_DICE`].
    pub const DAMAGE_SIDES: i32 = 2;

    /// How far, in tiles, a kicked cube with no foe to home on flies before it
    /// gives up and shatters in open air.
    pub const FLIGHT_RANGE: i32 = 40;

    /// How far, in tiles, the vapor of a shattering cube reaches.
    pub const VAPOR_RADIUS: i32 = 2;
    /// Bone shards a shattering cube throws.
    pub const BONE_SHARDS: usize = 16;
}
