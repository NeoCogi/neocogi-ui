//! Shared one-axis scrollbar mapping, independent of retained nodes and popup ownership.
//!
//! Screen tracks use i32 rectangles; full content and offsets use i64. Translation
//! into a visible viewport must precede narrowing cumulative menu coordinates.

use crate::{ControlRole, Recti, Vec2i, WidgetPaintCtx};

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
/// Axis selector used by shared scrollbar geometry helpers.
pub enum ScrollbarAxis {
    /// Vertical scrollbar operating on y offsets.
    Vertical,
    /// Horizontal scrollbar operating on x offsets.
    Horizontal,
}

impl ScrollbarAxis {
    /// Returns the rectangle extent along this scrollbar's movement axis.
    fn rect_len(self, rect: Recti) -> i32 {
        match self {
            Self::Vertical => rect.height,
            Self::Horizontal => rect.width,
        }
    }

    /// Returns one point component along this scrollbar's movement axis.
    fn point(self, point: Vec2i) -> i32 {
        match self {
            Self::Vertical => point.y,
            Self::Horizontal => point.x,
        }
    }

    /// Returns the rectangle origin along this scrollbar's movement axis.
    fn origin(self, rect: Recti) -> i32 {
        match self {
            Self::Vertical => rect.y,
            Self::Horizontal => rect.x,
        }
    }

    /// Replaces the rectangle extent along this scrollbar's movement axis.
    fn with_len(self, mut rect: Recti, len: i32) -> Recti {
        match self {
            Self::Vertical => rect.height = len,
            Self::Horizontal => rect.width = len,
        }
        rect
    }

    /// Translates the rectangle origin along this scrollbar's movement axis.
    fn translate(self, mut rect: Recti, amount: i32) -> Recti {
        // translated_axis_origin = rectangle_axis_origin + amount.
        match self {
            Self::Vertical => rect.y = rect.y.saturating_add(amount),
            Self::Horizontal => rect.x = rect.x.saturating_add(amount),
        }
        rect
    }
}

/// Complete mapping between one scrollbar track and one content axis.
///
/// `thumb`, [`Self::drag_delta`], and [`Self::centered_offset`] all use the same scroll range and
/// thumb travel. Keeping those values together is important when the minimum thumb length is
/// larger than the proportional thumb.
#[derive(Copy, Clone, Debug)]
pub struct ScrollbarGeometry {
    /// Axis used to select rectangle and pointer components.
    axis: ScrollbarAxis,
    /// Complete local track rectangle.
    track: Recti,
    /// Proportional, minimum-bounded thumb rectangle inside `track`.
    thumb: Recti,
    /// Largest valid content offset.
    max_offset: i64,
    /// Number of pixels through which the thumb can move.
    thumb_travel: i32,
}

/// Multiplies before dividing in a wider domain and clamps the signed result to one coordinate.
fn scaled_ratio(value: i64, numerator: i64, denominator: i64) -> i64 {
    debug_assert!(numerator >= 0 && denominator > 0);
    // Full content may exceed screen coordinates. Even i64 content times an i32
    // pointer/track value remains exact in i128 before division and narrowing.
    (i128::from(value) * i128::from(numerator) / i128::from(denominator)).clamp(i128::from(i64::MIN), i128::from(i64::MAX)) as i64
}

impl ScrollbarGeometry {
    /// Resolves one track and thumb for the supplied visible/content lengths and offset.
    pub fn new(axis: ScrollbarAxis, track: Recti, view_len: impl Into<i64>, content_len: impl Into<i64>, offset: impl Into<i64>, min_thumb_len: i32) -> Self {
        // Normalize every external length once so later mapping functions operate on valid ranges.
        let track_len = axis.rect_len(track).max(0);
        let view_len = view_len.into().max(0);
        let content_len = content_len.into().max(0);
        let offset = offset.into();
        let max_offset = scrollbar_max_scroll(content_len, view_len);

        // Proportional length represents the visible fraction; the style minimum preserves usability.
        let proportional = if content_len > 0 {
            // proportional_thumb_length = track_length * visible_length / content_length.
            scaled_ratio(i64::from(track_len), view_len, content_len).clamp(0, i64::from(track_len)) as i32
        } else {
            track_len
        };
        let thumb_len = proportional.max(min_thumb_len.max(0)).min(track_len);
        // thumb_travel = track_length - thumb_length.
        let thumb_travel = track_len.saturating_sub(thumb_len).max(0);
        // Map clamped content offset into thumb travel using the same integer ratio inverted by drag.
        let thumb_offset = if max_offset > 0 && thumb_travel > 0 {
            // thumb_offset = clamped_content_offset * thumb_travel / maximum_content_offset.
            scaled_ratio(offset.clamp(0, max_offset), i64::from(thumb_travel), max_offset) as i32
        } else {
            0
        };
        let thumb = axis.translate(axis.with_len(track, thumb_len), thumb_offset);

        Self {
            axis,
            track,
            thumb,
            max_offset,
            thumb_travel,
        }
    }

