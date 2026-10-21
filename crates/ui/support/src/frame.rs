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

//! Pure frame geometry and scoped recording, with no retained node or root ownership.
use crate::render::Painter;
use crate::{AvailableSpace, Constraints, ControlState, Dimensioni, FrameRole, NinePatch, Recti, SliceInsets, Skin, SurfaceState, Visual};

/// Geometry derived from one authoritative outer allocation.
#[derive(Copy, Clone, Debug)]
pub struct FrameGeometry {
    /// Exact caller-owned outer border box in the caller's coordinate system.
    pub outer: Recti,
    /// Positive content box after insets, or no drawable content when the frame consumes it.
    pub content: Option<Recti>,
}

impl FrameGeometry {
    /// Returns a non-negative rectangle suitable for layout/traversal when no content survives.
    pub fn content_or_empty(self) -> Recti {
        self.content.unwrap_or_else(|| Recti::new(self.outer.x, self.outer.y, 0, 0))
    }
}

/// Resolves the border box and its derived content rectangle.
pub fn frame_geometry(outer: Recti, role: Option<FrameRole>, style: &Skin) -> FrameGeometry {
    // A non-framed node uses zero insets. A framed node uses its semantic normal-state geometry;
    // every interaction state for one role is required to preserve those destination insets.
    let insets = role
        .map(|role| normal_frame_visual(style, role).patch.insets.normalized())
        .unwrap_or(SliceInsets::ZERO);
    frame_geometry_with_insets(outer, insets)
}

/// Resolves one node frame from its deliberately limited surface-or-control role.
pub fn frame_visual(style: &Skin, role: FrameRole, control_state: ControlState) -> Visual {
    match role {
        FrameRole::Surface(role) => {
            // Structural surfaces inherit only availability from the surrounding control state.
            let state = SurfaceState::from_enabled(!matches!(control_state, ControlState::Disabled));
            style.surface(role, state)
        }
        FrameRole::Control(role) => style.control(role, control_state),
    }
}

/// Resolves the stable state used for frame geometry during measurement and placement.
pub fn normal_frame_visual(style: &Skin, role: FrameRole) -> Visual {
    match role {
        FrameRole::Surface(role) => style.surface(role, SurfaceState::Normal),
        FrameRole::Control(role) => style.control(role, ControlState::Enabled(crate::PointerState::Normal)),
    }
}

/// Resolves a border box using structural insets supplied independently from appearance artwork.
pub fn frame_geometry_with_insets(outer: Recti, insets: SliceInsets) -> FrameGeometry {
    // Window chrome uses this path because its fixed corner artwork may extend much farther along
    // an edge than the narrow client inset represented by that edge's real border thickness.
    let insets = insets.normalized();
    let content = if outer.width <= 0 || outer.height <= 0 {
        None
    } else if insets.horizontal_extent() == 0 && insets.vertical_extent() == 0 {
        Some(outer)
    } else {
        checked_inset(outer, insets)
    };
    FrameGeometry { outer, content }
}

/// Reconstructs a framed outer rectangle from one known content rectangle and matching insets.
pub fn outer_from_content(content: Recti, insets: SliceInsets) -> Recti {
    // This is the exact inverse of `checked_inset` for a valid content rectangle. Saturation keeps
    // the operation total at coordinate limits while ordinary layout values round-trip exactly.
    let insets = insets.normalized();
    Recti::new(
        content.x.saturating_sub(insets.left),
        content.y.saturating_sub(insets.top),
        content.width.saturating_add(insets.horizontal_extent()),
        content.height.saturating_add(insets.vertical_extent()),
    )
}

/// Removes a frame from each finite measurement bound while preserving unbounded axes.
pub fn content_constraints(constraints: Constraints, insets: SliceInsets) -> Constraints {
    // Each axis removes its independently configured pair of patch cells. Unbounded space stays
    // unbounded because a finite frame cannot constrain an intrinsically open measurement axis.
    Constraints::new(
        inset_available(constraints.width, insets.horizontal_extent()),
        inset_available(constraints.height, insets.vertical_extent()),
    )
}

