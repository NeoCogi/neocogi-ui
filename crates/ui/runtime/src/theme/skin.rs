//
// Copyright 2022-Present (c) Raja Lehtihet & Wael El Oraiby
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
// -----------------------------------------------------------------------------
// Ported to rust from https://github.com/rxi/microui/ and the original license
//
// Copyright (c) 2020 rxi
//
// Permission is hereby granted, free of charge, to any person obtaining a copy
// of this software and associated documentation files (the "Software"), to
// deal in the Software without restriction, including without limitation the
// rights to use, copy, modify, merge, publish, distribute, sublicense, and/or
// sell copies of the Software, and to permit persons to whom the Software is
// furnished to do so, subject to the following conditions:
//
// The above copyright notice and this permission notice shall be included in
// all copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
// IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING
// FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS
// IN THE SOFTWARE.
//
//! Resolved, structured, atlas-bound skin values used across the crate.

use super::{
    ChromeRole, ChromeState, ControlRole, ControlState, FontRef, FontRole, IconRole, MenuRole, MenuState, SurfaceRole, SurfaceState, Visual,
    catalog::AppearanceCatalog,
};
use crate::atlas::{AtlasHandle, FontId};
use crate::render::NinePatch;
#[cfg(test)]
use crate::SliceInsets;
#[cfg(test)]
use super::FlatPalette;

/// Opaque identity of one immutable skin value installed through a [`crate::SkinBundle`].
///
/// Retained measurement caches compare this concrete token rather than mirroring selected Skin
/// fields or holding atlas pointers. The process-wide allocator never reuses a value, so an old
/// cache entry cannot match a later skin even when both happen to contain equal metrics.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub(crate) struct SkinRevision(
    /// Non-reused process identity kept private to the skinning subsystem.
    crate::identity::ProcessUniqueId,
);

impl SkinRevision {
    /// Allocates a revision for one newly constructed or newly bundled skin value.
    fn allocate() -> Self {
        // The identity source is shared with other capability domains but the wrapper prevents a
        // renderer, node, or surface identity from entering measurement-cache comparisons.
        Self(crate::identity::ProcessUniqueId::allocate())
    }
}

/// Plain geometry belongs to the resource contract; this module owns only resolved UI state.
pub use neocogi_ui_contracts::{CaptionButtonSide, CaptionButtonsSkin, SkinMetrics, TitleBackdropSkin, WindowChromeSkin, WindowTitleAlignment};

/// Complete resolved runtime skin installed with one matching atlas.
///
/// A `Skin` contains only values consumed at runtime. Flat palettes and serialized authoring data
/// are compiled into these concrete fields and are not retained as alternate sources of truth.
#[derive(Clone)]
pub struct Skin {
    /// Exact generation used by retained caches; refreshed whenever a bundle takes ownership.
    revision: SkinRevision,
    /// Layout and widget geometry values.
    pub metrics: SkinMetrics,
    /// Complete patch and content-color visuals stored in their semantic state families.
    ///
    /// Storage stays private so every public read and replacement crosses the API as one `Visual`.
    /// This prevents callers from temporarily or permanently pairing a new patch with the content
    /// color belonging to an unrelated state.
    appearances: AppearanceCatalog,
    /// Concrete font identities selected once for every semantic typography role.
    ///
    /// Optional roles contain the required body ID when their conventional named resource is not
    /// present, making runtime role lookup direct and total.
    font_roles: [FontId; FontRole::COUNT],
    /// Concrete icon identities selected once for every required semantic icon role.
    icon_roles: [crate::IconId; IconRole::COUNT],
    /// Window-title and caption-control arrangement.
    pub window_chrome: WindowChromeSkin,
}

