//! Regression benchmarks for the particle layer's hot math.
//!
//! These exist to catch a change that makes a mote more expensive, not to
//! produce an impressive number. So each one isolates a single operation the
//! frame loop performs thousands of times, and every input is fixed: no RNG, no
//! clock, no reel on disk. Criterion compares each run against the last stored
//! baseline and reports the delta, which is the whole point -- an absolute
//! nanosecond count means very little across machines, but "advance got 30%
//! slower since you touched it" means a great deal.
//!
//!     cargo bench -p roog-perf                 measure, compare to baseline
//!     cargo bench -p roog-perf -- --save-baseline main
//!     cargo bench -p roog-perf -- --baseline main
//!
//! The five cases map onto the phases `roog-perf --headless` reports, so a
//! regression seen in the dashboard can be chased straight down to the
//! operation responsible.

use std::hint::black_box;

use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};

use crossterm::style::Color;
use models::{BlastPalette, MAP_HEIGHT, MAP_WIDTH, Particles};

/// Population sizes worth measuring: a heavy in-game effect, a frame of the
/// reel, and two multiples past it. The curve across these is what says whether
/// the layer is linear in mote count -- and `Particles::advance` ends with a
/// `Vec::retain`, which is the operation most likely to stop being linear.
const POPULATIONS: [usize; 4] = [64, 1_600, 6_400, 25_600];

/// A deterministic spread of motes over the map, so every iteration sees the
/// same work.
fn populate(fx: &mut Particles, n: usize) {
    for i in 0..n {
        let x = (i % MAP_WIDTH as usize) as u16;
        let y = ((i / MAP_WIDTH as usize) % MAP_HEIGHT as usize) as u16;
        fx.blip(x, y, '@', Color::White);
    }
}

/// Spawning: one `blip` per lit cell, which is one heap allocation per mote for
/// the keyframe `Vec`. In the phase breakdown this is `spawn`.
fn bench_spawn(c: &mut Criterion) {
    let mut group = c.benchmark_group("spawn");
    for n in POPULATIONS {
        group.throughput(Throughput::Elements(n as u64));
        group.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, &n| {
            b.iter(|| {
                let mut fx = Particles::new();
                populate(&mut fx, n);
                black_box(fx.live.len())
            });
        });
    }
    group.finish();
}

/// Ageing and culling. Run at a lifetime the motes survive, so this measures
/// the steady-state cost rather than the cost of a mass extinction.
fn bench_advance(c: &mut Criterion) {
    let mut group = c.benchmark_group("advance");
    for n in POPULATIONS {
        group.throughput(Throughput::Elements(n as u64));
        group.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, &n| {
            b.iter_batched_ref(
                || {
                    let mut fx = Particles::new();
                    populate(&mut fx, n);
                    fx
                },
                |fx| {
                    fx.advance(black_box(33.0));
                    black_box(fx.live.len())
                },
                criterion::BatchSize::LargeInput,
            );
        });
    }
    group.finish();
}

/// The cull itself: every mote dies on this tick, so `Vec::retain` has to move
/// the whole buffer. The worst case, and the one a frame hits when an effect
/// ends all at once.
fn bench_cull(c: &mut Criterion) {
    let mut group = c.benchmark_group("cull");
    for n in POPULATIONS {
        group.throughput(Throughput::Elements(n as u64));
        group.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, &n| {
            b.iter_batched_ref(
                || {
                    let mut fx = Particles::new();
                    populate(&mut fx, n);
                    fx
                },
                |fx| {
                    // `blip` lives 130 ms; one step past that kills every mote.
                    fx.advance(black_box(999.0));
                    black_box(fx.live.len())
                },
                criterion::BatchSize::LargeInput,
            );
        });
    }
    group.finish();
}

/// Resolving a mote's current keyframe: the arithmetic in `Particle::current`,
/// called once per live mote per frame by the renderer. This is `raster` in the
/// phase breakdown, minus the terminal write.
fn bench_current(c: &mut Criterion) {
    let mut group = c.benchmark_group("current");
    for n in POPULATIONS {
        let mut fx = Particles::new();
        populate(&mut fx, n);
        // Mid-life, so the keyframe index is actually computed rather than
        // short-circuiting on the not-yet-visible branch.
        fx.advance(65.0);
        group.throughput(Throughput::Elements(n as u64));
        group.bench_with_input(BenchmarkId::from_parameter(n), &fx, |b, fx| {
            b.iter(|| {
                let mut lit = 0usize;
                for p in &fx.live {
                    if black_box(p.current()).is_some() {
                        lit += 1;
                    }
                }
                black_box(lit)
            });
        });
    }
    group.finish();
}

/// The fat constructors. A blast allocates a five-keyframe `Vec` per cell
/// rather than a one-element one, and a beam does per-cell direction work on
/// top; both are worth watching separately from `blip`.
fn bench_constructors(c: &mut Criterion) {
    let mut group = c.benchmark_group("constructors");

    let cells: Vec<(u16, u16, f32)> = (0..MAP_HEIGHT)
        .flat_map(|y| (0..MAP_WIDTH).map(move |x| (x, y, (x as f32 / 8.0))))
        .collect();
    group.throughput(Throughput::Elements(cells.len() as u64));
    group.bench_function("explosion_full_map", |b| {
        b.iter(|| {
            let mut fx = Particles::new();
            fx.explosion(black_box(&cells), BlastPalette::Fire);
            black_box(fx.live.len())
        });
    });

    let line: Vec<(u16, u16)> = (0..MAP_WIDTH).map(|x| (x, x % MAP_HEIGHT)).collect();
    group.throughput(Throughput::Elements(line.len() as u64));
    group.bench_function("beam_full_width", |b| {
        b.iter(|| {
            let mut fx = Particles::new();
            fx.beam(black_box(&line), Color::Cyan);
            black_box(fx.live.len())
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_spawn,
    bench_advance,
    bench_cull,
    bench_current,
    bench_constructors
);
criterion_main!(benches);
