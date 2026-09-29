//
// Copyright 2022-Present (c) Raja Lehtihet & Wael El Oraiby
//
// Redistribution and use in source and binary forms, with or without
// modification, are permitted provided that the conditions in LICENSE are met.
//

//! Typed views over one canonical, atlas-bound asset snapshot.

use super::{ChromeRole, ChromeState, ControlRole, ControlState, FontRef, FontRole, IconRole, MenuRole, MenuState, PointerState, SurfaceRole, SurfaceState, Visual};
use crate::atlas::{AtlasHandle, FontId};
use crate::render::{Color, NinePatch, NinePatchCell, NinePatchCells, NinePatchImage, SliceInsets};
use neocogi_ui_core_contracts::{AssetChrome, AssetMetrics, AssetPatch, AssetVisual, Insets};

/// Opaque identity of one immutable skin snapshot installed through [`crate::SkinBundle`].
///
/// Retained measurement caches compare this token rather than mirroring individual table fields.
/// The process allocator never reuses a value, so an old cache cannot alias a later revision.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct SkinRevision(
    /// Non-reused process identity private to the skin subsystem.
    crate::identity::ProcessUniqueId,
);

impl SkinRevision {
    /// Allocates an identity for one completed immutable skin snapshot.
    fn allocate() -> Self {
        Self(crate::identity::ProcessUniqueId::allocate())
    }
}

/// Immutable runtime skin bound intrinsically to its canonical asset snapshot.
///
/// `Skin` retains no duplicate metric, chrome, appearance, font-role, or icon-role records. The
/// paired [`AtlasHandle`] owns the table collection of generated records and the narrow lookup indices required by hot
/// paths. Appearance getters select one generated record with exhaustive pattern matching and
/// project it transiently into the renderer-facing [`Visual`] value.
#[derive(Clone)]
pub struct Skin {
    /// Exact generation used by retained measurement caches.
    revision: SkinRevision,
    /// Canonical tables together with catalog-scoped identities and sorted lookup indices.
    atlas: AtlasHandle,
}

impl Skin {
    /// Binds a skin to the exact canonical tables already owned by `atlas`.
    pub fn from_atlas(atlas: AtlasHandle) -> Self {
        // Semantic resources were resolved while building the atlas indices, so construction adds
        // only the snapshot identity used by retained measurement caches.
        Self {
            revision: SkinRevision::allocate(),
            atlas,
        }
    }

    /// Returns the opaque generation associated with this complete snapshot.
    pub const fn revision(&self) -> SkinRevision {
        self.revision
    }

    /// Assigns a fresh generation when a bundle publishes this skin value.
    pub(crate) fn refresh_revision(&mut self) {
        self.revision = SkinRevision::allocate();
    }

    /// Borrows the exact atlas and canonical tables intrinsic to this skin.
    pub(crate) fn atlas(&self) -> &AtlasHandle {
        &self.atlas
    }

    /// Borrows canonical layout metrics without copying them into a UI-owned model.
    pub fn metrics(&self) -> &AssetMetrics {
        &self.atlas.tables().metrics
    }

    /// Borrows canonical title and caption policy without translating it to native structs.
    pub fn window_chrome(&self) -> &AssetChrome {
        &self.atlas.tables().chrome
    }

    /// Returns one structural surface visual selected from the generated role record.
    pub fn surface(&self, role: SurfaceRole, state: SurfaceState) -> Visual {
        let values = &self.atlas.tables().surfaces[role.raw() as usize];
        let value = match state {
            SurfaceState::Normal => values.normal,
            SurfaceState::Disabled => values.disabled,
        };
        visual(value, &self.atlas)
    }

    /// Returns one interactive control visual selected from its nested generated state record.
    pub fn control(&self, role: ControlRole, state: ControlState) -> Visual {
        let values = &self.atlas.tables().controls[role.raw() as usize];
        let value = match state {
            ControlState::Disabled => values.disabled,
            ControlState::Enabled(pointer) => pointer_visual(values.enabled, pointer),
            ControlState::Focused(pointer) => pointer_visual(values.focused, pointer),
        };
        visual(value, &self.atlas)
    }

