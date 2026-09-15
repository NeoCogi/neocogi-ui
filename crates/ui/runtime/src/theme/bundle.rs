//
// Copyright 2023-Present (c) Raja Lehtihet & Wael El Oraiby
//
// Redistribution and use in source and binary forms, with or without
// modification, are permitted provided that the conditions in LICENSE are met.
//

//! Atomic ownership of one canonical asset snapshot and its typed UI views.

use crate::AtlasHandle;

use super::Skin;

/// Failure to install a bundle without invalidating the original application catalog.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum SkinInstallError {
    /// The candidate omits a logical font from the original application catalog.
    MissingFont {
        /// Catalog spelling retained only for actionable diagnostics.
        name: String,
        /// Stable font identity that existing retained widgets may reference.
        id: crate::FontId,
    },
    /// The candidate omits a logical icon from the original application catalog.
    MissingIcon {
        /// Catalog spelling retained only for actionable diagnostics.
        name: String,
        /// Stable icon identity that existing retained widgets may reference.
        id: crate::IconId,
    },
}

/// One immutable canonical asset snapshot with its intrinsically bound UI skin view.
///
/// The atlas is reached through [`Skin`] instead of stored as a second independently pairable
/// field. Consequently an image-backed appearance and its icon identities cannot be combined with
/// an unrelated table allocation, and bundle construction needs no late ownership scan.
#[derive(Clone)]
pub struct SkinBundle {
    /// Typed selectors and cache revision bound to the snapshot-owning atlas handle.
    skin: Skin,
}

impl SkinBundle {
    /// Publishes an already complete skin as one cache revision.
    pub(crate) fn from_skin(mut skin: Skin) -> Self {
        skin.refresh_revision();
        Self { skin }
    }

    /// Builds a canonical skin view for an atlas and publishes it atomically.
    #[cfg(test)]
    pub(crate) fn from_atlas(atlas: AtlasHandle) -> Self {
        Self::from_skin(Skin::from_atlas(atlas))
    }

    /// Borrows the typed skin view over the canonical snapshot.
    pub fn skin(&self) -> &Skin {
        &self.skin
    }

    /// Borrows the immutable atlas and lookup indices intrinsic to this snapshot.
    pub fn atlas(&self) -> &AtlasHandle {
        self.skin.atlas()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Verifies clones identify one value while republishing starts a new revision.
    #[test]
    fn bundle_publication_assigns_one_complete_non_reused_skin_revision() {
        let atlas = crate::test_support::test_atlas();
        let first = SkinBundle::from_atlas(atlas);
        let first_clone = first.clone();
        let second = SkinBundle::from_skin(first.skin().clone());

        assert_eq!(first.skin().revision(), first_clone.skin().revision());
        assert_ne!(first.skin().revision(), second.skin().revision());
    }

    /// Verifies the skin and atlas expose fields from one canonical table allocation.
    #[test]
    fn skin_views_borrow_the_atlas_canonical_snapshot() {
        let bundle = SkinBundle::from_atlas(crate::test_support::test_atlas());

        // Pointer equality is intentional here: a merely equal copied metric record would recreate
        // the dual model that this ownership boundary exists to prevent.
        assert!(std::ptr::eq(bundle.skin().metrics(), &bundle.atlas().tables().metrics));
        assert!(std::ptr::eq(bundle.skin().window_chrome(), &bundle.atlas().tables().chrome,));
    }
}
