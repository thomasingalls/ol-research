//! A counting global allocator (plan §5, rule 13): how many allocations a
//! run makes is a number in the report, and a budget afterwards.
//!
//! The binary installs [`Counting`] as its global allocator; the runner
//! reads the counters before and after a run. When it is not installed the
//! counters stay at zero and the report says nothing about allocations.
//!
//! Under the counting is mimalloc, not the system's allocator (deployment
//! plan §2): the counts are of the program's requests, so they do not
//! change with it, and the dev store's rebuild ran 7-8% faster on it.
//!
//! Each thread counts in a tally of its own, alone on its cache line, and a
//! reading adds the tallies up: one set of counters that every allocation
//! on every core wrote made a parse on ten cores about 2.5 times slower
//! (2026-09-29, ingestion plan §10). Allocations, bytes and live bytes are
//! exact; the peak is looked for as the tallies grow, so it can read low by
//! up to [`PEAK_STEP`] per thread (§13).

use std::alloc::{GlobalAlloc, Layout};
use std::cell::Cell;
use std::sync::atomic::{AtomicI64, AtomicU64, AtomicUsize, Ordering};

use mimalloc::MiMalloc;

/// How many tallies the threads share out. A thread keeps to one, handed
/// out in turn, so up to this many threads count without writing a line
/// another thread writes; past it, two threads share one, still exactly.
const TALLIES: usize = 64;

/// How far one tally's live bytes grow before it looks at the whole heap
/// for a new peak.
pub const PEAK_STEP: u64 = 1 << 20;

/// One thread's counts. `live` can go below zero: a block freed on this
/// thread may have been allocated on another.
#[repr(align(128))]
struct Tally {
    allocations: AtomicU64,
    bytes: AtomicU64,
    live: AtomicI64,
    /// `live` when this tally last looked for the peak, or lower since.
    looked: AtomicI64,
}

impl Tally {
    const fn new() -> Self {
        Self {
            allocations: AtomicU64::new(0),
            bytes: AtomicU64::new(0),
            live: AtomicI64::new(0),
            looked: AtomicI64::new(0),
        }
    }
}

static TALLY: [Tally; TALLIES] = [const { Tally::new() }; TALLIES];
static NEXT: AtomicUsize = AtomicUsize::new(0);
static PEAK: AtomicU64 = AtomicU64::new(0);

thread_local! {
    /// This thread's tally, chosen at its first allocation. A constant
    /// initialiser and no destructor: reaching it never allocates.
    static MINE: Cell<usize> = const { Cell::new(usize::MAX) };
}

fn mine() -> &'static Tally {
    let i = MINE
        .try_with(|m| {
            let mut i = m.get();
            if i == usize::MAX {
                i = NEXT.fetch_add(1, Ordering::Relaxed) % TALLIES;
                m.set(i);
            }
            i
        })
        .unwrap_or(0);
    &TALLY[i]
}

#[allow(clippy::cast_possible_wrap, reason = "no allocation is 2^63 bytes")]
fn signed(bytes: usize) -> i64 {
    bytes as i64
}

/// mimalloc, counting every allocation and reallocation.
#[derive(Debug, Default, Clone, Copy)]
pub struct Counting;

/// Allocations and reallocations so far, in this process.
#[must_use]
pub fn allocations() -> u64 {
    TALLY
        .iter()
        .map(|t| t.allocations.load(Ordering::Relaxed))
        .sum()
}

/// Bytes requested by those allocations, not net of frees.
#[must_use]
pub fn allocated_bytes() -> u64 {
    TALLY.iter().map(|t| t.bytes.load(Ordering::Relaxed)).sum()
}

/// Bytes allocated and not yet freed, now: the heap the process holds, as
/// it asked for it — the allocator's own overhead and pages it keeps from
/// freed blocks are not in it, so the resident set is larger.
#[must_use]
pub fn live_bytes() -> u64 {
    let live: i64 = TALLY.iter().map(|t| t.live.load(Ordering::Relaxed)).sum();
    u64::try_from(live).unwrap_or(0)
}

/// The most [`live_bytes`] has been, in this process. Read at a phase's
/// end it says what the phase needed at its worst, which a reading of
/// [`live_bytes`] at the same moment does not (2026-09-24: the corpus
/// merge was killed for memory, and where it goes is the question).
#[must_use]
pub fn peak_bytes() -> u64 {
    let now = live_bytes();
    PEAK.fetch_max(now, Ordering::Relaxed).max(now)
}

/// Starts [`peak_bytes`] again from [`live_bytes`], so that the next
/// reading of it is one phase's worst rather than the process's (the
/// serving layer's memory, 2026-09-25: the corpus's publish peaked at 18 GB
/// and which of its steps did it is the question).
pub fn reset_peak() {
    PEAK.store(live_bytes(), Ordering::Relaxed);
}

fn count(tally: &Tally, bytes: usize) {
    tally.allocations.fetch_add(1, Ordering::Relaxed);
    tally.bytes.fetch_add(bytes as u64, Ordering::Relaxed);
}

fn grow(tally: &Tally, bytes: usize) {
    let live = tally.live.fetch_add(signed(bytes), Ordering::Relaxed) + signed(bytes);
    #[allow(clippy::cast_possible_wrap, reason = "a constant of 2^20")]
    if live - tally.looked.load(Ordering::Relaxed) > PEAK_STEP as i64 {
        tally.looked.store(live, Ordering::Relaxed);
        PEAK.fetch_max(live_bytes(), Ordering::Relaxed);
    }
}

fn shrink(tally: &Tally, bytes: usize) {
    let live = tally.live.fetch_sub(signed(bytes), Ordering::Relaxed) - signed(bytes);
    if live < tally.looked.load(Ordering::Relaxed) {
        tally.looked.store(live, Ordering::Relaxed);
    }
}

// SAFETY: every call is forwarded to `MiMalloc` unchanged; the counters are
// atomics touched only with relaxed loads and stores, and the thread-local
// that picks a tally neither allocates nor has a destructor.
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let tally = mine();
        count(tally, layout.size());
        grow(tally, layout.size());
        // SAFETY: the caller upholds `GlobalAlloc::alloc`'s contract.
        unsafe { MiMalloc.alloc(layout) }
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        let tally = mine();
        count(tally, layout.size());
        grow(tally, layout.size());
        // SAFETY: as above.
        unsafe { MiMalloc.alloc_zeroed(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        shrink(mine(), layout.size());
        // SAFETY: `ptr` came from `MiMalloc` through this allocator.
        unsafe { MiMalloc.dealloc(ptr, layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        // Counted as an allocation of the new size, as before; the live
        // bytes move by the difference, and only if the move succeeded.
        let tally = mine();
        count(tally, new_size);
        // SAFETY: `ptr` came from `MiMalloc` through this allocator, and the
        // caller upholds `GlobalAlloc::realloc`'s contract.
        let moved = unsafe { MiMalloc.realloc(ptr, layout, new_size) };
        if !moved.is_null() {
            grow(tally, new_size);
            shrink(tally, layout.size());
        }
        moved
    }
}
