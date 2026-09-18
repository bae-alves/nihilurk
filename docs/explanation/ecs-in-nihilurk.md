The ECS in nihilurk
===============

    Audience       Anyone who has opened `models/src/` expecting bevy
                   and found something that looks like it half-uses it.
    Prerequisites  You know what bevy_ecs is for. You do not need to
                   have used it.
    This is        Understanding: what nihilurk asks of components,
                   resources and systems, and why fifteen of its
                   sixteen schedule steps take `&mut World` instead of
                   a `Query`. The recipes are
                   `../how-to/work-with-the-ecs.md`.

nihilurk uses `bevy_ecs` as a **world**, not as a framework. There is no `App`, no plugin, no `SystemSet`, no change detection, no events in the bevy sense. There is one `World`, one `Schedule` of sixteen steps run once per player turn, and a main loop that owns the terminal.

That is a smaller slice of bevy than most projects take, and the parts left on the shelf were left there on purpose.


The three nouns, and what each is allowed to be
-----------------------------------------------

### Components are pure data

A component says *what a thing is*. It never says what happens as a result. `models/src/components.rs` is one file of nouns — every `Component`, `Resource` and `Event` the game is made of, and nothing that reaches into the `World`. What methods it does carry are about the type itself: `Name::article`, `SpeedKind::faster`, `GameLog::add`. Two shapes recur:

  * **Markers** carry nothing. `Player`, `Blood`, `Curse`, `Confused`. Their presence *is* the fact; a system asks `world.get::<Blood>(e).is_some()` and that is the whole check.
  * **Type keys** carry one enum naming which one it is — `Potion` → `PotionEffect`. The key is identity for the save file and for identification. It is **never** a description of behaviour: what a potion does is a `match` in `items/potions.rs`, and what it *is made of* is the catalog row.

The rule this buys is the one the whole codebase rests on: **a property is a component, never an enum a subsystem has to recognise.** A dragon born fire-immune and a player wearing a ring of fire resistance both carry `FireImmune`, so the wand-of-fire code asks one question and never learns that rings exist. Adding a ring of fire resistance is a table row and no behaviour code at all.

The counterpart is that behaviour lives in tables of *data about behaviour* rather than in branches: `PASSIVE_ABILITIES` pairs a `Grant` with a `fn`, `ON_HIT_ABILITIES` does the same for blows that land, and `combat::resolve_attack` fires the second without ever learning what is in it. See `data-driven-content.md`.

### Resources are the run's global objects

Everything singular about a run: the `Map`, the `GameRng`, the `Depth`, the `GameLog`, the four intent queues, the UI's modal flags. If there is exactly one of a thing and it is not attached to an entity, it is a resource.

Two habits keep them from becoming a junk drawer:

  * **A resource with real logic lives with that logic**, not in `components.rs`. `Shake`, `AutoExplore`, `FastMove`, `PackIsOpen` and `MagicMapReveal` are declared in the modules that own them and re-exported, so a call site never has to know which file they came from.
  * **Cosmetic resources are optional.** `Particles`, `Shake`, `ScoreFlash` and `FxRng` are reached through `get_resource_mut`, never `resource_mut`, because every test builds a bare world without them. Gameplay code arms a flourish and forgets; it must never *require* one.

### Systems own behaviour in one domain

The schedule is sixteen steps, and each has one job. This diagram is a summary, not the list — three steps (`reveal_mimics`, `monster_pickup_system`, `move_system`) are left out because they add nothing to the point this diagram is making about tail position; the full sixteen, in order, are `../reference/input-and-turn-loop.md`:

```mermaid
%%{init: {'theme':'base','themeVariables':{
  'primaryColor':'#20242b','primaryTextColor':'#d7dae0',
  'primaryBorderColor':'#5c6370','lineColor':'#8a8f98',
  'fontFamily':'ui-monospace, SFMono-Regular, Menlo, monospace',
  'fontSize':'13px'}}}%%
flowchart LR
  SM["smoke"]:::magic --> SN["snare"]:::peril --> AI["ai"]:::peril
  AI --> TR["trap"]:::peril --> TH["throw"]:::magic --> IT["item"]:::magic
  IT --> EQ["equipment<br/>effects"]:::magic --> CO["combat"]:::peril
  CO --> RE["reaper"]:::peril --> DL["dungeon<br/>lord"]:::peril
  DL --> PA["passive<br/>abilities"]:::magic --> VI["visibility"]:::cold
  VI --> SC["score"]:::hero
  classDef hero fill:#3a3418,stroke:#d7ba4a,color:#e8dfa8
  classDef peril fill:#3a1f1f,stroke:#c05050,color:#f0c8c8
  classDef magic fill:#2f2038,stroke:#a86fc0,color:#e6cdf0
  classDef cold  fill:#17323a,stroke:#4aa3c0,color:#bfe4f0
```

What each step does, and every `.after()` edge that holds them in that order, is `../reference/input-and-turn-loop.md` — this page is about *why the shape is a shape*, and repeating the list here would be one more copy to keep true.

The two tail positions are what the diagram is for. `passive_ability_system` rolls *after* the monsters move so a ring of teleportation's jump lands at the top of the player's next turn — they see the new tile and act from it before anything on the floor moves again, which is the difference between a ring you might keep on and a curse. `score_turn_system` is dead last so a combo multiplier is applied once the dying is over, never to a number still growing.

