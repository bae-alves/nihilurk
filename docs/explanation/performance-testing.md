Why the performance rig is built this way
=========================================

    Audience       Anyone changing `perf/`, or wondering why the
                   particle stress test replays a music video.
    Prerequisites  You have run `./perf_test.sh` once.
    This is        Understanding. The recipe is
                   `../how-to/run-the-perf-pipeline.md`.

roog's particle layer is decoration: sparks, bolts, blast rings. It never
touches the turn schedule and is never saved. It is also the only part of
the game that runs at video rate, which makes it the only part where a
frame budget exists to be blown.

The rig measures it under a load the game will never produce, and there
are a handful of decisions behind that which are worth writing down.


Three workloads, nested
-----------------------

The particle layer is not the only thing that happens in an animation
frame, and for a while it was the only thing measured. `play_particles`
in `engine/src/view.rs` calls the full `render` between every pair of
`advance` calls: the whole 80x25 grid repainted, diffed against what is
already on screen, and the cells that changed turned into escape
sequences. Measuring the motes and not the repaint answered half of a
question.

So the rig runs three:

    particles   the layer alone, into an off-screen canvas
    screen      the reel painted into the grid, diffed, emitted
    both        the two composed as the game composes them

They are nested deliberately. `screen` is `both` without the particle
layer; `both` is `screen` with it. Neither number means much alone --
what means something is the gap between them, because that gap is what
the particle layer costs *on top of a repaint the game performs every
frame regardless*. A phase breakdown cannot show that. Two runs that
differ in exactly one thing can.

`--workload all` runs the three off a single parse of the reel. That
saves 59 MiB and several seconds three times over, but the reason is
comparability: same frames, same machine, same moment, differing in
nothing but the work under test.


A separate crate, outside the default members
---------------------------------------------

`perf/` is a workspace member, but the workspace also declares:

    default-members = ["engine", "models"]

So `cargo build` and `cargo test` at the root do not touch it. Reaching
it is always explicit -- `-p roog-perf`, or the script.

This is the portability rule doing its work. roog is meant to build and
run on anything with a terminal, and the game crates depend on almost
nothing. The rig depends on ratatui, sysinfo and criterion, and none of
those belong anywhere near the shipped binary. Keeping it a separate
crate means the dependency can only ever point one way: `roog-perf`
depends on `models`, never the reverse.


Bad Apple, and why a video
--------------------------

The reel is the whole video -- 6572 frames of 80x20 ASCII, three minutes
and thirty-nine seconds at 30 fps -- which is exactly roog's map width
and fits inside its 22-row map. Each lit cell becomes one call to the
real `Particles::blip`, so a frame queues up to 1600 motes, thirty times
a second, and averages a little under 800. It loops when it reaches the
end, so the dashboard runs until you stop it.

A wand bolt queues a few dozen. So the reel is two to three orders of
magnitude past the working range, which is the point: the interesting
question is not whether the layer survives a fireball, but where it stops
keeping up. You cannot find a cliff without driving off it.

It also has a property a synthetic benchmark would not: the load varies
with the picture. A frame that is mostly silhouette spawns half what a
filled frame does, so the memory graph breathes with the animation, and
a leak shows up as a line that stops coming back down.

And you can see it. A stress test you watch is a stress test you run.


The reel is parsed once, before the clock starts
------------------------------------------------

`Reel::load` resolves every frame into pre-computed `(x, y, glyph,
colour)` cells at startup. Nothing in the frame loop splits a string or
matches a character.

Without that, the flamegraph would be a picture of `str::split` and the
memory graph would be tracking the parser's garbage. The measurement
would still be *a* measurement; it just would not be a measurement of the
particle layer. The same reasoning is why the headless report subtracts a
baseline snapshot taken after the reel is loaded: several megabytes of
parsed frames are not particle churn, and counting them made every mote
look three times fatter than it is.


Counting allocations instead of trusting RSS
--------------------------------------------

The obvious way to watch memory is RSS, and RSS is nearly useless here.