impl Skin {
    /// Binds validated semantic resources to already compiled geometry and appearance arrays.
    /// The caller proves required names before this one-time local ID resolution.
    pub(crate) fn from_resolved(atlas: &AtlasHandle, metrics: SkinMetrics, window_chrome: WindowChromeSkin, appearances: AppearanceCatalog) -> Self {
        // Resolve names once at construction. Every retained value below is a stable resource ID,
        // so layout and paint perform only typed table lookup against the selected atlas.
        let body = atlas.font_id(FontRole::Body.atlas_name()).expect("atlas does not contain required font `body`");
        let font_roles = FontRole::ALL.map(|role| atlas.font_id(role.atlas_name()).unwrap_or(body));
        let icon_roles = IconRole::ALL.map(|role| {
            let name = role.atlas_name();
            atlas
                .icon_id(name)
                .unwrap_or_else(|| panic!("atlas does not contain required skin icon `{name}`"))
        });
        Self {
            revision: SkinRevision::allocate(),
            metrics,
            appearances,
            font_roles,
            icon_roles,
            window_chrome,
        }
    }

    /// Builds the historical flat fixture only for native runtime regression tests.
    #[cfg(test)]
    pub(crate) fn from_atlas(atlas: &AtlasHandle) -> Self {
        // Test defaults provide an independent oracle for the asset compiler's portable output.
        Self::from_resolved(
            atlas,
            SkinMetrics::default(),
            WindowChromeSkin::default(),
            AppearanceCatalog::from_flat_palette(SliceInsets::uniform(1), &FlatPalette::default()),
        )
    }

    /// Returns the opaque generation associated with this complete skin value.
    pub(crate) const fn revision(&self) -> SkinRevision {
        // Copies are safe because a revision is an immutable identity, not a mutable counter.
        self.revision
    }

    /// Assigns a fresh generation when a bundle takes ownership of this skin.
    pub(crate) fn refresh_revision(&mut self) {
        // Public Skin fields remain ordinary concrete values. The bundle boundary is the one place
        // that turns their completed combination into a new cacheable immutable generation.
        self.revision = SkinRevision::allocate();
    }

    /// Replaces flat visual fallbacks from one authored palette.
    ///
    /// This operation is intended for builders and live skin editors. It compiles the palette
    /// immediately into the resolved catalog rather than retaining the palette as shadow state.
    #[cfg(test)]
    pub(crate) fn apply_flat_palette(&mut self, palette: FlatPalette) {
        // Preserve the currently selected generic frame geometry while replacing all flat paint
        // values. Callers that need different frame geometry can update that typed visual after.
        let frame_insets = self.frame_insets();
        self.appearances = AppearanceCatalog::from_flat_palette(frame_insets, &palette);
        self.window_chrome.set_backdrop_color(palette.title_background);
    }

    /// Applies a concrete metrics edit and returns the resulting skin.
    ///
    /// The closure keeps grouped metrics construction concise in programmatic skins and tests
    /// without restoring a flat duplicate field surface on `Skin`.
    #[cfg(test)]
    pub(crate) fn with_metrics(mut self, configure: impl FnOnce(&mut SkinMetrics)) -> Self {
        // Metrics are plain concrete values, so configuration is immediate and cannot be retained
        // as an erased callback or deferred mutation.
        configure(&mut self.metrics);
        self
    }

    /// Reports whether `atlas` contains every semantic and image resource used by this skin.
    pub(crate) fn belongs_to(&self, atlas: &AtlasHandle) -> bool {
        // Semantic resource tables and image-backed appearances all carry stable IDs. Checking the
        // complete concrete skin here prevents a bundle from publishing unresolved resources.
        self.font_roles.iter().all(|font| atlas.contains_font(*font))
            && self.icon_roles.iter().all(|icon| atlas.contains_icon(*icon))
            && self
                .appearances
                .iter()
                .map(|visual| visual.patch)
                .filter_map(NinePatch::image_content)
                .all(|image| atlas.contains_icon(image.icon))
    }

    /// Returns normalized structural frame insets shared by measurement and placement.
    #[cfg(test)]
    pub(crate) fn frame_insets(&self) -> SliceInsets {
        // NinePatch owns normalization so layout and renderer geometry cannot disagree on negative
        // application-provided style components.
        self.surface(SurfaceRole::GenericFrame, SurfaceState::Normal).patch.insets.normalized()
    }

