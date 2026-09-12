Why the compatibility matrix is built this way
==============================================

    Audience       Anyone changing `compat/`, adding a machine to the
                   matrix, or wondering why a roguelike is being
                   compiled for a microcontroller.
    Prerequisites  You have run `./compat_test.sh` once.
    This is        Understanding. The recipe is
                   `../how-to/run-the-compat-pipeline.md`.

`gdd.md` makes a claim: roog runs on anything with a terminal. That is a
nice thing to say and an easy thing to be wrong about. Nobody notices a
32-bit build breaking, because nobody builds 32-bit; nobody notices the
binary gaining three megabytes, because the machine it was built on has
plenty; and nobody notices that the animation crawls on a Pi, because
the person who wrote it has a workstation.

The compatibility matrix exists to turn that claim into something that
fails loudly.


What is being measured
----------------------

Whether **roog** runs on the machine, and how well.

This is worth stating plainly because the rig it borrows -- `roog-perf`
-- was built to answer a different question, and the two are easy to
confuse. The perf pipeline asks *how fast is the particle layer*, and it
answers that by replaying Bad Apple at eight hundred motes a frame,
which is two to three orders of magnitude past anything the game
produces. That is the right load for finding a cliff on a workstation.

It is the wrong thing to judge a Raspberry Pi on. A Pi Zero that cannot
animate a music video may still play roog at a comfortable thirty frames
a second, because roog does not animate music videos -- it animates a
dozen sparks when something dies. Grading the Pi on the reel would fail
the machine and tell you nothing about the game.

So every row is run twice, and only one of the two counts:

    --load game   a real dungeon floor, generated from a fixed seed and
                  animated by the batches the game actually queues, one
                  per turn. This is roog running. It is the gate.
    --load reel   Bad Apple. This is the ceiling: how much harder the
                  machine could be pushed before the layer gives out.
                  Nothing is ever gated on it, and on the slowest rows
                  it is expected to lose.

`compat/src/verdict.rs` refuses to grade a reel run at all -- it returns
`Unknown` rather than a band -- because a reel verdict printed next to a
machine's name would be read as a claim about the game, and it is not
one.

Both runs use `--workload both`: the floor repainted, the live motes
composited over it, one diff and one flush over the result. That is what
`engine/src/view.rs` does every frame. Measuring the particle layer
alone would leave out the redraw, which on a slow machine is most of the
cost.


One table, three readers
------------------------

`compat/matrix.tsv` is the pipeline. One row per machine, and adding a
machine is adding a line -- nothing else holds the list.

Three things read it: `cross_build.sh` builds a row,
`stress_test_matrix.sh` runs a row with that row's limits applied, and
`compat/src/matrix.rs` compiles it in with `include_str!` so the
dashboard and the report can label what they are showing. Same decision
as the content tables, for the same reasons
(`adr-0001-tables-not-raws.md`): a malformed row is caught when the
pipeline is built rather than twenty minutes into a matrix run.

This is also why there is no `docker-compose.yml`. A compose file would
be a second copy of the machine list, written in a second syntax, and
the day the two disagree is the day the matrix quietly stops testing
what it says it tests. The runner is a shell script over the table
instead.

There are no Dockerfiles either, for a reason that is the whole point of
the musl decision below: there is nothing to install. A row's container
is an unmodified upstream image with one static binary mounted into it
read-only. Building a custom image per architecture would add a layer to
maintain, a thing to rebuild, and a place for a runtime dependency to
creep in unnoticed -- and if roog ever did need something installed
alongside it, the pipeline would be quietly hiding the fact that the
claim in `gdd.md` had stopped being true.

The images are pinned rather than floating on `:latest`, so the same
command on two machines a month apart is the same measurement.


Why every Linux row is musl
---------------------------

A glibc binary is bound to the glibc it was linked against. Build on a
current Arch and the result refuses to start on a Debian oldstable or an
older Pi OS, with a `GLIBC_2.38 not found` that has nothing to do with
roog and everything to do with the machine it was built on.

A `-musl` target links the whole libc in. What comes out is one file
with no `INTERP` segment and no runtime dependency of any kind -- copy
it to the machine and run it. That is the only kind of build that
honours the claim in `gdd.md`, so `matrix.rs` has a test that fails if a
`-gnu` row is ever added.

It costs a little size, because a static binary carries its own libc,
and that cost is on the report per target rather than hidden.


Why the limits are mean
-----------------------

A container that can use four modern cores tells you nothing about a Pi.
The point of a row is to be as slow as the thing it names.

So `pi-zero` gets `--cpus 0.4` and 512 MB, `potato` gets one core and
512 MB, and swap is disabled on every row -- `--memory-swap` is set
equal to `--memory`. That last one matters more than it looks: without
it Docker grants the same amount again as swap, and a row that should
have been killed for running out of memory quietly swaps instead, then
reports a frame time from a machine that does not exist.

An OOM kill is a result, not a pipeline error. It is the honest answer
to "will roog run in 512 MB".


Why each target gets its own target directory
--------------------------------------------

`compat/lib.sh` puts every triple's build in `target/cross/<triple>/`
rather than the workspace's own `target/`, and that is a bug fix rather
than housekeeping.

A cross build compiles two different things. The crate is compiled for
the target, and cargo already namespaces that by triple. Every
dependency's *build script* is compiled for the host, and those all land
in one shared `target/release/build/` with nothing in the path to say
which toolchain produced them.

So they get reused where they must not be:

    cargo build --release      # host: Arch, glibc 2.42
    ./compat_test.sh           # i686 image: Ubuntu 16.04, glibc 2.23

