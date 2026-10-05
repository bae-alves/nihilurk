ADR 0005: a zap and a throw resolve before the mobs move
========================================================

    Status         Accepted
    Audience       Anyone reordering the turn schedule, or wondering why
                   `item_system` and `throw_system` sit ahead of `ai`.
    Supersedes     --
    Related        ../reference/input-and-turn-loop.md

`item_system` and `throw_system` run before `ai`. `throw_system` is not ordered after `trap_system`.


Context
-------

A zapped wand is aimed at the dungeon as it stood when the key was pressed. For a while the schedule resolved it after `ai`, so it read a tile the target had already walked off. That made the aiming reticle look broken.

The bolt wands hid it: a bolt sweeps a line and a blast a disc, so they still caught somebody. But every wand that reads one exact tile (teleport away, teleport to, polymorph, haste, slow, cancellation) reported finding nothing there while the reticle had been on the monster the whole time.

`throw_system` showed the same fault from the other side. Aimed at the empty tile in front of an approaching orc, the dagger arrived after the orc had stepped onto it and hit one it was never thrown at.

`throw_system` was also ordered after `trap_system`. Nothing needed that.


Decision
--------

**Order both before `ai`, and drop the edge after `trap_system`.**

  1. A shot that comes down on a trap sets the trap off through `detonate_at`, inside the throw's own resolution, so it never waits for the trap step.
  2. The two orderings are pinned by `a_zap_lands_on_the_tile_the_player_aimed_at_not_the_one_the_target_left` and `a_throw_lands_where_the_floor_was_when_the_player_let_go` in `update.rs`.
  3. The schedule is built by `turn_schedule()` rather than inline in `main`, so those tests can run it.


Consequences
------------

Accepted costs:

  * **A turn order with load-bearing edges.** Moving either system later breaks aiming, and only the two tests say so.

Benefits realised:

  * What the reticle sat on is what the zap or the throw hits.


When to revisit
---------------

Reopen this decision if any of these becomes true:

  * **Aiming becomes real-time.** If the target can move between the key and the resolution on purpose, the ordering is a design question.
  * **Another system needs the post-`ai` world.** Then it needs its own step, not a move of these two.


See also
--------

  ../reference/input-and-turn-loop.md   the schedule, step by step
  ../how-to/work-with-the-ecs.md        adding a step with an `.after()` edge
