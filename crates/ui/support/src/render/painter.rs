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
//! Local-coordinate recording into an owned display list.

use super::display_list::DisplayList;
use crate::{
    atlas::{FontId, IconId},
    math::RectExt,
    render::{Color, SliceInsets, NinePatch},
};
use rs_math3d::{Recti, Vec2f, Vec2i, color4b};

/// Records backend-neutral drawing operations in local coordinates.
///
/// A Painter borrows the crate-owned display list for the current frame. It translates local
/// primitives into screen space, attaches the current effective screen-space clip to every
/// operation, and tessellates custom solid geometry without consulting style, input, atlas, the
/// context-owned executor, or [`crate::render::RendererBackend`] state.
///
/// Recording returns an error when the retained frame budget would be exceeded. Callers must
/// propagate that error so traversal can discard the partial frame before submission.
/// Built-in algorithms receive this recorder through their private phase context. Independent
/// widgets use the facade painter over IPainter; this Rust reference never crosses a plugin ABI.
/// A backend-local helper can record ordinary primitives without depending on widget ownership:
///
/// ```
/// use neocogi_ui_support::{prelude::{color, Recti, Vec2f}, render::Painter};
/// fn swatch(painter: &mut Painter<'_>) -> Result<(), neocogi_com::NcStatus> {
///     let bounds = painter.local_rect();
///     painter.fill_rect(bounds, color(42, 48, 60, 255))?;
///     let inset = Recti::new(2, 2, bounds.width - 4, bounds.height - 4);
///     painter.with_clip(inset, |painter| {
///         painter.stroke_line(Vec2f::new(0.0, 0.0),
///             Vec2f::new(bounds.width as f32, bounds.height as f32),
///             2.0, color(110, 190, 255, 255))
///     })
/// }
/// ```
pub struct Painter<'a> {
    /// Display list receiving operations.
    list: &'a mut DisplayList,
    /// Translation from local to screen coordinates.
    origin: Vec2i,
    /// Local drawable extent exposed to callers.
    local_bounds: Recti,
    /// Current effective clip in screen space.
    clip: Recti,
}

impl<'a> Painter<'a> {
    /// Creates a widget-local Painter from one authoritative screen-space content rectangle.
    pub fn for_widget(list: &'a mut DisplayList, screen_content_bounds: Recti, screen_clip: Recti) -> Self {
        let origin = Vec2i::new(screen_content_bounds.x, screen_content_bounds.y);
        let local_bounds = Recti::new(0, 0, screen_content_bounds.width, screen_content_bounds.height);
        Self {
            list,
            origin,
            local_bounds,
            clip: screen_clip,
        }
    }

    /// Creates an internal Painter whose input coordinates are already in screen space.
    pub fn screen_space(list: &'a mut DisplayList, screen_clip: Recti) -> Self {
        Self {
            list,
            origin: Vec2i::default(),
            local_bounds: screen_clip,
            clip: screen_clip,
        }
    }

    /// Returns the drawable rectangle in this Painter's coordinate space.
    pub fn local_rect(&self) -> Recti {
        self.local_bounds
    }

    /// Returns the current effective clip translated into local coordinates.
    pub fn current_clip_rect(&self) -> Recti {
        // local_clip_origin = screen_clip_origin - painter_origin.
        self.clip.relative_to(self.origin)
    }

    /// Records an owned texture without calling foreign code while the display list is borrowed.
    pub fn com_texture(
        &mut self,
        rect: Recti,
        color: Color,
        texture: neocogi_com::ComPtr<neocogi_ui::abi::core::ITextureAbi>,
    ) -> Result<(), neocogi_com::NcStatus> {
        self.com_texture_region(rect, neocogi_ui::abi::core::TextureRegion::FULL, color, texture)?;
        Ok(())
    }

    /// Records explicit source UVs without querying immutable texture dimensions during painting.
    pub fn com_texture_region(
        &mut self,
        rect: Recti,
        source: neocogi_ui::abi::core::TextureRegion,
        color: Color,
        texture: neocogi_com::ComPtr<neocogi_ui::abi::core::ITextureAbi>,
    ) -> Result<(), neocogi_com::NcStatus> {
        self.list.push_com_texture(self.clip, self.screen_rect(rect), source, color, texture)?;
        Ok(())
    }

