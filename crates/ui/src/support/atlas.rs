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

//! UI-local indices over one canonical portable resource snapshot.
//!
//! [`neocogi_ui::assets::AssetTables`] remains the sole retained representation of font metrics,
//! glyph records, icon rectangles, layout metrics, chrome policy, and resolved appearances. The
//! UI adds only lookup order and opaque process-local identities: neither index duplicates the
//! underlying records. Runtime measurement and drawing substitute the selected font's underscore
//! entry for a missing character.

use super::*;
use crate::support::identity::ProcessUniqueId;
use neocogi_ui::assets::AssetTables;
use neocogi_ui::abi::core::{AssetFont, AssetGlyph, FontRole, IconRole};

/// Opaque identity of one logical font resource.
///
/// A builder preserves this identity when it copies or replaces the named font in a derived atlas.
/// Retained UI state can consequently keep the ID across theme changes without retaining a name or
/// depending on a table position. An unrelated atlas does not contain the identity and rejects it
/// before accessing font data.
///
/// IDs cannot be fabricated without an atlas owner. The diagnostic-matched
/// `tests/ui/font_id_default.rs` contract test verifies the absence of a default constructor beside
/// a passing fixture that obtains IDs from an [`AtlasHandle`].
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct FontId {
    /// Non-reusing identity allocated when the logical resource first enters an atlas pipeline.
    resource: ProcessUniqueId,
}

impl FontId {
    /// Allocates an identity for one newly introduced font resource.
    fn allocate() -> Self {
        // Atlas loading and successful builder insertion are the only allocation sites. Copying a
        // resource carries this value forward rather than manufacturing replacement identities.
        Self { resource: ProcessUniqueId::allocate() }
    }
}

/// Opaque identity of one logical bitmap icon resource.
///
/// Atlas derivation preserves this value together with the named icon's pixels. Theme-private
/// images receive fresh identities, so an image from one sibling theme cannot alias an unrelated
/// image that happens to occupy the same table position in another atlas.
///
/// IDs cannot be fabricated without an atlas owner. The diagnostic-matched
/// `tests/ui/icon_id_default.rs` contract test verifies the absence of a default constructor beside
/// a passing fixture that obtains IDs from an [`AtlasHandle`].
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct IconId {
    /// Non-reusing identity allocated when the logical resource first enters an atlas pipeline.
    resource: ProcessUniqueId,
}

impl IconId {
    /// Allocates an identity for one newly introduced icon resource.
    fn allocate() -> Self {
        // Keeping construction private prevents callers from forging identities. Atlas copies use
        // the existing value stored beside the source icon instead of calling this constructor.
        Self { resource: ProcessUniqueId::allocate() }
    }
}

/// UI-owned indices accompanying one canonical validated asset snapshot.
///
/// The vectors contain only process-local identities or indices into [`Self::tables`]. They never
/// copy a portable font, glyph, icon, metric, chrome, or appearance record. Keeping this boundary
/// explicit makes generated NCIDL records the only source of resource semantics in the runtime.
#[derive(Clone)]
struct Atlas {
    /// Complete validated NCIDL-backed resource and appearance snapshot.
    tables: AssetTables,
    /// Stable font identities in canonical font-table order.
    font_ids: Vec<FontId>,
    /// Canonical font slots sorted by their borrowed names for binary lookup.
    font_name_order: Vec<usize>,
    /// Stable icon identities in canonical icon-table order.
    icon_ids: Vec<IconId>,
    /// Canonical icon slots sorted by their borrowed names for binary lookup.
    icon_name_order: Vec<usize>,
    /// Canonical glyph slots, sorted by scalar inside each font's unchanged glyph partition.
    glyph_order: Vec<usize>,
    /// Bounded exact ASCII slots belonging to this same imported immutable revision.
    glyph_ascii: Vec<neocogi_ui::assets::AsciiGlyphIndex>,
    /// Required opaque-white rendering tile resolved once during index construction.
    white_icon: IconId,
    /// Font identities resolved once for the dense semantic role domain.
    font_roles: [FontId; FontRole::COUNT],
    /// Icon identities resolved once for the dense semantic role domain.
    icon_roles: [IconId; IconRole::COUNT],
    /// RGBA bytes retained only by direct algorithm fixtures that must publish a source bundle.
    #[cfg(feature = "test-support")]
    pixels: Vec<u8>,
}

#[derive(Clone)]
/// Shared read-only handle to fully validated UI lookup metadata.
///
/// Handles are created only while the UI imports a validated portable asset revision. Applications
/// obtain typed resource references through the COM facade rather than constructing native atlases.
/// Production handles contain dimensions, glyph metrics and icon rectangles; complete RGBA storage
/// remains at the asset-to-platform upload boundary.
pub struct AtlasHandle(Rc<Atlas>);

mod runtime;
pub(crate) use runtime::TextRun;
