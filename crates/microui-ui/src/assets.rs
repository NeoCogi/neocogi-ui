//! Shared validated bundle import/export used by the UI, renderers and optional asset plugin.
//!
//! Public values here are implementation-library helpers, never application ABI payloads.
//! Each importing binary creates local resource IDs in an authenticated catalog namespace.
use crate::{AtlasHandle, FontRole, IconRole, ResourceCatalog, Skin, SkinBundle};
use crate::atlas::transfer::Tables;
use microui_contracts::*;
use nc_com::{ComPtr, ComRef, IUnknown, NcStatus, NC_E_INVALID_ARG, NC_E_OUT_OF_MEMORY};

mod theme;

/// One imported immutable revision and the original application catalog used by derived themes.
pub struct ImportedAssets {
    /// Retained source allows renderers and other consumers to import this exact revision.
    source: ComPtr<IAssetBundleAbi>,
    /// Canonical namespace identity is checked before preserving any local resource ID.
    identity: ComPtr<IUnknown>,
    /// Original local resources; private theme artwork never enters the application catalog.
    original: AtlasHandle,
    /// Complete validated local skin and matching pixel/font/icon storage.
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
        // Shared table validation belongs to the portable data representation. The UI
        // then creates only its own lookup IDs and resolved painting state.
        let mut data = microui_redux::assets::AssetData::read(source)?;
        let info = data.info()?;
        let tables = Tables {
            fonts: std::mem::take(&mut data.fonts).into_owned(),
            glyphs: std::mem::take(&mut data.glyphs).into_owned(),
            icons: std::mem::take(&mut data.icons).into_owned(),
            names: std::mem::take(&mut data.names).into_owned(),
        };
        let pixels = std::mem::take(&mut data.pixels).into_owned();
        let atlas = tables.import(info, pixels, previous.map(|previous| &previous.original))?;
        // Skin's native constructor assumes the semantic resources exist. Prove that before
        // calling it so malformed plugins produce a status, never a panic across the ABI.
        semantic_resources(&atlas)?;
        let mut skin = Skin::from_atlas(&atlas);
        theme::read(&data, &atlas, &mut skin)?;
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
    pub fn export_derived(&self, bundle: &SkinBundle) -> Result<ComPtr<IAssetBundleAbi>, NcStatus> {
        ResourceCatalog::new(self.original.clone())
            .validate_bundle(bundle)
            .map_err(|_| NC_E_INVALID_ARG)?;
        export_bundle_data(bundle)?.into_revision(self.source())
    }

    /// Compiles an authoring edit against already validated resources, preserving catalog identity.
    pub(crate) fn derive_style(&self, metrics: AssetMetrics, palette: PaletteEditAbi) -> Result<ComPtr<IAssetBundleAbi>, NcStatus> {
        let palette = PaletteEdit::try_from(palette).map_err(|_| NC_E_INVALID_ARG)?;
        let mut skin = self.bundle.skin().clone();
        skin.metrics = theme::read_metrics(metrics)?;
        if let PaletteEdit::Flat(palette) = palette {
            // The existing compiler supplies every role/state rule; applications keep no copy.
            skin.apply_flat_palette(palette);
        }
        self.export_derived(&SkinBundle::new(self.bundle.atlas().clone(), skin))
    }

    /// Compiles a JSON theme locally against the pristine application atlas.
    /// The optional provider then calls export_derived on the result's bundle.
    #[cfg(feature = "theme-json")]
    pub fn load_theme(&self, path: &std::path::Path) -> Result<crate::LoadedTheme, crate::ThemeLoadError> {
        crate::theme::loader::load(path, &self.original)
    }
}

/// Exports a validated native bundle into a fresh logical catalog, with no parser dependency.
pub fn export_bundle(bundle: &SkinBundle) -> Result<ComPtr<IAssetBundleAbi>, NcStatus> {
    export_bundle_data(bundle)?.into_bundle()
}

/// Produces the same concrete tables consumed by generated prebuilt source and runtime providers.
pub fn export_bundle_data(bundle: &SkinBundle) -> Result<microui_redux::assets::AssetData, NcStatus> {
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
    Ok(microui_redux::assets::AssetData {
        size: info.size,
        pixels: pixels.into(),
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
    })
}

/// Allocates bounded owned output without converting allocation failure into an ABI panic.
fn filled<T: Clone>(count: usize, value: T) -> Result<Vec<T>, NcStatus> {
    let mut values = Vec::new();
    values.try_reserve_exact(count).map_err(|_| NC_E_OUT_OF_MEMORY)?;
    values.resize(count, value);
    Ok(values)
}

#[cfg(feature = "save-to-rust")]
mod export;

/// Exports a freshly built atlas with the standard resolved skin, rejecting missing semantics.
pub fn export_atlas(atlas: AtlasHandle) -> Result<ComPtr<IAssetBundleAbi>, NcStatus> {
    semantic_resources(&atlas)?;
    export_bundle(&SkinBundle::from_atlas(atlas))
}

/// Proves the required default skin lookups are total before invoking native constructors.
fn semantic_resources(atlas: &AtlasHandle) -> Result<(), NcStatus> {
    if atlas.font_id(FontRole::Body.atlas_name()).is_none() || IconRole::ALL.iter().any(|role| atlas.icon_id(role.atlas_name()).is_none()) {
        return Err(NC_E_INVALID_ARG);
    }
    Ok(())
}

#[cfg(test)]
mod tests;
