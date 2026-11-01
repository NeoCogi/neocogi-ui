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
//! Stateless vertex construction, tessellation, bounds, translation, and final triangle clipping.

/// Sentinel clip rectangle used when command recording starts without a root clip.
pub static UNCLIPPED_RECT: Recti = Recti {
    x: 0,
    y: 0,
    width: i32::MAX,
    height: i32::MAX,
};
use rs_math3d::{Color4b, RealVector, Recti, Vec2f};
use std::ops::Range;

/// Floating-point tolerance shared by tessellation and clipping predicates.
pub const GEOM_EPS: f32 = 1.0e-5;
/// Squared tolerance used for duplicate-position checks.
const GEOM_EPS_SQ: f32 = GEOM_EPS * GEOM_EPS;

/// Texture-independent vertex used by solid geometry.
#[derive(Clone, Copy)]
pub struct SolidVertex {
    /// Screen-space position.
    pub position: Vec2f,
    /// Interpolated vertex color.
    pub color: Color4b,
}

/// Strongly typed, texture-independent solid triangle.
#[derive(Clone, Copy)]
pub struct SolidTriangle([SolidVertex; 3]);

impl SolidTriangle {
    /// Creates a triangle from three positions sharing one color.
    pub const fn new(positions: [Vec2f; 3], color: Color4b) -> Self {
        Self([
            SolidVertex { position: positions[0], color },
            SolidVertex { position: positions[1], color },
            SolidVertex { position: positions[2], color },
        ])
    }

    /// Returns the triangle's three vertices in winding order.
    pub const fn vertices(&self) -> &[SolidVertex; 3] {
        &self.0
    }

    /// Returns this triangle translated by a screen-space offset.
    fn translated(mut self, offset: Vec2f) -> Self {
        for vertex in &mut self.0 {
            vertex.position = vertex.position + offset;
        }
        self
    }
}

impl From<[SolidVertex; 3]> for SolidTriangle {
    /// Creates a triangle from three independently colored vertices.
    fn from(vertices: [SolidVertex; 3]) -> Self {
        Self(vertices)
    }
}

/// Validated range inside a [`SolidGeometry`] triangle arena.
pub struct SolidTriangleRange {
    /// Inclusive triangle index.
    start: usize,
    /// Exclusive triangle index.
    end: usize,
}

impl SolidTriangleRange {
    /// Creates a range only when at least one complete triangle was appended.
    fn new(start: usize, end: usize) -> Option<Self> {
        (start < end).then_some(Self { start, end })
    }

    /// Returns a standard range for read-only execution.
    pub fn as_range(&self) -> Range<usize> {
        self.start..self.end
    }

    /// Extends this range over an immediately adjacent range.
    pub fn extend(&mut self, next: &Self) -> bool {
        if self.end != next.start {
            return false;
        }
        self.end = next.end;
        true
    }
}

/// Retained solid-triangle arena with reusable polygon tessellation storage.
#[derive(Default)]
pub struct SolidGeometry {
    /// Screen-space triangles referenced by validated ranges.
    triangles: Vec<SolidTriangle>,
    /// Active polygon boundary reused by simplification and ear clipping.
    polygon_boundary: Vec<Vec2f>,
}

impl SolidGeometry {
    /// Creates empty solid geometry.
    pub const fn new() -> Self {
        Self {
            triangles: Vec::new(),
            polygon_boundary: Vec::new(),
        }
    }

    /// Clears retained and working geometry while preserving allocations.
    pub fn clear(&mut self) {
        self.triangles.clear();
        self.polygon_boundary.clear();
    }

    /// Returns whether no solid triangles are retained.
    #[cfg(any(test, feature = "test-support"))]
    pub fn is_empty(&self) -> bool {
        self.triangles.is_empty()
    }

    /// Returns all retained solid triangles.
    pub fn triangles(&self) -> &[SolidTriangle] {
        &self.triangles
    }

