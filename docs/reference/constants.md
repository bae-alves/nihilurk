Tuning constants
================

    Audience       Anyone rebalancing the game — changing how much a
                   wand hurts, how cursed the loot is, how fast the
                   Dungeon Lord evicts you.
    Prerequisites  None.
    Status         Describes `models/src/constants.rs` as it is in the
                   source. If this page and the source disagree, the
                   source is right and this page is a bug.

The dry facts: where each knob lives and what it does. *Why* the numbers are what they are is `../explanation/combat-and-balance.md`.

Every balance number in the model is defined in one file:

    models/src/constants.rs

It is organised into `pub mod` blocks by domain, and every constant carries a doc comment explaining what it does and — where it matters — what else you have to touch if you change it. The file itself is the reference; this page is the map to it.

The module that *uses* a constant re-exports it (often under its old name), so existing call sites and doc links keep working. You change the value in `constants.rs` and nowhere else, unless a doc comment there tells you otherwise.


The modules
-----------

| `constants::` | Holds | Read when you want to change |
|---------------|-------|-----------------------------|
| `combat`      | Excellent-hit odds and dice, the chip-damage floor, the odds a corpse keeps each piece of gear | how swingy a fight is |
| `player`      | Starting HP / armour / power / magic, sight radius | the hero's opening position (new games only) |
| `lurk`        | Starting HP / magic (`-b lurk`), `GROWTH_CHANCE` and `GROWTH_STEP` for feeding off a kill | how the lurk starts and how fast it grows |
| `progression` | `FINAL_DEPTH`, `DUNGEON_LORD_PATIENCE`, the staircase heal divisor, `DIFFICULTY_TIER_LAST_DEPTH` (the depth bands the crowding budgets step at) | how long a run is and how hard attrition bites |
| `map`         | `WIDTH`, `HEIGHT`, dark-room chance | the playfield (see the determinism caveat below) |
| `population`  | Monster / trap / item budgets per floor, how they scale with the difficulty tier, how many runs of the budgets a battlefield, vault or bee world gets, corridor lurkers, hidden items | how crowded and dangerous a floor is |
| `potions`     | What a dose is worth: the max-HP a healing / extra healing potion adds, the power gain strength adds and poison takes (with its floor), the share of turns paralysis eats | how much a potion swings a run |
| `scrolls`     | What one enchantment is worth (and that a minus is mended whole), how long sleep and hold last, how often sleep backfires on the reader | how strong the room-clearing scrolls are |
| `decks`       | `DECK_SIZE`, `REVERSED_MIN` / `REVERSED_MAX`, `CARD_CHAIN_CAP`, `BALA_POWER`, `BOLE_ARMOR`, `WORLD_TURNS`, `CARD_CASTER_CASTS`, `GOLDEN_WIND_SUMMONS`, the score of each hand | how wild a deck of cards runs |
| `runes`       | `PROTECTION_TURNS`, `RECHARGE_STEP`, `RECHARGE_CAP` | how long a rune of protection holds and how much one of recharging gives |
| `traps`       | Arrow / dart damage dice, `TRAP_DAMAGE_TIER_LAST_DEPTH` and the per-tier bonus / strength drain, the bear-trap thrash, the trick-shot burst (`TRICK_SHOT_RADIUS`, `PICKUP_TRICK_SHOT_RADIUS` and the shared dice), `TRAP_BREAK_CHANCE` | how much a trap hurts and how fast it scales |
| `wands`       | `WAND_CHARGES`, zap damage dice, both blast radii, per-charge dice a thrown wand spends, `DIG_RANGE` (how deep a wand of digging bores) | how good a wand is |
| `loot`        | Enchantment odds (normal / exceptional / cursed), the bonus ranges, ammo bundle size, the launcher die multiplier | how the drop table feels |
| `items`       | `THROW_RANGE`, `LIGHT_THROW_RANGE`, `LAUNCHER_RANGE`, `STACK_LIMIT`, `PACK_CAPACITY` | reach and pack density |
| `rings`       | `STEALTH_RANGE` (how close a stealthy player is noticed at), `TELEPORT_MAGIC_COST` | the two rings with a number that isn't on their row |
| `score`       | `KILL_PER_MAX_HP`, `COMBO_BONUS_PER_KILL`, `COMBO_PRIDE_CHANCE`, `STAIR_PER_TIER`, `SCORE_FLASH_TURNS` | what the run is scored on, and how loudly |
| `monsters`    | `DEFAULT_SPAWN_WEIGHT` | the baseline rarity a bestiary row gets |
| `spirits`     | `ALIGNMENT_POLE` (how far alignment drifts before spirits turn hostile for good), `SPAWN_WEIGHT`, `PINK_DEMON_ODDS_PER_PIECE` (each piece of gear the pink demon destroys adds this to the odds they join you), the red demon's `RED_DEMON_GEAR_PRICE` (Max HP) and the gnome's `GNOME_SCROLL_PRICE` / `GNOME_POTION_PRICE` / `GNOME_WAND_PRICE` (Max Ma), the barterer's `BARTER_STOCK_MIN` / `BARTER_STOCK_MAX`, the `ALIGNMENT_STEP` a poof moves alignment by, and the angel's test of faith (`TEST_OF_FAITH_HP_DIVISOR`, `TEST_OF_FAITH_GEAR_BONUS`, `TEST_OF_FAITH_DUD_BONUS`) | how common spirits are and how fast they turn on you |
| `helpers`     | `ACCEPT_CHANCE`, `SHAPESHIFT_CHANCE` | how often a thrown treat wins a Helper, how often a dog's kill changes its shape |
| `spells`      | `SPELLSET_CAP`, the dice behind the spells that roll their own (Thunderbolt, Force Lance, Circle of Death, Frost Nova), the stand-in batteries (`LUX_CHARGES`, `METEOR_STRIKE_CHARGES`), and a staff's `TURBO_MAGIC_COST_MULT` / `TURBO_MAGIC_POWER_MULT` | how much a spell costs and how hard it lands |
| `travel`      | Step caps on autoexplore / fast-move, the HP divisor below which auto-fight refuses | only if a walk loops, or auto-fight is too brave or too timid |
| `hud`         | Message-log rows shown at once, the two wrap widths (normal and `--MORE--`), and how much scrollback `LOG_HISTORY_CAP` keeps | the log's footprint on screen |
| `abilities`   | The per-turn odds of the passive abilities (aggravate monster, regeneration, teleportitis, polymorphitis) and `MOMENTUM_PER_HIT` | how often a ring does its thing on its own |
| `conditions`  | `CONFUSION_STUMBLE_CHANCE`, the share of a confused player's steps that go astray | how much confusion costs (paralysis's dial is in `potions`) |
| `speed`       | `SLOW_RATE`, `NORMAL_RATE`, `QUICK_RATE`, `FAST_RATE` and the `ACTION_COST` they are read against | how often each tempo acts |


