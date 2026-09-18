# Retiring the weak and the ugly

A correction before anything else: `scs.md` and the `scs-architecture/` set were written against a
project called roog. The rename landed at `8efa8ec`, and it reached everywhere — there is not one
occurrence of "roog" left in tracked source or docs (`git grep -il roog` outside those two documents
returns nothing). This plan uses the current name throughout and treats the essays' "roog" as
"nihilurk" without further comment.

## Where it stands

The strong case in `scs-architecture/02-strong-weak-ugly.md` holds up against the actual tree. Every
monster is a row in `BESTIARY` (`models/src/monsters.rs:247`), every item a row in one of nine
tables in `models/src/catalog.rs`, every trap a row in `TRAPS` (`models/src/traps.rs:152`), and
nothing else in the codebase enumerates a species by name — `models/tests/content.rs` proves it by
walking `content_names()` and spawning every one of them
(`every_content_name_spawns_and_keeps_its_name`, `content.rs:32`). A property is a component a
system asks about, never an enum a system has to recognise: `FireImmune`, `SeesInvisible` and the
rest of the thirty-six effects in `EFFECTS` (`models/src/effects.rs:475`) are attached the same way
whether a dragon is born with one or a ring lends it. The turn is one `bevy_ecs::Schedule`, run once
per player action, and the order is `.after()` edges, not convention — `engine/src/main.rs:428-467`.

That schedule is sixteen systems, not the thirteen `docs/explanation/ecs-in-nihilurk.md` claimed
before this plan touched it: `smoke_system`, `snare_system`, `reveal_mimics`, `ai`,
`monster_pickup_system`, `trap_system`, `throw_system`, `item_system`, `move_system`,
`equipment_effects_system`, `combat_system`, `reaper_system`, `dungeon_lord_system`,
`passive_ability_system`, `visibility_system`, `score_turn_system`. Fifteen take `&mut World`;
`visibility_system` alone is a `Query` system. The doc's own diagram had thirteen nodes and matched
its own wrong count, so the drift was internally consistent and still wrong — the kind of error a
reader has no way to catch from the page alone. `docs/reference/input-and-turn-loop.md`, the page
whose own header claims "if this page and the source disagree, the source is right and this page is
a bug", was itself missing `score_turn_system` from the fixed order it prints. Both are fixed as
part of this plan; see Phase 1.

The intent queues are four, not three: `AttackQueue`, `UseQueue`, `ThrowQueue` and `MoveQueue`, all
`Vec`-backed resources at `models/src/components.rs:1014-1049`, each filled by input or AI and
drained by exactly one named system (`combat_system`, `item_system`, `throw_system`, `move_system`).
Three separate pages said "three" — `docs/explanation/ecs-in-nihilurk.md:35`,
`docs/how-to/work-with-the-ecs.md:90`, and `docs/reference/components.md:262` — which means the
drift was not one stale sentence but a fact that had already fallen out of three places at once with
nothing to notice. That is itself evidence for the weak point this plan is about: a fact repeated in
prose, with no single source, drifts in exactly the way a table does not.

`models/tests/content.rs` is real and does real work: sixteen tests (`rg -c '#\[test\]'
models/tests/content.rs`) covering uniqueness, spawnability, depth gating, weight-proportional
drawing, and the specific "a dud effect must never be normal loot" rule
(`the_dungeon_never_generates_a_dud_as_normal_loot`, `content.rs:349`). What it does not cover is a
single row's own legality: nothing stops a future `BESTIARY` or `TRAPS` row from carrying
`.weight(0)` (silently unreachable, the exact mistake
`every_drop_category_can_actually_produce_something` already catches for `DROPS`, `content.rs:216`),
a `min_depth` past `FINAL_DEPTH` (`13`, `models/src/constants.rs:123`), or a nonsensical pair like
`.mimics().invisible()` — no row does either today, but nothing would refuse one that did.

