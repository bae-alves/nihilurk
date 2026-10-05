ADR 0006: effects are saved by stable string id
===============================================

    Status         Accepted
    Audience       Anyone adding, retiring or renaming a row in `EFFECTS`.
    Supersedes     --
    Related        ../how-to/add-an-effect.md

The save file stores each effect as a stable string id, not a position. Enum variants elsewhere in the save are still stored by position, and are append-only.


Context
-------

Effects were once saved as positions in a `u64` bitset. That capped the list at 64, and it made retiring a row dangerous: removing one shifted every later effect in every save, and nothing could tell, because every index was still a valid index.


Decision
--------

**Pair each effect with a string id in `EFFECTS`, and save the id.**

  1. Rows may be reordered and retired freely. The one rule is: never rename an id. A bestiary row lives by the same rule.
  2. An id a save names and the build has no row for is dropped, counted, and reported to the player in one line at the end of the load. Losing one property beats losing the run.
  3. There is no ceiling on how many effects there can be.


Consequences
------------

Accepted costs:

  * **A rename is a save break.** An effect renamed in code but not in its id is fine; an id renamed breaks every save that holds it.
  * **A missing row half-works.** An effect missing from `EFFECTS` attaches but is not saved, so it vanishes on reload.

Benefits realised:

  * Retiring a row changes nothing about the others.
  * A save from a build with an effect the current build lacks still loads.


When to revisit
---------------

Reopen this decision if any of these becomes true:

  * **The save is versioned.** A migration step could carry renames, and the never-rename rule could relax.
  * **Ids become a maintenance burden.** If keeping them stable costs more than losing a property, the policy of dropping unknown ids is the thing to revisit.


See also
--------

  ../how-to/add-an-effect.md            adding a row, and the warning about ids
  ../reference/content-tables.md        the `EFFECTS` table
