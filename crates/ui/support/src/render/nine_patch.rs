//
// Copyright 2026-Present (c) Raja Lehtihet & Wael El Oraiby
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

//! Concrete three-by-three patch descriptions and destination geometry.
//!
//! A patch is deliberately a small value rather than an erased paint callback. Skin code can
//! therefore describe a background with nine typed cells, the display list can retain that exact
//! description, and the renderer can expand it without consulting a widget or theme registry.

use crate::{Color, IconId};

/// Shared concrete inset record used by asset compilation, layout and painting.
pub use neocogi_ui_core_contracts::Insets as SliceInsets;

/// Concrete flat content drawn in one cell of a [`NinePatch`].
///
/// The enum is intentionally closed and typed. A complete image-backed patch uses the separate
/// [`NinePatchImage`] representation, so neither rendering mode needs a dynamic payload or a
/// erased runtime downcast.
#[derive(Copy, Clone)]
pub enum NinePatchCell {
    /// Leaves the cell transparent and records no renderer work.
    Empty,
    /// Fills the complete cell with one premultiplier-independent RGBA color.
    Color {
        /// Color submitted through the renderer's opaque-white atlas tile.
        color: Color,
    },
}

impl NinePatchCell {
    /// Creates a color cell while treating a fully transparent color as an empty cell.
    pub const fn color(color: Color) -> Self {
        // Canonicalizing transparent colors here makes visibility checks and renderer execution
        // agree without repeatedly inspecting alpha in every patch consumer.
        if color.a == 0 { Self::Empty } else { Self::Color { color } }
    }
}

/// Nine named cells arranged in top-to-bottom, left-to-right order.
///
/// Named fields keep style and theme construction readable and prevent a positional array from
/// silently swapping an edge or corner. Renderer traversal still obtains one fixed array without
/// allocation through a private row-conversion helper.
#[derive(Copy, Clone)]
pub struct NinePatchCells {
    /// Fixed top-left corner.
    pub top_left: NinePatchCell,
    /// Horizontally stretchable top edge.
    pub top: NinePatchCell,
    /// Fixed top-right corner.
    pub top_right: NinePatchCell,
    /// Vertically stretchable left edge.
    pub left: NinePatchCell,
    /// Horizontally and vertically stretchable center.
    pub center: NinePatchCell,
    /// Vertically stretchable right edge.
    pub right: NinePatchCell,
    /// Fixed bottom-left corner.
    pub bottom_left: NinePatchCell,
    /// Horizontally stretchable bottom edge.
    pub bottom: NinePatchCell,
    /// Fixed bottom-right corner.
    pub bottom_right: NinePatchCell,
}

/// One atlas bitmap divided into a three-by-three image grid.
///
/// Source insets are measured inside the referenced atlas icon and may differ from destination
/// [`SliceInsets`]. The opaque [`IconId`] keeps theme artwork tied to its exact immutable atlas,
/// while ordinary application images continue to use the separate external-texture draw path.
#[derive(Copy, Clone)]
pub struct NinePatchImage {
    /// Atlas capability whose rectangle contains all nine source cells.
    pub icon: IconId,
    /// Nonnegative source rows and columns; GFX shrinks oversized opposing slices and
    /// overlays the surviving image cells with pink at 0.5 opacity.
    pub source_insets: SliceInsets,
    /// RGBA modulation applied uniformly to all nine sampled cells.
    pub tint: Color,
}

impl NinePatchImage {
    /// Creates one explicitly sliced atlas-image description.
    pub const fn new(icon: IconId, source_insets: SliceInsets, tint: Color) -> Self {
        // Retain the capability rather than raw UV coordinates. Atlas ownership validation and
        // rectangle lookup can then remain centralized at Skin and renderer boundaries.
        Self { icon, source_insets, tint }
    }
}

/// Closed content vocabulary for a complete three-by-three patch.
#[derive(Copy, Clone)]
pub enum NinePatchContent {
    /// Nine independently visible flat-color cells.
    Flat {
        /// Named cells traversed in geometric order by the renderer.
        cells: NinePatchCells,
    },
    /// One texture source divided by its own source-space slice insets.
    Image {
        /// Complete typed image description.
        image: NinePatchImage,
    },
}

impl NinePatchCells {
    /// Creates a grid whose complete visible area uses one cell value.
    pub const fn all(cell: NinePatchCell) -> Self {
        // Copy the typed value into every named position. This is useful for theme defaults whose
        // edge and center appearance are intentionally identical.
        Self {
            top_left: cell,
            top: cell,
            top_right: cell,
            left: cell,
            center: cell,
            right: cell,
            bottom_left: cell,
            bottom: cell,
            bottom_right: cell,
        }
    }

    /// Creates a transparent grid with only its center cell populated.
    pub const fn center(cell: NinePatchCell) -> Self {
        // A center-only patch is the three-by-three representation of the former filled quad.
        // Keeping it in this vocabulary ensures flat and image themes share one display operation.
        Self {
            top_left: NinePatchCell::Empty,
            top: NinePatchCell::Empty,
            top_right: NinePatchCell::Empty,
            left: NinePatchCell::Empty,
            center: cell,
            right: NinePatchCell::Empty,
            bottom_left: NinePatchCell::Empty,
            bottom: NinePatchCell::Empty,
            bottom_right: NinePatchCell::Empty,
        }
    }

