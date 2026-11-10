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

//! Lightweight geometry conveniences shared across retained layout and rendering.

use rs_math3d::{Recti, Vec2i, Vector};
use neocogi_ui::geometry::{wide_integer, WidePoint, WideRectangle, WideScalar};

/// Clamps one wider geometry calculation into the crate's signed coordinate domain.
pub fn clamp_i64_to_i32(value: i64) -> i32 {
    // Multi-term expressions should retain cancellation in i64 and narrow exactly once at the
    // Recti/Vec2i boundary. A cast would wrap values outside the destination range.
    value.clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32
}

/// UI pixel/half-open policies over rs-math3d checked fixed-point rectangle operations.
pub trait RectExt: Sized {
    /// Expands the rectangle uniformly on all sides with saturated coordinates and extents.
    fn expanded(self, amount: i32) -> Self;

    /// Translates the rectangle while saturating its origin at the integer bounds.
    fn translated(self, offset: Vec2i) -> Self;

    /// Converts the rectangle into coordinates relative to an origin with saturated subtraction.
    fn relative_to(self, origin: Vec2i) -> Self;

    /// Returns whether both rectangle extents are positive.
    fn has_positive_area(self) -> bool;

    /// Returns whether a point lies inside the positive rectangle using overflow-safe edges.
    fn contains_point(self, point: Vec2i) -> bool;

    /// Returns the positive-area portion shared with another rectangle.
    fn positive_intersection(self, other: Self) -> Option<Self>;

    /// Returns whether two positive-area rectangles overlap.
    fn overlaps(self, other: Self) -> bool;

    /// Returns the smallest rectangle containing both input rectangles.
    fn union(self, other: Self) -> Self;
}

/// Converts stored integer pixel geometry to the shared wide fixed working domain.
fn wide_rect(rect: Recti) -> WideRectangle {
    WideRectangle::new(wide_integer(rect.x), wide_integer(rect.y), wide_integer(rect.width), wide_integer(rect.height))
}

/// Narrows only final integral virtual-layout results; all preceding cancellation stays exact.
fn narrow(value: WideScalar) -> i32 {
    clamp_i64_to_i32(value.to_bits() / 4096)
}

/// Retained integer rectangles are the public pixel representation, not a second arithmetic kernel.
impl RectExt for Recti {
    fn expanded(self, amount: i32) -> Self {
        let r = wide_rect(self);
        let amount = wide_integer(amount);
        let twice = amount.checked_add(amount).expect("two i32 extents fit the wide fixed domain");
        Self::new(
            narrow(r.x.checked_sub(amount).expect("i32 difference fits")),
            narrow(r.y.checked_sub(amount).expect("i32 difference fits")),
            narrow(r.width.checked_add(twice).expect("three i32 extents fit")),
            narrow(r.height.checked_add(twice).expect("three i32 extents fit")),
        )
    }

    fn translated(self, offset: Vec2i) -> Self {
        let origin = WidePoint::new(wide_integer(self.x), wide_integer(self.y));
        let offset = WidePoint::new(wide_integer(offset.x), wide_integer(offset.y));
        let p = WidePoint::checked_add_vv(&origin, &offset).expect("sum of i32 coordinates fits the wide fixed domain");
        Self::new(narrow(p.x), narrow(p.y), self.width, self.height)
    }

    fn relative_to(self, origin: Vec2i) -> Self {
        let p = WidePoint::checked_sub_vv(
            &WidePoint::new(wide_integer(self.x), wide_integer(self.y)),
            &WidePoint::new(wide_integer(origin.x), wide_integer(origin.y)),
        )
        .expect("difference of i32 coordinates fits the wide fixed domain");
        Self::new(narrow(p.x), narrow(p.y), self.width, self.height)
    }

    fn has_positive_area(self) -> bool {
        self.width > 0 && self.height > 0
    }

    fn contains_point(self, point: Vec2i) -> bool {
        if !self.has_positive_area() {
            return false;
        }
        let r = wide_rect(self);
        let max = r.checked_max().expect("i32 origin plus extent fits the wide fixed domain");
        let p = WidePoint::new(wide_integer(point.x), wide_integer(point.y));
        // UI hit regions are half-open; rs-math3d's inclusive checked_contains has a different contract.
        p.x >= r.x && p.y >= r.y && p.x < max.x && p.y < max.y
    }