    /// Returns one structural surface visual.
    pub fn surface(&self, role: SurfaceRole, state: SurfaceState) -> Visual {
        // Family-specific parameters make pointer, menu-open, and activation states unrepresentable.
        self.appearances.surface(role, state)
    }

    /// Replaces one complete structural surface visual.
    pub fn set_surface(&mut self, role: SurfaceRole, state: SurfaceState, visual: Visual) {
        // Patch and content color cross the mutation boundary together as one complete value.
        self.appearances.set_surface(role, state, visual);
    }

    /// Returns one interactive control visual.
    pub fn control(&self, role: ControlRole, state: ControlState) -> Visual {
        // Disabled controls cannot be queried with a contradictory hover or focus state.
        self.appearances.control(role, state)
    }

    /// Replaces one complete interactive control visual.
    pub fn set_control(&mut self, role: ControlRole, state: ControlState, visual: Visual) {
        // Patch and content color cross the mutation boundary together as one complete value.
        self.appearances.set_control(role, state, visual);
    }

    /// Returns one menu visual.
    pub fn menu(&self, role: MenuRole, state: MenuState) -> Visual {
        // The menu family alone exposes the persistent Open selection state.
        self.appearances.menu(role, state)
    }

    /// Replaces one complete menu visual.
    pub fn set_menu(&mut self, role: MenuRole, state: MenuState, visual: Visual) {
        // Patch and content color cross the mutation boundary together as one complete value.
        self.appearances.set_menu(role, state, visual);
    }

    /// Returns one manager-owned window chrome visual.
    pub fn chrome(&self, role: ChromeRole, state: ChromeState) -> Visual {
        // Pointer and keyboard focus states cannot enter activation-driven chrome lookup.
        self.appearances.chrome(role, state)
    }

    /// Replaces one complete manager-owned window chrome visual.
    pub fn set_chrome(&mut self, role: ChromeRole, state: ChromeState, visual: Visual) {
        // Patch and content color cross the mutation boundary together as one complete value.
        self.appearances.set_chrome(role, state, visual);
    }

    /// Returns the active atlas capability for one semantic font role.
    ///
    /// Optional semantic names were mapped to the required body identity during construction, so
    /// compact atlases remain useful without runtime name lookup.
    pub fn resolve_font_role(&self, atlas: &AtlasHandle, role: FontRole) -> FontId {
        let font = self.font_roles[role.index()];
        // Bundle construction proves this invariant for installed skins. Retain the assertion for
        // direct callers pairing a cloned Skin with an unrelated atlas.
        assert!(atlas.contains_font(font), "semantic font ID does not belong to the supplied atlas: {font:?}");
        font
    }

    /// Returns the active atlas identity for one semantic icon role.
    pub fn resolve_icon_role(&self, atlas: &AtlasHandle, role: IconRole) -> crate::IconId {
        let icon = self.icon_roles[role.index()];
        // Required icon names were resolved exactly once in from_atlas; runtime only validates the
        // concrete skin/atlas pairing before returning that typed value.
        assert!(atlas.contains_icon(icon), "semantic icon ID does not belong to the supplied atlas: {icon:?}");
        icon
    }

    /// Returns the active atlas capability for one stable retained font reference.
    pub fn resolve_font(&self, atlas: &AtlasHandle, font: &FontRef) -> FontId {
        // Delegate to the typed reference so named and semantic references have one resolution
        // implementation shared by application and built-in widgets.
        font.resolve(self, atlas)
    }
}

#[cfg(test)]
mod tests {
    use crate::Color;
    use super::*;
    use crate::ControlRole;
    use crate::test_support::test_atlas_with_font_sizes as make_test_atlas;

