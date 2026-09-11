Reference: content tables
=========================

    Audience       Anyone editing content. Look things up here; do not
                   read it end to end.
    Prerequisites  None.
    Status         Describes the tables as they are in the source. If
                   this page and the source disagree, the source is
                   right and this page is a bug.

Every table in the game, every field, every default.


Where everything is
-------------------

| Table               | File                       | Row type       |
|---------------------|----------------------------|----------------|
| `BESTIARY`          | `models/src/monsters.rs`   | `MonsterDef`   |
| `POTIONS`           | `models/src/catalog.rs`    | `PotionDef`    |
| `SCROLLS`           | `models/src/catalog.rs`    | `ScrollDef`    |
| `WANDS`             | `models/src/catalog.rs`    | `WandDef`      |
| `WEAPONS`           | `models/src/catalog.rs`    | `WeaponDef`    |
| `AMMO`              | `models/src/catalog.rs`    | `AmmoDef`      |
| `LAUNCHERS`         | `models/src/catalog.rs`    | `LauncherDef`  |
| `ARMORS`            | `models/src/catalog.rs`    | `ArmorDef`     |
| `RINGS`             | `models/src/catalog.rs`    | `RingDef`      |
| `COINS`             | `models/src/catalog.rs`    | `CoinDef`      |
| `TRAPS`             | `models/src/traps.rs`      | `TrapDef`      |
| `DROPS`             | `models/src/spawn.rs`      | `DropCategory` |
| `EFFECTS`           | `models/src/effects.rs`    | `Grant`        |
| `PASSIVE_ABILITIES` | `models/src/abilities.rs`  | `PassiveAbility` |

For the live contents of any of them:

    cargo run -p engine -- -content


BESTIARY -- MonsterDef
----------------------

Constructor: `MonsterDef::row(name, glyph, color, movement, hp, power,
power_bonus, armor, armor_bonus, min_depth)`, then optional chains.

| Field         | Type               | Def.    | Meaning                    |
|---------------|--------------------|---------|----------------------------|
| `name`        | `&'static str`     | --      | Unique; its save identity. |
| `glyph`       | `char`             | --      | One character on the map.  |
| `color`       | `Color`            | --      | See the palette below.     |
| `movement`    | `MovementType`     | --      | Static/Chase/Flee/Confused |
| `hp`          | `i32`              | --      | Hit points; also `max_hp`. |
| `power`       | `i32`              | --      | Attack die: `1d[power]`.   |
| `power_bonus` | `i32`              | --      | Flat, once, on that roll.  |
| `armor`       | `i32`              | --      | Defence die: `1d[armor]`.  |
| `armor_bonus` | `i32`              | --      | Flat, once, on that roll.  |
| `min_depth`   | `u8`               | --      | Shallowest floor it spawns.|
| `weight`      | `u32`              | `10`    | Rarity vs eligible pool.   |
| `grants`      | `&'static [Grant]` | `&[]`   | Effects it is born with.   |
| `invisible`   | `bool`             | `false` | Born unseeable.            |

Chains: `.grants(&[...])`, `.invisible()`, `.weight(n)`.

Combat: `damage = (1d[power] + power_bonus) - (1d[armor] + armor_bonus)`,
both sides rolled independently. Nothing ever misses.

Spawning a row attaches: `Name`, `Mob`, `Fighter`, `Renderable`,
`Position`, `Faction::Monster`, `Blood`, `Grants`, `Speed(Normal)`, plus
each granted effect component, plus `Invisible` if the row asked.

`MovementType::Aggravated { tx, ty }` exists but is applied at run time by
the scroll of aggravate monsters. Never put it in a row.


Item tables
-----------

Every row implements `ItemDef`, which supplies:

    fn name(&self) -> &'static str            required
    fn spawn(&self, world, pos) -> Entity      required
    fn weight(&self) -> u32                    default 10
    fn min_depth(&self) -> u8                  default 1
    fn spawn_as_loot(&self, world, rng, pos)   default: calls spawn

