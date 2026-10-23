//! Tuning for the world: `map`, `population`, `traps`, `layout`, `special_levels`,
//! `travel` and `ice`.

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
    pub const DRAGON_HOARD_CHANCE: f64 = 0.025;

    /// Chance a room past the starting one is packed wall to wall with
    /// monsters. See [`crate::map::SpecialRoom::MonsterZoo`].
    pub const MONSTER_ZOO_CHANCE: f64 = 0.025;

    /// Chance a room past the starting one is entirely coin, guarded by one
    /// apis. See [`crate::map::SpecialRoom::TreasureHive`].
    pub const TREASURE_HIVE_CHANCE: f64 = 0.025;

    /// Chance a room past the starting one is a hive when the player is an
    /// apis (`-am apis`), the only special room such a run has.
    pub const BEE_RUN_HIVE_CHANCE: f64 = 0.5;

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
    pub const BATTLEFIELD_CHANCE: f64 = 0.1;

    /// Chance an eligible floor is a maze of passages. See
    /// [`crate::map::SpecialLevel::Labyrinth`].
    pub const LABYRINTH_CHANCE: f64 = 0.1;

    /// Chance an eligible floor is a treasure vault of doored cells. See
    /// [`crate::map::SpecialLevel::Vault`].
    pub const VAULT_CHANCE: f64 = 0.1;

    /// Chance an eligible floor is one yellow cave full of apis. See
    /// [`crate::map::SpecialLevel::BeeWorld`].
    pub const BEE_WORLD_CHANCE: f64 = 0.05;

    /// Chance an eligible floor has a castle in the middle of it. See
    /// [`crate::map::SpecialLevel::Castle`].
    pub const CASTLE_CHANCE: f64 = 0.05;

    /// Chance an eligible floor is an island in deep water. See
    /// [`crate::map::SpecialLevel::Island`].
    pub const ISLAND_CHANCE: f64 = 0.05;
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
    pub const MONSTER_SLOTS_BASE: usize = 6;

    /// Chance a non-first monster slot actually spawns something, at tier 0.
    pub const MONSTER_FILL_CHANCE_BASE: f64 = 0.80;

    /// Added to the monster fill chance per tier.
    pub const MONSTER_FILL_CHANCE_PER_TIER: f64 = 0.05;

    /// Ceiling on the monster fill chance, so a slot is never quite certain.
    pub const MONSTER_FILL_CHANCE_CAP: f64 = 0.95;

    /// Trap slots on floor 1, before the per-tier bonus.
    pub const TRAP_SLOTS_BASE: usize = 6;

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
    pub const CORRIDOR_LURKER_CHANCE: f64 = 0.30;

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
    pub const START_CLEARING_RADIUS: u16 = 6;

    /// How many times a floor with no special level (and the labyrinth, castle
    /// and island, which promise no crowd) runs each of the monster and item
    /// budgets: once, the ordinary floor.
    pub const ORDINARY_BUDGET_RUNS: usize = 1;

    /// A battlefield runs the monster budget this many times: one lit room, all
    /// of it in view, so it needs the crowd to be a fight at all.
    pub const BATTLEFIELD_MONSTER_RUNS: usize = 3;

    /// A battlefield runs the item budget this many times.
    pub const BATTLEFIELD_ITEM_RUNS: usize = 3;

    /// A vault runs the monster budget this many times.
    pub const VAULT_MONSTER_RUNS: usize = 3;

    /// A vault runs the item budget this many times: the haul is the point.
    pub const VAULT_ITEM_RUNS: usize = 3;

    /// A bee world runs the item budget this many times (its monster budget
    /// runs once per difficulty tier instead).
    pub const BEE_WORLD_ITEM_RUNS: usize = 3;

    /// Where the level feeling draws its lines, per difficulty tier: a floor
    /// with at most `.0` monsters feels safe, one with more than `.1` feels
    /// dangerous. Cut from the counts 300 seeds of floors actually held, so
    /// about one floor in ten reads safe and one in four reads dangerous. Special
    /// rooms make a long tail (a zoo holds 40 to 190), so the dangerous line sits
    /// just over what an ordinary floor holds, not at the tail's edge.
    ///
    /// **If you change a budget above:** the shape of these counts moves with
    /// it. `models/tests/it/level_feeling.rs` fails when a line stops firing or
    /// starts firing on most floors; re-cut the numbers then.
    pub const FEELING_MONSTERS: [(usize, usize); 5] = [(3, 7), (5, 19), (7, 22), (9, 27), (10, 41)];

    /// [`FEELING_MONSTERS`] for items on the floor: at most `.0` feels poor,
    /// more than `.1` feels rich.
    pub const FEELING_ITEMS: [(usize, usize); 5] = [(5, 14), (6, 18), (7, 20), (8, 24), (10, 47)];

    /// How many times a placement will re-roll before giving the slot up.
    ///
    /// Thirteen, not a hundred. A floor is a few hundred open tiles holding a dozen
    /// things, so the first draw nearly always lands somewhere free and the budget
    /// is never spent; the only floors that reach the end of it are ones so
    /// crowded that the fourteenth try would not have helped either. Spending a
    /// hundred draws to find that out costs the same seeded RNG stream everything
    /// else on the floor draws from, for a slot the dungeon is happy to skip.
    pub const PLACEMENT_TRIES: usize = 13;
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
    pub const ARROW_DAMAGE_SIDES: i32 = 3;
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
    pub const TRAP_BREAK_CHANCE: f64 = 0.5;

    /// Radius, in tiles, of the burst a trap makes when something sets it off
    /// from a distance — `1` is the 3×3 around it. See `traps::detonate_trap`.
    pub const TRICK_SHOT_RADIUS: i32 = 1;

    /// The reach of a trick shot set off on something that is not a trap — a
    /// coin, or the Element of Yoord itself. `1` is the 3x3 around the tile,
    /// the same reach as a trap's.
    pub const PICKUP_TRICK_SHOT_RADIUS: i32 = 1;

    /// A trick shot's burst deals `TRICK_SHOT_DAMAGE_DICE d
    /// TRICK_SHOT_DAMAGE_SIDES`, rolled once and applied whole to everything
    /// caught — no armour of any kind is subtracted, not even the armour plus
    /// that a trap's own damage still allows.
    pub const TRICK_SHOT_DAMAGE_DICE: i32 = 2;
    /// See [`TRICK_SHOT_DAMAGE_DICE`].
    pub const TRICK_SHOT_DAMAGE_SIDES: i32 = 2;
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
    pub const VAPOR_RADIUS: i32 = 1;
    /// Bone shards a shattering cube throws.
    pub const BONE_SHARDS: usize = 16;
}