    /// Returns one menu visual selected from the generated named-state record.
    pub fn menu(&self, role: MenuRole, state: MenuState) -> Visual {
        let values = &self.atlas.tables().menus[role.raw() as usize];
        let value = match state {
            MenuState::Normal => values.normal,
            MenuState::Hovered => values.hovered,
            MenuState::Pressed => values.pressed,
            MenuState::Focused => values.focused,
            MenuState::Open => values.open,
            MenuState::Disabled => values.disabled,
        };
        visual(value, &self.atlas)
    }

    /// Returns one manager-owned chrome visual selected from the generated role record.
    pub fn chrome(&self, role: ChromeRole, state: ChromeState) -> Visual {
        let values = &self.atlas.tables().window_chrome[role.raw() as usize];
        let value = match state {
            ChromeState::Base => values.base,
            ChromeState::Active => values.active,
            ChromeState::Disabled => values.disabled,
        };
        visual(value, &self.atlas)
    }

    /// Returns the pre-resolved font identity for one semantic role.
    pub fn resolve_font_role(&self, atlas: &AtlasHandle, role: FontRole) -> FontId {
        let font = self.atlas.font_role(role);
        // The explicit atlas argument remains part of FontRef's ownership check. A skin can no
        // longer be constructed with a different atlas, but named references still need this guard.
        assert!(atlas.contains_font(font), "semantic font ID does not belong to the supplied atlas: {font:?}");
        font
    }

    /// Returns the pre-resolved icon identity for one semantic role.
    pub fn resolve_icon_role(&self, atlas: &AtlasHandle, role: IconRole) -> crate::IconId {
        let icon = self.atlas.icon_role(role);
        assert!(atlas.contains_icon(icon), "semantic icon ID does not belong to the supplied atlas: {icon:?}");
        icon
    }

    /// Resolves one stable font reference against the active atlas.
    pub fn resolve_font(&self, atlas: &AtlasHandle, font: &FontRef) -> FontId {
        font.resolve(self, atlas)
    }

    /// Returns normalized structural frame insets for native geometry tests.
    #[cfg(any(test, feature = "test-support"))]
    pub fn frame_insets(&self) -> SliceInsets {
        self.surface(SurfaceRole::GenericFrame, SurfaceState::Normal).patch.insets.normalized()
    }

    /// Mutably borrows the canonical metric record in an explicitly edited test snapshot.
    #[cfg(any(test, feature = "test-support"))]
    pub fn metrics_mut(&mut self) -> &mut AssetMetrics {
        &mut self.atlas.tables_mut().metrics
    }

    /// Mutably borrows the canonical chrome policy in an explicitly edited test snapshot.
    #[cfg(any(test, feature = "test-support"))]
    pub fn window_chrome_mut(&mut self) -> &mut AssetChrome {
        &mut self.atlas.tables_mut().chrome
    }

    /// Replaces one canonical surface record from a renderer-facing test visual.
    #[cfg(any(test, feature = "test-support"))]
    pub fn set_surface(&mut self, role: SurfaceRole, state: SurfaceState, value: Visual) {
        let value = self.asset_visual(value);
        let values = &mut self.atlas.tables_mut().surfaces.to_mut()[role.raw() as usize];
        *match state {
            SurfaceState::Normal => &mut values.normal,
            SurfaceState::Disabled => &mut values.disabled,
        } = value;
    }

    /// Replaces one canonical control record from a renderer-facing test visual.
    #[cfg(any(test, feature = "test-support"))]
    pub fn set_control(&mut self, role: ControlRole, state: ControlState, value: Visual) {
        let value = self.asset_visual(value);
        let values = &mut self.atlas.tables_mut().controls.to_mut()[role.raw() as usize];
        *match state {
            ControlState::Disabled => &mut values.disabled,
            ControlState::Enabled(pointer) => pointer_visual_mut(&mut values.enabled, pointer),
            ControlState::Focused(pointer) => pointer_visual_mut(&mut values.focused, pointer),
        } = value;
    }

