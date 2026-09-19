# Ability coupling: assessment and refactoring plan

    Status      In progress. Architecture decisions marked [BAE] are not mine
                to make; the ones taken are recorded with their date.
    Date        2026-09-19
    Baseline    31015cf, `cargo +stable test --workspace` green, 315 tests.
                Phase 0 adds 31, for 346.

---

## 0. Order of work

    [done]  Phase 0      the ability net (§4)
    [done]  ConfusingTouch and Bided become effects (§6.1)
    [done]  test fixtures    no test reaches for another file's names (§6.2)
    [done]  Effects v2       lifetimes replace the bitset (§6.3)  <- save break
    [done]  Phase 1          kill MONSTER_DANGERS (§4)
    [done]  every creature is a they (§6.4)
    [done]  Phase 2          one ABILITIES table, `when` field (§5, option B)
    [done]  Phase 3          the damage funnel (§4)
    [done]  Phase 4          the AI context struct (§6.8)

    [done]  §2.3 solved      the dragon fits the one table (§6.9)

Effects v2 moved ahead of Phase 1 because it breaks the save format, and the
save format should break **once**. Anything else that wants a format change
rides along with it or waits.

---

## 1. The premise, corrected

Two examples were named. Both are already fixed, and they are the two that
prove the intended design works:

**`FireImmune` is not checked at the fire site.** There is exactly one seam,
`Element::immunity()` (`models/src/components.rs:687-692`), and two call sites
in the whole workspace: `items/wands.rs:38` and `items/moves.rs:671`. No
damage path anywhere probes `FireImmune`, `ColdImmune` or `Undead` by name.

**The aquator's corrosion is a table row**, not attack logic:
`abilities.rs:114`, fired by `fire_on_hit` (`abilities.rs:447`) from one line
of `combat::land_swing` (`combat.rs:492`). `resolve_attack` does not know what
is in the table.

So the diagnosis needs restating. The problem is not that properties are
checked by name. It is this:

> **`Grant` was built for properties — things a creature *has*, which answer a
> question someone else asks. It is being used for behaviours — things a
> creature *does*, which need a moment to fire at. There are two moments, and
> the game already needs at least ten.**

Everything below follows from that one sentence.

---

## 2. Evidence

### 2.1 Two table-backed moments, ten hand-welded ones

Table-backed (`models/src/abilities.rs`):

| table | rows | driver | fired from |
|---|---|---|---|
| `PASSIVE_ABILITIES` (`:60`) | 3 | `passive_ability_system` (`:496`) | schedule step 14 |
| `ON_HIT_ABILITIES` (`:107`) | 12 | `fire_on_hit` (`:447`) | `combat.rs:492` |

Hand-welded, one `if` each, no table:

| moment | marker(s) | where it is welded |
|---|---|---|
| AI picks its attack | `FireBreath` | `ai.rs:278-292` — probe + RNG + `match` inside the movement resolver |
| AI picks its goal | `CoinGreedy` | `ai.rs:337-349`, a fn named `orc_coin_goal` in the pathing code |
| on taking damage | `Splits` | `monsters.rs:620`, called from a 3-line hardcoded dispatch in `helpers::took_damage` (`helpers.rs:308-312`) |
| on entering a tile | `Flies` | `traps.rs:334`, a bare `continue` in `trap_system` |
| on being targeted | `Gorgon` | `medusa_gaze` (`abilities.rs:380`) hand-called from 4 sites: `combat.rs:239`, `combat.rs:594`, `wands.rs:466`, `throwing.rs:560` |
| pre-swing roll bonus | `Bided` | `combat.rs:244` (consume) + `combat.rs:380` (apply) |
| lethality override | `VorpalTarget`, `VorpalOnCondition` | `combat.rs:459-478`, four branches replacing HP arithmetic |
| attacker re-entry | `Fencer`, `Cleaves` | `combat.rs:279-283` |
| on being hit by a thrown item | `ItemUser` | `throwing.rs:507`, `throwing.rs:578` |
| resisting an effect | `SustainsStrength` | the same guard written out 3×: `abilities.rs:309`, `traps.rs:916`, `moves.rs:228` |
| **on death** | — | **no hook exists at all.** `reaper_system` (`combat.rs:84`) is fixed. The slime rides `took_damage` because there is nowhere else to stand. |

Of 13 bestiary markers audited, **6 reach no table**: `Gorgon`, `FireBreath`,
`CoinGreedy`, `Flies`, `ItemUser`, `Splits`.

### 2.2 The fourth registry, in the wrong crate, with nothing holding it true

`engine/src/update.rs:774-805`:

```rust
type DangerCheck = fn(&World, Entity) -> bool;
const MONSTER_DANGERS: &[(DangerCheck, &str)] = &[
    (|w, e| w.get::<FireBreath>(e).is_some(), "fire breath"),
    (|w, e| w.get::<Freezing>(e).is_some(),   "paralysing touch"),
    ...                                       // 15 rows
];
```

15 rows, one closure per marker, in the **engine** crate. It duplicates the
membership of `ON_HIT_ABILITIES` and `PASSIVE_ABILITIES` with no compiler link
to either. `grep` finds exactly two references to it — its own declaration and
its one loop (`update.rs:841`). Nothing fails when a 13th on-hit ability lands
and this list is not updated; `Look` just quietly stops warning about it.

This is a display list that has to agree with a behaviour list, maintained by
hand, in a different crate. It is the same "two lists and nothing checking
that they agree" that `Cargo.toml` grew `engine/tests/workspace.rs` to catch —
and here there is no such test.

### 2.3 One ability, five files, no place that names it

