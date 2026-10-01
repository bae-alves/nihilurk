# Journal

## Contents

- [The EFFECTS table in content-tables.md lists 16 of ~45 rows (2026-09-25)](#the-effects-table-in-content-tablesmd-lists-16-of-45-rows-2026-09-25)
- [Petrification split out of sleep (2026-09-19)](#petrification-split-out-of-sleep-2026-09-19)
- [The spell docs after the move→spell rename (2026-09-20)](#the-spell-docs-after-the-movespell-rename-2026-09-20)
- [Second sight stopped being omniscience (2026-09-20)](#second-sight-stopped-being-omniscience-2026-09-20)
- [Worn gear was still lying on the floor (2026-09-20)](#worn-gear-was-still-lying-on-the-floor-2026-09-20)
- [The drop-table test was reading the floor, not the roller (2026-09-20)](#the-drop-table-test-was-reading-the-floor-not-the-roller-2026-09-20)
- [Trick shots became a chain (2026-09-20)](#trick-shots-became-a-chain-2026-09-20)
- [Special levels, and what Rogue's layout had been hiding (2026-09-26)](#special-levels-and-what-rogues-layout-had-been-hiding-2026-09-26)
- [Helpers, and the tests that were one floor from failing (2026-09-26)](#helpers-and-the-tests-that-were-one-floor-from-failing-2026-09-26)
- [Every mob became an agent (2026-09-26)](#every-mob-became-an-agent-2026-09-26)


## The EFFECTS table in content-tables.md lists 16 of ~45 rows (2026-09-25)

Found while documenting the playable-body feature: `docs/reference/content-tables.md`'s
"EFFECTS — the marker registry" table stops at id 15 (`Teleportitis`).
`models/src/effects.rs`'s actual `effects!` macro call runs to id ~44
(`Flies`, `Batty`, `Binds`, `Gorgon`, `Vampiric`, `Venomous`, `CoinGreedy`,
`Splits`, `Freezing`, both steal variants, `FireBreath`, `Cleaves`,
`HeavySwing`, `Fencer`, `Lunges`, `Lurk`, `WhirlOnMove`,
`VorpalOnCondition`, `TurboMagic`, `SelfDamageOnHit`, `BuildsMomentum`,
`ShattersStone`, `ConfusingTouch`, `Bided`, and the four holds/afflictions
that got their own prose sections lower in the same file). Not something
from the last dozen commits — this predates them by a long way — so it's
out of scope for the current pass and left here rather than rewritten
inline. Whoever regenerates that table should check it against the macro
call directly rather than trusting the row count.


## Petrification split out of sleep (2026-09-19)

A medusa's gaze used to *be* `Asleep`, so coming out of stone was reported with
the sleeper's line ("You shake off the drowsiness"). One effect serving two
flavours has exactly one place to put its sentence, so the flavour had to
become its own row: `Petrified`.

What it taught:

* An effect's `ends` line is the tell for a shared mechanic. Two things that
  end differently are two effects, however identical the mechanic looks the
  day it is written.
* `Petrified` is the first hold that is *not* in `HOLDS`. The array is "things
  holding a creature in a place", which is why both teleports let go of all of
  it; stone is the victim's own body and travels with them. Worth keeping that
  distinction explicit, because "add it to HOLDS" is the obvious wrong move.
* The damage cap had to go in two places — `combat::clamp_swing` (melee applies
  its own HP) and `helpers::apply_hit` (everything else) — so the rule and its
  sentence live together in `effects::stone_chip` and both paths ask it. Two
  copies would have been two answers to "can a petrified player die".
* `apply_hit`'s `announce` is the caller's sentence about a hit that, against
  stone, did not happen. The chip line replaces it. Callers that log their own
  damage number *before* calling (the arrow and dart traps) would still
  overstate — unreachable today, since a petrified player cannot step on a
  trap, but it is the shape of a future bug.


## The spell docs after the move→spell rename (2026-09-20)

The rename was a clean sed, which is exactly why the spell pages went stale
without breaking anything: every word was right and several facts were not.

What the audit turned up, and what it teaches:

* **A centralisation the docs never followed.** `MagicWard` is now checked once,
  in `helpers::apply_hit` (on `Hit::magical`), plus `abilities::fire_on_hit` for
  the rider on a blow. The how-to still told authors to check it per-victim and
  call `ward_block` — a function that does not exist. When a check moves into a
  shared function, the page that told people to write it by hand is the one
  thing that will keep them writing it by hand.
* **Markers moved into the `EFFECTS` ledger; the save advice didn't.** The docs
  said `MagicWard`/`Bided` are `bool` fields in `EntitySave`. They are rows in
  the registry, lent with a `Lifetime`, saved by id. Advice about persistence
  ages fastest, because it is the part nobody re-reads until a reload loses
  something.
* **A sed carried a balance change with it.** `power_mult` went 2→3 in the same
  uncommitted batch, silently making a staff triple damage while every comment,
  page and the `equip` test still said "doubles". A rename diff is where a
  number change hides best — read it for the lines that are *not* the rename.
* **`crate::ai` vs `crate::abilities`.** The dragon's fireball moved into an
  `ABILITIES` row at `Moment::InsteadOfAttacking` (its own commit said so); four
  doc sites still pointed at `ai`. `docs_style.sh` checks that source *paths*
  exist — it cannot check that a named *module* is still the right one.

Fixed at the root afterwards: the staff's two multipliers were bare literals in
two functions (`spell_cost`, `spell_system`) with their *meaning* written out in
six prose sites and a third literal asserted in `equip.rs`. They are now
`constants::spells::TURBO_MAGIC_COST_MULT` / `TURBO_MAGIC_POWER_MULT`, which is
what `constants.rs` says it is for — "every tuning knob in the model, in one
place". `docs/reference/constants.md` had no `spells` row at all, so the whole
module (thirteen knobs, since before the rename) was invisible to anyone
rebalancing from the docs.


## Second sight stopped being omniscience (2026-09-20)

`reveal_traps` treated `SeesInvisible` as a short-circuit *before* the viewshed
check, so a ring of perception (or a potion of see invisible) revealed and
logged every hidden trap on the floor, including ones in rooms the player had
never entered. Every other perception path in `visibility.rs` was already
gated: `hide_and_announce` computes `perceptible = in_view && (…)`. Traps were
the one place the `&& in_view` was missing, and the doc comment had
rationalised it ("reveals every trap on the floor at once") — which is how it
survived a test that asserted the bug by name.

The tell: a flag that widens *what* you perceive should never also widen
*where*. `perception` answers "can I see through invisibility", not "how far
can I see"; the moment it sits on the same line as `continue`, it is answering
both.

The test that caught it had to stop hardcoding a "far away" tile — `(2, 2)` was
inside the starting room on that seed. It now asks the player's own
`visible_tiles` for a tile genuinely out of view.


## Worn gear was still lying on the floor (2026-09-20)

Adding the `"(w. a short bow)"` tag to sighting and `look` lines turned up the
reason a hobgoblin's armour was being announced twice: `equip_silently` never
took the item's `Position` off. Monster gear is spawned *as loot on the tile*
and then put on, so every worn piece stayed a floor item — drawn under its
owner, spotted on its own line, and pickable off the ground while the monster
still got its bonus from it.

What it teaches:

* `drop_equipment` inserting a `Position` when gear comes off is the tell: if
  worn gear kept one, that insert would be redundant. The invariant ("worn is
  carried, carried has no `Position`") was already written down in
  `levels.rs::tear_down_the_floor` — just never enforced at the one door onto
  it.
* A test can be the thing that hides the bug. `saveload::round_trip` swept the
  floor clean by despawning everything with a `Position`, which silently also
  swept up worn gear; the moment worn gear lost its `Position`, the sweep
  missed it and the save grew an extra suit of armour. A cleanup filter that
  names a component is an assumption about that component.
* Articles are per-slot, not per-item: "a short bow" but "ring mail". `Slot::Body`
  is the whole rule, which is why `worn_phrase` takes a slot and not a name.


## The drop-table test was reading the floor, not the roller (2026-09-20)

Every floor now gets a coin before its item budget is spent — blue on odd
depths, red on even — and the last floor of each difficulty tier gets a draw
from `catalog::PROGRESSION_ITEMS` too. The first thing that broke was
`loot::floor_loot_follows_the_rogue_drop_table`, which walked four floors per
seed and counted what was lying on them.

What it taught:

* **A test that samples the world instead of the function measures everything
  the world does.** Its own comment said it proved "the roller honours the
  weights it is given", but it read floors, so any placement that skipped
  `roll_item` counted as a roll. It now calls `roll_item` 12000 times directly
  and is five times faster besides — the floor walk was never the property.
* **The second loot test was passing by a hair.** `the_item_budget_scales_with_depth`
  compares shallow against deep; three items handed to every floor alike
  diluted the ratio to 1.57 against a 1.5 threshold. It passed, which is the
  worrying part. It now subtracts the guarantee from both ends, and the margin
  is back where the author put it.
* **Two arrow-trap tests counted arrows floor-wide.** They mean "the trap's
  tile is clean / has one spent arrow", and they said "the floor has no arrows
  on it" — so the moment the content stream shifted and seed 2 rolled a bundle
  of arrows into some room, both failed. Now counted on the trap's tile, which
  is also the player's landing tile and so is the one tile floor loot can never
  reach.
* **`PotionEffect` is alphabetical and the save file is positional.** The new
  `Magic` row goes at the *end*, not under M, with a comment saying why — the
  one convention in that enum that a reader would otherwise "tidy".


## Trick shots became a chain (2026-09-20)

Five changes that turned out to be one: a trick shot is now a *reaction*, not
an event. `traps::burst` — the single place every trick-shot burst is queued —
ends by setting off everything under its own footprint (`chain_react`), so the
chain came for free everywhere bursts already were: a shot trap, a shot coin,
the relic's triple answer, a wand's blast.

What it taught:

* **The termination argument belongs next to the recursion.** Every link is
  despawned *before* its own burst opens, so nothing is ever a link twice and a
  floor holds finitely many. The one thing that is never spent — the Element of
  Yoord — is therefore the one thing deliberately left out of the chain, or a
  burst that reached it would answer itself forever. That exclusion is not an
  oversight to be "fixed" later; it is the base case.
* **An author has to be threaded or the mechanic inverts.** Chaining into coins
  destroys them. Without carrying `shooter` through `detonate_trap` → `burst` →
  `chain_react`, a good shot would nuke your own coin pile for nothing. The
  param is noise on four signatures and the whole point of the feature.
* **A chain runs on past the blast that started it.** `claim_from_afar` took an
  `Entity` and assumed it still existed; `Promise::attach` uses `entity_mut`,
  which panics. A monster that zaps a wand into its own feet was already enough
  to reach that before any of this — the chain only made it likely.
* **`would_help` is a gate on *stepping*, not on the effect.** `learn_spell`
  trusted it for the `SPELLSET_CAP`, and `claim_from_afar` deliberately has no
  such gate ("a coin you shoot is allowed to be a waste"). So a *shot* hero coin
  taught a fifth spell past the cap. A cap enforced by a caller is not enforced.
* **Aiming and covering are different verbs.** `detonate_at` (the aimed shot)
  skips a `Hidden` trap — you cannot line a shot up on a mechanism nobody has
  found. A blast rolling over the same tile has no such rule. Putting the check
  in `trap_at` would have been one line and wrong: `spells.rs` asks it "is this
  tile free to plant on", where hidden traps very much count.
* **A tell that can be acted on outranks a tell that cannot.** The magenta cell
  under a monster standing on a coin or a found trap is painted *after* the
  status tints, so it wins over "asleep". The two never actually disagree — a
  helpless monster on a live trap is both at once — but the one that offers you
  a move is the one worth reading first.
* **A chain of heavy thumps is a map that never stops moving.** The burst's
  shake dropped from `Heavy` to `Hit` for the same reason its explosions now
  queue one behind the other (`Particles::hold`): the feel layer's dials are
  sized for *one* of a thing, and a feature that makes a thing happen five times
  has to re-ask all of them.

## Special levels, and what Rogue's layout had been hiding (2026-09-26)

Six whole-floor variants (`models/src/map/special.rs`). Most of the work was
finding the places the old 3x3 grid had quietly guaranteed something.

* **Rogue rooms never touch, so a door never led into a room.**
  `flood_fill_room` spread through `Door`, which was harmless while every door
  opened onto a corridor. Once vault cells and castle towers share walls, it
  lit the whole vault from the first cell. The flood now looks past a door and
  stops, except when the player is standing on the door.
* **`Map::blocks` answered two questions.** It stood for "a wall" to sight and
  shots, and for "can't stand here" to feet. Water splits them: `walkable(x, y,
  swims)` covers feet, and `blocks` stays for sight. Every `blocks(` caller got
  sorted into one bucket or the other. Particle, blood and smoke callers stay
  on `blocks`.
* **Population assumed rectangles and a start room to skip.** Rooms are now
  tile lists (`Rooms`), and a one-room floor spawns in its only room.
  `random_point_in_room` on a Rect could hand out a stair tile. Tile lists
  can't.
* **A spell written for the player to cast only ever looked for monsters.**
  `thunderbolt` used `monster_at` (Faction::Monster), so the eel's innate
  bolt missed the player every time. It now uses `actor_at`. `sting` still uses
  `monster_at`, which is harmless until something casts it innately.
* **The level roll has its own seed stream (`level_rng`).** Rolling the kind
  from `layout_rng` would have reshuffled every ordinary floor on every seed.
* **Water takes items in one schedule step (`sink_system`), not at each
  landing site.** Throws, drops, corpse gear and a swimmer's spawn kit all
  reach the floor by different paths. Worn and packed items have no
  `Position`, so "an item with a `Position` on water" means exactly "lying in
  the sea". Population calls the same `sink_items` silently at the end.
* **Worn gear sits in the `Backpack`, and the thief tests never knew it.**
  `steal_unequipped_item` only filtered out the Element, so a leprechaun
  could lift your worn armour, which its own doc and name ruled out. The
  `abilities.rs` `worn()` fixture never stows gear in the pack, so no test
  could see it. `a_thief_that_flees_never_lifts_worn_gear` stows it the way
  `initialize_world` does.
* **`NIHILURK_LEVEL` is read on load too.** The map is rebuilt from the seed,
  so loading a save under a different value rebuilds a different floor under
  the saved entities.
* **Eyeballing the TUI without tmux:** `script -qfc "stty cols 100 rows 30;
  ./target/debug/nihilurk NAME -ns" out.raw`, then replay the escape stream
  into a grid. Closing stdin reaches the game as a keypress and opens the drop
  menu, so ignore anything drawn after that.

## Helpers, and the tests that were one floor from failing (2026-09-26)

Snacks and fancies of peace (`TREATS`), and the boon companion they win
(`models/src/companion.rs`). A Helper is `Faction::Ally` plus a `Helper`
component. `Faction::Ally` had been in the enum, unused, since the start.

* **Loyalty is a component, not an effect row.** An `EFFECTS` row would
  have saved for free, but `revoke_all` (the wand of cancellation) strips
  every row that isn't in `IDENTITY_EFFECTS`. A cancelled Helper would keep
  `Faction::Ally` and lose the marker: an ally that neither follows nor
  swaps. It is saved as its own `EntitySave` field instead.
* **`Projectile` doesn't mean ammunition.** The dagger and spear carry it,
  and you wield both. "It's for throwing" keys on `LaunchedBy`.
* **`ai`'s `spatial` is a `HashMap`, and its iteration order is random.**
  Choosing "the nearest foe" by `min_by_key` over it picks a different one
  on every run when two are equally near. The tiebreak is now the tile.
* **Most "is that an enemy" checks asked `With<Mob>`.** That covers
  auto-explore's halt, cleave, whirl, lunge, the reticle's first pick, and
  the "You see a rat" line. A Helper in view halted every walk and
  re-announced itself at every corner. Each got the `Faction`/`Helper` filter
  where it lives. `monster_at` already filtered on faction.
* **Two tests were passing by a hair, and a new drop category tipped
  both.**
  * `loot::rolled_loot_follows_the_rogue_drop_table` counted anything with
    `Stack` as ammo, so treats (which stack) inflated the armoury. Treats get
    their own count now, checked before ammo.
  * `traps::no_trap_is_planted_in_a_doorway…` wanted more than 50 of 60
    floors trapped, and HEAD rolled 51. The expected share is about 87%
    (four slots at 40%), so that bound failed on about a third of possible
    content streams. It now samples 200 floors and asks for 75%.
  * Measured by setting the treat weight to 0: the count went back to 51.
* **Out of path, not fixed:**
  * The `pt` and `es` builds were already broken at HEAD: 24 string
    functions existed only in `en` (`beware_lightning_breath`, the
    special-level arrivals, the `_self` ghost lines…). Another session filled
    them in the same day, and `.githooks/pre-commit` now builds every
    language.
  * Use on an item with no use still spends a turn ("You can't use X right
    now"). Only treats and ammo are refused before the queue now.
  * `split_one` and `restore_from_catalog` match a row by `Name.what`, which
    is `content_name(id)`. That holds while no language translates content
    names. The day one does, a quiver throws whole and a save loses its
    arrows' flight.
* `docs/reference/rendering.md`'s layer 9 listed tint colours the code
  hasn't used for a while. It now matches `status_tint`.

* **A new `en.rs` string breaks the pt/es builds silently.** `pt.rs` and
  `es.rs` don't glob-import `en`, so each new `strings::` function needs its
  own pt/es version or those binaries stop compiling — and the default
  `cargo test`/`clippy` only builds `lang-en`, so nothing flags it. 24 went
  missing that way; fixed 2026-09-26. Check with `cargo check -p engine
  --no-default-features --features lang-pt` (and `lang-es`).

## Every mob became an agent (2026-09-26)

`ai.rs` no longer decides anything. Each turn it builds a `Percept`,
`agents::think` runs the mob's `RuleSet`, and `ai::act` carries out the one
`Action` that comes back. No memory is kept between turns
(`models/src/agents.rs`, `docs/reference/agents.md`).

* **Aggravation had been overwriting the tactic it was laid on.**
  `MovementType::Aggravated` replaced `Chase`/`Ambush`, so "switch behaviour
  once in view" had nothing to switch back to. It is an `Aggravated`
  component now. The variant stays in the enum (saves are positional), and an
  old save's `Aggravated` loads as `Chase` plus the component.
* **I had room-leashed the Helper.** `recruit` sets `movement_type = Chase`,
  and the step check leashed every `Chase` mob, so a Helper in a room could not
  follow the player into a corridor. That contradicted the doc written the
  same day, and no test walked a Helper out of a room. The leash now belongs
  to the rule set (`RuleSet::leashed`, `CHASER` only).
  `agents::a_helper_is_not_room_leashed` walks one out.
* **The old shot ignored the tactic.** Anything with a bow and a line to the
  player shot it, `Static` included, and a test elsewhere relied on a `Static`
  archer shooting. Under rule sets `STILL` never acts; that archer is a
  chaser now.
* **The dragon's 1-in-6 `InsteadOfAttacking` roll is gone, and so is the
  `Moment`.** The chaser's own loop ("pick a spell at random from the
  spellset, fire it if you can, else melee") made it a second, competing way
  to cast. Its four tests in `abilities.rs` and one in `body.rs` became agent
  tests covering the same behaviour: a wild dragon breathes on you, an eel's
  bolt finds you, and a spell-less creature melees. Wild dragons now breathe
  whenever they have a line, and that is a real balance change.
* **A spellset is derived, not stored.** It is the `INNATE_SPELLS` rows whose
  grant the mob carries, plus any `Spellset`. A wand of cancellation that
  strips `FireBreath` therefore strips the breath too, with no second copy to
  forget.
* **Allies check the footprint; monsters don't.** `CAST` asks `ally` and
  `agents::reaches`, which knows the Fireball (`items::blast_cells`, split out
  of `elemental_blast` so both use the same cells) and the Thunderbolt. An
  unknown footprint counts as "reaches the player", so an ally never casts
  it. Force Lance has none yet, which matters for the GDD's Black Mage.
* **The random roll uses `getrandom`, not `GameRng`.** Drawing from the seed
  stream once per mob per turn would have shifted every later combat roll in
  every seeded test. Confused staggering was already on `getrandom`.
* **Walking the tutorial for real caught a monster that doesn't exist.** The
  first draft of `give-a-monster-a-mind.md` made a kobold a coward; the
  bestiary has no kobold. It was run end to end in a scratch copy and uses the
  emu now. Also: this machine has no `rsync`.

## 2026-09-30 -- crates.io prep

* **`engine`, `models`, `strings` are taken on crates.io.** Packages are now
  `nihilurk`, `nihilurk-models`, `nihilurk-strings`, `nihilurk-particle-core`.
  Directories and library names are unchanged (`[lib] name`, and
  `package = "..."` on the dependency), so no `use` line moved. `cargo -p`
  takes the package name; `docs/explanation/why-the-crates-are-named-twice.md`.
* **`cargo publish --workspace --dry-run --allow-dirty` passes** for all four.
  Nothing has been uploaded. The real publish needs a version bump first: the
  `v0.1.1` tag predates `master`.
* **`cargo install nihilurk` without `--bin nihilurk` also installs
  `nihilurk-dispatch`,** which then can't find `nihilurk-<lang>`. Documented,
  not fixed. `required-features` on that bin would hide it if it bites.
* **`aur/PKGBUILD` now says `-p nihilurk`,** which is right for the next tag
  and wrong for `v0.1.1`. Bump `pkgver` and the sha in the same commit.
* **Only `nihilurk -content` was run on a `--features lang-pt` build,** and it
  prints English. Not checked whether that is intended.

## 2026-09-30 -- GitHub release workflow

* **`release/package.sh` + `release/test_package.sh`** hold the build logic so
  it runs locally; `.github/workflows/release.yml` only calls them. The test
  passed for `x86_64-unknown-linux-musl` (4m31s, fat LTO x4). It also checks
  the four language binaries differ, which catches a stale `cp` in the loop.
* **Not run: the workflow itself.** actionlint is clean (brew-installed);
  there has been no push, so the first tag is the first real run.
  `ubuntu-24.04-arm` is unverified for this repo; the arm runner is free only
  on public repos.
* **`nihilurk-dispatch` is behind `required-features = ["dispatch"]`,** so
  `cargo install nihilurk` installs one binary. Packaging passes the feature.
* **`aur/PKGBUILD` stays on the `v0.1.1` snapshot on purpose;** `aur_check.sh
  --head` is red until the release that ships the rename. Version is 0.1.2.
* **`-content` in English on `lang-pt` is expected:** pt re-exports English
  until translated (CONTRIBUTING.md).

## 2026-10-01 -- crates.io metadata

* **MSRV is 1.91 for `models`/`nihilurk`, 1.85 for `strings`/`particle-core`.**
  Built on 1.85.0 and 1.91.0; 1.90 not tried. `models` uses let chains (1.88)
  and a const `TypeId::of` (1.91). The floors are per crate on purpose: a
  shared 1.91 would make `compat/`'s ESP32 check refuse `particle-core`.
* **The new metadata test caught `procedural-generation` (21 chars).**
  crates.io caps keywords at 20, so that upload would have been refused.
* **`readme` is inherited from the workspace** (`README.md`), so the library
  pages show the game's front page and crate map.
* **Categories were checked against the live crates.io list;** keywords and
  descriptions are my wording, not tested against search ranking.