The one place this codebase's own inconsistency showed up in the source rather than in a doc: every
logging call site but one assumed `GameLog` exists (`world.resource_mut::<GameLog>()`, 231 of the
232 hits `rg -o 'GameLog>\(\)' models/src engine/src | wc -l` found before this plan's fix), while
`score::announce_combo` alone treated it as optional (`get_resource_mut`, `models/src/score.rs:211`
before the fix). No test relied on the difference — `models/tests/score.rs` does not exist, and
nothing else in the tree calls `announce_combo` from a bare world. It was an unstated invariant with
one silent exception, not a bug with a victim; Phase 1 closes it by stating the invariant in the one
place it was violated.

## What the current shape buys

**`&mut World` is not a workaround here; it is what a roguelike rule actually is.**
`models/src/equipment.rs:117-135`'s own doc comment makes the case better than a fresh one would:
resolving a thrown wand of polymorph despawns a creature, spawns its replacement, sets off every
trap and coin in the blast, and logs a dozen lines, and none of that has an honest tuple of
component accesses to declare up front. `visibility_system` (`engine/src/main.rs`, registered
last-but-one) is the one system in the tree that reads position and writes fog without ever
branching on what it just wrote — the whole population of systems shaped like a `Query`. Forcing the
other fifteen into that shape would not remove the coupling `resolve_attack` or `resolve_wand_throw`
actually has; it would just split one legible procedure into a `Query`, a `Commands`, and six
`ResMut`s a reader has to reassemble by hand.

Compactness is the right optimisation target for the same reason. `docs/how-to/add-a-monster.md`
states the cost of a new species as "one line in one table. There is no second place" and it is
true: `README.md`'s own worked example — `MonsterDef::row("pink dragon", ...)` plus `cargo build` —
needs nothing else touched, because nothing else names a species. That property is what every phase
below is priced against: a phase that adds a step to it has failed regardless of what else it buys.

The one deliberate full-world scan, `equipment::equipped` (`models/src/equipment.rs:131`), is not an
oversight left for later. Gear points at its wearer, the item system lifts an item out of the pack
mid-resolution, and every caller already holds `&World` partway through reading something else — the
narrow query that would replace it, `Query<&Equipped>`, needs `&mut World` and would cost every one
of those callers the borrow they are mid-use of. It is allocation-free and, on a floor of tens of
entities, cheap. That is a real trade already made with its eyes open, not a gap.

## Where the magic hides

**A content row's shape is checked; its legality is not.** `MonsterDef::row`
(`models/src/monsters.rs:79`) takes ten positional numbers and the builder methods chained after it
— `.grants`, `.invisible`, `.mimics`, `.weight` — compose freely, because nothing enforces that they
should not. `.mimics()` marks a row born disguised as an item until touched (`disguise_as_item`,
read by `reveal_mimics`); `.invisible()` marks a row unseeable without a ring of perception. Nothing
says a row cannot be both, and if one ever were, `reveal_mimics` would strip the mimic's disguise
correctly and the creature would still be invisible underneath — a mimic that, having been noticed,
cannot be seen, which is not a state either mechanic was written to produce. No row does this today;
the point is that the type system has no opinion, and the only thing that would notice is a player.

**A schedule step's assumptions live in a comment, when they live anywhere.** `rg -oi
'assumes|invariant|must run after' engine/src models/src` returns zero hits. The `.after()` edges
are real and are commented at the call site (`engine/src/main.rs:436-467` — the
reveal-mimics-before-ai reasoning, the monster-pickup-after-ai reasoning, and so on), which is the
ordering half of the contract. What is missing is the other half: what a system assumes is *already
true* of the world state when it starts, stated where a reader would look for it rather than
reconstructed by reading every system that could run before it. `combat_system` assumes
`equipment_effects_system` has already folded lent effects into `Loadout`-relevant components;
nothing at `combat_system`'s own definition says so, only the `.after()` edge fifty lines away in
`main.rs`.

**Three files said "three queues" and one said "thirteen steps"; both were provably wrong by the
time this plan opened `git log`.** That is not one stale sentence, it is the shape of the actual
risk: a fact restated in prose, with no compiler and no test standing behind it, drifts the moment
the code it describes moves and nothing forces the two back into agreement. `docs_style.sh` checks
link targets and header shape; it does not and could not check that a number in one file still
matches a count in another. `MANUAL.md` has the same disease from the player's side: `L` opens a
real, un-hidden look cursor (`engine/src/update.rs:1144`, `begin_look`) documented in
`docs/reference/input-and-turn-loop.md`'s "The aiming reticle" section, and `MANUAL.md` never
mentioned it before this plan's fix — not a deliberate secret like `T` (which
`input-and-turn-loop.md` explicitly says to keep out of the manual), just a command nobody added to
the page when it landed.

**Structural mutation, once actually counted, is not the seam it looks like.** `rg -o '\.despawn\('
engine/src models/src | wc -l` is 24 sites; `rg -o 'world\.spawn\(' … | wc -l` is 7. Reading all 24
despawns shows every one sitting exactly at the rule that decided the thing was gone — an item
consumed in `items/throwing.rs`, a corpse in `combat.rs:154`, a trap sprung in `traps.rs`, a level
transition clearing the floor in `map/levels.rs:249`. None of them despawn something a *different*
rule decided about; the mutation and the decision are the same function. The seven spawns are the
`Def`+`Bundle` chokepoints `docs/explanation/data-driven-content.md` describes — one per content
kind, plus `catalog.rs`'s own three category constructors. Item 4's stated worry — structural
mutation folded into rule evaluation in a way that hides what decided it — does not describe this
codebase as it stands.

## The model

The seven items were never seven different problems; reading them against the actual tree collapses
most of them.

**Items 2, 3 and 7 are the same question asked at three altitudes: "does the codebase know what it
thinks it knows?"** Item 7 asks it of the schedule and the queues — is the doc's claim about the
code still true. Item 2 asks it of a system — does anyone but the author know what it assumes. Item
3 asks it of a row — does anything check that what it declares is legal. None of the three wants new
machinery; each wants an existing fact stated somewhere a compiler, a test, or a reader passing
through will actually meet it. **State the seam, then guard it** is the one move underneath all
three, and it is not new to this codebase — `every_drop_category_can_actually_produce_something`
(`content.rs:216`) already does it for `DROPS`; the gap is that `BESTIARY` and `TRAPS` never got the
same treatment, and the doc pages never got a mechanism at all.

**Item 1 is closed, not open.** The inbound half (the four queues) and the cosmetic outbound half
(`Particles`/`Shake`, reached through `get_resource_mut` so a headless test world runs the same path
— `docs/explanation/the-feel-layer.md`) were already exactly what the manifesto asks for: a system
decides, arms one resource, and moves on without waiting to see what happened. The narrative
outbound half — 232 raw `GameLog` writes — looks like the same problem wearing a bigger number, and
it is not: `GameLog` is read back, same turn, by `engine/src/update.rs:1538` (auto-explore stops
when `!log.unread.is_empty()`) and `:1756` (fast-move breaks on the same condition). Those two reads
make `GameLog.add` a *synchronous* gameplay signal, not a fire-and-forget cosmetic one — the one
thing `Particles` and `Shake` are and `GameLog` structurally cannot be. Wrapping 232 heterogeneous
messages behind named "arms" would either produce 232 near-identical one-line wrappers (ceremony,
not a seam closed) or collapse to a single generic `log::add(world, msg)` that renames `.add` and
changes nothing — the "bus by another name" this plan's own calibration rules out. The one real
defect item 1 left behind was the `score.rs` inconsistency above, and that is a stated-invariant
problem (item 2), not a missing-queue problem. Closing it there is what Phase 1 does.

**Item 6 is closed for the same reason item 1 is.** "Visible effect resolution" already describes
`GameLog`: a rule that decides something worth saying happened writes it down, in the rule, in the
same line a reader would read to understand what the rule does. That is the *inspectable* half of
the `careful-review` specimen's own distinction, achieved by never having left it — there is no
resolver, no id, no handler table to go missing.

**Item 4 is closed on the evidence in "Where the magic hides".** Structural mutation is not isolated
from rule evaluation because it does not need to be; it already sits at the decision site in every
one of the 24 despawns and 7 spawns this plan found. There is no phase to write here, because there
is no gap to close.

**Item 5 stays declined**, on the reasoning `models/src/equipment.rs:125-131` already gives and this
plan re-verified rather than re-argued: the scan is allocation-free and cheap at "a floor holds tens
of entities." The threshold at which that stops holding is when a floor's live entity count reaches
the low hundreds — the point at which a linear scan called from several systems per turn
(`equipped`, folded again by `loadout` and `equipped_total` in `models/src/effects.rs:638-658`)
starts costing more per turn than the rest of that turn's resolution combined. Nothing today
measures floor population, which is also the detection: the trigger is `populate_level`'s own entity
count crossing a few hundred, and the way to notice is the same `cargo run -p engine --
-content`-adjacent instinct that already exists — a debug counter on `world.iter_entities().count()`
printed once per floor, cheap enough to leave permanently on a debug build and unnecessary until the
day a floor's population design changes by an order of magnitude. Building the index resource now
would be a cache with nothing yet to invalidate it correctly against.

That leaves two items with real, un-closed work: **2+7, collapsed, as "seams named and guarded"**,
and **3, standing alone, as "row legality"**. No eighth item clears the bar set for one — the two
candidates this research turned up, the `L`-in-`MANUAL.md` gap and the `score.rs` inconsistency, are
both folded into Phase 1 below as evidence and fixes, not proposals, because both are instances of
item 7 and item 2 respectively rather than a distinct architectural altitude.

## Roadmap

### Phase 1 — Seams named and guarded (landed this session)

**The change.** Corrected every place this research found the schedule step count, the schedule's
own printed order, or the intent queue count stale — four doc pages, one player-facing manual, and
one source inconsistency — and added a mechanism so the same drift cannot recur silently.

* `docs/explanation/ecs-in-nihilurk.md`: "thirteen steps" → sixteen, "twelve… exactly one" →
  fifteen/one, "three intent queues" → four; the schedule diagram now says explicitly which three
  steps it omits and why, instead of silently matching a wrong count.
* `docs/how-to/work-with-the-ecs.md`: the same step-count fix, "all three queues" → four, and the
  "queue an intent from input" table gained the `WantsToMove`/`MoveQueue` row it was missing.
* `docs/reference/components.md`: the events table gained the same missing row; "all three queues" →
  four.
* `docs/reference/input-and-turn-loop.md`: the printed schedule order gained `score_turn_system`,
  the one step it was missing, plus the one-sentence reason it runs last (mirroring the existing
  reasons given for the other tail positions).
* `MANUAL.md`: `L` (look) added to "Useful commands" and "Quick reference" — a real, undocumented,
  non-secret command.
* `models/src/score.rs:211`: `announce_combo` now requires `GameLog` like every other logging call
  site, with a comment stating the invariant instead of silently exempting itself from it.
* `.githooks/pre-commit` (new): runs `docs_style.sh` when a doc changes, and — by grepping the
  *staged diff*, not the whole file, for `.after(` edges in `engine/src/main.rs` or queue-struct
  additions/removals in `models/src/components.rs` — refuses a commit that changes the schedule or a
  queue without touching a page under `docs/`. Enabled per clone with `git config core.hooksPath
  .githooks`; documented in `README.md`.

**Files.** `docs/explanation/ecs-in-nihilurk.md`, `docs/how-to/work-with-the-ecs.md`,
`docs/reference/components.md`, `docs/reference/input-and-turn-loop.md`, `MANUAL.md`,
`models/src/score.rs`, `README.md`, `.githooks/pre-commit`.

**Verification.** `cargo fmt --all` (clean; also caught pre-existing, unrelated drift in
`compat/src/main.rs` — verified via `git stash` that it predates this session and reformatted it as
a zero-risk side effect), `cargo build --workspace`, `cargo test --workspace` (every suite green,
`models/tests/determinism.rs`'s nine tests included), `cargo clippy --all-targets` (clean),
`./docs_style.sh` (29 pages, house style kept). The hook itself was exercised by hand: staging an
unrelated file passes; staging a real `.after()` edit to `engine/src/main.rs` alone fails with the
guidance message; staging the same edit alongside a `docs/` file passes and runs `docs_style.sh`.
`compat_test.sh` was attempted for a before/after comparison and fails identically with or without
this plan's changes (`no matrix rows matched` / `target was empty`, a pre-existing
`cross`/matrix-configuration issue in this environment, confirmed via `git stash`) — the failure is
independent of this diff, and nothing in this phase touches platform-sensitive code.

**Cost.** The hook is opt-in per clone; nobody is forced to enable it, and a contributor who never
runs `git config core.hooksPath .githooks` gets none of its protection. Its diff match is a
heuristic — grepping for `.after(` and struct names, not parsing Rust — so a change that reorders
systems without adding or removing an `.after(` line, or that touches a queue's *fields* rather than
its declaration, would not trip it. Six doc sentences and one source line changed meaning; each is a
fact that can drift again the moment the schedule or the queues change further, with only the hook
(when enabled) standing between that and a seventh stale reference.

**Success metric.** `rg -i 'thirteen|twelve' docs/explanation/ecs-in-nihilurk.md
docs/how-to/work-with-the-ecs.md` and `rg 'three queues|all three' docs/` both return nothing;
`docs/reference/input-and-turn-loop.md`'s printed order has sixteen names; the hook fires on a
synthetic `.after()` edit and clears when a `docs/` file is staged alongside it.

**Ratio.** Do it now. Lowest cost on the list, fixes provable defects rather than theoretical ones,
and the guard is the only phase here that prevents its own problem from recurring rather than just
repairing the current instance of it. Helps the next person who opens
`docs/explanation/ecs-in-nihilurk.md` while deciding where a seventeenth system should sit in the
order, or opens `docs/how-to/work-with-the-ecs.md` while wiring a fifth intent queue — before this
phase, both would have copied a wrong count forward without any way to notice.

### Phase 2 — Row legality (landed this session)

**The change.** Extend `models/tests/content.rs` with the structural checks `DROPS` already has and
`BESTIARY`/`TRAPS` did not: every bestiary and trap row's `weight` is non-zero (mirroring
`every_drop_category_can_actually_produce_something`, `content.rs:216`), every `min_depth` is at
most `FINAL_DEPTH` (`13`, `models/src/constants.rs:123`), and `.mimics()` and `.invisible()` are
never set on the same row — the one combination this research found that both compiles cleanly and
produces a creature no mechanic was written to handle correctly. A doc comment on
`MonsterDef::mimics` (`models/src/monsters.rs:135-143`) now states the exclusivity as a rule, not
just a test, so a future row author meets the reason before the row fails to build a test's
assertion.

**Files.** `models/tests/content.rs` (three new tests: `every_bestiary_row_can_actually_be_drawn`,
`a_mimic_is_never_also_invisible`, `every_trap_row_can_actually_be_drawn`), `models/src/monsters.rs`
(the doc comment on `mimics`).

**Verification.** `cargo test -p models --test content` — 16 tests before this phase, 19 after, all
green. Each new test was independently confirmed to fail before being left correct: bat's weight set
to `0` (`every_bestiary_row_can_actually_be_drawn` red, "bat has no weight, so it can never be
drawn"), bat's `min_depth` set to `99` (same test, red on the depth assertion), xeroc given
`.invisible()` alongside its existing `.mimics()` (`a_mimic_is_never_also_invisible` red), and
trapdoor's weight set to `0` (`every_trap_row_can_actually_be_drawn` red) — each reverted immediately
after, confirmed via `git diff --stat` showing zero net change to the content tables themselves.

**Cost.** Three test functions, run once per `cargo test`, no runtime cost in the shipped binary —
this validation belongs in tests and debug builds, never on the turn loop. The new invariant ("a row
cannot be both a mimic and invisible") is one more thing a future content author has to have read
about, though the doc comment is exactly what makes that a one-sentence cost rather than a
rediscovery.

**Success metric.** `cargo test -p models --test content` gains three tests and stays green; each one
independently verified to fail against a deliberately broken row before being left correct.

**Ratio.** Do it now, immediately after Phase 1. It is the one item on the original seven that is
genuinely unfinished rather than already done, already fine, or already declined, and it costs a
handful of test lines against a real gap `DROPS`'s own test already proved worth closing. Helps
whoever adds the next bestiary or trap row and mistypes a weight or a depth — today that mistake
surfaces as a species nobody ever sees, three sessions into a playtest; after this phase, `cargo test`
says so on the spot.

### Already closed: items 1, 4, and 6

No phase is proposed for these, and that is the finding, not an omission. Item 1's inbound and
cosmetic-outbound halves already match the manifesto; its narrative-outbound half (`GameLog`) cannot
be queued the way `Particles` is without breaking the two same-turn reads in
`engine/src/update.rs:1538` and `:1756`, and the one real defect it left (`score.rs`'s
inconsistency) is closed in Phase 1 as a stated-invariant fix. Item 4 is closed on the despawn/spawn
count in "Where the magic hides" — the mutation already sits at the decision site everywhere this
research checked. Item 6 is closed because `GameLog` already is the inspectable, in-the-rule effect
record the item asks for.

### Declined: item 5

Relationship indexes for `equipment::equipped`. The scan is allocation-free and cheap on the floor
sizes `populate_level` actually produces (tens of entities). Revisit when a floor's live entity
count reaches the low hundreds — the point at which several per-turn callers (`loadout`,
`equipped_total`, HUD reads) each re-walking the whole world starts costing real turn time.
Detection: a debug-build counter on `world.iter_entities().count()` per floor transition, cheap
enough to leave running, unnecessary until floor population design changes by an order of magnitude
from what it is today.

## The new weak and the ugly

Stating a seam costs a place for the statement to go stale in a *new* way. The pre-commit hook this
plan adds is a heuristic over a git diff, not a parser over the AST — it will catch a `.after()`
edge changing and a queue struct being added or removed, and it will not catch every way the
schedule's *meaning* could change without either of those textual markers moving. A maintainer three
years from now who reorders two systems by editing the tuple `main.rs` passes to `add_systems`
without touching an `.after()` line at all — because the two systems in question have no ordering
dependency on each other, only on their neighbours — will get no warning from this hook and no
reason to open the docs, and the schedule's printed order will drift exactly the way it already did
once. The honest fix for that is a test that walks the schedule's actual system order at runtime and
diffs it against a constant the docs also cite — genuinely more machinery than this plan adds, and
not proposed here because the drift it would prevent has happened exactly once in the project's
history and cost one afternoon of a session like this one to find and fix. That is the trade being
made, not a problem solved: cheaper now, and the next person to hit it will have less warning than a
full parser would have given them.

## Thesis

nihilurk's architecture was never short on discipline; it was short on places for that discipline to
be checked instead of remembered — and this is falsifiable in one clause: find a fact this plan
stated as guarded (the schedule count, the queue count, a bestiary row's legality) that has since
gone stale with nothing catching it, and the claim is wrong.

