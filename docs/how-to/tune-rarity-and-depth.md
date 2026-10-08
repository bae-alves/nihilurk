How to tune rarity and depth
============================

    Audience       Content author balancing a floor.
    Prerequisites  You have added something and now want it to show up
                   less, or later, or at all.
    Result         You know which of the four dials to turn, and which
                   ones you should leave alone.

There are four places rarity is decided, and they answer different questions. Turning the wrong one is the usual mistake.

    1. How many things per floor?        engine knob, map/population.rs
    2. Which category of item?           DROPS, models/src/spawn.rs
    3. Which row within a category?      the row's own weight
    4. Is it allowed here at all?        the row's min_depth


How weights work
----------------

An entry's chance is its weight over the sum of the weights it competes against, and it competes only inside one draw: a monster's weight against other monsters, a `DROPS` weight against other categories.

**The default weight is the baseline** (`constants::monsters::DEFAULT_SPAWN_WEIGHT` for the bestiary). A row at half of it is half as common as its neighbours; one at double it is twice. Nothing has to total anything, so you can add a row without editing another number. Weights are whole numbers, so 1 is the floor: to make a row rarer than 1 you raise the baseline and every explicit weight with it (spirits did exactly this).

Dial 1: how many things per floor
---------------------------------

This is not content. It is floor generation, in `populate_level` in `models/src/map/population.rs`, and it is the same for every row.

Everything scales off `tier` -- `map::difficulty_tier(depth)`, which steps at the depths in `constants::progression::DIFFICULTY_TIER_LAST_DEPTH`: tier 0 on the shallowest floors, and the deepest floor a tier of its own. The damage traps scale too, but on their own coarser bands (`constants::traps::TRAP_DAMAGE_TIER_LAST_DEPTH`). Every name below is in `constants::population`.

    guaranteed    placed before any budget is spent, and never out of
                  one: a blue coin on the odd floors and a red one on
                  the even. On the last floor of each tier -- the
                  depths in DIFFICULTY_TIER_LAST_DEPTH -- one draw from
                  `catalog::PROGRESSION_ITEMS` as well.

    monsters      MONSTER_SLOTS_BASE + tier slots. The first always
                  fills; each later one fills with probability
                  MONSTER_FILL_CHANCE_BASE + MONSTER_FILL_CHANCE_PER_TIER
                  * tier, capped at MONSTER_FILL_CHANCE_CAP.

    lurkers       from floor CORRIDOR_LURKER_MIN_DEPTH, each corridor
                  centre has a CORRIDOR_LURKER_CHANCE chance of hiding
                  one more.

    items         ITEM_SLOTS_BASE + tier attempts. Every attempt that
                  finds a free tile drops an item (no fill roll).

    hidden item   HIDDEN_ITEM_CHANCE of floors hide one more in plain
                  sight -- no glyph until you walk onto it.

    traps         TRAP_SLOTS_BASE + tier slots, each filling with
                  probability TRAP_FILL_CHANCE_BASE +
                  TRAP_FILL_CHANCE_PER_TIER * tier, capped at
                  TRAP_FILL_CHANCE_CAP.

Change these when the dungeon feels too empty or too crowded. Do not change them to make one creature rarer -- that is dial 3.


> **Turning any of these cannot move a wall.** A floor's layout is built
> from `layout_rng(seed, depth)` and its contents from
> `content_rng(seed, depth, changes)` -- neither is the shared run stream,
> so nothing a player does mid-fight can shift a floor. (The contents
> *do* re-roll each visit, off the staircase count.) What your weights
> change is what a given seed produces from the table you edited, which is
> the whole point of editing it. See `../explanation/data-driven-content.md`.

Dial 2: which category of item
------------------------------

    models/src/spawn.rs     ->  DROPS

    category!("scroll",   <weight>,    1,     SCROLLS),
    category!("potion",   <weight>,    1,     POTIONS),
    ...
                              |        |
                           weight   min_depth

Those weights are Rogue's own drop odds in tenths of a percent. `../reference/content-tables.md` carries the current table, and a test holds it in step with `DROPS`. They do not have to total anything round, which is only convenient to read.

To double how often rings turn up, double the ring row's weight. Every other share drops slightly to pay for it, automatically.

