//! UI-local import of complete portable resource revisions.
//!
//! Public values here are implementation-library helpers, never application ABI payloads.
//! Each importing binary creates local resource IDs in an authenticated catalog namespace.
use crate::{AtlasHandle, FontRole, IconRole, ResourceCatalog, Skin, SkinBundle};
use crate::atlas::transfer::Tables;
use neocogi_ui_core_contracts::*;
use neocogi_ui::assets::AssetTables;
use neocogi_com::{ComPtr, ComRef, IUnknown, NcStatus, NC_E_INVALID_ARG};

mod theme;

/// One imported immutable revision and the original application catalog used by derived themes.
pub struct ImportedAssets {
    /// Retained source allows renderers and other consumers to import this exact revision.
    source: ComPtr<IAssetBundleAbi>,
    /// Narrow renderer projection avoids querying or exposing UI metadata during every frame.
    image: ComPtr<IRgbaImageAbi>,
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
        let image = source.add_ref().query::<IRgbaImageAbi>()?;
        if previous.is_some_and(|previous| previous.identity.as_ptr() != identity.as_ptr()) {
            return Err(UI_E_OWNER);
        }
        // UI imports lookup metadata and validates only the required white-tile rows. Full
        // RGBA transfers belong to GFX and the renderer, including in test builds.
        let mut data = AssetTables::read(source)?;
        let size = data.size;
        let tables = Tables {
            fonts: std::mem::take(&mut data.fonts).into_owned(),
            glyphs: std::mem::take(&mut data.glyphs).into_owned(),
            icons: std::mem::take(&mut data.icons).into_owned(),
            names: std::mem::take(&mut data.names).into_owned(),
        };
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
            image,
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

    /// Borrows only the packed RGBA capability consumed by graphics and renderer plugins.
    pub fn image(&self) -> ComRef<'_, IRgbaImageAbi> {
        self.image.as_com_ref()
    }

    /// Borrows the original catalog atlas; named application references must resolve here.
    pub fn original(&self) -> &AtlasHandle {
        &self.original
    }
}

/// Proves the required default skin lookups are total before invoking native constructors.
fn semantic_resources(atlas: &AtlasHandle) -> Result<(), NcStatus> {
    let body = FontRole::Body.canonical_name().expect("known body font role has an IDL name");
    if atlas.font_id(body).is_none()
        || IconRole::ALL.iter().any(|role| {
            atlas
                .icon_id(role.canonical_name().expect("IconRole::ALL contains only declared IDL values"))
                .is_none()
        })
    {
        return Err(NC_E_INVALID_ARG);
    }
    Ok(())
}

#[cfg(test)]
mod tests;

/// Asset writers belong to test setup; deployed import never reads their pixel buffers.
#[cfg(test)]
#[path = "test_support/assets.rs"]
pub(crate) mod fixtures;