    /// Returns the single committed upper bound in content units.
    pub fn max_offset(self) -> i64 {
        self.max_offset
    }

    /// Returns the full scrollbar track.
    pub fn track(self) -> Recti {
        self.track
    }

    /// Returns the painted and hit-tested thumb rectangle.
    pub fn thumb(self) -> Recti {
        self.thumb
    }

    /// Converts pointer movement into the exactly inverse content-offset movement.
    pub fn drag_delta(self, delta: Vec2i) -> i64 {
        // A non-scrollable range or immobile thumb cannot produce a meaningful content delta.
        if self.thumb_travel <= 0 || self.max_offset <= 0 {
            return 0;
        }
        // content_delta = pointer_delta * maximum_content_offset / thumb_travel.
        scaled_ratio(i64::from(self.axis.point(delta)), self.max_offset, i64::from(self.thumb_travel))
    }

    /// Returns the offset that centers the thumb on `pointer`, clamped to the track.
    pub fn centered_offset(self, pointer: Vec2i) -> i64 {
        // Track clicks use the same travel/range mapping as dragging, centered on the thumb.
        if self.thumb_travel <= 0 || self.max_offset <= 0 {
            return 0;
        }
        let thumb_len = self.axis.rect_len(self.thumb);
        // track_pointer = pointer_axis_position - track_axis_origin.
        let track_pointer = self.axis.point(pointer).saturating_sub(self.axis.origin(self.track));
        // centered_thumb = clamp(track_pointer - thumb_length / 2, 0, thumb_travel).
        let centered = track_pointer.saturating_sub(thumb_len / 2).clamp(0, self.thumb_travel);
        // content_offset = centered_thumb * maximum_content_offset / thumb_travel.
        scaled_ratio(i64::from(centered), self.max_offset, i64::from(self.thumb_travel))
    }
}

/// Returns the largest scroll offset needed to reveal all content.
pub fn scrollbar_max_scroll(content_len: i64, view_len: i64) -> i64 {
    // maximum_scroll = max(content_length - visible_length, 0).
    content_len.max(0).saturating_sub(view_len.max(0)).max(0)
}

/// Paints the shared track and thumb using the caller's phase-local clip and state.
pub fn paint_scrollbar(ctx: &mut WidgetPaintCtx<'_>, geometry: ScrollbarGeometry) {
    let _ = ctx.draw_control(ControlRole::ScrollbarTrack, geometry.track());
    let _ = ctx.draw_control(ControlRole::ScrollbarThumb, geometry.thumb());
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Full menu ranges survive beyond i32 while both thumb endpoints stay exact.
    #[test]
    fn wide_content_endpoints_and_inverse() {
        let content = 65_536_i64 * 40_000;
        let track = Recti::new(10, 20, 12, 400);
        let start = ScrollbarGeometry::new(ScrollbarAxis::Vertical, track, 400_000, content, 0, 16);
        assert_eq!(start.thumb().y, 20);
        assert_eq!(start.max_offset(), content - 400_000);
        let end = ScrollbarGeometry::new(ScrollbarAxis::Vertical, track, 400_000, content, start.max_offset(), 16);
        assert_eq!(end.thumb().y + end.thumb().height, 420);
        assert_eq!(end.centered_offset(Vec2i::new(10, 420)), end.max_offset());
        assert_eq!(end.drag_delta(Vec2i::new(0, 384)), end.max_offset());
        assert_eq!(end.centered_offset(Vec2i::new(10, -100)), 0);
    }

    /// The widest admitted content and pointer multiplication cannot overflow.
    #[test]
    fn wide_products_and_empty_tracks_are_total() {
        let full = ScrollbarGeometry::new(ScrollbarAxis::Horizontal, Recti::new(0, 0, 100, 8), 10, i64::MAX, 0, 1);
        assert_eq!(full.drag_delta(Vec2i::new(i32::MAX, 0)), i64::MAX);
        for extent in [0, -1] {
            let empty = ScrollbarGeometry::new(ScrollbarAxis::Vertical, Recti::new(0, 0, 8, extent), 0, i64::MAX, 0, 20);
            assert_eq!(empty.drag_delta(Vec2i::new(0, i32::MAX)), 0);
            assert_eq!(empty.centered_offset(Vec2i::new(0, i32::MAX)), 0);
        }
        assert_eq!(scrollbar_max_scroll(-1, 10), 0);
    }

    /// Content shorter than the viewport fills the track even at wide view lengths.
    #[test]
    fn oversized_viewport_has_a_full_inert_thumb() {
        let track = Recti::new(4, 6, 100, 8);
        let geometry = ScrollbarGeometry::new(ScrollbarAxis::Horizontal, track, i64::MAX, 1, 0, 1);
        let thumb = geometry.thumb();
        assert_eq!((thumb.x, thumb.y, thumb.width, thumb.height), (4, 6, 100, 8));
        assert_eq!(geometry.max_offset(), 0);
        assert_eq!(geometry.centered_offset(Vec2i::new(90, 6)), 0);
    }
}