To keep a category out of the shallow dungeon, raise its `min_depth`. A category that cannot appear is simply not in the draw, and the remaining categories divide its share between them in proportion.

The rune row is the example. Its `min_depth` is above 1, so the shallowest floors draw from every other category and the rune joins the draw below that. The weights table in `../reference/content-tables.md` shows the share with the rune in the draw, and says what the shallower floors total. `models/tests/loot.rs` scales a category's expected share by the floors it is on, so a new category that starts deeper needs no special case there.


Dial 3: which row within a category
-----------------------------------

**Monsters** carry the dial on the row:

    MonsterDef::row("dragon", ...).weight(3),

**Traps** carry it as a field:

    TrapDef { effect: ..., weight: 3, min_depth: 9, ... },

**Items** do not have per-row rarity today, on purpose: within a category nihilurk picks evenly, and the design likes it that way. The hook exists if you want it -- `ItemDef::weight()` and `ItemDef::min_depth()` are trait methods with defaults of `10` and `1`, and the picker already honours them. To give one category per-row rarity, add a `weight: u32` field to its `Def` struct and return it:

    impl ItemDef for ArmorDef {
        fn name(&self) -> &'static str { self.name }
        fn weight(&self) -> u32 { self.weight }     // <- added
        ...
    }

Then every `ArmorDef` row must carry a weight, which is the cost. Decide whether the category deserves it before you pay it.


Dial 4: is it allowed here at all
---------------------------------

`min_depth` is the shallowest floor a thing may appear on. It is a hard gate, not a weight: below it the row is not in the draw.

Monsters use it as the last column of the row:

    //              name         glyph  colour     move   hp pow pb  ar ab  dep
    MonsterDef::row("dragon",    'D',   Color::Red, Chase, 8, 12, 2, 10, 2, 10),

The bestiary uses 1, 5 and 10 -- the shallow stat band from the start, the middle band from floor 5, the nastiest letters from floor 10. Nothing stops you using 2 or 11.

The dungeon is `FINAL_DEPTH` floors deep, so a `min_depth` above that means "never" -- on the way in. The climb out is the exception: once the Element of Yoord is in the pack, floor population switches from `pick` to `MonsterDef::pick_any`, which drops the gate entirely, so a `min_depth` of 10 (or 99) is no protection on the ascent.

> **Every floor still draws from everything it has unlocked.** A bat
> does not stop appearing on floor 9; it competes with the dragon. That
> is what keeps deep floors feeling like a dungeon rather than a boss
> rush. If you want something to *stop* appearing, `min_depth` is the
> wrong dial -- there isn't one, and adding one is an engine change.


Verify what you changed
-----------------------

The table tests cover the gating but not your intent, so measure it:

    cargo test --test content

`every_loot_category_can_be_drawn_and_has_rows_to_give` asks the table, not a sampler: every category keeps a nonzero weight and has rows at its debut floor and at the deepest. If you weight a category down to nothing, that test tells you.

For a feel, the quickest instrument is a throwaway loop over `roll_item` or `MonsterDef::pick` counting names -- see `models/tests/content.rs` for the shape.


Appendix: quick check
---------------------

1. Decide which question you are asking: how many per floor, which category, which row, or allowed here at all.
2. Per floor: edit `populate_level` in `models/src/map/population.rs`; this moves every row at once.
3. Category: change its weight in `DROPS` (`models/src/spawn.rs`).
4. Row: `.weight(n)` on a monster, or `weight` on a trap; twenty is the baseline (monsters and traps; spirits sit at 1, the floor) and items have no per-row weight.
5. Gate: raise `min_depth`; it is a hard gate, and nothing makes a row stop appearing deeper.
6. Run `cargo test --test content`.
7. Count what a draw gives you with a throwaway loop over `roll_item` or `MonsterDef::pick`.
8. Fix the docs: if you changed a `DROPS` weight, update the weights table in `../reference/content-tables.md` and the listing in this page.


See also
--------

  ../reference/spawn-api.md        pick_weighted, roll_item, DROPS
  ../reference/content-tables.md   where weight and min_depth live
  add-an-item-category.md          adding a row to DROPS itself