    /// Appends pre-tessellated triangles after applying a screen-space offset.
    pub fn append_triangles(&mut self, triangles: &[SolidTriangle], offset: Vec2f) -> Option<SolidTriangleRange> {
        let start = self.triangles.len();
        self.triangles.reserve(triangles.len());
        for triangle in triangles {
            self.triangles.push(triangle.translated(offset));
        }
        SolidTriangleRange::new(start, self.triangles.len())
    }

    /// Tessellates and appends one thick line.
    pub fn append_line(&mut self, from: Vec2f, to: Vec2f, width: f32, color: Color4b, offset: Vec2f) -> Option<SolidTriangleRange> {
        let triangles = Self::line_triangles(from, to, width, color)?;
        self.append_triangles(&triangles, offset)
    }

    /// Cleans, triangulates, and appends one simple polygon.
    ///
    /// Convex polygons use a triangle fan. Concave polygons use ear clipping against the reusable
    /// `polygon_boundary` workspace. Both paths append strongly typed triangles directly, so no
    /// flat triangle-position buffer escapes this object.
    pub fn append_polygon(&mut self, points: &[Vec2f], color: Color4b, offset: Vec2f) -> Option<SolidTriangleRange> {
        self.append_polygon_points(points.iter().copied(), color, offset)
    }

    /// Cleans, triangulates, and appends copied points from a typed boundary iterator.
    ///
    /// COM points can therefore enter the retained tessellation workspace directly instead of
    /// allocating a temporary vector merely to change between equivalent coordinate structs.
    pub fn append_polygon_points(&mut self, points: impl ExactSizeIterator<Item = Vec2f>, color: Color4b, offset: Vec2f) -> Option<SolidTriangleRange> {
        let start = self.triangles.len();
        if points.len() < 3 || !self.prepare_polygon(points) {
            return None;
        }

        let area = self.polygon_signed_area();
        if !area.is_finite() || area.abs() <= GEOM_EPS {
            self.polygon_boundary.clear();
            return None;
        }
        if area < 0.0 {
            self.polygon_boundary.reverse();
        }

        self.triangles.reserve(self.polygon_boundary.len() - 2);
        if self.polygon_is_convex_ccw() {
            for index in 1..self.polygon_boundary.len() - 1 {
                self.triangles.push(
                    SolidTriangle::new(
                        [self.polygon_boundary[0], self.polygon_boundary[index], self.polygon_boundary[index + 1]],
                        color,
                    )
                    .translated(offset),
                );
            }
            self.polygon_boundary.clear();
            return SolidTriangleRange::new(start, self.triangles.len());
        }

        // Ear clipping repeatedly removes one convex corner whose triangle contains no other
        // active polygon vertex. The reusable workspace therefore remains the active boundary
        // throughout the algorithm instead of doubling as an externally visible output buffer.
        while self.polygon_boundary.len() > 3 {
            let active_len = self.polygon_boundary.len();
            let mut ear = None;
            for index in 0..active_len {
                let previous = (index + active_len - 1) % active_len;
                let next = (index + 1) % active_len;
                let a = self.polygon_boundary[previous];
                let b = self.polygon_boundary[index];
                let c = self.polygon_boundary[next];
                if !is_convex_ccw(a, b, c) {
                    continue;
                }

                let contains_other = (0..active_len)
                    .any(|probe| probe != previous && probe != index && probe != next && point_in_triangle_ccw(self.polygon_boundary[probe], a, b, c));
                if !contains_other {
                    ear = Some((index, [a, b, c]));
                    break;
                }
            }

            let Some((index, triangle)) = ear else {
                // A malformed or numerically unstable boundary must not leave a partial polygon
                // in the retained triangle arena.
                self.triangles.truncate(start);
                self.polygon_boundary.clear();
                return None;
            };
            self.polygon_boundary.remove(index);
            self.triangles.push(SolidTriangle::new(triangle, color).translated(offset));
        }

        let final_triangle = [self.polygon_boundary[0], self.polygon_boundary[1], self.polygon_boundary[2]];
        self.triangles.push(SolidTriangle::new(final_triangle, color).translated(offset));
        self.polygon_boundary.clear();
        SolidTriangleRange::new(start, self.triangles.len())
    }

