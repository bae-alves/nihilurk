Why the compatibility matrix is built this way
==============================================

    Audience       Anyone changing `compat/`, adding a machine to the
                   matrix, or wondering why a roguelike is being
                   compiled for a microcontroller.
    Prerequisites  You have run `./compat_test.sh` once.
    This is        Understanding. The recipe is
                   `../how-to/run-the-compat-pipeline.md`.

`gdd.md` makes a claim: nihilurk runs on anything with a terminal. That is a nice thing to say and an easy thing to be wrong about. Nobody notices a 32-bit build breaking, because nobody builds 32-bit, and nobody notices the binary gaining three megabytes, because the machine it was built on has plenty.

The compatibility matrix exists to turn that claim into something that fails loudly.


What is being checked
----------------------

If it builds, the image it runs in has a shell, and the target is std (every `linux` row here is), nihilurk can run there.

That claim used to be checked by driving the game under load inside a resource-capped container -- a real dungeon floor for the gate, Bad Apple at eight hundred motes a frame for a ceiling, and a pty-attached run to prove the terminal stack actually draws -- borrowed wholesale from a stress rig built to find performance cliffs on a workstation. It was the wrong tool for this job even when it worked: nihilurk has no workload that asks a machine to be fast. Every feel-layer effect is bounded -- a dozen sparks when something dies, not a music video -- and a Pi that could not keep up with Bad Apple might still play nihilurk at a comfortable thirty frames a second. Grading a Pi on a synthetic ceiling it was never going to face answered a question nobody asked, at the cost of a stress rig with its own CPU caps, memory caps, qemu interpreter plumbing and pty sizing to keep working.

What the matrix actually needs to prove is narrower, and it was hiding in plain sight the whole time: `cross_build.sh` already cross-compiles a full Rust toolchain for the target, which is by every measure that matters (time, memory, code paths exercised) a heavier task than nihilurk's own frame loop will ever ask of that machine. A machine that can do that can run nihilurk. The two things left to check are the ones the build itself cannot prove -- does the image have a shell to launch the binary from, and does the binary actually start there -- and `run_check.sh` checks exactly those two things, with `engine -content`: a headless print-and-exit that needs nothing from the environment beyond a shell to launch it from.


One table, two readers
-----------------------

`compat/matrix.tsv` is the pipeline. One row per machine, and adding a machine is adding a line -- nothing else holds the list.

Two things read it: `cross_build.sh` builds a row, and `run_check.sh` checks it. `compat/src/matrix.rs` compiles the same file in with `include_str!` so the report can label what it is showing. Same decision as the content tables, for the same reasons (`adr-0001-tables-not-raws.md`): a malformed row is caught when the pipeline is built rather than in the middle of a run.

This is also why there is no `docker-compose.yml`. A compose file would be a second copy of the machine list, written in a second syntax, and the day the two disagree is the day the matrix quietly stops testing what it says it tests. The runner is a shell script over the table instead.

There are no Dockerfiles either, for a reason that is the whole point of the musl decision below: there is nothing to install. A row's container is an unmodified upstream image with one static binary mounted into it read-only. Building a custom image per architecture would add a layer to maintain, a thing to rebuild, and a place for a runtime dependency to creep in unnoticed -- and if nihilurk ever did need something installed alongside it, the pipeline would be quietly hiding the fact that the claim in `gdd.md` had stopped being true.

The images are pinned rather than floating on `:latest`, so the same command on two machines a month apart is the same measurement.


Why every Linux row is musl
---------------------------

A glibc binary is bound to the glibc it was linked against. Build on a current Arch and the result refuses to start on a Debian oldstable or an older Pi OS, with a `GLIBC_2.38 not found` that has nothing to do with nihilurk and everything to do with the machine it was built on.

A `-musl` target links the whole libc in. What comes out is one file with no `INTERP` segment and no runtime dependency of any kind -- copy it to the machine and run it. That is the only kind of build that honours the claim in `gdd.md`, so `matrix.rs` has a test that fails if a `-gnu` row is ever added.

