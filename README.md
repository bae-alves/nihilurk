nihilurk
====

nihilurk is a free, open-source Rogue-like (roguelike) for the terminal, written in Rust. A Rogue retroclone about descending thirteen floors, taking the Element of Yoord off the Dungeon Lord, and carrying it back up. Life is unfair and death is permanent. Terminal only, one binary, no assets, a lot of punch.

If you're enjoying nihilurk, [buy me a coffee](https://ko-fi.com/baealves).

What you get:

  * A classic roguelike on a turn-based grid: thirteen floors, traps, gear of unknown quality, and permanent death. There is one save, and it is deleted when an expedition ends.
  * Auto-explore (`o`) and auto-fight (`Tab`), so safe ground is quick and the fights are the game.
  * Trick shots, the thing nihilurk does that Rogue never did: shoot a trap, a coin or a potion on the floor from across the room and it goes off over everything around it. Bursts chain. Only ranged attacks do it, and it catches you too if you stand close.
  * Seeded floors (`-s 1234`), and two ways down: nihil, who carries gear, or the lurk (`-b lurk`), who has claws and fur.
  * Four languages, English, Portuguese, Spanish and Haitian Creole, one binary each, picked at build time.

Install from crates.io (needs Rust 1.91 or newer) and play:

    cargo install nihilurk                  # install the English game
    cargo install nihilurk --no-default-features --features lang-pt    # or another language
    nihilurk                                # play

If the shell says `nihilurk: command not found`, the cargo bin directory is not on your `PATH`. `rustup` normally adds it, but a rebuilt dotfile or a distro Rust package may not. Add it, then open a new shell:

    # bash: append to ~/.bashrc    zsh: append to ~/.zshrc
    export PATH="$HOME/.cargo/bin:$PATH"

    # PowerShell on Windows, once (it sticks for your user)
    [Environment]::SetEnvironmentVariable("Path", "$env:USERPROFILE\.cargo\bin;" + [Environment]::GetEnvironmentVariable("Path", "User"), "User")

Until then, `~/.cargo/bin/nihilurk` (on Windows `%USERPROFILE%\.cargo\bin\nihilurk.exe`) runs it.

From a clone:

    cargo run -p nihilurk                  # play
    cargo run -p nihilurk -- -s 1234       # play a specific seed
    cargo test                           # everything


The game needs a terminal of at least 80 columns by 25 rows. In a smaller one the message log draws garbled letters. That is a known limit and will not be fixed.

Prebuilt downloads are on the [releases page](https://github.com/bae-alves/nihilurk/releases): Linux tarballs (x86_64 and aarch64, static musl), a Windows `.zip` (x86_64) and a macOS tarball (Apple Silicon). Unpack one and run `./nihilurk` (`nihilurk.exe` on Windows). Keep the files together: the dispatcher starts the `nihilurk-<lang>` binary next to it. The binaries are not signed, so macOS quarantines a browser download: `xattr -d com.apple.quarantine nihilurk*` clears it.

It runs on Windows: a tester built it there and played it. On macOS the prebuilt binary is built and started by CI (Apple Silicon), but nobody has played it there yet. If you try Windows or macOS, [open an issue](https://github.com/bae-alves/nihilurk/issues) and say what happened, good or bad.


Where things are
----------------

    MANUAL.md      how to play: controls, combat math, items, monsters.
    the site       https://bae-alves.github.io/nihilurk/, the manual and docs/, built from this repository.
    doc/nihilurk.6     the command reference. `man nihilurk` works from the prebuilt tarball (it ships `nihilurk.6`) and the AUR package; `cargo install` installs the binary only, so use `nihilurk -h` or `man ./doc/nihilurk.6` from a clone.
    docs/          how to add content to the game. Start at docs/README.md.
    CONTRIBUTING.md    reporting bugs, sending patches and translations, and the LLM rule.
    gdd.md         what nihilurk is trying to be. Design, not code.
    models/        the game: rules, content tables, ECS systems.
    strings/       every player-facing sentence, one file per language.
    engine/        the terminal front end: input, rendering, the loop.
    particle-core/ the particle arithmetic, `no_std` and dependency-free.
    compat/        the machine matrix. Does it still run on a Pi?

`compat/` is a test rig, kept out of the workspace's default members, so a bare `cargo build` or `cargo test` never compiles it:

    ./compat_test.sh                     # nihilurk, built and checked on six machines
    ./docs_style.sh                      # docs/, held to the house style
    ./aur_check.sh                       # aur/PKGBUILD, against the tag it pins and this working copy

Once per clone, `git config core.hooksPath .githooks` turns on a pre-commit check that runs `docs_style.sh` and refuses a commit that changes the turn schedule or an intent queue without touching a page under `docs/`. It also names the pages to update when you add a ring, a flag, a key or the like, and never refuses for that -- see `.githooks/pre-commit`.


Adding content
--------------

There is no pink dragon in nihilurk. The whole point is putting one in.

Open `models/src/monsters.rs`, find `BESTIARY`, add a line:

    MonsterDef::row("pink dragon", 'D', Color::Magenta, Chase,
                    8, 12, 2, 10, 2, 7)
        .grants(&[Grant::of::<FireImmune>()]),

That is: name, glyph, colour, how it moves, then hit points, attack die, attack bonus, armour die, armour bonus, and the shallowest floor it can appear on -- plus, chained on, the fact that fire does nothing to it.

    cargo build

That is the change. The pink dragon now populates floors from depth 7, is something a wand of polymorph can turn a foxbat into, something a scroll of create monster can summon, a legal bane for a vorpal weapon, and it comes back correctly out of a save file. Nothing else in the codebase had to be told it exists, because nothing else enumerates species.

Items are the same shape, in `models/src/catalog.rs` -- one table per kind, one row per thing:

    WeaponDef::new("quarterstaff", Color::DarkYellow, 7).missile(6),
    ArmorDef { name: "brigandine", color: Color::Grey, armor_die: 6 },

A row is a name, an appearance, and the components the thing carries into the world. There is no dragon code and no ring code: combat reads components and never learns what kind of thing put them there. A few categories need one edit more -- somebody has to say what drinking a new potion does -- and the docs say which, and why.

To see what you made without playing down to floor 7:

    NIHILURK_SPAWN="pink dragon" cargo run -p nihilurk
    cargo run -p nihilurk -- -content       # every name the game knows


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

    cat docs/explanation/ecs-in-nihilurk.md         # how bevy_ecs is used here
    cat docs/how-to/work-with-the-ecs.md        # and how to get past the borrow checker

`gdd.md` is a fifth thing: what nihilurk is trying to *be*. Design, not code.


License
-------

nihilurk is free software: you can redistribute it and/or modify it under the terms of the GNU General Public License as published by the Free Software Foundation, either version 3 of the License, or (at your option) any later version. It comes with no warranty. The full text is in `LICENSE`.