Two caveats worth repeating
---------------------------

**`map::WIDTH` / `map::HEIGHT` pin every seed.** `models/tests/determinism.rs` asserts that a given seed produces the same walls it always has. Changing either dimension moves every wall on every seed and that test will fail — correctly. Only change them as a deliberate "all old seeds are void" decision.

**Enchantment odds are read as ranges against a `0..100` roll.** `loot::NORMAL_QUALITY_PCT` and `loot::EXCEPTIONAL_QUALITY_PCT` must sum to 100 or less; the remainder is the cursed share. The prose odds tables in this directory's `content-tables.md` and in `catalog::Quality::roll` do not update themselves.


What is deliberately *not* in `constants.rs`
-------------------------------------------

* **Content-table numbers** — a monster's HP, a weapon's die, a potion's colour. Those are data, one row per thing, in `catalog.rs` / `monsters.rs` / `traps.rs`. See `content-tables.md`.
* **Animation timing** — the millisecond figures in `particles.rs` (bolt speed, blast ripple) and the wave counts in `magicmap.rs`. Presentation feel, wound tightly around the code that reads them.
* **Map-layout geometry**, RNG salts, and the neighbour-offset tables. Structural, not balance.
* A couple of one-off rolls still inline where they fire, each named and doc-commented where it lives — the room-placement geometry in `map/generate.rs`, the placement retry budget in `map/population.rs`.


See also
--------

    constants.rs                         the file itself, fully commented
    ../explanation/combat-and-balance.md  why the numbers are what they are
    content-tables.md                     the per-row content numbers