It costs a little size, because a static binary carries its own libc, and that cost is on the report per target rather than hidden.


Why every `linux` row must have a shell
----------------------------------------

A row's `class` of `linux` is a claim: this machine has an operating system, a terminal and a shell, and nihilurk can therefore run on it. That is not a decorative distinction from `bare` -- it is the one thing that determines whether `run_check.sh` will even try to execute a row, and it is checked, not assumed. `check_has_shell` in `compat/lib.sh` runs `/bin/sh -c 'echo shell-ok'` in the row's image before the binary runs, and a row that fails it fails the pipeline -- an image with no shell in it cannot host a terminal program no matter how the binary was built, and `bare` already exists for "compiled, never executed"; a `linux` row that cannot prove a shell is effectively a `bare` row with the wrong label.

This is also why the ARM lineup is Raspberry Pi boards -- `pi-zero`, `pi2`, `pi3`, `pi4`, `cm` -- rather than a longer list of cloud SKUs. `graviton` earns its place because an M-series Mac's Linux VM is a common, real way nihilurk gets built and played, but a cloud instance is otherwise an abstraction over hardware someone else runs. A Pi is hardware: it is a board with a shell on it that someone might actually plug a keyboard into, which is closer to what "does nihilurk run on that machine" is asking than a t4g-family instance size is. `pi2` and `pi3`/`pi4`/`cm` share `pi-zero` and `graviton`'s existing triples (`armv7-unknown-linux-musleabihf` and `aarch64-unknown-linux-musl` respectively) rather than needing new `Cross.toml` entries -- the point of adding them was a wider, more realistic spread of real ARM boards, not a new architecture.


Why `engine -content` and not a real session
----------------------------------------------

The check has to run under qemu-user-static for most of the matrix (see below), and it has to run unattended, so it needs to be something that starts, does something observable, and exits on its own. A real game session does none of that: it waits on a keyboard.

`engine -content` prints the game's content index and exits -- see the comment above `list_content` in `engine/src/main.rs`, which is explicit that this never touches the terminal's alternate screen. That is what makes it safe under emulation: there is no pty to size and no terminal ioctl (`TIOCGWINSZ`, raw-mode toggles) for a translation layer to get subtly wrong, only a shell to launch the binary from and stdout to read back. If it prints something and exits 0, the row can run nihilurk -- the terminal-drawing half of that claim is exactly what cross-compiling and starting the binary already stand behind, and testing crossterm's raw-mode behaviour under an architecture qemu-user does not fully emulate would be answering a question about qemu, not about nihilurk.


Why each target gets its own target directory
-----------------------------------------------

`compat/lib.sh` puts every triple's build in `target/cross/<triple>/` rather than the workspace's own `target/`, and that is a bug fix rather than housekeeping.

A cross build compiles two different things. The crate is compiled for the target, and cargo already namespaces that by triple. Every dependency's *build script* is compiled for the host, and those all land in one shared `target/release/build/` with nothing in the path to say which toolchain produced them.

So they get reused where they must not be:

    cargo build --release      # host: Arch, glibc 2.42
    ./compat_test.sh           # i686 image: Ubuntu 16.04, glibc 2.23

The second command finds the first one's build scripts already built, runs them inside the older container, and they die on `weak version GLIBC_2.29 not found`. The error names `thiserror`'s build script, which nihilurk does not depend on and which has nothing to do with the change being tested. Two cross images of different vintages collide the same way, so it is not enough to keep the host out of it.

One directory per triple means no two toolchains ever share a host artifact. It costs disk and a full recompile the first time each target is built, and it buys a pipeline whose answer does not depend on what happened to be built before it -- which for a pipeline whose entire job is to be trusted about other machines is the only acceptable trade.


Running under emulation
------------------------

`x86_64` runs natively. So does `i686`: a 64-bit kernel runs 32-bit user space directly, so the potato row is starved rather than emulated.