The dragon's fireball:

    models/src/effects.rs        the FireBreath marker + its EFFECTS slot
    models/src/monsters.rs:266   .grants(&[Grant::of::<FireBreath>()])
    models/src/constants.rs:560  DRAGON_FIREBALL_CHANCE
    models/src/ai.rs:278         the trigger
    models/src/items/wands.rs:554 dragon_breath, the mechanic

Nothing anywhere holds those five together. The docs already concede it —
`docs/how-to/add-a-move.md` says the dragon's fireball "is wired straight into
`crate::ai` and `crate::items::dragon_breath`".

### 2.4 The churn says the same thing

- `models/src/components.rs` is touched in **49 of 114 commits** (43%).
- The commit `c5d32fa` **"Enemy wiring!"** — adding monster abilities — changed
  **27 `.rs` files, +2,289 lines**, including `main.rs`, `update.rs`, `view.rs`,
  `catalog.rs`, `constants.rs`, `saveload.rs` and `map/population.rs`.

### 2.5 No safety net

315 tests across 25 files. There is **no `models/tests/abilities.rs`**, and
not one test in the suite names `Gorgon`, `FireBreath`, `Splits`, `Batty`,
`Venomous` or any other ability marker. The most tangled subsystem in the
codebase is the only one with no dedicated test file.

### 2.6 `apply_damage` is a bookkeeping funnel, not a mitigation funnel

`helpers.rs:289` — blood, broken promises, low-HP warning, splitting. It
checks **neither immunity nor `MagicWard`**. Of its 13 call sites, 2 check the
element first and 6 check the ward. The `MagicWard` guard is copy-pasted 7
times with 3 different behaviours (`wands.rs:61`, `abilities.rs:452`, and five
identical blocks in `moves.rs` at `:202`, `:253`, `:386`, `:512`, `:667`).

**This is latent, not live.** Today's six `TrapEffect` variants are Arrow,
Bear, Dart, Sleep, Teleport, Trapdoor — none elemental — so nothing currently
burns a dragon. The first elemental trap or elemental monster attack that does
not route through `elemental_blast` will.

The duplication that goes with it: `moves::sting` (`moves.rs:190-235`) is a
near-verbatim reimplementation of `traps::dart_effect` (`traps.rs:882-940`) —
same dice constants, same armour subtraction, same `SustainsStrength` bail,
same drain formula. The shape *roll → maybe-ward → maybe-immune → zero-check →
log → `apply_damage` → spark → death check* appears **11 times**, each with a
different subset of the guards.

### 2.7 `EFFECTS` is 36 of 64 and every ability burns a slot

`effects.rs:475-511`, `EffectSet = u64` (`:525`), append-only because the index
is the save-file bit. Already widened once from `u32`. Roughly half the
occupied slots are behaviour markers rather than properties. Adding abilities
at the current rate has a visible ceiling.

### 2.8 Adjacent, real, out of scope here

`engine/src/update.rs` is 2,123 lines, ~18% of it game mechanics rather than
input handling. Player melee never touches `AttackQueue` — `move_player`
calls `melee_attack` inline (`update.rs:195`) while monster attacks queue
(`ai.rs:286`). The whole drop mechanic is at `update.rs:963-979`;
`change_level` is called from the key handler at `update.rs:1462`. This is a
separate refactor with a separate rationale. Flagging, not touching.

### 2.9 `CoinGreedy`'s goal override is unreachable in a real game

Found while writing the Phase 0 net, and it is the coupling costing something
today rather than at 1.0.0.

`orc_coin_goal` (`ai.rs:337-343`) requires `hp < max_hp`:

```rust
if world.get::<CoinGreedy>(mob).is_none() { return None; }
if !world.get::<Fighter>(mob).is_some_and(|f| f.hp < f.max_hp) { return None; }
```

The only creature in the game granted `CoinGreedy` is the orc
(`monsters.rs:285-286`), and its bestiary row gives it **1 hit point**. An orc
is therefore never both alive and wounded, so the branch never runs. Verified
directly: a freshly spawned orc reports `hp/max = (1, 1)`.

Two files, each correct on its own, disagreeing about a creature neither of
them names — which is §2.3's cost, collected. Nothing in the suite noticed,
because nothing in the suite covered it.

**Fixed 2026-09-19: the orc is 3 HP** (bae's call, of the three on the table —
raise the HP, drop the `hp < max_hp` condition, or move `CoinGreedy` to
something with room to bleed). `orc_coin_goal` is now reachable, and
`being_wounded_is_what_turns_a_greedy_orc_toward_a_coin` wounds the orc to 1
off its own bestiary maximum rather than a number the test invents. The row
carries a comment saying why the number is what it is, so a future rebalance
does not quietly put it back to 1.

Balance consequence, flagged rather than decided: the orc is now the only
floor-1 creature with more than one hit point — the bat, emu, hobgoblin, ice
monster and kestral are all at 1 — and it shares power 8 / armor 6 with the
hobgoblin, so it is distinctly the toughest thing on floor 1. That also puts it
outside the "fodder hp 1-2" band in `docs/how-to/add-a-monster.md`. The band is
guidance for new rows rather than a description of existing ones, so it is left
alone; if the orc is meant to define a new band, that doc is the place to say
so.

### 2.10 Stale doc

`abilities.rs:97-99` says "The two rows are the aquator's corrosive touch and
a scroll of monster confusion's charm." There are 12.

---

## 3. Severity