No item row overrides `weight` or `min_depth` today -- within a category
roog picks evenly on purpose.

### POTIONS -- PotionDef

| Field    | Type           | Notes                                    |
|----------|----------------|------------------------------------------|
| `effect` | `PotionEffect` | Keys the mechanic; identity in saves.    |
| `name`   | `&'static str` |                                          |
| `color`  | `Color`        |                                          |

Draws `!`. Attaches `Item`, `Potion`, `Consume`.
Mechanic: `apply_potion_effect` in `models/src/items/potions.rs`.

### SCROLLS -- ScrollDef

| Field    | Type           | Notes                                    |
|----------|----------------|------------------------------------------|
| `effect` | `ScrollEffect` | Keys the mechanic; identity in saves.    |
| `name`   | `&'static str` |                                          |

Draws `?`, always white -- a scroll has no colour field.
Attaches `Item`, `Scroll`, `Consume`.
Mechanic: `apply_scroll_effect` in `models/src/items/scrolls.rs`.

### WANDS -- WandDef

| Field    | Type           | Notes                                    |
|----------|----------------|------------------------------------------|
| `effect` | `WandEffect`   | Keys the mechanic; identity in saves.    |
| `name`   | `&'static str` |                                          |
| `color`  | `Color`        |                                          |
| `range`  | `i32`          | Feeds the aiming reticle. In use: 6, 8.  |

Draws `/`. Attaches `Item`, `Wand`, `Ranged`, `Battery`.
A floor drop rolls `2d6 + 1` charges (`roll_wand_charges`). Charge dice,
zap damage dice and both blast radii live in `models/src/constants.rs` →
`wands` (see `reference/constants.md`).
Whether zapping opens the reticle: `WandEffect::needs_target`, which is
true for everything except the wand of light.
Mechanic: `apply_wand_effect` in `models/src/items/wands.rs`. Throwing a
wand is resolved in `models/src/items/throwing.rs` (`resolve_wand_throw`).

A zapped bolt (`fire_bolt` → `Particles::beam`) flickers white/its own
colour twice per cell rather than fading once, and lands with
`Particles::impact_sparks` — a brighter flash on the hit tile plus a small
ring of offset sparks around it, timed off `beam`'s returned flight time so
they pop right as the bolt arrives. Flashier and denser than the beam
alone, on every bolt-type wand (fire, cold, lightning, magic missile,
striking, drain life).

Teleport away/to (`teleport_entity_away`, `teleport_target_here`) leaves a
`Particles::poof` and a short `map::Smoke` puff (`TRANSMUTATION_SMOKE_TURNS`,
2 turns) where the creature stood — the wand's own signature; the scroll of
teleportation gets no such flourish. Polymorph (`polymorph_entity`) puffs
the same smoke in a small ring around the transformed creature's tile
(`leave_smoke_ring`), staggered so it reads as smoke rolling outward.

Every particle animation's frame pacing (a zap's beam, a blast's ripple, a
teleport's poof, the magic-mapping reveal wipe) scales with the `AnimRate`
resource, set once at startup from `-anim-rate` — see
`reference/cli-and-env.md`.

Neither a zap nor a throw can be aimed at the player's own tile: the
engine refuses it with "Great idea! But no." and no turn passes.

**Thrown**, a wand only bursts on *impact* — hitting a creature or a
wall, or flying its full `THROW_RANGE` leash. Lobbed into open floor
short of that, it just lands with its charges and its secret intact.

On impact (`resolve_wand_throw`) it spends every remaining charge at
once. The blast animation is coloured per wand (`blast_palette` →
`particles::BlastPalette`) and always opens on that palette's bright
first frame — a primary blast never reads as dark. On a light or
utility wand's throw (never an attack wand's — it returns before the
per-creature loop), every creature the blast actually caught also gets
a small, darker `Particles::secondary_burst` a beat after the primary
ripple passes its tile — purely cosmetic, confirming who the effect
landed on, no gameplay of its own.

