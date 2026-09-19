# SCS: The Simulationist Component System

## Executive summary

SCS is nihilurk's architecture in one sentence: a simulation-first, component-backed, system-driven world model whose rules are executed in an ordered turn schedule and whose state changes are intentionally direct where the game needs them to be.

This is not a generic ECS framework. It is not a purity test. It is a roguelike architecture built to preserve causality, performance, and game meaning — general enough that it would fit another turn-based, tactical, card or board game with no change to the idea, only to the rows.

---

## 1. Definition

Five questions, answered once, the same way every time:

| Modeling... | Use a... | In nihilurk |
|---|---|---|
| a fact about one entity | **component** | `Poisoned`, `FireImmune`, `Position` |
| a fact about the whole run | **resource** | `Map`, `GameRng`, `GameLog`, the four intent queues |
| a rule that runs every turn | **system** | `combat_system`, `ai`, `reaper_system` |
| "this before/after that" | **the schedule** | one `Schedule`, sixteen systems, run once a turn |
| a thing that exists | **a data row** | `BESTIARY`, `TRAPS`, the nine item tables |

That is the whole architecture. Not a generic ECS app, not a plugin-based engine, not a purity model — a game architecture shaped by the fact that a roguelike is a simulation with causal order, structural mutation, and explicit turn semantics. Everything past this point either justifies these five rows or corrects a place a project drifted from them.

**Who this is for.** Roguelikes, tactics, card and board games, sim-heavy prototypes — anything where "what happened, in what order, because of what" is the game. A worse fit for real-time action, where frame-coherent query batching beats turn causality; that is a different game, and a different architecture is right for it.

---

## 2. Why the schedule is law, and direct mutation is correct

```mermaid
flowchart LR
    I[Player input] --> S[Systems, in schedule order]
    S --> M[World mutates]
    M --> R[Render current state]
    R --> I
```

A roguelike is not a static set of UI widgets reacting to framework events. Actors move, terrain changes, effects cascade, entities spawn and despawn, and *order changes outcomes* — a bear trap has to resolve before the movement it blocks, damage has to land before the death check reads it, who acted first changes who is still standing. A generic app framework has no domain notion of this; it optimises for a clean lifecycle — mount, update, unmount — because it has nothing to be causal about. Skip an explicit schedule here and that ordering knowledge does not disappear, it just hides in call order and registration, which is the hidden contract the rest of this document is about making legible.

That is why the schedule is not plumbing. It is one of the two opinions SCS adds on top of plain ECS; the other is that a system may mutate the world directly when the mechanics call for it. The standard objection to that — `&mut World` "escapes the model", a system should declare its reads and writes as a query so the scheduler can parallelise and the contract shows in the signature — assumes a system can state that set in advance. Many cannot. One attack can damage several actors, despawn a corpse, spawn loot in its place, trigger a trap standing under it, update visibility, and log a line, and which of those actually fire depends on runtime state a query shape cannot express. Forced into pure queries, the same logic does not vanish, it scatters across narrower systems glued by ad hoc flags — harder to read, not easier. `&mut World` is the honest shape of a rule like that. The failure mode was always mutation nobody wrote down, never mutation itself.

None of this makes Rust's borrow-checker friction go away, and it should not try to. That friction is universal in large Rust codebases; the only real question is whether it gets absorbed into written convention — what belongs where, who mutates when — or fought fresh at every call site. SCS bets on explicit phases and explicit content tables as the one place to write that convention down, which is also why the project reads as *feral* rather than polished: direct, compact, opinionated, optimising for code that reads like the mechanics rather than for abstraction as its own reward. That is the right call for a simulation. The goal was never maximum abstraction; it was maximum clarity of the behaviour actually intended.

---

## 3. The three: what is strong, weak, and ugly

Revised after a first pass actually checked it against the tree instead of asserting it. Two rounds
of that checking are in `scs-improvement-plan.md`, which this section now defers to for anything that
needs a file and a line rather than a claim.

