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

use crate::{
    AtlasHandle, AtlasSource, CharEntry, ChromeRole, ChromeState, ControlRole, ControlState, FontEntry, NinePatch, Recti, Skin, SurfaceRole, SurfaceState,
    Vec2i, Visual,
};
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

/// Required semantic icon names make the small fixture a valid complete asset catalog.
const ICON_NAMES: [&str; 9] = [
    "white",
    "close",
    "expand",
    "collapse",
    "check",
    "expand_down",
    "open_folder",
    "closed_folder",
    "file",
];

/// Constructs the standard immutable atlas shared within one test's ownership domain.
pub(crate) fn test_atlas() -> AtlasHandle {
    // Use the required semantic body name so runtime and skin validation see a complete catalog.
    test_atlas_with_font_sizes(&[("body", 10)])
}

/// Constructs a standard icon set plus caller-selected named font metrics.
pub(crate) fn test_atlas_with_font_sizes(fonts: &[(&str, usize)]) -> AtlasHandle {
    let pixels: [u8; 4] = [0xFF, 0xFF, 0xFF, 0xFF];
    let icons: Vec<(&str, Recti)> = ICON_NAMES.iter().map(|name| (*name, Recti::new(0, 0, 1, 1))).collect();
    let entries = vec![
        (
            '_',
            CharEntry {
                offset: Vec2i::new(0, 0),
                advance: Vec2i::new(8, 0),
                rect: Recti::new(0, 0, 1, 1),
            },
        ),
        (
            'a',
            CharEntry {
                offset: Vec2i::new(0, 0),
                advance: Vec2i::new(8, 0),
                rect: Recti::new(0, 0, 1, 1),
            },
        ),
        (
            'b',
            CharEntry {
                offset: Vec2i::new(0, 0),
                advance: Vec2i::new(8, 0),
                rect: Recti::new(0, 0, 1, 1),
            },
        ),
    ];
    let fonts: Vec<(&str, FontEntry<'_>)> = fonts
        .iter()
        .map(|(name, size)| {
            (
                *name,
                FontEntry {
                    line_size: *size,
                    baseline: (*size as i32 * 4) / 5,
                    font_size: *size,
                    entries: &entries,
                },
            )
        })
        .collect();
    let source = AtlasSource {
        width: 1,
        height: 1,
        pixels: &pixels,
        icons: &icons,
        fonts: &fonts,
    };
    // Shared fixtures use the public fallible loader so malformed test metadata cannot bypass the
    // same validation boundary required of downstream applications.
    AtlasHandle::try_from(&source).expect("shared test atlas must satisfy the complete atlas contract")
}

/// Constructs a resolved skin whose resource capabilities belong to `atlas`.
pub(crate) fn test_skin(atlas: &AtlasHandle) -> Skin {
    // Tests must retain and pass this same handle allocation; reconstructing identical metadata is
    // intentionally a different ownership domain after atlas IDs become scoped.
    Skin::from_atlas(atlas)
}

/// Replaces every structural-surface patch while retaining fixture content colors.
pub(crate) fn replace_surface_patches(skin: &mut Skin, role: SurfaceRole, mut patch: impl FnMut(SurfaceState) -> NinePatch) {
    // Iterate the exact surface state domain and publish complete visuals one slot at a time.
    for state in SurfaceState::ALL {
        skin.set_surface(role, state, Visual::new(patch(state), skin.surface(role, state).content_color));
    }
}

/// Replaces every control patch while retaining fixture content colors.
pub(crate) fn replace_control_patches(skin: &mut Skin, role: ControlRole, mut patch: impl FnMut(ControlState) -> NinePatch) {
    // Tests express state differences through a typed closure instead of rebuilding a generic table.
    for state in ControlState::ALL {
        skin.set_control(role, state, Visual::new(patch(state), skin.control(role, state).content_color));
    }
}

/// Replaces one chrome patch while retaining that state's content color.
pub(crate) fn replace_chrome_patch(skin: &mut Skin, role: ChromeRole, state: ChromeState, patch: NinePatch) {
    // Resolve both halves before publishing one complete chrome visual.
    skin.set_chrome(role, state, Visual::new(patch, skin.chrome(role, state).content_color));
}

#[neocogi_com::com_object(interfaces(neocogi_ui_core_contracts::ITexture))]
/// CPU-only immutable texture supplies dimensions without any native GPU handle.
struct TestImage {
    /// Positive test dimensions used by the real owning facade image wrapper.
    size: neocogi_ui_core_contracts::Size,
}
impl neocogi_ui_core_contracts::ITexture for TestImage {
    /// A real COM projection obeys the production immutable-dimension contract.
    fn size(&self) -> Result<neocogi_ui_core_contracts::Size, neocogi_com::NcStatus> {
        Ok(self.size)
    }
}
/// Constructs an ordinary owned COM image for algorithm measurement fixtures.
pub(crate) fn test_image(width: i32, height: i32) -> neocogi_ui::Image {
    neocogi_ui::Image::from_interface(
        TestImage::from_value(TestImage {
            size: neocogi_ui_core_contracts::Size { width, height },
        })
        .query()
        .unwrap(),
    )
    .unwrap()
}

/// Constructs an actual GFX compositor and then uses the production runtime entrypoint.
///
/// Keeping this composition in fixtures leaves the UI implementation dependent only on GFX
/// contracts. Each fixture receives its own compositor and immutable source revision.
pub(crate) fn runtime_from_assets(
    source: neocogi_com::ComRef<'_, neocogi_ui_core_contracts::IAssetBundleAbi>,
) -> Result<neocogi_com::ComPtr<neocogi_ui_core_contracts::UiRuntimeAbi>, neocogi_com::NcStatus> {
    let compositor = neocogi_gfx_impl::GfxCompositorObject::create(source)?;
    crate::create_runtime_from_assets(source, compositor.as_com_ref())
}

/// Exports an algorithm fixture once before constructing the production runtime and compositor.
pub(crate) fn create_runtime(atlas: AtlasHandle) -> neocogi_com::ComPtr<neocogi_ui_core_contracts::UiRuntimeAbi> {
    let source = crate::test_support::assets::export_bundle(&crate::SkinBundle::from_atlas(atlas)).expect("fixture bundle is valid");
    runtime_from_assets(source.as_com_ref()).expect("fixture constructs the production runtime")
}

pub(crate) mod headless;

/// Direct algorithm ownership keeps the host outside production widget storage.
pub(crate) mod algorithm;

/// Explicit test asset construction stays outside the deployed importer.
pub(crate) use crate::assets::fixtures as assets;
