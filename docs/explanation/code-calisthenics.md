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


Phase 2: shallow nesting
------------------------

**Done in `models/` and `engine/`.** No function nests more than about
three blocks deep -- a function body, a loop, and one conditional inside
it, give or take. The `match` inside an `if` inside a `for` inside a
`while` inside a `for` is gone.

### The limit is ~3, not 1, on purpose

The strict calisthenics rule is *one* level of indentation per method.
That is the right pressure but the wrong stopping point for roog. Pushed
all the way it turns one readable 40-line function into eight
three-line ones, each named, each taking six `world`-threading
parameters, and you now read a call graph instead of a procedure. The
bloat costs more than the nesting did.

So the rule here is a **ceiling, not a target**: get out of the deep
nests, stop when the function reads top to bottom without scrolling your
eye rightward. In practice that lands around three levels.

### How the depth comes down

Mostly the same moves as Phase 1, plus two:

**Extract the loop body.** A `for` whose body is 60 lines and three
levels deep is a function that has not been written yet. `ai::monster_round`
became a `for` over `step_one_mob`; `items::item_system` a `for` over
`resolve_use`; `update::process_input_and_update` -- a 600-line, 13-deep
input handler -- split into `handle_targeting_input`,
`handle_inventory_input`, `handle_movement_input` and their callees.

**Reach for iterators before nesting.** `.filter().map().flat_map()` and
`.any()` / `.find_map()` flatten a loop-plus-conditional into one
expression with no indentation at all. `visibility::flood_fill_room`
walks `neighbours(x, y)` -- an iterator that yields the in-bounds
neighbours -- instead of a `for dy { for dx { if in_bounds ...`.

Iterator chains are *not* subject to a "one method call per line" rule.
Chaining is how idiomatic Rust expresses a data-structure transform, and
breaking a chain across statements to satisfy a dot budget makes it
harder to read, not easier. A long chain is one thought; leave it whole.

### Checking a pass

    # statements indented 24+ spaces (6+ levels) -- should print nothing
    grep -rnP '^ {24,}\S' models/src engine/src --include='*.rs' \
      | grep -v '^\S*:[0-9]*: *//'

A handful of 4-5 level spots remain by choice -- the renderer's
cell-diff double loop, a couple of `for` / `match` / `if` combinations
where the extraction would be pure ceremony. The test suite is again the
proof the pass changed only shape.


Rules roog deliberately does *not* adopt
----------------------------------------

The rest of the classic Object Calisthenics list, and why each is a poor
fit here rather than an oversight:

**Wrap every primitive in a type.** roog passes `(u16, u16)` tile
coordinates and `(i16, i16)` steps around by the hundred. A `Point`
newtype would carry the arithmetic, but Rust tuples already destructure,
`Copy`, and pattern-match cleanly, and the operations are mostly
one-liners (`.signum()`, `saturating_add_signed`) that read fine inline.
The wrapper would be ceremony without a payoff. If a coordinate type ever
grows real behaviour -- distance metrics, neighbour iteration used
everywhere -- revisit it then.

**First-class collections** (wrap every `Vec`/`HashMap` in a domain
type). This one would actively hurt. The collections in roog are
short-lived locals -- a `HashSet` of visible tiles, a `Vec` of mob
entities for this pass, a spatial `HashMap` rebuilt every tick. Wrapping
each in a named type with its own methods would couple call sites to an
interface that exists for one function's benefit, and make the loose,
rearrange-it-in-five-minutes character of this code stiff. The ECS
already supplies the real domain structure; the collections are just
scratch space, and scratch space benefits from staying informal.

**One dot per line.** Covered above: chaining is load-bearing in Rust
and roog uses it deliberately. Not adopted.

**Keep entities (structs) small.** This one is *sound* -- a component
with eight fields is usually two components -- and worth keeping in mind
when you add one. It is not enforced here only because roog's components
already tend to be small (`Position` is two fields, most grants are
zero), so there is nothing to clean up. Treat it as advice, not a pass.


One rule of roog's own: a test never asserts a constant
------------------------------------------------------

Not a calisthenics rule, but it belongs next to them, because it is the
same kind of discipline and it has already cost us a morning.

**Never write a test whose assertion is a copy of a tuning number.** A
test that says a wand rolls `3..=9`, that a floor hides a stash one time
in five, or that a kill is worth 700 points is not testing the game; it
is testing `constants.rs`, which needs no help. The moment somebody
rebalances -- and the working tree is *usually* mid-rebalance -- that
test goes red while the code is perfectly correct, and it teaches the
next person to distrust the suite.

Three ways out, in order of preference:

1. **Assert the relation.** "A thrown wand hits harder than the zap it
   gave up." "Two corpses in one turn beat two corpses in two turns." "A
   coin's burst reaches a tile a trap's does not." These stay true across
   every rebalance because they are what the feature *is*.
2. **Read the constant.** `CHARGE_DICE * CHARGE_SIDES + CHARGE_BONUS`
   instead of `13`; `share_of("coin")` off `DROPS` instead of `17.0`.
   Now the test proves the roller honours the table rather than
   proving the table says what it says.
3. **Purge it.** If a test's only claim was the number, delete it and
   put something real in its place. The stash test became "a stashed
   item carries `Invisible` *and* `Hidden`", which is an invariant a
   rebalance cannot touch and a bug could.

A test's own loop bound is not a tuning number and should not borrow
one. `models/tests/autoexplore.rs` asserts termination against a local
`NEVER: u32 = 20_000`, not against `travel::AUTO_EXPLORE_STEP_CAP` --
the cap is a safety valve somebody tunes, and borrowing it made a
tightened valve look like an infinite loop.


See also
--------

  data-driven-content.md      the other thing that keeps roog small
  ../README.md                "documentation ships with the change"