    /// Replaces one canonical chrome appearance from a renderer-facing test visual.
    #[cfg(any(test, feature = "test-support"))]
    pub fn set_chrome(&mut self, role: ChromeRole, state: ChromeState, value: Visual) {
        let value = self.asset_visual(value);
        let values = &mut self.atlas.tables_mut().window_chrome.to_mut()[role.raw() as usize];
        *match state {
            ChromeState::Base => &mut values.base,
            ChromeState::Active => &mut values.active,
            ChromeState::Disabled => &mut values.disabled,
        } = value;
    }

    /// Recompiles canonical flat appearance records for one edited test snapshot.
    #[cfg(any(test, feature = "test-support"))]
    pub fn apply_flat_palette(&mut self, palette: super::FlatPalette) {
        let insets = self.frame_insets();
        let compiled = neocogi_appearance::AppearanceTables::flat(insets, &palette);
        let tables = self.atlas.tables_mut();
        tables.surfaces = Vec::from(compiled.surfaces).into();
        tables.controls = Vec::from(compiled.controls).into();
        tables.menus = Vec::from(compiled.menus).into();
        tables.window_chrome = Vec::from(compiled.window_chrome).into();
        match tables.chrome.has_backdrop {
            0 => {}
            1 => {
                tables.chrome.backdrop_rgba = u32::from_le_bytes([
                    palette.title_background.r,
                    palette.title_background.g,
                    palette.title_background.b,
                    palette.title_background.a,
                ])
            }
            _ => unreachable!("canonical validation accepts only boolean chrome flags"),
        }
    }

    /// Converts one transient runtime visual back into the canonical test-edit record.
    #[cfg(any(test, feature = "test-support"))]
    fn asset_visual(&self, value: Visual) -> AssetVisual {
        use crate::render::NinePatchContent;

        let patch = match value.patch.content {
            NinePatchContent::Flat { cells } => AssetPatch::Flat {
                top_left: packed_cell(cells.top_left),
                top: packed_cell(cells.top),
                top_right: packed_cell(cells.top_right),
                left: packed_cell(cells.left),
                center: packed_cell(cells.center),
                right: packed_cell(cells.right),
                bottom_left: packed_cell(cells.bottom_left),
                bottom: packed_cell(cells.bottom),
                bottom_right: packed_cell(cells.bottom_right),
            },
            NinePatchContent::Image { image } => AssetPatch::Image {
                icon: self.atlas.icon_index(image.icon),
                source: image.source_insets,
                tint: packed_color(image.tint),
            },
        };
        AssetVisual {
            insets: value.patch.insets,
            center_visible: u8::from(value.patch.center_visible),
            content_color: packed_color(value.content_color),
            patch: patch.into(),
        }
    }
}

/// Selects one named pointer branch without flattening the generated state structure.
fn pointer_visual(values: neocogi_ui_core_contracts::AssetPointerVisuals, state: PointerState) -> AssetVisual {
    match state {
        PointerState::Normal => values.normal,
        PointerState::Hovered => values.hovered,
        PointerState::Pressed => values.pressed,
    }
}

/// Selects one mutable named pointer branch for canonical test editing.
#[cfg(any(test, feature = "test-support"))]
fn pointer_visual_mut(values: &mut neocogi_ui_core_contracts::AssetPointerVisuals, state: PointerState) -> &mut AssetVisual {
    match state {
        PointerState::Normal => &mut values.normal,
        PointerState::Hovered => &mut values.hovered,
        PointerState::Pressed => &mut values.pressed,
    }
}