Fire and cold blasts (zapped or thrown — both run through the one
`elemental_blast`) also billow a grey/white `Particles::smoke_burst`
across the blast cells a beat after the flames. Fire's smoke additionally
lingers on the map for `SMOKE_LINGER_TURNS` (4) real turns afterward,
DCSS-style — the `map::Smoke` resource, ticked once a turn by
`smoke_system`, ages every puff down and the renderer draws a grey `≈`
over any smoky tile currently in view (never on a tile an actor stands
on, like blood). Cold's puff is animation only; nothing persists.

Anyone `elemental_blast` kills outright is finished off right there
(`combat::finish_indirect_kill`) rather than left for `reaper_system`'s
next sweep, so their death burst knows the blast's centre and flings the
corpse radially outward through where they stood — an edge casualty gets
launched straight on away from the blast, not a random direction like an
ordinary indirect kill. A casualty standing exactly on the blast's own
centre has no "outward" to speak of, so it falls back to the usual random
fling.

- *Attack* wands (fire, cold, lightning, magic missile, striking, drain
  life — `is_attack_wand`) throw the wide grenade: `GRENADE_RADIUS`, `d4`
  per remaining charge, armour-ignoring. Fire and cold carry their
  element (immunities apply); the rest are non-elemental.
- *Wand of light* throws the same wide `d4`-a-charge grenade, but instead
  of an element it **dazzles** every creature caught (`dazzle`) — a
  monster flips to `MovementType::Confused`, the player gains the
  `Confused` condition. "dazzle" in the log.
- *Utility* wands (polymorph, haste, slow, teleport away/to,
  cancellation) throw a small `BLAST_RADIUS` blast that deals **no
  damage** — the effect is the whole payload, worked on every creature
  caught (`apply_thrown_wand_effect`), thrower included.
- Wand of nothing: bursts in magenta/cyan confetti particles, no blast.

Effects that can land on the **player** (via a thrown blast): polymorph
logs "You feel like a new person"; haste/slow set the player's `Speed`;
dazzle gives `Confused`; teleport away runs the scroll-of-teleportation
relocation; teleport-to with no other target logs the "straight to
yourself" joke; cancellation is `cancel_player` — zeroes every
`PowerBonus`/`ArmorBonus` on weapons and armour, turns unread scrolls to
`BlankPaper` and potions to `Water`, and lifts every `Curse` without
destroying the item.

**Player conditions** (`Confused`, and `Speed` haste/slow) are
treacherous: they never wear off with time. Only two things clear them,
both via `clear_player_conditions`, which logs "You are no longer {}."
for each: **using a staircase** (`transition_level`) and **a wand of
cancellation** (`cancel_player`). The HUD shows them as 4-letter
mnemonics (`FAST` cyan / `SLOW` green / `CONF` magenta), suppressing the
score line for space when any is lit.

While `Confused`, half of every walk or swing goes off in a random
direction ("You stumble foolishly"; `maybe_stumble` in
`engine/src/update.rs`), and fast movement, auto-explore and auto-fight
all refuse with "You are too confused for that right now."

### WEAPONS -- WeaponDef

Constructor: `WeaponDef::new(name, color, power_die)`, then chains.

| Field        | Type           | Default      | Notes                     |
|--------------|----------------|--------------|---------------------------|
| `name`       | `&'static str` | --           |                           |
| `color`      | `Color`        | --           |                           |
| `power_die`  | `i32`          | --           | Damage rolls `1d[n]`.     |
| `thrown_die` | `i32`          | `power_die`  | Die rolled on impact.     |
| `projectile` | `bool`         | `false`      | Built to be thrown.       |
| `piercing`   | `bool`         | `false`      | Throw runs the whole line.|

Chains: `.missile(die)` sets `thrown_die` and `projectile`;
`.piercing()` sets `piercing`.

