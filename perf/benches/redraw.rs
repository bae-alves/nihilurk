//! Regression benchmarks for the terminal redraw: diffing the grid and turning
//! the cells that changed into escape sequences.
//!
//! The stress run reports what a redraw costs while a video plays through it,
//! which mixes in whatever the reel happens to be doing that second. These pin
//! the one variable that actually drives the cost -- how many cells changed --
//! and hold everything else still, so a change to `Screen::flush` shows up as a
//! number rather than a shrug.
//!
//!     cargo bench -p nihilurk-perf --bench redraw
//!
//! `screen.rs` is a module of the binary, not a library, so it is pulled in by
//! path. That is the same copy the rig measures, compiled into this target --
//! not a third version of it.
#[path = "../src/screen.rs"]
mod screen;

use std::hint::black_box;

use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use crossterm::style::Color;

use screen::{SCREEN_H, SCREEN_W, Screen, Sink};

/// Cells changed between one frame and the next.
///
/// `0` is the floor: nothing moved, and `flush` still walks all 2000 cells to
/// prove it -- that scan is the cost a completely static frame pays. `76` is
/// what Bad Apple actually averages, measured. `2000` is every cell on the
/// grid, which is what the first frame after a resize costs.
const CHANGED: [usize; 5] = [0, 76, 400, 1000, 2000];

const CELLS: usize = SCREEN_W as usize * SCREEN_H as usize;

/// A grid with `prev` already established and `changed` cells differing from
/// it, which is the state `flush` is always called in.
fn staged(changed: usize) -> (Screen, Sink) {
    let mut screen = Screen::new();
    let mut sink = Sink::new();
    // Two flushes: the first clears `dirty_all` (a fresh grid repaints whole),
    // the second settles `prev` so only what we dirty below differs.
    let _ = screen.flush(&mut sink, (0, 0));
    let _ = screen.flush(&mut sink, (0, 0));
    sink.take_frame();

    for i in 0..changed {
        let x = (i % SCREEN_W as usize) as u16;
        let y = ((i / SCREEN_W as usize) % SCREEN_H as usize) as u16;
        // A glyph and colour that differ from the blank cell, so the cell is
        // genuinely dirty rather than dirty-looking.
        screen.put(x, y, '#', Color::White);
    }
    (screen, sink)
}

fn bench_flush(c: &mut Criterion) {
    let mut group = c.benchmark_group("redraw");
    for changed in CHANGED {
        // Throughput is per cell *on the grid*, not per changed cell: the scan
        // is over all of them, and that is the part that does not vary.
        group.throughput(Throughput::Elements(CELLS as u64));
        group.bench_with_input(
            BenchmarkId::from_parameter(changed),
            &changed,
            |b, &changed| {
                b.iter_batched_ref(
                    || staged(changed),
                    |(screen, sink)| {
                        let drawn = screen.flush(sink, (0, 0)).unwrap();
                        sink.take_frame();
                        black_box(drawn)
                    },
                    criterion::BatchSize::SmallInput,
                );
            },
        );
    }
    group.finish();
}

/// Painting the grid, as distinct from flushing it: `clear` plus one `put` per
/// lit cell. This is the phase the stress run calls `raster`, and it is the
/// half of a redraw that does not care what changed.
fn bench_paint(c: &mut Criterion) {
    let mut group = c.benchmark_group("paint");
    for lit in [76usize, 787, 1600] {
        group.throughput(Throughput::Elements(lit as u64));
        group.bench_with_input(BenchmarkId::from_parameter(lit), &lit, |b, &lit| {
            let mut screen = Screen::new();
            b.iter(|| {
                screen.clear();
                for i in 0..lit {
                    let x = (i % SCREEN_W as usize) as u16;
                    let y = ((i / SCREEN_W as usize) % SCREEN_H as usize) as u16;
                    screen.put(x, y, '#', Color::White);
                }
                // The grid outlives the closure, so the writes cannot be
                // elided; this just stops the loop itself being reordered out.
                black_box(&mut screen);
            });
        });
    }
    group.finish();
}

criterion_group!(benches, bench_flush, bench_paint);
criterion_main!(benches);
