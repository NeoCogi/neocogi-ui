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

//! Stable semantic and exact font references for retained UI state.

use crate::atlas::{AtlasHandle, FontId};

use super::Skin;

/// Shared semantic role; concrete atlas capabilities remain local to the UI.
pub use microui_contracts::FontRole;

/// Stable reference to a font used by retained UI state.
///
/// Semantic roles select concrete IDs already resolved by the active skin. Named application fonts
/// carry their exact stable [`FontId`] from [`crate::ResourceCatalog`], eliminating retained names
/// and repeated atlas scans from layout.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum FontRef {
    /// Resolves through one standard semantic font role.
    Role(FontRole),
    /// Resolves one exact application-owned font by its stable baked identity.
    Named(FontId),
}

impl Default for FontRef {
    /// Selects ordinary body typography for a newly constructed widget.
    fn default() -> Self {
        // Body is the only required semantic font and is therefore the unsurprising default.
        Self::Role(FontRole::Body)
    }
}

impl From<FontRole> for FontRef {
    /// Converts a semantic role without resolving it against the current atlas.
    fn from(role: FontRole) -> Self {
        // Preserve the role so a later skin replacement can select its resolved concrete font.
        Self::Role(role)
    }
}

impl FontRef {
    /// Creates a semantic font selection.
    pub fn role(role: FontRole) -> Self {
        // Keep this named constructor beside `named` so call sites never construct variants only
        // to communicate whether a resource is semantic or application-owned.
        Self::Role(role)
    }

    /// Creates a stable reference to one exact named font identity.
    pub const fn named(font: FontId) -> Self {
        // The application resource catalog has already validated name membership before returning
        // this ID, so retained widget construction needs no string or fallible lookup.
        Self::Named(font)
    }

    /// Resolves this stable reference into a font identity contained by `atlas`.
    ///
    /// # Panics
    ///
    /// Panics when the exact or semantic font identity is absent from `atlas`.
    pub fn resolve(&self, skin: &Skin, atlas: &AtlasHandle) -> FontId {
        // Skin owns the compiled semantic table; exact references bypass roles but still validate
        // against the atlas paired with the active skin.
        match self {
            Self::Role(role) => skin.resolve_font_role(atlas, *role),
            Self::Named(font) => {
                assert!(atlas.contains_font(*font), "font ID does not belong to the active skin atlas: {font:?}");
                *font
            }
        }
    }
}
