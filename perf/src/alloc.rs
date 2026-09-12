//! A counting wrapper around the system allocator.
//!
//! RSS answers "how many pages does the kernel have mapped for us", which is
//! the wrong question for this rig and answers it late: the allocator caches
//! freed blocks, so a workload that churns hard and frees everything shows a
//! flat RSS line while doing millions of allocations a second. That is exactly
//! the shape of roog's particle layer, where every mote heap-allocates a
//! `Vec<(char, Color)>` for its keyframes and drops it a few frames later.
//!
//! So the rig counts the calls itself. Three relaxed atomics updated on the
//! allocation path cost a few nanoseconds each and, unlike RSS, they move the
//! instant a `blip` happens. This is the piece that replaces Valgrind/Heaptrack
//! for the live run: not a full call-graph attribution, but exact totals, free,
//! and readable at 30 Hz without perturbing what is being measured.
//!
//! Relaxed is the correct ordering here. The counters guard no other memory --
//! nothing is published through them -- and a reader only ever wants a recent
//! value, not a synchronised one.

use std::alloc::{GlobalAlloc, Layout};
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering::Relaxed};

static LIVE_BYTES: AtomicUsize = AtomicUsize::new(0);
static LIVE_BLOCKS: AtomicUsize = AtomicUsize::new(0);
static TOTAL_ALLOCS: AtomicU64 = AtomicU64::new(0);
static TOTAL_BYTES: AtomicU64 = AtomicU64::new(0);

/// Wraps another allocator, counting what passes through. Install it with
/// `#[global_allocator] static A: Tracking<System> = Tracking::new(System);`.
pub struct Tracking<A> {
    inner: A,
}

impl<A> Tracking<A> {
    pub const fn new(inner: A) -> Self {
        Self { inner }
    }
}

// SAFETY: every method forwards to `inner`, which is a valid allocator, and
// returns its pointer unchanged. The counter updates touch no memory the
// allocator owns and cannot unwind.
unsafe impl<A: GlobalAlloc> GlobalAlloc for Tracking<A> {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let ptr = unsafe { self.inner.alloc(layout) };
        if !ptr.is_null() {
            record_alloc(layout.size());
        }
        ptr
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        let ptr = unsafe { self.inner.alloc_zeroed(layout) };
        if !ptr.is_null() {
            record_alloc(layout.size());
        }
        ptr
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        LIVE_BYTES.fetch_sub(layout.size(), Relaxed);
        LIVE_BLOCKS.fetch_sub(1, Relaxed);
        unsafe { self.inner.dealloc(ptr, layout) }
    }

    // Forwarded rather than left to the trait's default, which is
    // alloc-copy-dealloc. The system allocator can often grow a block in place
    // (and on Linux `mremap` a large one), and a rig that quietly disabled that
    // would be measuring its own instrumentation.
    //
    // The live *block* count is deliberately left alone -- a grown `Vec` is the
    // same one block it was before -- while the call itself still counts toward
    // the allocation total, because a realloc costs roughly what an allocation
    // costs and hiding it would understate the churn.
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        let new_ptr = unsafe { self.inner.realloc(ptr, layout, new_size) };
        if new_ptr.is_null() {
            return new_ptr;
        }
        LIVE_BYTES.fetch_add(new_size.wrapping_sub(layout.size()), Relaxed);
        TOTAL_ALLOCS.fetch_add(1, Relaxed);
        TOTAL_BYTES.fetch_add(new_size.saturating_sub(layout.size()) as u64, Relaxed);
        new_ptr
    }
}

fn record_alloc(size: usize) {
    LIVE_BYTES.fetch_add(size, Relaxed);
    LIVE_BLOCKS.fetch_add(1, Relaxed);
    TOTAL_ALLOCS.fetch_add(1, Relaxed);
    TOTAL_BYTES.fetch_add(size as u64, Relaxed);
}

/// A reading of the counters. Cheap enough to take every frame.
#[derive(Clone, Copy, Default)]
pub struct HeapStats {
    /// Bytes requested and not yet freed. Rises and falls with the particle
    /// population; the number RSS is too coarse to show.
    pub live_bytes: usize,
    /// Outstanding allocations. One `Vec` of keyframes per live mote, plus
    /// whatever the UI is holding.
    pub live_blocks: usize,
    /// Every allocation since the process started. The churn number -- this is
    /// the one that runs into the millions.
    pub total_allocs: u64,
    /// Every byte ever requested.
    pub total_bytes: u64,
}

pub fn stats() -> HeapStats {
    HeapStats {
        live_bytes: LIVE_BYTES.load(Relaxed),
        live_blocks: LIVE_BLOCKS.load(Relaxed),
        total_allocs: TOTAL_ALLOCS.load(Relaxed),
        total_bytes: TOTAL_BYTES.load(Relaxed),
    }
}
