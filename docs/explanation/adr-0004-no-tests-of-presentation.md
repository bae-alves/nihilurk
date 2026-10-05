ADR 0004: presentation is checked by playing, not by `cargo test`
=================================================================

    Status         Accepted
    Audience       Anyone about to add a test of how something looks,
                   or restore one.
    Supersedes     --
    Related        the-feel-layer.md

Nothing in the feel layer, the renderer or the HUD has a test of how it looks. Game rules keep their tests. The rule is: **assert game rules, not their presentation.**


Context
-------

The feel layer once had a lot of coverage: which shake a blow earned, how a shake decayed and what outranked what, the corpse-fling and bone scatter, mote birth and culling, the map-cell clipping a shake pushes tiles through, the log line's colours and the pride stripes, and an end-to-end test that rendered a frame and asserted glyph rows. It came to just over seventeen hundred lines.

All of it shared two problems.

**It asserted the wrong thing.** A shake is punctuation: what matters is whether it reads as heavier than the one below it, and no assertion can tell you that. A test can say `ShakeKind::Heavy` lasts 260 ms, a number you can also read off `ShakeKind::shape` in less time than the test takes to compile.

**It broke for reasons that were never bugs.** Presentation tests are coupled to presentation, so moving a layer, renaming a badge or reordering the HUD fields turned them red without anything being wrong. The end-to-end render test was the worst: its fixture hand-mirrored twenty-two of `main`'s startup resources, with a comment admitting as much, so it drifted every time startup changed and said nothing when it did.


Decision
--------

**Delete the presentation tests, all in one pass, and keep the rule.**

A test earns its place if it pins something a player could be cheated by: damage arithmetic, what a seed generates, what survives a save, what a menu offers. If the worst case of it being wrong is "that looked a bit off", it belongs in a playtest.

Three things keep tests, because they are not about feel:

  1. **The `--MORE--` gate.** `log_panel` holds the prompt back until the effect layer is empty, because `play_particles` reads any keypress as "skip". A trick shot's blast is queued behind the missile's flight, so the key would eat the explosion and leave the flight looking fine. The player is cheated of the shot they lined up, and the frame it happens in looks correct, which is the one case playing does not catch. Two tests in `view.rs` pin it, on a fixture of two resources.
  2. **`particle-core`.** It is `no_std` and compiles for a microcontroller, and `f32::ceil` has two implementations. If they disagree, the bare-metal build compiles arithmetic the game does not run. `tests/parity.rs` runs first in `nostd_check.sh`.
  3. **`models/tests/determinism.rs`.** It proves cosmetic randomness comes off `FxRng` and never perturbs `GameRng`.


Consequences
------------

Accepted costs:

  * **The draw order in `render` is guarded by nobody.** A later layer silently covers an earlier one, and only a look at the terminal, in more than one terminal if you can, catches it.
  * **A regression in a shake or a colour ships unless someone sees it.**

Benefits realised:

  * Moving a layer, renaming a badge or reordering HUD fields turns nothing red.
  * No fixture mirrors `main`'s startup.


When to revisit
---------------

Reopen this decision if any of these becomes true:

  * **Presentation carries a rule.** If a colour or a glyph becomes the only way a player can tell something that changes their choice, it is a game rule and gets a test.
  * **Playtesting stops catching things.** If layers regress and nobody sees it for releases, the cost above has come due.


See also
--------

  the-feel-layer.md                    the layer this covers
  ../reference/rendering.md            the frame, and the order it is drawn in
  cross-platform-testing.md            why `particle-core` stays tested
