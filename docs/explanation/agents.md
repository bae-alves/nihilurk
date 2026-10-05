Why monsters think in rule sets
===============================

    Audience       Anyone deciding how a creature should behave, or
                   wondering why `ai.rs` makes no decisions of its own.
    Prerequisites  `../reference/agents.md` for the names.
    This is        Reasoning, not instructions.

Every mob in nihilurk is a reflex agent. Each turn it is handed what is true right now, it picks one thing to do from an ordered list of rules, and it forgets all of it. There is no memory, no plan, no state machine to be in the middle of. This page is why.


No memory
---------

A monster that remembers has somewhere for a bug to live. "It saw you two rooms ago and is still coming" needs a place to keep the sighting, a rule for when to drop it, and a save field for it, and every one of those is a way for a monster to act on something the player can no longer see or check. A turn built from a fresh percept can only ever act on the floor as it is.

It also makes the rules testable without a world. A rule is a plain function of the `Percept`, so a test builds one by hand, runs `think`, and reads the answer. Nothing has to be spawned, walked into place or aged. The randomness a rule needs (which spell to try, which way to stagger) is rolled into the percept for the same reason: the rule stays a function of what it is handed.

The one thing that looks like memory is `Aggravated`, and it is not the monster's. It is a fact laid on the monster from outside, by a scroll or a ring, the way `Asleep` is.


The player's view is the gate
-----------------------------

A mob on a tile the player cannot see does nothing. Sight in nihilurk is symmetrical (the player sees the room, the room sees the player), so a monster out of view has, as far as the game is concerned, nothing to perceive. The floor stays quiet out of sight, and nothing happens off-screen that the player could not have watched.

Two things break that on purpose. An aggravated monster heard a shriek, and a shriek carries through walls, so it walks toward the noise. The player's Helper walks back to the player, because a companion stranded around a corner is no companion. Both are single moves with no percept behind them, and both hand back to their rule set the moment they are in view: an aggravated venus flytrap walks in from the next room, then lies in wait like a flytrap once you can see it.

That is also why aggravation is a component. As a `MovementType` it would overwrite the tactic it was laid on, and an aggravated flytrap would forget it was a flytrap. As a state on top of the tactic, the tactic is still there to go back to.


First match wins
----------------

A rule set is an ordered list, and the first rule that fires takes the turn. Order is the whole design: `CAST` before `SHOOT` before `STRIKE` before `HUNT` says "magic if you can, a bow if you can, otherwise go to melee", and that sentence is the set. There are no weights to tune and no scores to compare, so reading a set tells you exactly what the creature does, and moving one line changes it.

The chaser's loop is deliberately dumb about its spells: it picks one at random and fires it if it can, and if it cannot it does not try another. A dragon with one spell breathes every time it has a line. A creature with three spells is unpredictable in the way a player can plan around, rather than always casting its best one.


Allies look before they cast
----------------------------

A wild monster does not care whom its fireball catches, its kin included. An ally does: every spell an ally casts is checked against the player's tile first, and a foe standing too close to the player is passed over for one further off, or for a bite. That check lives in the rule (`CAST` asks `ally`), not in the spell, because the spell is the same mechanic whoever casts it. What differs is whose side the caster is on.

The check knows each spell's footprint. A spell it has no footprint for is assumed to reach the player, so an ally never casts one blind. It does not follow chain reactions: a fireball that sets off a trap or a coin can still burst onto the player.


What `ai.rs` keeps
------------------

The clock (energy, rounds, the two passes a fast monster gets), the gates (dead, asleep, stone) and the hands (carrying an `Action` into the world). It never decides anything. A new behaviour is a new rule or a new set in `agents.rs`, and `ai.rs` does not change.


See also
--------

  ../reference/agents.md            every rule and set
  data-driven-content.md            the same idea for content tables
  adr-0007-aggravation-is-a-component.md  why aggravation sits on top of the tactic