| # | Finding | Severity | Why |
|---|---|---|---|
| 1 | No test covers any ability marker (§2.5) | **critical** | blocks every item below — there is nothing to refactor against |
| 2 | `MONSTER_DANGERS` duplicates the tables across a crate boundary, untested (§2.2) | **high** | silent, user-visible drift; already the shape of a known bug class |
| 3 | Only two moments exist; ten are hand-welded (§2.1) | **high** | this is the 1.0.0 spaghetti risk, stated precisely |
| 4 | ~~`apply_damage` has no mitigation hook~~ (§2.6) | **fixed** | `apply_hit`, 2026-09-19 |
| 5 | ~~An ability spans 5 files with nothing binding them~~ (§2.3) | **fixed** | `Moment::InsteadOfAttacking`, 2026-09-19 — see §6.9 |
| 5b | ~~`CoinGreedy`'s goal override cannot fire~~ (§2.9) | **fixed** | orc buffed to 3 HP, 2026-09-19 |
| 6 | ~~`SustainsStrength` guard triplicated~~ (§2.1) | **fixed** | `conditions::drain_power`, 2026-09-19 |
| 7 | ~~`EFFECTS` at 36/64~~ (§2.7) | **fixed** | the bitset is gone; no ceiling |
| 8 | ~~Stale doc comment~~ (§2.10) | **fixed** | rewritten with the merged table |

---

## 4. Plan

Ordering principle: build the net, then remove the duplicate that can silently
lie, then generalise the moments, then fix the damage funnel. Each phase is
independently shippable and leaves the suite green.

### Phase 0 — the net  **[DONE, 2026-09-19]**

`models/tests/abilities.rs`, 31 tests, workspace green (`cargo +stable test
--workspace`), clippy and `fmt` clean. Coverage: all 12 `ON_HIT_ABILITIES`
rows, all 3 `PASSIVE_ABILITIES` rows, the two row gates, `fire_on_hit`'s ward
guard, the player-only gate, and the six markers that reach no table
(`Gorgon`, `FireBreath`, `CoinGreedy`, `Flies`, `Splits`, and `ItemUser`
indirectly through the theft rows), plus three tests over the tables as data.

Acceptance criterion held: every test goes red when its wiring is removed.
Checked by mutation, each reverted after:

| mutation | test that caught it |
|---|---|
| `fire_on_hit` call deleted (`combat.rs:491`) | `a_landed_blow_fires_the_on_hit_table` |
| `medusa_gaze` call deleted (`combat.rs:239`) | `looking_upon_a_medusa_turns_the_player_to_stone` |
| `maybe_split` call deleted (`helpers.rs:311`) | `a_wounded_slime_becomes_two` |
| `Flies` guard forced false (`traps.rs:334`) | `a_flier_crosses_a_dart_trap_untouched_…` |
| `orc_coin_goal` removed from the chain (`ai.rs:261`) | `being_wounded_is_what_turns_a_greedy_orc_toward_a_coin` |
| `FireBreath` forced off (`ai.rs:278`) | `a_dragon_sometimes_answers_with_fire_…` |

Two things the writing turned up, both recorded above: §2.9 (`CoinGreedy` is
unreachable) and the `ConfusingTouch` exemption from `EFFECTS`, which is
deliberate and now pinned at exactly one by
`only_one_ability_marker_sits_outside_the_save_format`.

One test had to be restated rather than fixed: a ring of regeneration mends
*conditions and power*, never HP (`rings.rs:151-156`). The first draft asserted
an HP drip, which is a mechanic the game does not have.

### Phase 0 as originally specified

New `models/tests/abilities.rs`. One behavioural test per marker, asserting the
*relation*, never a tuning constant (`docs/explanation/code-calisthenics.md`,
"a test never asserts a constant"):

- an aquator's blow leaves the target's armour worth less than before
- a rattlesnake's blow leaves the target carrying the venom condition
- a slime wounded and left alive is two slimes; a slime killed is none
- a dragon adjacent to the player, over enough turns, deals damage at a tile
  it is not standing on (fireball fired) — relation, not the 1/6
- a player who attacks a medusa is snared; a monster that does is not
- an orc below max HP walks toward a red coin rather than the player
- a flier crossing a dart trap does not trigger it; a walker does
- **and one structural test**: every `ON_HIT_ABILITIES` / `PASSIVE_ABILITIES`
  row has a `Look` description, and vice versa (this test is what Phase 1
  makes possible, and it is the test that would have caught the drift)

Criteria: suite green, and each test demonstrably fails when its ability's
wiring is commented out. Nothing after this phase starts until this is in.

### Phase 1 — kill the cross-crate duplicate

Move the danger prose onto the ability row as a field, and give `models` a
function the engine asks:

```rust
// models/src/abilities.rs
pub fn dangers_of(world: &World, entity: Entity) -> Vec<&'static str>
```

`engine/src/update.rs:774-805` deletes, `:841` becomes a call. The structural
test from Phase 0 now has teeth. Markers that carry no row yet (`Gorgon`,
`FireBreath`, `Splits`, and `wielded_launcher`, which is a gear query not a
marker) need a home — either a small third list in `models` with the same
test, or they wait for Phase 2 and the list is knowingly incomplete for one
phase. Small, self-contained, high value.

### Phase 2 — generalise the moments

Mechanism: **option B**, decided 2026-09-19 (§5). One `ABILITIES` table, each
row naming its moment, `danger` on the row so the `Look` list is derived.
Phase 1 lands `dangers_of` against the two existing tables; Phase 2 folds
those two into `ABILITIES` and adds the new moments, and `dangers_of` keeps
its signature across the change.

Migrate in this order, lowest risk first:

1. **on-damaged** — `took_damage`'s hardcoded trio (`helpers.rs:308-312`)
   becomes a table. Moves `Splits` and `break_promises` out of hand-dispatch.
