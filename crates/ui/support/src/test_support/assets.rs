//! Input construction for asset-boundary and algorithm tests; this code never imports a runtime frame.
use crate::{ResourceCatalog, SkinBundle};
use crate::assets::ImportedAssets;
use neocogi_ui_core_contracts::IAssetBundleAbi;
use neocogi_com::{ComPtr, NcStatus, NC_E_INVALID_ARG};

/// Copies pixels explicitly for a fixture revision instead of adding transfers to runtime import.
pub fn export_imported_data(imported: &ImportedAssets, bundle: &SkinBundle) -> Result<neocogi_ui::assets::AssetData, NcStatus> {
    let mut data = export_bundle_data(bundle)?;
    let count = imported.source().info()?.rgba_bytes()?;
    let mut pixels = neocogi_com::NcOutputBuffer::new();
    imported.image().pixels(pixels.output(0, count))?;
    data.pixels = pixels.into_vec().into();
    Ok(data)
}

impl ImportedAssets {
    /// Exports a locally derived bundle with the same authenticated catalog namespace.
    pub fn export_derived(&self, bundle: &SkinBundle) -> Result<ComPtr<IAssetBundleAbi>, NcStatus> {
        ResourceCatalog::new(self.original.clone())
            .validate_bundle(bundle)
            .map_err(|_| NC_E_INVALID_ARG)?;
        export_imported_data(self, bundle)?.into_revision(self.source())
    }
}

/// Exports a validated native bundle into a fresh logical catalog, with no parser dependency.
pub fn export_bundle(bundle: &SkinBundle) -> Result<ComPtr<IAssetBundleAbi>, NcStatus> {
    export_bundle_data(bundle)?.into_bundle()
}

/// Produces the same concrete tables consumed by generated prebuilt source and runtime providers.
pub fn export_bundle_data(bundle: &SkinBundle) -> Result<neocogi_ui::assets::AssetData, NcStatus> {
    let atlas = bundle.atlas();
    // Test publication copies the already canonical snapshot. It never reconstructs portable
    // tables from a UI-specific atlas or appearance model, because neither model exists anymore.
    Ok(neocogi_ui::assets::AssetData {
        pixels: atlas.pixels().to_vec().into(),
        tables: atlas.tables().clone(),
    })
}
