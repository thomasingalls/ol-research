//! A counting global allocator (plan §5, rule 13): how many allocations a
//! run makes is a number in the report, and a budget afterwards.
//!
//! The binary installs [`Counting`] as its global allocator; the runner
//! reads the counters before and after a run. When it is not installed the
//! counters stay at zero and the report says nothing about allocations.

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicU64, Ordering};

static ALLOCATIONS: AtomicU64 = AtomicU64::new(0);
static BYTES: AtomicU64 = AtomicU64::new(0);
static LIVE: AtomicU64 = AtomicU64::new(0);
static PEAK: AtomicU64 = AtomicU64::new(0);

/// The system allocator, counting every allocation and reallocation.
#[derive(Debug, Default, Clone, Copy)]
pub struct Counting;

/// Allocations and reallocations so far, in this process.
#[must_use]
pub fn allocations() -> u64 {
    ALLOCATIONS.load(Ordering::Relaxed)
}

/// Bytes requested by those allocations, not net of frees.
#[must_use]
pub fn allocated_bytes() -> u64 {
    BYTES.load(Ordering::Relaxed)
}

/// Bytes allocated and not yet freed, now: the heap the process holds, as
/// it asked for it — the allocator's own overhead and pages it keeps from
/// freed blocks are not in it, so the resident set is larger.
#[must_use]
pub fn live_bytes() -> u64 {
    LIVE.load(Ordering::Relaxed)
}

/// The most [`live_bytes`] has been, in this process. Read at a phase's
/// end it says what the phase needed at its worst, which a reading of
/// [`live_bytes`] at the same moment does not (2026-09-24: the corpus
/// merge was killed for memory, and where it goes is the question).
#[must_use]
pub fn peak_bytes() -> u64 {
    PEAK.load(Ordering::Relaxed)
}

/// Starts [`peak_bytes`] again from [`live_bytes`], so that the next
/// reading of it is one phase's worst rather than the process's (the
/// serving layer's memory, 2026-09-25: the corpus's publish peaked at 18 GB
/// and which of its steps did it is the question).
pub fn reset_peak() {
    PEAK.store(LIVE.load(Ordering::Relaxed), Ordering::Relaxed);
}

fn count(bytes: usize) {
    ALLOCATIONS.fetch_add(1, Ordering::Relaxed);
    BYTES.fetch_add(bytes as u64, Ordering::Relaxed);
    grow(bytes);
}

fn grow(bytes: usize) {
    let live = LIVE.fetch_add(bytes as u64, Ordering::Relaxed) + bytes as u64;
    PEAK.fetch_max(live, Ordering::Relaxed);
}

fn shrink(bytes: usize) {
    LIVE.fetch_sub(bytes as u64, Ordering::Relaxed);
}

// SAFETY: every call is forwarded to `System` unchanged; the counters are
// atomics touched only with relaxed loads and stores.
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        count(layout.size());
        // SAFETY: the caller upholds `GlobalAlloc::alloc`'s contract.
        unsafe { System.alloc(layout) }
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        count(layout.size());
        // SAFETY: as above.
        unsafe { System.alloc_zeroed(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        shrink(layout.size());
        // SAFETY: `ptr` came from `System` through this allocator.
        unsafe { System.dealloc(ptr, layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        // Counted as an allocation of the new size, as before; the live
        // bytes move by the difference, and only if the move succeeded.
        ALLOCATIONS.fetch_add(1, Ordering::Relaxed);
        BYTES.fetch_add(new_size as u64, Ordering::Relaxed);
        // SAFETY: `ptr` came from `System` through this allocator, and the
        // caller upholds `GlobalAlloc::realloc`'s contract.
        let moved = unsafe { System.realloc(ptr, layout, new_size) };
        if !moved.is_null() {
            grow(new_size);
            shrink(layout.size());
        }
        moved
    }
}
