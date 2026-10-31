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
//! Interactive perspective grid keeps its tested homogeneous clipping and emits solid COM geometry.
use super::{graphics::*, camera::Camera, view3d::View3D};
use std::{cell::RefCell, rc::Rc};

/// Half width of the finite ground grid, in world units.
const GRID_HALF_EXTENT: i32 = 20;
/// Every fifth division remains visible when minor lines are disabled.
const GRID_MAJOR_INTERVAL: i32 = 5;
/// Closest useful camera distance stays outside the grid bounding sphere.
const GRID_MIN_CAMERA_DISTANCE: f32 = 32.;
/// Far limit keeps the finite grid legible.
const GRID_MAX_CAMERA_DISTANCE: f32 = 120.;
/// Native/browser wheel units map to the same camera distance.
const GRID_WHEEL_DISTANCE_SCALE: f32 = 0.1;

/// Explicit shared camera and display policy used by the widget and menu actions.
pub(super) struct GridState {
    /// Camera and cached matrices; never held borrowed across COM calls.
    pub(super) view: View3D,
    /// Grid-density toggle changes drawing only.
    pub(super) minor_lines: bool,
    /// Packed background color edited by the application sliders; geometry stays fixed.
    pub(super) background: u32,
}
impl Default for GridState {
    /// Starts at the original demo's oblique viewing angle.
    fn default() -> Self {
        Self {
            view: create_grid_view_3d(),
            minor_lines: true,
            background: u32::from_le_bytes([90, 95, 100, 255]),
        }
    }
}
/// One local COM leaf owns an explicit shared application model.
pub(super) struct GridWidget {
    /// Menus reset this same camera, avoiding a rendering-state mirror.
    data: Rc<RefCell<GridState>>,
    /// Paint-only workspace is detached before a foreign call and restored even on a returned error.
    vertices: std::cell::Cell<Vec<ColoredVertex>>,
}
impl GridWidget {
    /// Binds the shared camera model and starts one reusable geometry buffer for this leaf.
    pub(super) fn new(data: Rc<RefCell<GridState>>) -> Self {
        Self {
            data,
            vertices: std::cell::Cell::new(Vec::new()),
        }
    }
}
impl LeafContent for GridWidget {
    /// Pointer focus supports orbit capture but has no discrete keyboard action.
    const KEYBOARD: u16 = 1;
    /// The surrounding flex allocation expands this small intrinsic content size.
    fn measure(&self, _: LayoutLimits, _: ComRef<'_, ITextMetricsAbi>) -> Result<Size, NcStatus> {
        Ok(Size { width: 80, height: 24 })
    }
    /// Synchronizes aspect before routing a local drag or wheel zoom.
    fn update(&mut self, state: WidgetUpdate, event: WidgetEvent, _: &str) -> Result<GeometryChange, NcStatus> {
        if state.clip.width <= 0 || state.clip.height <= 0 {
            return Ok(GeometryChange::NoChange);
        }
        let mut grid = self.data.borrow_mut();
        grid.view.set_viewport(local_rect(state.clip));
        match event {
            WidgetEvent::Drag { position, delta, buttons } if buttons & 1 != 0 => {
                let p = point(position);
                grid.view.update_drag(Vec2i::new(p.x.saturating_sub(delta.x), p.y.saturating_sub(delta.y)), p);
            }
            WidgetEvent::Scroll { delta, .. } => {
                let wheel = if delta.y != 0 { delta.y } else { delta.x };
                grid.view.apply_scroll(wheel as f32 * GRID_WHEEL_DISTANCE_SCALE);
            }
            _ => {}
        }
        Ok(GeometryChange::NoChange)
    }
    /// Reuses owned geometry without holding model or cache borrows across painter calls.
    fn paint(&mut self, state: WidgetPaint, painter: &mut Painter<'_>) -> Result<(), NcStatus> {
        if state.clip.width <= 0 || state.clip.height <= 0 {
            return Ok(());
        }
        let (pvm, minor_lines, background) = {
            let mut grid = self.data.borrow_mut();
            grid.view.set_viewport(local_rect(state.clip));
            (grid.view.pvm(), grid.minor_lines, grid.background)
        };
        painter.fill(state.rect, background)?;
        // Own the workspace during the COM call; no cache or application-model borrow crosses it.
        let mut vertices = self.vertices.take();
        write_xy_grid_vertices(local_rect(state.clip), minor_lines, pvm, &mut vertices);
        let result = painter.colored_triangles(&vertices);
        self.vertices.set(vertices);
        result
    }
}