    /// Produces the two triangles forming one valid thick line.
    fn line_triangles(from: Vec2f, to: Vec2f, width: f32, color: Color4b) -> Option<[SolidTriangle; 2]> {
        if !width.is_finite() || width <= 0.0 || !point_is_finite(from) || !point_is_finite(to) {
            return None;
        }

        let delta = to - from;
        let len_sq = delta.length_squared();
        let (p0, p1, p2, p3) = if len_sq <= GEOM_EPS_SQ {
            let half = width * 0.5;
            (
                Vec2f::new(from.x - half, from.y - half),
                Vec2f::new(from.x + half, from.y - half),
                Vec2f::new(from.x + half, from.y + half),
                Vec2f::new(from.x - half, from.y + half),
            )
        } else {
            // inverse_length = 1 / sqrt(delta_x^2 + delta_y^2).
            let inv_len = len_sq.sqrt().recip();
            let normal = Vec2f::new(-delta.y * inv_len, delta.x * inv_len) * (width * 0.5);
            (from + normal, to + normal, to - normal, from - normal)
        };
        Some([SolidTriangle::new([p0, p1, p2], color), SolidTriangle::new([p0, p2, p3], color)])
    }

    /// Copies a finite polygon into the reusable workspace and removes redundant vertices.
    fn prepare_polygon(&mut self, points: impl ExactSizeIterator<Item = Vec2f>) -> bool {
        self.polygon_boundary.clear();
        // The second pass appends a simplified boundary behind the copied input, so reserve both
        // spans up front and avoid a growth allocation halfway through tessellation.
        // workspace_capacity = copied_point_count + simplified_point_count = point_count * 2.
        let workspace_capacity = points.len().saturating_mul(2);
        self.polygon_boundary.reserve(workspace_capacity);
        for point in points {
            if !point_is_finite(point) {
                self.polygon_boundary.clear();
                return false;
            }
            if self
                .polygon_boundary
                .last()
                .map(|previous| (*previous - point).length_squared() > GEOM_EPS_SQ)
                .unwrap_or(true)
            {
                self.polygon_boundary.push(point);
            }
        }

        if self.polygon_boundary.len() > 1
            && (self.polygon_boundary[0] - *self.polygon_boundary.last().expect("non-empty polygon")).length_squared() <= GEOM_EPS_SQ
        {
            self.polygon_boundary.pop();
        }
        if self.polygon_boundary.len() < 3 {
            self.polygon_boundary.clear();
            return false;
        }

        // Append the simplified boundary after the copied boundary, then discard the original
        // prefix. This reuses one allocation without requiring a second temporary vector.
        let copied_len = self.polygon_boundary.len();
        for index in 0..copied_len {
            let previous = self.polygon_boundary[(index + copied_len - 1) % copied_len];
            let current = self.polygon_boundary[index];
            let next = self.polygon_boundary[(index + 1) % copied_len];
            if (previous - current).length_squared() <= GEOM_EPS_SQ || (current - next).length_squared() <= GEOM_EPS_SQ {
                continue;
            }
            if cross2(current - previous, next - current).abs() <= GEOM_EPS {
                continue;
            }
            self.polygon_boundary.push(current);
        }
        self.polygon_boundary.drain(..copied_len);
        if self.polygon_boundary.len() < 3 {
            self.polygon_boundary.clear();
            return false;
        }
        true
    }

