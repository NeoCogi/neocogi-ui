//
// Copyright 2023-Present (c) Raja Lehtihet & Wael El Oraiby
//
// Redistribution and use in source and binary forms, with or without
// modification, are permitted provided that the following conditions are met:
//
// 1. Redistributions of source code must retain the above copyright notice,
// this list of conditions and the following disclaimer.
//
// 2. Redistributions in binary form must reproduce the above copyright notice,
// this list of conditions and the following disclaimer in the documentation
// and/or other materials provided with the distribution.
//
// 3. Neither the name of the copyright holder nor the names of its contributors
// may be used to endorse or promote products derived from this software without
// specific prior written permission.
//
// THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS "AS IS"
// AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE
// IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE
// ARE DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT HOLDER OR CONTRIBUTORS BE
// LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR
// CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF
// SUBSTITUTE GOODS OR SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS
// INTERRUPTION) HOWEVER CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN
// CONTRACT, STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE)
// ARISING IN ANY WAY OUT OF THE USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE
// POSSIBILITY OF SUCH DAMAGE.
//

//! Deterministic resources, direct algorithm ownership and public-runtime fixtures.

use crate::AtlasHandle;
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
        let pointer = unsafe { System.alloc(layout) };
        count_allocation(pointer, layout.size());
        pointer
    }

    /// Preserves system zero-initialization before observing the allocation.
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        let pointer = unsafe { System.alloc_zeroed(layout) };
        count_allocation(pointer, layout.size());
        pointer
    }

    /// Returns storage to the same allocator without affecting allocation totals.
    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        unsafe { System.dealloc(pointer, layout) };
    }

    /// Preserves realloc semantics and counts successful capacity changes.
    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
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

pub(crate) use neocogi_ui_support::test_support::*;

/// Activates the production GFX factory and lets the runtime create its private compositor.
///
/// Keeping this composition in fixtures leaves the UI implementation dependent only on GFX
/// contracts. Each fixture receives its own compositor and immutable source revision.
pub(crate) fn runtime_from_assets(
    source: neocogi_com::ComRef<'_, neocogi_ui::abi::core::IAssetBundleAbi>,
) -> Result<neocogi_com::ComPtr<neocogi_ui::abi::core::UiRuntimeAbi>, neocogi_com::NcStatus> {
    let gfx = neocogi_ui_support::test_support::gfx_factory(crate::test_support::host_runtime());
    let mut runtime = crate::create_runtime_from_assets(source, gfx.as_com_ref())?;
    runtime.set_render_host(neocogi_gfx_support::testing::host().as_com_ref(), neocogi_gfx_support::testing::framebuffer().as_com_ref()).map_err(|error| error.status)?;
    Ok(runtime)
}

/// Exports an algorithm fixture once before constructing the production runtime and compositor.
pub(crate) fn create_runtime(atlas: AtlasHandle) -> neocogi_com::ComPtr<neocogi_ui::abi::core::UiRuntimeAbi> {
    let source = crate::test_support::assets::export_bundle(&crate::SkinBundle::from_atlas(atlas)).expect("fixture bundle is valid");
    runtime_from_assets(source.as_com_ref()).expect("fixture constructs the production runtime")
}

pub(crate) mod headless;

/// Explicit test asset construction stays outside the deployed importer.
pub(crate) use neocogi_ui_support::test_support::assets;

/// Initializes the selected implementation owned by this test executable.
#[cfg(test)]
pub(crate) fn host_runtime() -> neocogi_com::Runtime {
    // SAFETY: this explicit host owns the complete immutable process-lived graph.
    unsafe { neocogi_com::Runtime::__from_host_api(neocogi_com_host::initialize_process()) }.expect("compatible test host")
}
