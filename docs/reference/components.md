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

Almost every `Component`, `Resource` and `Event` is declared in `models/src/components.rs`. That file is **nouns only** — a component says what a thing *is*, never what happens as a result. The verbs are the systems: `combat.rs`, `ai.rs`, `items/`, `visibility.rs`, `traps.rs`.

The exceptions are the handful of resources that come with real logic attached and live with it instead: `Shake` (`shake.rs`), `AutoExplore` / `AutoPickup` (`autoexplore.rs`), `FastMove` (`fastmove.rs`), and the pack screen's `PackIsOpen` / `PackMode` / `ItemAction` (`pack.rs`). All of them are re-exported from `models`, so a call site never has to know which file they came from.

A `Bundle` — the struct that assembles components for one spawn (`MonsterBundle`, `TrapBundle`) — is **not** here. It lives next to its content table (`monsters.rs`, `traps.rs`), because it names that table's `Def` row and is the one place an entity of that kind is described. See `../explanation/data-driven-content.md`, "Where the pieces live".

Two shapes recur:

  * **Marker** — no fields. Presence is the fact (`Player`, `Blood`, `Curse`, `Confused`).
  * **Type key** — one enum naming which one it is (`Potion` → `PotionEffect`). The key is identity for `identify.rs` and the save file; it is never behaviour. The catalog row turns a key into the components that do something; the mechanic is a `match` on the key.

"Saved?" below means the field survives `models/src/saveload.rs`. A **transient** field is rebuilt every frame or every load and deliberately left out of the save. A **catalog** component is re-attached by name on load (`restore_from_catalog`), so it is never written either.


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


Components — what the player is
--------------------------------

`Body` (`models/src/body.rs`, not `components.rs` — it is the one type this
whole page's "nouns only" file doesn't hold, because it is read at
character-build time, not by a system) is which of three shapes the run's
hero descends in: `Nihil` (default), `Lurk`, or `Monster(&'static
MonsterDef)` — a bestiary row worn as a costume, `-am <species>`. One enum
rather than three flags, because a player is exactly one of them.

| Type | Data | On | Saved? |
|------|------|----|--------|
| `StartingBody` | `Resource`, `Body` | which body the *next* spawned player wakes up in; read once by `map::initialize_world` | not saved directly — see below |
| `MonsterBody`  | `Component`, `&'static MonsterDef` | the player, only when wearing a species | **no** — read back from `Name`, which *is* the species name, on load |
| `Lurk`         | effect marker (`crate::effects`) | the player, only as a lurk | yes — an ordinary row in `EFFECTS`, held like anything else a creature was born with |

Neither class nor species costs `saveload::EntitySave` a field: the save
format isn't versioned, and every field it has ever gained has killed every
save already in progress. So a body is never written down directly — it is
reconstructed from something that was already being saved for another
reason (`Name`, or the effect ledger).

`Body::equip_refusal` is the one gate on whether a body can wear something,
and `Body::innate_tempo` is the tempo it returns to when a staircase lifts
whatever the floor lent it — both take `&World` and an `Entity` rather than
reading `Renderable`'s glyph back out, on purpose: nothing else in the game
guesses what a creature is from how it's drawn.

See `../how-to/add-a-body.md` for adding a fourth one.


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

Combat: `damage = (1d[power] + power_bonus) - (1d[armor] + armor_bonus)`, both sides rolled independently, nothing ever misses. `power` is dropped by a poisoned dart trap — one point per depth tier — and healed back toward `max_power`.


Components — speed and tempo
----------------------------

The player is the clock. Monsters bank energy on each player turn and spend it in `ai.rs`; the player's own tempo is run by the engine loop through the `PlayerTempo` resource.

| Component | Data | On | Saved? |
|-----------|------|-----|--------|
| `Speed`   | `kind: SpeedKind`, `energy: i32` | every actor | `kind` yes; `energy` **transient** (resets to 0) |

`SpeedKind` — enum, **saved by variant order** (`Slow`, `Normal`, `Fast`; `Normal` is `#[default]`):

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
| `Invisible`| marker — intrinsically unseeable without `SeesInvisible` | the phantom, and the invisibly-stashed floor item a floor hides at `population::HIDDEN_ITEM_CHANCE` | yes |
| `Spotted`  | marker — inside the player's viewshed this turn | anything currently seen | **transient** |
| `Detected` | marker — drawn on the map even out of view, in `DarkMagenta` | whatever a potion of magic / monster detection turned up | yes |

A detection that finds a *stashed* item strips its `Hidden` and `Invisible` too, the way a ring of perception does — `Detected` only paints tiles the player cannot see, so a stash left hidden would glow from across the floor and vanish on arrival.

