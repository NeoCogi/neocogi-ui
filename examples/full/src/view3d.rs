#![allow(dead_code)]
//
// Copyright 2021-Present (c) Raja Lehtihet & Wael El Oraiby
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
//! View state and interaction handling for the 3D demo viewport.

use rs_math3d::*;
use super::camera::*;

pub enum UpdateResult {
    Handled,
    Unhandled,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum NavigationMode {
    Pan,
    Orbit,
}

pub struct View3D {
    camera: Camera,
    nav_mode: NavigationMode,
    dimension: Dimensioni,
    /// Visible client origin in widget-local coordinates; captured drags may lie outside it.
    origin: Vec2i,
    scroll: f32,
    bounds: Box3f,
    pvm: Mat4f,
}

impl View3D {
    pub fn new(camera: Camera, dimension: Dimensioni, bounds: Box3f) -> Self {
        let scroll = camera.distance();
        Self {
            camera,
            nav_mode: NavigationMode::Orbit,
            dimension,
            origin: Vec2i::default(),
            scroll,
            bounds,
            pvm: *camera.projection_matrix() * *camera.view_matrix(),
        }
    }

    /// Translates captured signed coordinates before float conversion, preserving small movements
    /// even when the visible origin is near either integer limit.
    fn viewport_position(&self, pos: &Vec2i) -> Vec2f {
        Vec2f::new(
            (i64::from(pos.x) - i64::from(self.origin.x)) as f32,
            (i64::from(pos.y) - i64::from(self.origin.y)) as f32,
        )
    }

    /// Maps positions relative to the same visible origin and dimensions used by projection.
    fn normalize_pointer_pos(&self, pos: &Vec2i) -> Vec2f {
        let dim_f = Vec2f::new(self.dimension.width as _, self.dimension.height as _);
        let pos_f = self.viewport_position(pos);

        let mut norm_pos = 2.0 * pos_f / dim_f - Vec2f::new(1.0, 1.0);

        norm_pos.y *= -1.0;
        norm_pos
    }

    fn refresh_projection(&mut self) {
        // TODO: do proper computation of the far plane
        let far_plane = self.bounds.extent().length() * 100.0;
        self.camera = self.camera.with_far_plane(far_plane);
        self.pvm = *self.camera.projection_matrix() * *self.camera.view_matrix();
    }

    pub fn update_drag(&mut self, prev: Vec2i, curr: Vec2i) -> UpdateResult {
        self.refresh_projection();

        let handled = match self.nav_mode {
            NavigationMode::Orbit => {
                let p = self.normalize_pointer_pos(&prev);
                let c = self.normalize_pointer_pos(&curr);

                self.camera = self.camera.trackball_rotate(self.dimension, &p, &c);
                UpdateResult::Handled
            }

            NavigationMode::Pan => {
                let p = self.viewport_position(&prev);
                let c = self.viewport_position(&curr);
                self.camera = self.camera.pan(self.dimension, &p, &c);
                UpdateResult::Handled
            }
        };

        self.pvm = *self.camera.projection_matrix() * *self.camera.view_matrix();
        handled
    }

    pub fn apply_scroll(&mut self, delta: f32) -> UpdateResult {
        self.apply_scroll_with_limits(delta, 0.5, f32::MAX)
    }

    /// Moves the orbit camera along its view radius while enforcing caller-selected limits.
    pub fn apply_scroll_with_limits(&mut self, delta: f32, minimum: f32, maximum: f32) -> UpdateResult {
        // Components know the useful scale of their own scene. Keeping the clamp here ensures the
        // authoritative `scroll` radius and reconstructed Camera can never diverge when a wheel
        // delta crosses either limit.
        let minimum = minimum.max(0.5);
        let maximum = maximum.max(minimum);
        self.scroll = (self.scroll + delta).clamp(minimum, maximum);
        let distance = self.scroll;
        let aspect = (self.dimension.width as f32) / (self.dimension.height as f32);
        self.camera = Camera::new(
            self.camera.target(),
            distance,
            self.camera.rotation(),
            self.camera.fov(),
            aspect,
            self.camera.near_plane(),
            self.camera.far_plane(),
        );
        self.pvm = *self.camera.projection_matrix() * *self.camera.view_matrix();
        UpdateResult::Handled
    }

