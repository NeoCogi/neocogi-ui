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

//! Stable semantic and exact icon references for retained UI state.

use crate::support::atlas::{AtlasHandle, IconId};

use super::Skin;

/// Shared semantic role; concrete atlas capabilities remain local to the UI.
pub use neocogi_ui::abi::core::IconRole;

/// Stable reference to an icon used by retained UI state.
///
/// Semantic roles are resolved once into the active skin, while an exact reference carries the
/// atlas-baked [`IconId`] obtained from [`crate::support::ResourceCatalog`]. Both variants are small typed
/// values and require no retained strings or name lookup during measurement and paint.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum IconRef {
    /// Selects one required semantic icon from the active skin.
    Role(IconRole),
    /// Selects one exact application-owned resource by its stable baked identity.
    Named(IconId),
}

impl From<IconRole> for IconRef {
    /// Converts a semantic role without binding it to the current atlas.
    fn from(role: IconRole) -> Self {
        // Preserve semantic intent so a skin can select its already-resolved concrete icon.
        Self::Role(role)
    }
}

impl IconRef {
    /// Creates a semantic icon reference.
    pub fn role(role: IconRole) -> Self {
        // This constructor mirrors `named` and keeps public widget construction explicit.
        Self::Role(role)
    }

    /// Creates a stable reference to one exact named resource identity.
    pub const fn named(icon: IconId) -> Self {
        // ResourceCatalog performs the string lookup once and supplies the validated typed ID.
        Self::Named(icon)
    }

    /// Resolves this stable reference into an icon identity contained by `atlas`.
    ///
    /// # Panics
    ///
    /// Panics when the semantic or exact icon identity is absent from `atlas`.
    pub fn resolve(&self, skin: &Skin, atlas: &AtlasHandle) -> IconId {
        // Role resolution is a direct skin-array lookup. Exact resources keep their baked ID and
        // verify that the selected derived atlas copied that logical resource.
        match self {
            Self::Role(role) => skin.resolve_icon_role(atlas, *role),
            Self::Named(icon) => {
                assert!(atlas.contains_icon(*icon), "icon ID does not belong to the active skin atlas: {icon:?}");
                *icon
            }
        }
    }
}

#[cfg(all(test, feature = "test-support"))]
mod tests {
    use super::*;

    /// Verifies semantic names are compiled once into IDs independent of table position.
    #[test]
    fn icon_roles_resolve_independent_of_slot_order() {
        let atlas = crate::support::test_support::test_atlas();
        let skin = Skin::from_atlas(atlas.clone());
        for role in IconRole::ALL {
            let name = role.canonical_name().expect("IconRole::ALL contains only declared IDL values");
            assert_eq!(skin.resolve_icon_role(&atlas, role), atlas.icon_id(name).unwrap());
        }

        let close = atlas.icon_id("close").unwrap();
        let named = IconRef::named(close);
        assert_eq!(named.resolve(&skin, &atlas), close);
    }
}
