How to add a monster
====================

    Audience       Content author.
    Prerequisites  You have done `../tutorial/add-your-first-item.md`,
                   or you are comfortable enough to skip it.
    Result         A new species that populates floors, can be
                   polymorphed into, and can be created by a scroll.

One line in one table. There is no second place.


The table
---------

    models/src/monsters.rs      ->  BESTIARY

Every creature in the game is a row in it. Level population, the scroll
of create monster, the wand of polymorph and the scroll of vorpalize
weapon all draw from this table, so a row is a species everywhere at
once.


The recipe
----------

1. Open `models/src/monsters.rs` and find `BESTIARY`.

2. Copy a row that is close to what you want and change it. A basilisk,
   say -- slow, armoured, nasty, and not something floor 1 should meet:

       MonsterDef::row("basilisk", 'b', Color::DarkGreen, Chase,
                       5, 10, 1, 10, 2, 5),

   The arguments, in order:

       name          "basilisk"          unique, lowercase, displayed
       glyph         'b'                 one character on the map
       color         Color::DarkGreen    see the palette note below
       movement      Chase               Static | Chase | Flee | Confused
       hp            5                   total hit points
       power         10                  attack die: damage rolls 1d10
       power_bonus   1                   flat, added once to that roll
       armor         10                  defence die: rolls 1d10
       armor_bonus   2                   flat, added once to that roll
       min_depth     5                   first floor it can appear on

3. Build and look at it:

       cargo build
       ROOG_SPAWN="basilisk" cargo run -p engine

4. Run the table tests, which will now include your row:

       cargo test --test content

That is the whole procedure. Nothing else in the codebase needs to be
told that basilisks exist.


Choosing the numbers
--------------------

Copy the band your creature belongs to:

    fodder      hp 1-2    power 4-8     armor 4-8       min_depth 1
    mid         hp 3-6    power 6-10    armor 6-10      min_depth 5
    deep        hp 8-12   power 8-12    armor 6-10      min_depth 10

Raise `armor` to make it hard to kill, `power` to make it frightening to
stand next to, `hp` only to buy it one more exchange. Why that is so --
and why nothing in the bestiary has thirty hit points -- is
`../explanation/combat-and-balance.md`.

`min_depth` in use is 1, 5 and 10. Nothing stops you using 2 or 11.

Optional extras, chained on
---------------------------

Anything past the ten numbers is chained, so a plain creature stays one
readable line.

**Innate magic** -- attach effect components at birth:

    MonsterDef::row("basilisk", ...).grants(&[Grant::of::<ColdImmune>()]),

The effect must already exist. See `add-an-effect.md`. There is a
shorthand for the common one -- `.grants(ITEM_USER)` -- which makes a
creature clever enough to catch thrown gear, wear it, and read scrolls
that land on it.

**Born invisible** -- unseeable without see-invisible (the phantom):

    MonsterDef::row("basilisk", ...).invisible(),

**Rarity** -- how often it is drawn against the rest of the floor's
eligible pool. The default is 10; leave it alone unless you mean it:

    MonsterDef::row("basilisk", ...).weight(3),

See `tune-rarity-and-depth.md`.

Chains combine in any order:

    MonsterDef::row("basilisk", 'b', Color::DarkGreen, Chase,
                    5, 10, 1, 10, 2, 5)
        .grants(&[Grant::of::<ColdImmune>()])
        .weight(4),


> **Do not write `Aggravated` in a row.** It is a fifth `MovementType`,
> applied at run time by the scroll of aggravate monsters. The four legal
> values for a row are the ones listed above; what each does is in
> `../reference/content-tables.md`.

> **Only 16 colours survive a save.** The file packs a colour into one
> byte against a fixed palette; anything off it -- an RGB value, a
> 256-colour index -- draws correctly and reloads as White. The palette is
> listed in `../reference/content-tables.md`.

> **Renaming a row breaks old saves.** A saved monster stores its name and
> looks its species back up on load. Rename `"basilisk"` and every
> basilisk in an existing save comes back without its innate magic. Adding
> and removing rows is safe; renaming is not.


Checks that will catch you
--------------------------

`cargo test --test content` fails if:

  * two rows share a name (`every_content_name_is_unique`)
  * your row cannot be built by name (`every_content_name_spawns_and_...`)
  * a species turns up above its `min_depth`
    (`a_species_never_appears_above_its_min_depth`)

Glyph collisions are *not* checked. Two creatures may share a letter; the
existing bestiary uses one letter per species by convention, and `g` is
already the goblin's.


See also
--------

  add-an-effect.md                 give it a property nothing else has
  tune-rarity-and-depth.md         where and how often it shows up
  ../reference/content-tables.md   every field, every type, the palette
  ../reference/spawn-api.md        MonsterDef::pick, spawn_monster
  ../explanation/combat-and-balance.md   what the numbers mean