/// Creates the documented oblique orbit camera and finite bounds for the fullscreen grid.
pub(super) fn create_grid_view_3d() -> View3D {
    // Camera::new positions cameras by rotating the +Z basis vector. This shortest-arc
    // quaternion maps that basis onto the original `(24, -32, 26)` viewing offset, preserving the
    // useful initial composition while allowing View3D to accumulate arbitrary arcball rotation.
    let camera_offset = Vec3f::new(24.0, -32.0, 26.0);
    let camera_direction = Vec3f::normalize(&camera_offset);
    let basis = Vec3f::new(0.0, 0.0, 1.0);
    let axis = Vec3f::cross(&basis, &camera_direction);
    let rotation = Quatf::normalize(&Quatf::new(axis.x, axis.y, axis.z, 1.0 + Vec3f::dot(&basis, &camera_direction)));
    let extent = GRID_HALF_EXTENT as f32;
    let camera = Camera::new(
        Vec3f::new(0.0, 0.0, 0.0),
        camera_offset.length(),
        rotation,
        std::f32::consts::FRAC_PI_3,
        1.0,
        0.1,
        GRID_MAX_CAMERA_DISTANCE + extent * 2.0,
    );
    View3D::new(camera, Dimensioni::new(1, 1), (GRID_MIN_CAMERA_DISTANCE, GRID_MAX_CAMERA_DISTANCE)).expect("constant grid camera is valid")
}

/// Projection state reused while the fullscreen widget converts world-space grid lines.
struct GridProjection {
    /// Interactive projection-view-model matrix copied from the shared [`View3D`].
    pvm: Mat4f,
    /// Current content allocation used to map normalized device coordinates to pixels.
    area: Recti,
}

impl GridProjection {
    /// Captures one render allocation and its already aspect-correct interactive projection.
    fn new(area: Recti, pvm: Mat4f) -> Self {
        Self { pvm, area }
    }

    /// Clips a world-space segment against all six homogeneous view-frustum planes.
    fn clip_segment(&self, start: Vec3f, end: Vec3f) -> Option<(Vec4f, Vec4f)> {
        let start = self.pvm * Vec4f::new(start.x, start.y, start.z, 1.0);
        let end = self.pvm * Vec4f::new(end.x, end.y, end.z, 1.0);
        if !clip_point_is_finite(start) || !clip_point_is_finite(end) {
            return None;
        }

        // A point lies inside the OpenGL-style homogeneous frustum when every expression below
        // is non-negative. Parametric clipping happens before division by W, preventing segments
        // behind the camera from producing the enormous stray screen coordinates seen previously.
        let start_planes = clip_plane_distances(start);
        let end_planes = clip_plane_distances(end);
        let mut enter = 0.0_f32;
        let mut leave = 1.0_f32;
        for (start_distance, end_distance) in start_planes.into_iter().zip(end_planes) {
            if start_distance < 0.0 && end_distance < 0.0 {
                return None;
            }
            if start_distance < 0.0 || end_distance < 0.0 {
                let denominator = start_distance - end_distance;
                if denominator.abs() <= f32::EPSILON {
                    return None;
                }
                let intersection = start_distance / denominator;
                if start_distance < 0.0 {
                    enter = enter.max(intersection);
                } else {
                    leave = leave.min(intersection);
                }
                if enter > leave {
                    return None;
                }
            }
        }

        let delta = end - start;
        let clipped_start = start + delta * enter;
        let clipped_end = start + delta * leave;
        (clipped_start.w > f32::EPSILON && clipped_end.w > f32::EPSILON).then_some((clipped_start, clipped_end))
    }

    /// Converts one clipped homogeneous point to microui's top-left-origin screen coordinates.
    fn screen_point(&self, point: Vec4f) -> Option<Vec2f> {
        if !clip_point_is_finite(point) || point.w <= f32::EPSILON {
            return None;
        }
        let normalized_x = point.x / point.w;
        let normalized_y = point.y / point.w;
        let screen = Vec2f::new(
            self.area.x as f32 + (normalized_x * 0.5 + 0.5) * self.area.width as f32,
            self.area.y as f32 + (-normalized_y * 0.5 + 0.5) * self.area.height as f32,
        );
        (screen.x.is_finite() && screen.y.is_finite()).then_some(screen)
    }

