Code calisthenics in roog
=========================

    Audience       Anyone changing engine or model code and wondering
                   why a function they were about to write an `if/else`
                   in has none anywhere near it.
    Prerequisites  You write Rust. You have a diff open.
    This is        Understanding and a house rule, not a tutorial. It
                   tells you what shape the code is kept in and why, so
                   your change lands in the same shape.

Code calisthenics is a set of deliberately strict constraints you apply
while editing, to push code toward a shape it does not fall into on its
own. The constraints are not laws of good code -- some of them are
arguable -- but holding to one across a whole crate makes that crate
uniform, and uniform code is code you can skim.

roog adopts them one at a time, crate by crate, each pass touching
nothing but the shape. A pass never changes behaviour: the test suite is
the proof, and it stays green from the first edit to the last.


Phase 1: no `else`
------------------

**Done in `models/` and `engine/`.** A control-flow `else` -- `} else {`
or `} else if` -- does not appear in either crate.

An `else` is almost always one of a small number of things wearing a
disguise, and each has a plainer form:

### A guard that should have returned early

    fn tile(&self, x: u16, y: u16) -> TileType {
        if x >= MAP_WIDTH || y >= MAP_HEIGHT {
            TileType::Wall
        } else {
            self.tiles[tile_index(x, y)]
        }
    }

becomes

    fn tile(&self, x: u16, y: u16) -> TileType {
        if x >= MAP_WIDTH || y >= MAP_HEIGHT {
            return TileType::Wall;
        }
        self.tiles[tile_index(x, y)]
    }

The out-of-bounds case is handled and dismissed; the rest of the function
runs at one indent level, unconditionally, because by the time you reach
it the bad input is gone. This is the single most common transformation,
and in a loop the early exit is `continue` rather than `return`.

### A branch on an enum or an `Option` that should have been a `match`

    if arg == "-s" {
        ...
    } else if arg == "-c" {
        centered_mode = true;
    } else if arg == "-ns" {
        no_save = true;
    } else {
        positional = Some(arg.clone());
    }

becomes

    match arg.as_str() {
        "-s" => { ... }
        "-c" => centered_mode = true,
        "-ns" => no_save = true,
        _ => positional = Some(arg.clone()),
    }

A chain of `else if` comparing the same value against constants is a
`match` that has not admitted it yet. When the arms test *two* bools that
are really one decision, match the pair:

    let color = match (i == selected_idx, *equipped) {
        (true, _) => Color::Yellow,
        (_, true) => Color::Cyan,
        _ => Color::White,
    };

An `if let ... else` whose `else` is the `None` case is
`match ... { Some(x) => ..., None => ... }`.

### Two branches that do most of the same thing

    if last && more {
        screen.puts(0, y, line, Color::White);
        screen.puts(57, y, "--MORE--", Color::Yellow);
    } else {
        screen.puts(0, y, line, Color::White);
    }

becomes

    screen.puts(0, y, line, Color::White);
    if last && more {
        screen.puts(57, y, "--MORE--", Color::Yellow);
    }

Pull the shared statement out; keep only the difference behind the `if`.
The same move collapses `if a { X } else if b { X }` into
`if a || b { X }`.

### A tail of cases too big for the calling function

When the branches are long and the function is already deep -- the
speed-shift message, the auto-fight turn, the two attack-report blocks in
`resolve_attack` -- lift them into a named function whose body is guard
clauses:

    fn speed_shift_message(name: &str, faster: bool, is_player: bool, changed: bool) -> String {
        let extreme = if faster { "quick" } else { "sluggish" };
        if !changed && is_player {
            return format!("You are already as {extreme} as you can be.");
        }
        if !changed {
            return format!("The {name} is already as {extreme} as it can be.");
        }
        match (is_player, faster) { ... }
    }

The call site loses a branch and gains a name that says what the branch
was for.

### A returning `if` followed by its own `else`

    if let Some(idx) = action_mode {
        ...
        return Ok(turn_taken);
    }
    // Sub-branch: the main list. Not an `else` -- the branch above
    // always returns, so this is the fall-through.
    else { ... }

If the `if` block ends in `return` (or `continue`, or `break`), the
`else` is noise. Drop the keyword, dedent the block, and leave a comment
saying the branch above is terminal.


The one `else` that stays: the expression ternary
-------------------------------------------------

Rust has no `?:`. The nearest thing is `let x = if c { a } else { b }`,
and for a **two-way choice of a value** that is the clearest form there
is:

    let sx = if x0 < x1 { 1 } else { -1 };
    let dice = if excellent { EXCELLENT_HIT_DICE } else { 1 };
    let label = if equipped { " (E)" } else { "" };

These are kept. Rewriting them as `match c { true => a, false => b }`
trades a familiar idiom for an unfamiliar one and reads worse. A
two-way `if`/`else` in **expression position, both arms a bare value,
no statements** is not the `else` this rule is about.

The moment a branch grows a statement, or a third case appears, it is
back in scope: a three-way value choice is a `match`, and a branch that
*does* something rather than *evaluating to* something is a guard.


Checking a pass
---------------

    grep -rn '} else' models/src engine/src --include='*.rs'

Every hit should be a `let ... else { ... }` (the guard-clause let, which
is encouraged) or an expression ternary as described above. Anything
else is a regression.

`cargo test --workspace` is the proof that the pass changed only shape.
Run it before the first edit and after the last; it does not move.


Later phases
------------

Not yet started. Candidates, roughly in order of how much they would
tighten roog specifically:

  * **One level of indentation per function.** Falls out of "no `else`"
    most of the way; the rest is extracting nested loops.
  * **Wrap primitives that travel together.** `(x, y)` pairs are
    everywhere; a `Point` would carry the arithmetic that is currently
    inlined at every call site.
  * **No getters/setters on the ECS components** -- already mostly true,
    worth making a rule.

Each will get a section here when it is done, with the crate it covers
and the grep that checks it.


See also
--------

  data-driven-content.md      the other thing that keeps roog small
  ../README.md                "documentation ships with the change"
