# SCS: The Simulationist Component System

## Executive summary

SCS is roog's architecture in one sentence: a simulation-first, component-backed, system-driven world model whose rules are executed in an ordered turn schedule and whose state changes are intentionally direct where the game needs them to be.

This is not a generic ECS framework. It is not a purity test. It is a roguelike architecture built to preserve causality, performance, and game meaning.

---

## 1. Definition

SCS, or the Simulationist Component System, is the architectural model used by roog.

It is a simulation-first architecture in which world state is represented as component-backed entities and resources, behavior is expressed as ordered systems, and the game loop advances through a deterministic turn schedule instead of a general-purpose framework lifecycle.

This is not a generic ECS app. It is not a plugin-based engine architecture. It is not a purity model. It is a game architecture shaped by the fact that a roguelike is a simulation with causal order, structural mutation, and explicit turn semantics.

At its core, SCS says:

- state is represented as components and resources
- behavior is expressed as systems
- the world advances in explicit stages
- mutation is allowed where the mechanics require it
- the architecture is optimized for clarity, determinism, and performance
- the framework is a tool, the simulation is the truth

---

## 2. Why SCS exists

A roguelike is not a generic app. It is a live world with ordered consequences.

The game world is not a static set of UI widgets or services reacting to framework events. It is a state machine in which:

- actors move
- terrain changes
- effects cascade
- entities are spawned and despawned
- resources are mutated
- the schedule matters
- order changes outcomes
- the rules are their own source of truth

A framework that optimizes for clean lifecycle boundaries is a good fit for many app domains. It is a poor fit for a game in which the mechanics are:

- stateful
- order-sensitive
- structural
- turn-based
- highly causal
- performance-sensitive

SCS exists because roog needs to model the dungeon as a simulation, not as a framework-shaped abstraction pretending to be one.

---

## 3. The three: what is strong, weak, and ugly

### The Strong

The strongest thing about this architecture is that it has a coherent identity.

- The world is the primary object of computation.
- Components are the vocabulary of state.
- Systems are the verbs of behavior.
- The turn schedule is part of the rules, not merely an implementation detail.
- Content is data-driven and declarative.
- The codebase remains compact because it is expressive, not because it is hiding complexity.

This gives the project a very real strength: it is easy to tell what the game is doing and why.

Parts of roog are deliberately built around direct mutation because the mechanics demand it. The project’s docs are explicit about this, and that is a sign of health. The architecture is not trying to be an idealized ECS in the abstract. It is trying to be honest about the actual problems the game solves.

The architecture also has real extensibility. New monsters, items, traps, and effects are added as data rows, not through a giant list of hardcoded branches. That is a major engineering win.

### The Weak

The weak point is not the imperative mutation model. The weak point is the boundary clarity.

The architecture is powerful, but some of the contracts are still implicit.

That means:

- data-driven content is extremely expressive but sometimes under-specified
- systems rely on conventions rather than explicit interfaces
- some effect chains are harder to reason about than they ought to be
- mutation-heavy systems can hide assumptions inside the turn order
- the borrow checker pain is real, but it is not the true problem; the true problem is that some seams are not formal enough yet

This is the main design pressure in roog: the architecture is good, but the hidden contracts need to be made more legible.

### The Ugly

The ugly part is not that roog is low-level or imperfect. The ugly part is that it is a little feral.

It is feral in the good sense: direct, compact, and fast.

It is ugly in the sense that it is not a polished, framework-shaped abstraction. Some of the movement between:

- content definitions
- simulation logic
- structural mutation
- effect resolution
- world-state invariants

still feels a little too magical to the outside observer.

That is the real weakness in the human sense: the architecture is disciplined, but not yet as explicit as it could be at the seams. The code is not sloppy. The design is not broken. It is just not always formal enough for someone who is trying to understand it without living inside the docs for a month.

---

## 4. The SCS model in plain language

SCS says the world is not an object graph. It is a simulation state.

The game world is a living place in which actors have nouns, rules have verbs, and the turn schedule is sacred.

That means:

- components are the nouns
- resources are the globals
- systems are the verbs
- the schedule is the law
- effects are the consequences
- content is the data that feeds the law

This is why roog can treat its content like a table and not a hierarchy. A row is a thing, and the simulation interprets the row according to generic rules.

This is also why roog can be fast without pretending the world is a nice, static object model. It is not. It is a moving, mutating thing with lots of causal edges.

---

## 5. Why direct world mutation is correct here

A common criticism of imperative ECS is that it “escapes the model.” In roog, that criticism is often wrong.

A turn can involve:

- damage to multiple actors
- despawning corpses and enemies
- spawning replacements
- triggering traps
- moving items
- altering resources
- affecting global score or visibility state
- queuing particles and logs
- changing what can be seen next turn

This is not a static query problem. It is a simulation problem.

A query system is appropriate when the system can honestly state the components it reads and writes before the fact. Many roguelike mechanics cannot. They are dynamic, structural, and stateful in a way that makes a static query insufficient or misleading.

So in SCS, `&mut World` is not a cop-out. It is often the correct expression of the game’s rules.

---

## 6. Why Rust borrow-checker pain is not the enemy

