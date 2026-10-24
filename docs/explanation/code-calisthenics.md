Code calisthenics in nihilurk
=========================

    Audience       Anyone changing engine or model code and wondering
                   why a function they were about to write an `if/else`
                   in has none anywhere near it.
    Prerequisites  You write Rust. You have a diff open.
    This is        Understanding and a house rule, not a tutorial. It
                   tells you what shape the code is kept in and why, so
                   your change lands in the same shape.

Code calisthenics is a set of deliberately strict constraints that push code toward a shape it does not fall into on its own. Some of them are arguable, but holding to one across a whole crate makes that crate uniform, and uniform code is code you can skim. A change that only reshapes code never changes behaviour: `cargo test --workspace` is the proof, green before the first edit and after the last.


No `else`
---------

**In `models/` and `engine/`, a control-flow `else` -- `} else {` or `} else if` -- has no place.** An `else` is almost always one of these in disguise:

  * **A guard.** Handle the bad case, `return` (or `continue` in a loop), and let the rest of the function run at one indent level.
  * **A `match`.** A chain of `else if` on the same value is a `match` that has not admitted it yet. Two bools that are one decision are a `match` on the pair. An `if let ... else` is a `match` on `Some` and `None`.
  * **A shared statement.** When both branches do most of the same thing, pull the shared part out and keep only the difference behind the `if`.
  * **A function.** A long tail of cases in an already deep function is a named function whose body is guard clauses (`conditions::speed_shift_message`).
  * **Noise.** An `if` that ends in `return`, `continue` or `break` needs no `else`; dedent the block.

**The `else` that stays is the expression ternary.** Rust has no `?:`, and `let sx = if x0 < x1 { 1 } else { -1 };` is the clearest two-way choice of a value there is. Both arms a bare value, no statements. The moment a branch grows a statement or a third case appears, it is back in scope. `let ... else` is a guard and is encouraged.

    grep -rn '} else' models/src engine/src --include='*.rs'   # each hit: a let-else or a ternary


Shallow nesting
---------------

**No function nests more than about three blocks deep** -- a function body, a loop, and one conditional inside it, give or take. The strict calisthenics rule is one level, and that is the wrong stopping point here: pushed all the way, one readable 40-line function becomes eight three-line ones, each threading six `world` parameters, and you read a call graph instead of a procedure. Three is a ceiling, not a target.

The depth comes down the same ways an `else` goes, plus three:

  * **Extract the loop body.** `ai::monster_round` is a `for` over `step_one_mob`; `items::item_system` a `for` over `resolve_use`.
  * **Name the phases of a long procedure.** `combat::resolve_attack` reads as `fold_matchup`, `roll_swing`, `clamp_swing`, `land_swing`, `punctuate`, `report_blow`, `settle_the_dead`. An extraction earns its place if you can name it in three words without "helper" or "handle".
  * **Reach for iterators.** `.filter().map()`, `.any()` and `.find_map()` flatten a loop and a conditional into one expression. A long chain is one thought; leave it whole.

Each hit of this check is a spot to look at, not a failure: the renderer's cell-diff double loop can sit that deep where an extraction would be ceremony.

    # statements indented 24+ spaces (6+ levels)
    grep -rnP '^ {24,}\S' models/src engine/src --include='*.rs' \
      | grep -v '^\S*:[0-9]*: *//'


Rules nihilurk deliberately does *not* adopt
----------------------------------------

**Wrap every primitive in a type.** Tile coordinates are `(u16, u16)` tuples, which already destructure, `Copy` and pattern-match, and the arithmetic on them is one-liners. A coordinate newtype would be ceremony until it grows real behaviour.

**First-class collections.** The collections here are short-lived locals: a set of visible tiles, the mobs for this pass. The ECS supplies the real domain structure, and wrapping scratch space in named types would couple call sites to an interface built for one function.

**One dot per line.** Chaining is how Rust expresses a data transform.

**Keep entities small.** Sound advice -- a component with eight fields is usually two -- but the components are already small, so it is not a pass.


One rule of nihilurk's own: a test never asserts a constant
------------------------------------------------------

**Never write a test whose assertion is a copy of a tuning number.** A test that says a wand rolls `3..=9` is testing `constants/`, which needs no help. The next rebalance turns it red while the code is correct, and it teaches the next person to distrust the suite. In order of preference:

  1. **Assert the relation.** "A thrown wand hits harder than the zap it gave up." It stays true across every rebalance because it is what the feature *is*.
  2. **Read the constant.** `DAMAGE_DICE * DAMAGE_SIDES`, not `8`. The test proves the roller honours the table.
  3. **Purge it.** If the number was the only claim, delete the test and assert an invariant a rebalance cannot touch and a bug could.

A test's own loop bound is not a tuning number and should not borrow one. `models/tests/it/mind/autoexplore.rs` asserts termination against a local `NEVER`, not `travel::AUTO_EXPLORE_STEP_CAP`: borrowing the cap would make a tightened safety valve look like an infinite loop.


One rule of nihilurk's own: behavior tests build components, not bestiary rows
-------------------------------------------------------------------------------

**A behavior test builds the smallest entity that carries the property under test.** Do not spawn an orc to test equipment or a dragon to test fire immunity. A bestiary row brings its grants, gear, speed and balance with it, so an unrelated row edit changes the fixture before the assertion runs.

Start from the neutral fixtures in `models/tests/it/common/monster.rs` and attach what the test needs: `Grant::of::<Undead>()`, `ItemUser`, `Invisible`. The assertion then names the contract and cannot pass because a species happened to carry extra behaviour. Keep bestiary-backed tests for where the bestiary is the subject: content-table validation, playable bodies, spawning, roster-wide contracts.


One rule of nihilurk's own: no comments inside a function body
---------------------------------------------------------------

**In `engine` and `models`, a comment sits above the thing it describes, never inside a function body**, test functions included. A comment in a body is a name or a function not written yet: "skip the dead" becomes `if is_dead(mob) { return }`. In an identifier, a rename keeps it true and an edit to the code cannot leave it behind.

These sit outside a body and stay: doc comments (`///`) and file headers (`//!`); a note over a signature or between its parameters; a note on its own line directly above a closure, with no blank line between; the how-to-add-a-row notes over a catalog table like `BESTIARY`.

The pre-commit hook enforces this (`.githooks/no_body_comments.lua`, tested by `.githooks/no_body_comments_test.lua`) on the staged file as `cargo fmt` writes it. CI does not run it, so a clone without the hook can break the rule and review is what catches that.

    lua .githooks/no_body_comments.lua --tree      # every tracked file; reads what git has staged


See also
--------

  data-driven-content.md      the other thing that keeps nihilurk small
  ecs-in-nihilurk.md              what the shape is holding up
  ../how-to/work-with-the-ecs.md   the borrow patterns this shape assumes
  ../README.md                "documentation ships with the change"
