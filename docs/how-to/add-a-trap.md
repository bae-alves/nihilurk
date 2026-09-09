How to add a trap
=================

    Audience       Content author.
    Prerequisites  Comfortable adding a row. See
                   `../tutorial/add-your-first-item.md`.
    Result         A new trap the dungeon lays on floors, that reveals
                   itself in the usual three ways, and that survives a
                   save.

Three edits, and the compiler enforces the third.


Everything about traps is in one file:

    models/src/traps.rs


The recipe
----------

1. **Append a variant** to `TrapEffect`:

       pub enum TrapEffect {
           ...
           Dart,
           Pit,        // <- at the end; see the save-format warning
       }

2. **Add the row** to `TRAPS`:

       TrapDef { effect: TrapEffect::Pit, name: "pit trap",
                 glyph: '^', color: Color::Red, weight: 10, min_depth: 1 },

   The name is what the player sees once the trap is known -- there is no
   separate label function to update. `weight` is how often the dungeon
   lays this one against the others it could lay; `min_depth` is the
   first floor it appears on. All six existing traps are `10` and `1`,
   which is to say equally likely from the start.

3. **Write the mechanic** as an arm of `spring_trap`:

       TrapEffect::Pit => pit_effect(world, victim, is_player, seen),

   `spring_trap`'s match has no catch-all, so the compiler will refuse to
   build until you write this arm. That is deliberate: a trap that does
   nothing is not a trap.

Then:

    cargo build
    cargo test --test content
    ROOG_SPAWN="pit trap" cargo run -p engine


Writing the mechanic
--------------------

Look at what the existing six do, and reuse the pieces:

    trapdoor_effect       moves the victim to the next floor down
    snare_victim(...)     costs the victim N turns (bear trap, sleep gas)
    teleport_effect       relocates the victim on this floor
    arrow_effect          damage, and drops a real arrow on a miss
    dart_effect           damage, plus a permanent point of strength

Your arm receives:

    world       the ECS world
    victim      whoever stepped on it -- may be a monster, not the player
    is_player   whether it was
    seen        whether the player can see this happen
    trap_pos    where the trap is, if you need it

> **A trap fires for monsters too.** `victim` is whoever moved onto the
> tile. Write the log line through the `seen` / `is_player` pair the way
> the existing arms do, or the player will read second-person messages
> about a goblin.

> **Damage traps ignore the armour die but not the armour bonus.** If your
> trap deals damage, subtract `total_armor_plus(world, victim)` and
> nothing else -- see `arrow_effect`. Rolling a full opposed defence would
> make armour far better against traps than the design intends.

If your trap costs the victim turns, use `Snare`. It is aged down once
per turn by `snare_system`, the engine forfeits the player's input while
it is present, and the AI skips snared monsters. You do not have to
implement any of that; you attach the component.


Reveal styles
-------------

Every trap rolls one of three discovery styles at spawn, with equal odds,
and you do not choose it per row:

    Sight       shows itself as soon as its tile enters the viewshed
    Adjacent    stays hidden until the player is standing next to it
    Triggered   invisible until it goes off

roog has no "search" action, so these three are the only ways a trap ever
comes to light before it bites. If you want a trap that is always visible
or never visible, that is a change to `TrapBundle::random`, not to a row.


> **Append `TrapEffect` variants; never insert or reorder.** A saved trap
> stores its effect as a position in the enum. Inserting a variant in the
> middle turns every saved trapdoor into something else.


What you never have to do
-------------------------

  * Give the trap a label. `TrapEffect::label()` reads the row.
  * Add it to a list of trap kinds. `TrapDef::pick` walks the table.
  * Teach the save file about it, beyond the enum variant.
  * Place it. Floor generation decides how many traps and where; the
    table decides which.


See also
--------

  tune-rarity-and-depth.md         make a trap rare, or deep-only
  ../reference/content-tables.md   the TrapDef fields
  ../reference/spawn-api.md        TrapDef::pick, TrapBundle::from_def
