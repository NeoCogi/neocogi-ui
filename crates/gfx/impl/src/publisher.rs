//! Resource-preserving GFX revision publication independent of optional atlas construction.

use super::Diagnostic;
use neocogi_com::{ComPtr, ComRef, INcStringAbi, NC_E_INVALID_ARG, NC_E_OUT_OF_MEMORY, NcCopyFromBufferOffset, NcStatus, com_factory};
use neocogi_gfx_contracts::*;
use neocogi_gfx_support::AssetTables;

/// GFX publication root with diagnostic state owned independently of returned revisions.
#[com_factory(neocogi_gfx_contracts::AssetPublisher)]
#[derive(Default)]
pub struct AssetPublisherObject {
    /// Last transfer, validation, or publication failure observed by this service instance.
    diagnostic: Diagnostic,
}

impl AssetPublisher for AssetPublisherObject {
    /// Copies appearance-owned records into a validated revision without interpreting Skin policy.
    fn revise_appearance(
        &self,
        base: ComRef<'_, IAssetBundleAbi>,
        revision: AssetAppearanceRevision,
        surfaces: NcCopyFromBufferOffset<'_, AssetSurfaceVisuals>,
        controls: NcCopyFromBufferOffset<'_, AssetControlVisuals>,
        menus: NcCopyFromBufferOffset<'_, AssetMenuVisuals>,
        window_chrome: NcCopyFromBufferOffset<'_, AssetChromeVisuals>,
    ) -> Result<ComPtr<IAssetBundleAbi>, NcStatus> {
        // GFX owns bundle transfer, validation, and publication. The caller supplies only closed
        // appearance records, so neither its schema nor its authoring model leaks into this layer.
        // Read only metadata and the required white tile. The immutable image is
        // retained by the new bundle, eliminating full pixel copies and GPU uploads.
        let mut data = AssetTables::read(base).inspect_err(|status| {
            self.diagnostic.failure(status);
        })?;
        data.metrics = revision.metrics;
        data.chrome = revision.chrome;
        data.surfaces = copied_records(surfaces, SurfaceRole::COUNT)?.into();
        data.controls = copied_records(controls, ControlRole::COUNT)?.into();
        data.menus = copied_records(menus, MenuRole::COUNT)?.into();
        data.window_chrome = copied_records(window_chrome, ChromeRole::COUNT)?.into();
        data.into_revision(base).inspect_err(|status| {
            self.diagnostic.failure(status);
        })
    }
}

impl IGfxDiagnostics for AssetPublisherObject {
    /// Returns an owned snapshot that remains unchanged by later publication failures.
    fn diagnostic(&self) -> Result<ComPtr<INcStringAbi>, NcStatus> {
        Ok(self.diagnostic.snapshot())
    }
}

/// Copies one complete fixed-role table after validating its ABI range and exact cardinality.
fn copied_records<T: Copy>(input: NcCopyFromBufferOffset<'_, T>, expected: usize) -> Result<Vec<T>, NcStatus> {
    if input.offset() != 0 || input.len() != expected {
        return Err(NC_E_INVALID_ARG);
    }
    let mut records = Vec::new();
    records.try_reserve_exact(expected).map_err(|_| NC_E_OUT_OF_MEMORY)?;
    records.extend_from_slice(input.as_slice());
    Ok(records)
}
