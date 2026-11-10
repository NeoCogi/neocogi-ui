//! Dialog-specific observations use the implementation unit test's one shared allocator.
use crate::test_allocator::AllocationMeasurement;

/// Allocation/release observations used by dialog lifetime assertions.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub(super) struct Counts {
    /// Successful allocation and reallocation requests.
    pub allocations: usize,
    /// Explicit releases, excluding reallocations.
    pub deallocations: usize,
}

/// Measures one synchronous operation without nesting measurement windows.
pub(super) fn measure<T>(operation: impl FnOnce() -> T) -> (T, Counts) {
    let measurement = AllocationMeasurement::begin();
    let result = operation();
    let count = measurement.finish();
    (
        result,
        Counts {
            allocations: usize::try_from(count.events).expect("bounded test allocation count"),
            deallocations: count.deallocations,
        },
    )
}