    /// Appends solid COM geometry directly to the reusable arena; the executor supplies the current
    /// atlas material later, so theme replacement cannot invalidate application-side UV caches.
    pub fn colored_triangles(&mut self, vertices: &[neocogi_ui::abi::core::ColoredVertex]) -> Result<(), neocogi_com::NcStatus> {
        self.list.push_colored_triangles(self.clip, self.origin, vertices)
    }

    /// Copies validated COM atlas geometry and applies this painter's local origin.
    pub fn atlas_triangles(&mut self, vertices: &[neocogi_ui::abi::core::RenderVertex]) -> Result<(), neocogi_com::NcStatus> {
        self.list.push_atlas_triangles(self.clip, self.origin, vertices)?;
        Ok(())
    }

    /// Records a semantic filled rectangle.
    pub fn fill_rect(&mut self, rect: Recti, color: Color) -> Result<(), neocogi_com::NcStatus> {
        // A filled rectangle is represented as a center-only three-by-three patch. Keeping this
        // familiar convenience method lets custom widgets remain concise without bypassing the
        // unified patch path used by built-in flat and image themes.
        self.nine_patch(rect, NinePatch::solid(color))?;
        Ok(())
    }

    /// Records one backend-neutral three-by-three patch.
    pub fn nine_patch(&mut self, rect: Recti, patch: NinePatch) -> Result<(), neocogi_com::NcStatus> {
        // Painter owns only local-to-screen translation and coarse clip rejection. Cell visibility,
        // source slicing, and final geometry are deliberately deferred to the typed GFX operation.
        if !rect.has_positive_area() {
            return Ok(());
        }
        let screen_rect = self.screen_rect(rect);
        if screen_rect.overlaps(self.clip) {
            self.list.push_nine_patch(self.clip, screen_rect, patch)?;
        }
        Ok(())
    }

    /// Records an inside-aligned rectangle outline with the requested integer width.
    pub fn stroke_rect(&mut self, rect: Recti, width: i32, color: Color) -> Result<(), neocogi_com::NcStatus> {
        if !rect.has_positive_area() || color.a == 0 || width <= 0 {
            return Ok(());
        }
        // The proportional tiny-destination policy now belongs to NinePatch geometry rather than a
        // second outline implementation. An empty center naturally produces an inside-aligned
        // border and collapses safely when opposing sides consume the complete destination.
        self.nine_patch(rect, NinePatch::framed(SliceInsets::uniform(width), color, None))?;
        Ok(())
    }

    /// Records one UTF-8 string at a local position using the selected atlas font's glyph coverage.
    ///
    /// Text measurement remains outside Painter. Context's executor performs final glyph lookup,
    /// clipping, and the same underscore substitution used by
    /// [`crate::AtlasHandle::get_text_size`].
    pub fn text(&mut self, font: FontId, text: &str, pos: Vec2i, color: Color) -> Result<(), neocogi_com::NcStatus> {
        if text.is_empty() || color.a == 0 || !self.clip.has_positive_area() {
            return Ok(());
        }
        self.list.push_text(self.clip, font, self.screen_pos(pos), color, text)?;
        Ok(())
    }

    /// Records one atlas icon in a local rectangle.
    pub fn icon(&mut self, id: IconId, rect: Recti, color: Color) -> Result<(), neocogi_com::NcStatus> {
        self.record_rect(rect, color, |list, clip, screen_rect| list.push_icon(clip, id, screen_rect, color))?;
        Ok(())
    }

    /// Tessellates and records one thick local line without clipping its generated triangles.
    pub fn stroke_line(&mut self, from: Vec2f, to: Vec2f, width: f32, color: Color) -> Result<(), neocogi_com::NcStatus> {
        neocogi_ui::geometry::fractional_point(from)?;
        neocogi_ui::geometry::fractional_point(to)?;
        if width < 0.0 {
            return Err(neocogi_com::NC_E_INVALID_ARG);
        }
        neocogi_ui::geometry::fractional_extent(width)?;
        if color.a == 0 || !self.clip.has_positive_area() {
            return Ok(());
        }

        if !self.has_visible_bounds([from, to].into_iter(), width)? {
            return Ok(());
        }
        let offset = Vec2f::new(self.origin.x as f32, self.origin.y as f32);
        self.list
            .push_line(self.clip, from, to, width, color4b(color.r, color.g, color.b, color.a), offset)?;
        Ok(())
    }

