Why the compatibility matrix is built this way
==============================================

    Audience       Anyone changing `compat/`, adding a machine to the
                   matrix, or wondering why a roguelike is being
                   compiled for a microcontroller.
    Prerequisites  You have run `./compat_test.sh` once.
    This is        Understanding. The recipe is
                   `../how-to/run-the-compat-pipeline.md`.

`gdd.md` makes a claim: nihilurk runs on anything with a terminal. That is a nice thing to say and an easy thing to be wrong about. Nobody notices a 32-bit build breaking, because nobody builds 32-bit; nobody notices the binary gaining three megabytes, because the machine it was built on has plenty; and nobody notices that the animation crawls on a Pi, because the person who wrote it has a workstation.

The compatibility matrix exists to turn that claim into something that fails loudly.


What is being measured
----------------------

Whether **nihilurk** runs on the machine, and how well.

This is worth stating plainly because the rig it borrows -- `nihilurk-perf` -- was built to answer a different question, and the two are easy to confuse. The perf pipeline asks *how fast is the particle layer*, and it answers that by replaying Bad Apple at eight hundred motes a frame, which is two to three orders of magnitude past anything the game produces. That is the right load for finding a cliff on a workstation.

It is the wrong thing to judge a Raspberry Pi on. A Pi Zero that cannot animate a music video may still play nihilurk at a comfortable thirty frames a second, because nihilurk does not animate music videos -- it animates a dozen sparks when something dies. Grading the Pi on the reel would fail the machine and tell you nothing about the game.

So a row that is actually executed -- a native row always, an emulated one only with `--exec-emulated`, see "why emulated rows are build-only" below -- is run three times, and only one of the three counts:

    --load game   a real dungeon floor, generated from a fixed seed and
                  animated by the batches the game actually queues, one
                  per turn. This is nihilurk running. It is the gate.
    --load reel   Bad Apple. This is the ceiling: how much harder the
                  machine could be pushed before the layer gives out.
                  Nothing is ever gated on it, and on the slowest rows
                  it is expected to lose.
    the screen    the redraw viewer, not headless, with a pty sized from
                  inside the container so it has a real terminal to draw
                  into. Not timed; it only has to come up and keep
                  drawing. See below.

`compat/src/verdict.rs` refuses to grade a reel run at all -- it returns `Unknown` rather than a band -- because a reel verdict printed next to a machine's name would be read as a claim about the game, and it is not one. The screen run is not graded either, and for a related reason: it answers "does this row draw", not "how fast", so it has no place in a frame-time verdict.

Both the game and reel runs use `--workload both`: the floor repainted, the live motes composited over it, one diff and one flush over the result. That is what `engine/src/view.rs` does every frame. Measuring the particle layer alone would leave out the redraw, which on a slow machine is most of the cost.


Why every row also gets a real screen
--------------------------------------

The game and reel runs are deliberately headless. `--headless` sends the frame's diff-and-flush into a counting sink instead of a real terminal -- see `perf/src/screen.rs` -- which is what makes the frame-time numbers reproducible run to run. A number that includes that day's pty latency, the host's TERM setting, or however Docker felt about allocating a terminal that morning would not be a number worth comparing against last week's.

But a pipeline that only ever ran nihilurk headless would be answering "how fast is the particle layer" and quietly never answering "does this machine's terminal stack let nihilurk draw on it at all" -- and for a game, that second question is not optional. crossterm and ratatui are real dependencies with real platform-specific behaviour (raw mode, cursor control, colour support), and none of that is exercised by a run that never touches a terminal.

So `stress_test_matrix.sh` also runs nihilurk-perf without `--headless`, `--workload both --load game`, inside a container started with `docker run -t`. `-t` allocates a pseudo-TTY even though nothing is attached to it (there is no `-i`, and nothing ever sends the container a keystroke) -- but `-t` only allocates the pty, it does not size it.

`--workload both` redraws, and `run_stress` in `perf/src/main.rs` sends every redrawing workload to `viewer::watch` in `perf/src/viewer.rs` rather than the dashboard -- see the parity comment on `Workload::redraws` in `perf/src/scene.rs`. `viewer::watch` takes the terminal directly and checks its size before it touches raw mode, refusing to run under its 80x26 minimum. An unresized pty reads back as 0x0 -- nothing at the other end ever sent it a window-change ioctl -- so `stress_test_matrix.sh`'s `run_screen_check` wraps the command as `sh -c 'stty rows 26 cols 80; exec "$@"'`, setting that size on the pty from inside the container before nihilurk-perf starts. That is the concrete reason a shell is not optional on these rows, beyond "nihilurk needs one to run" in the abstract: this is what it is actually used for.

