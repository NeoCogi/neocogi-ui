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

/// Whether a usable navigation candidate was committed.
pub enum UpdateResult {
    /// The resulting camera/PVM is usable, including unchanged no-op movement.
    Handled,
    /// Invalid numeric input/candidate preserves the previous usable state.
    Unhandled,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum NavigationMode {
    Pan,
    Orbit,
}

/// View-local interaction with one authoritative camera and checked distance interval.
pub struct View3D {
    /// Owns current distance, clip planes and derived view/projection matrices.
    camera: Camera,
    /// Chooses the existing orbit/pan calculation.
    nav_mode: NavigationMode,
    /// Positive dimensions corresponding to the last committed projection.
    dimension: Dimensioni,
    /// Visible client origin in widget-local coordinates; captured drags may lie outside it.
    origin: Vec2i,
    /// Caller-selected finite distance interval; Camera owns the sole current distance.
    zoom: (f32, f32),
    /// Derived from the admitted camera in the same commit as that camera.
    pvm: Mat4f,
}

impl View3D {
    /// Admits a finite camera and one useful zoom interval before navigation begins.
    pub fn new(camera: Camera, dimension: Dimensioni, zoom: (f32, f32)) -> Result<Self, String> {
        if dimension.width <= 0
            || dimension.height <= 0
            || !zoom.0.is_finite()
            || !zoom.1.is_finite()
            || zoom.0 <= 0.
            || zoom.1 < zoom.0
            || !(zoom.0..=zoom.1).contains(&camera.distance())
        {
            return Err("invalid viewport or finite zoom interval".into());
        }
        let camera = camera.with_aspect(dimension.width as f32 / dimension.height as f32);
        for distance in [zoom.0, camera.distance(), zoom.1] {
            let candidate = Camera::new(
                camera.target(),
                distance,
                camera.rotation(),
                camera.fov(),
                dimension.width as f32 / dimension.height as f32,
                camera.near_plane(),
                camera.far_plane(),
            );
            if !candidate.is_usable() {
                return Err("camera scale/translation is unsupported by f32 projection".into());
            }
        }
        Ok(Self {
            camera,
            nav_mode: NavigationMode::Orbit,
            dimension,
            origin: Vec2i::default(),
            zoom,
            pvm: *camera.projection_matrix() * *camera.view_matrix(),
        })
    }

    /// Publishes a candidate and its derived PVM together, preserving the last valid state on failure.
    fn commit_camera(&mut self, candidate: Camera) -> UpdateResult {
        if !candidate.is_usable() {
            return UpdateResult::Unhandled;
        }
        self.pvm = *candidate.projection_matrix() * *candidate.view_matrix();
        self.camera = candidate;
        UpdateResult::Handled
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

    /// Orbit/pan changes retain the caller's clip policy and publish only usable matrices.
    pub fn update_drag(&mut self, prev: Vec2i, curr: Vec2i) -> UpdateResult {
        let candidate = match self.nav_mode {
            NavigationMode::Orbit => self
                .camera
                .trackball_rotate(self.dimension, &self.normalize_pointer_pos(&prev), &self.normalize_pointer_pos(&curr)),
            NavigationMode::Pan => self.camera.pan(self.dimension, &self.viewport_position(&prev), &self.viewport_position(&curr)),
        };
        self.commit_camera(candidate)
    }

    /// Every wheel/key zoom derives from Camera's distance and the same checked finite interval.
    pub fn apply_scroll(&mut self, delta: f32) -> UpdateResult {
        if !delta.is_finite() {
            return UpdateResult::Unhandled;
        }
        let distance = (f64::from(self.camera.distance()) + f64::from(delta)).clamp(f64::from(self.zoom.0), f64::from(self.zoom.1)) as f32;
        let candidate = Camera::new(
            self.camera.target(),
            distance,
            self.camera.rotation(),
            self.camera.fov(),
            self.dimension.width as f32 / self.dimension.height as f32,
            self.camera.near_plane(),
            self.camera.far_plane(),
        );
        self.commit_camera(candidate)
    }

    /// Updates the positive viewport size and projection; origin belongs to `set_viewport`.
    fn set_dimension(&mut self, dimension: Dimensioni) {
        if (self.dimension.width, self.dimension.height) == (dimension.width, dimension.height) {
            return;
        }
        let aspect = dimension.width as f32 / dimension.height as f32;
        if matches!(self.commit_camera(self.camera.with_aspect(aspect)), UpdateResult::Handled) {
            self.dimension = dimension;
        }
    }

    /// Fits projection and input normalization to a nonempty visible widget-local client rectangle.
    /// Callers skip empty clips. Paint also calls this after final layout, so a layout-only change
    /// cannot leave the camera using dimensions from an earlier input traversal.
    pub fn set_viewport(&mut self, viewport: Recti) {
        if viewport.width <= 0 || viewport.height <= 0 {
            return;
        }
        self.origin = Vec2i::new(viewport.x, viewport.y);
        self.set_dimension(Dimensioni::new(viewport.width, viewport.height));
    }

    pub fn get_navigation_mode(&self) -> NavigationMode {
        self.nav_mode
    }
    pub fn set_navigation_mode(&mut self, nav_mode: NavigationMode) {
        if nav_mode != self.nav_mode {
            self.nav_mode = nav_mode;
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

    /// Resizing, orbiting and all zoom inputs preserve clips and refuse corrupt numeric candidates.
    #[test]
    fn navigation_retains_clip_policy_and_last_valid_camera() {
        let mut view = crate::grid::create_grid_view_3d();
        let clips = (view.camera.near_plane(), view.camera.far_plane());
        for distance_delta in [f32::MAX, -f32::MAX, 0.5, -0.5] {
            view.apply_scroll(distance_delta);
            assert!(view.camera.is_usable());
            assert!((32. ..=120.).contains(&view.camera.distance()));
            assert_eq!((view.camera.near_plane(), view.camera.far_plane()), clips);
        }
        let previous = view.pvm();
        assert!(matches!(view.apply_scroll(f32::NAN), UpdateResult::Unhandled));
        assert!(matches!(view.apply_scroll(f32::INFINITY), UpdateResult::Unhandled));
        same_matrix(previous, view.pvm());
        for dimension in [(80, 40), (1, 100_000), (100_000, 1)] {
            view.set_viewport(Recti::new(3, 7, dimension.0, dimension.1));
            for mode in [NavigationMode::Pan, NavigationMode::Orbit] {
                view.set_navigation_mode(mode);
                view.update_drag(Vec2i::new(8, 10), Vec2i::new(10, 12));
                assert!(view.camera.is_usable());
                assert_eq!((view.camera.near_plane(), view.camera.far_plane()), clips);
            }
        }
    }

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
