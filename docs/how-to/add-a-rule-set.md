How to add a rule set
=====================

    Audience       Engine developer.
    Prerequisites  `../reference/agents.md`. For a rule that does not
                   exist yet, `add-a-rule.md` first.
    Result         A new way for a creature to think, picked by its
                   tactic.

A rule set is an ordered list of rules. The first one that fires takes the turn.


Step 1: write the set
---------------------

In `models/src/agents.rs`, with the other sets:

    /// Keeps its distance and shoots; never closes in.
    pub static SKIRMISHER: RuleSet = RuleSet {
        name: "skirmisher",
        leashed: false,
        rules: &[SHOOT, KEEP_DISTANCE, STRIKE],
    };

Order is the behaviour. Write the set as a sentence first ("shoot if it can, otherwise back off, otherwise bite") and list the rules in that order.

`leashed: true` keeps it from stepping out of a room into a doorway or a corridor. Only a set that hunts the player wants it.


Step 2: give it a tactic
------------------------

A set is picked from the monster's `MovementType` in `rule_set_for`. Add a variant at the **end** of the enum in `models/src/components.rs` (a save writes it by position), then one arm:

    MovementType::Skirmish => &SKIRMISHER,

The compiler will not build until every `match` on `MovementType` has the new arm. Those are the places that still read the tactic directly (the renderer's tints, the garrote).


Step 3: use it
--------------

Put the variant on a bestiary row's `movement` column (`add-a-monster.md`).


Step 4: test it
---------------

A unit test in `agents.rs` per rule the set lists, built on `percept(...)`: one where the rule should win and the ones before it should pass.


Verify
------

    cargo test -p models --lib agents
    NIHILURK_SPAWN="your monster" cargo run -p engine

Then add the set's row to the sets table in `../reference/agents.md`.


See also
--------

  add-a-rule.md                     when no rule does what you need
  ../reference/agents.md            every set, and what picks it
  ../tutorial/give-a-monster-a-mind.md   the same thing, taught slowly