Left alone, `viewer::watch`'s loop has no exit condition but a quit keypress -- right for a human at a keyboard, and wrong for a detached, keyboard-less container, which would just run forever. So it also honours `--frames`, exactly the way the headless run does: the same flag bounds both code paths, and the screen check runs to completion the same way the gate does, unattended.

There is nothing to parse out of a viewer run -- it has no textual report -- so the screen check feeds nothing into `results.tsv` or `nihilurk-compat`. Its only signal is the exit code: zero means the row drew its own screen under whatever emulation stands between it and the host, without crossterm or ratatui falling over. `--no-screen` skips it, for the same reason `--no-reel` does: sometimes the gate is all you want.


One table, three readers
------------------------

`compat/matrix.tsv` is the pipeline. One row per machine, and adding a machine is adding a line -- nothing else holds the list.

Three things read it: `cross_build.sh` builds a row, `stress_test_matrix.sh` runs a row with that row's limits applied, and `compat/src/matrix.rs` compiles it in with `include_str!` so the dashboard and the report can label what they are showing. Same decision as the content tables, for the same reasons (`adr-0001-tables-not-raws.md`): a malformed row is caught when the pipeline is built rather than twenty minutes into a matrix run.

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

A row's `class` of `linux` is a claim: this machine has an operating system, a terminal and a shell, and nihilurk can therefore run on it. That is not a decorative distinction from `bare` -- it is the one thing that determines whether `stress_test_matrix.sh` will even try to execute a row, and it is checked, not assumed. `check_has_shell` in `compat/lib.sh` runs `/bin/sh -c 'echo shell-ok'` in the row's image before the game or reel run starts, and a row that fails it is skipped with the same `skipped` status a missing qemu interpreter gets. An image with no shell in it cannot host a terminal program no matter how the binary was built, so there is no point spending a build and a run finding that out -- `bare` already exists for "compiled, never executed", and a `linux` row that cannot prove a shell is effectively a `bare` row with the wrong label.

This is also why the ARM lineup is Raspberry Pi boards -- `pi-zero`, `pi2`, `pi3`, `pi4`, `cm` -- rather than a longer list of cloud SKUs. `graviton` earns its place because an M-series Mac's Linux VM is a common, real way nihilurk gets built and played, but a cloud instance is otherwise an abstraction over hardware someone else runs. A Pi is hardware: it is a board with a shell on it that someone might actually plug a keyboard into, which is closer to what "does nihilurk run on that machine" is asking than a t4g-family instance size is. `pi2` and `pi3`/`pi4`/`cm` share `pi-zero` and `graviton`'s existing triples (`armv7-unknown-linux-musleabihf` and `aarch64-unknown-linux-musl` respectively) rather than needing new `Cross.toml` entries -- the point of adding them was a wider, more realistic spread of real ARM boards, not a new architecture.


Why the limits are mean
-----------------------

A container that can use four modern cores tells you nothing about a Pi. The point of a row is to be as slow as the thing it names.

So `pi-zero` gets `--cpus 0.4` and 512 MB, `potato` gets one core and 512 MB, and swap is disabled on every row -- `--memory-swap` is set equal to `--memory`. That last one matters more than it looks: without it Docker grants the same amount again as swap, and a row that should have been killed for running out of memory quietly swaps instead, then reports a frame time from a machine that does not exist.

An OOM kill is a result, not a pipeline error. It is the honest answer to "will nihilurk run in 512 MB".


Why each target gets its own target directory
--------------------------------------------

`compat/lib.sh` puts every triple's build in `target/cross/<triple>/` rather than the workspace's own `target/`, and that is a bug fix rather than housekeeping.

A cross build compiles two different things. The crate is compiled for the target, and cargo already namespaces that by triple. Every dependency's *build script* is compiled for the host, and those all land in one shared `target/release/build/` with nothing in the path to say which toolchain produced them.

So they get reused where they must not be:

    cargo build --release      # host: Arch, glibc 2.42
    ./compat_test.sh           # i686 image: Ubuntu 16.04, glibc 2.23

The second command finds the first one's build scripts already built, runs them inside the older container, and they die on `weak version GLIBC_2.29 not found`. The error names `thiserror`'s build script, which nihilurk does not depend on and which has nothing to do with the change being tested. Two cross images of different vintages collide the same way, so it is not enough to keep the host out of it.

