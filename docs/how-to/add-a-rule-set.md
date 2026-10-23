How to add a rule set
=====================

    Audience       Engine developer.
    Prerequisites  `../explanation/agents.md`. For a rule that does not
                   exist yet, `add-a-rule.md` first.
    Result         A new way for a creature to think, picked by its
                   tactic.

A rule set is an ordered list of rules. The first one that fires takes the turn.


Step 1: write the set
---------------------

In `models/src/mind/agents.rs`, with the other sets:

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

A set is picked from the monster's `MovementType` in `rule_set_for`. Add a variant at the **end** of the enum in `models/src/meta/components/` (a save writes it by position), with a `///` line: `models` warns on any public item without one (`missing_docs`). Then one arm:

    MovementType::Skirmish => &SKIRMISHER,

A test pins every saved enum's variant order, so `saved_enums_keep_their_variant_order` fails until you record the new variant. Appending at the end is safe; regenerate the golden file:

    NIHILURK_REGEN_ENUMS=1 cargo test -p nihilurk-models --test it saved_enums::

`rule_set_for` is the one exhaustive `match` on `MovementType`, so the compiler stops there and nowhere else. The other places that read the tactic use `matches!` and treat the new one as none of theirs: the renderer's staggering tint (`Confused`, `Flee`) and the garrote's helpless-target check in `combat/`. Read both and decide whether the new tactic belongs.


Step 3: use it
--------------

Put the variant on a bestiary row's `movement` column (`add-a-monster.md`).


Step 4: test it
---------------

A unit test in `agents.rs` per rule the set lists, built on `percept(...)`: one where the rule should win and the ones before it should pass.


Verify
------

    cargo test -p nihilurk-models --lib agents
    NIHILURK_SPAWN="your monster" cargo run -p nihilurk

Then add the set's row to the sets table in `../explanation/agents.md`.


Appendix: quick check
---------------------

1. Write `pub static <NAME>: RuleSet` in `models/src/mind/agents.rs`; the order of `rules` is the behaviour.
2. Set `leashed: true` only for a set that hunts the player.
3. Append a documented `MovementType` variant at the end in `models/src/meta/components/`, add its arm to `rule_set_for`, and regenerate `models/tests/golden/saved_enums.txt`.
4. Check the `matches!` readers of `MovementType` (the renderer's tint, the garrote) and add the new tactic where it belongs.
5. Put the variant in the `movement` column of a bestiary row.
6. Test each rule the set lists: one case where it wins, one where an earlier rule passes.
7. Run `cargo test -p nihilurk-models --lib agents` and `NIHILURK_SPAWN="<monster>" cargo run -p nihilurk`.
8. Add the set's row to the sets table in `../explanation/agents.md`.


See also
--------

  add-a-rule.md                     when no rule does what you need
  ../explanation/agents.md            every set, and what picks it