    /// Computes winding and signed area for the prepared polygon boundary.
    fn polygon_signed_area(&self) -> f32 {
        let mut area = 0.0;
        for index in 0..self.polygon_boundary.len() {
            let current = self.polygon_boundary[index];
            let next = self.polygon_boundary[(index + 1) % self.polygon_boundary.len()];
            area += current.x * next.y - next.x * current.y;
        }
        area * 0.5
    }

    /// Returns whether every prepared polygon corner is convex and counter-clockwise.
    fn polygon_is_convex_ccw(&self) -> bool {
        (0..self.polygon_boundary.len()).all(|index| {
            is_convex_ccw(
                self.polygon_boundary[(index + self.polygon_boundary.len() - 1) % self.polygon_boundary.len()],
                self.polygon_boundary[index],
                self.polygon_boundary[(index + 1) % self.polygon_boundary.len()],
            )
        })
    }

    #[cfg(test)]
    /// Returns retained triangle allocation capacity.
    pub fn triangle_capacity(&self) -> usize {
        self.triangles.capacity()
    }

    #[cfg(test)]
    /// Returns polygon workspace allocation capacity.
    pub fn polygon_capacity(&self) -> usize {
        self.polygon_boundary.capacity()
    }

    #[cfg(test)]
    /// Reserves polygon workspace for allocation-reuse tests.
    pub fn reserve_polygon_capacity(&mut self, additional: usize) {
        self.polygon_boundary.reserve(additional);
    }
}

/// Returns whether both point components are finite.
fn point_is_finite(point: Vec2f) -> bool {
    point.x.is_finite() && point.y.is_finite()
}

/// Returns the signed 2D cross product.
fn cross2(left: Vec2f, right: Vec2f) -> f32 {
    left.x * right.y - left.y * right.x
}

/// Returns whether three points form a convex counter-clockwise corner.
fn is_convex_ccw(previous: Vec2f, current: Vec2f, next: Vec2f) -> bool {
    cross2(current - previous, next - current) > GEOM_EPS
}

/// Returns whether a point lies inside or on a counter-clockwise triangle.
fn point_in_triangle_ccw(point: Vec2f, a: Vec2f, b: Vec2f, c: Vec2f) -> bool {
    let ab = cross2(b - a, point - a);
    let bc = cross2(c - b, point - b);
    let ca = cross2(a - c, point - c);
    ab >= -GEOM_EPS && bc >= -GEOM_EPS && ca >= -GEOM_EPS
}

#[cfg(test)]
mod tests {
    use super::*;
    use rs_math3d::color4b;

    fn white() -> Color4b {
        color4b(255, 255, 255, 255)
    }

    fn test_polygon(points: &[Vec2f]) -> Vec<SolidTriangle> {
        let mut geometry = SolidGeometry::new();
        geometry.append_polygon(points, white(), Vec2f::new(0.0, 0.0));
        geometry.triangles().to_vec()
    }

    #[test]
    fn convex_polygon_uses_triangle_fan() {
        let triangles = test_polygon(&[Vec2f::new(0.0, 0.0), Vec2f::new(10.0, 0.0), Vec2f::new(10.0, 10.0), Vec2f::new(0.0, 10.0)]);
        assert_eq!(triangles.len(), 2);
    }

    #[test]
    fn concave_polygon_uses_ear_clipping() {
        let triangles = test_polygon(&[
            Vec2f::new(0.0, 0.0),
            Vec2f::new(10.0, 0.0),
            Vec2f::new(5.0, 5.0),
            Vec2f::new(10.0, 10.0),
            Vec2f::new(0.0, 10.0),
        ]);
        assert_eq!(triangles.len(), 3);
    }

    #[test]
    fn duplicate_and_collinear_points_are_simplified() {
        let triangles = test_polygon(&[
            Vec2f::new(0.0, 0.0),
            Vec2f::new(5.0, 0.0),
            Vec2f::new(10.0, 0.0),
            Vec2f::new(10.0, 10.0),
            Vec2f::new(0.0, 10.0),
            Vec2f::new(0.0, 0.0),
            Vec2f::new(0.0, 0.0),
        ]);
        assert_eq!(triangles.len(), 2);
    }

