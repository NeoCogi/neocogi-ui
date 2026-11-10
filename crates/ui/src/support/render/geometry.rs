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
use neocogi_com::{NcStatus, NC_E_INVALID_ARG, NC_E_OUT_OF_RANGE};
use neocogi_ui::geometry::{self, WidePoint, WideScalar};
use rs_math3d::{Color4b, RealVector, Recti, Scalar, Vec2f, Vector};
use std::ops::Range;

/// Final float vertex produced after checked fixed-point UI geometry calculations.
#[derive(Clone, Copy)]
pub struct SolidVertex {
    /// Bounded screen-space rendering position.
    pub position: Vec2f,
    /// Interpolated vertex color.
    pub color: Color4b,
}

/// Texture-independent triangle ready for the existing floating-point renderer.
#[derive(Clone, Copy)]
pub struct SolidTriangle([SolidVertex; 3]);

impl SolidTriangle {
    /// Creates a final rendering triangle. Admission validates externally supplied vertices.
    pub const fn new(positions: [Vec2f; 3], color: Color4b) -> Self {
        Self([
            SolidVertex { position: positions[0], color },
            SolidVertex { position: positions[1], color },
            SolidVertex { position: positions[2], color },
        ])
    }

    /// Returns the three rendering vertices in winding order.
    pub const fn vertices(&self) -> &[SolidVertex; 3] {
        &self.0
    }
}

impl From<[SolidVertex; 3]> for SolidTriangle {
    fn from(vertices: [SolidVertex; 3]) -> Self {
        Self(vertices)
    }
}

/// Nonempty range into the retained final-triangle arena.
pub struct SolidTriangleRange {
    /// First included triangle.
    start: usize,
    /// First excluded triangle.
    end: usize,
}

impl SolidTriangleRange {
    /// Empty or degenerate shapes do not create a retained operation.
    fn new(start: usize, end: usize) -> Option<Self> {
        (start < end).then_some(Self { start, end })
    }

    /// Exposes an immutable arena range to the final executor.
    pub fn as_range(&self) -> Range<usize> {
        self.start..self.end
    }

    /// Coalesces adjacent ranges without crossing operation/clip boundaries.
    pub fn extend(&mut self, next: &Self) -> bool {
        if self.end != next.start {
            return false;
        }
        self.end = next.end;
        true
    }
}

/// One arena of final float triangles and one reusable fixed-point polygon workspace.
#[derive(Default)]
pub struct SolidGeometry {
    /// Final vertices, never used as authoritative inputs for geometric predicates.
    triangles: Vec<SolidTriangle>,
    /// Checked wide fixed coordinates retain small areas at large accepted origins.
    polygon_boundary: Vec<WidePoint>,
}

impl SolidGeometry {
    /// Constructs empty reusable storage.
    pub const fn new() -> Self {
        Self {
            triangles: Vec::new(),
            polygon_boundary: Vec::new(),
        }
    }

    /// Releases retained contents while preserving frame-to-frame capacity.
    pub fn clear(&mut self) {
        self.triangles.clear();
        self.polygon_boundary.clear();
    }

    /// Observes actual retained output for focused regression fixtures.
    #[cfg(feature = "test-support")]
    pub fn is_empty(&self) -> bool {
        self.triangles.is_empty()
    }

    /// Borrows the final renderer-compatible triangle arena.
    pub fn triangles(&self) -> &[SolidTriangle] {
        &self.triangles
    }

    /// Checks a complete external batch before mutating the arena, then quantizes positions once.
    pub fn append_triangles(&mut self, triangles: &[SolidTriangle], offset: Vec2f) -> Result<Option<SolidTriangleRange>, NcStatus> {
        let offset = geometry::wide_fractional_point(offset)?;
        for triangle in triangles {
            for vertex in triangle.vertices() {
                translated(geometry::wide_fractional_point(vertex.position)?, offset)?;
            }
        }
        let start = self.triangles.len();
        self.triangles.reserve(triangles.len());
        for triangle in triangles {
            let mut result = *triangle;
            for vertex in &mut result.0 {
                vertex.position = translated(geometry::wide_fractional_point(vertex.position)?, offset)?;
            }
            self.triangles.push(result);
        }
        Ok(SolidTriangleRange::new(start, self.triangles.len()))
    }

