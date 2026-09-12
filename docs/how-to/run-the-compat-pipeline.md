How to run the compatibility pipeline
=====================================

    Audience       Anyone who wants to know whether roog still runs on
                   a Pi, a 32-bit netbook, or a Graviton instance --
                   and anyone who just made the binary bigger.
    Prerequisites  Docker, and `cargo install cross`. QEMU hooks for
                   the ARM rows; the script installs them for you.
                   Everything else is optional and is skipped with a
                   note where it is missing.
    Result         One line per machine saying whether roog is playable
                   there, the size of the binary for each, which crate
                   is responsible for those bytes, and a dashboard of
                   what each container's CPU and memory did.

One script:

    ./compat_test.sh

Read no further if it comes back green.


Quick commands
--------------

    ./compat_test.sh                  everything (~20 min cold)
    ./compat_test.sh --targets cloud  one machine
    ./compat_test.sh --quick          gate only; skip the ceiling and screen check
    ./compat_test.sh --no-build       reuse what is already built
    ./compat_test.sh --targets pi-zero --no-bare     one row, no ESP32
    ./compat_test.sh --no-bare        skip the microcontroller check
    ./compat_test.sh --no-screen      skip the pty/screen check on its own
    ./compat_test.sh --gui            finish in the dashboard
    ./compat_test.sh --help

The three phases on their own:

    ./compat/cross_build.sh           build for every machine + blame
    ./compat/nostd_check.sh           the microcontroller rows
    ./compat/stress_test_matrix.sh    run it on every machine

The dashboard and the report:

    target/release/roog-compat            the dashboard
    target/release/roog-compat --watch    ...while the matrix runs
    target/release/roog-compat report     plain text, for CI and pipes
    target/release/roog-compat gate       one line, and an exit code

Reports, logs and metrics land in `target/compat/`. The binaries
themselves land in `target/cross/<triple>/<triple>/release/`.

Each stress run replaces `target/compat/results.tsv` rather than adding
to it, `--targets` included. So a one-row run reports one row, and the
machines it skipped show `-`. That is deliberate: a report mixing rows
measured against two different builds of the game, with nothing on the
page saying which was which, is worse than a report with gaps in it.


The machines
------------

`compat/matrix.tsv` is the list, and it is the only list. One row per
machine, tab-separated; adding a machine is adding a line.

    cloud      x86_64 musl     1 cpu    512m   t3.micro, and the control
    graviton   aarch64 musl    2 cpu      1g   AWS Graviton, an M-series VM
    pi-zero    armv7 musl    0.4 cpu    512m   Raspberry Pi Zero 2 W
    pi2        armv7 musl      1 cpu      1g   Raspberry Pi 2 B
    pi3        aarch64 musl  1.2 cpu      1g   Raspberry Pi 3 B
    pi4        aarch64 musl  1.5 cpu      2g   Raspberry Pi 4 B
    cm         aarch64 musl  1.5 cpu      1g   Compute Module 4 (1GB Lite)
    potato     i686 musl       1 cpu    512m   32-bit netbook, ca. 2008
    esp32c3    riscv32imc         -         -  ESP32-C3, compiled only
    esp32      xtensa-esp32       -         -  ESP32, compiled only

The limits are handed straight to Docker, and swap is disabled, so the
memory cap is a real ceiling. `cloud` and `potato` run on the host CPU.
Every other `linux` row is a `qemu` row: cross-compiled and
static-link-checked always, but only actually run under a
qemu-user-static interpreter -- fetched by the pipeline on its own, no
`binfmt_misc`, no `--privileged` -- when `--exec-emulated` asks for it.

Every `linux` row's image is checked for a shell before it is run --
that is not optional, roog cannot start without a real terminal under
it -- and a row that fails the check is skipped rather than run. See
"why every `linux` row must have a shell" in
`../explanation/cross-platform-testing.md`.

