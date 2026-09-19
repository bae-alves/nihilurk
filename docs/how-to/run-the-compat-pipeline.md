How to run the compatibility pipeline
=====================================

    Audience       Anyone who wants to know whether nihilurk still runs on
                   a Pi, a 32-bit netbook, or a Graviton instance --
                   and anyone who just made the binary bigger.
    Prerequisites  Docker, and `cargo install cross`. QEMU hooks for
                   the ARM rows; the script installs them for you.
                   Everything else is optional and is skipped with a
                   note where it is missing.
    Result         One line per machine saying whether nihilurk runs
                   there, the size of the binary for each, and which
                   crate is responsible for those bytes.

One script:

    ./compat_test.sh

Read no further if it comes back green.


Quick commands
--------------

    ./compat_test.sh                  everything (~10 min cold)
    ./compat_test.sh --targets cloud  one machine
    ./compat_test.sh --no-build       reuse what is already built
    ./compat_test.sh --targets pi-zero --no-bare     one row, no ESP32
    ./compat_test.sh --no-bare        skip the microcontroller check
    ./compat_test.sh --timeout 60     per-row seconds for the run check
    ./compat_test.sh --help

The three phases on their own:

    ./compat/cross_build.sh           build for every machine + blame
    ./compat/nostd_check.sh           the microcontroller rows
    ./compat/run_check.sh             check each machine actually runs it

The report:

    target/release/nihilurk-compat            the matrix, as text
    target/release/nihilurk-compat report     same thing, explicit
    target/release/nihilurk-compat gate       one line, and an exit code

Reports and logs land in `target/compat/`. The binaries themselves land in `target/cross/<triple>/<triple>/release/`.

Each run replaces `target/compat/results.tsv` rather than adding to it, `--targets` included. So a one-row run reports one row, and the machines it skipped show `-`. That is deliberate: a report mixing rows checked against two different builds of the game, with nothing on the page saying which was which, is worse than a report with gaps in it.


The machines
------------

`compat/matrix.tsv` is the list, and it is the only list. One row per machine, tab-separated; adding a machine is adding a line.

    cloud      x86_64 musl     native   t3.micro, and the control
    graviton   aarch64 musl    qemu     AWS Graviton, an M-series VM
    pi-zero    armv7 musl      qemu     Raspberry Pi Zero 2 W
    pi2        armv7 musl      qemu     Raspberry Pi 2 B
    pi3        aarch64 musl    qemu     Raspberry Pi 3 B
    pi4        aarch64 musl    qemu     Raspberry Pi 4 B
    cm         aarch64 musl    qemu     Compute Module 4 (1GB Lite)
    potato     i686 musl       native   32-bit netbook, ca. 2008
    esp32c3    riscv32imc      none     ESP32-C3, compiled only
    esp32      xtensa-esp32    none     ESP32, compiled only

There is no CPU or memory cap on any row, and there used to be one. nihilurk has no workload heavy enough to need a stress test -- every feel-layer effect is bounded -- so the matrix checks a narrower, cheaper claim instead: if it builds, the image has a shell, and the target is std (every `linux` row here is), nihilurk can run there. `cloud` and `potato` run on the host CPU directly; every other `linux` row runs under a directly-invoked qemu-user-static interpreter, fetched by the pipeline on its own -- no `binfmt_misc`, no `--privileged`.

Every `linux` row's image is checked for a shell before the binary is run -- that is not optional, nihilurk cannot start without a real terminal under it -- and a row that fails the check fails the pipeline rather than being silently skipped. See "why every `linux` row must have a shell" in `../explanation/cross-platform-testing.md`.

The two `esp32*` rows are never executed, under any flag: they are `no_std`, and there is no "run it for real" for a target with no OS to run it under. See "The microcontroller rows" below.


1. Does nihilurk still run everywhere?
----------------------------------

1. Run the pipeline:

       ./compat_test.sh

2. Read the table. One line per machine:

       machine   exec       binary  verdict
       cloud     native    1.8 MiB  runs
       graviton  qemu      1.6 MiB  runs
       pi-zero   qemu      1.6 MiB  runs
       potato    native    1.8 MiB  runs

   The verdict column is the whole story:

       runs          the image has a shell and the binary started cleanly
       no shell      the image has no usable shell -- nihilurk cannot run
                     without one
       does not run  had a shell, but the binary did not start
       -             not attempted (excluded by --targets, or never built)

3. `runs` exits 0. `no shell` and `does not run` fail the pipeline, because they mean a machine that is supposed to run nihilurk cannot.


2. What is being checked
-------------------------

`nihilurk -content`: it prints the game's content index and exits, without ever touching the terminal's alternate screen. That is the whole check -- if it prints something and exits 0, the row can run nihilurk. There is nothing to grade beyond that: nihilurk asks nothing of a machine that cross-compiling a full Rust toolchain (which `cross_build.sh` already proved this target can do) does not ask for first.


3. How big is it, and whose fault is that?
------------------------------------------

Phase 1 prints it per target, for the binary that actually ships -- `--release`, fat LTO, one codegen unit, symbols stripped:

       target     triple                                 game      bytes
       cloud      x86_64-unknown-linux-musl           1.8 MiB    1934384
       potato     i686-unknown-linux-musl             1.8 MiB    1846016

The exact byte count is there because this is a table you diff against the last run, and the 32-bit rows come out about 4% smaller -- which rounds to the same "1.8 MiB" and would otherwise be invisible.

