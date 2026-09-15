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

fn count(bytes: usize) {
    ALLOCATIONS.fetch_add(1, Ordering::Relaxed);
    BYTES.fetch_add(bytes as u64, Ordering::Relaxed);
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
        // SAFETY: `ptr` came from `System` through this allocator.
        unsafe { System.dealloc(ptr, layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        count(new_size);
        // SAFETY: `ptr` came from `System` through this allocator, and the
        // caller upholds `GlobalAlloc::realloc`'s contract.
        unsafe { System.realloc(ptr, layout, new_size) }
    }
}