    /// Updates the positive viewport size and projection; origin belongs to `set_viewport`.
    fn set_dimension(&mut self, dimension: Dimensioni) {
        if (self.dimension.width, self.dimension.height) == (dimension.width, dimension.height) {
            return;
        }
        self.dimension = dimension;
        let aspect = (self.dimension.width as f32) / (self.dimension.height as f32);
        self.camera = self.camera.with_aspect(aspect);
        self.refresh_projection();
    }

    /// Fits projection and input normalization to a nonempty visible widget-local client rectangle.
    /// Callers skip empty clips. Paint also calls this after final layout, so a layout-only change
    /// cannot leave the camera using dimensions from an earlier input traversal.
    pub fn set_viewport(&mut self, viewport: Recti) {
        debug_assert!(viewport.width > 0 && viewport.height > 0);
        self.origin = Vec2i::new(viewport.x, viewport.y);
        self.set_dimension(Dimensioni::new(viewport.width, viewport.height));
    }

    pub fn get_navigation_mode(&self) -> NavigationMode {
        self.nav_mode
    }
    pub fn set_navigation_mode(&mut self, nav_mode: NavigationMode) {
        if nav_mode != self.nav_mode {
            self.nav_mode = nav_mode;
            self.refresh_projection();
        }
    }

    pub fn pvm(&self) -> Mat4f {
        self.pvm
    }

    pub fn projection_matrix(&self) -> Mat4f {
        *self.camera.projection_matrix()
    }

    pub fn view_matrix(&self) -> Mat4f {
        *self.camera.view_matrix()
    }
}

#[cfg(test)]
mod viewport_tests {
    use super::*;

    /// Compares numerical output rather than camera bookkeeping or cached field values.
    fn same_matrix(left: Mat4f, right: Mat4f) {
        for (left, right) in left.col.iter().zip(right.col.iter()) {
            for (left, right) in [left.x, left.y, left.z, left.w].into_iter().zip([right.x, right.y, right.z, right.w]) {
                assert!((left - right).abs() < 0.00001, "{left} != {right}");
            }
        }
    }

    /// Projection and both navigation modes match an independently positioned visible client.
    #[test]
    fn visible_client_projection_and_drag_match_a_full_client_of_that_size() {
        for viewport in [
            Recti::new(100, 0, 100, 100),
            Recti::new(0, 0, 100, 100),
            Recti::new(0, 80, 200, 20),
            Recti::new(0, 0, 200, 20),
            Recti::new(37, 23, 91, 57),
        ] {
            for mode in [NavigationMode::Orbit, NavigationMode::Pan] {
                let mut clipped = crate::grid::create_grid_view_3d();
                let mut reference = crate::grid::create_grid_view_3d();
                clipped.set_viewport(Recti::new(0, 0, 200, 100));
                clipped.set_viewport(viewport);
                reference.set_viewport(Recti::new(0, 0, viewport.width, viewport.height));
                same_matrix(clipped.projection_matrix(), reference.projection_matrix());
                same_matrix(clipped.pvm(), reference.pvm());
                clipped.set_navigation_mode(mode);
                reference.set_navigation_mode(mode);
                let previous = Vec2i::new(viewport.width / 2, viewport.height / 2);
                let current = previous + Vec2i::new(13, -7);
                let origin = Vec2i::new(viewport.x, viewport.y);
                clipped.update_drag(previous + origin, current + origin);
                reference.update_drag(previous, current);
                same_matrix(clipped.view_matrix(), reference.view_matrix());
                same_matrix(clipped.pvm(), reference.pvm());
            }
        }
    }

    /// Moving the visible origin without resizing still changes pointer normalization.
    #[test]
    fn viewport_translation_keeps_its_center_at_normalized_zero() {
        let mut view = crate::grid::create_grid_view_3d();
        for origin in [Vec2i::new(120, 70), Vec2i::new(i32::MAX - 80, i32::MIN)] {
            view.set_viewport(Recti::new(0, 0, 80, 40));
            view.set_viewport(Recti::new(origin.x, origin.y, 80, 40));
            let center = view.normalize_pointer_pos(&(origin + Vec2i::new(40, 20)));
            assert_eq!((center.x, center.y), (0., 0.));
            let movement = view.viewport_position(&(origin + Vec2i::new(41, 21)));
            assert_eq!((movement.x, movement.y), (41., 21.));
        }
        let outside = view.normalize_pointer_pos(&Vec2i::new(i32::MIN, i32::MAX));
        assert!(outside.x.is_finite() && outside.y.is_finite());
    }
}
