//! UI-local import of complete portable resource revisions.
//!
//! Public values here are implementation-library helpers, never application ABI payloads.
//! Each importing binary creates local resource IDs in an authenticated catalog namespace.
use crate::support::{AtlasHandle, ResourceCatalog, Skin, SkinBundle};
use neocogi_ui::abi::core::*;
use neocogi_ui::assets::AssetTables;
use neocogi_com::{ComPtr, ComRef, IUnknown, NcStatus, NC_E_INVALID_ARG};
use std::rc::Rc;

/// Immutable original application namespace, independently owned during imports.
/// Cloning this basis performs only local reference-count operations, so a cache
/// can release its borrow before any provider callback. It contains no revision
/// source, pixel image or mutable widget state.
#[derive(Clone)]
pub struct ImportedCatalog {
    /// Local ownership of the canonical COM identity; cloning never invokes AddRef.
    identity: Rc<ComPtr<IUnknown>>,
    /// Original local names and IDs, shared without copying tables across revisions.
    original: AtlasHandle,
}

/// One imported immutable revision and the original application catalog used by derived themes.
pub struct ImportedAssets {
    /// Retained source allows renderers and other consumers to import this exact revision.
    source: ComPtr<IAssetBundleAbi>,
    /// Stable original namespace; private theme artwork never enters its named catalog.
    catalog: ImportedCatalog,
    /// Complete validated local skin and matching font/icon lookup metadata.
    bundle: SkinBundle,
}

impl ImportedAssets {
    /// Imports one complete revision. A replacement must retain the original catalog identity
    /// and every original resource name; same-named unrelated catalogs are rejected.
    /// The caller lends an owned catalog basis, not a borrow of its current cache.
    /// Provider getters may reenter siblings while this unpublished import runs.
    pub fn read(source: ComRef<'_, IAssetBundleAbi>, previous: Option<&ImportedCatalog>) -> Result<Self, NcStatus> {
        let catalog = source.catalog()?;
        let identity = catalog.query::<IUnknown>()?;
        if previous.is_some_and(|previous| previous.identity.as_raw().object != identity.as_raw().object) {
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
        let catalog = previous.cloned().unwrap_or_else(|| ImportedCatalog {
            identity: Rc::new(identity),
            original: atlas,
        });
        let original = &catalog.original;
        ResourceCatalog::new(original.clone()).validate_bundle(&bundle).map_err(|_| NC_E_INVALID_ARG)?;
        Ok(Self {
            source: source.try_add_ref()?,
            catalog,
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
        &self.catalog.original
    }

    /// Owns the stable import prerequisites without retaining any cache borrow or
    /// invoking foreign lifetime methods. A failed candidate cannot mutate this basis.
    pub fn catalog(&self) -> ImportedCatalog {
        self.catalog.clone()
    }
}

/// Asset writers belong to test setup; deployed import never reads their pixel buffers.
#[cfg(feature = "test-support")]
#[path = "test_support/assets.rs"]
pub mod fixtures;
