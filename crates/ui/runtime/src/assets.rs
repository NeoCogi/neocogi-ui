//! UI-local import of complete portable resource revisions.
//!
//! Public values here are implementation-library helpers, never application ABI payloads.
//! Each importing binary creates local resource IDs in an authenticated catalog namespace.
use crate::{AtlasHandle, FontRole, IconRole, ResourceCatalog, Skin, SkinBundle};
use crate::atlas::transfer::Tables;
use neocogi_ui_contracts::*;
use neocogi_ui::assets::AssetTables;
use neocogi_com::{ComPtr, ComRef, IUnknown, NcStatus, NC_E_INVALID_ARG};

mod theme;

/// One imported immutable revision and the original application catalog used by derived themes.
pub struct ImportedAssets {
    /// Retained source allows renderers and other consumers to import this exact revision.
    source: ComPtr<IAssetBundleAbi>,
    /// Canonical namespace identity is checked before preserving any local resource ID.
    identity: ComPtr<IUnknown>,
    /// Original local resources; private theme artwork never enters the application catalog.
    original: AtlasHandle,
    /// Complete validated local skin and matching font/icon lookup metadata.
    bundle: SkinBundle,
}

impl ImportedAssets {
    /// Imports one complete revision. A replacement must retain the original catalog identity
    /// and every original resource name; same-named unrelated catalogs are rejected.
    pub fn read(source: ComRef<'_, IAssetBundleAbi>, previous: Option<&Self>) -> Result<Self, NcStatus> {
        let catalog = source.catalog()?;
        let identity = catalog.query::<IUnknown>()?;
        if previous.is_some_and(|previous| previous.identity.as_ptr() != identity.as_ptr()) {
            return Err(UI_E_OWNER);
        }
        // Shared table validation belongs to the portable data representation. Production UI
        // import reads metadata and only the required white-tile rows; the platform is the sole
        // owner of the complete RGBA transfer and GPU upload. Tests retain pixels because their
        // native atlas fixtures deliberately exercise local round-trip export.
        let mut data = AssetTables::read(source)?;
        let size = data.size;
        #[cfg(test)]
        let pixels = {
            let info = data.info()?;
            let mut pixels = filled(info.rgba_bytes()?, 0)?;
            source.pixels(neocogi_com::NcCopyToBufferOffset::new(0, &mut pixels))?;
            pixels
        };
        let tables = Tables {
            fonts: std::mem::take(&mut data.fonts).into_owned(),
            glyphs: std::mem::take(&mut data.glyphs).into_owned(),
            icons: std::mem::take(&mut data.icons).into_owned(),
            names: std::mem::take(&mut data.names).into_owned(),
        };
        #[cfg(test)]
        let atlas = tables.import(size, pixels, previous.map(|previous| &previous.original))?;
        #[cfg(not(test))]
        let atlas = tables.import(size, previous.map(|previous| &previous.original))?;
        // Skin's native constructor assumes the semantic resources exist. Prove that before
        // calling it so malformed plugins produce a status, never a panic across the ABI.
        semantic_resources(&atlas)?;
        let skin = theme::read(&data, &atlas)?;
        let bundle = SkinBundle::new(atlas.clone(), skin);
        let original = previous.map_or(atlas, |previous| previous.original.clone());
        ResourceCatalog::new(original.clone()).validate_bundle(&bundle).map_err(|_| NC_E_INVALID_ARG)?;
        Ok(Self {
            source: source.add_ref(),
            identity,
            original,
            bundle,
        })
    }

    /// Borrows the validated local resources used for every measurement and draw lookup.
    pub fn bundle(&self) -> &SkinBundle {
        &self.bundle
    }

    /// Borrows the retained immutable source for another binary's bulk import.
    pub fn source(&self) -> ComRef<'_, IAssetBundleAbi> {
        self.source.as_com_ref()
    }

    /// Borrows the original catalog atlas; named application references must resolve here.
    pub fn original(&self) -> &AtlasHandle {
        &self.original
    }

    /// Exports a locally derived bundle with the same authenticated catalog namespace.
    #[cfg(test)]
    pub(crate) fn export_derived(&self, bundle: &SkinBundle) -> Result<ComPtr<IAssetBundleAbi>, NcStatus> {
        ResourceCatalog::new(self.original.clone())
            .validate_bundle(bundle)
            .map_err(|_| NC_E_INVALID_ARG)?;
        export_bundle_data(bundle)?.into_revision(self.source())
    }
}

/// Exports a validated native bundle into a fresh logical catalog, with no parser dependency.
#[cfg(test)]
pub(crate) fn export_bundle(bundle: &SkinBundle) -> Result<ComPtr<IAssetBundleAbi>, NcStatus> {
    export_bundle_data(bundle)?.into_bundle()
}

/// Produces the same concrete tables consumed by generated prebuilt source and runtime providers.
#[cfg(test)]
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
#[cfg(test)]
fn filled<T: Clone>(count: usize, value: T) -> Result<Vec<T>, NcStatus> {
    let mut values = Vec::new();
    values.try_reserve_exact(count).map_err(|_| NC_E_OUT_OF_MEMORY)?;
    values.resize(count, value);
    Ok(values)
}

/// Proves the required default skin lookups are total before invoking native constructors.
fn semantic_resources(atlas: &AtlasHandle) -> Result<(), NcStatus> {
    if atlas.font_id(FontRole::BODY.atlas_name()).is_none() || IconRole::ALL.iter().any(|role| atlas.icon_id(role.atlas_name()).is_none()) {
        return Err(NC_E_INVALID_ARG);
    }
    Ok(())
}

#[cfg(test)]
mod tests;

#[cfg(test)]
use neocogi_com::NC_E_OUT_OF_MEMORY;