Draws `)`. Attaches `Item`, `Equipped::loose(Slot::Hand)`, `PowerDie`,
`ThrownDamage`, and `Projectile` / `Piercing` when asked.
A floor drop is enchanted (see below).

`Projectile` means three things at once: the throw ignores the target's
armour die, the missile is spent on what it hits, and nothing can catch
it. A non-projectile throw is blunted by armour and can be caught and
used against you.

### AMMO -- AmmoDef

| Field         | Type           | Notes                                   |
|---------------|----------------|-----------------------------------------|
| `name`        | `&'static str` |                                         |
| `color`       | `Color`        |                                         |
| `die`         | `i32`          | Rolled when hurled by hand.             |
| `launched_by` | `Grant`        | The effect that doubles the die.        |

Draws `)`. Attaches `Item`, `ThrownDamage`, `Projectile`, `LaunchedBy`,
`Stack { count: 1 }`. No `PowerDie` and no `Equipped` -- there is nothing
to wield and nothing to wear.

Stacks to `STACK_LIMIT` (26) per pack slot. A floor drop arrives as a
bundle of 3-12.

### LAUNCHERS -- LauncherDef

| Field       | Type               | Notes                              |
|-------------|--------------------|------------------------------------|
| `name`      | `&'static str`     |                                    |
| `color`     | `Color`            |                                    |
| `grants`    | `&'static [Grant]` | Effects lent to whoever holds it.  |
| `melee_cap` | `i32`              | Most it is worth swung. Both are 1.|

Draws `}`. Attaches `Item`, `Equipped::loose(Slot::Hand)`, `Launcher`,
`ThrowBonus(0)`, `Grants`, `MeleeCap`. Contributes no attack or armour die
at all -- its enchantment therefore lands on `ThrowBonus`.

`melee_cap` becomes a `MeleeCap` component, which clamps the wielder's
melee damage however the dice fell. A launcher takes the hand a sword
would have had; this is what that hand costs.

### ARMORS -- ArmorDef

| Field       | Type           | Notes                                  |
|-------------|----------------|----------------------------------------|
| `name`      | `&'static str` |                                        |
| `color`     | `Color`        |                                        |
| `armor_die` | `i32`          | Defence roll adds `1d[n]`. In use 2-9. |

Draws `]`. Attaches `Item`, `Equipped::loose(Slot::Body)`, `ArmorDie`.

### RINGS -- RingDef

Constructor: `RingDef::new(effect, name)`, then chains.

| Field         | Type                | Default | Notes                     |
|---------------|---------------------|---------|---------------------------|
| `effect`      | `RingEffect`        | --      | Identity for ident/saves. |
| `name`        | `&'static str`      | --      |                           |
| `power_die`   | `i32`               | `0`     | Rarely used.              |
| `power_bonus` | `i32`               | `0`     | `.power_bonus(n)`         |
| `armor_die`   | `i32`               | `0`     | Rarely used.              |
| `armor_bonus` | `i32`               | `0`     | `.armor_bonus(n)`         |
| `throw_bonus` | `i32`               | `0`     | `.throw_bonus(n)`         |
| `grants`      | `&'static [Grant]`  | `&[]`   | `.grants(&[...])`         |

Draws `=`, always yellow. Attaches `Item`, `Ring`,
`Equipped::loose(Slot::Finger)`, each non-zero modifier, and `Grants`
when non-empty. A zero modifier attaches nothing.

There is no ring behaviour code anywhere. A ring is numbers and grants.

### COINS -- CoinDef

| Field   | Type           | Notes                                       |
|---------|----------------|---------------------------------------------|
| `name`  | `&'static str` |                                             |
| `color` | `Color`        |                                             |
| `value` | `i32`          | Added to score. Coins buy nothing.          |

Draws `$`. Attaches `Value`, `Item`.

### The relic