Rust is not a bad fit for this architecture; it is the reason the architecture is disciplined.

All large Rust codebases have borrow-checker complexity. The real question is not whether you have pain, but whether the pain is being channeled into clear patterns.

In roog, the pain is handled by explicit conventions. The docs are not decoration; they are the architecture’s way of saying:

- what belongs where
- how systems read and write the world
- how to avoid the common mistakes
- what the system-level contracts are

This is not a weakness. It is how a Rust simulation architecture remains maintainable.

The project is not trying to be a casual dynamic-language game. It is trying to be an engine-like simulation in a language that punishes lazy design. That is a strength.

---

## 7. Why the architecture is feral and why that is good

The project is feral in the sense that it does not pretend to be a polished generic framework.

It is direct.
It is compact.
It is opinionated.
It is authentic to the domain.
It makes a conscious choice to optimize for expressiveness and performance rather than for abstraction for abstraction’s sake.

That is the right call for a roguelike.

When the game is a simulation of a dungeon, a good architecture is one that lets the code read like the mechanics. A feral architecture can do that better than a generic one.

The goal is not maximum abstraction. The goal is maximum clarity of the actual intended behavior.

---

## 8. The problem with hidden magic

The real thing to fix is not the direct mutation model; it is the hidden magic.

The project has a big and valuable quality: it is compact and elegant because it relies on conventions and data-driven design. But conventions can become magic when they are not written down or validated.

The hidden magic lives in places like:

- data rows that encode behavior implicitly
- effects whose meaning is spread across tables and systems
- content definitions that rely on successful assumptions
- turn-order invariants that are known but not always stated plainly
- world mutation that is efficient but not well-separated at the boundary

This is not a reason to throw away the design. It is a reason to formalize the contracts.

The plan is not “make the architecture generic.” The plan is “make the implicit explicit.”

---

## 9. The SCS improvement path

SCS should evolve in a way that keeps its core intact:

- simulation-first
- component-backed
- system-driven
- direct mutation where necessary
- data-driven content
- explicit turn sequencing

But it should add formal boundaries around its weakest points.

### Recommended improvements

1. Command and effect queues
   - Systems should decide outcomes explicitly.
   - Mutations should be staged through command or effect queues where appropriate.
   - This reduces hidden side effects and makes debugging easier.

2. System invariants
   - Every system should state what it assumes about the world before it runs.
   - This turns implicit contracts into actual rules.

3. Content validation
   - Rows should be validated structurally.
   - Required data, legal combinations, and impossible states should be checked.

4. Mutation boundaries
   - Structural mutation should be isolated from rule evaluation where possible.
   - The world should change in explicit phases, not as a side effect of uncertain logic.

5. Relationship indexes
   - Expensive world-wide scans should be replaced with indexed resources when the system genuinely needs them.
   - This keeps the simulation fast without letting the architecture rot into a global scan swamp.

6. Better effect resolution separation
   - Effects should be explicit and inspectable as a phase.
   - This is especially important for roguelike chains.

7. Clearer docs at the seam
   - The docs are doing the right thing already.
   - They should keep formalizing the architecture, especially around system contracts and side effects.

---

## 10. The SCS thesis statement

SCS is not a compromise. It is a philosophy of simulation.

It says that a roguelike world should be modeled as a live, ordered, stateful system where components describe facts, systems enact rules, and the turn schedule is a first-class part of the simulation.

It is a direct architecture for direct mechanics.

It preserves the performance and expressiveness of imperative mutation while trying to formalize the boundary between simulation logic and implicit conventions.

In other words:

- the world is the thing being simulated
- the system is the thing doing the simulating
- the architecture should be honest about both

That is SCS.

---

## 11. The formal design thesis

A god is not a clean abstraction; a dungeon is a causal machine.

The SCS architecture takes that seriously. It treats the world as a live simulation whose rules are executed in ordered stages, whose entities are assembled from data, whose systems resolve behavior, and whose mutating logic is allowed to be direct where the game demands directness.

The architecture is therefore not defined by the absence of complexity. It is defined by the presence of intentional complexity: the complexity of turn order, consequence chains, and structural mutation handled in a disciplined way.

This is why SCS feels feral in the right places. It is not trying to pretend the world is neat. It is trying to be honest enough that the world can remain fun, fast, and coherent.

---

## 12. Final verdict

SCS is not a fallback. It is a proper architecture.

It is the architecture of a game that understands that the world is not static, and that the game loop is not a decoration. It is the architecture of a game whose mechanics are the product of ordered state changes, data-driven content, and causal logic.

It is feral where it needs to be. It is disciplined where it matters. It is efficient where it must be. It is sharp where it should be.

That is why roog is roog.

And that is why SCS is the right name.

---

## Manifesto

We do not build the world as a framework. We build the world as a simulation.

We do not ask the game to be polite about mutation. We ask it to be honest about consequences.

We do not worship generic abstraction. We worship causal clarity.

We do not treat the turn schedule as a convenience. We treat it as law.

We do not hide complexity behind a prettier shell. We make the complexity legible, keep it fast, and let the mechanics tell the truth.

That is SCS.