    /// Normalizes and expands one line entirely in checked fixed-point math.
    pub fn append_line(&mut self, from: Vec2f, to: Vec2f, width: f32, color: Color4b, offset: Vec2f) -> Result<Option<SolidTriangleRange>, NcStatus> {
        let Some(points) = Self::line_points(from, to, width)? else {
            return Ok(None);
        };
        let offset = geometry::wide_fractional_point(offset)?;
        // Validate all expanded/transformed positions before publishing either triangle.
        let [a, b, c, d] = [
            translated(points[0], offset)?,
            translated(points[1], offset)?,
            translated(points[2], offset)?,
            translated(points[3], offset)?,
        ];
        let start = self.triangles.len();
        self.triangles
            .extend([SolidTriangle::new([a, b, c], color), SolidTriangle::new([a, c, d], color)]);
        Ok(SolidTriangleRange::new(start, self.triangles.len()))
    }

    /// Admits finite, bounded fractional points before shared fixed-point polygon tessellation.
    pub fn append_polygon(&mut self, points: &[Vec2f], color: Color4b, offset: Vec2f) -> Result<Option<SolidTriangleRange>, NcStatus> {
        self.append_polygon_points(points.iter().copied(), color, offset)
    }

    /// Reuses the same workspace for native and copied COM inputs without a second point vector.
    pub fn append_polygon_points(
        &mut self,
        points: impl ExactSizeIterator<Item = Vec2f>,
        color: Color4b,
        offset: Vec2f,
    ) -> Result<Option<SolidTriangleRange>, NcStatus> {
        if points.len() > neocogi_platform_contracts::render::abi::MAX_POLYGON_POINTS {
            return Err(NC_E_OUT_OF_RANGE);
        }
        let offset = geometry::wide_fractional_point(offset)?;
        let start = self.triangles.len();
        let result = self.triangulate(points, color, offset);
        if !matches!(result, Ok(Some(_))) {
            self.triangles.truncate(start);
        }
        self.polygon_boundary.clear();
        result
    }

    /// Emits one complete polygon, rolling back externally at every failure/degenerate boundary.
    fn triangulate(&mut self, points: impl ExactSizeIterator<Item = Vec2f>, color: Color4b, offset: WidePoint) -> Result<Option<SolidTriangleRange>, NcStatus> {
        let start = self.triangles.len();
        if !self.prepare_polygon(points)? {
            return Ok(None);
        }
        // Translation cannot invalidate a partially published polygon: preflight the whole boundary.
        for &point in &self.polygon_boundary {
            translated(point, offset)?;
        }
        let area = self.polygon_signed_area()?;
        if area == WideScalar::ZERO {
            return Ok(None);
        }
        if area < WideScalar::ZERO {
            self.polygon_boundary.reverse();
        }
        self.triangles.reserve(self.polygon_boundary.len() - 2);
        if self.polygon_is_convex_ccw()? {
            for index in 1..self.polygon_boundary.len() - 1 {
                self.triangles.push(fixed_triangle(
                    [self.polygon_boundary[0], self.polygon_boundary[index], self.polygon_boundary[index + 1]],
                    color,
                    offset,
                )?);
            }
            return Ok(SolidTriangleRange::new(start, self.triangles.len()));
        }
        while self.polygon_boundary.len() > 3 {
            let len = self.polygon_boundary.len();
            let mut ear = None;
            for index in 0..len {
                let previous = (index + len - 1) % len;
                let next = (index + 1) % len;
                let [a, b, c] = [self.polygon_boundary[previous], self.polygon_boundary[index], self.polygon_boundary[next]];
                let Some(candidate) = EarTriangle::new([a, b, c])? else {
                    continue;
                };
                let mut contains = false;
                for (probe, &point) in self.polygon_boundary.iter().enumerate() {
                    if probe != previous && probe != index && probe != next && candidate.contains(point)? {
                        contains = true;
                        break;
                    }
                }
                if !contains {
                    ear = Some((index, [a, b, c]));
                    break;
                }
            }
            let Some((index, triangle)) = ear else {
                return Ok(None);
            };
            self.polygon_boundary.remove(index);
            self.triangles.push(fixed_triangle(triangle, color, offset)?);
        }
        self.triangles.push(fixed_triangle(
            [self.polygon_boundary[0], self.polygon_boundary[1], self.polygon_boundary[2]],
            color,
            offset,
        )?);
        Ok(SolidTriangleRange::new(start, self.triangles.len()))
    }