/// Adds a resolved frame to positive preferred content dimensions.
pub fn outer_preferred(preferred: Dimensioni, insets: SliceInsets) -> Dimensioni {
    // Positive content receives both fixed patch sides on each axis. Non-positive sentinel
    // dimensions retain their established meaning instead of becoming visible solely due to chrome.
    Dimensioni::new(
        expand_positive_axis(preferred.width, insets.horizontal_extent()),
        expand_positive_axis(preferred.height, insets.vertical_extent()),
    )
}

/// Paints a checked inside-aligned internal frame and returns its usable interior.
pub fn paint_internal_frame(painter: &mut Painter<'_>, outer: Recti, patch: NinePatch) -> Result<Option<Recti>, neocogi_com::NcStatus> {
    // Invalid outer geometry produces neither layout content nor paint. Positive rectangles share
    // the same normalized inset calculation used by node layout before one compact patch record.
    if outer.width <= 0 || outer.height <= 0 {
        return Ok(None);
    }
    let insets = patch.insets.normalized();
    let content = if insets.horizontal_extent() == 0 && insets.vertical_extent() == 0 {
        Some(outer)
    } else {
        checked_inset(outer, insets)
    };
    painter.with_clip(outer, |painter| {
        // NinePatch itself decides which of its nine cells are visible. Structural insets therefore
        // remain effective even when border artwork or colors are transparent.
        painter.nine_patch(outer, patch)
    })?;
    Ok(content)
}

/// Removes four normalized patch insets from one positive outer rectangle.
fn checked_inset(outer: Recti, insets: SliceInsets) -> Option<Recti> {
    // Reject a fully consumed axis so callers represent the lack of usable content explicitly.
    let insets = insets.normalized();
    let horizontal = insets.left.checked_add(insets.right)?;
    let vertical = insets.top.checked_add(insets.bottom)?;
    if horizontal >= outer.width || vertical >= outer.height {
        return None;
    }
    // Leading insets move the content origin while both opposing sides reduce its dimensions.
    let x = checked_add(outer.x, insets.left)?;
    let y = checked_add(outer.y, insets.top)?;
    let content_width = outer.width.checked_sub(horizontal)?;
    let content_height = outer.height.checked_sub(vertical)?;
    Some(Recti::new(x, y, content_width, content_height))
}

/// Removes a non-negative inset without conflating a bounded zero with unbounded space.
fn inset_available(space: AvailableSpace, inset: i32) -> AvailableSpace {
    match space {
        AvailableSpace::Unbounded => AvailableSpace::Unbounded,
        AvailableSpace::Bounded(value) => AvailableSpace::bounded(value.saturating_sub(inset.max(0))),
    }
}

fn expand_positive_axis(value: i32, inset_extent: i32) -> i32 {
    if value <= 0 {
        value
    } else {
        // Preferred geometry is allowed to saturate; exact placement will still be bounded by its
        // parent constraint, and an extreme font or style must not turn measurement into a panic.
        value.saturating_add(inset_extent.max(0))
    }
}

fn checked_add(left: i32, right: i32) -> Option<i32> {
    // sum = left + right, rejected when it is outside the i32 range.
    i32::try_from(i64::from(left) + i64::from(right)).ok()
}