### The Strong

The strongest thing about this architecture is that it has a coherent identity, and that identity has
now been audited rather than taken on faith.

- The world is the primary object of computation.
- Components are the vocabulary of state.
- Systems are the verbs of behavior.
- The turn schedule is part of the rules, not merely an implementation detail.
- Content is data-driven and declarative.
- The codebase remains compact because it is expressive, not because it is hiding complexity.

This gives the project a very real strength: it is easy to tell what the game is doing and why. It is
no longer only easy to tell — it is checked. `models/tests/content.rs`'s nineteen tests audit exactly
this: every monster, item and trap the game knows is a row and nothing outside the tables enumerates
one by name; a row that cannot actually be drawn, or claims a floor past the last one, or is a mimic
and invisible at once, now fails a test instead of surfacing as a species nobody ever meets.

Parts of nihilurk are deliberately built around direct mutation because the mechanics demand it. The
project's docs are explicit about this, and that is a sign of health. The architecture is not trying
to be an idealized ECS in the abstract. It is trying to be honest about the actual problems the game
solves — and when that claim was actually tested, by reading every one of the two dozen places the
game despawns something and the seven places it spawns one, it held: every one sat at the exact rule
that decided the thing was gone or born, not scattered where a different rule's decision has to be
reconstructed from a distance.

The architecture also has real extensibility. New monsters, items, traps, and effects are added as
data rows, not through a giant list of hardcoded branches. That is a major engineering win, and it is
the one claim in this section that was already a test before anyone reread this essay
(`every_content_name_spawns_and_keeps_its_name`) rather than becoming one because of it.

### The Weak

The weak point is not the imperative mutation model. The weak point is the boundary clarity — and a
verification pass has now narrowed what that means to one specific, remaining thing.

What turned out to already be fine, on inspection: content rows now check their own legality, not
just their shape. Mutation sits at the decision site everywhere it was checked. The turn's cosmetic
output and its narrative output were already exactly the "decide, arm, forget" shape the architecture
was always meant to have — nothing needed inventing there, only proving.

What turned out to be real, and worse than a hand-wave: the same fact, stated in prose in more than
one place, drifts and nothing notices. The schedule's own step count was wrong in the page written to
explain it, and had been for long enough that the page's own diagram matched the wrong count rather
than the code. The intent queue count was wrong in three separate files at once. Neither was caught by
any tool in the tree, because no tool was checking a number in one file against a count in another —
only a person rereading everything found it.

What used to remain open, narrowly and by name: **a schedule step did not state what it assumed.**
The `.after()` edges were real, commented, and load-bearing — the ordering half of the contract was
explicit. The other half, what a system assumes is already true of the world when it starts, now
carries a doc comment at the system's own definition, for all fifteen `&mut World` schedule steps;
`visibility_system` needs none, since its `Query` signature already states its assumptions in a form
the compiler checks. Closing this found one genuinely dead guard clause along the way — `snare_system`
checks `Ending::player_dead` at a point in the schedule where nothing can yet have set it true — left
in place and explained rather than removed, since the item was to state what a system assumes, not to
prune defensive code that costs nothing to keep.

### The Ugly

The ugly part is not that nihilurk is low-level or imperfect. The ugly part is that it is a little
feral, and that closing one seam opens a smaller one right behind it.

It is feral in the good sense: direct, compact, and fast. That has not changed and should not.

The newer ugliness is sharper than "some of the movement still feels magical." Naming a seam and
writing it down is not the same as guarding it — a stated invariant is a sentence about the code,
sitting next to it but checked by nothing, which is exactly the shape of claim that already went stale
three times over in one small area before anyone caught it. The one mechanism built to stop that from
recurring — a commit guard on the two files that drifted — only watches those two files; it says
nothing about the player-facing manual or the README, both of which are prose about the code with
nothing checking either against it, and one of which was already found silently missing a real
command. The honest reading is not that the architecture is dishonest about its seams. It is that
every seam this project has closed so far was closed by a person reading the file end to end, and the
next one will be too, because the alternative — a parser over the schedule's actual meaning rather
than a grep over its `.after()` edges — has not yet been worth its own cost.