`revealed_tiles` is indexed by `crate::map::tile_index`.

`Detected` is an effect held for `Lifetime::Floor`, and leaving the floor despawns every entity that carries one anyway. It says *where*, not what is happening there — a detected monster is never announced and never animates.


Components — items on the floor and in the pack
-----------------------------------------------

| Component | Data | On | Saved? |
|-----------|------|-----|--------|
| `Item`    | marker — can be picked up | every item | yes |
| `Value`   | `amount: i32` — score, paid the moment it is picked up | treasure coins, the relic | yes |
| `Pickup`  | `effect: PickupEffect`, `amount: i32` — works where it lies and is gone; never carried | every coin | yes |
| `Backpack`| `items: Vec<Entity>` — inventory order | actors that carry | yes |
| `Consume` | marker — used up on use | potions, scrolls | yes |
| `Battery` | `charges: i8` | wands | yes |
| `Stack`   | `count: u8` — how many share one slot | ammunition only | yes |
| `Ranged`  | `range: i32` — feeds the zap reticle | wands | yes |
| `Amulet`  | marker — the Element of Yoord; carrying it inverts the staircases | the relic | yes |

`Stack` tops back up to `STACK_LIMIT` on pickup. `Backpack` holds at most `PACK_CAPACITY` slots; `stow` refuses anything past that ("Your pack is full.") and leaves it on the floor. Both are `constants::items`.

**`items::pick_up` is the only way anything leaves the floor.** The input handler calls it and knows none of what follows: the invisible-stash reveal, `Value` into the score, a `Pickup` spent where it lies, or `stow`. `None` back means the item is still there.

A `Pickup` is a different kind of thing from an item you stow, in three ways that all follow from "never carried":

* **A full pack is no obstacle.** There is nothing to find room for.
* **It is left alone when it would do nothing.** `items::would_help` gates it — a red coin at full health stays on the floor, and `autoexplore::known_item_tiles` skips it so a walk never beelines for something it will refuse. It keeps until the day it helps.
* **It can be shot** — and the shooter gets the effect across the room, plus a burst twice a trap's width. A shot that stops on a *creature* standing on one counts, and the renderer advertises that with a magenta cell. See `content-tables.md`, "Trick shots".

`PickupEffect`: Coin, Health, Power, Cleanse, Strength, Platinum, Forge — saved by variant order, mechanic in `models/src/items/pickups.rs`.


Components — item type keys
---------------------------

| Component | Key enum      | Mechanic                              | Saved? |
|-----------|---------------|--------------------------------------|--------|
| `Potion`  | `PotionEffect`| `items/potions.rs`                    | key yes |
| `Scroll`  | `ScrollEffect`| `items/scrolls.rs`                    | key yes |
| `Wand`    | `WandEffect`  | `items/wands.rs`; thrown, `items/throwing.rs` | key yes |
| `Ring`    | `RingEffect`  | numbers + `Grants` from the `RingDef` row; the three with verbs are `items/rings.rs` | key yes |
| `Curse`   | marker        | equipped-and-stuck until remove curse destroys it, or a scroll of enchantment burns it off | yes |
| `Vorpal`  | `bane: String`| any blooding hit slays `bane` (or any `VorpalTarget`) outright | yes |
| `KnownQuality` | marker   | this instance's enchantment plus/curse/vorpal bane are known — set by wearing it (`equipment::toggle_equipped`/`equip_silently`) or a scroll of identify singling it out | yes |

All four key enums are **saved by variant order** — append, never reorder. The catalog tables (`crate::catalog`) are the other thing keyed off these enums.

`KnownQuality` is per-*instance*: two rings of protection are two separate rolls of the curse dice, so each needs its own `KnownQuality`, set the moment it's worn (`equipment::toggle_equipped`/`equip_silently`) or a scroll of identify singles it out. `identify::display_name` reads it to decide whether to print a weapon/armour/launcher's `+N` prefix, a `(cursed)` suffix, or a `(vorpal vs. X)` suffix — hidden for anything not yet known. Potions, scrolls, wands and rings carry no such hidden state of their own; they always show their true name.

`WandEffect::needs_target()` is `false` only for the wand of light (it floods the room, no reticle).

`PotionEffect`: Blindness, Confusion, ExtraHealing, FruitJuice, GainStrength, Haste, Healing, MagicDetection, MonsterDetection, Paralysis, Poison, RaiseLevel, RestoreStrength, SeeInvisible, Water.

