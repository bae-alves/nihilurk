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
Mechanic: `apply_potion_effect` in `models/src/items.rs`.

### SCROLLS -- ScrollDef

| Field    | Type           | Notes                                    |
|----------|----------------|------------------------------------------|
| `effect` | `ScrollEffect` | Keys the mechanic; identity in saves.    |
| `name`   | `&'static str` |                                          |

Draws `?`, always white -- a scroll has no colour field.
Attaches `Item`, `Scroll`, `Consume`.
Mechanic: `apply_scroll_effect` in `models/src/items.rs`.

### WANDS -- WandDef

| Field    | Type           | Notes                                    |
|----------|----------------|------------------------------------------|
| `effect` | `WandEffect`   | Keys the mechanic; identity in saves.    |
| `name`   | `&'static str` |                                          |
| `color`  | `Color`        |                                          |
| `range`  | `i32`          | Feeds the aiming reticle. In use: 6, 8.  |

Draws `/`. Attaches `Item`, `Wand`, `Ranged`, `Battery`.
A floor drop rolls `3d4` charges (`roll_wand_charges`).
Whether zapping opens the reticle: `WandEffect::needs_target`, which is
true for everything except the wand of light.
Mechanic: `apply_wand_effect` in `models/src/items.rs`.

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

| Field       | Type          | Notes                                    |
|-------------|---------------|------------------------------------------|
| `effect`    | `TrapEffect`  | Keys the mechanic; identity in saves.    |
| `name`      | `&'static str`| What `TrapEffect::label()` returns.      |
| `glyph`     | `char`        | `'^'` for all six.                       |
| `color`     | `Color`       | `Red` for all six.                       |
| `weight`    | `u32`         | Rarity. All six are `10`.                |
| `min_depth` | `u8`          | All six are `1`.                         |

A trap entity carries `Name`, `Renderable`, `Position`, `Trap`, `Hidden`.
It is never an `Item` and never a tile type.

Mechanic: `spring_trap` in `models/src/traps.rs`. That match has no
catch-all, so a new `TrapEffect` variant will not compile until it has an
arm.

Reveal style is rolled per trap at spawn, equal odds, not per row:
`Sight` / `Adjacent` / `Triggered`.

Damage traps ignore the defender's armour *die* but still subtract the
armour *plus* (`total_armor_plus`).


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
that arrives as a floor drop. Never for a `spawn_named` spawn.

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
    DUNGEON_LORD_PATIENCE  240      turns on one floor before eviction
    STACK_LIMIT             26      most of one item per pack slot
    THROW_RANGE              7      how far you can hurl a thing
    Speed::COST              2      energy one action costs


See also
--------

  spawn-api.md                  the functions that read these tables
  cli-and-env.md                flags and environment variables
  ../how-to/add-an-item.md      how to add a row to one of these
