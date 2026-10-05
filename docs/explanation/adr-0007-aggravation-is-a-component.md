ADR 0007: aggravation is a component laid over the tactic
=========================================================

    Status         Accepted
    Audience       Anyone touching `Aggravated`, or tempted to model a
                   monster's state as a `MovementType`.
    Supersedes     --
    Related        agents.md

A shriek makes every monster on the floor beeline for the noise. That state is the `Aggravated` component. It is not a `MovementType`.


Context
-------

Aggravation was once a `MovementType` variant carrying the tile of the noise. As a tactic it overwrote the tactic it was laid on, so an aggravated venus flytrap forgot it was a flytrap.


Decision
--------

**Make aggravation a component on top of the tactic, so the tactic is still there to go back to.**

  1. Out of the player's view, a monster carrying `Aggravated` takes one step toward the noise, never leashed, and waits once there. In view, it hands back to its rule set. An aggravated flytrap walks in from the next room, then lies in wait like a flytrap once you can see it.
  2. `MovementType::Aggravated { tx, ty }` stays in the enum, retired, because a save writes the enum by position. A save that carries it loads as `Chase` plus the component. It is never set.


Consequences
------------

Accepted costs:

  * **A dead variant.** Nothing can remove it without breaking old saves.

Benefits realised:

  * A tactic survives being aggravated and being un-aggravated.
  * Another state that should not replace a tactic has a precedent.


When to revisit
---------------

Reopen this decision if any of these becomes true:

  * **The save is versioned.** The retired variant could then be migrated away.
  * **A second state needs the same treatment.** Then the pattern wants a name of its own.


See also
--------

  agents.md                             the rule sets and what each tactic does
  ../reference/agents.md                the `Aggravated` rule, in the tables
  ../reference/components.md            the component and the retired variant