    /// Tessellates and records one simple local polygon without clipping its generated triangles.
    ///
    /// Convex polygons use a triangle fan; concave polygons use ear clipping. Degenerate,
    /// non-finite, and self-invalidating inputs safely emit no operation.
    pub fn fill_polygon(&mut self, points: &[Vec2f], color: Color) -> Result<(), neocogi_com::NcStatus> {
        // Reject oversized inputs before even the visibility bounds scan.
        if points.len() > neocogi_platform_contracts::render::abi::MAX_POLYGON_POINTS {
            return Err(neocogi_com::NC_E_OUT_OF_RANGE);
        }

        for &point in points {
            neocogi_ui::geometry::fractional_point(point)?;
        }
        if points.len() < 3 || color.a == 0 || !self.clip.has_positive_area() {
            return Ok(());
        }
        if !self.has_visible_bounds(points.iter().copied(), 0.0)? {
            return Ok(());
        }
        let offset = Vec2f::new(self.origin.x as f32, self.origin.y as f32);
        self.list.push_polygon(self.clip, points, color4b(color.r, color.g, color.b, color.a), offset)?;
        Ok(())
    }

    /// Uses validated integer pixel inputs without a temporary conversion vector.
    pub fn fill_integer_polygon(&mut self, points: &[neocogi_ui::Point], color: Color) -> Result<(), neocogi_com::NcStatus> {
        if points.len() > neocogi_platform_contracts::render::abi::MAX_POLYGON_POINTS {
            return Err(neocogi_com::NC_E_OUT_OF_RANGE);
        }
        for point in points {
            neocogi_ui::geometry::integer_point(Vec2i::new(point.x, point.y))?;
        }
        if points.len() < 3 || color.a == 0 || !self.clip.has_positive_area() {
            return Ok(());
        }
        if !self.has_visible_bounds(points.iter().map(|p| Vec2f::new(p.x as f32, p.y as f32)), 0.0)? {
            return Ok(());
        }
        let offset = Vec2f::new(self.origin.x as f32, self.origin.y as f32);
        self.list
            .push_integer_polygon(self.clip, points, color4b(color.r, color.g, color.b, color.a), offset)
    }

    /// Tessellates copied COM polygon points directly into this painter's retained frame storage.
    pub fn fill_contract_polygon(&mut self, points: &[neocogi_ui::abi::core::PaintPoint], color: Color) -> Result<(), neocogi_com::NcStatus> {
        if points.len() > neocogi_platform_contracts::render::abi::MAX_POLYGON_POINTS {
            return Err(neocogi_com::NC_E_OUT_OF_RANGE);
        }
        for point in points {
            neocogi_ui::geometry::fractional_point(Vec2f::new(point.x, point.y))?;
        }
        if points.len() < 3 || color.a == 0 || !self.clip.has_positive_area() {
            return Ok(());
        }
        if !self.has_visible_bounds(points.iter().map(|p| Vec2f::new(p.x, p.y)), 0.0)? {
            return Ok(());
        }
        let offset = Vec2f::new(self.origin.x as f32, self.origin.y as f32);
        self.list
            .push_contract_polygon(self.clip, points, color4b(color.r, color.g, color.b, color.a), offset)?;
        Ok(())
    }

