//! UI-local import of complete portable resource revisions.
//!
//! Public values here are implementation-library helpers, never application ABI payloads.
//! Each importing binary creates local resource IDs in an authenticated catalog namespace.
use crate::{AtlasHandle, ResourceCatalog, Skin, SkinBundle};
use neocogi_ui_core_contracts::*;
use neocogi_ui::assets::AssetTables;
use neocogi_com::{ComPtr, ComRef, IUnknown, NcStatus, NC_E_INVALID_ARG};

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
        let image = source.image()?;
        if previous.is_some_and(|previous| previous.identity.as_ptr() != identity.as_ptr()) {
            return Err(UI_E_OWNER);
        }
        // UI imports lookup metadata and validates only the required white-tile rows. Full
        // RGBA transfers belong to GFX and the renderer, including in test builds.
        let tables = AssetTables::read(source)?;
        // The canonical validator has already established every resource, geometry, and appearance
        // invariant. UI import retains that exact value and derives only local identities and
        // sorted lookup slots; it does not translate records into a parallel atlas or skin model.
        let atlas = AtlasHandle::from_validated_tables(tables, previous.map(|previous| &previous.original))?;
        let skin = Skin::from_atlas(atlas.clone());
        let bundle = SkinBundle::from_skin(skin);
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

/// Asset writers belong to test setup; deployed import never reads their pixel buffers.
#[cfg(any(test, feature = "test-support"))]
#[path = "test_support/assets.rs"]
pub mod fixtures;