The second command finds the first one's build scripts already built,
runs them inside the older container, and they die on `weak version
GLIBC_2.29 not found`. The error names `thiserror`'s build script, which
roog does not depend on and which has nothing to do with the change
being tested. Two cross images of different vintages collide the same
way, so it is not enough to keep the host out of it.

One directory per triple means no two toolchains ever share a host
artifact. It costs disk and a full recompile the first time each target
is built, and it buys a pipeline whose answer does not depend on what
happened to be built before it -- which for a pipeline whose entire job
is to be trusted about other machines is the only acceptable trade.


The emulation tax, and how to read around it
--------------------------------------------

`x86_64` runs natively. So does `i686`: a 64-bit kernel runs 32-bit user
space directly, so the potato row is starved rather than emulated.

`aarch64` and `armv7` are emulated, by qemu-user through `binfmt_misc`.
Everything measured on those rows carries the emulator's tax, and that
tax is not a constant -- it is heavier on branchy code than on
arithmetic, so it does not divide out.

Which is why the report prints two CPU figures per row. The in-process
one is what roog saw of itself; the cgroup one is what Docker saw of the
whole container, qemu included. The gap between them is the tax. Read
the emulated rows as an upper bound on cost: real Graviton hardware is
faster than the row that stands for it, never slower.

The `cloud` row is the control. It is the host's own architecture with
no emulation in the way, which is what makes it the yardstick the
emulated rows are read against.


Why microcontrollers are in a matrix of machines that run the game
------------------------------------------------------------------

They are not, and the report says so in as many words.

roog draws with crossterm. crossterm drives a terminal. An ESP32 has no
terminal and no operating system to provide one, and no amount of
cross-compilation will change that. A bare-metal row failing to produce
a game is not a bug, because no game was ever being built.

What the bare rows prove is narrower. `particle-core` -- when a mote is
visible, which keyframe is showing, which glyph a beam segment draws --
is `no_std`, allocates nothing, and depends on no crate at all. It
compiles for `riscv32imc-unknown-none-elf` and `xtensa-esp32-none-elf`,
and `nostd_check.sh` proves it on demand.

That is worth having for a reason that has nothing to do with
microcontrollers: it is a standing structural check on the game. The
moment someone gives the particle arithmetic a `Vec`, a `String`, or a
call into libm, the bare-metal build breaks -- and it breaks in the
crate the game itself calls, not a copy of it. The ESP32 is a canary,
not a port.

The one seam is `ceil`. `f32::ceil` lives in `std` rather than `core`
because it lowers to a libm call, so a hosted build takes the intrinsic
and a bare-metal build takes a hand-rolled branch. Two branches means
they can disagree, and if they do, the bare-metal build is compiling
arithmetic the game does not run and the whole proof is worthless.
`particle-core/tests/parity.rs` holds that line, which is why
`nostd_check.sh` runs it *first*, before it compiles anything.

Both bare rows also name an SDK image with a shell in it, so the
toolchain can be poked at by hand rather than only through the script:

    ./compat/nostd_check.sh --shell esp32


Why the grading lives in Rust
-----------------------------

The scripts write raw numbers and never judge them. `roog-compat` does
all the judging, in `verdict.rs`, with tests.

The alternative is a threshold in awk, and then a second threshold in
the Rust report, and then a CI check with a third. Three answers to one
question is worse than no answer, because everyone believes whichever
one they saw first.

The bands themselves are deliberately forgiving in one direction and
strict in the other. `Janky` does not fail the pipeline: half the matrix
is hardware roog is *expected* to be slow on, and a gate that went red
on a Pi Zero every night would be muted within a week. `Unplayable` and
`DoesNotRun` do fail, because those mean a machine that used to run the
game no longer does -- which is the only thing this pipeline is really
for.


Why the dashboard reads files instead of Docker
-----------------------------------------------

`roog-compat` measures nothing. It reads `results.tsv`, the
per-container `stats-*.tsv`, and `footprint.tsv`, all of which the
scripts wrote.

Partly this is the same instinct as the perf rig's sampler thread: a
dashboard that shelled out to `docker stats` on every frame would be
spending the CPU of the very container it is measuring, on measuring it.
On a row capped at 0.4 of a core that is not a rounding error.

Mostly, though, it is that the numbers outlive the run. A matrix that
finished last week reads exactly like one still going, the same report
opens over a `target/compat/` copied off a build machine as a tarball,
and `--watch` is just the same reader on a 500 ms timer. Nothing about
the report needs the containers to still exist.


Who is to blame for the binary
------------------------------

`cross_build.sh` prints the size of the shipped binary per target, and
then attributes it per crate.

`cargo bloat` is the better tool for that and cannot be used here:
pointing it at a foreign target means having that target's linker on the
host, which is exactly what building in a container exists to avoid. So
the attribution is read off symbol sizes with `llvm-nm`, which reads
every architecture rustc can emit.

It reports *symbol* bytes, not file bytes -- the two differ by section
headers, alignment padding, relocations and the constant pool, so the
table's total comes in under the size printed beside it. Read the
shares, not the absolute totals.

The attribution needs symbols, and what ships is stripped. So the blame
stage builds the `profiling` profile as well: byte-for-byte the same
machine code with the symbols left in. Same trick `perf_test.sh` uses,
for the same reason.


See also
--------

    ../how-to/run-the-compat-pipeline.md   the recipe
    performance-testing.md                 the rig this borrows
    adr-0001-tables-not-raws.md            why the matrix is a table
    ../../gdd.md                           the portability claim itself