Not a table -- one function, `spawn_element_of_yoord`. Draws `"` in
magenta, worth 25000, carries `Amulet`. Its name is the constant
`ELEMENT_OF_YOORD`. Spawned in place of the down-stair on floor 13.


TRAPS -- TrapDef
----------------

| Field         | Type          | Notes                                          |
|---------------|---------------|------------------------------------------------|
| `effect`      | `TrapEffect`  | Keys the mechanic; identity in saves.          |
| `name`        | `&'static str`| What `TrapEffect::label()` returns.            |
| `glyph`       | `char`        | `'^'` for all six.                             |
| `color`       | `Color`       | One per row.                                   |
| `weight`      | `u32`         | Rarity. All six are `10`.                      |
| `min_depth`   | `u8`          | All six are `1`.                               |
| `snare_turns` | `u32`         | Turns a bear / gas trap holds you; `0` otherwise. |

A trap entity carries `Name`, `Renderable`, `Position`, `Trap`, `Hidden`.
It is never an `Item` and never a tile type.

Mechanic: `spring_trap` in `models/src/traps.rs`. That match has no
catch-all, so a new `TrapEffect` variant will not compile until it has an
arm. A snaring trap reads its duration straight off the row, so it stays a
one-file change.

Reveal style is rolled per trap at spawn, equal odds, not per row:
`Sight` / `Adjacent` / `Triggered`.

Damage traps ignore the defender's armour *die* but still subtract the
armour *plus* (`total_armor_plus`). Their bite also scales with depth in
three bands (floors 1-4, 5-8, 9-13): each band adds a point to the arrow
trap's roll and a point to the dart trap's permanent power drain. Dial:
`constants::traps` (`TRAP_DAMAGE_TIER_LAST_DEPTH` and the per-tier steps).

The `Trap`, `TrapEffect`, `TrapReveal`, `Snare` and `SnareKind` types are
defined in `components.rs`, not `traps.rs` -- see `components.md`.


DROPS -- DropCategory
---------------------

    models/src/spawn.rs

| Field       | Type           | Notes                                   |
|-------------|----------------|-----------------------------------------|
| `name`      | `&'static str` | The category, not an item name.         |
| `weight`    | `u32`          | Share of a floor's drops.               |
| `min_depth` | `u8`           | Shallowest floor it drops on.           |

Three further fields are function pointers filled in by the `category!`
macro from the table's name. Never write them by hand.

Current weights, which happen to total 1000:

| Category | Weight | Share |
|----------|--------|-------|
| scroll   | 300    | 30.0% |
| potion   | 270    | 27.0% |
| coin     | 170    | 17.0% |
| armor    |  80    |  8.0% |
| wand     |  50    |  5.0% |
| ring     |  50    |  5.0% |
| weapon   |  36    |  3.6% |
| ammo     |  28    |  2.8% |
| launcher |  16    |  1.6% |

The share column is derived, not maintained.


EFFECTS -- the marker registry
------------------------------

    models/src/effects.rs

Order is the save format: an effect's index is its bit in an `EffectSet`.
**Append only.** `EffectSet` is a `u32`, so the ceiling is 32 effects.

| # | Effect              | Meaning                                        |
|---|---------------------|------------------------------------------------|
| 0 | `FireImmune`        | Fire does nothing.                             |
| 1 | `ColdImmune`        | Cold does nothing.                             |
| 2 | `Undead`            | Draining passes through, healing nothing.      |
| 3 | `VorpalTarget`      | Any vorpal weapon slays it outright.           |
| 4 | `SeesInvisible`     | Sees hidden traps, monsters, stashed items.    |
| 5 | `SustainsStrength`  | Immune to dart-trap strength drain.            |
| 6 | `AggravatesMonsters`| Periodically wakes the floor. Passive.         |
| 7 | `ItemUser`          | Catches and wears thrown gear; reads scrolls.  |
| 8 | `FireArrow`         | Looses arrows properly (doubles their die).    |
| 9 | `FireQuarrel`       | The crossbow's half of the same bargain.       |

