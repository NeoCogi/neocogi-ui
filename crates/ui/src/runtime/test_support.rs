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

use crate::runtime::AtlasHandle;
pub(crate) use crate::test_allocator::AllocationMeasurement;
pub(crate) use neocogi_ui::support::test_support::*;

/// Activates the production GFX factory and lets the runtime create its private compositor.
///
/// Keeping this composition in fixtures leaves the UI implementation dependent only on GFX
/// contracts. Each fixture receives its own compositor and immutable source revision.
pub(crate) fn runtime_from_assets(
    source: neocogi_com::ComRef<'_, neocogi_ui::abi::core::IAssetBundleAbi>,
) -> Result<neocogi_com::ComPtr<neocogi_ui::abi::core::UiRuntimeAbi>, neocogi_com::NcStatus> {
    let gfx = neocogi_ui::support::test_support::gfx_factory(crate::runtime::test_support::host_runtime());
    let mut runtime = crate::runtime::create_runtime_from_assets(source, gfx.as_com_ref())?;
    runtime
        .set_render_host(
            neocogi_ui::gfx::testing::host().as_com_ref(),
            neocogi_ui::gfx::testing::framebuffer().as_com_ref(),
        )
        .map_err(|error| error.status)?;
    Ok(runtime)
}

/// Exports an algorithm fixture once before constructing the production runtime and compositor.
pub(crate) fn create_runtime(atlas: AtlasHandle) -> neocogi_com::ComPtr<neocogi_ui::abi::core::UiRuntimeAbi> {
    let source = crate::runtime::test_support::assets::export_bundle(&crate::runtime::SkinBundle::from_atlas(atlas)).expect("fixture bundle is valid");
    runtime_from_assets(source.as_com_ref()).expect("fixture constructs the production runtime")
}

pub(crate) mod headless;

/// Explicit test asset construction stays outside the deployed importer.
pub(crate) use neocogi_ui::support::test_support::assets;

/// Initializes the selected implementation owned by this test executable.
#[cfg(all(test, feature = "test-support"))]
pub(crate) fn host_runtime() -> neocogi_com::Runtime {
    // SAFETY: this explicit host owns the complete immutable process-lived graph.
    unsafe { neocogi_com::Runtime::__from_host_api(neocogi_com_host::initialize_process()) }.expect("compatible test host")
}
