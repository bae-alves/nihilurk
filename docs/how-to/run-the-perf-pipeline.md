How to run the performance pipeline
===================================

    Audience       Anyone who changed `models/src/particles.rs`, the
                   render loop, or anything they suspect is now slower.
    Prerequisites  A Rust toolchain. Nothing else. Optional tools are
                   named where they help, and skipped where they are not
                   installed.
    Result         Numbers for the particle layer, the terminal redraw,
                   and the two together, under a load two orders of
                   magnitude past anything the game asks of them -- plus
                   a flamegraph you can read in the terminal.

Everything below is one script and one binary. Start here and read no
further if it comes back green:

    ./perf_test.sh


Quick commands
--------------

    ./perf_test.sh                  everything available (~6 min)
    ./perf_test.sh --quick          skip the benchmarks (~2 min)
    ./perf_test.sh --gui            finish in the live dashboard
    ./perf_test.sh --frames 900     longer stress run   [default 450]
    ./perf_test.sh --duration 60    seconds to profile  [default 15]
    ./perf_test.sh --density 8      motes per lit cell  [default 1]
    ./perf_test.sh --fps 60         drive the reel at 60 fps  [default 30]
    ./perf_test.sh --baseline main  pin a criterion baseline by that name
    ./perf_test.sh --help

The rig on its own -- `P="cargo run -p roog-perf --profile profiling --"`:

    $P --headless --workload all      all three, measured and compared
    $P --headless --workload screen   the redraw alone
    $P                                watch: particle dashboard
    $P --workload both                watch: the reel repainted for real
    $P --headless --workload both --full        the whole video, measured

    cargo bench -p roog-perf                    every benchmark
    cargo bench -p roog-perf --bench redraw     just the redraw ones

Artifacts always land in `target/perf/`.


The three workloads
-------------------

    particles   the particle layer alone, painted into an off-screen
                canvas. No terminal, no diff.
    screen      the reel painted into the game's 80x25 grid, diffed
                against the last frame, and turned into escape
                sequences. The redraw on its own.
    both        the two composed the way `engine/src/view.rs` composes
                them: base layer painted, live motes over the top, one
                diff and one flush over the lot.

They are nested, and that is the point: `screen` is `both` without the
particle layer. The gap between those two reports is what the particles
cost *on top of a repaint the game does every frame anyway*, which is the
only form of the question the game actually asks.

`--workload all` runs the three off one parse of the reel and prints a
comparison. Stage 7 does exactly that.


1. Did I make it slower?
------------------------

1. Run the pipeline:

       ./perf_test.sh

2. Read the last block it prints. That is the summary, and these are the
   lines worth looking at:

       mean / p99      frame time against a 33.33 ms budget
       dropped         frames that overran it; should be 0
       advance / spawn / raster    where the frame went
       per mote        bytes allocated per particle
       peak live       simultaneous motes at the worst moment

3. Compare against the numbers in the table further down. If nothing
   moved, you are done.

The script exits non-zero if formatting, clippy, the test suite or the
stress run failed, so it drops straight into CI with no wrapper.


2. Watch it happen
------------------

1. Start the dashboard:

       cargo run -p roog-perf --profile profiling

   It wants a terminal at least 116x35. Smaller and it says so.

2. Three panes: the reel playing through the real particle layer in the
   middle, memory down the right, frame timing along the bottom.

       q / Esc   quit
       space     pause
       + / -     density, live
       b         fire a full-map blast through `Particles::explosion`

3. Hold `+` until the frame time crosses the budget line. The density at
   which a 30 fps frame stops fitting in 33 ms is the one number worth
   writing down when you change the particle layer.

`./perf_test.sh --gui` runs the whole pipeline and then drops you here.


3. Get the same numbers without the UI
--------------------------------------

    cargo run -p roog-perf --profile profiling -- --headless --workload all

450 frames of reel per workload -- fifteen seconds each at 30 fps, and
enough: past that the phase shares and the per-frame costs stop moving,
and a full pass reports the same numbers in 3m39s instead. This is
exactly what stage 7 runs, and what CI should diff run to run.

Bounding by frames rather than by seconds is deliberate. The same 450
frames render on any machine, so the totals underneath them -- cells,
bytes, allocations -- are comparable between runs in a way a wall-clock
window is not.

    --frames 900    a longer run
    --full          the whole reel, 6572 frames
    --duration 30   bound by wall clock instead (what stage 8 uses)


3b. Watch the redraw itself
---------------------------

    cargo run -p roog-perf --profile profiling -- --workload screen
    cargo run -p roog-perf --profile profiling -- --workload both

These do not open the dashboard. They take the terminal and repaint it
for real, through the same grid the headless run counts bytes of -- so
what you are watching is the workload, not a picture of it. The reel
loops; `q`, `Esc` or `Ctrl-C` stops it. Needs an 80x25 terminal.

