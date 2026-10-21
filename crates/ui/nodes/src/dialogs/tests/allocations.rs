//! Thread-local allocation observations around real picker calls, without production hooks.
//!
//! All other node tests continue using the system allocator. Counting is enabled only
//! on the calling thread after fixtures are built; it neither allocates nor locks.
use std::{
    alloc::{GlobalAlloc, Layout, System},
    cell::Cell,
};

/// Allocator calls made during one synchronous operation; no byte or timing claim.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(super) struct Counts {
    /// Successful allocation requests, including zeroed allocations and reallocations.
    pub allocations: usize,
    /// Explicit deallocation requests; reallocations are not counted as deallocations.
    pub deallocations: usize,
}

std::thread_local! {
    /// This test observation is thread-local because the allocator is shared by parallel tests.
    /// Cell permits allocation callbacks to update counters without borrowing test state.
    static COUNTS: Cell<Option<Counts>> = const { Cell::new(None) };
}

/// Uses the same system allocator while optionally observing this test thread.
struct ObservedAllocator;

#[global_allocator]
static ALLOCATOR: ObservedAllocator = ObservedAllocator;

/// Records a callback without allocating, including during thread-local teardown.
fn record(allocation: bool) {
    let _ = COUNTS.try_with(|cell| {
        if let Some(mut counts) = cell.get() {
            if allocation {
                counts.allocations += 1;
            } else {
                counts.deallocations += 1;
            }
            cell.set(Some(counts));
        }
    });
}

// SAFETY: Every callback forwards the original pointer, layout and size unchanged
// to System. Observation uses const-initialized, nonallocating thread-local cells.
unsafe impl GlobalAlloc for ObservedAllocator {
    /// Observes successful allocation without changing the allocation contract.
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        // SAFETY: The caller supplies a valid allocation layout.
        let pointer = unsafe { System.alloc(layout) };
        if !pointer.is_null() {
            record(true);
        }
        pointer
    }
    /// Preserves zero initialization while counting a successful request.
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        // SAFETY: The caller supplies a valid allocation layout.
        let pointer = unsafe { System.alloc_zeroed(layout) };
        if !pointer.is_null() {
            record(true);
        }
        pointer
    }
    /// Observes successful reallocation; in-place growth is still one allocator call.
    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        // SAFETY: The caller supplies its live allocation, original layout and valid new size.
        let replacement = unsafe { System.realloc(pointer, layout, size) };
        if !replacement.is_null() {
            record(true);
        }
        replacement
    }
    /// Counts release before freeing through the original allocator.
    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        record(false);
        // SAFETY: The caller supplies the live allocation and its original layout.
        unsafe { System.dealloc(pointer, layout) }
    }
}

/// Disables observation if the measured test operation unwinds.
struct Reset;
impl Drop for Reset {
    fn drop(&mut self) {
        COUNTS.with(|cell| cell.set(None));
    }
}

/// Measures an isolated synchronous call, leaving formatting and fixture work outside.
pub(super) fn measure<T>(operation: impl FnOnce() -> T) -> (T, Counts) {
    COUNTS.with(|cell| {
        assert!(cell.get().is_none(), "allocation measurements cannot nest");
        cell.set(Some(Counts::default()));
    });
    let reset = Reset;
    let result = operation();
    let counts = COUNTS.with(|cell| cell.replace(None).unwrap());
    drop(reset);
    (result, counts)
}