    /// Expands admitted line endpoints with a scale-aware checked rs-math3d normalization.
    fn line_points(from: Vec2f, to: Vec2f, width: f32) -> Result<Option<[WidePoint; 4]>, NcStatus> {
        let from = geometry::wide_fractional_point(from)?;
        let to = geometry::wide_fractional_point(to)?;
        let width = geometry::fractional_extent(width)?;
        if width < geometry::Scalar::ZERO {
            return Err(NC_E_INVALID_ARG);
        }
        if width == geometry::Scalar::ZERO {
            return Ok(None);
        }
        let half = width
            .try_rescale::<i64, 12>()
            .map_err(geometry::status)?
            .checked_div(WideScalar::from_bits(8192))
            .map_err(geometry::status)?;
        let delta = WidePoint::checked_sub_vv(&to, &from).map_err(geometry::status)?;
        let normal = if delta.x == WideScalar::ZERO && delta.y == WideScalar::ZERO {
            let a = WidePoint::new(half.checked_neg().map_err(geometry::status)?, half.checked_neg().map_err(geometry::status)?);
            let b = WidePoint::new(half, half.checked_neg().map_err(geometry::status)?);
            let c = WidePoint::new(half, half);
            let d = WidePoint::new(half.checked_neg().map_err(geometry::status)?, half);
            return Ok(Some([add(from, a)?, add(from, b)?, add(from, c)?, add(from, d)?]));
        } else {
            let unit = delta.checked_normalize(WideScalar::DELTA).map_err(geometry::status)?;
            WidePoint::checked_mul_vs(&WidePoint::new(unit.y.checked_neg().map_err(geometry::status)?, unit.x), half).map_err(geometry::status)?
        };
        Ok(Some([add(from, normal)?, add(to, normal)?, sub(to, normal)?, sub(from, normal)?]))
    }

    /// Copies/quantizes once, removes exact duplicate and collinear points, and reuses one allocation.
    fn prepare_polygon(&mut self, points: impl ExactSizeIterator<Item = Vec2f>) -> Result<bool, NcStatus> {
        self.polygon_boundary.clear();
        self.polygon_boundary.reserve(points.len() * 2);
        for point in points {
            let point = geometry::wide_fractional_point(point)?;
            if self.polygon_boundary.last().is_none_or(|p| !same_point(*p, point)) {
                self.polygon_boundary.push(point);
            }
        }
        if self.polygon_boundary.len() > 1 && same_point(self.polygon_boundary[0], *self.polygon_boundary.last().expect("nonempty boundary")) {
            self.polygon_boundary.pop();
        }
        if self.polygon_boundary.len() < 3 {
            return Ok(false);
        }
        let copied = self.polygon_boundary.len();
        for index in 0..copied {
            let previous = self.polygon_boundary[(index + copied - 1) % copied];
            let current = self.polygon_boundary[index];
            let next = self.polygon_boundary[(index + 1) % copied];
            if same_point(current, previous) || same_point(current, next) || cross2(sub(current, previous)?, sub(next, current)?)? == WideScalar::ZERO {
                continue;
            }
            self.polygon_boundary.push(current);
        }
        self.polygon_boundary.drain(..copied);
        Ok(self.polygon_boundary.len() >= 3)
    }

    /// Accumulates exact products on the admitted grid; no float winding cancellation is possible.
    fn polygon_signed_area(&self) -> Result<WideScalar, NcStatus> {
        let mut area = WideScalar::ZERO;
        for index in 0..self.polygon_boundary.len() {
            area = area
                .checked_add(cross2(
                    self.polygon_boundary[index],
                    self.polygon_boundary[(index + 1) % self.polygon_boundary.len()],
                )?)
                .map_err(geometry::status)?;
        }
        Ok(area)
    }