Every mote heap-allocates a `Vec<(char, Color)>` for its keyframes and
drops it a few frames later. The allocator caches those freed blocks and
does not return the pages, so RSS goes up once and then sits flat while
the program allocates a hundred thousand times a second underneath it.
Watching RSS, that workload looks idle.

So the rig installs a counting wrapper around the system allocator and
counts the calls itself: live bytes, live blocks, total allocations,
total bytes. Three relaxed atomics on the allocation path, a few
nanoseconds each, and unlike RSS they move the instant a `blip` happens.
That is what stands in for Heaptrack and Valgrind during the live run --
not a call-graph attribution, but exact totals, readable at 30 Hz without
perturbing what is being measured.

Relaxed ordering is correct: the counters guard no other memory, nothing
is published through them, and a reader only ever wants a recent value.

The counters are cheap, not free, and at reel volumes that shows. In an
unpaced profile the atomic increments alone are around 28% of the
samples, and the wrapped `dealloc` sitting above them carries more,
because the workload is dominated by allocator traffic and the wrapper
is on every call of it.
That cost is the rig's, not the game's -- the shipped binary has no
`Tracking` in it. Read the flamegraph accordingly: `atomic_add`,
`atomic_sub` and `dealloc` at the top of the self-time table are the
instrument, and what matters is the ordering of everything else.


What is measured on which thread
--------------------------------

    heap counters    in the frame loop -- three atomic loads
    RSS and CPU%     on a sampler thread, 10 Hz and 2.5 Hz

`sysinfo`'s process refresh is far too expensive to sit inside a 33 ms
budget; doing it inline would put the observer inside the observation.
It runs on its own thread and posts down a channel the frame loop drains
without blocking. A stalled sampler shows a stale number and does not
drop a frame -- the right trade for a metric that moves on the order of
seconds.

The heap counters go the other way for the same reason: they are cheap
enough to read in the loop, and reading them there means they line up
exactly with the frame whose particle count is displayed beside them.

Frame time is the *work*, never the wait. The loop sleeps out the rest of
its budget; counting that sleep would report a perfectly idle rig as
fully busy.


Two profiles, one machine code
------------------------------

`release` is what ships: `lto = "fat"`, `codegen-units = 1`, `panic =
"abort"`, symbols stripped. `profiling` inherits from it and puts the
symbols back.

Inheriting rather than redeclaring is the whole trick. A flamegraph of a
differently-optimised binary is a flamegraph of a program you do not
ship, and the two profiles cannot drift because only one of them sets the
optimisation knobs.

`panic = "abort"` is safe here because there is not a single
`#[should_panic]` in the suite, and cargo ignores the setting for the
`test` and `bench` profiles anyway.


The grid is a copy, and it can drift
------------------------------------

`Screen` -- the double-buffered grid, the diff, the escape-sequence
emitter -- lives in `engine/src/view.rs`. `engine` is a binary crate with
no library target, and `Screen::put`, `clear` and `flush` are private to
that module, so the rig cannot reach it. Three ways out were weighed:

  1. give `engine` a library target and depend on it
  2. move `Screen` down into `models`, which both crates already share
  3. copy it into `perf/src/screen.rs`

The third was chosen, and the game crates stay untouched. It is worth
being plain about what that costs, because it runs against the rule the
rest of this rig is built on -- `scene.rs` says *nothing is
reimplemented here*, and this is a reimplementation.

Nothing checks the two stay in step. There is no compile error, no
failing test, no warning. If `engine/src/view.rs` changes and
`perf/src/screen.rs` does not, the rig goes on cheerfully reporting
numbers for a renderer the game no longer has, and the report will look
exactly as trustworthy as it did the day before. The mitigation is a
header comment in `perf/src/screen.rs` listing what has to match for the
measurement to mean anything -- the cell shape, the double buffer, the
skip-unchanged test, the colour-change test, `dirty_all` -- and the
discipline to read it when touching either file.

One difference is intentional: the rig's `flush` returns how many cells
it drew. The original returns `()`. Counting them in the loop that
already visits them is free, where a second diff pass over all 2000 cells
per frame would have added several percent to the thing being measured.


Bytes, not syscalls
-------------------