`ScrollEffect` (every arm wired; `BlankPaper` does nothing on purpose): MonsterConfusion, MagicMapping, HoldMonster, Sleep, EnchantArmor, Identify, ScareMonster, FoodDetection, Teleportation, EnchantWeapon, CreateMonster, RemoveCurse, AggravateMonsters, BlankPaper, VorpalizeWeapon.

`WandEffect`: Light, Striking, Lightning, Fire, Cold, Polymorph, MagicMissile, HasteMonster, SlowMonster, DrainLife, Nothing, TeleportAway, TeleportTo, Cancellation.

`RingEffect`: Protection, Strength, Perception, Adornment, AggravateMonster, Sharpshooting, IncreaseDamage, Regeneration, SlowDigestion, Teleportation, Stealth, MaintainArmor.


`Element` — not a component
---------------------------

`Element` (`Fire` / `Cold` / `Drain`) is a plain enum, not a component — it never lives on an entity. It sits in `components.rs` because both the wand mechanic and the throwing mechanic use it.

| Method              | Returns                                          |
|---------------------|-------------------------------------------------|
| `Element::of(wand)` | the element a `WandEffect` carries, or `None`.   |
| `.immunity()`       | the `Grant` that shrugs it off (`FireImmune` / `ColdImmune` / `Undead`). |
| `.noun()`           | "flames" / "cold" / "evil magic" for the log.    |


Components — throwing and launchers
-----------------------------------

A missile and a launcher never name each other; they meet at an effect (`FireArrow` / `FireQuarrel`). Resolution is `items/throwing.rs`.

| Component      | Data                     | Meaning                          | Saved? |
|----------------|--------------------------|----------------------------------|--------|
| `ThrownDamage` | `i32`                    | die rolled on impact; no component ⇒ bounces off harmlessly | catalog |
| `LaunchedDamage` | `i32`                  | die rolled instead, once `LaunchedBy` fires | catalog |
| `Projectile`   | marker                   | ignores armour die, spent on what it hits, never caught | catalog |
| `Piercing`     | marker                   | runs the whole aimed line, hitting everyone in it | catalog |
| `LaunchedBy`   | `Grant`                  | the effect a launcher must grant to switch this missile to its `LaunchedDamage` die | catalog |
| `Launcher`     | marker                   | a bow / crossbow — no attack die, enchant lands on `ThrowBonus` | catalog |

"catalog" = re-attached by item name on load (`restore_from_catalog`), never written to the save.


Components — player conditions
------------------------------