A status line under the frame carries the live figures: work per frame
against the budget, the flush time, and the cells and bytes that frame
cost. Watch the byte counter while the video goes from silhouette to
full frame -- that is the diff earning its keep.


4. Compare before and after a change
------------------------------------

1. On the unchanged tree, pin a baseline:

       cargo bench -p roog-perf -- --save-baseline before

2. Make the change.

3. Measure against it:

       cargo bench -p roog-perf -- --baseline before

Criterion prints the delta per benchmark and says whether it considers
the difference significant. Five groups -- `spawn`, `advance`, `cull`,
`current`, `constructors` -- each at four population sizes, so the curve
across them also says whether the layer is still linear in mote count.

With no `--baseline`, criterion compares against the previous run
automatically. That is enough for a quick check; pin a named baseline
when you are going to iterate.


5. Chase it down to a function
------------------------------

Stage 8 of the pipeline does this for you and prints the flamegraph as
text. To do it by hand:

1. Record. `--flat-out` is what makes this worth doing -- see the note
   under *Why unpaced* below:

       perf record -F 999 -g --call-graph fp -o target/perf/perf.data -- \
           target/profiling/roog-perf --headless --flat-out --duration 15

2. Draw it:

       perf script -i target/perf/perf.data | \
           target/profiling/roog-perf flame -

The graph is an icicle -- root at the top, children below, width
proportional to samples -- followed by a self-time table, which is
usually the part you want.

       --width N    columns to draw in   [default: terminal width]
       --depth N    deepest stack row    [default: 24]
       --no-color   plain text, for piping to a file

`flame` reads folded stacks (`main;turn;blip 1234`) or raw `perf script`
output and works out which it was given. `-` or no argument reads stdin.

**Why unpaced.** A paced run sleeps out the rest of every 33 ms budget,
which at in-range densities is about 99% of its wall clock, and a
sampling profiler only sees a running process. Recorded paced, the
flamegraph is a picture of `Reel::load` with a few dozen samples of the
particle layer under it. `--flat-out` runs the same frames back to back
-- identical per-frame work, `dt_ms` is a constant -- so every sample
lands in the frame loop. Use the paced run for "does it keep up" and the
unpaced one for "where did the cycles go"; never quote the unpaced run's
`fps achieved` as a result.


The stages
----------

| # | Stage       | Needs             | If the tool is missing       |
|---|-------------|-------------------|------------------------------|
| 1 | Formatting  | rustfmt           | skipped                      |
| 2 | Lints       | clippy            | skipped                      |
| 3 | Correctness | --                | --                           |
| 4 | Build       | --                | --                           |
| 5 | Footprint   | cargo-bloat       | falls back to `size`         |
| 6 | Benchmarks  | --                | --                           |
| 7 | Stress run  | --                | -- (all three workloads)     |
| 8 | CPU profile | perf              | skipped; stage 7 covers it   |
| 9 | Summary     | --                | --                           |

Stages 1-4, 6, 7 and 9 need only cargo. That is deliberate: see
`../explanation/performance-testing.md`.

Everything each stage writes:

    target/perf/clippy.txt        full lint output
    target/perf/test.txt          full test output
    target/perf/build.txt         build log, printed on failure
    target/perf/bloat-*.txt       per-crate and per-function size
    target/perf/bench.txt         criterion, in full
    target/perf/stress.txt        the three reports and the comparison
    target/perf/perf.data         the recording
    target/perf/perf.script       decoded, for any other flamegraph tool
    target/perf/flame.txt         the graph, plain text


Reading the output
------------------

The pipeline prints four different kinds of thing, and they are read four
different ways.


### The stage lines

    ok   the stage passed
    ..   the stage was skipped, and why
    !!   the stage failed

`!!` on formatting, lints, tests or the stress run fails the whole run
and sets a non-zero exit code. `..` never does: a skipped stage means a
tool is not installed, and the note beside it says which. Nothing in the
pipeline needs anything but cargo, so a run full of `..` is still a
valid run -- it just has less detail in it.


### The stress report (stage 7)

`FRAME TIME` is the per-frame *work*, never the wait -- the rig sleeps
out the rest of the budget, and counting that sleep would report an idle
rig as fully busy.

    mean / p50      the typical frame
    p95 / p99       the jitter; this is where a stall shows up
    max             the single worst frame, usually the first
    dropped         frames that overran the budget -- must be 0

`p50` above `mean` is normal and not a bug: Bad Apple opens on a nearly
blank screen, so the first seconds spawn almost nothing and drag the
mean below the median.

`PHASES` splits that frame time up to four ways. A workload that never
runs a phase does not list it:

    advance     ageing and culling every live mote
    spawn       one `Particles::blip` per lit cell
    raster      painting cells into the target -- the canvas, or the grid
    flush       diffing the grid and emitting the escape sequences