    #[test]
    fn clockwise_polygon_is_normalized() {
        let triangles = test_polygon(&[Vec2f::new(0.0, 0.0), Vec2f::new(0.0, 10.0), Vec2f::new(10.0, 10.0), Vec2f::new(10.0, 0.0)]);
        assert_eq!(triangles.len(), 2);
    }

    #[test]
    fn degenerate_and_non_finite_polygons_emit_nothing() {
        for points in [
            vec![Vec2f::new(0.0, 0.0), Vec2f::new(1.0, 1.0), Vec2f::new(2.0, 2.0)],
            vec![Vec2f::new(0.0, 0.0), Vec2f::new(f32::NAN, 1.0), Vec2f::new(2.0, 0.0)],
            vec![Vec2f::new(0.0, 0.0), Vec2f::new(1.0, 0.0)],
        ] {
            let triangles = test_polygon(&points);
            assert!(triangles.is_empty());
        }
    }

    #[test]
    fn polygon_triangulation_reuses_solid_geometry_workspace() {
        let mut geometry = SolidGeometry::new();
        geometry.append_polygon(
            &[Vec2f::new(0.0, 0.0), Vec2f::new(10.0, 0.0), Vec2f::new(10.0, 10.0), Vec2f::new(0.0, 10.0)],
            white(),
            Vec2f::new(0.0, 0.0),
        );
        let capacity = geometry.polygon_capacity();
        assert_eq!(geometry.triangles().len(), 2);

        geometry.clear();
        geometry.append_polygon(
            &[Vec2f::new(0.0, 0.0), Vec2f::new(5.0, 0.0), Vec2f::new(0.0, 5.0)],
            white(),
            Vec2f::new(0.0, 0.0),
        );
        assert_eq!(geometry.triangles().len(), 1);
        assert_eq!(geometry.polygon_capacity(), capacity);
    }

    #[test]
    fn solid_geometry_owns_translation_ranges_and_polygon_scratch() {
        let mut geometry = SolidGeometry::new();
        let polygon = geometry
            .append_polygon(
                &[Vec2f::new(0.0, 0.0), Vec2f::new(10.0, 0.0), Vec2f::new(10.0, 10.0), Vec2f::new(0.0, 10.0)],
                white(),
                Vec2f::new(5.0, 7.0),
            )
            .expect("valid polygon");
        let line = geometry
            .append_line(Vec2f::new(0.0, 20.0), Vec2f::new(10.0, 20.0), 2.0, white(), Vec2f::new(5.0, 7.0))
            .expect("valid line");

        assert_eq!(polygon.as_range(), 0..2);
        assert_eq!(line.as_range(), 2..4);
        assert!(
            geometry
                .triangles()
                .iter()
                .flat_map(|triangle| triangle.vertices())
                .all(|vertex| vertex.position.x >= 5.0 && vertex.position.y >= 7.0)
        );

        let triangle_capacity = geometry.triangle_capacity();
        let polygon_capacity = geometry.polygon_capacity();
        geometry.clear();
        assert!(geometry.is_empty());
        assert_eq!(geometry.triangle_capacity(), triangle_capacity);
        assert_eq!(geometry.polygon_capacity(), polygon_capacity);
    }

    #[test]
    fn thick_and_zero_length_lines_emit_complete_quads() {
        for (from, to) in [(Vec2f::new(0.0, 0.0), Vec2f::new(10.0, 0.0)), (Vec2f::new(5.0, 5.0), Vec2f::new(5.0, 5.0))] {
            let triangles = SolidGeometry::line_triangles(from, to, 2.0, white()).expect("valid line");
            assert_eq!(triangles.len(), 2);
            assert!(triangles.iter().all(|triangle| triangle.vertices().len() == 3));
        }
    }
}