Cap components -- ceilings the dice cannot beat. Folded with `min`, not
`+`, because the strictest one wins. Not in `EFFECTS`, not bits:

    MeleeCap     most the bearer can deal in one melee blow (a bow: 1)

Modifier components -- numbers that stack across equipped gear, folded by
`equipped_total::<C>()`. Not in `EFFECTS`, not bits:

    PowerDie     adds to the attack die size
    PowerBonus   flat, added once to the damage roll
    ArmorDie     adds to the defence die size
    ArmorBonus   flat, added once to the armour roll
    ThrowBonus   flat, added once to anything thrown


PASSIVE_ABILITIES -- PassiveAbility
-----------------------------------

    models/src/abilities.rs

| Field     | Type                     | Notes                            |
|-----------|--------------------------|----------------------------------|
| `effect`  | `Grant`                  | The marker that arms it.         |
| `chance`  | `f64`                    | Probability per acting turn.     |
| `action`  | `fn(&mut World, Entity)` | Run on the bearer.               |
| `flavour` | `&'static str`           | Logged only for the player.      |

Write `flavour` in the second person.


Enchantment
-----------

Rolled by `enchant_equipment` for every weapon, armour, launcher and ring
that arrives as a floor drop. Never for a `spawn_named` spawn. The odds
and the bonus ranges are `models/src/constants.rs` → `loot` (see
`reference/constants.md`).

| Quality     | Odds | Bonus            |
|-------------|------|------------------|
| Normal      | 25%  | +0               |
| Exceptional | 10%  | +1 .. +3         |
| Cursed      | 65%  | -5 .. +5, plus a `Curse` tag |

A cursed item can roll better than a clean one; it simply cannot be taken
off once equipped, short of a scroll of remove curse.

The bonus lands on whichever roll the item feeds, read off the item
itself: a thing with a `PowerDie` gets `PowerBonus`, a thing with an
`ArmorDie` gets `ArmorBonus`, a `Launcher` gets `ThrowBonus`. Something
that is two of those would get both. Never on the die size.


Identification
--------------

    models/src/identify.rs

Four categories arrive unidentified: potions, scrolls, wands, rings. Each
has a pool of 20 cosmetic appearances, shuffled against the catalog once
per run from the seeded RNG and then stored in the save.

| Category | Types | Pool | Headroom |
|----------|-------|------|----------|
| Potions  | 14    | 20   | 6        |
| Scrolls  | 15    | 20   | 5        |
| Wands    | 14    | 20   | 6        |
| Rings    | 12    | 20   | 8        |

Exceeding a pool leaves the extra types with no appearance, permanently
generic. Guarded by `every_identifiable_type_gets_an_appearance` in
`models/tests/content.rs`.

Knowledge is stored against the effect, not the entity: identifying one
bubbly potion identifies every bubbly potion, forever.


The colour palette
------------------

The save file packs a colour into one byte against this fixed list.
Anything not on it draws correctly but reloads as `White`.

    Black       DarkGrey    Grey        White
    Red         DarkRed     Green       DarkGreen
    Yellow      DarkYellow  Blue        DarkBlue
    Magenta     DarkMagenta Cyan        DarkCyan


Dungeon constants
-----------------

    MAP_WIDTH               80
    MAP_HEIGHT              22
    FINAL_DEPTH             13      the floor holding the relic
    DUNGEON_LORD_PATIENCE  260      turns on one floor before eviction
    STACK_LIMIT             26      most of one item per pack slot
    PACK_CAPACITY            13      most slots a pack will hold at once
    THROW_RANGE              7      how far you can hurl a thing
    Speed::COST              2      energy one action costs

These, and every other balance number, are defined and explained in
`models/src/constants.rs`. See `reference/constants.md` for the tour.


See also
--------

  spawn-api.md                  the functions that read these tables
  cli-and-env.md                flags and environment variables
  ../how-to/add-an-item.md      how to add a row to one of these