In the particle workloads `spawn` leads, because every mote
heap-allocates a `Vec` for its keyframes. In `screen` it is `flush`, by
about four to one over `raster` -- painting the grid is cheap and
diffing-and-emitting is where a redraw's time goes. The *shares* are the
number to compare between runs; the absolute milliseconds move with
whatever else the machine is doing.

`REDRAW`, on the two workloads that have one, is per frame:

    cells       how many of the 2,000 changed and had to be written
    bytes       the escape sequences those cells turned into
    per cell    the second divided by the first, ~13 bytes

`cells` is the number that explains everything else, and it is usually
far smaller than you expect -- around 77 of 2,000 on Bad Apple, because
consecutive frames of a 30 fps video mostly agree with each other and the
diff writes only the difference. If `cells` ever approaches 2,000, the
diff has stopped working and the grid is being repainted whole.

`bytes` is the most transferable number the rig produces. Milliseconds
belong to the machine; bytes per frame belong to the program, and they
are identical on every machine that runs it. Treat a jump there as real
without needing a second run.

`MEMORY` subtracts the reel, which is roughly 59 MiB of parsed frames
and is not what you are measuring:

    reel at rest    what `Reel::load` cost; the subtracted baseline
    live heap       net of that -- tracks the live mote population
    allocations     the churn figure; millions per second when unpaced
    per mote        bytes of churn per particle
    peak RSS        whole process, reel included, and see below

`live heap` should land near `peak live` in block count -- one keyframe
`Vec` per live mote. If blocks climb run over run while the population
does not, that is a leak, and it is the one thing in this report worth
treating as a bug rather than a number.

`per mote` converges on 8 bytes -- one keyframe `Vec` of one
`(char, Color)` -- but only once the run is long enough to amortise the
one-off growth of `Particles::live` over it. A short run reads high for
that reason and not because a mote got fatter; compare like with like.

RSS is the *least* useful number on the page: the allocator caches freed
blocks, so a workload that churns hard and frees everything leaves RSS
flat. The tracking allocator's counters are what actually move.

Rough shape of a default 15s run on a current desktop, at 30 fps and
density 1:

    mean frame       0.2-0.3 ms of a 33.3 ms budget
    dropped          0
    phases           spawn ~41%, advance ~39%, raster ~20%
    peak live        5,000-6,400 motes
    per mote         ~20 bytes at 15s, ~8 at length


### The benchmark deltas (stage 6)

There are seven groups. `spawn`, `advance`, `cull`, `current` and
`constructors` measure `models::Particles`; `redraw` and `paint` measure
the grid, and live in `--bench redraw`. `redraw/N` is a flush with N of
the 2,000 cells changed, so `redraw/0` is the floor -- the scan that
proves nothing moved -- and `redraw/2000` is the full repaint after a
resize.

Criterion prints three numbers per benchmark and then an opinion:

    spawn/1600   time:   [60.060 µs 60.755 µs 61.544 µs]
                 change: [+2.9256% +4.0411% +5.1342%] (p = 0.00 < 0.05)
                 Performance has regressed.

The bracket is a confidence interval -- lower, estimate, upper. `change`
compares against the last stored run, and `p` is the chance the two
samples came from the same distribution.

**"Performance has regressed" is not a verdict on your change.** It is a
verdict on two samples, and criterion has no idea the second one was
taken while a compile was running. A busy machine routinely produces
`+3%` to `+14%` with `p = 0.00` across every benchmark at once, which is
the tell: a real regression lands on the benchmarks that touch the code
you changed, not uniformly on all eighteen.

So read it in this order:

1. **Which** benchmarks moved. All of them, by a similar amount, on a
   machine you were using = noise. `spawn/*` alone after you edited
   `blip` = real.
2. **How much.** Under 5% on a desktop is inside the noise floor unless
   it reproduces. Treat 10%+ as worth a second run.
3. **Does it reproduce.** Run it again on an idle machine before you
   believe it. To make the comparison fair, pin a baseline rather than
   drifting against whatever ran last:

       cargo bench -p roog-perf -- --save-baseline before
       ...make the change...
       cargo bench -p roog-perf -- --baseline before

The curve *across* the four population sizes is the other thing to read,
and it is noise-proof in a way the deltas are not. `thrpt` is per-element
throughput, so a layer that is linear in mote count holds it roughly flat
as the population grows:

    spawn      31 -> 26 -> 27 -> 26 Melem/s     flat
    cull       44 -> 45 -> 46 -> 42 Melem/s     flat
    current   196 -> 202 -> 194 -> 189 Melem/s  flat
    advance   953 -> 951 -> 897 -> 639 Melem/s  falls off at 25,600