An emulated (`qemu`) row is not run by default: it is still
cross-compiled and static-link-checked by `cross_build.sh`, and that is
treated as sufficient, because roog's own frame loop asks far less of a
machine than the Rust toolchain that just cross-compiled it does.
qemu-user-static's syscall translation also does not reliably extend to
the architecture-specific ioctls crossterm needs (terminal size, raw
mode), so there is a real reason not to lean on it here beyond the
build. A build failure still fails the pipeline -- that part is
unchanged and non-negotiable. `--exec-emulated` opts back into real
execution for anyone with actual hardware, or a qemu build, to check it
against. See "why emulated rows are build-only" in
`../explanation/cross-platform-testing.md`.

The two `esp32*` rows are never executed, under any flag: they are
`no_std`, and there is no "run it for real" for a target with no OS to
run it under. See "The microcontroller rows" below.


1. Does roog still run everywhere?
----------------------------------

1. Run the pipeline:

       ./compat_test.sh

2. Read the first table. One line per machine:

       machine   exec       binary      mean       p99  drops  peak rss  verdict
       cloud     native    1.8 MiB    0.04ms    0.15ms      0   1.7 MiB  plays
       graviton  qemu      1.6 MiB         -         -      -         -  builds
       pi-zero   qemu      1.6 MiB         -         -      -         -  builds
       potato    native    1.8 MiB    0.05ms    0.07ms      0   1.6 MiB  plays

   `builds` is the verdict for every emulated row by default: the table
   is the whole story for it, not a hedge -- cross-compiled, statically
   linked (the binary column is a real size, not a placeholder), and
   trusted on that basis. It is not the same as a dash. A dash means
   compat/ has nothing to say about the row at all -- a missing shell in
   its image, or Docker failing to pull the qemu-user-static interpreter
   for a row run with `--exec-emulated` -- and is rare precisely because
   `builds` covers the routine, expected case. Neither fails the
   pipeline; only a row that was actually run and lost does. The verdict
   is the column that matters:

       builds        cross-compiled and statically linked; not run here
       plays         the frame is done in under half the budget
       playable      keeps up; roog is playable on this hardware
       janky         over budget, or dropping frames you can see
       unplayable    cannot hold the frame rate at all
       does not run  the container failed, was killed, or OOMed

3. `builds`, `plays`, `playable` and `janky` all exit 0. `unplayable`
   and `does not run` fail the pipeline, because they mean a machine
   that used to run roog no longer does.

A `!` beside a verdict means the run finished but came within 10% of
that machine's memory cap. Not a failure -- it passed -- but it is one
dungeon level away from not passing.


2. What is being measured
-------------------------

roog. A real dungeon floor from a fixed seed, animated by the batches
the game actually queues, one per turn, with the whole frame drawn:
floor repainted, motes composited over it, one diff and one flush.

Every row is also run against the Bad Apple reel, at ~800 motes a
frame. That is the *ceiling*, printed under "THE CEILING", and nothing
is graded on it:

       machine        mean       p99  drops  headroom over the game's load
       cloud        0.28ms    0.79ms      0  7x the work
       potato       0.38ms    0.69ms      0  8x the work

A machine that cannot keep up with Bad Apple may still play roog
perfectly well, because roog does not animate music videos. If a
ceiling row says `timeout -- too slow for the reel, which is allowed`,
that is not a failure.

`--quick` skips the ceiling and roughly halves the runtime.

An executed row also gets one more pass: the same game load, run through
roog-perf's redraw viewer instead of the numeric report, with a
pseudo-terminal attached and sized from inside the container
(`docker run -t`, then `stty`) so crossterm has an actual terminal to
draw into instead of the counting sink the two runs above use. It is not
timed and prints no numbers -- it only has to come up and keep drawing
for its frame count without falling over. A `bad` line under a row's
name naming `screen` is that check failing; `--no-screen` skips it, same
as `--no-reel` skips the ceiling. See "why every row also gets a real
screen" in
`../explanation/cross-platform-testing.md`.

If you shorten the ceiling run with `--reel-frames`, keep it above 300.
Bad Apple opens on a nearly black screen, so a 60-frame run measures
the titles and reports a machine with far more headroom than it has.
The script warns you.


3. How big is it, and whose fault is that?
------------------------------------------

