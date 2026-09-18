roog
====

A classic roguelike about descending thirteen floors, taking the Element of Yoord off the Dungeon Lord, and carrying it back up. Life is unfair and death is permanent. Terminal only, one binary, no assets.

    cargo run -p engine                  # play
    cargo run -p engine -- -s 1234       # play a specific seed
    cargo test                           # everything


Where things are
----------------

    MANUAL.md      how to play: controls, combat math, items, monsters.
    doc/roog.6     the installed `man roog` command reference.
    docs/          how to add content to the game. Start at docs/README.md.
    gdd.md         what roog is trying to be. Design, not code.
    models/        the game: rules, content tables, ECS systems.
    view/          the terminal grid: the double buffer and its diff.
    engine/        the terminal front end: input, rendering, the loop.
    particle-core/ the particle arithmetic, `no_std` and dependency-free.
    perf/          the stress rig. Did I make it slower?
    compat/        the machine matrix. Does it still run on a Pi?

The last three are test rigs, kept out of the workspace's default members, so a bare `cargo build` or `cargo test` never compiles them:

    ./perf_test.sh                       # the particle layer, measured
    ./compat_test.sh                     # roog, built and run on six machines
    ./docs_style.sh                      # docs/, held to the house style


Adding content
--------------

There is no pink dragon in roog. This is the whole of putting one in.

Open `models/src/monsters.rs`, find `BESTIARY`, add a line:

    MonsterDef::row("pink dragon", 'D', Color::Magenta, Chase,
                    8, 12, 2, 10, 2, 7)
        .grants(&[Grant::of::<FireImmune>()]),

That is: name, glyph, colour, how it moves, then hit points, attack die, attack bonus, armour die, armour bonus, and the shallowest floor it can appear on -- plus, chained on, the fact that fire does nothing to it.

    cargo build

That is the change. The pink dragon now populates floors from depth 7, is something a wand of polymorph can turn a bat into, something a scroll of create monster can summon, a legal bane for a vorpal weapon, and it comes back correctly out of a save file. Nothing else in the codebase had to be told it exists, because nothing else enumerates species.

Items are the same shape, in `models/src/catalog.rs` -- one table per kind, one row per thing:

    WeaponDef::new("quarterstaff", Color::DarkYellow, 7).missile(6),
    ArmorDef { name: "brigandine", color: Color::Grey, armor_die: 6 },

A row is a name, an appearance, and the components the thing carries into the world. There is no dragon code and no ring code: combat reads components and never learns what kind of thing put them there. A few categories need one edit more -- somebody has to say what drinking a new potion does -- and the docs say which, and why.

To see what you made without playing down to floor 7:

    ROOG_SPAWN="pink dragon" cargo run -p engine
    cargo run -p engine -- -content       # every name the game knows


Documentation
-------------

`docs/` is split by what you need at the moment you open it.

    Learning        docs/tutorial/       a lesson, followed start to end
    Solving         docs/how-to/         a recipe for one real task
    Information     docs/reference/      every field, flag and signature
    Understanding   docs/explanation/    why it is built this way

    cat docs/README.md                          # the index
    cat docs/tutorial/add-your-first-item.md    # start here, ~15 minutes
    cat docs/tutorial/add-your-first-monster.md # then the bestiary
    cat docs/how-to/add-a-monster.md            # or go straight to a recipe

Changing the *engine* rather than the content is a different door:

    cat docs/explanation/ecs-in-roog.md         # how bevy_ecs is used here
    cat docs/how-to/work-with-the-ecs.md        # and how to get past the borrow checker

`gdd.md` is a fifth thing: what roog is trying to *be*. Design, not code.