    /// Checks each corner against the exact fixed-point orientation predicate.
    fn polygon_is_convex_ccw(&self) -> Result<bool, NcStatus> {
        let len = self.polygon_boundary.len();
        for index in 0..len {
            if !is_convex_ccw(
                self.polygon_boundary[(index + len - 1) % len],
                self.polygon_boundary[index],
                self.polygon_boundary[(index + 1) % len],
            )? {
                return Ok(false);
            }
        }
        Ok(true)
    }

    /// Existing arena reservation avoids allocation during publication of a preflighted batch.
    pub fn reserve_triangles(&mut self, additional: usize) {
        self.triangles.reserve(additional);
    }
    /// Observes retained capacity for allocation reuse tests.
    pub fn triangle_capacity(&self) -> usize {
        self.triangles.capacity()
    }
    /// Observes polygon workspace capacity without exposing its representation.
    pub fn polygon_capacity(&self) -> usize {
        self.polygon_boundary.capacity()
    }
    #[cfg(all(test, feature = "test-support"))]
    /// Makes allocation reuse explicit in focused tests.
    pub fn reserve_polygon_capacity(&mut self, additional: usize) {
        self.polygon_boundary.reserve(additional);
    }
}

/// Vector components have exact fixed equality; rs-math3d vectors intentionally omit PartialEq.
fn same_point(a: WidePoint, b: WidePoint) -> bool {
    a.x == b.x && a.y == b.y
}
/// Adds working vectors with the library's checked arithmetic.
#[inline(always)]
fn add(a: WidePoint, b: WidePoint) -> Result<WidePoint, NcStatus> {
    WidePoint::checked_add_vv(&a, &b).map_err(geometry::status)
}
/// Subtracts before any narrowing or float conversion.
#[inline(always)]
fn sub(a: WidePoint, b: WidePoint) -> Result<WidePoint, NcStatus> {
    WidePoint::checked_sub_vv(&a, &b).map_err(geometry::status)
}
/// Applies a checked translation and converts only the final admitted rendering coordinate.
fn translated(point: WidePoint, offset: WidePoint) -> Result<Vec2f, NcStatus> {
    geometry::render_point(add(point, offset)?)
}
/// Builds rendering vertices only after fixed geometry and placement are complete.
fn fixed_triangle(points: [WidePoint; 3], color: Color4b, offset: WidePoint) -> Result<SolidTriangle, NcStatus> {
    Ok(SolidTriangle::new(
        [translated(points[0], offset)?, translated(points[1], offset)?, translated(points[2], offset)?],
        color,
    ))
}
/// Computes only the 2D determinant, with one exact accumulation and rounding.
/// A 3D cross product also evaluates two unused zero components under min-size.
fn cross2(a: WidePoint, b: WidePoint) -> Result<WideScalar, NcStatus> {
    <WideScalar as Scalar>::checked_sum_products(&[(a.x, b.y, false), (a.y, b.x, true)]).map_err(geometry::status)
}
/// Exact positive winding after duplicate/collinear cleanup.
fn is_convex_ccw(a: WidePoint, b: WidePoint, c: WidePoint) -> Result<bool, NcStatus> {
    Ok(cross2(sub(b, a)?, sub(c, b)?)? > WideScalar::ZERO)
}
/// One positively wound ear candidate reuses exact edges across every boundary-point probe.
struct EarTriangle {
    /// Local fixed points remain authoritative; float vertices are never geometric predicates.
    vertices: [WidePoint; 3],
    /// Checked directed edges in the same order as vertices.
    edges: [WidePoint; 3],
    /// Closed component bounds cheaply reject probes that cannot lie in the triangle.
    minimum: WidePoint,
    /// Closed upper component bounds retain probes on an edge or vertex.
    maximum: WidePoint,
}
impl EarTriangle {
    /// Rejects nonconvex/degenerate corners before constructing the remaining reusable edge.
    fn new(vertices: [WidePoint; 3]) -> Result<Option<Self>, NcStatus> {
        let [a, b, c] = vertices;
        let ab = sub(b, a)?;
        let bc = sub(c, b)?;
        if cross2(ab, bc)? <= WideScalar::ZERO {
            return Ok(None);
        }
        Ok(Some(Self {
            vertices,
            edges: [ab, bc, sub(a, c)?],
            minimum: WidePoint::new(a.x.min(b.x).min(c.x), a.y.min(b.y).min(c.y)),
            maximum: WidePoint::new(a.x.max(b.x).max(c.x), a.y.max(b.y).max(c.y)),
        }))
    }