---

## 4. Hidden magic, and where it stands

The thing worth fixing was never the direct mutation model; it was the hidden magic — conventions that are never written down or validated, which is what a convention becomes when nobody checks it. Two verification passes have now gone through the places that magic used to live:

- data rows that encode behavior implicitly — still true, and correctly so: `Grant` naming a component from a `const` table is the whole trick that makes a ring of fire resistance free once a dragon is fire-immune. What changed is that `BESTIARY` and `TRAPS` rows are now checked for legality before they reach a system, the same way `DROPS` already was.
- effects whose meaning is spread across tables and systems — still the intended shape, not a defect: one vocabulary in `models/src/effects.rs`, read by many systems that never learn where an effect came from.
- content definitions that rely on successful assumptions — narrowed to what a test can actually check: a row's weight, its depth, and one now-forbidden combination (a mimic that is also invisible) are enforced; what a mechanic *does* with a legal row is still, correctly, code rather than data.
- turn-order invariants that are known but not always stated plainly — closed. The `.after()` edges were already stated; the preconditions they exist to satisfy now are too, at each of the fifteen `&mut World` systems' own definitions.
- world mutation that is efficient but not well-separated at the boundary — checked, and turned out not to be true. Every despawn and every spawn in the tree already sits at the rule that decided it.

That was never a reason to throw the design out. It was a reason to formalize the contracts — and, once formalized, to find out which of them were already true. Status against the original seven-item improvement path, as of `scs-improvement-plan.md`'s third phase:

1. **Command and effect queues — closed.** Inbound intent already ran through four named queues; the cosmetic half already decided, armed, and forgot. The narrative half (the message log) looked like the same gap wearing a bigger number and was not: it is read back by the game loop in the same turn it is written, which makes it a synchronous signal, not a deferrable one. Queueing it would have broken the thing it was meant to fix.

2. **System invariants — closed.** Every `&mut World` schedule system states what it assumes about the world before it runs, at its own definition, not reconstructed from `main.rs`'s `.after()` chain. The statement is prose, not a `debug_assert!` — deliberately, since what each one actually assumes is the world's position in a schedule the compiler and the `Schedule` builder already fix, not a runtime fact a guard could check without inventing new bookkeeping the anti-goals would call a bus by another name. Written down is not enforced: this is a seam named, not a seam guarded, the way item 7's doc pages now are.

3. **Content validation — closed.** A weight that would leave a row undrawable, a depth past the last floor, and one impossible combination of properties are all checked before a row reaches a system, the same way the loot table already was.

4. **Mutation boundaries — closed, and needed no work.** Every place the world spawns or despawns something already sits at the exact rule that decided it.

5. **Relationship indexes — declined, on purpose, with a trigger.** The one expensive-looking world-wide scan is cheap at the floor sizes the game actually generates and allocation-free besides; replacing it now would be a cache with nothing yet to invalidate it against. Revisit when a floor's population reaches the low hundreds, and notice by counting.

6. **Better effect resolution separation — closed, for the same reason as item 1.** The message log already is effects made explicit and inspectable as a phase.

7. **Clearer docs at the seam — closed, for the seams that had actually drifted.** Two specific claims — how many steps the schedule has, how many intent queues exist — were flatly wrong, in three files at once for the second one, with nothing in the tree that would ever have noticed. Both are fixed, and a guard now watches the two files most likely to make either claim wrong again. It does not watch everything prose says about the code; the player-facing manual is unwatched, and was already found missing a real command by the same process that found the rest of this.

---