    /// Projects a 3D segment and appends a constant-pixel-width screen-space quad.
    fn push_line(&self, vertices: &mut Vec<ColoredVertex>, start: Vec3f, end: Vec3f, width: f32, color: u32) {
        // The world endpoints establish perspective geometry. Clip them before the homogeneous
        // divide, then apply thickness after projection so distant lines remain legible instead of
        // collapsing below one framebuffer pixel as a thin world-space mesh would.
        let Some((start, end)) = self.clip_segment(start, end) else {
            return;
        };
        let Some(start) = self.screen_point(start) else {
            return;
        };
        let Some(end) = self.screen_point(end) else {
            return;
        };
        let delta = end - start;
        let length = delta.length();
        if !length.is_finite() || length <= f32::EPSILON {
            return;
        }

        // Rotate the normalized segment direction ninety degrees to obtain the two quad edges.
        let half_width = width.max(0.5) * 0.5;
        let normal = Vec2f::new(-delta.y / length, delta.x / length) * half_width;
        let a = start + normal;
        let b = end + normal;
        let c = end - normal;
        let d = start - normal;
        for point in [a, b, c, a, c, d] {
            vertices.push(ColoredVertex { x: point.x, y: point.y, rgba: color });
        }
    }
}

/// Returns whether every component is safe for homogeneous clipping and division.
fn clip_point_is_finite(point: Vec4f) -> bool {
    point.x.is_finite() && point.y.is_finite() && point.z.is_finite() && point.w.is_finite()
}

/// Evaluates the six canonical homogeneous frustum half-spaces for one clip-space point.
fn clip_plane_distances(point: Vec4f) -> [f32; 6] {
    [
        point.x + point.w,
        point.w - point.x,
        point.y + point.w,
        point.w - point.y,
        point.z + point.w,
        point.w - point.z,
    ]
}

/// Replaces the perspective grid geometry while retaining the caller's allocation, including empty viewports.
fn write_xy_grid_vertices(area: Recti, show_minor_lines: bool, pvm: Mat4f, vertices: &mut Vec<ColoredVertex>) {
    vertices.clear();
    if area.width <= 0 || area.height <= 0 {
        return;
    }

    let projection = GridProjection::new(area, pvm);

    // Draw non-axis lines first so the red X and green Y axes remain visually authoritative at the
    // origin. Major divisions survive when the menu disables the denser unit-spaced lines.
    for coordinate in -GRID_HALF_EXTENT..=GRID_HALF_EXTENT {
        if coordinate == 0 {
            continue;
        }
        let major = coordinate % GRID_MAJOR_INTERVAL == 0;
        if !major && !show_minor_lines {
            continue;
        }
        let coordinate = coordinate as f32;
        let extent = GRID_HALF_EXTENT as f32;
        let (width, line_color) = if major {
            (1.5, color(78, 92, 118, 230))
        } else {
            (1.0, color(48, 58, 76, 180))
        };
        projection.push_line(
            vertices,
            Vec3f::new(coordinate, -extent, 0.0),
            Vec3f::new(coordinate, extent, 0.0),
            width,
            line_color,
        );
        projection.push_line(
            vertices,
            Vec3f::new(-extent, coordinate, 0.0),
            Vec3f::new(extent, coordinate, 0.0),
            width,
            line_color,
        );
    }

    // Conventional axis colors distinguish the two directions without requiring text labels in
    // the non-interactive rendering surface.
    let extent = GRID_HALF_EXTENT as f32;
    projection.push_line(
        vertices,
        Vec3f::new(-extent, 0.0, 0.0),
        Vec3f::new(extent, 0.0, 0.0),
        2.5,
        color(220, 72, 72, 255),
    );
    projection.push_line(
        vertices,
        Vec3f::new(0.0, -extent, 0.0),
        Vec3f::new(0.0, extent, 0.0),
        2.5,
        color(72, 205, 112, 255),
    );
}

#[cfg(test)]
mod grid_tests {
    use super::*;

    /// Paint uses final settled geometry even if input last saw a full-size allocation.
    #[test]
    fn clipped_grid_paint_matches_an_independent_visible_client() {
        let data = Rc::new(RefCell::new(GridState::default()));
        let mut widget = GridWidget::new(data);
        let full = Rect { x: 0, y: 0, width: 200, height: 100 };
        widget
            .update(crate::viewport_test_support::update_state(full, full), WidgetEvent::None, "")
            .unwrap();
        let visible = Rect { x: 37, y: 23, width: 91, height: 57 };
        let (recorder, output) = crate::viewport_test_support::recorder();
        let mut painter = Painter::from_call(recorder.as_com_ref(), PaintScope { invocation: 1, clip: visible });
        widget
            .paint(
                WidgetPaint {
                    rect: full,
                    clip: visible,
                    ..WidgetPaint::default()
                },
                &mut painter,
            )
            .unwrap();
        let expected = build_xy_grid_vertices(local_rect(visible), true, initial_grid_pvm(local_rect(visible)));
        let output = output.borrow();
        assert!(!output.vertices.is_empty());
        assert_eq!(output.vertices.len(), expected.len());
        for (actual, expected) in output.vertices.iter().zip(expected) {
            assert_eq!((actual.x, actual.y, actual.rgba), (expected.x, expected.y, expected.rgba));
        }
    }

