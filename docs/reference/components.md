Reference: components, resources and events
==========================================

    Audience       Engine developers. Look a type up here; do not read it
                   end to end.
    Prerequisites  A passing familiarity with `bevy_ecs`: components are
                   per-entity data, resources are singletons, events here
                   travel by queue.
    Status         Describes `models/src/components.rs` as it is in the
                   source. If this page and the source disagree, the
                   source is right and this page is a bug.

Every `Component`, `Resource` and `Event` is declared in
`models/src/components.rs`. That file is **nouns only** — a component says
what a thing *is*, never what happens as a result. The verbs are the
systems: `combat.rs`, `ai.rs`, `items/`, `visibility.rs`, `traps.rs`.

A `Bundle` — the struct that assembles components for one spawn
(`MonsterBundle`, `TrapBundle`) — is **not** here. It lives next to its
content table (`monsters.rs`, `traps.rs`), because it names that table's
`Def` row and is the one place an entity of that kind is described. See
`../explanation/data-driven-content.md`, "Where the pieces live".

Two shapes recur:

  * **Marker** — no fields. Presence is the fact (`Player`, `Blood`,
    `Curse`, `Confused`).
  * **Type key** — one enum naming which one it is (`Potion` →
    `PotionEffect`). The key is identity for `identify.rs` and the save
    file; it is never behaviour. The catalog row turns a key into the
    components that do something; the mechanic is a `match` on the key.

"Saved?" below means the field survives `models/src/saveload.rs`. A
**transient** field is rebuilt every frame or every load and deliberately
left out of the save. A **catalog** component is re-attached by name on
load (`restore_from_catalog`), so it is never written either.


Components — identity, position, appearance
-------------------------------------------

| Component    | Data                                    | On            | Saved? |
|--------------|-----------------------------------------|---------------|--------|
| `Name`       | `what: String`                          | monsters, items, hero | yes |
| `Player`     | marker                                  | the hero      | yes    |
| `Position`   | `x, y: u16`                             | anything on the floor; removed while in a `Backpack` | yes |
| `Renderable` | `glyph: char`, `color: Color`           | anything drawn | yes (colour packs to one byte against a 16-entry palette) |
| `Faction`    | enum `Player` / `Monster` / `Ally`      | every actor   | yes    |

`Name::article()` returns `"a"` / `"an"` for the name.


Components — creatures and combat
---------------------------------

| Component | Data | On | Saved? |
|-----------|------|-----|--------|
| `Mob`     | `movement_type: MovementType` | every monster | yes |
| `Fighter` | `hp, max_hp, armor, power, max_power, armor_bonus, power_bonus: i32` | every actor | yes |
| `Blood`   | marker | creatures that bleed | **transient** — re-attached to the player and every mob on load |
| `Magic`   | `points, max_points: u8` | the hero | yes |

`MovementType` — enum, **saved by variant order**:

| Variant                  | Meaning                                        |
|--------------------------|------------------------------------------------|
| `Static`                 | Holds still.                                   |
| `Chase`                  | Walks toward the player when it can see them.  |
| `Flee`                   | Walks away.                                    |
| `Confused`               | Staggers at random (the monster's confusion).  |
| `Aggravated { tx, ty }`  | Homes in on `(tx, ty)` from anywhere, in or out of view. Applied at run time by the scroll of aggravate monsters — **never put it in a table.** |

Combat: `damage = (1d[power] + power_bonus) - (1d[armor] + armor_bonus)`,
both sides rolled independently, nothing ever misses. `power` is dropped
by a poisoned dart trap — one point per depth tier — and healed back
toward `max_power`.


Components — speed and tempo
----------------------------

The player is the clock. Monsters bank energy on each player turn and
spend it in `ai.rs`; the player's own tempo is run by the engine loop
through the `PlayerTempo` resource.

| Component | Data | On | Saved? |
|-----------|------|-----|--------|
| `Speed`   | `kind: SpeedKind`, `energy: i32` | every actor | `kind` yes; `energy` **transient** (resets to 0) |

`SpeedKind` — enum, **saved by variant order** (`Slow`, `Normal`,
`Fast`; `Normal` is `#[default]`):

| Method     | Returns                                            |
|------------|---------------------------------------------------|
| `rate()`   | energy banked per player turn — 1 / 2 / 4.        |
| `faster()` | one notch up, `Fast` the ceiling (haste monster). |
| `slower()` | one notch down, `Slow` the floor (slow monster).  |

`Speed::COST` = 2 — the energy one action costs.


Components — perception and memory
----------------------------------

| Component  | Data | On | Saved? |
|------------|------|-----|--------|
| `Viewshed` | `visible_tiles: Vec<(u16,u16)>`, `revealed_tiles: FixedBitSet`, `range: u16`, `dirty: bool` | the hero (and anything that needs sight) | `revealed_tiles` + `range` yes; `visible_tiles` + `dirty` **transient** |
| `Hidden`   | marker — not drawn or announced right now | out-of-view monsters, undiscovered traps, unperceived invisibles | **transient** (visibility rebuilds it; traps clear it on reveal) |
| `Invisible`| marker — intrinsically unseeable without `SeesInvisible` | the phantom, the 1-in-10 invisible floor item | yes |
| `Spotted`  | marker — inside the player's viewshed this turn | anything currently seen | **transient** |

`revealed_tiles` is indexed by `crate::map::tile_index`.


Components — items on the floor and in the pack
-----------------------------------------------

| Component | Data | On | Saved? |
|-----------|------|-----|--------|
| `Item`    | marker — can be picked up | every item | yes |
| `Value`   | `amount: i32` — end-of-run score | coins, the relic | yes |
| `Backpack`| `items: Vec<Entity>` — inventory order | actors that carry | yes |
| `Consume` | marker — used up on use | potions, scrolls | yes |
| `Battery` | `charges: i8` | wands | yes |
| `Stack`   | `count: u8` — how many share one slot | ammunition only | yes |
| `Ranged`  | `range: i32` — feeds the zap reticle | wands | yes |
| `Amulet`  | marker — the Element of Yoord; carrying it inverts the staircases | the relic | yes |

`Stack` tops back up to `STACK_LIMIT` (26) on pickup. `Backpack` holds at most
`PACK_CAPACITY` (13) slots; `stow` refuses anything past that ("Your pack is
full.") and leaves it on the floor.


Components — item type keys
---------------------------

| Component | Key enum      | Mechanic                              | Saved? |
|-----------|---------------|--------------------------------------|--------|
| `Potion`  | `PotionEffect`| `items/potions.rs`                    | key yes |
| `Scroll`  | `ScrollEffect`| `items/scrolls.rs`                    | key yes |
| `Wand`    | `WandEffect`  | `items/wands.rs`; thrown, `items/throwing.rs` | key yes |
| `Ring`    | `RingEffect`  | none — numbers + `Grants`, from the `RingDef` row | key yes |
| `Curse`   | marker        | equipped-and-stuck until remove curse | yes |
| `Vorpal`  | `bane: String`| any blooding hit slays `bane` (or any `VorpalTarget`) outright | yes |
| `KnownQuality` | marker   | this instance's enchantment plus/curse/vorpal bane are known — set by wearing it (`equipment::toggle_equipped`/`equip_silently`) or a scroll of identify singling it out | yes |

All four key enums are **saved by variant order** — append, never
reorder. The catalog tables (`crate::catalog`) and the appearance pools
(`crate::identify`) are the other things keyed off these enums.

`KnownQuality` is per-*instance*, unlike `Identified` (`crate::identify`),
which is per-*effect*: two rings of protection share one `Identified`
entry the moment either is worn, but each rolled its own curse, so each
needs its own `KnownQuality`. `identify::display_name` reads it to decide
whether to print a weapon/armour/launcher's `+N` prefix, a `(cursed)`
suffix, or a `(vorpal vs. X)` suffix — hidden for anything not yet known.

`WandEffect::needs_target()` is `false` only for the wand of light (it
floods the room, no reticle).

`PotionEffect`: Blindness, Confusion, ExtraHealing, FruitJuice,
GainStrength, Haste, Healing, MagicDetection, MonsterDetection,
Paralysis, Poison, RaiseLevel, RestoreStrength, SeeInvisible, Water.

`ScrollEffect`: MonsterConfusion, MagicMapping, HoldMonster, Sleep,
EnchantArmor, Identify, ScareMonster, FoodDetection, Teleportation,
EnchantWeapon, CreateMonster, RemoveCurse, AggravateMonsters, BlankPaper,
VorpalizeWeapon.

`WandEffect`: Light, Striking, Lightning, Fire, Cold, Polymorph,
MagicMissile, HasteMonster, SlowMonster, DrainLife, Nothing, TeleportAway,
TeleportTo, Cancellation.

`RingEffect`: Protection, Strength, Perception, Adornment,
AggravateMonster, Dexterity, IncreaseDamage, Regeneration, SlowDigestion,
Teleportation, Stealth, MaintainArmor.


`Element` — not a component
---------------------------

`Element` (`Fire` / `Cold` / `Drain`) is a plain enum, not a component —
it never lives on an entity. It sits in `components.rs` because both the
wand mechanic and the throwing mechanic use it.

| Method              | Returns                                          |
|---------------------|-------------------------------------------------|
| `Element::of(wand)` | the element a `WandEffect` carries, or `None`.   |
| `.immunity()`       | the `Grant` that shrugs it off (`FireImmune` / `ColdImmune` / `Undead`). |
| `.noun()`           | "flames" / "cold" / "evil magic" for the log.    |


Components — throwing and launchers
-----------------------------------

A missile and a launcher never name each other; they meet at an effect
(`FireArrow` / `FireQuarrel`). Resolution is `items/throwing.rs`.

| Component      | Data                     | Meaning                          | Saved? |
|----------------|--------------------------|----------------------------------|--------|
| `ThrownDamage` | `i32`                    | die rolled on impact; no component ⇒ bounces off harmlessly | catalog |
| `Projectile`   | marker                   | ignores armour die, spent on what it hits, never caught | catalog |
| `Piercing`     | marker                   | runs the whole aimed line, hitting everyone in it | catalog |
| `LaunchedBy`   | `Grant`                  | the effect a launcher must grant to double this missile's die | catalog |
| `Launcher`     | marker                   | a bow / crossbow — no attack die, enchant lands on `ThrowBonus` | catalog |

"catalog" = re-attached by item name on load
(`restore_from_catalog`), never written to the save.


Components — player conditions
------------------------------

| Component  | Data   | Meaning | Saved? |
|------------|--------|---------|--------|
| `Confused` | marker | player-only stumble (a monster uses `MovementType::Confused`); blocks fast-move / auto-explore / auto-fight; HUD `CONF` | yes |

`Confused` and `Speed` haste/slow are treacherous — they never wear off
with time. Only a staircase or a wand of cancellation clears them, both
through `crate::helpers::clear_player_conditions`.


Components — traps and snares
-----------------------------

Defined in `components.rs` (nouns); the mechanics are `traps.rs`.

| Component     | Data                                        | Meaning | Saved? |
|---------------|---------------------------------------------|---------|--------|
| `Trap`        | `effect: TrapEffect`, `reveal: TrapReveal`, `revealed: bool` | a `^` entity; `revealed` latches once known | yes |
| `EntityMoved` | marker                                       | changed `Position` this turn — `trap_system` checks its tile | **transient** (cleared each `trap_system` run) |
| `Snare`       | `turns: u32`, `kind: SnareKind`              | losing turns to a trap | yes |

`TrapEffect` — enum, **saved by variant order** (`Trapdoor`, `Bear`,
`Sleep`, `Teleport`, `Arrow`, `Dart`). Keys the mechanic in `spring_trap`;
the catalog row (`TrapDef`) is name / glyph / rarity / `snare_turns`.
Arrow and dart damage scale with depth — `constants::traps`.

`TrapReveal` — enum, saved by variant order (`Sight`, `Adjacent`,
`Triggered`). Rolled equal-odds at spawn; read by `visibility.rs`.

`SnareKind` — enum, saved by variant order (`Bear`, `Sleep`). `Sleep`
forfeits the turn outright (`player_incapacitated`); `Bear` blocks
movement only — a swing still lands, a step is a bloody thrash
(`bear_trap_thrash`). `ai.rs` applies the same rule to snared monsters.


Components — score
------------------

| Component | Data         | On       | Saved? |
|-----------|--------------|----------|--------|
| `Score`   | `value: i32` | the hero | yes    |


Events and their queues
-----------------------

An input handler or the AI pushes an intent onto a queue resource; the
matching system drains the queue once per turn.

| Event / queue                       | Fields                                   | Drained by            |
|-------------------------------------|------------------------------------------|-----------------------|
| `WantsToAttack` → `AttackQueue`     | `attacker`, `target`                     | `combat_system`       |
| `WantsToUse` → `UseQueue`           | `user`, `item`, `target: Option<Position>`, `slot_idx: Option<usize>` | `item_system` |
| `WantsToThrow` → `ThrowQueue`       | `thrower`, `item`, `target: Position`    | `throw_system`        |

All three queues are transient (empty at save time).


Resources
---------

### UI and input state — all transient, `Default` on load

| Resource        | Fields                                                    | Notes |
|-----------------|----------------------------------------------------------|-------|
| `RenderConfig`  | `centered: bool`                                          | `-centered` flag. |
| `ActionMenu`    | `drop_first: bool`                                        | `-dropthrow` flag. `actions()` / `at(idx)` give the Use/Throw/Drop order. |
| `PackIsOpen`    | `open`, `selected`, `action_mode: Option<usize>`, `action_selected` | pack modal cursors. |
| `TargetingState`| `active`, `item: Option<Entity>`, `throwing: bool`, `cursor_x`, `cursor_y: i16` | aiming reticle; `throwing` swaps the range to `THROW_RANGE` and confirm to a hurl. |
| `PlayerTempo`   | `fast_parity: bool`                                       | the player half of the speed system: a `Fast` turn flips it, monsters move only when it flips back. |
| `AttackQueue` / `UseQueue` / `ThrowQueue` | `Vec<…>`                        | see Events above. |

### Run state

| Resource      | Fields                              | Saved? |
|---------------|-------------------------------------|--------|
| `GameLog`     | `history: Vec<String>` (capped 50), `unread: Vec<String>` (waiting for `--MORE--`) | **transient** — not saved; a reload starts with a fresh log ("Welcome back to roog!") |
| `Depth`        | `what: u8` — current floor, 1-based | yes    |
| `FloorChanges` | `count: u32` — staircase/portal/trapdoor traversals this run; salts `content_rng` so a repeat visit re-stocks the same layout | yes |
| `PlayerName`   | `what: String`                      | yes    |
| `DungeonLord`  | `idle_turns: u32` — turns lingered on this floor; at `DUNGEON_LORD_PATIENCE` (260) a portal opens | **transient** (resets to 0) |

`GameLog::add()` pushes to both `history` and `unread`.

The log panel is plain white except for a sparing set of colours
(`hud::log_line_color`), applied only to a packed line that mentions the
player ("you"/"your") and falls into one of: a curse taking hold (dark
red), a dazzle (magenta), the low-HP warning (red — "You are badly
wounded!", fired once as HP crosses down through
`constants::player::LOW_HP_WARNING_FRACTION` of max, see
`helpers::apply_damage`), the player's own speed shifting (cyan hasted,
dark cyan slowed), or the player's own throw/fire (yellow). Matched by
substring, not by threading a colour through every `GameLog::add()` call
— see `hud::log_line_color` for the exact phrases it keys on.

Other run-state resources live outside this file: `Map`, `GameRng` /
`RngSeed`, `Identified` / `ItemAppearances` (`identify.rs`), `BloodStains`,
`Smoke` and `Corpses` (`map.rs`), `GameState` (`state.rs`). The save file also
persists the RNG state, the identification tables and the dark-tile set —
see `models/src/saveload.rs`. `Smoke` and `Corpses` are not saved, like
`BloodStains`: a fire blast's lingering puffs (`SMOKE_LINGER_TURNS`, 4 real
turns, ticked by `smoke_system`) and a death's corpse marks
(`helpers::death_burst`) are cosmetic and reset to empty on load.

`FxRng` (`map.rs`) is a second RNG stream, seeded from the run seed but
salted apart from `GameRng` the same way `ItemAppearances` gets its own —
for animation/particle randomness only (a death burst's fling direction and
reach, a blood splatter's spray). Nothing that reads it feeds back into
gameplay, so a purely cosmetic feature (`-nb` skipping the roll entirely,
say) can never perturb the shared `GameRng` stream everything else depends
on for determinism. Not saved — a reload just reseeds it fresh.


See also
--------

  content-tables.md            the tables that attach these components
  spawn-api.md                 the functions that build entities
  input-and-turn-loop.md       what reads and writes the UI resources above
  ../how-to/add-an-effect.md   adding a new marker / modifier component
  ../explanation/data-driven-content.md  why behaviour is not in the row