/// Projects one canonical visual into the short-lived renderer-facing drawing vocabulary.
fn visual(value: AssetVisual, atlas: &AtlasHandle) -> Visual {
    let insets = runtime_insets(value.insets);
    let mut patch = match value.patch {
        AssetPatch::Flat {
            top_left,
            top,
            top_right,
            left,
            center,
            right,
            bottom_left,
            bottom,
            bottom_right,
        } => NinePatch::new(
            insets,
            NinePatchCells {
                top_left: NinePatchCell::color(color(top_left)),
                top: NinePatchCell::color(color(top)),
                top_right: NinePatchCell::color(color(top_right)),
                left: NinePatchCell::color(color(left)),
                center: NinePatchCell::color(color(center)),
                right: NinePatchCell::color(color(right)),
                bottom_left: NinePatchCell::color(color(bottom_left)),
                bottom: NinePatchCell::color(color(bottom)),
                bottom_right: NinePatchCell::color(color(bottom_right)),
            },
        ),
        AssetPatch::Image { icon, source, tint } => NinePatch::image(
            insets,
            NinePatchImage::new(
                atlas.icon_at_index(icon).expect("validated appearance icon index"),
                runtime_insets(source),
                color(tint),
            ),
        ),
    };
    patch.center_visible = match value.center_visible {
        0 => false,
        1 => true,
        _ => unreachable!("canonical validation accepts only boolean center flags"),
    };
    Visual::new(patch, color(value.content_color))
}

/// Converts generated inset fields to the renderer-facing alias at point of use.
fn runtime_insets(value: Insets) -> SliceInsets {
    SliceInsets::new(value.left, value.top, value.right, value.bottom)
}

/// Decodes the renderer contract's stable packed RGBA convention.
fn color(value: u32) -> Color {
    let [r, g, b, a] = value.to_le_bytes();
    Color { r, g, b, a }
}

/// Encodes one runtime color for canonical test fixture editing.
#[cfg(any(test, feature = "test-support"))]
fn packed_color(value: Color) -> u32 {
    u32::from_le_bytes([value.r, value.g, value.b, value.a])
}

/// Encodes one runtime flat cell for canonical test fixture editing.
#[cfg(any(test, feature = "test-support"))]
fn packed_cell(value: NinePatchCell) -> u32 {
    match value {
        NinePatchCell::Empty => 0,
        NinePatchCell::Color { color } => packed_color(color),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::test_atlas_with_font_sizes;

    /// Verifies semantic and exact font references resolve through the active snapshot.
    #[test]
    fn font_references_resolve_semantic_and_named_fonts() {
        let atlas = test_atlas_with_font_sizes(&[("body", 12), ("heading", 18)]);
        let heading = atlas.font_id("heading").unwrap();
        let skin = Skin::from_atlas(atlas.clone());

        assert_eq!(FontRef::role(FontRole::Heading).resolve(&skin, &atlas), heading);
        assert_eq!(FontRef::named(heading).resolve(&skin, &atlas), heading);
    }

    /// Verifies missing optional semantic fonts resolve to the required body identity.
    #[test]
    fn semantic_font_indices_fall_back_to_body() {
        let atlas = test_atlas_with_font_sizes(&[("body", 12), ("heading", 18)]);
        let skin = Skin::from_atlas(atlas.clone());
        let body = atlas.font_id("body").unwrap();

        assert_eq!(skin.resolve_font_role(&atlas, FontRole::Heading), atlas.font_id("heading").unwrap());
        assert_eq!(skin.resolve_font_role(&atlas, FontRole::Mono), body);
    }

    /// Verifies normalized frame geometry is derived from the canonical appearance record.
    #[test]
    fn frame_insets_normalize_each_component_without_changing_cells() {
        let atlas = test_atlas_with_font_sizes(&[("body", 12)]);
        let mut skin = Skin::from_atlas(atlas);
        let patch = NinePatch::framed(SliceInsets::new(-4, 2, -3, 5), Color { r: 10, g: 20, b: 30, a: 255 }, None);
        let current = skin.surface(SurfaceRole::GenericFrame, SurfaceState::Normal);
        skin.set_surface(SurfaceRole::GenericFrame, SurfaceState::Normal, Visual::new(patch, current.content_color));

        let insets = skin.frame_insets();
        assert_eq!((insets.left, insets.top, insets.right, insets.bottom), (0, 2, 0, 5));
    }
}