2. **on-resist** — the three `SustainsStrength` copies become one guard.
3. **on-targeted** — `medusa_gaze`'s four call sites become one table plus one
   call in a shared "before a hostile act" funnel. (Check first: the four
   sites are currently correct, and a fifth attack path would silently omit
   it — that is the bug this prevents, not one it fixes.)
4. **on-death** — a moment that does not exist today. Adding it unblocks
   content rather than fixing anything, so it goes last in this phase and can
   be dropped if the budget runs out.

Deliberately **not** in this phase: `Bided`, the vorpal/garrote lethality
overrides, `Fencer`, `Cleaves`. Those modify the swing arithmetic itself
rather than riding on its outcome, and a hook table is the wrong shape for
them. They want a `Matchup`-folding seam, which is a different design
question. Leave them welded and say so.

### Phase 3 — one mitigation funnel

Give `apply_damage` the source of the damage so it can own the two checks
everyone currently copies:

```rust
pub struct Hit { pub amount: i32, pub element: Option<Element>, pub warded: bool }
pub fn apply_hit(world: &mut World, entity: Entity, hit: Hit) -> i32  // damage actually dealt
```

Keep `apply_damage` as a thin wrapper through the migration so the 13 call
sites move one at a time, each with its own test. Outcomes: the 7 `MagicWard`
copies collapse to one; `moves::frost_nova`'s hand-inlined immunity check
(`moves.rs:671`) deletes; `moves::sting` and `traps::dart_effect` become one
function with two callers. This is the phase that fixes the latent bug in
§2.6 and removes the most code.

### Phase 4 — the AI intent seam

Keep for now and document.

### Not in this plan

§2.8 — `update.rs`'s engine/models boundary and the player-melee/`AttackQueue`
asymmetry. Real, separate rationale, separate risk. Should not ride along.

---

## 5. The decision — settled

**Chosen: B, one table with a `when` field.** Decided by bae, 2026-09-19.

```rust
pub struct Ability {
    pub effect: Grant,
    pub when:   Moment,   // OnHit { glancing, lethal } | EachTurn(f64)
                          // | OnDamaged | OnDeath | OnTargeted | OnResist
    pub action: fn(&mut World, AbilityCtx) -> bool,
    pub danger: Option<&'static str>,   // Look reads this — no second list
}

pub const ABILITIES: &[Ability] = &[ .. ];

pub fn dangers_of(world: &World, e: Entity) -> Vec<&'static str>;
```

Open sub-questions for implementation time, none blocking:

  * what `AbilityCtx` carries (`actor`, `target: Option<Entity>`, and whether
    the `hp_before` the on-damaged moment needs rides in it);
  * whether `-> bool` stays meaningful for every moment or only for the ones
    that log flavour (today only passives use the return);
  * where the four gear/attacker markers that re-check `is_player` inside
    their bodies (`abilities.rs:206-208`) put that predicate — a row field
    would remove four copies.

The three shapes as weighed:

**A. One `const` table per moment.** `ON_DAMAGED_ABILITIES`,
`ON_DEATH_ABILITIES`, `ON_RESIST_ABILITIES`, each beside the two that exist.
Each keeps a precise, typed action signature (`fn(&mut World, Entity)` vs
`fn(&mut World, Entity, Entity)`), matching the current idiom exactly. Cost:
N tables to read, and a new moment is still a new table plus a new driver.

**B. One `ABILITIES` table with a `when: Moment` field.** One list to read,
one place to add a row, and `dangers_of` falls out of it for free. Cost: the
actions need a uniform signature, so a context struct — `AbilityCtx { actor,
target: Option<Entity>, .. }` — and each action unpacks what it needs. Trades
some compile-time precision for one source of truth.

**C. bevy events / observers.** Rejected, and worth writing down as rejected:
`docs/explanation/ecs-in-nihilurk.md` states there are "no events in the bevy
sense" on purpose, and the exclusive-`&mut World` idiom is what makes these
mechanics readable as procedures. Raising it only so the answer is on record.

Why B won: §2.2's failure mode is specifically *two lists that must agree and
nothing checking that they do*, and B is the only one of the three that makes
the display list derived rather than parallel. A was the safer, smaller diff
and kept the typed signatures; it loses because `dangers_of` would still have
to walk five tables by hand, which is the same bug one level up.

---

## 6. Work taken on after the assessment

### 6.1 `ConfusingTouch` and `Bided` are effects  **[DONE, 2026-09-19]**

Both were unit markers declared in `components.rs` and saved as bespoke bools.
They are buffs the bearer holds — something you have, that something else asks
about — so they are effects. Moved to `effects.rs`, appended to `EFFECTS`
(now 38 of 64), every qualified path repointed. Suite green.

Appending alone is save-compatible: `EntitySave.effects` is a fixed-width
`u64`, so an older save simply has the new bits clear. The bespoke
`confusing_touch` and `bided` fields are therefore **written twice** as of this
change — dupe state, deliberately left until §6.3 removes them in one break.

`every_ability_marker_is_in_the_save_format` caught the change and now asserts
zero exemptions rather than one. The invariant got stronger for free.

### 6.2 Tests stop reaching for other files' names  **[DONE, 2026-09-19]**

All 31 tests rebuilt on fixtures. Not one content name survives: every `Name`
in the file is the test's own (`"you"`, `"creature"`, `"dummy"`, `"gear"`,
`"trap"`, `"coin"`). `initialize_world` is gone from the suite, replaced by an
`arena()` of open tiles — which also removed the seed-hunting the old
`CoinGreedy` test needed to find workable map geometry, and the stray coins
that were quietly competing with its planted one.