/// One mapping serves both COM policy decoding and concrete built-in reporting.
pub(crate) const FRAME_ROLES: &[(neocogi_ui_core_contracts::WidgetFrame, crate::FrameRole)] = &[
    (
        neocogi_ui_core_contracts::WidgetFrame::Generic,
        crate::FrameRole::Surface(crate::SurfaceRole::GenericFrame),
    ),
    (
        neocogi_ui_core_contracts::WidgetFrame::Button,
        crate::FrameRole::Control(crate::ControlRole::Button),
    ),
    (
        neocogi_ui_core_contracts::WidgetFrame::Checkbox,
        crate::FrameRole::Control(crate::ControlRole::Checkbox),
    ),
    (
        neocogi_ui_core_contracts::WidgetFrame::TextInput,
        crate::FrameRole::Control(crate::ControlRole::TextInput),
    ),
    (
        neocogi_ui_core_contracts::WidgetFrame::Item,
        crate::FrameRole::Control(crate::ControlRole::Item),
    ),
    (
        neocogi_ui_core_contracts::WidgetFrame::Combo,
        crate::FrameRole::Control(crate::ControlRole::Combo),
    ),
    (
        neocogi_ui_core_contracts::WidgetFrame::SliderTrack,
        crate::FrameRole::Control(crate::ControlRole::SliderTrack),
    ),
    (
        neocogi_ui_core_contracts::WidgetFrame::SliderThumb,
        crate::FrameRole::Control(crate::ControlRole::SliderThumb),
    ),
    (
        neocogi_ui_core_contracts::WidgetFrame::ScrollbarTrack,
        crate::FrameRole::Control(crate::ControlRole::ScrollbarTrack),
    ),
    (
        neocogi_ui_core_contracts::WidgetFrame::ScrollbarThumb,
        crate::FrameRole::Control(crate::ControlRole::ScrollbarThumb),
    ),
    (
        neocogi_ui_core_contracts::WidgetFrame::CloseButton,
        crate::FrameRole::Control(crate::ControlRole::CloseButton),
    ),
    (
        neocogi_ui_core_contracts::WidgetFrame::MinimizeButton,
        crate::FrameRole::Control(crate::ControlRole::MinimizeButton),
    ),
    (
        neocogi_ui_core_contracts::WidgetFrame::MaximizeButton,
        crate::FrameRole::Control(crate::ControlRole::MaximizeButton),
    ),
    (
        neocogi_ui_core_contracts::WidgetFrame::RestoreButton,
        crate::FrameRole::Control(crate::ControlRole::RestoreButton),
    ),
    (
        neocogi_ui_core_contracts::WidgetFrame::ResizeGrip,
        crate::FrameRole::Control(crate::ControlRole::ResizeGrip),
    ),
    (
        neocogi_ui_core_contracts::WidgetFrame::Window,
        crate::FrameRole::Surface(crate::SurfaceRole::Window),
    ),
    (
        neocogi_ui_core_contracts::WidgetFrame::Panel,
        crate::FrameRole::Surface(crate::SurfaceRole::Panel),
    ),
];

/// Encodes the exact built-in frame family without collapsing input or item roles.
pub fn frame_contract(role: crate::FrameRole) -> neocogi_ui_core_contracts::WidgetFrame {
    FRAME_ROLES.iter().find(|(_, value)| *value == role).expect("all frame roles are represented").0
}