The redraw workloads flush into a `Vec<u8>` whose capacity is reused
between frames, and report how many bytes that was. The game flushes into
a `BufWriter<Stdout>`: the same formatting into the same kind of buffer,
followed by one write syscall handing it to the terminal.

The syscall is left out on purpose. What it costs is the terminal
emulator's business, it varies by an order of magnitude between them, and
including it would quietly turn a measurement of roog into a benchmark of
whatever happened to be attached to stdout. What is measured is
everything roog controls: the diff, the escape-sequence generation, and
the byte count handed over at the end of it.

That byte count turns out to be the most transferable number the rig
produces. Milliseconds are the machine's; bytes per frame are the
program's, and they are the same on every machine that runs it.


A flamegraph you can read over SSH
----------------------------------

`cargo flamegraph` writes an SVG, which assumes a machine with a browser
on it. That contradicts the same portability rule as everything else
here, so `roog-perf flame` renders the graph as text instead.

It reads folded stacks or raw `perf script` output, doing the collapsing
itself so the pipeline needs `perf` and not also inferno or a Perl
script. The layout is an icicle -- root at the top -- because a terminal
scrolls downward and a graph you have to scroll backwards to find the
root of is a graph nobody reads.

Sibling frames come out of a `BTreeMap`, so ordering is alphabetical and
stable between runs. A graph that reshuffles itself cannot be compared
against yesterday's.


The profiled run is unpaced; the measured run is not
----------------------------------------------------

The stress run sleeps out the rest of every 33 ms budget, and at density
1 the work is around 0.2 ms of it. So the process is idle for 99% of its
wall clock, and a sampling profiler counts cycles, not seconds: it sees
only the 1%.

Recorded that way, a 15-second profile lands a few dozen samples in the
particle layer and several hundred in `Reel::load`, which runs flat out
at startup and is the one part of the rig that deliberately does not
matter. The flamegraph comes out a picture of the parser -- the exact
failure `frames.rs` was written to avoid, arriving through the other
door.

So stage 8 records `--flat-out`, which drops the pacing and runs frames
back to back. The per-frame work is identical: `dt_ms` is a constant the
loop passes to `advance`, not a measured wall-clock delta, so the
simulation is the same one either way. All that changes is that the idle
between frames is gone, and every sample lands in the frame loop.

The two runs answer different questions and the pipeline keeps both.
Stage 7, paced, answers "does it keep up" -- frame time against a budget,
dropped frames, achieved fps. Stage 8, unpaced, answers "where did the
cycles go". An unpaced run's `fps achieved` is a throughput figure with
no budget behind it, and the report labels it `unpaced` so it never gets
quoted as the other thing.


Every stage degrades
--------------------

`perf` is Linux-only and needs permission to open a performance counter
-- which, on a stock kernel, it has: `perf_event_paranoid` defaults to 2,
and 2 forbids sampling kernel symbols, not sampling a user-space process
you own. Only the hardened 3 blocks the stage outright. cargo-bloat is an
extra install. Neither is present on a fresh machine,
and a pipeline that only ran on a fully-equipped workstation would be a
pipeline nobody ran.

So every optional stage checks for its tool and, missing it, says what to
install and carries on. The core -- fmt, clippy, tests, benchmarks, the
stress run -- needs nothing but cargo, and it includes a phase-level
profile of its own: three timers around `advance`, `spawn` and `raster`.

That breakdown is coarser than a sampled call graph. It also answers the
first question you would ask a flamegraph, on any platform, with no
tooling at all, which makes it the more valuable of the two more often
than is comfortable to admit.


What it found
-------------

`spawn` leads the phase breakdown, and the reason is one line in
`particles.rs`: every `Particle` owns a `Vec` of keyframes, and almost
every mote in existence has exactly one keyframe in it. At in-game
volumes -- a few dozen motes per bolt -- that is free and the ownership
is worth having. At reel volumes it is tens of thousands of allocations a
second for eight bytes apiece.

That is not a bug, and nothing in the game is slow because of it. It is
written down here because it is the first thing any future profile will
point at, and because if the particle layer is ever asked to do
substantially more, a `SmallVec`-shaped inline array is where the win is.