    fn positive_intersection(self, other: Self) -> Option<Self> {
        if !self.has_positive_area() || !other.has_positive_area() {
            return None;
        }
        let r = wide_rect(self)
            .checked_intersect(&wide_rect(other))
            .expect("i32 rectangle edges fit the wide fixed domain")?;
        (r.width > WideScalar::ZERO && r.height > WideScalar::ZERO).then(|| Self::new(narrow(r.x), narrow(r.y), narrow(r.width), narrow(r.height)))
    }

    fn overlaps(self, other: Self) -> bool {
        self.positive_intersection(other).is_some()
    }

    fn union(self, other: Self) -> Self {
        let a = wide_rect(self);
        let b = wide_rect(other);
        let min = WidePoint::min(&a.min(), &b.min());
        let max = WidePoint::max(
            &a.checked_max().expect("i32 rectangle edges fit"),
            &b.checked_max().expect("i32 rectangle edges fit"),
        );
        let r = WideRectangle::checked_from(&min, &max).expect("union of i32 rectangles fits the wide fixed domain");
        Self::new(narrow(r.x), narrow(r.y), narrow(r.width), narrow(r.height))
    }
}

/// Convenience constructor for [`Vec2i`].
pub fn vec2(x: i32, y: i32) -> Vec2i {
    Vec2i { x, y }
}

/// Convenience constructor for [`Recti`].
pub fn rect(x: i32, y: i32, w: i32, h: i32) -> Recti {
    Recti { x, y, width: w, height: h }
}

/// Expands (or shrinks) a rectangle uniformly on all sides.
pub fn expand_rect(rectangle: Recti, amount: i32) -> Recti {
    rectangle.expanded(amount)
}

#[cfg(all(test, feature = "test-support"))]
mod tests {
    use super::*;

    fn assert_rect(actual: Recti, expected: (i32, i32, i32, i32)) {
        assert_eq!((actual.x, actual.y, actual.width, actual.height), expected);
    }

    #[test]
    fn rectangle_transforms_preserve_extents() {
        let source = Recti::new(10, 20, 30, 40);

        assert_rect(source.expanded(2), (8, 18, 34, 44));
        assert_rect(source.translated(Vec2i::new(-3, 5)), (7, 25, 30, 40));
        assert_rect(source.relative_to(Vec2i::new(3, 5)), (7, 15, 30, 40));
        assert_rect(
            Recti::new(i32::MAX - 1, i32::MIN + 1, 30, 40).translated(Vec2i::new(10, -10)),
            (i32::MAX, i32::MIN, 30, 40),
        );
    }

    #[test]
    fn rectangle_queries_require_positive_shared_area() {
        let left = Recti::new(0, 0, 10, 10);
        let overlapping = Recti::new(5, 4, 10, 3);
        let touching = Recti::new(10, 0, 4, 4);

        assert!(left.has_positive_area());
        assert!(!Recti::new(0, 0, 0, 10).has_positive_area());
        assert_rect(left.positive_intersection(overlapping).expect("rectangles overlap"), (5, 4, 5, 3));
        assert!(left.overlaps(overlapping));
        assert!(!left.overlaps(touching));
        assert!(left.positive_intersection(touching).is_none());
        assert_rect(left.union(overlapping), (0, 0, 15, 10));
        assert!(left.contains_point(Vec2i::new(9, 9)));
        assert!(!left.contains_point(Vec2i::new(10, 10)));
    }

    /// Verifies rectangle queries form exclusive edges in wider arithmetic at coordinate limits.
    #[test]
    fn rectangle_intersection_handles_exclusive_edges_beyond_i32() {
        let extreme = Recti::new(i32::MAX, i32::MIN, 1, 2);

        // An identical rectangle still intersects even though its mathematical right edge is one
        // greater than i32::MAX. A normal viewport remains disjoint without evaluating x + width
        // in the narrower coordinate type.
        assert_rect(
            extreme.positive_intersection(extreme).expect("identical extreme rectangles overlap"),
            (i32::MAX, i32::MIN, 1, 2),
        );
        assert!(extreme.positive_intersection(Recti::new(0, 0, 32, 32)).is_none());
        assert!(extreme.contains_point(Vec2i::new(i32::MAX, i32::MIN)));
    }
}