`aarch64` and `armv7` run under a qemu-user-static interpreter `run_check.sh` fetches and runs directly -- no `binfmt_misc`, no `--privileged`. It can do that because every binary the matrix runs is static: the interpreter never needs a foreign sysroot, only to translate the guest's syscalls, so it works as the container's own command on a container built for the *host's* architecture.

qemu-user-static's job is exactly that -- syscall translation -- and nothing more, which is what makes it safe to lean on for `engine -content`: the check makes no ioctl calls a translation layer could get wrong, because it never touches the terminal. That was not true of the old game-load and reel runs, which is the reason those existed as a separate, opt-in `--exec-emulated` path rather than the default; there is no equivalent carve-out needed here.

The `cloud` row is the control: the host's own architecture, no emulation in the way.


Why microcontrollers are in a matrix of machines that run the game
--------------------------------------------------------------------

They are not, and the report says so in as many words.

nihilurk draws with crossterm. crossterm drives a terminal. An ESP32 has no terminal and no operating system to provide one, and no amount of cross-compilation will change that. A bare-metal row failing to produce a game is not a bug, because no game was ever being built.

What the bare rows prove is narrower. `particle-core` -- when a mote is visible, which keyframe is showing, which glyph a beam segment draws, and which way the screen shake throws the map on a given frame -- is `no_std`, allocates nothing, and depends on no crate at all. It compiles for `riscv32imc-unknown-none-elf` and `xtensa-esp32-none-elf`, and `nostd_check.sh` proves it on demand.

That is worth having for a reason that has nothing to do with microcontrollers: it is a standing structural check on the game. The moment someone gives the particle arithmetic a `Vec`, a `String`, or a call into libm, the bare-metal build breaks -- and it breaks in the crate the game itself calls, not a copy of it. The ESP32 is a canary, not a port.

The one seam is `ceil`. `f32::ceil` lives in `std` rather than `core` because it lowers to a libm call, so a hosted build takes the intrinsic and a bare-metal build takes a hand-rolled branch. Two branches means they can disagree, and if they do, the bare-metal build is compiling arithmetic the game does not run and the whole proof is worthless. `particle-core/tests/parity.rs` holds that line, which is why `nostd_check.sh` runs it *first*, before it compiles anything.

Both bare rows also name an SDK image with a shell in it, so the toolchain can be poked at by hand rather than only through the script:

    ./compat/nostd_check.sh --shell esp32


Why the grading lives in Rust
-----------------------------

The scripts write raw status strings and never judge them beyond that. `nihilurk-compat` turns them into a verdict, in `results.rs`, with tests.

The alternative is a threshold in awk, and then a second threshold in the Rust report, and then a CI check with a third. Three answers to one question is worse than no answer, because everyone believes whichever one they saw first. There is less to disagree about now than there used to be -- the verdict is `runs`, `no shell`, `does not run` or `-`, not a performance band -- but the reason for keeping it in one place, checked by tests, has not changed.


Who is to blame for the binary
-------------------------------

`cross_build.sh` prints the size of the shipped binary per target, and then attributes it per crate.

`cargo bloat` is the better tool for that and cannot be used here: pointing it at a foreign target means having that target's linker on the host, which is exactly what building in a container exists to avoid. So the attribution is read off symbol sizes with `llvm-nm`, which reads every architecture rustc can emit.

It reports *symbol* bytes, not file bytes -- the two differ by section headers, alignment padding, relocations and the constant pool, so the table's total comes in under the size printed beside it. Read the shares, not the absolute totals.

The attribution needs symbols, and what ships is stripped. So the blame stage builds the `profiling` profile as well: byte-for-byte the same machine code with the symbols left in.


See also
--------

    ../how-to/run-the-compat-pipeline.md   the recipe
    adr-0001-tables-not-raws.md            why the matrix is a table
    ../../gdd.md                           the portability claim itself
