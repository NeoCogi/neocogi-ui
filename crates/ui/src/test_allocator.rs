//! One unit-test allocator shared by runtime and node allocation regressions.

use std::cell::Cell;
use std::alloc::{GlobalAlloc, Layout, System};

/// Test-process allocator whose counters are enabled only inside an explicit measurement window.
pub(crate) struct CountingAllocator;

/// Transparent system allocator with opt-in thread-local observations.
#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;

thread_local! {
    /// Isolates parallel tests; the allocator API has shared receivers and cannot borrow a guard.
    static ALLOCATION_MEASUREMENT: Cell<Option<AllocationCount>> = const { Cell::new(None) };
}

// SAFETY: Every allocation and deallocation is forwarded unchanged to the system allocator.
// Observation uses only fixed-size thread-local values and never recursively allocates.
unsafe impl GlobalAlloc for CountingAllocator {
    /// Delegates allocation and counts a successful result only inside a measurement window.
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        // SAFETY: the caller supplies the valid allocation layout required by GlobalAlloc.
        let pointer = unsafe { System.alloc(layout) };
        count_allocation(pointer, layout.size());
        pointer
    }

    /// Preserves system zero-initialization before observing the allocation.
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        // SAFETY: the caller supplies the valid allocation layout required by GlobalAlloc.
        let pointer = unsafe { System.alloc_zeroed(layout) };
        count_allocation(pointer, layout.size());
        pointer
    }

    /// Returns storage to the same allocator without affecting allocation totals.
    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        let _ = ALLOCATION_MEASUREMENT.try_with(|measurement| {
            if let Some(mut count) = measurement.get() {
                count.deallocations = count.deallocations.saturating_add(1);
                measurement.set(Some(count));
            }
        });
        // SAFETY: the caller supplies a live system allocation and its original layout.
        unsafe { System.dealloc(pointer, layout) };
    }

    /// Preserves realloc semantics and counts successful capacity changes.
    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        // SAFETY: the caller supplies live system storage, its layout and a valid new size.
        let pointer = unsafe { System.realloc(pointer, layout, new_size) };
        count_allocation(pointer, new_size);
        pointer
    }
}

/// Records bounded copied counters while tolerating thread-local teardown.
fn count_allocation(pointer: *mut u8, bytes: usize) {
    if pointer.is_null() {
        return;
    }
    let _ = ALLOCATION_MEASUREMENT.try_with(|measurement| {
        if let Some(mut count) = measurement.get() {
            count.events = count.events.saturating_add(1);
            count.bytes = count.bytes.saturating_add(bytes as u64);
            measurement.set(Some(count));
        }
    });
}

/// Aggregate allocation and reallocation activity from one isolated test window.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct AllocationCount {
    /// Successful allocation/reallocation requests in the measured region.
    pub(crate) events: u64,
    /// Sum of requested allocation sizes, including successful reallocations.
    pub(crate) bytes: u64,
    /// Explicit release requests; reallocations remain allocation events only.
    pub(crate) deallocations: usize,
}

/// Enables allocation counters on the calling thread until [`AllocationMeasurement::finish`] is
/// called or the guard is dropped.
pub(crate) struct AllocationMeasurement {
    /// True until an explicit finish transfers the counters to the assertion owner.
    active: bool,
}

impl AllocationMeasurement {
    /// Starts one non-nested window on the current test thread.
    pub(crate) fn begin() -> Self {
        ALLOCATION_MEASUREMENT.with(|measurement| {
            assert!(
                measurement.replace(Some(AllocationCount::default())).is_none(),
                "allocation measurement windows must not overlap on one thread"
            );
        });
        Self { active: true }
    }

    /// Ends observation before assertions can allocate formatting buffers.
    pub(crate) fn finish(mut self) -> AllocationCount {
        self.active = false;
        ALLOCATION_MEASUREMENT.with(|measurement| measurement.replace(None).expect("allocation measurement is active"))
    }
}

impl Drop for AllocationMeasurement {
    /// Cancels unfinished measurement after either normal return or panic.
    fn drop(&mut self) {
        if self.active {
            let _ = ALLOCATION_MEASUREMENT.try_with(|measurement| measurement.set(None));
        }
    }
}
