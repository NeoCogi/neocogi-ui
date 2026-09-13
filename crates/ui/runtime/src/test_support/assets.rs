//! Input construction for asset-boundary and algorithm tests; this code never imports a runtime frame.
use crate::{SkinBundle, ResourceCatalog};
use crate::assets::{ImportedAssets, theme};
use crate::atlas::transfer::Tables;
use neocogi_ui_core_contracts::*;
use neocogi_ui::assets::AssetTables;
use neocogi_com::{ComPtr, NcStatus, NcCopyToBufferOffset, NC_E_INVALID_ARG, NC_E_OUT_OF_MEMORY};

/// Copies pixels explicitly for a fixture revision instead of adding transfers to runtime import.
pub(crate) fn export_imported_data(imported: &ImportedAssets, bundle: &SkinBundle) -> Result<neocogi_ui::assets::AssetData, NcStatus> {
    let mut data = export_bundle_data(bundle)?;
    let mut pixels = vec![0; imported.source().info()?.rgba_bytes()?];
    imported.image().pixels(NcCopyToBufferOffset::new(0, &mut pixels))?;
    data.pixels = pixels.into();
    Ok(data)
}

impl ImportedAssets {
    /// Exports a locally derived bundle with the same authenticated catalog namespace.
    pub(crate) fn export_derived(&self, bundle: &SkinBundle) -> Result<ComPtr<IAssetBundleAbi>, NcStatus> {
        ResourceCatalog::new(self.original.clone())
            .validate_bundle(bundle)
            .map_err(|_| NC_E_INVALID_ARG)?;
        export_imported_data(self, bundle)?.into_revision(self.source())
    }
}

/// Exports a validated native bundle into a fresh logical catalog, with no parser dependency.
pub(crate) fn export_bundle(bundle: &SkinBundle) -> Result<ComPtr<IAssetBundleAbi>, NcStatus> {
    export_bundle_data(bundle)?.into_bundle()
}

/// Produces the same concrete tables consumed by generated prebuilt source and runtime providers.
pub(crate) fn export_bundle_data(bundle: &SkinBundle) -> Result<neocogi_ui::assets::AssetData, NcStatus> {
    let atlas = bundle.atlas();
    let tables = Tables::export(atlas)?;
    let info = AssetInfo {
        schema: ASSET_SCHEMA_VERSION,
        size: Size {
            width: atlas.width() as i32,
            height: atlas.height() as i32,
        },
        fonts: tables.fonts.len() as u32,
        icons: tables.icons.len() as u32,
        glyphs: tables.glyphs.len() as u32,
        names: tables.names.len() as u32,
    };
    let mut pixels = filled(info.rgba_bytes()?, 0)?;
    for (destination, color) in pixels.as_chunks_mut::<4>().0.iter_mut().zip(atlas.pixels()) {
        destination.copy_from_slice(&[color.x, color.y, color.z, color.w]);
    }
    let theme = theme::ExportedTheme::new(bundle)?;
    Ok(neocogi_ui::assets::AssetData {
        pixels: pixels.into(),
        tables: AssetTables {
            size: info.size,
            fonts: tables.fonts.into(),
            glyphs: tables.glyphs.into(),
            icons: tables.icons.into(),
            names: tables.names.into(),
            metrics: theme.metrics,
            chrome: theme.chrome,
            surfaces: theme.surfaces.into(),
            controls: theme.controls.into(),
            menus: theme.menus.into(),
            window_chrome: theme.window_chrome.into(),
        },
    })
}

/// Allocates bounded owned output without converting allocation failure into an ABI panic.
fn filled<T: Clone>(count: usize, value: T) -> Result<Vec<T>, NcStatus> {
    let mut values = Vec::new();
    values.try_reserve_exact(count).map_err(|_| NC_E_OUT_OF_MEMORY)?;
    values.resize(count, value);
    Ok(values)
}
