Tutorial: add your first item
=============================

    Audience       Anyone who wants to add content to roog. You have
                   written a little Rust, or a little of something like
                   it. You have never touched this codebase.
    Prerequisites  A checkout, and a working `cargo`. That is all.
    Time           About fifteen minutes.
    You will       Add a weapon to the game, see it in the dungeon,
                   prove it works, then change how it behaves.

This is a lesson, not a recipe. It takes the longest road on purpose.
When you want the short version, read `../how-to/add-an-item.md`.


What you are about to learn
---------------------------

In roog, a piece of content is **a row in a table**. Not a class, not a
file, not a subclass of `Weapon` -- a line. The line says what a thing is
called, how it draws, and what it carries into the world. Nothing else in
the game enumerates weapons, so nothing else needs to hear about yours.

By the end you will have added a quarterstaff, and you will believe that
sentence.


Step 1: find the table
----------------------

Every item in the game lives in one file:

    models/src/catalog.rs

Open it and search for `WEAPONS`. You are looking at this:

    #[rustfmt::skip]
    pub const WEAPONS: &[WeaponDef] = &[
        WeaponDef::new("dagger",           Color::Grey,     4).missile(4).piercing(),
        WeaponDef::new("spear",            Color::DarkGrey, 6).missile(8).piercing(),
        WeaponDef::new("mace",             Color::DarkGrey, 6),
        WeaponDef::new("long sword",       Color::White,    8),
        WeaponDef::new("two-handed sword", Color::Cyan,    10),
    ];

Five weapons, five lines. Read one:

    WeaponDef::new("mace", Color::DarkGrey, 6)
                     |          |           |
                  its name   its colour   its damage die

The mace rolls `1d6` when you swing it. The long sword rolls `1d8`. That
third number is the whole of what a weapon is worth in a fight -- see
`../reference/content-tables.md` if you want the combat maths now,
though you do not need it yet.


Step 2: add a row
-----------------

A quarterstaff should sit between the mace and the long sword: a bit
better than a club, nowhere near a sword. Add this line after the mace:

    WeaponDef::new("quarterstaff",     Color::DarkYellow, 7),

Line the columns up with its neighbours. The table carries
`#[rustfmt::skip]`, which means `cargo fmt` will leave your alignment
alone -- these tables are meant to be read as columns.

Now build:

    cargo build

It compiles. That is the entire change. There is no registration step,
no `match` arm waiting for you, no list of weapon names somewhere else
that is now out of date.


Step 3: see that the game knows about it
----------------------------------------

Ask the game what content it has:

    cargo run -p engine -- -content | grep quarterstaff

You should get:

      quarterstaff

That listing is not a hand-maintained document. It reads the tables at
run time, walks every row, and prints what it finds. Your weapon is in
it because the table is the only place weapons are defined.


Step 4: hold it in your hands
-----------------------------

Waiting for a 3.6%-chance drop to prove your work is a miserable way to
spend an evening. So there is a shortcut:

    ROOG_SPAWN="quarterstaff" cargo run -p engine

The floor is built as normal, and then the things you named are dropped
on free tiles next to you. Walk one step, pick it up, wield it.

You can ask for several at once, and mix kinds freely:

    ROOG_SPAWN="quarterstaff,dragon,ring of protection" cargo run -p engine

Anything the tables do not recognise is skipped in silence. See
`../reference/cli-and-env.md`.


Step 5: prove it, so it stays proved
------------------------------------

Looking at it is not the same as knowing it works. Run the table tests:

    cargo test --test content

Several of those tests just read your new row. They checked that no two
rows share a name, that every name spawns something, that what spawns
keeps the name it was spawned by, and that the loot table can still
produce one of everything. Your quarterstaff was included automatically,
because those tests iterate the tables too.

That is the pattern worth taking away: in this codebase, *the tables are
data, so tests can check the data*. You did not have to write a test to
get your row tested.


Step 6: change what it does
---------------------------

A row is not only numbers. It is also behaviour, expressed by chaining.
Look at the dagger again:

    WeaponDef::new("dagger", Color::Grey, 4).missile(4).piercing(),

`.missile(4)` says: this thing is built to be thrown, and rolls `1d4`
when it lands. `.piercing()` says: a throw does not stop at the first
body -- it runs the whole line you aimed down.

Suppose your quarterstaff is really a javelin. Change your row to:

    WeaponDef::new("quarterstaff",     Color::DarkYellow, 7).missile(6),

Rebuild, and throw it at something:

    ROOG_SPAWN="quarterstaff,goblin" cargo run -p engine

It now flies properly: it goes around the target's armour die instead of
being blunted by it, it is spent on what it hits, and nothing can pluck
it out of the air. You did not implement any of that. Those three
behaviours belong to `.missile(...)`, and every row that asks for them
gets all three.

Nothing in the throwing code knows a quarterstaff exists. It asks
"is this a `Projectile`?", and your row answered.


Step 7: keep it or drop it
--------------------------

If you like the quarterstaff, leave it. If this was a dry run:

    git checkout models/src/catalog.rs


What you actually learned
-------------------------

  * Content is a row in a table, and the table is the only definition.

  * Behaviour is attached by naming it, not by writing it. `.missile()`
    and `.piercing()` are shorthands for components, and the systems
    that care look for the components, never for your item.

  * Nothing else has to be told. No registry, no factory, no `match`.

  * The tables are data, so the tools work on them for free: the
    `-content` listing, `ROOG_SPAWN`, and the table tests all read the
    same rows you edited.

Weapons are the easy case. Some categories need one more edit -- a
potion needs somebody to say what drinking it does, a ring needs an
identity to be identified by. Which ones, and why, is the subject of
the next page.


Where to go next
----------------

  ../how-to/add-an-item.md          every item category, one recipe each
  ../how-to/add-a-monster.md        the bestiary works the same way
  ../reference/content-tables.md    every field of every table
  ../explanation/data-driven-content.md   why it is built like this