| Component   | Data   | Meaning | Saved? |
|-------------|--------|---------|--------|
| `Confused`  | marker | player-only stumble (a monster uses `MovementType::Confused`); blocks fast-move / auto-explore / auto-fight; HUD `CONF` | yes |
| `Blind`     | marker | player-only: viewshed cut to the 3x3, every glyph in it painted white, every mob `Hidden` (so auto-walk and auto-fight stall too). The AI is unaffected — see below. HUD `BLND` | yes |
| `Paralyzed` | marker | `Speed` dropped to `Slow`, and for the player a `PARALYSIS_LOST_TURN_CHANCE` share of turns forfeited outright before a key is read. A monster carries it for the renderer's tint too, plus a log line of its own if the player can actually see it land (`conditions::paralyse`). HUD `PARL` | yes |
| `ConfusingTouch` | marker | hands charged by a scroll of monster confusion: the next blow the bearer *lands* confuses what it hits and is spent doing it (an `ABILITIES` row keyed on `Moment::OnHit` — see `content-tables.md`). Not an impairment, and it survives a staircase. HUD `GLOW` | yes |
| `Plated` | marker | the platinum coin's promise: reach the next **staircase** unhurt and it pays a permanent point of attack or defence *die*, the dungeon's coin flip. HUD `PLAT` | yes |
| `Forged` | marker | the forge coin's promise: the same terms, paying a point of *plus* on the wielded weapon or worn armour, exactly as the matching scroll would. HUD `FORG` | yes |
| `MagicWard` | marker | the spell Magic Ward: for the rest of the floor, checked in `helpers::apply_hit` (every `Hit` with `magical` set -- zapped, thrown, breathed or cast -- bounces off with a cosmetic ricochet, `wands::ward_ricochet`) and `abilities::fire_on_hit` (nothing a monster's landed blow carries with it takes hold). Lifted like `Confused`/`Blind`/`Paralyzed` above. HUD `WARD` | yes |
| `Bided` | marker | the spell Bide: `combat::fold_matchup` folds `constants::combat::BIDE_ATTACK_BONUS` into the bearer's very next attack roll, and `combat::resolve_attack` removes the marker the instant that roll is folded — hit, glancing or miss. Anything else done with a turn instead (a step, a used/thrown item, another spell) spends it unfired, via `equipment::reset_momentum`. HUD `BIDE` | yes |

`Plated` and `Forged` are the only conditions a staircase does not lift — the staircase is what *settles* them (`items::settle_promises`, and only for `LevelChange::Stairs`: a trapdoor is falling, not arriving). Any damage at all takes them back, through `helpers::took_damage`, which is also where the low-HP warning lives: the two things that happen to a creature *because it was hurt*, in one place, called from both damage paths.

`Confused`, `Blind`, `Paralyzed` and `Speed` haste/slow are treacherous — they never wear off with time. Only a staircase or a wand of cancellation clears them, all through `crate::conditions::clear_player_conditions`.

The first three are **effects**, not components of their own: rows in `crate::effects`'s `EFFECTS`, held for `Lifetime::Floor`, and listed once in `conditions::AFFLICTIONS` with the words for lifting each. That one table is what `afflicted`, `cure_one_condition` and `clear_player_conditions` all read — they used to be three hand-written lists in three different orders. The table's order is worst-first, because a cure takes the first row it finds. `Speed` haste/slow is not a row and cannot be: a tempo is a value, not a marker something either has or has not.

The verbs that put them on — `confuse`, `blind`, `paralyse`, `hasten`, `shift_entity_speed`, `snare` — live in `crate::conditions`, one per affliction, and each one already knows the difference between the player and a monster. `snare` is the exception to the "never wears off" rule above: it is counted in turns from the moment it lands, and it logs nothing, because the sentence belongs to whatever pinned you. A blinded monster has no viewshed to put out, so it gets `MovementType::Confused`; a paralysed one gets the slowing and no coin flip.

**A creature carries at most three conditions.** A fourth sheds the oldest, in `effects::lend` — the one gate every transient effect already passes through, so a trap, a potion and a monster's touch are all capped by the same line and none of them needs to know the rule exists. Oldest first because the newest is the one that just happened, and a blow that lands should be felt. Shedding runs the same `AFFLICTIONS` `after` column a cure does (`conditions::after_lifted`), so a shed blindness recomputes the viewshed exactly as a cured one would.

What counts is `Held::is_condition`: transient **and** named by `conditions::is_condition`, which reads the lists that already declare conditions — `AFFLICTIONS`, `FLOOR_BOONS`, `effects::HOLDS`, and `OTHER_CONDITIONS` for the four with no list of their own. Both halves have to hold, and the default is exemption. A ring of regeneration lends `Lifetime::WhileEquipped` and fails the first half; a potion of magic detection's `Detected` mark is transient but is nothing the marked creature feels, and fails the second. Neither may shoulder a real condition off.

**Shedding is never silent.** `conditions::shed_line` gives the sentence: a hold says what it already says when its own clock runs out (`Effect::ends`), and everything else borrows the staircase's phrasing — "You are no longer blind." — because that is the sentence the player has already learned to read as "that one is over". Only the player is told, the same rule `tick_effects` keeps. A condition whose badge vanished with nothing said would read as a bug in the badge line rather than as a rule.

### The priority badges

`Speed` haste/slow, `Plated` and `Forged` are **priority badges**: they are outside the ledger, so they are neither counted against the ceiling nor ever shed for a fourth condition. That is deliberate, not an oversight of the cap.

They earn it by being things the player cannot act correctly without. A tempo changes what every single step costs, and it is a value on `Speed` rather than a marker something either has or has not — there is nothing to shed. The two coin promises are standing bets that any damage at all cancels (`helpers::took_damage`) and a staircase settles (`items::settle_promises`); a promise silently displaced by a fourth condition would be a bet the player is still playing around and can no longer see. So they always show.

The bill for that lands on the line's width. Three conditions, a tempo, a ring's `STLH`, both promises and an auto-walk badge is a badge run long enough to reach the centred `DEPTH` and paint over it. **That is accepted.** Nothing panics — `Screen::puts` no-ops past the frame edge — and a player wearing that much at once did it to themselves, one potion and one coin at a time. Correcting it would mean either dropping a badge the player needs or making the line's three anchors depend on each other again, and the whole point of splitting the HUD in two was that they do not.

**Blindness does not blind the dungeon.** `crate::ai` recomputes the view the player *would* have (`visibility::visible_from(map, pos, false)`) when they are `Blind`, so the monsters in the room still know exactly where they are. Drinking one is never a way to hide.


Components — traps and snares
-----------------------------

Defined in `components.rs` (nouns); the mechanics are `traps.rs`.

| Component     | Data                                        | Meaning | Saved? |
|---------------|---------------------------------------------|---------|--------|
| `Trap`        | `effect: TrapEffect`, `reveal: TrapReveal`, `revealed: bool` | a `^` entity; `revealed` latches once known | yes |
| `EntityMoved` | marker                                       | changed `Position` this turn — `trap_system` checks its tile | **transient** (cleared each `trap_system` run) |

`TrapEffect` — enum, **saved by variant order** (`Trapdoor`, `Bear`, `Sleep`, `Teleport`, `Arrow`, `Dart`). Keys the mechanic in `apply_trap_effect`; the catalog row (`TrapDef`) is name / glyph / rarity / `snare_turns`. Arrow and dart damage scale with depth — `constants::traps`.

`TrapReveal` — enum, saved by variant order (`Sight`, `Adjacent`, `Triggered`). Rolled equal-odds at spawn; read by `visibility.rs`.

The three holds — `Asleep`, `Pinned`, `Rooted` — are effects rather than components of their own, held for `Lifetime::Turns` and aged by `effects::tick_effects`. `Asleep` forfeits the turn outright (`player_incapacitated`); `Pinned` blocks movement only — a swing still lands, a step is a bloody thrash (`bear_trap_thrash`). `Rooted` is `Pinned` without the teeth: still biting, no thrash damage — a scroll of hold monster's doing, and the one no trap lays. `ai.rs` applies the same rules to held monsters. Each runs on its own clock, so a creature can carry more than one.

`Petrified` is a fourth hold and the odd one out. A medusa's gaze puts it on, it forfeits the victim's turn exactly as `Asleep` does, and it *protects*: `effects::stone_chip` caps every hit at `constants::combat::CHIP_DAMAGE` and never lets one take the victim's last point, on both damage paths (`combat::resolve_attack` and `helpers::apply_hit`), so the blow is reported as a chip rather than as a wound. Its one exception is a war hammer, which lends its wielder `ShattersStone` and goes through whole. It is deliberately not one of the `HOLDS` the two teleports let go of: those are things holding a creature in a place, and stone travels with the victim.


Components — score
------------------

| Component | Data         | On       | Saved? |
|-----------|--------------|----------|--------|
| `Score`   | `value: i64` | the hero | yes    |

Every change to it goes through `models/src/score.rs`, which is the whole scoring table in one screen:

| What | Worth | Where |
|---|---|---|
| A creature dies | `KILL_PER_MAX_HP` × its `max_hp` | `combat::pay_for_the_corpse`, from both the melee path and `finish_indirect_kill` |
| More than one dies in a turn | the turn's kills together, ×(1 + `COMBO_BONUS_PER_KILL` per corpse past the first) | `score::Combo`, settled by `score_turn_system` |
| A staircase is used | `STAIR_PER_TIER` × (difficulty tier + 1) | `map::levels::award_stair_score` |
| A ring of adornment goes on | doubled | `items::rings::wear_adornment` |
| Treasure picked up | its `Value` | `items::pick_up` → `score::award` |
| The run is won | doubled | `map::levels::win_with_style` |

Deaths are paid for without asking whose blade it was: half the ways a monster dies have no attacker entity to ask about. Only a *staircase* pays — a trapdoor, the Dungeon Lord's portal and a potion of raise level all move you between floors for free. Treasure pays as it is *taken*, not at the end of the run: a gold coin's `amount`, and the relic's 25000 the moment it is in hand — and the `Value` comes off with the payment, so however the relic comes to hand again (it cannot be dropped, but deep water throws it back) it never pays twice.

Kills are **not** paid one at a time. `award_kill` only files the corpse under `Combo`, the turn's pile; `score_turn_system` — dead last in the schedule, after everything that can kill — totals it with the combo multiplier, pays it in one go, and empties the pile. A multiplier applied to a number that is still growing is not one anybody can read, and one turn's killing never combos into the next. `score::double` settles the pile first, so a run that ends on the same turn as a kill doubles a score that already counts it.

`i64`, and every write to it saturates. Nothing caps how many rings of adornment a dungeon hands out and every one of them doubles the score, so the arithmetic has to have a ceiling it stops at rather than one it wraps past — a score is always a multiple of a hundred, and a multiple of a hundred that wraps an integer lands on exactly zero.

Every payment lights the `ScoreFlash` resource, which the HUD shows in the scorekeeper's place for exactly one frame (armed during the turn, aged at the same tail, dark by the next): `+700` in a random bright colour, `COMBO! +2400` with the word in the stripes `pride::stripes(world)` returns, or `DOUBLE` in the same. A combo also writes one log line — "With style.", or `COMBO_PRIDE_CHANCE` of the time "With pride." All of its rolls are off `FxRng`, never `GameRng`: decoration does not get to move the gameplay dice.


Events and their queues
-----------------------

An input handler or the AI pushes an intent onto a queue resource; the matching system drains the queue once per turn.

| Event / queue                       | Fields                                   | Drained by            |
|-------------------------------------|------------------------------------------|-----------------------|
| `WantsToAttack` → `AttackQueue`     | `attacker`, `target`                     | `combat_system`       |
| `WantsToUse` → `UseQueue`           | `user`, `item`, `target: Option<Position>`, `slot_idx: Option<usize>` | `item_system` |
| `WantsToThrow` → `ThrowQueue`       | `thrower`, `item`, `target: Position`    | `throw_system`        |
| `WantsToCast` → `SpellQueue`         | `user`, `effect: SpellEffect`, `target: Position` | `spell_system`   |

All four queues are transient (empty at save time).


Resources
---------

### UI and input state — all transient, `Default` on load

| Resource        | Fields                                                    | Notes |
|-----------------|----------------------------------------------------------|-------|
| `RenderConfig`  | `centered: bool`                                          | `-centered` flag. |
| `QuitPrompt`    | `open: bool`                                              | the "Really quit?" modal, raised by `Q` / `X` **with nothing else open** and answered `y` / `n`. Ctrl+C bypasses it; `Esc` never raises it. |
| `PackIsOpen`    | `open`, `mode: PackMode`, `selected`, `action_mode: Option<usize>`, `action_selected` | pack modal cursors; `selected` and `action_mode` are **backpack indices**, not row numbers. `pack.rs`. |
| `PackMode`      | enum: `Browse` `Use` `Throw` `Drop` `Equip` `Quaff` `Read` `Zap` `Wield` `Wear` `PutOn` | which key opened the pack, and therefore its title, its rows, and what picking one does. See below. |
| `AutoPickup`    | `enabled: bool` (default `true`)                          | the `A` toggle: whether auto-explore detours for loot. `autoexplore.rs`. |
| `TargetingState`| `active`, `item: Option<Entity>`, `throwing: bool`, `cursor_x`, `cursor_y: i16` | aiming reticle; `throwing` swaps the range to whatever `throw_reach` gives the item and confirm to a hurl. |
| `PlayerTempo`   | `fast_parity: bool`                                       | the player half of the speed system: a `Fast` turn flips it, monsters move only when it flips back. |
| `AttackQueue` / `UseQueue` / `ThrowQueue` / `SpellQueue` | `Vec<…>`            | see Events above. |
| `Shake`         | `enabled: bool` (`-nshake`), plus a private kind + age    | screen shake. Gameplay arms one with `shake::kick_shake(world, ShakeKind::…)` and forgets; the engine ages it, reads `offset()` and `settle()`s it. See below. |

### The pack screen

`PackMode` (`pack.rs`) is a four-column table — title, "nothing to show" line, verb, row filter — with one row per key that opens the pack:

| Mode     | Key | Shows                     | Picking a row |
|----------|-----|---------------------------|---------------|
| `Browse` | `i` | everything                | opens the `ItemAction::MENU` modal |
| `Use`    | `a` | everything                | `ItemAction::Use` |
| `Throw`  | `t` | everything                | `ItemAction::Throw` |
| `Drop`   | `d` | everything                | `ItemAction::Drop` |
| `Equip`  | `e` | anything with `Equipped`  | `ItemAction::Use` |
| `Quaff`  | `q` | anything with `Potion`    | `ItemAction::Use` |
| `Read`   | `r` | anything with `Scroll`    | `ItemAction::Use` |
| `Zap`    | `z` | anything with `Wand`      | `ItemAction::Use` |
| `Wield`  | `w` | `Equipped { slot: Hand }` | `ItemAction::Use` |
| `Wear`   | `W` | `Equipped { slot: Body }` | `ItemAction::Use` |
| `PutOn`  | `P` | `Equipped { slot: Finger }` | `ItemAction::Use` |

Everything but Browse, Throw and Drop is `Use`, because "quaff", "read" and "wear" are all one verb once `item_system` has the item in hand — the mode only decides what you were offered.

`ItemAction::MENU` is the fixed Use / Throw / Drop order of the Browse modal (`ItemAction::at(idx)` indexes it). It was a resource with a flag behind it (`ActionMenu`, `-dropthrow`) while that modal was the only route to any of the three; `a`, `t` and `d` are what replaced the flag.

`pack_rows(world, mode)` is the single place the filter is applied, and it returns **backpack indices**. Both the renderer and the cursor key off that list, so a menu can never highlight or act on a row it isn't showing, and a row keeps its pack letter in every mode (the potion that is `c` in the pack is `c` in the quaff menu, alone on screen or not).

Worn gear still appears on the equip menus — that is how it comes back off.

### The screen shake

`Shake` (`shake.rs`) is the effect layer's second half, and the only cosmetic resource that is deliberately *not* played the way [`Particles`] is. Thirteen call sites arm it, and nothing else may:

| `ShakeKind` | Armed by | Shape |
|-------------|----------|-------|
| `Hit`       | the light stuff. Anything of the player's that got through armour: `combat::resolve_attack` on an ordinary blow — not a crit (that is `Heavy`), not a kill (that is `Kill`), never a glancing blow; `items::throwing::strike_victim` on a throw or shot that drew blood; `items::wands::fire_bolt` on a bolt that bit something the player can see; `traps::trap_flourish` when a shooting trap (arrow or dart) goes off in sight, hit or miss — there the shake is the mechanism firing, not the damage; and `traps::burst` on the first burst of a trick shot the player can see — deliberately the lightest kick there is, because a trick shot is a *chain* and a chain of heavy thumps is a map that never stops moving | 80 ms, 1 cell — a tick |
| `Kill`      | `combat::kill_shake`, from `resolve_attack` and `finish_indirect_kill`, when the player can see the victim's tile | 120 ms, 1 cell — short |
| `Heavy`     | everything that hits hard: `combat::resolve_attack` on the player's own excellent hit and on a garrote's pop; `items::wands::elemental_blast` if the player can see the blast centre; `items::rings::do_it_with_style`, the adornment / victory flourish; and the three spells that go off with a bang — `circle_of_death`, `meteor_strike`, `frost_nova` | 260 ms, 2 cells — medium |
| `Wounded`   | `helpers::warn_if_newly_low`, on the same crossing that logs "You are badly wounded!" | 460 ms, 2 cells — long, and the heaviest there is |

**The player's own death arms nothing.** Both player-death paths in `combat.rs` blank the `@` and set `Ending::player_dead` without kicking the map: a death is watched, not felt through the floor. What replaces the shake is time — `helpers::death_burst` runs the player's burst at `PLAYER_DEATH_STRETCH` (3x) the length of a monster's, which it can afford because nothing is waiting behind it: the run is over and the death screen is next.

The durations are on `ShakeKind::shape()`, not in `constants.rs`. A kick only displaces an already-running shake if it is worth more than what is *left* of it, so a kill mid-blast cannot truncate the blast — and an ordinary hit landed during either cannot truncate anything. Amplitude 2 means the first half throws the map two cells and the rest one; a terminal has no half-cell to decay through.

No kind may be shorter than two of `play_shake`'s 33 ms frames: it ages the shake *before* it draws, so anything shorter would retire without ever displacing a frame the player saw. `Hit`'s 80 ms is that floor. Nothing asserts it — the shake tests were removed with the rest of the feel layer's coverage — so the constraint lives in `ShakeKind::shape`'s doc comment, and a kind that breaks it shows up as a shake nobody can see.

A glancing blow is the one hit that draws blood and arms no shake. It gets `Particles::clink_spark` instead of the landed hit's `hit_spark` — same shape, no warm colour, gone quicker — because the chip-damage floor exists so a turned-aside swing isn't *nothing*, not so it lands like a real one. A throw or shot the armour turned aside (`strike_victim`'s "glances off" branch) is the same rule at range, and takes the same spark — harsher, in fact: there is no chip-damage floor out there, so it deals nothing at all.

Melee excludes a lethal blow from the `Hit` kick by hand; the ranged path excludes it by asking whether the victim is at 0 HP, because `helpers::apply_damage` leaves a lethal shot for `reaper_system` to finalise. Either way the kill's own kick — the sight-gated one — is the only shake a killing hit arms.

The sight gate on `elemental_blast`, `kill_shake` and `fire_bolt` is a real rule, not politeness: a shake for a blast — or a death — in an unexplored room would hand the player information the renderer goes out of its way not to draw. `helpers::player_sees` is the shared check, the same one the trap messages use.

Melee and the throw path need no such gate, because both *log* the damage they deal whether or not it was seen — the shake says nothing the message line hasn't. A bolt is the one damage source that logs nothing per victim, so `trace_bolt` reports whether any of the HP it took came off a creature on a visible tile (`Bolt::bit_something_seen`), which is why that answer is computed there rather than in `fire_bolt`: the walk is the only place that still knows which tile each victim stood on.

Unlike every other animation in the game the shake **never blocks input** — see `rendering.md`, "The screen shake", for why and for how `play_shake` enforces it.

### Run state

| Resource      | Fields                              | Saved? |
|---------------|-------------------------------------|--------|
| `GameLog`     | `history: Vec<String>` (capped 50), `unread: Vec<LogEntry>` (waiting for `--MORE--`) | **transient** — not saved; a reload starts with a fresh log ("Welcome back to nihilurk!") |
| `Depth`        | `what: u8` — current floor, 1-based | yes    |
| `FloorChanges` | `count: u32` — staircase/portal/trapdoor traversals this run; salts `content_rng` so a repeat visit re-stocks the same layout | yes |
| `PlayerName`   | `what: String`                      | yes    |
| `DungeonLord`  | `idle_turns: u32` — turns lingered on this floor; at `DUNGEON_LORD_PATIENCE` a portal opens | **transient** (resets to 0) |

`GameLog::add()` pushes plain text to both `history` and `unread`. `GameLog::add_colored(message, category)` is the same, except the copy that lands in `unread` — a `LogEntry` — also carries a `LogCategory`. `history` never gets painted, so it stays bare `String`.

The log panel is plain white except for a sparing set of colours, one `LogCategory` per: a curse taking hold (dark red), a dazzle (magenta), the low-HP warning (red — "You are badly wounded!", fired once as HP crosses down through `constants::player::LOW_HP_WARNING_FRACTION` of max, by `helpers::warn_if_newly_low` — which `helpers::apply_damage` calls for every trap, dart and bolt, and `combat::resolve_attack` calls directly, melee being the one damage path that applies its own damage and would otherwise never report the crossing), the player's own speed shifting (cyan hasted, dark cyan slowed), or the player's own throw/fire (yellow). A trick shot and a combo's "With style." are magenta too. Every category is decided once, by the call that writes the message, and never re-derived from the rendered sentence — see `components::LogCategory` and `hud::log_paint`.

**Colour is per message, not per painted row.** Several messages share a row (`hud::pack_line_segments`, which is what `log_view` now returns — the messages on each row, in order, displayed joined by one space), and the renderer paints each one with its own colour. Asking the question of the joined row instead is the bug that had one shouting message repainting every sentence beside it.

`hud::log_paint(entry, stripes)` is the painter's entry point and returns a `LogPaint`: `Solid(Color)` for every category but `LogCategory::Pride`, which comes back `Striped` — the one line that comes out in colours rather than a colour (`hud::pride_line()`, "With pride.", the rare alternative to "With style." on a combo), painted a character at a time, cycling the stripes so red follows purple and no two neighbouring letters match. The stripes come from `pride::stripes(world)`; see `models/src/pride.rs`.

Other run-state resources live outside this file: `Map` (`map.rs`), `GameRng` / `RngSeed` / `FxRng` (`map/streams.rs`), `BloodStains`, `Smoke` and `Corpses` (`map/overlays.rs`), `GameState` (`state.rs`). The save file persists the RNG state and the dark-tile set — see `models/src/saveload.rs`.

**No cosmetic state is saved, on purpose.** `BloodStains`, `Corpses`, `Smoke`, `Particles`, `Shake`, `ScoreFlash` and `FxRng` are all rebuilt empty by `load_game`. A fire blast's lingering puffs (`SMOKE_LINGER_TURNS`, ticked by `smoke_system`), a death's corpse marks (`helpers::death_burst`) and the blood under a fight say nothing the game needs back — and the three map-sized overlays alone would come to 2.2 KB, more than the whole save file they would be joining. A reloaded floor is the floor you left, scrubbed of the mess you made on it. The same reasoning keeps the map itself and the message log out; the four exclusions are listed at the top of `saveload.rs`.

`FxRng` (`map/streams.rs`) is a second RNG stream, seeded from the run seed but salted apart from `GameRng` — for animation/particle randomness only (a death burst's fling direction and reach, a blood splatter's spray). Nothing that reads it feeds back into gameplay, so a purely cosmetic feature (`-nb` skipping the roll entirely, say) can never perturb the shared `GameRng` stream everything else depends on for determinism. Not saved — a reload just reseeds it fresh.


See also
--------

  content-tables.md            the tables that attach these components
  spawn-api.md                 the functions that build entities
  ../how-to/work-with-the-ecs.md  reading and changing these
  ../explanation/ecs-in-nihilurk.md   what each of the three nouns may be
  input-and-turn-loop.md       what reads and writes the UI resources above
  ../how-to/add-an-effect.md   adding a new marker / modifier component
  ../how-to/add-a-body.md      adding a new hand-written `Body` variant
  ../explanation/data-driven-content.md  why behaviour is not in the row