    /// Creates equal border cells around an independently configured center.
    pub const fn framed(border: NinePatchCell, center: NinePatchCell) -> Self {
        // Corners and edges deliberately share one value for the current flat skin. The named
        // fields remain independently mutable for later classic raised/sunken theme definitions.
        Self {
            top_left: border,
            top: border,
            top_right: border,
            left: border,
            center,
            right: border,
            bottom_left: border,
            bottom: border,
            bottom_right: border,
        }
    }
}

/// Backend-neutral visual patch divided into three rows and three columns.
///
/// The outer columns and rows retain the requested inset thickness while the center consumes the
/// remaining destination. GFX owns proportional reduction when a destination is too small, so the
/// UI value contains no derived rectangles or renderer-facing coordinates.
#[derive(Copy, Clone)]
pub struct NinePatch {
    /// Destination-space sizes of the outer columns and rows.
    pub insets: SliceInsets,
    /// Concrete flat or image content associated with the complete grid.
    pub content: NinePatchContent,
    /// Whether expansion includes the stretchable center cell.
    pub center_visible: bool,
}

impl NinePatch {
    /// Creates a flat patch from explicit destination insets and named cells.
    pub const fn new(insets: SliceInsets, cells: NinePatchCells) -> Self {
        // Preserve exact author input. The GFX boundary normalizes it once before the compositor
        // applies its sole partitioning policy, avoiding a second UI-side geometry representation.
        Self {
            insets,
            content: NinePatchContent::Flat { cells },
            center_visible: true,
        }
    }

    /// Creates an image patch with independent destination and source slice geometry.
    pub const fn image(insets: SliceInsets, image: NinePatchImage) -> Self {
        // Store the complete source description in one explicit enum variant. Image patches never
        // masquerade as nine unrelated external images and therefore remain easy to validate.
        Self {
            insets,
            content: NinePatchContent::Image { image },
            center_visible: true,
        }
    }

    /// Creates the three-by-three equivalent of one solid filled rectangle.
    pub const fn solid(color: Color) -> Self {
        // Zero outer insets collapse the eight surrounding cells and assign the full destination
        // to the center color cell.
        Self::new(SliceInsets::ZERO, NinePatchCells::center(NinePatchCell::color(color)))
    }

    /// Creates a uniformly colored frame around an optional center fill.
    pub const fn framed(insets: SliceInsets, border: Color, fill: Option<Color>) -> Self {
        // Absence and transparent color both become an empty center. This preserves structural
        // insets without requiring a visible fill operation.
        let center = match fill {
            Some(color) => NinePatchCell::color(color),
            None => NinePatchCell::Empty,
        };
        Self::new(insets, NinePatchCells::framed(NinePatchCell::color(border), center))
    }

    /// Replaces only the center cell while retaining every edge and corner.
    pub const fn with_center(mut self, fill: Option<Color>) -> Self {
        // Only flat content has an independently replaceable center. A theme image already owns
        // its complete center artwork, so deriving a role-specific flat fill leaves it unchanged.
        if let NinePatchContent::Flat { cells } = &mut self.content {
            cells.center = match fill {
                Some(color) => NinePatchCell::color(color),
                None => NinePatchCell::Empty,
            };
        }
        self
    }

    /// Replaces destination insets while retaining the complete flat or image payload.
    pub const fn with_insets(mut self, insets: SliceInsets) -> Self {
        // Theme documents use this when a role supplies structural insets but omits one or more PNG
        // states. Those states keep their flat content while matching the role's requested layout.
        self.insets = insets;
        self
    }

    /// Omits the center cell while retaining all eight border cells.
    pub const fn without_center(mut self) -> Self {
        // Window frames are initially recorded below their client content, then repeat only their
        // edges above descendants. A typed visibility bit applies identically to flat and image
        // patches and avoids manufacturing a second border-only image representation.
        self.center_visible = false;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::color;

    /// Verifies transparent color construction canonicalizes to the explicit empty variant.
    #[test]
    fn transparent_cells_are_canonicalized_before_recording() {
        let transparent = NinePatch::solid(color(1, 2, 3, 0));
        let opaque = NinePatch::solid(color(1, 2, 3, 1));

        assert!(matches!(transparent.content, NinePatchContent::Flat { cells } if matches!(cells.center, NinePatchCell::Empty)));
        assert!(matches!(opaque.content, NinePatchContent::Flat { cells } if matches!(cells.center, NinePatchCell::Color { .. })));
    }

    /// Verifies border-only values retain their typed edge cells and explicit center policy.
    #[test]
    fn border_only_patch_retains_edges_and_hides_center() {
        let center_only = NinePatch::solid(color(1, 2, 3, 255)).without_center();
        let framed = NinePatch::framed(SliceInsets::uniform(1), color(4, 5, 6, 255), Some(color(7, 8, 9, 255))).without_center();

        assert!(!center_only.center_visible);
        assert!(!framed.center_visible);
        assert!(matches!(framed.content, NinePatchContent::Flat { cells } if matches!(cells.top, NinePatchCell::Color { .. })));
    }

    /// Verifies replacing the flat center never destroys a complete image-backed patch.
    #[test]
    fn image_patch_ignores_flat_center_replacement() {
        let atlas = crate::test_support::test_atlas();
        let icon = atlas.icon_id("close").expect("test atlas must contain the close icon");
        let image = NinePatchImage::new(icon, SliceInsets::uniform(2), color(255, 255, 255, 255));
        let patch = NinePatch::image(SliceInsets::uniform(3), image).with_center(Some(color(1, 2, 3, 255)));

        let NinePatchContent::Image { image: resolved } = patch.content else {
            panic!("image content was replaced by flat cells");
        };
        assert_eq!(resolved.icon, icon);
        assert_eq!(resolved.source_insets.left, 2);
    }
}