All seven mutations still kill their test (re-verified after the rewrite),
including a new one: dropping `ConfusingTouch` from `EFFECTS` fails
`every_ability_marker_is_in_the_save_format`.

**One pre-existing bug found and fixed on the way.**
`docs/explanation/ecs-in-nihilurk.md` states that `FxRng` is "reached through
`get_resource_mut`, never `resource_mut`, because every test builds a bare
world without them. Gameplay code arms a flourish and forgets; it must never
*require* one." `rings.rs` and `score.rs` honour that; `helpers.rs` broke it
in five places, so `spill_blood` and `death_burst` panicked in any world
without the cosmetic stream — which is every bare test that kills something.
Fixed at the two roots: the droplet roll yields no splats without the stream
(the tile is still stained, since that part was never a roll), and
`death_burst` marks the corpse and returns, exactly as it already does with
blood switched off.

No test may depend on what another file calls a thing. A test that says
`spawn_named(w, "aquator", ..)` is coupled to the bestiary for no reason: what
it is testing is *a creature carrying `RustsArmor`*, which the test can build
itself.

  * **Behaviour tests build fixtures.** An entity assembled in the test out of
    the components the behaviour actually needs. `Map` is a plain public
    struct, so an empty all-floor arena is a few lines and removes the stray
    monsters, stray coins and map-geometry roulette that `initialize_world`
    brings with it.
  * **Table coverage stays where it belongs.** `content_names()` already
    enumerates every row, and `tests/content.rs` already spawns each one by
    the name it *found*. That is the test for "the tables can build their own
    contents", and it needs no help from the ability suite.

One test cannot be fully freed and the reason is a finding: `maybe_split`
(`monsters.rs:639`) calls `MonsterDef::named(&name)` on the victim's own
`Name`, and panics on a miss. The mechanic looks its subject up in the
bestiary by string, so a fixture with a name the test invented cannot split.
The test takes a name off `BESTIARY` at runtime rather than hardcoding one,
and the coupling is logged here as Phase 2 work.

### 6.3 Effects v2: lifetimes replace the bitset  **[BAE, decided 2026-09-19]**

`Snare` is the only turn-counted condition in the game, and `snare_system`
(schedule step 2) is a tick system built for that one component. Generalising
it is what makes the next twenty buffs and debuffs cost one row each.

```rust
pub enum Lifetime {
    Permanent,     // fire immunity
    Turns(u32),    // snare, and every future debuff
    Floor,         // magic ward, detected   (was GrantedForFloor)
    NextAction,    // bide
}

#[derive(Component, Default)]
pub struct Effects(Vec<(EffectId, Lifetime)>);
```