Behaviour that does *not* belong to a schedule step belongs to a module verb called from one — `conditions::confuse`, `equipment::toggle_equipped`, `items::pick_up`. The test for whether something is in the right place is whether you can name the domain it owns in three words.


Why almost everything is an exclusive system
--------------------------------------------

Fifteen of the sixteen take `&mut World`. Exactly one — `visibility_system` — is written the bevy way, with `Query`, `Res` and `Commands`.

That split is not laziness, and it is not a migration half-finished. It falls out of what roguelike mechanics actually do.

**A `Query` describes a fixed set of component accesses up front.** That is what makes it safe to parallelise and what makes the filters self-documenting. It also means the system has declared, before it runs, every component it will touch.

nihilurk's mechanics cannot make that declaration. Resolving one thrown wand of polymorph:

  * blows a disc open and damages everything standing in it,
  * **despawns** each creature it killed,
  * **spawns** a fresh species in place of each one it transformed,
  * sets off every trap and every coin the disc covered, which may teleport a fourth creature, drop a fifth through a trapdoor, and heal the thrower,
  * and logs a dozen lines, rolls the shared RNG, and queues particles.

There is no honest tuple of component accesses for that. It touches the archetype graph structurally, mid-resolution, in a way that depends on what the previous step found. Written as a `Query` system it would be a `Query` plus `Commands` plus six `ResMut`s plus a deferred-command dance to see your own writes — which is `&mut World` with extra steps and a worse error message.

So nihilurk takes `&mut World` where mechanics are, keeps `Query` where a system genuinely only reads and tags, and pays two prices for it:

  * **The borrow checker is stricter, not looser.** Holding the world means every `world.get::<C>(e)` conflicts with every `world.resource_mut()`. Five patterns get past that and they are written down in `../how-to/work-with-the-ecs.md`; they are worth learning once, because they are the whole idiom.
  * **No parallelism.** Nothing here needs it. A turn is a sequence by nature — the player moves, then every monster, then the consequences — and there is nothing in it to run at the same time as anything else.

What nihilurk gets back is that a mechanic reads as a procedure. `resolve_use` works out what the item is, decides where it physically ends up, logs the beat and applies the effect, top to bottom, in one function you can read in one sitting.

**If your new system only reads and tags, write it as a `Query`.** The narrow filter is genuinely better documentation than a comment, and `visibility_system` is the worked example.


Narrow what you fetch
---------------------

Exclusive systems make it easy to reach for the whole world, so the discipline has to be deliberate. Three rules:

  * **Ask for actors when you mean actors.** `passive_ability_system` used to probe every entity in the world for `Regenerates`. It walks `Or<(With<Player>, With<Mob>)>` now, because an effect never lands on an item — a ring carries `Grants`, and it is the *wearer* who ends up with the marker.
  * **Fetch nothing the body does not read.** A tuple that grows past what a system uses is the first sign it is doing two jobs. `visibility_system`'s `spot_query` used to carry `Option<&Potion>`/`Scroll`/`Wand`/`Ring` fields purely to feed `identify::named_display`'s old cosmetic-appearance branches; once those were gone, so were the fields nobody else read.
  * **Collect ids, not references.** The point of collecting before a mutating loop is to stop borrowing; a `Vec<&Position>` has not stopped.

One place still scans the whole world on purpose, and it is worth knowing why. `equipment::equipped` answers "what is this creature wearing?" by walking every entity and asking whose `Equipped.by` points at them. Gear points at its wearer rather than the other way round — which is right, because the item system lifts an item out of the pack while it resolves a use, and a ring must not stop working for those few lines, and a monster that caught a thrown dagger has no pack to look in at all. The narrow query that would answer it, `Query<&Equipped>`, needs `&mut World`, and every caller holds `&World` partway through reading something else. The scan is cheap on a floor of tens of entities and allocation-free; if a floor ever held thousands, the fix is an index resource, not a smaller loop.


What the world is *not* asked to remember
-----------------------------------------

Two whole categories of state are kept out of the ECS, and both for the same reason: they are cheaper to derive than to store.

**Terrain is a resource, not entities.** `Map` is one `Vec<TileType>` plus a bitset of which tiles are unlit. nihilurk never had one entity per tile, and a 1,760-entity floor with a `Renderable` each would cost more to iterate every frame than the whole rest of the world put together.

**Nothing cosmetic is saved.** Bloodstains, corpse marks, smoke, live particles, the shake, the scorekeeper's flash and `FxRng` are all rebuilt empty on load. The three map-sized overlays alone would come to 2.2 KB — more than the entire save file they would be joining — and none of it is gameplay. A reloaded floor is the floor you left, scrubbed of the mess you made on it. The map and the message log are out for the same reason; `saveload.rs` lists all four exclusions at the top.

The related rule is that **cosmetic randomness comes off `FxRng`, never `GameRng`.** A firework's colour drawn from the gameplay stream would mean every flourish quietly reshuffled the dice for everything after it, and `models/tests/determinism.rs` exists to catch exactly that class of mistake.


See also
--------

  ../how-to/work-with-the-ecs.md   the recipes, and the borrow patterns
  data-driven-content.md           why behaviour is a table, not a branch
  code-calisthenics.md             the shape this code is held to
  ../reference/components.md       every component, resource and event
  ../reference/input-and-turn-loop.md   the schedule and the main loop