## 5. Why this helps a human, and why it helps an LLM

Five questions, same answer every time: fact-on-an-entity is a component, global-for-the-run is a resource, rule is a system, phase boundary is the schedule, content is a data row. That consistency is a design-time payoff, not a performance number. Debugging is easier because the causal path is not hidden behind a lifecycle abstraction; refactors are safer because the schedule stays stable while everything around it changes.

An LLM benefits from the same grammar for a sharper reason: less guessing. Nouns are always components, verbs are always systems, content is always a row and never sometimes-inline-logic. A narrower target surface means fewer wrong-layer edits and less inventing framework machinery that is not there. The data-driven bestiary and effects catalog already turn "add a monster" into a one-line change, not a hunt through control flow — that is the edit an LLM, or a new teammate, gets right the first time, and it is the whole reason this architecture is worth keeping legible rather than merely correct.

---

## 6. Designing with SCS

Struct, function, `for` loop: the pieces are not new. What SCS adds is knowing which one to reach for, in order:

1. **Nouns** — what exists? A per-instance fact is a component; a run-wide fact is a resource.
2. **Content** — which of those nouns are actually data (a monster's stats, an item's effects)? A table row, not a struct per kind.
3. **Rules** — what has to happen, triggered by what? Each becomes a system: reads this, writes that.
4. **Order** — what precedes what? An explicit position in the schedule, never implicit call order.
5. **Invariants** — what does the system assume is already true when it starts? State it, even in a comment — the most-skipped step, and the most common source of a bug nobody can reproduce.
6. **Render boundary** — simulation state is truth; derived display state is disposable. Never let the renderer be the only place a fact lives.

The real worked example is already in this repo, not invented for this page: `README.md`'s "there is no pink dragon in nihilurk" walk-through is the whole checklist run once. A dragon that is fire-immune is a fact about one entity (`FireImmune`, already a component nothing new had to be written for); its stats are content (one row in `BESTIARY`); nothing new needed a rule, an order, or an invariant, because a dragon answers questions the engine already asks. `cargo build`, and it is real. If a new feature ever needs a branch in combat, in movement, *and* in render, that is the tell — it was not decomposed into the six steps above, it was patched in sideways.

**Smells**, in the same spirit as `docs/explanation/code-calisthenics.md`'s for code shape:

- A system taking `&mut World` where you cannot say in one sentence what it is allowed to touch.
- A new Rust type where a data row would have done.
- Turn order known only by call order, written down nowhere.
- Game state that exists only inside the renderer.
- One feature touching three unrelated systems — usually a sign a step above was skipped, not a sign SCS does not fit.

---

## 7. Thesis

SCS is not a compromise, and it is not a fallback. It is a philosophy of simulation: a roguelike world modelled as a live, ordered, stateful system where components describe facts, systems enact rules, and the turn schedule is a first-class part of the rules rather than plumbing underneath them. A dungeon is a causal machine, not a clean abstraction, and the architecture is defined by the presence of that intentional complexity — turn order, consequence chains, structural mutation — handled in a disciplined way, not by its absence.

It preserves the performance and expressiveness of imperative mutation while trying to formalize the boundary between simulation logic and implicit convention. The world is the thing being simulated; the system is the thing doing the simulating; the architecture should be honest about both, and two rounds of actually checking it now stand behind that sentence rather than in front of it.

It is feral where it needs to be, disciplined where it matters, efficient where it must be, sharp where it should be. That is why nihilurk is nihilurk, and why SCS is the right name for it.

---

## Manifesto

We do not build the world as a framework. We build the world as a simulation.

We do not ask the game to be polite about mutation. We ask it to be honest about consequences.

We do not worship generic abstraction. We worship causal clarity.

We do not treat the turn schedule as a convenience. We treat it as law.

We do not hide complexity behind a prettier shell. We make the complexity legible, keep it fast, and let the mechanics tell the truth.

That is SCS.