Do not read too much into the last few kilobytes. A fat-LTO build is not reproducible to the byte, and the same source rebuilt can move by a page either way. A crate's *share* moving, or a target gaining tens of kilobytes, is the signal.

Then it attributes those bytes per crate:

       crate                                   bytes    share
       models                                 477599    28.7%
       bevy_ecs                               343492    20.7%
       engine                                 210844    12.7%
       core                                   159366     9.6%
       std                                     98965     6.0%
       libc + compiler builtins                65823     4.0%
       total in symbols                      1662671

Those are *symbol* bytes, not file bytes -- the difference is section headers, padding, relocations and the constant pool -- so the total comes in under the size printed above it. Read the shares.

The full table per machine is in `target/compat/blame-<machine>.txt`. `--no-blame` skips the stage; `--top 20` lists more crates.


4. The microcontroller rows
---------------------------

    ./compat/nostd_check.sh

This does not build nihilurk for an ESP32. nihilurk draws with crossterm, crossterm needs a terminal, and a microcontroller has neither a terminal nor an OS to provide one.

What it checks is that `particle-core` -- the arithmetic of the particle layer, which the game itself calls -- still compiles with no operating system under it, for `riscv32imc-unknown-none-elf` and `xtensa-esp32-none-elf`. It is a standing structural check: the moment that arithmetic is given a `Vec`, a `String`, or a libm call, this goes red.

It runs the parity test first, which is what makes the rest mean anything -- see the explanation page.

RISC-V is a rustup target, so that row is a one-second `cargo check` on the host. Xtensa needs Espressif's rustc fork, so that row runs in `espressif/idf-rust:all_latest`. Both rows name an image with a shell, for poking at the toolchain by hand:

    ./compat/nostd_check.sh --shell esp32
    ./compat/nostd_check.sh --shell esp32c3
    ./compat/nostd_check.sh --pull        fetch it first (a few GB)


5. Adding a machine
-------------------

Add a line to `compat/matrix.tsv`. Seven tab-separated fields:

    id        pi5
    target    aarch64-unknown-linux-musl
    class     linux            has an OS and a shell; nihilurk runs here
    platform  linux/arm64      docker --platform for a native row; for a
                               qemu row, only picks the interpreter --
                               see run_check.sh
    image     alpine:3.22      what the binary is executed in -- checked
                               for a shell before it is trusted, see
                               "the machines" above
    exec      qemu             native | qemu | none
    note      Raspberry Pi 5, 64-bit OS

Then, if it is a new triple, add its cross image to `compat/Cross.toml`:

    [target.aarch64-unknown-linux-musl]
    image = "ghcr.io/cross-rs/aarch64-unknown-linux-musl:0.2.5"

That is all. `cross_build.sh`, `run_check.sh` and the report all read the table.

Two rules the tests enforce, so you will hear about it:

  - Every `linux` row must be a `-musl` triple. A `-gnu` binary is bound to the glibc it was linked against, which is the one thing the matrix exists to rule out.
  - Every `bare` row must have `exec` of `none`. Those rows are compiled, never run.

    cargo test -p nihilurk-compat


Troubleshooting
---------------

A qemu row skipped saying it could not fetch its interpreter

    `run_check.sh` pulls the static qemu-user interpreter it needs
    from `tonistiigi/binfmt` the first time a qemu row runs, and
    caches it in `target/compat/qemu/`. If that pull fails -- no
    network, registry unreachable -- the row is skipped rather than
    run against the wrong thing. Check docker can reach the registry:

        docker pull tonistiigi/binfmt

    No `--privileged`, no binfmt_misc, and nothing written outside
    this pipeline's own output directory.

`docker is not answering`

        sudo systemctl start docker
        sudo usermod -aG docker $USER      # then log out and back in

`cross not found`

        cargo install cross

    The host's own musl triple will still build with plain cargo:

        cargo build --release --target x86_64-unknown-linux-musl -p engine

`no binary at target/cross/<triple>/<triple>/release/nihilurk`

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

    The container exited non-zero or the timeout killed it. The report
    points at `target/compat/run-<machine>.log`.

The report contradicts what the pipeline just printed live

    The matrix loop and `nihilurk-compat report` disagree -- a row the loop
    just logged `ok  runs: ...` for shows up as `does not run`, or is
    missing from the table entirely. `matrix.tsv` is compiled into
    `nihilurk-compat` with `include_str!`, so a `target/release/nihilurk-compat`
    built before the last change to it, or to `compat/src/*.rs`, is
    reading fresh results with stale code. Both scripts build
    `nihilurk-compat` unconditionally rather than only when the binary is
    missing, specifically because "it already exists" and "it matches
    the current source" are different questions and only `cargo build`
    can answer the second one. If you still see this on an older
    checkout:

        cargo build --release -p nihilurk-compat

`llvm-nm cannot read a <triple> binary`

    GNU `nm` often refuses a foreign ELF; llvm-nm reads all of them.

        rustup component add llvm-tools

The blame table says "no sized symbols"

    It read the stripped binary. The `profiling` build is what carries
    the symbols; check the build log for why it did not produce one.


See also
--------

    ../explanation/cross-platform-testing.md   why it is built this way
    ../../compat/matrix.tsv                    the machine list itself

Every script takes `--help`, and each one's help is the authority on its own flags:

    ./compat_test.sh --help
    ./compat/cross_build.sh --help
    ./compat/run_check.sh --help
    ./compat/nostd_check.sh --help
    target/release/nihilurk-compat --help