One directory per triple means no two toolchains ever share a host artifact. It costs disk and a full recompile the first time each target is built, and it buys a pipeline whose answer does not depend on what happened to be built before it -- which for a pipeline whose entire job is to be trusted about other machines is the only acceptable trade.


The emulation tax, and how to read around it
--------------------------------------------

`x86_64` runs natively. So does `i686`: a 64-bit kernel runs 32-bit user space directly, so the potato row is starved rather than emulated.

`aarch64` and `armv7` *can* run under a qemu-user-static interpreter `stress_test_matrix.sh` fetches and runs directly -- no `binfmt_misc`, no `--privileged` -- when asked to with `--exec-emulated`. It can do that because every binary the matrix runs is static: the interpreter never needs a foreign sysroot, only to translate the guest's syscalls, so it works as the container's own command on a container built for the *host's* architecture. Anything measured that way still carries the emulator's tax, and that tax is not a constant -- it is heavier on branchy code than on arithmetic, so it does not divide out.

Which is why, on the runs where it applies, the report prints two CPU figures per row. The in-process one is what nihilurk saw of itself; the cgroup one is what Docker saw of the whole container, qemu included. The gap between them is the tax. Read an emulated row actually executed this way as an upper bound on cost: real Graviton hardware is faster than the row that stands for it, never slower.

The `cloud` row is the control. It is the host's own architecture with no emulation in the way, which is what makes it the yardstick an executed emulated row is read against.

By default, though, no `qemu` row is executed at all -- see "why emulated rows are build-only" below.


Why emulated rows are build-only
---------------------------------

qemu-user-static's job is narrower than it sounds: it translates the guest's *syscalls*, which is what makes running a static binary this way safe and simple in the first place. It does not promise to get every architecture-specific `ioctl` right, and the ones a terminal program needs -- `TIOCGWINSZ` to ask the window's size, the raw-mode toggles underneath `enable_raw_mode` -- are exactly the kind that varies by architecture and is easy for a translation layer to get subtly wrong. nihilurk draws through crossterm, which leans on precisely those ioctls, so an emulated row running nihilurk is really testing two things at once: does nihilurk run here, and does this qemu build's ioctl translation hold up -- and there is no clean way to tell the two apart from a container that exited 1.

That is a bad trade for a compat gate. A row reporting `does not run` because of the second question, not the first, is a false alarm dressed as a real one: it reads exactly like nihilurk having broken on that architecture, and by the time someone has read the log and ruled out their own code, the false alarm has cost more than a true one would have.

So `stress_test_matrix.sh` does not execute a `qemu` row by default. What it still does, unconditionally, is what `cross_build.sh` already proves: a full cross-compile with a real Rust toolchain, checked afterward to be a correctly statically-linked binary for that architecture (see "why every Linux row is musl"). That is not a token gesture at "we still checked something" -- cross-compiling nihilurk is, by every measure that matters here (time, memory, code paths exercised), a heavier task than nihilurk's own frame loop ever asks of the machine it is built for. A row that survives that build is a real claim about the hardware, made without leaning on qemu's ioctl handling at all.

An unexecuted emulated row is graded `Band::Builds` -- and deliberately not `Band::Unknown`, even though both are non-failing and both sit below every band a real run can produce. `Unknown` means compat/ has nothing to say about a row: no shell in its image, no qemu interpreter fetched, no binary built. `Builds` means the opposite -- a specific, positive claim was made and backed by a real cross-compile -- and a row's verdict column has to say which of those two happened, not paper over the difference with the same dash. Reusing `Unknown` for "chose not to run it" was tried first and reads exactly as confusing as it sounds: the table would show a row this pipeline is actively vouching for next to a row it knows nothing about, both as `-`, both glossed "not measured". That is what "the table should be enough" actually requires -- not fewer bands, but the *right* one on each row.

`verdict::grade` checks `Status::BuildOnly` before it would otherwise fall through to `DoesNotRun`, exactly the way it already special-cased `Status::Skipped`. Both bands sit below every real verdict in `Band`'s severity order -- below `Plays`, not just below `DoesNotRun` -- so neither can mask a real failure elsewhere in `overall()`, and neither can get picked as the "worst machine" ahead of a row that actually ran and lost. `Band::Builds` sits directly above `Unknown`: a matrix that is all build-only rows reports `builds` as its overall verdict, not `-`, because that is the honest, positive answer for the case where nothing was executed but everything that was attempted compiled clean.