    /// Includes the closed boundary and uses the shared checked determinant after coarse rejection.
    fn contains(&self, point: WidePoint) -> Result<bool, NcStatus> {
        if point.x < self.minimum.x || point.x > self.maximum.x || point.y < self.minimum.y || point.y > self.maximum.y {
            return Ok(false);
        }
        for index in 0..3 {
            if cross2(self.edges[index], sub(point, self.vertices[index])?)? < WideScalar::ZERO {
                return Ok(false);
            }
        }
        Ok(true)
    }
}

#[cfg(all(test, feature = "test-support"))]
mod tests {
    use super::*;
    use rs_math3d::color4b;

    fn white() -> Color4b {
        color4b(255, 255, 255, 255)
    }

    /// Orientation retains exact cancellation, subpixel areas and checked overflow.
    #[test]
    fn determinant_preserves_cancellation_and_rejects_final_overflow() {
        let largest = WideScalar::from_bits(i64::MAX);
        let diagonal = WidePoint::new(largest, largest);
        assert_eq!(cross2(diagonal, diagonal), Ok(WideScalar::ZERO));
        let side = WideScalar::try_from_int(262_144).unwrap();
        let zero = WideScalar::ZERO;
        assert_eq!(
            cross2(WidePoint::new(side, zero), WidePoint::new(zero, side)),
            Ok(WideScalar::try_from_int(1_i64 << 36).unwrap())
        );
        assert_eq!(
            cross2(
                WidePoint::new(WideScalar::DELTA, zero),
                WidePoint::new(zero, WideScalar::try_from_int(1).unwrap())
            ),
            Ok(WideScalar::DELTA)
        );
        assert_eq!(cross2(WidePoint::new(largest, zero), WidePoint::new(zero, largest)), Err(NC_E_OUT_OF_RANGE));
    }

    /// A coarse bound must preserve closed edges and still reject points outside the diagonal.
    #[test]
    fn ear_candidate_contains_closed_edges_and_large_origin_subpixels() {
        let point = |x, y| geometry::wide_fractional_point(Vec2f::new(x, y)).unwrap();
        for origin in [0., 131_000.] {
            let triangle = EarTriangle::new([point(origin, origin), point(origin + 0.5, origin), point(origin, origin + 0.5)])
                .unwrap()
                .unwrap();
            for (x, y) in [(0., 0.), (0.5, 0.), (0., 0.5), (0.25, 0.), (0.25, 0.25), (0.125, 0.125)] {
                assert!(triangle.contains(point(origin + x, origin + y)).unwrap());
            }
            for (x, y) in [(-0.125, 0.), (0.625, 0.), (0., 0.625), (0.375, 0.375)] {
                assert!(!triangle.contains(point(origin + x, origin + y)).unwrap());
            }
        }
    }