    /// Verifies semantic and named font references resolve through the active atlas.
    #[test]
    fn font_references_resolve_semantic_and_named_fonts() {
        let atlas = make_test_atlas(&[(FontRole::Body.atlas_name(), 12), (FontRole::Heading.atlas_name(), 18)]);
        let heading = atlas.font_id(FontRole::Heading.atlas_name()).unwrap();
        let style = Skin::from_atlas(&atlas);

        assert_eq!(FontRef::from(FontRole::Heading), FontRef::role(FontRole::Heading));
        assert_eq!(FontRef::role(FontRole::Heading).resolve(&style, &atlas), heading);
        assert_eq!(FontRef::named(heading).resolve(&style, &atlas), heading);
    }

    /// Verifies one-pass skin construction resolves named roles and uses body for missing roles.
    #[test]
    fn from_atlas_resolves_named_fonts_and_falls_back_to_body() {
        let atlas = make_test_atlas(&[
            (FontRole::Small.atlas_name(), 10),
            (FontRole::Body.atlas_name(), 12),
            (FontRole::Heading.atlas_name(), 18),
            (FontRole::Title.atlas_name(), 16),
        ]);

        let style = Skin::from_atlas(&atlas);

        let body = atlas.font_id(FontRole::Body.atlas_name()).unwrap();
        assert_eq!(style.resolve_font_role(&atlas, FontRole::Body), body);
        assert_eq!(
            style.resolve_font_role(&atlas, FontRole::Small),
            atlas.font_id(FontRole::Small.atlas_name()).unwrap()
        );
        assert_eq!(
            style.resolve_font_role(&atlas, FontRole::Title),
            atlas.font_id(FontRole::Title.atlas_name()).unwrap()
        );
        assert_eq!(
            style.resolve_font_role(&atlas, FontRole::Heading),
            atlas.font_id(FontRole::Heading.atlas_name()).unwrap()
        );
        assert_eq!(style.resolve_font_role(&atlas, FontRole::Mono), body);
        assert!(style.belongs_to(&atlas));
    }

    /// Verifies every image-backed visual capability participates in the ownership predicate.
    #[test]
    fn belongs_to_rejects_a_foreign_appearance_capability() {
        let local_atlas = make_test_atlas(&[(FontRole::Body.atlas_name(), 12)]);
        let foreign_atlas = make_test_atlas(&[(FontRole::Body.atlas_name(), 12)]);
        let local = Skin::from_atlas(&local_atlas);

        // Semantic IDs already belong to the local atlas. Replacing one compiled image with a
        // foreign resource must make the otherwise complete skin fail the same predicate.
        let mut candidate = local;
        crate::test_support::replace_control_patches(&mut candidate, ControlRole::Button, |_| {
            NinePatch::image(
                SliceInsets::ZERO,
                crate::NinePatchImage::new(
                    Skin::from_atlas(&foreign_atlas).resolve_icon_role(&foreign_atlas, IconRole::Close),
                    SliceInsets::ZERO,
                    Color { r: 255, g: 255, b: 255, a: 255 },
                ),
            )
        });

        assert!(!candidate.belongs_to(&local_atlas));
    }

    /// Verifies standard skin construction never substitutes a positional font for missing body.
    #[test]
    #[should_panic(expected = "atlas does not contain required font `body`")]
    fn from_atlas_requires_the_exact_body_font_name() {
        let atlas = make_test_atlas(&[("caption", 12)]);

        // Even though slot zero is a valid font, its unrelated name cannot satisfy the body role.
        let _ = Skin::from_atlas(&atlas);
    }

    /// Verifies frame normalization remains independent of atlas-bound asset construction.
    #[test]
    fn frame_insets_normalize_each_component_without_changing_cells() {
        let atlas = make_test_atlas(&[(FontRole::Body.atlas_name(), 12)]);
        let mut style = Skin::from_atlas(&atlas);
        crate::test_support::replace_surface_patches(&mut style, SurfaceRole::GenericFrame, |_| {
            NinePatch::framed(SliceInsets::new(-4, 2, -3, 5), Color { r: 10, g: 20, b: 30, a: 255 }, None)
        });
        let insets = style.frame_insets();

        assert_eq!((insets.left, insets.top, insets.right, insets.bottom), (0, 2, 0, 5));
    }
}