/// Decodes a validated semantic frame role for runtime-owned paint resolution.
pub fn local_frame(role: neocogi_ui_core_contracts::WidgetFrame) -> Result<crate::FrameRole, neocogi_com::NcStatus> {
    FRAME_ROLES
        .iter()
        .find(|(contract, _)| *contract == role)
        .map(|(_, local)| *local)
        .ok_or(neocogi_com::NC_E_INVALID_ARG)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SurfaceRole;
    use crate::{Color, color};
    use crate::render::DisplayList;
    use crate::test_support::{replace_surface_patches, test_atlas, test_skin};

    /// Replaces the generic frame role used by the frame geometry under test.
    fn with_frame(mut style: Skin, patch: NinePatch) -> Skin {
        // Tests mutate the same concrete catalog entry that production generic framing resolves.
        replace_surface_patches(&mut style, SurfaceRole::GenericFrame, |_| patch);
        style
    }

    #[test]
    fn frame_geometry_derives_inside_content() {
        let atlas = test_atlas();
        let style = with_frame(test_skin(&atlas), NinePatch::framed(SliceInsets::uniform(2), color(1, 2, 3, 255), None));
        let geometry = frame_geometry(Recti::new(10, 20, 30, 40), Some(FrameRole::Surface(SurfaceRole::GenericFrame)), &style);
        assert_eq!(rect_tuple(geometry.outer), (10, 20, 30, 40));
        assert_eq!(geometry.content.map(rect_tuple), Some((12, 22, 26, 36)));
    }

    /// Inverting valid content geometry recovers all four outer edges without fixed offsets.
    #[test]
    fn outer_from_content_round_trips_frame_geometry() {
        let insets = SliceInsets { left: 2, top: 3, right: 5, bottom: 7 };
        let outer = Recti::new(11, 13, 80, 50);
        let content = frame_geometry_with_insets(outer, insets).content.expect("frame leaves positive content");
        let recovered = outer_from_content(content, insets);
        assert_eq!(
            (recovered.x, recovered.y, recovered.width, recovered.height),
            (outer.x, outer.y, outer.width, outer.height)
        );
    }

    #[test]
    fn zero_width_frame_keeps_the_complete_content_rect() {
        let atlas = test_atlas();
        let style = with_frame(test_skin(&atlas), NinePatch::framed(SliceInsets::ZERO, color(1, 2, 3, 255), None));
        let outer = Recti::new(10, 20, 30, 40);
        let geometry = frame_geometry(outer, Some(FrameRole::Surface(SurfaceRole::GenericFrame)), &style);
        assert_eq!(geometry.content.map(rect_tuple), Some(rect_tuple(outer)));
    }

    #[test]
    fn transparent_border_keeps_structural_inset() {
        let atlas = test_atlas();
        let style = with_frame(test_skin(&atlas), NinePatch::framed(SliceInsets::uniform(1), color(0, 0, 0, 0), None));
        assert_eq!(
            frame_geometry(Recti::new(4, 5, 8, 7), Some(FrameRole::Surface(SurfaceRole::GenericFrame)), &style)
                .content
                .map(rect_tuple),
            Some((5, 6, 6, 5))
        );
    }

    #[test]
    fn tiny_frame_has_no_content() {
        let atlas = test_atlas();
        let style = test_skin(&atlas);
        assert!(
            frame_geometry(Recti::new(7, 8, 2, 10), Some(FrameRole::Surface(SurfaceRole::GenericFrame)), &style)
                .content
                .is_none()
        );
    }

    #[test]
    fn preferred_measurement_preserves_non_positive_results() {
        let preferred = outer_preferred(Dimensioni::new(10, -1), SliceInsets::uniform(2));
        assert_eq!((preferred.width, preferred.height), (14, -1));
    }

    #[test]
    fn frame_inset_preserves_zero_and_unbounded_as_distinct_constraints() {
        let constraints = Constraints::new(AvailableSpace::Bounded(1), AvailableSpace::Unbounded);
        assert_eq!(
            content_constraints(constraints, SliceInsets::uniform(2)),
            Constraints::new(AvailableSpace::Bounded(0), AvailableSpace::Unbounded)
        );
    }

    #[test]
    fn internal_frame_records_one_semantic_border_and_fill_patch() {
        let outer = Recti::new(10, 20, 8, 7);
        let viewport = Recti::new(0, 0, 100, 100);
        let border = color(1, 2, 3, 255);
        let fill = color(4, 5, 6, 255);
        let mut list = DisplayList::new();
        let mut painter = Painter::screen_space(&mut list, viewport);

        let content = paint_internal_frame(&mut painter, outer, NinePatch::framed(SliceInsets::uniform(1), border, Some(fill))).unwrap();
        assert_eq!(content.map(rect_tuple), Some((11, 21, 6, 5)));

        let recorded = list.debug_nine_patches();
        assert_eq!(recorded.len(), 1);
        let (destination, clip, patch) = recorded[0];
        assert_eq!(rect_tuple(destination), rect_tuple(outer));
        assert_eq!(rect_tuple(clip), rect_tuple(outer));
        assert_eq!(inset_tuple(patch.insets), (1, 1, 1, 1));
        assert!(patch.center_visible);
        let crate::NinePatchContent::Flat { cells } = patch.content else {
            panic!("an internal flat frame must retain flat patch content");
        };
        assert_eq!(cell_color(cells.top_left).map(color_tuple), Some(color_tuple(border)));
        assert_eq!(cell_color(cells.top).map(color_tuple), Some(color_tuple(border)));
        assert_eq!(cell_color(cells.top_right).map(color_tuple), Some(color_tuple(border)));
        assert_eq!(cell_color(cells.left).map(color_tuple), Some(color_tuple(border)));
        assert_eq!(cell_color(cells.center).map(color_tuple), Some(color_tuple(fill)));
        assert_eq!(cell_color(cells.right).map(color_tuple), Some(color_tuple(border)));
        assert_eq!(cell_color(cells.bottom_left).map(color_tuple), Some(color_tuple(border)));
        assert_eq!(cell_color(cells.bottom).map(color_tuple), Some(color_tuple(border)));
        assert_eq!(cell_color(cells.bottom_right).map(color_tuple), Some(color_tuple(border)));
    }

    #[test]
    fn tiny_internal_frame_retains_one_patch_while_layout_has_no_content() {
        let outer = Recti::new(3, 4, 2, 9);
        let viewport = Recti::new(0, 0, 100, 100);
        let border = color(7, 8, 9, 255);
        let mut list = DisplayList::new();
        let mut painter = Painter::screen_space(&mut list, viewport);

        assert!(
            paint_internal_frame(
                &mut painter,
                outer,
                NinePatch::framed(SliceInsets::uniform(1), border, Some(color(10, 11, 12, 255))),
            )
            .unwrap()
            .is_none()
        );
        let recorded = list.debug_nine_patches();
        assert_eq!(recorded.len(), 1);
        let (destination, _, patch) = recorded[0];
        assert_eq!(rect_tuple(destination), rect_tuple(outer));
        assert_eq!(inset_tuple(patch.insets), (1, 1, 1, 1));
        let crate::NinePatchContent::Flat { cells } = patch.content else {
            panic!("a tiny internal frame must remain a semantic flat patch");
        };
        assert_eq!(cell_color(cells.top).map(color_tuple), Some(color_tuple(border)));
        assert_eq!(cell_color(cells.center).map(color_tuple), Some((10, 11, 12, 255)));
    }

    #[test]
    fn transparent_internal_border_retains_insets_and_explicit_empty_cells() {
        let outer = Recti::new(5, 6, 9, 8);
        let viewport = Recti::new(0, 0, 100, 100);
        let fill = color(20, 30, 40, 255);
        let mut list = DisplayList::new();
        let mut painter = Painter::screen_space(&mut list, viewport);

        let content = paint_internal_frame(&mut painter, outer, NinePatch::framed(SliceInsets::uniform(2), color(0, 0, 0, 0), Some(fill))).unwrap();
        assert_eq!(content.map(rect_tuple), Some((7, 8, 5, 4)));
        let recorded = list.debug_nine_patches();
        assert_eq!(recorded.len(), 1);
        let (destination, _, patch) = recorded[0];
        assert_eq!(rect_tuple(destination), rect_tuple(outer));
        assert_eq!(inset_tuple(patch.insets), (2, 2, 2, 2));
        let crate::NinePatchContent::Flat { cells } = patch.content else {
            panic!("a transparent internal border must remain a semantic flat patch");
        };
        assert!(matches!(cells.top_left, crate::NinePatchCell::Empty));
        assert_eq!(cell_color(cells.center).map(color_tuple), Some(color_tuple(fill)));
    }

    /// Converts a rectangle into components supported by equality assertions.
    fn rect_tuple(rect: Recti) -> (i32, i32, i32, i32) {
        // The external math type intentionally does not define structural equality.
        (rect.x, rect.y, rect.width, rect.height)
    }

    /// Converts a color into components supported by equality assertions.
    fn color_tuple(color: Color) -> (u8, u8, u8, u8) {
        // Component comparison makes failures report the complete expected channel sequence.
        (color.r, color.g, color.b, color.a)
    }

    /// Converts semantic insets into components supported by equality assertions.
    fn inset_tuple(insets: SliceInsets) -> (i32, i32, i32, i32) {
        // Keeping this assertion at the UI boundary verifies no destination slicing happened here.
        (insets.left, insets.top, insets.right, insets.bottom)
    }

    /// Returns the concrete color of a visible flat patch cell.
    fn cell_color(cell: crate::NinePatchCell) -> Option<Color> {
        // Empty cells map to absence so assertions can distinguish structural insets from pixels.
        match cell {
            crate::NinePatchCell::Empty => None,
            crate::NinePatchCell::Color { color } => Some(color),
        }
    }
}