    fn test_polygon(points: &[Vec2f]) -> Vec<SolidTriangle> {
        let mut geometry = SolidGeometry::new();
        geometry.append_polygon(points, white(), Vec2f::new(0.0, 0.0)).unwrap();
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
    fn degenerate_polygons_emit_nothing() {
        for points in [
            vec![Vec2f::new(0.0, 0.0), Vec2f::new(1.0, 1.0), Vec2f::new(2.0, 2.0)],
            vec![Vec2f::new(0.0, 0.0), Vec2f::new(1.0, 0.0)],
        ] {
            let triangles = test_polygon(&points);
            assert!(triangles.is_empty());
        }
    }

    #[test]
    fn polygon_triangulation_reuses_solid_geometry_workspace() {
        let mut geometry = SolidGeometry::new();
        geometry
            .append_polygon(
                &[Vec2f::new(0.0, 0.0), Vec2f::new(10.0, 0.0), Vec2f::new(10.0, 10.0), Vec2f::new(0.0, 10.0)],
                white(),
                Vec2f::new(0.0, 0.0),
            )
            .unwrap();
        let capacity = geometry.polygon_capacity();
        assert_eq!(geometry.triangles().len(), 2);

        geometry.clear();
        geometry
            .append_polygon(
                &[Vec2f::new(0.0, 0.0), Vec2f::new(5.0, 0.0), Vec2f::new(0.0, 5.0)],
                white(),
                Vec2f::new(0.0, 0.0),
            )
            .unwrap();
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
            .expect("valid polygon")
            .expect("nondegenerate polygon");
        let line = geometry
            .append_line(Vec2f::new(0.0, 20.0), Vec2f::new(10.0, 20.0), 2.0, white(), Vec2f::new(5.0, 7.0))
            .expect("valid line")
            .expect("nonzero line");

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
            let mut geometry = SolidGeometry::new();
            geometry.append_line(from, to, 2.0, white(), Vec2f::new(0.0, 0.0)).unwrap().unwrap();
            assert_eq!(geometry.triangles().len(), 2);
            assert!(geometry.triangles().iter().all(|triangle| triangle.vertices().len() == 3));
        }
    }

    /// A valid large-origin half-pixel square must keep its nonzero rendered area.
    #[test]
    fn fixed_winding_preserves_small_geometry_at_large_origins() {
        let triangles = test_polygon(&[
            Vec2f::new(131_000.0, 131_000.0),
            Vec2f::new(131_000.5, 131_000.0),
            Vec2f::new(131_000.5, 131_000.5),
            Vec2f::new(131_000.0, 131_000.5),
        ]);
        assert_eq!(triangles.len(), 2);
        let mut area = 0.0_f64;
        for t in triangles {
            let [a, b, c] = t.vertices().map(|v| v.position);
            area += ((f64::from(b.x) - f64::from(a.x)) * (f64::from(c.y) - f64::from(a.y))
                - (f64::from(b.y) - f64::from(a.y)) * (f64::from(c.x) - f64::from(a.x)))
            .abs();
        }
        assert_eq!(area, 0.5);
    }

    /// Both old counterexamples now fail admission explicitly rather than disappearing successfully.
    #[test]
    fn out_of_domain_geometry_and_stroke_expansion_reject_atomically() {
        let mut geometry = SolidGeometry::new();
        for x in [i32::MIN as f32, 1e20, f32::MAX] {
            assert!(matches!(
                geometry.append_line(Vec2f::new(x, 10.0), Vec2f::new(1000.0, 10.0), 2.0, white(), Vec2f::new(0.0, 0.0)),
                Err(NC_E_OUT_OF_RANGE)
            ));
            assert!(geometry.is_empty());
        }
        assert!(matches!(
            geometry.append_line(Vec2f::new(131_072.0, 10.0), Vec2f::new(131_072.0, 20.0), 2.0, white(), Vec2f::new(0.0, 0.0)),
            Err(NC_E_OUT_OF_RANGE)
        ));
        assert!(matches!(
            geometry.append_polygon(
                &[Vec2f::new(0.0, 0.0), Vec2f::new(f32::NAN, 0.0), Vec2f::new(1.0, 1.0)],
                white(),
                Vec2f::new(0.0, 0.0)
            ),
            Err(NC_E_INVALID_ARG)
        ));
        assert!(geometry.is_empty());
        geometry
            .append_line(Vec2f::new(-131_000.0, 10.0), Vec2f::new(131_000.0, 10.0), 1.5, white(), Vec2f::new(0.0, 0.0))
            .unwrap()
            .unwrap();
        let ys: Vec<_> = geometry.triangles().iter().flat_map(|t| t.vertices()).map(|v| v.position.y).collect();
        assert_eq!(
            ys.iter().copied().fold(f32::NEG_INFINITY, f32::max) - ys.iter().copied().fold(f32::INFINITY, f32::min),
            1.5
        );
    }
}