Phase 1 prints it per target, for the binary that actually ships --
`--release`, fat LTO, one codegen unit, symbols stripped:

       target     triple                                 game      bytes        rig
       cloud      x86_64-unknown-linux-musl           1.8 MiB    1934384    1.6 MiB
       potato     i686-unknown-linux-musl             1.8 MiB    1846016    1.6 MiB

The exact byte count is there because this is a table you diff against
the last run, and the 32-bit rows come out about 4% smaller -- which
rounds to the same "1.8 MiB" and would otherwise be invisible.

Do not read too much into the last few kilobytes. A fat-LTO build is
not reproducible to the byte, and the same source rebuilt can move by a
page either way. A crate's *share* moving, or a target gaining tens of
kilobytes, is the signal.

Then it attributes those bytes per crate:

       crate                                   bytes    share
       models                                 477599    28.7%
       bevy_ecs                               343492    20.7%
       engine                                 210844    12.7%
       core                                   159366     9.6%
       std                                     98965     6.0%
       libc + compiler builtins                65823     4.0%
       total in symbols                      1662671

Those are *symbol* bytes, not file bytes -- the difference is section
headers, padding, relocations and the constant pool -- so the total
comes in under the size printed above it. Read the shares.

The full table per machine is in `target/compat/blame-<machine>.txt`.
`--no-blame` skips the stage; `--top 20` lists more crates.

For the host binary specifically, `cargo bloat` via `./perf_test.sh` is
the better tool. It cannot be pointed at a foreign target without that
target's linker, which is the whole reason this uses `llvm-nm` instead.


4. Watching it run
------------------

The dashboard, in another terminal, while the matrix is going:

    target/release/roog-compat --watch

One row per machine with the verdict, and below it what Docker saw the
selected container doing -- CPU against the row's core budget, memory
against the row's cap, both as time series.

    up / down   select a machine
    l           graph the reel instead of the game
    r           reload now
    q           quit

Or `./compat_test.sh --gui` to land in it when the run finishes. It
needs a 92x24 terminal; `roog-compat report` is the same information as
text and needs nothing.

Note which numbers are which. The table's `peak rss` is what roog saw
of itself, inside the container. The graphs are the cgroup's, from
outside, and on a `qemu` row they include the emulator. The gap between
them is the emulation tax.


5. The microcontroller rows
---------------------------

    ./compat/nostd_check.sh

This does not build roog for an ESP32. roog draws with crossterm,
crossterm needs a terminal, and a microcontroller has neither a
terminal nor an OS to provide one.

What it checks is that `particle-core` -- the arithmetic of the
particle layer, which the game itself calls -- still compiles with no
operating system under it, for `riscv32imc-unknown-none-elf` and
`xtensa-esp32-none-elf`. It is a standing structural check: the moment
that arithmetic is given a `Vec`, a `String`, or a libm call, this goes
red.

It runs the parity test first, which is what makes the rest mean
anything -- see the explanation page.

RISC-V is a rustup target, so that row is a one-second `cargo check` on
the host. Xtensa needs Espressif's rustc fork, so that row runs in
`espressif/idf-rust:all_latest`. Both rows name an image with a shell,
for poking at the toolchain by hand:

    ./compat/nostd_check.sh --shell esp32
    ./compat/nostd_check.sh --shell esp32c3
    ./compat/nostd_check.sh --pull        fetch it first (a few GB)


6. Adding a machine
-------------------

Add a line to `compat/matrix.tsv`. Nine tab-separated fields:

    id        pi5
    target    aarch64-unknown-linux-musl
    class     linux            has an OS and a shell; roog runs here
    platform  linux/arm64      docker --platform for a native row; for a
                               qemu row, only picks the interpreter --
                               see stress_test_matrix.sh
    image     alpine:3.22      what the binary is executed in -- checked
                               for a shell before it is trusted, see
                               "the machines" above
    exec      qemu             native | qemu | none
    cpus      2                docker --cpus
    memory    2g               docker --memory
    note      Raspberry Pi 5, 64-bit OS