`--exec-emulated` is the escape hatch, unchanged from before this default existed: point it at real hardware, or at a qemu build you trust for this, and every `qemu` row runs exactly the way `cloud` and `potato` always have -- graded `Plays`/`Playable`/`Janky`/`Unplayable`/ `DoesNotRun` like any executed row, never `Builds`, because at that point a real measurement exists and `Builds` would be the smaller claim.


Why microcontrollers are in a matrix of machines that run the game
------------------------------------------------------------------

They are not, and the report says so in as many words.

nihilurk draws with crossterm. crossterm drives a terminal. An ESP32 has no terminal and no operating system to provide one, and no amount of cross-compilation will change that. A bare-metal row failing to produce a game is not a bug, because no game was ever being built.

What the bare rows prove is narrower. `particle-core` -- when a mote is visible, which keyframe is showing, which glyph a beam segment draws, and which way the screen shake throws the map on a given frame -- is `no_std`, allocates nothing, and depends on no crate at all. It compiles for `riscv32imc-unknown-none-elf` and `xtensa-esp32-none-elf`, and `nostd_check.sh` proves it on demand.

That is worth having for a reason that has nothing to do with microcontrollers: it is a standing structural check on the game. The moment someone gives the particle arithmetic a `Vec`, a `String`, or a call into libm, the bare-metal build breaks -- and it breaks in the crate the game itself calls, not a copy of it. The ESP32 is a canary, not a port.

The one seam is `ceil`. `f32::ceil` lives in `std` rather than `core` because it lowers to a libm call, so a hosted build takes the intrinsic and a bare-metal build takes a hand-rolled branch. Two branches means they can disagree, and if they do, the bare-metal build is compiling arithmetic the game does not run and the whole proof is worthless. `particle-core/tests/parity.rs` holds that line, which is why `nostd_check.sh` runs it *first*, before it compiles anything.

Both bare rows also name an SDK image with a shell in it, so the toolchain can be poked at by hand rather than only through the script:

    ./compat/nostd_check.sh --shell esp32


Why the grading lives in Rust
-----------------------------

The scripts write raw numbers and never judge them. `nihilurk-compat` does all the judging, in `verdict.rs`, with tests.

The alternative is a threshold in awk, and then a second threshold in the Rust report, and then a CI check with a third. Three answers to one question is worse than no answer, because everyone believes whichever one they saw first.

The bands themselves are deliberately forgiving in one direction and strict in the other. `Janky` does not fail the pipeline: half the matrix is hardware nihilurk is *expected* to be slow on, and a gate that went red on a Pi Zero every night would be muted within a week. `Unplayable` and `DoesNotRun` do fail, because those mean a machine that used to run the game no longer does -- which is the only thing this pipeline is really for.


Why the dashboard reads files instead of Docker
-----------------------------------------------

`nihilurk-compat` measures nothing. It reads `results.tsv`, the per-container `stats-*.tsv`, and `footprint.tsv`, all of which the scripts wrote.

Partly this is the same instinct as the perf rig's sampler thread: a dashboard that shelled out to `docker stats` on every frame would be spending the CPU of the very container it is measuring, on measuring it. On a row capped at 0.4 of a core that is not a rounding error.

Mostly, though, it is that the numbers outlive the run. A matrix that finished last week reads exactly like one still going, the same report opens over a `target/compat/` copied off a build machine as a tarball, and `--watch` is just the same reader on a 500 ms timer. Nothing about the report needs the containers to still exist.


Who is to blame for the binary
------------------------------

`cross_build.sh` prints the size of the shipped binary per target, and then attributes it per crate.

`cargo bloat` is the better tool for that and cannot be used here: pointing it at a foreign target means having that target's linker on the host, which is exactly what building in a container exists to avoid. So the attribution is read off symbol sizes with `llvm-nm`, which reads every architecture rustc can emit.

It reports *symbol* bytes, not file bytes -- the two differ by section headers, alignment padding, relocations and the constant pool, so the table's total comes in under the size printed beside it. Read the shares, not the absolute totals.

The attribution needs symbols, and what ships is stripped. So the blame stage builds the `profiling` profile as well: byte-for-byte the same machine code with the symbols left in. Same trick `perf_test.sh` uses, for the same reason.


See also
--------

    ../how-to/run-the-compat-pipeline.md   the recipe
    performance-testing.md                 the rig this borrows
    adr-0001-tables-not-raws.md            why the matrix is a table
    ../../gdd.md                           the portability claim itself
