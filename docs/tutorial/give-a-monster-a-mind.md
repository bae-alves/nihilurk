Tutorial: give a monster a mind
===============================

    Audience       Anyone who has added a monster and now wants it to
                   behave differently, not just hit harder.
    Prerequisites  `add-your-first-monster.md`, or the same comfort with
                   the bestiary.
    Time           About twenty minutes.
    You will       Write one rule and one rule set, give them to the
                   emu, test them on a percept you build by hand, and
                   watch an emu run from you.

This is a lesson, not a recipe. When you want the short versions, read `../how-to/add-a-rule.md` and `../how-to/add-a-rule-set.md`.


What you are about to learn
---------------------------

A monster does not have behaviour code. Each turn it is handed a `Percept` (what is true right now, around it) and a `RuleSet` (an ordered list of rules). The first rule that fires decides the turn. That is all a mind is here. You will write a coward: an emu that fights anything next to it, but bolts the moment you come within three tiles, and otherwise stands still.


Step 1: look at a mind that already exists
------------------------------------------

Open `models/src/agents.rs` and find `CHASER`:

    pub static CHASER: RuleSet = RuleSet {
        name: "chaser",
        leashed: true,
        rules: &[CAST, SHOOT, STRIKE, HUNT],
    };

Read it as a sentence: cast a spell if it can, else shoot if it can, else strike what is next to it, else walk at the player. Every emu on the floor thinks exactly that today.


Step 2: write the rule
----------------------

There is no rule for "run when close", so write one. Next to `flee`, add:

    fn bolt_when_close(p: &Percept) -> Option<Action> {
        let player = p.foes.iter().find(|f| f.is_player)?;
        if chebyshev(p.at, player.at) > 3 {
            return None;
        }
        let (dx, dy) = toward(p.at, player.at);
        Some(Action::Step(-dx, -dy))
    }

    /// A noticed player within three tiles: step straight away from them.
    pub const BOLT_WHEN_CLOSE: Rule = Rule {
        name: "bolt when close",
        fire: bolt_when_close,
    };

Two things to notice. The rule reads only `p`: it never touches the world. And `p.foes` holds the player only once the emu has noticed them, so a player with a ring of stealth walks up to a coward unchased.


Step 3: write the set
---------------------

With the other sets:

    /// Bites what is next to it, bolts from a player who comes close,
    /// and otherwise stands its ground.
    pub static COWARD: RuleSet = RuleSet {
        name: "coward",
        leashed: false,
        rules: &[STRIKE, BOLT_WHEN_CLOSE],
    };

The order is the personality. `STRIKE` first means a cornered coward still bites. Swap the two and it would rather run than bite, even with you standing on its toes.


Step 4: test it before you play it
----------------------------------

In the `tests` module at the bottom of `agents.rs`:

    #[test]
    fn a_coward_bolts_then_bites_when_cornered() {
        let map = room();
        let e = entities(1);
        let mut p = percept(&map, (8, 6), (10, 6));
        p.foes = vec![sighting(e[0], (10, 6), true)];
        assert_eq!(think(&p, &COWARD), Action::Step(-1, 0));
        p.foes = vec![sighting(e[0], (9, 6), true)];
        assert_eq!(think(&p, &COWARD), Action::Strike(e[0]));
    }

    cargo test -p nihilurk-models --lib coward

No world was built. That is the point of a mind made of plain functions: the test is three lines of percept.


Step 5: give the emu the mind
--------------------------------

A set is picked by the monster's tactic. Add a variant at the **end** of `MovementType` in `models/src/components.rs` (a save writes it by position):

    Cower,

Then build. The compiler lists every `match` that needs to hear about it. The one that matters is `rule_set_for` in `agents.rs`:

    MovementType::Cower => &COWARD,

In `models/src/monsters.rs`, change the emu's movement from `Chase` to `Cower`, and add `Cower` to the `use MovementType::{...}` line at the top.


Step 6: meet it
---------------

    NIHILURK_SPAWN="emu" cargo run -p nihilurk

Walk toward it. At three tiles it runs. Corner it and it bites. Step back past three and it stops and watches. Walk out of the room: it does nothing at all, because a mob off your view never acts.


Step 7: keep it, or put it back
-------------------------------

    git checkout models/src

If you keep it, add the rule and the set to `../reference/agents.md`.


See also
--------

  ../reference/agents.md          every rule, every set
  ../explanation/agents.md        why a mind is a list