    /// Rejects definitely hidden primitives using exact fixed bounds before any pixel narrowing.
    /// All input points are admitted even when the complete shape falls outside the current clip.
    fn has_visible_bounds(&self, points: impl Iterator<Item = Vec2f>, width: f32) -> Result<bool, neocogi_com::NcStatus> {
        use neocogi_ui::geometry::{self, WidePoint, WideRectangle, WideScalar, wide_integer};
        use rs_math3d::Vector;
        let half = geometry::fractional_extent(width)?
            .try_rescale::<i64, 12>()
            .map_err(geometry::status)?
            .checked_div(WideScalar::from_bits(8192))
            .map_err(geometry::status)?;
        let mut bounds: Option<(WidePoint, WidePoint)> = None;
        for point in points {
            let point = geometry::wide_fractional_point(point)?;
            bounds = Some(match bounds {
                None => (point, point),
                Some((min, max)) => (WidePoint::min(&min, &point), WidePoint::max(&max, &point)),
            });
        }
        let Some((min, max)) = bounds else {
            return Ok(false);
        };
        let origin = WidePoint::new(wide_integer(self.origin.x), wide_integer(self.origin.y));
        let expansion = WidePoint::new(half, half);
        let min = WidePoint::checked_sub_vv(&min, &expansion)
            .and_then(|p| WidePoint::checked_add_vv(&p, &origin))
            .map_err(geometry::status)?;
        let max = WidePoint::checked_add_vv(&max, &expansion)
            .and_then(|p| WidePoint::checked_add_vv(&p, &origin))
            .map_err(geometry::status)?;
        let bounds = WideRectangle::checked_from(&min, &max).map_err(geometry::status)?;
        let clip = WideRectangle::new(
            wide_integer(self.clip.x),
            wide_integer(self.clip.y),
            wide_integer(self.clip.width),
            wide_integer(self.clip.height),
        );
        Ok(bounds
            .checked_intersect(&clip)
            .map_err(geometry::status)?
            .is_some_and(|r| r.width > WideScalar::ZERO && r.height > WideScalar::ZERO))
    }