A bare `u32` with `0` meaning "permanent" was considered and rejected: the
game already has **three** non-turn cleanup rules (permanent, until the next
staircase, until the bearer's next action), and one sentinel cannot tell them
apart — it would still need `GrantedForFloor` carried alongside, which is the
second structure this is meant to remove.

What it absorbs:

  * `GrantedForFloor` — becomes `Lifetime::Floor`.
  * `Snare { turns, kind }` — becomes three ordinary effects (asleep, pinned,
    held) with `Lifetime::Turns`. Deletes the `match snare.kind` at
    `ai.rs:228`; the garrote check at `combat.rs:519` becomes "any of the
    three".
  * The five unit-marker conditions — `Confused`, `Blind`, `Paralyzed`,
    `Detected`, `MagicWard`.
  * `snare_system` — becomes one tick system over every `Lifetime::Turns`.
  * The 64-effect ceiling, and `EffectSet` with it.
  * Three hand-written condition lists (§2.1) — one mask each.

**The save break.** Authorised 2026-09-19. `EntitySave` loses seven of its
eight bespoke condition fields (`snare` goes too, absorbed). The format is
unversioned and postcard is not self-describing, so every run in progress
stops loading — `saveload.rs:39-44` states this is the known price of any
field change. (`rogue.sav` in the repo root needs nothing: it is gitignored
and untracked, a leftover local save rather than a fixture.)

**`EffectId` on disk: a stable string per row.** Decided 2026-09-19.

Measured, not guessed: the save is 1,963 bytes, a floor carries roughly one
monster per room, and `BESTIARY` holds 22 grants across 26 rows — so a save
holds 15-25 effect entries. At ~13 bytes each that is **+250 bytes, about
12%**, against ~20 bytes for an index.

An index was rejected because its drift failure is silent and unrecoverable:
delete a row and every later effect in every existing save shifts by one, a
saved `Stealthy` loads as `Regenerates`, and every index is still a valid
index so nothing can detect it. That is precisely the failure this whole
refactor exists to remove, and here it would land on the player's save file.
A string id makes an unknown effect detectable and local, lets rows be
reordered and retired, and matches how items (`restore_from_catalog`) and
monsters (bestiary lookup) already round-trip — effects are currently the only
content identified positionally.

It also retires one of ADR-0001's four objections to external raw files: that
`Grant` "cannot survive the trip" because a raw port "would need a
name-to-`Grant` lookup table maintained by hand". That lookup now exists and
is derived from the table rather than hand-maintained. This does not make raws
a good idea; it removes one reason they were ruled out, and
`adr-0001-tables-not-raws.md` should say so when this lands.

**What does *not* change, and must not.** The marker components stay
components. `FireImmune` is still a component the fire code asks about with
`world.get::<FireImmune>(e)`, and `SeesInvisible` is still a query filter —
that is the part of the design that works (§1), and `Effects` is a *ledger*
beside it, not a replacement for it. The ledger owns what is attached, from
where, and for how long; it does not own whether the entity has the property.

**One thing the decision forces, flagged rather than assumed:**
`GrantedByGear` is an `EffectSet`, so it cannot survive `EffectSet` retiring.
Gear-lent becomes a fifth lifetime, `WhileEquipped(Entity)`, naming the item
that lends it — which also turns `sync_equipment_effects` from a bitset diff
into "drop the entries whose item came off". Gear-lent entries are not
serialised, exactly as today (`saveload.rs` masks them out and re-lends on
load), so the `Entity` never reaches the disk.

**Staging.** Four compile-green steps, each a checkpoint:

    [done] 1. add `id` to every EFFECTS row     mechanical, no behaviour change
    [done] 2. Lifetime + Effects ledger; retire EffectSet,
              GrantedByGear, GrantedForFloor    <- the save break
    [done] 3. tick system; Snare -> three Turns effects; retire snare_system
    [done] 4. the five unit-marker conditions; collapse the three condition lists

**Step 4, as built.** `Confused`, `Blind`, `Paralyzed`, `MagicWard` and
`Detected` moved from `components.rs` to `effects.rs` and became `EFFECTS`
rows, applied with `Lifetime::Floor`. `EFFECTS` is 46 rows; `EntitySave` has
**no bespoke condition fields left** — `snare`, `confused`, `blind`,
`paralyzed`, `detected`, `confusing_touch`, `magic_ward`, `bided` and
`floor_grants` are all gone, carried by name in the ledger instead.

The three lists became one `AFFLICTIONS` table in `conditions.rs`, holding the
`Grant`, the cure line, the monster noun, the staircase adjective and the
optional side effect. `afflicted`, `cure_one_condition` and
`clear_player_conditions` all walk it. Its order is load-bearing — worst
first, because a cure takes the first row it finds — and that is now stated in
one place instead of being implied by three.

`Speed` haste/slow is **not** a row and cannot be: a tempo is a value rather
than a marker something either has or has not. Handled on its own, and said so
in the table's doc.

Two hazards this surfaced, both fixed and tested:

  * **A condition attached with a bare `insert` has no ledger entry**, so a
    staircase that trusted the ledger alone would make it permanent. The
    afflictions are floor-scoped by definition, so `clear_player_conditions`
    lifts them whether or not anything recorded that it had — but never one a
    worn item is still lending, which is what the ledger is for.
  * **Cancellation silenced its own report.** `revoke_all` strips the
    afflictions now that they are effects, so it ran before
    `clear_player_conditions` could say the confusion had lifted. The order is
    swapped, with the reason written at the call site.

Eight new tests. Seven killed their mutation first time; the eighth did not —
`a_staircase_leaves_what_gear_is_still_lending` passed with its guard removed,
because `SeesInvisible` is protected by the ledger path rather than by that
guard. Replaced with
`a_staircase_leaves_an_affliction_a_worn_item_is_lending`, which exercises it
and fails without it.

**Step 3, as built.** `Snare { turns, kind }` and `SnareKind` are gone.
The three holds are ordinary effects — `Asleep`, `Pinned`, `Rooted` — held for
`Lifetime::Turns` and aged by `effects::tick_effects`, which replaced
`snare_system` in the same schedule slot. One system now ages every timed
effect there will ever be; the old one knew about exactly one component and
could not have handled a second without being copied.

An `EFFECTS` row gained an optional expiry line, in brackets after the type:

    "asleep" => Asleep ["You shake off the drowsiness and come to."],

which is where the three per-kind messages `snare_system` matched on now live
— on the row, as data, rather than in a `match` in the tick.

**Behaviour change, deliberate and tested.** One `Snare` component could hold
one kind at a time, so a bear trap closing on a sleeping creature *replaced*
the sleep. Three effects mean both are true at once and each ends on its own
clock. `two_holds_at_once_run_on_their_own_clocks` pins it. The old behaviour
was an artefact of the storage, not a rule anybody wrote down.

A second dose of the same hold still lengthens rather than stacks, and still
refuses to shorten one already running — `hold` replaces that id's entry
rather than pushing a second.

Blast radius: ~35 production sites across 12 files, plus ~30 assertions in
five test files. Nine new tests, all mutation-checked. Docs updated in six
files, plus `add-an-effect.md`, which described the bitset.

**Step 2, as built.** `Effects(Vec<Held>)` is the ledger; `Held { id,
lifetime }`. Three overlapping bitsets and their intersection arithmetic are
gone, replaced by one list where **an id may appear more than once on
purpose** — a creature born fire-immune and also wearing a ring of fire
resistance holds two entries, and taking the ring off removes one while the
component stays. `revoke_matching` is the single place an effect is taken
away, so "is anything else still lending this?" is asked once there instead of
at every call site.

`sync_equipment_effects` stopped being a bitset diff and became "drop the
entries whose item came off, lend what is newly on". It is idempotent, which
matters because it runs every turn for every pack-carrying creature.

On disk, `SavedLifetime` deliberately has **no** `WhileEquipped` variant. A
gear-lent effect being excluded from the save used to be a mask that had to be
remembered (`effects_of(..) & !gear_granted`); now the type cannot express one,
so the rule is enforced rather than observed.

An id the save names and this build has no row for is dropped, counted, and
reported to the player in one line at the end of the load. That is the payoff
for names over indices, and it is a thing an index format could not have done:
every index would still have been a valid index.

Six new tests cover the ledger, all mutation-checked: two sources lending one
effect, the last source going, gear-lent staying out of the save, an unknown
id costing only itself, unequipping lifting only its own loan, and syncing
twice lending once.

**Sequencing.** §6.2 lands first. Effects v2 rewrites every `EFFECTS`
consumer — `effects_of`, `attach_effects`, `sync_equipment_effects`,
cancellation, `conditions.rs`, `ai`, `combat`, `traps`, `saveload` — and the
ability net is what holds that still. The net must not be coupled to the
content tables while it does.

### 6.4 Phase 1, and every creature is a they  **[DONE, 2026-09-19]**

**`MONSTER_DANGERS` is gone.** The phrase a marker is warned about lives on
its `EFFECTS` row now, as `beware`, beside the `ends` line step 3 added. The
engine calls `models::dangers_of` and **names zero ability markers** — it was
15 closures in the wrong crate with nothing holding them true.

The phrase went on the *effect* row rather than on an ability row, which is
what lets `Look` warn about `Gorgon`, `FireBreath` and `Splits` — three of the
six markers that reach no ability table at all. It also means nothing moves
when Phase 2 merges the two ability tables into one.

The macro now reads `"id" => Type, ends "..."`, `beware "...";`. Four tests,
all mutation-checked, including the exact historical drift: deleting a monster
ability's phrase fails `every_monster_ability_has_words_for_look`, which walks
`BESTIARY` and checks that anything a creature is born with, and that arms an
ability, has words. That test could not have existed before — the list it
would have checked was in another crate.

**Every creature is a they.** `Look` used to pick "their" for anything with
`ItemUser` and "its" for everything else, which made the pronoun a function of
whether a creature could work a doorknob. Thirteen strings changed: the `Look`
line, the flytrap's jaws, three potion reactions, two scroll ones, the
already-at-tempo refusal, the xeroc's reveal and the gear announcement ("They
are wielding a long sword."). `its` was left alone where it refers to an item,
a suit of armour, the floor or the world — those are not creatures.

### 6.5 Phase 2: the moment is a field  **[DONE, 2026-09-19]**

`ON_HIT_ABILITIES` and `PASSIVE_ABILITIES` are one `ABILITIES` table. Each row
carries `when: Moment`, and there are four moments where there were two:

    OnHit { glancing, lethal }   12 rows, as before
    EachTurn(f64)                 3 rows, as before
    OnDamaged                     the slime's split
    OnTargeted                    the medusa's gaze

One driver, `fire`, does the matching, the player-only gate and the flavour
line. `EachTurn` keeps an entry point of its own because the odds live on the
row and the roll belongs outside the mechanic.

**What the two new moments retired.** `medusa_gaze` was hand-called from four
sites (`combat` twice, `wands`, `throwing`) and a fifth path would silently
have missed it. `maybe_split` was a hardcoded line in `helpers::took_damage`,
beside two things that are not abilities at all. Both are rows now.

**`player_only` is a field.** It was the same `is_player` guard written into
four mechanic bodies — `heavy_stagger`, `chaos_recoil`, `build_momentum`,
`cleave_attack`. A gate written four times is a gate that can be forgotten the
fifth.

The sub-questions §5 left open, settled:

  * **`AbilityCtx` is not a struct.** `fn(&mut World, Entity, Option<Entity>)`
    carries the bearer and, for the moments that have one, the other end. A
    context struct would have been a struct with two fields and a name.
  * **Every row returns `bool`.** Only `EachTurn` reads it today, to gate the
    flavour line — but `fire` now folds it too, and the look reticle needed
    exactly that (below). A moment growing a use for it needs no new signature.
  * **The `is_player` predicate is a row field**, as above.

`Moment::OnDeath` was **dropped**. No row wanted it, and a variant with no rows
is dead code wearing a plan. It is a variant and one arm away whenever a
mechanic needs it.

### 6.6 The medusa gotcha  **[BAE, 2026-09-19]**

Looking at a medusa with `L` now petrifies you, spends the turn, closes the
reticle and says **"Well played."**

This is the seam paying for itself: look mode is the *fifth* path through
`Moment::OnTargeted`, and it cost one call plus the prose. Under the old
arrangement it would have been a fifth hand-written `medusa_gaze` call, which
is precisely the kind nobody remembers to add.

The reticle is the player's eyes, so a reticle that could rest on a gorgon in
perfect safety would be a way to scout a floor that nothing else in the game
offers. `answers_being_looked_at` lets the look path ask *before* it fires,
because for a look the answer is the whole event rather than a rider on a blow.

Eight tests for Phase 2, all mutation-checked. Two needed rewriting after a
mutation survived:

  * `one_moment_never_fires_another_moments_rows` had to be added at all —
    every other test asked whether the *right* thing happened, so breaking the
    moment match (firing everything, always) passed the lot.
  * `turns_passing_never_fire_an_on_hit_row` was vacuous in its first form: it
    used `ConfusingTouch`, whose row cannot act without a target, so the
    `Option` shape was protecting it rather than the moment filter. Rewritten
    around the chaos blade's recoil, which ignores its target and bites its own
    wielder — nothing but the filter stands between a turn passing and the
    player bleeding.

### 6.7 Phase 3: one funnel decides mitigation  **[DONE, 2026-09-19]**

    pub struct Hit { amount: i32, element: Option<Element>, magical: bool }
    pub fn apply_hit(world, entity, hit, announce: Option<&str>) -> i32

Every source of harm goes through it. It decides the two questions that used
to be answered at the call sites by whichever ones remembered — *does a ward
turn this aside* and *is this creature immune* — and returns the HP that
actually came off.

    MagicWard guards      7  ->  1
    SustainsStrength      3  ->  1
    element checks        2 call sites of 13  ->  the funnel

The one ward check left is in `fire_on_hit`, and it stays on purpose: that
decides whether a *rider* lands, not whether damage does. Same word, two
questions — a blow that hurt a warded creature can still be forbidden from
poisoning them. Said so at the call site.

**`Hit::magical` rather than "everything".** A ward stops magic; it does not
stop a thrown dagger. That distinction was implicit in *which* call sites
happened to check, and is now a field.

**The flavour line moved into the funnel**, which was not in the plan and is
the one thing here I would call a design decision rather than a move. The
ordering demands it: a warded hit must not announce damage it never did, and a
landed one must announce itself *before* "You are badly wounded!" answers it.
Leaving the line at the call site meant choosing which of those two to get
wrong. Two tests pin both directions.

**`drain_power` in `conditions`** holds the power-drain rule — the sustain
check, the clamp, and whether anything changed — with `floor: Option<i32>`
because the rattlesnake has none and the dart trap floors at 1. The prose
stays with whoever is inflicting it: a trap and a snake do not sound alike,
and flattening that to save three lines would have cost the voice.

Eight tests, all mutation-checked. The first is the latent bug from §2.6,
stated directly: elemental damage from a source that is not a wand, which the
game does not have yet and which would have burned a dragon the day somebody
added it.

### 6.8 Phase 4: the context struct  **[BAE, decided 2026-09-19]**

Three shapes were put up: two tables (one per decision point), a
decide-then-carry-out split with a `Plan`, or the context struct alone. **C
was chosen**, and the reasoning holds: the AI has exactly two hand-wired
abilities plus the launcher, and a table with one row is a table pretending.

What landed: `AiCtx` carries what every mob's turn is decided against — the
player, their position and faction, the viewshed, stealth and the map — and
`step_one_mob` went from **nine parameters to four**. The `player_stealthy`
bool that was threaded through `ai` → `monster_round` → `step_one_mob` so that
`notices` could ask one question is now read once into the context, and
`AiCtx::noticed_by` is the only caller of `notices`.

No behaviour change, and none expected: the suite was green before and after
without a single assertion moving. The one thing worth checking was whether
the tests still reach the new path, and they do —
`a_breather_sometimes_answers_with_fire` dies when `noticed_by` is made to
always return false.

**What stays welded, and it is a decision rather than an oversight.**
`FireBreath` is still an `if` at the attack branch and `CoinGreedy` still a
`.or_else` in the goal chain, so §2.1's two AI rows and §2.3's five-file
dragon remain open. The argument list no longer grows when a third arrives,
which was the part that would have made it worse.

Revisit when a third AI-moment ability appears. Two is a pair; three is a
pattern, and at three the table that was declined here starts paying for
itself.

---

## 7. Where this ended

    severity  1  no test covers any ability marker        fixed  Phase 0
              2  MONSTER_DANGERS, wrong crate, untested   fixed  Phase 1
              3  two moments, ten hand-welded             fixed  Phase 2
              4  apply_damage has no mitigation hook      fixed  Phase 3
              5  an ability spans five files              fixed  §6.9
              5b CoinGreedy could never fire              fixed  orc at 3 HP
              6  SustainsStrength guard triplicated       fixed  Phase 3
              7  EFFECTS at 36 of 64                      fixed  no ceiling
              8  stale doc comment                        fixed  Phase 2

    tests     315  ->  431
    saves     broken once, deliberately, in Effects v2

Still out of scope and still true: §2.8, `engine/src/update.rs` at 2,123 lines
with roughly a fifth of it game mechanics, and player melee resolved inline
while monster melee queues. Separate rationale, separate risk; it should not
ride along with this.

### 6.9 §2.3 solved: the dragon fits the one table  **[DONE, 2026-09-19]**

The fireball needed a moment that did not exist. All four moments Phase 2
added are *reactions* — something happened, now answer it — and the dragon's
breath is a **decision**, made before anything happens, at the point where a
mob is choosing what to do with its turn.

    /// The bearer is about to swing at something and would rather not, at
    /// these odds.
    InsteadOfAttacking(f64)

It fits the existing row signature exactly: the action does the thing and
reports whether it spent the turn. So no fifth registry, no second AI table —
one more variant on the moment enum the whole game already reads.

The dragon is one row now, naming its marker, its odds and its mechanic
together:

    Ability {
        effect: Grant::of::<FireBreath>(),
        when: Moment::InsteadOfAttacking(DRAGON_FIREBALL_CHANCE),
        action: |w, mob, target| { .. dragon_breath .. },
        ..
    }

and `ai::step_one_mob` reads:

    if !fire_instead_of_attacking(world, mob, target_entity) {
        // queue the ordinary blow
    }

**`ai.rs` no longer names a species at all.** It mentioned `FireBreath`,
`DRAGON_FIREBALL_CHANCE` and `dragon_breath`; now it mentions none of them,
and `orc_coin_goal` is `coin_goal` (the creature that wants the coin is
whatever carries the marker, which was always the point).

The fireball's five files are the four every ability has — the marker and its
registry row, the bestiary grant, the ability row, the mechanic in the module
that owns its domain — plus one tuning constant. Exactly the aquator's shape.

**What is still hand-written, and why it is not the same problem.**
`coin_goal` overrides a *goal* rather than taking the turn, which no moment in
the table can express: a moment's action returns "did I spend the turn", and a
goal override returns a tile and then lets the ordinary movement happen. It is
one function, named for what it does rather than for the orc, called from one
place. Revisit if a second goal-overriding ability appears — that is the point
at which the shape needs a name.

Three tests, all mutation-checked, including one that kills `ai`'s call.