`redraw` is read differently, because its input is cells changed rather
than a population, and it should be a straight line through a fixed
offset -- the scan -- plus a constant per changed cell:

    redraw/0       3.4 us    the scan alone, nothing changed
    redraw/76      7.8 us    what Bad Apple actually averages
    redraw/400    26.6 us
    redraw/1000   61.4 us
    redraw/2000  120.4 us    every cell, i.e. a full repaint

That is about 58 ns per changed cell on top of a 3.4 us floor. A full
repaint of the entire grid costs 0.12 ms, which is 0.4% of a 33 ms
frame -- worth knowing before optimising anything here.

`advance`'s falloff at the top of the range is cache, not algorithm: a
`Particle` is 40 bytes, so 25,600 of them is a megabyte of `live` to walk
plus a pointer chase per mote into its keyframe `Vec`, and that stops
fitting. It is expected, it is four times past anything the reel
produces, and the game will never come near it.

What would be a real finding is a falloff appearing where the table above
is flat -- `spawn` sagging at 6,400, say. That says something went
super-linear, and it matters far more than any single percentage.


### The flamegraph (stage 8)

Two parts. The icicle on top is the call tree -- root at the top,
children below, width proportional to samples. The table under it is
self time, and that is the half to read first: it is where cycles were
actually spent, rather than where they were accounted.

    self       %      total  frame
    1,825   22.4%      1,825  dealloc
    1,464   18.0%      1,464  atomic_add
      904   11.1%      1,211  current
      365    4.5%      1,690  rasterize

`self` is time in that frame alone; `total` includes everything it
called. A frame with high `total` and low `self` is a router, not a
cost -- `advance` is usually the clearest example.

**Discount the instrument.** `atomic_add`, `atomic_sub` and `dealloc` at
the top of that table are the rig's own tracking allocator, which wraps
every allocation to count it. The three atomic frames alone are ~28% of
a default profile, and `dealloc` above them is the wrapper as much as the
free. The shipped game has none of it -- there is no `Tracking` in the
release binary. What matters is the ordering of everything else.

A healthy graph has `tick` at ~95% with `step` and `rasterize` beneath
it. If `Reel::load`, `shade` or `run_utf8_validation` appear at all, the
recording was taken paced instead of `--flat-out` and is measuring the
parser -- see *Why unpaced* above.


### The comparison table

`--workload all` ends with one row per workload, and they are directly
comparable because the three ran off one parse of the reel:

    workload              mean     p99   flush  drop   cells/f   bytes/f  allocs/f
    particles            0.299   0.602      --     0        --        --     1,082
    screen               0.037   0.137   0.029     0        77       989         6
    screen+particles     0.328   0.559   0.031     0        71       925     1,082

Read it across, then down. `screen` against `screen+particles` is the
only subtraction that means anything: it is what the particle layer adds
to a repaint the game performs every frame regardless. The line printed
under the table does that subtraction for you.

`allocs/f` at 6 for `screen` is the honest zero -- a redraw allocates
nothing per frame, and those six are the rig's own bookkeeping. The
`particles` row having no `cells/f` or `bytes/f` is not a gap; that
workload never touches a terminal.


### What a real regression looks like

    dropped          stops being 0
    phases           one phase's *share* grows; the others shrink
    cells/f          climbs toward 2,000 -- the diff has stopped working
    bytes/f          moves at all; this one is machine-independent
    benchmarks       the group touching your change moves, alone,
                     reproducibly, on an idle machine
    throughput       falls as population grows -- something went
                     super-linear
    live heap blocks climb run over run while peak live does not

Everything else -- absolute milliseconds, RSS, a uniform few percent
across all eighteen benchmarks -- is the machine, not the code.


If `perf` will not record
-------------------------

    sudo pacman -S perf                          # Arch
    cat /proc/sys/kernel/perf_event_paranoid

`2` is the kernel default and is fine -- it forbids sampling kernel
symbols, not your own process. `3` and above block the stage outright:

    sudo sysctl kernel.perf_event_paranoid=2     # for this boot

Missing it entirely costs you stage 8 and nothing else. Stage 7's
`PHASES` block answers the same first question -- which phase the budget
went to -- with three timers instead of a sampled call graph, on any
platform, with no tooling at all.


If you would rather have the SVG
--------------------------------

    cargo install flamegraph
    cargo flamegraph --profile profiling -p roog-perf -- --headless --flat-out

Note `--profile profiling`, not `--release`: the release profile strips
its symbols, and a flamegraph of a stripped binary is a wall of
addresses. Both profiles are defined in the workspace `Cargo.toml`, and
`profiling` inherits from `release` so the two cannot drift apart.

This writes `flamegraph.svg` and `perf.data` into the working directory.
Both are gitignored.
