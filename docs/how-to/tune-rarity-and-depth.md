How to tune rarity and depth
============================

    Audience       Content author balancing a floor.
    Prerequisites  You have added something and now want it to show up
                   less, or later, or at all.
    Result         You know which of the four dials to turn, and which
                   ones you should leave alone.

There are four places rarity is decided, and they answer different
questions. Turning the wrong one is the usual mistake.

    1. How many things per floor?        engine knob, models/src/map.rs
    2. Which category of item?           DROPS, models/src/spawn.rs
    3. Which row within a category?      the row's own weight
    4. Is it allowed here at all?        the row's min_depth


How weights work
----------------

An entry's chance is its weight over the sum of the weights it competes
against, and it competes only inside one draw: a monster's weight against
other monsters, a `DROPS` weight against other categories.

**Ten is the baseline.** A row at 5 is half as common as its neighbours;
one at 20 is twice. Nothing has to total 100, so you can add a row
without editing another number.

Dial 1: how many things per floor
---------------------------------

This is not content. It is floor generation, in `populate_level` in
`models/src/map.rs`, and it is the same for every row.

Everything scales off `tier` -- `map::difficulty_tier(depth)`, which steps
at the depths in `constants::progression::DIFFICULTY_TIER_LAST_DEPTH`
(`[3, 6, 9, 12]`): tier 0 on floors 1-3, tier 1 on 4-6, tier 2 on 7-9,
tier 3 on 10-12, tier 4 on floor 13 alone. The damage traps scale too,
but on their own coarser three bands
(`constants::traps::TRAP_DAMAGE_TIER_LAST_DEPTH`, `[4, 8]`).

    monsters      3 + tier slots. The first always fills; each later one
                  fills with probability min(0.60 + 0.12 * tier, 0.95).

    lurkers       from floor 7, each corridor centre has a 5% chance of
                  hiding one more.

    items         3 + tier attempts. Every attempt that finds a free
                  tile drops an item (no fill roll).

    hidden item   one floor in five hides one more in plain sight.

    traps         4 + tier slots, each filling with probability
                  min(0.12 + 0.13 * tier, 0.75).

Change these when the dungeon feels too empty or too crowded. Do not
change them to make one creature rarer -- that is dial 3.


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

    category!("scroll",      300,      1,     SCROLLS),
    category!("potion",      270,      1,     POTIONS),
    category!("coin",        170,      1,     COINS),
    category!("armor",        80,      1,     ARMORS),
    category!("wand",         50,      1,     WANDS),
    category!("ring",         50,      1,     RINGS),
    category!("weapon",       36,      1,     WEAPONS),
    category!("ammo",         28,      1,     AMMO),
    category!("launcher",     16,      1,     LAUNCHERS),
                              |        |
                           weight   min_depth

Those weights are Rogue's own drop odds in tenths of a percent: scrolls
are 30% of drops, potions 27%, and so on down to launchers at 1.6%. They
happen to total 1000, which is convenient to read and not required.

To double how often rings turn up, change `50` to `100`. Every other
share drops slightly to pay for it, automatically.

To keep a category out of the shallow dungeon, raise its `min_depth`. A
category that cannot appear is simply not in the draw, and the remaining
categories divide its share between them in proportion.


Dial 3: which row within a category
-----------------------------------

**Monsters** carry the dial on the row:

    MonsterDef::row("dragon", ...).weight(3),

**Traps** carry it as a field:

    TrapDef { effect: ..., weight: 3, min_depth: 9, ... },

**Items** do not have per-row rarity today, on purpose: within a category
roog picks evenly, and the design likes it that way. The hook exists if
you want it -- `ItemDef::weight()` and `ItemDef::min_depth()` are trait
methods with defaults of `10` and `1`, and the picker already honours
them. To give one category per-row rarity, add a `weight: u32` field to
its `Def` struct and return it:

    impl ItemDef for ArmorDef {
        fn name(&self) -> &'static str { self.name }
        fn weight(&self) -> u32 { self.weight }     // <- added
        ...
    }

Then every `ArmorDef` row must carry a weight, which is the cost. Decide
whether the category deserves it before you pay it.


Dial 4: is it allowed here at all
---------------------------------

`min_depth` is the shallowest floor a thing may appear on. It is a hard
gate, not a weight: below it the row is not in the draw.

Monsters use it as the last column of the row:

    //              name         glyph  colour     move   hp pow pb  ar ab  dep
    MonsterDef::row("dragon",    'D',   Color::Red, Chase, 8, 12, 2, 10, 2, 10),

The bestiary uses 1, 5 and 10 -- fodder from the start, the mid tier from
floor 5, the nastiest letters from floor 10. Nothing stops you using 2 or 11.

The dungeon is 13 floors deep (`FINAL_DEPTH`), so a `min_depth` above 13
means "never" -- on the way in. The climb out is the exception: once the
Element of Yoord is in the pack, floor population switches from `pick` to
`MonsterDef::pick_any`, which drops the gate entirely, so a `min_depth` of
10 (or 99) is no protection on the ascent.

> **Every floor still draws from everything it has unlocked.** A goblin
> does not stop appearing on floor 9; it competes with the dragon. That
> is what keeps deep floors feeling like a dungeon rather than a boss
> rush. If you want something to *stop* appearing, `min_depth` is the
> wrong dial -- there isn't one, and adding one is an engine change.


Verify what you changed
-----------------------

The table tests cover the gating but not your intent, so measure it:

    cargo test --test content

`the_loot_table_covers_every_category_over_a_long_run` rolls twenty
thousand drops and asserts every category still produces something. If
you weight a category down to nothing, that test tells you.

For a feel, the quickest instrument is a throwaway loop over
`roll_item` or `MonsterDef::pick` counting names -- see
`models/tests/content.rs` for the shape.


See also
--------

  ../reference/spawn-api.md        pick_weighted, roll_item, DROPS
  ../reference/content-tables.md   where weight and min_depth live
  add-an-item-category.md          adding a row to DROPS itself