Then, if it is a new triple, add its cross image to `compat/Cross.toml`:

    [target.aarch64-unknown-linux-musl]
    image = "ghcr.io/cross-rs/aarch64-unknown-linux-musl:0.2.5"

That is all. `cross_build.sh`, `stress_test_matrix.sh`, the dashboard
and the report all read the table.

Two rules the tests enforce, so you will hear about it:

  - Every `linux` row must be a `-musl` triple. A `-gnu` binary is
    bound to the glibc it was linked against, which is the one thing
    the matrix exists to rule out.
  - Every `bare` row must have `exec` of `none`. Those rows are
    compiled, never run.

    cargo test -p roog-compat


Troubleshooting
---------------

A qemu row skipped saying it could not fetch its interpreter

    `stress_test_matrix.sh` pulls the static qemu-user interpreter it
    needs from `tonistiigi/binfmt` the first time a qemu row runs, and
    caches it in `target/compat/qemu/`. If that pull fails -- no
    network, registry unreachable -- the row is skipped rather than
    run against the wrong thing. Check docker can reach the registry:

        docker pull tonistiigi/binfmt

    No `--privileged`, no binfmt_misc, and nothing written outside
    this pipeline's own output directory -- see
    `../explanation/cross-platform-testing.md`.

`docker is not answering`

        sudo systemctl start docker
        sudo usermod -aG docker $USER      # then log out and back in

`cross not found`

        cargo install cross

    The host's own musl triple will still build with plain cargo:

        cargo build --release --target x86_64-unknown-linux-musl -p engine

`no binary at target/cross/<triple>/<triple>/release/roog-perf`

    Phase 2 was run before phase 1, or for a row phase 1 skipped:

        ./compat/cross_build.sh --targets <machine>

    The triple appears twice because each target gets its own cargo
    target directory. That is deliberate and it fixes a real failure --
    see "Why each target gets its own target directory" in the
    explanation page.

`<machine> is dynamically linked`

    The triple is not a `-musl` one. Fix the row; a dynamically linked
    binary will not start on a machine with a different libc.

A build dies on `weak version GLIBC_2.xx not found`

    A host artifact leaked into a cross container, or one cross image's
    artifact into another's. Each target has its own target directory
    precisely to stop that, so this means something was built with
    `CARGO_TARGET_DIR` unset or pointing elsewhere:

        rm -rf target/cross && ./compat/cross_build.sh

A row says `does not run` and the log ends abruptly

    On a memory-capped row that is usually the OOM killer, and it is a
    result rather than a bug: roog does not fit in that much RAM. The
    report points at `target/compat/run-<machine>-game.log`.

The report contradicts what the pipeline just printed live

    The matrix loop and `roog-compat report` disagree -- a row the loop
    just logged `ok  builds: ...` for shows up as `does not run`, or is
    missing from the table entirely. `matrix.tsv` is compiled into
    `roog-compat` with `include_str!`, so a `target/release/roog-compat`
    built before the last change to it, or to `compat/src/*.rs`, is
    reading fresh results with stale code -- an unrecognised status
    string falls back to `Failed`, and a row added to the table since
    the binary was built never appears at all. Both scripts now build
    `roog-compat` unconditionally rather than only when the binary is
    missing, specifically because "it already exists" and "it matches
    the current source" are different questions and only `cargo build`
    can answer the second one. If you still see this on an older
    checkout:

        cargo build --release -p roog-compat

`llvm-nm cannot read a <triple> binary`

    GNU `nm` often refuses a foreign ELF; llvm-nm reads all of them.

        rustup component add llvm-tools

The blame table says "no sized symbols"

    It read the stripped binary. The `profiling` build is what carries
    the symbols; check the build log for why it did not produce one.


See also
--------

    ../explanation/cross-platform-testing.md   why it is built this way
    run-the-perf-pipeline.md                   the host-side rig
    ../../compat/matrix.tsv                    the machine list itself

Every script takes `--help`, and each one's help is the authority on its
own flags:

    ./compat_test.sh --help
    ./compat/cross_build.sh --help
    ./compat/stress_test_matrix.sh --help
    ./compat/nostd_check.sh --help
    target/release/roog-compat --help