    /// Executes `paint` with a clip narrowed by a local rectangle.
    ///
    /// The parent Painter is unchanged after the closure returns. There is no mutable clip stack,
    /// public push/pop pair, or Drop-time restoration behavior.
    pub fn with_clip<T>(&mut self, rect: Recti, paint: impl FnOnce(&mut Painter<'_>) -> T) -> T {
        let screen_clip = self.screen_rect(rect);
        let effective = self.clip.positive_intersection(screen_clip).unwrap_or_else(|| {
            // empty_origin = componentwise_max(parent_clip_origin, requested_clip_origin).
            Recti::new(self.clip.x.max(screen_clip.x), self.clip.y.max(screen_clip.y), 0, 0)
        });
        let mut child = Painter {
            list: &mut *self.list,
            origin: self.origin,
            local_bounds: self.local_bounds,
            clip: effective,
        };
        paint(&mut child)
    }

    /// Converts a local integer position into screen space.
    fn screen_pos(&self, pos: Vec2i) -> Vec2i {
        // screen_position = local_position + painter_origin.
        Vec2i::new(pos.x.saturating_add(self.origin.x), pos.y.saturating_add(self.origin.y))
    }

    /// Converts a local rectangle into screen space.
    fn screen_rect(&self, rect: Recti) -> Recti {
        rect.translated(self.origin)
    }

    /// Applies the common rectangle visibility policy and records one semantic rectangle operation.
    fn record_rect(
        &mut self,
        rect: Recti,
        color: Color,
        record: impl FnOnce(&mut DisplayList, Recti, Recti) -> Result<(), neocogi_com::NcStatus>,
    ) -> Result<(), neocogi_com::NcStatus> {
        if !rect.has_positive_area() || color.a == 0 {
            return Ok(());
        }
        let screen_rect = self.screen_rect(rect);
        if screen_rect.overlaps(self.clip) {
            record(self.list, self.clip, screen_rect)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::test_atlas;
    use crate::{NinePatchCell, color};
    use super::super::display_list::DrawKind;

    fn rect_tuple(rect: Recti) -> (i32, i32, i32, i32) {
        (rect.x, rect.y, rect.width, rect.height)
    }

    #[test]
    fn fill_rect_records_a_translated_semantic_operation() {
        let mut list = DisplayList::new();
        {
            let mut painter = Painter::for_widget(&mut list, Recti::new(10, 20, 100, 80), Recti::new(12, 22, 50, 40));
            painter.fill_rect(Recti::new(1, 2, 10, 12), color(1, 2, 3, 255)).unwrap();
        }

        assert_eq!(list.ops.len(), 1);
        assert!(list.solid_geometry.is_empty());
        assert_eq!(rect_tuple(list.ops[0].clip), (12, 22, 50, 40));
        let DrawKind::NinePatch { rect, patch } = &list.ops[0].kind else {
            panic!("expected a semantic nine-patch");
        };
        assert_eq!(rect_tuple(*rect), (11, 22, 10, 12));
        let crate::NinePatchContent::Flat { cells } = patch.content else {
            panic!("expected flat nine-patch content");
        };
        let NinePatchCell::Color { color } = cells.center else {
            panic!("expected a colored center cell");
        };
        assert_eq!((color.r, color.g, color.b, color.a), (1, 2, 3, 255));
    }

    #[test]
    fn scoped_nested_clips_narrow_without_mutating_the_parent() {
        let mut list = DisplayList::new();
        {
            let mut painter = Painter::for_widget(&mut list, Recti::new(10, 20, 50, 50), Recti::new(10, 20, 50, 50));
            assert_eq!(rect_tuple(painter.current_clip_rect()), (0, 0, 50, 50));
            painter.with_clip(Recti::new(5, 5, 20, 20), |painter| {
                assert_eq!(rect_tuple(painter.current_clip_rect()), (5, 5, 20, 20));
                painter.with_clip(Recti::new(10, 0, 20, 20), |painter| {
                    assert_eq!(rect_tuple(painter.current_clip_rect()), (10, 5, 15, 15));
                    painter.fill_rect(Recti::new(0, 0, 50, 50), color(255, 0, 0, 255)).unwrap();
                });
            });
            assert_eq!(rect_tuple(painter.current_clip_rect()), (0, 0, 50, 50));
            painter.fill_rect(Recti::new(0, 0, 2, 2), color(0, 255, 0, 255)).unwrap();
        }

        assert_eq!(list.ops.len(), 2);
        assert_eq!(rect_tuple(list.ops[0].clip), (20, 25, 15, 15));
        assert_eq!(rect_tuple(list.ops[1].clip), (10, 20, 50, 50));
    }

    #[test]
    fn disjoint_clip_scope_cannot_expand_visibility() {
        let mut list = DisplayList::new();
        {
            let mut painter = Painter::for_widget(&mut list, Recti::new(10, 20, 50, 50), Recti::new(10, 20, 50, 50));
            painter.with_clip(Recti::new(100, 100, 10, 10), |painter| {
                assert_eq!(painter.current_clip_rect().width, 0);
                painter.with_clip(Recti::new(0, 0, 500, 500), |painter| {
                    assert_eq!(painter.current_clip_rect().width, 0);
                    painter.fill_rect(Recti::new(0, 0, 500, 500), color(255, 0, 0, 255)).unwrap();
                });
            });
        }
        assert!(list.is_empty());
    }

    #[test]
    fn line_triangles_remain_unclipped_and_carry_the_effective_clip() {
        let mut list = DisplayList::new();
        {
            let mut painter = Painter::for_widget(&mut list, Recti::new(0, 0, 20, 20), Recti::new(0, 0, 5, 5));
            painter
                .stroke_line(Vec2f::new(-10.0, 2.0), Vec2f::new(20.0, 2.0), 2.0, color(255, 0, 0, 255))
                .unwrap();
        }

        assert_eq!(list.ops.len(), 1);
        assert_eq!(list.solid_geometry.triangles().len(), 2);
        assert_eq!(rect_tuple(list.ops[0].clip), (0, 0, 5, 5));
        let positions = list
            .solid_geometry
            .triangles()
            .iter()
            .flat_map(|triangle| triangle.vertices())
            .map(|vertex| vertex.position)
            .collect::<Vec<_>>();
        assert!(positions.iter().any(|position| position.x < 0.0));
        assert!(positions.iter().any(|position| position.x > 5.0));
    }

    #[test]
    fn polygon_and_line_geometry_use_triangle_ranges() {
        let mut list = DisplayList::new();
        {
            let mut painter = Painter::for_widget(&mut list, Recti::new(5, 7, 100, 100), Recti::new(0, 0, 200, 200));
            painter
                .fill_polygon(
                    &[Vec2f::new(0.0, 0.0), Vec2f::new(10.0, 0.0), Vec2f::new(10.0, 10.0), Vec2f::new(0.0, 10.0)],
                    color(255, 255, 255, 255),
                )
                .unwrap();
            painter
                .stroke_line(Vec2f::new(0.0, 20.0), Vec2f::new(10.0, 20.0), 2.0, color(255, 255, 255, 255))
                .unwrap();
        }

        assert_eq!(list.ops.len(), 1, "adjacent compatible geometry should merge");
        assert_eq!(list.solid_geometry.triangles().len(), 4);
        let DrawKind::SolidTriangles { triangles } = &list.ops[0].kind else {
            panic!("expected a solid-triangle range");
        };
        assert_eq!(triangles.as_range(), 0..4);
        assert!(
            list.solid_geometry
                .triangles()
                .iter()
                .flat_map(|triangle| triangle.vertices())
                .all(|vertex| vertex.position.x >= 5.0 && vertex.position.y >= 7.0)
        );
    }

    #[test]
    fn stroke_rect_records_one_nine_patch_not_triangle_geometry() {
        let mut list = DisplayList::new();
        {
            let mut painter = Painter::screen_space(&mut list, Recti::new(0, 0, 20, 20));
            painter.stroke_rect(Recti::new(2, 2, 10, 10), 2, color(255, 255, 255, 255)).unwrap();
        }
        assert_eq!(list.ops.len(), 1);
        assert!(matches!(list.ops[0].kind, DrawKind::NinePatch { .. }));
        let patches = list.debug_nine_patches();
        assert_eq!(patches.len(), 1);
        let (destination, clip, patch) = patches[0];
        assert_eq!(rect_tuple(destination), (2, 2, 10, 10));
        assert_eq!(rect_tuple(clip), (0, 0, 20, 20));
        assert_eq!((patch.insets.left, patch.insets.top, patch.insets.right, patch.insets.bottom), (2, 2, 2, 2));
        let crate::NinePatchContent::Flat { cells } = patch.content else {
            panic!("a stroked rectangle must retain one typed flat patch");
        };
        assert!(matches!(cells.center, NinePatchCell::Empty));
        assert!(matches!(cells.top_left, NinePatchCell::Color { .. }));
        assert!(patch.center_visible);
        assert!(list.solid_geometry.is_empty());
    }

    #[test]
    fn semantic_primitives_translate_and_retain_clipping_for_the_compositor() {
        // Resolve semantic resources from one concrete atlas even though this focused test stops
        // after recording. Real atlas-scoped capabilities ensure Painter is exercised through the
        // same typed API used by renderer-bound application code.
        let atlas = test_atlas();
        let font = atlas.font_id("body").unwrap();
        let icon = atlas.white_icon();
        let mut list = DisplayList::new();
        {
            let mut painter = Painter::for_widget(&mut list, Recti::new(10, 20, 100, 100), Recti::new(10, 20, 20, 20));
            painter.text(font, "label", Vec2i::new(1, 2), color(255, 255, 255, 255)).unwrap();
            painter.icon(icon, Recti::new(2, 3, 4, 5), color(255, 255, 255, 255)).unwrap();
            painter
                .com_texture(
                    Recti::new(100, 100, 10, 10),
                    color(255, 255, 255, 255),
                    crate::test_support::test_image(10, 10).into_interface(),
                )
                .unwrap();
        }

        // COM textures retain their authoritative clip; GFX decides whether they contribute.
        assert_eq!(list.ops.len(), 3);
        assert!(matches!(list.ops[2].kind, DrawKind::ComTexture { .. }));
        let DrawKind::Text { pos, .. } = &list.ops[0].kind else {
            panic!("expected text");
        };
        assert_eq!((pos.x, pos.y), (11, 22));
        let DrawKind::Icon { rect, .. } = &list.ops[1].kind else {
            panic!("expected icon");
        };
        assert_eq!(rect_tuple(*rect), (12, 23, 4, 5));
    }

    #[test]
    fn fully_hidden_and_degenerate_solid_geometry_is_rejected() {
        let mut list = DisplayList::new();
        {
            let mut painter = Painter::screen_space(&mut list, Recti::new(0, 0, 20, 20));
            assert_eq!(rect_tuple(painter.local_rect()), (0, 0, 20, 20));
            painter
                .stroke_line(Vec2f::new(100.0, 100.0), Vec2f::new(120.0, 100.0), 2.0, color(255, 255, 255, 255))
                .unwrap();
            painter
                .fill_polygon(
                    &[Vec2f::new(100.0, 100.0), Vec2f::new(120.0, 100.0), Vec2f::new(110.0, 120.0)],
                    color(255, 255, 255, 255),
                )
                .unwrap();
            painter
                .fill_polygon(&[Vec2f::new(0.0, 0.0), Vec2f::new(1.0, 1.0), Vec2f::new(2.0, 2.0)], color(255, 255, 255, 255))
                .unwrap();
        }
        assert!(list.is_empty());
    }
}