    /// Independent output vectors keep geometry comparisons readable while production reuses its buffer.
    fn build_xy_grid_vertices(area: Recti, show_minor_lines: bool, pvm: Mat4f) -> Vec<ColoredVertex> {
        let mut vertices = Vec::new();
        write_xy_grid_vertices(area, show_minor_lines, pvm, &mut vertices);
        vertices
    }

    /// Produces the same aspect-correct initial PVM used by the live fullscreen widget.
    fn initial_grid_pvm(area: Recti) -> Mat4f {
        let mut view = create_grid_view_3d();
        view.set_viewport(rect(area.x, area.y, area.width.max(1), area.height.max(1)));
        view.pvm()
    }

    #[test]
    fn perspective_grid_density_changes_only_minor_line_geometry() {
        let area = rect(0, 20, 1024, 748);
        let pvm = initial_grid_pvm(area);
        let dense = build_xy_grid_vertices(area, true, pvm);
        let major_only = build_xy_grid_vertices(area, false, pvm);

        // Frustum clipping may discard whole lines at narrow aspect ratios, so assert structural
        // geometry invariants instead of coupling the test to one exact camera composition.
        assert!(!major_only.is_empty());
        assert!(dense.len() > major_only.len());
        assert_eq!(dense.len() % 6, 0);
        assert_eq!(major_only.len() % 6, 0);
        assert!(dense.iter().all(|vertex| {
            let position = Vec2f::new(vertex.x, vertex.y);
            position.x.is_finite()
                && position.y.is_finite()
                && position.x >= area.x as f32 - 2.0
                && position.x <= (area.x + area.width) as f32 + 2.0
                && position.y >= area.y as f32 - 2.0
                && position.y <= (area.y + area.height) as f32 + 2.0
        }));
    }

    #[test]
    fn empty_grid_viewport_emits_no_custom_geometry() {
        assert!(build_xy_grid_vertices(rect(0, 0, 0, 600), true, Mat4f::identity()).is_empty());
        assert!(build_xy_grid_vertices(rect(0, 0, 800, 0), true, Mat4f::identity()).is_empty());
        // An empty repaint must erase the previous frame, not replay retained stale geometry.
        let mut vertices = build_xy_grid_vertices(rect(0, 0, 800, 600), true, initial_grid_pvm(rect(0, 0, 800, 600)));
        assert!(!vertices.is_empty());
        write_xy_grid_vertices(rect(0, 0, 0, 0), true, Mat4f::identity(), &mut vertices);
        assert!(vertices.is_empty());
    }

    #[test]
    fn homogeneous_clipping_bounds_a_segment_that_crosses_the_viewport() {
        let area = rect(10, 20, 800, 600);
        let projection = GridProjection::new(area, Mat4f::identity());
        let mut vertices = Vec::new();

        projection.push_line(
            &mut vertices,
            Vec3f::new(-2.0, 0.0, 0.0),
            Vec3f::new(2.0, 0.0, 0.0),
            2.0,
            color(255, 255, 255, 255),
        );

        assert_eq!(vertices.len(), 6);
        assert!(vertices.iter().all(|vertex| {
            let position = Vec2f::new(vertex.x, vertex.y);
            position.x >= area.x as f32 - 1.1
                && position.x <= (area.x + area.width) as f32 + 1.1
                && position.y >= area.y as f32 - 1.1
                && position.y <= (area.y + area.height) as f32 + 1.1
        }));
    }

    #[test]
    fn arcball_and_zoom_change_the_grid_projection_without_invalid_vertices() {
        let area = rect(0, 0, 800, 600);
        let mut view = create_grid_view_3d();
        view.set_viewport(area);
        let before = build_xy_grid_vertices(area, false, view.pvm());

        let _ = view.update_drag(Vec2i::new(400, 300), Vec2i::new(460, 330));
        let _ = view.apply_scroll(-3.0);
        let after = build_xy_grid_vertices(area, false, view.pvm());

        assert!(!before.is_empty());
        assert!(!after.is_empty());
        assert!(
            before
                .iter()
                .zip(after.iter())
                .any(|(left, right)| (Vec2f::new(left.x, left.y) - Vec2f::new(right.x, right.y)).length() > 0.01)
        );
        assert!(after.iter().all(|vertex| {
            let position = Vec2f::new(vertex.x, vertex.y);
            position.x.is_finite() && position.y.is_finite()
        }));
    }
}
