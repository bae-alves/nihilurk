How to add a rule
=================

    Audience       Engine developer.
    Prerequisites  `../reference/agents.md`.
    Result         A new reflex any rule set can list.

A rule is a condition on the percept and the action it calls for, in one function. It answers `None` when its condition does not hold.


Step 1: write the function
--------------------------

In `models/src/agents.rs`, next to the other rule functions:

    fn keep_distance(p: &Percept) -> Option<Action> {
        let near = p.foes.iter().find(|f| chebyshev(p.at, f.at) <= 2)?;
        let (dx, dy) = toward(p.at, near.at);
        Some(Action::Step(-dx, -dy))
    }

Read only the percept. If the rule needs something the percept does not carry, add a field to `Percept` and fill it in `ai::perceive`. Never reach into the world from a rule: that is what keeps every rule testable on a hand-built percept.

If the rule needs chance, read `p.roll`. Do not roll your own.


Step 2: name it
---------------

    /// A foe within two tiles: step straight away from the nearest.
    pub const KEEP_DISTANCE: Rule = Rule {
        name: "keep distance",
        fire: keep_distance,
    };

The doc comment is the rule's one-line "fires when": copy its shape from the others.


Step 3: list it in a set
------------------------

A rule does nothing until a set lists it. See `add-a-rule-set.md`.


Step 4: test it
---------------

In the `tests` module of `agents.rs`, build a percept with `percept(...)`, set the fields your rule reads, and assert what `think` returns for a set that lists it. Test both sides: once where it fires, once where it passes.


Verify
------

    cargo test -p nihilurk-models --lib agents

Then add the rule's row to the rules table in `../reference/agents.md`.


Appendix: quick check
---------------------

1. Write `fn <name>(p: &Percept) -> Option<Action>` in `models/src/agents.rs`; read only the percept, and take chance from `p.roll`.
2. Name it as a `pub const <NAME>: Rule`, with a one-line "fires when" doc comment.
3. List it in a rule set (`add-a-rule-set.md`).
4. Test both sides on a hand-built `percept(...)`: once where it fires, once where it passes.
5. Run `cargo test -p nihilurk-models --lib agents`.
6. Add the rule's row to the rules table in `../reference/agents.md`.


See also
--------

  add-a-rule-set.md                 putting the rule to work
  ../reference/agents.md            every rule that exists already
  ../explanation/agents.md          why a rule may only read the percept
