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
//! A concrete COM leaf draws a CPU-projected cube through solid painter geometry.
//!
//! The application owns only camera/animation data. No backend frame, native handle, callback or
//! atlas packing detail crosses the widget boundary. The same code runs on every platform backend.
use neocogi_ui::*;
use neocogi_demo_host::{AppState, Host, HostError, report_exit};
use neocogi_com::{ComRef, NcStatus};
use rs_math3d::{EPS_F32, Vec2f, Vec3f, Quatf, lookat, perspective, project3};
use std::{cell::Cell, rc::Rc};

/// Stateless drawing behavior reads the application's concrete paint-only animation value.
struct Cube {
    /// External animation affects paint, so preferred size never needs to change with the angle.
    angle: Rc<Cell<f32>>,
}
impl LeafContent for Cube {
    /// The preferred square honors bounded zero and independent finite constraints.
    fn measure(&self, bounds: LayoutLimits, _metrics: ComRef<'_, ITextMetricsAbi>) -> Result<Size, NcStatus> {
        Ok(Size {
            width: if bounds.width.kind == LimitKind::Finite {
                300.min(bounds.width.extent)
            } else {
                300
            },
            height: if bounds.height.kind == LimitKind::Finite {
                300.min(bounds.height.extent)
            } else {
                300
            },
        })
    }
    /// Observational projection records one solid batch in normal painter order and clipping.
    fn paint(&self, state: WidgetPaint, painter: &mut Painter<'_>) -> Result<(), NcStatus> {
        painter.colored_triangles(&build_cube_vertices(state.rect, self.angle.get()))
    }
}

const CUBE_POINTS: [Vec3f; 8] = [
    Vec3f { x: -1.0, y: -1.0, z: -1.0 },
    Vec3f { x: 1.0, y: -1.0, z: -1.0 },
    Vec3f { x: 1.0, y: 1.0, z: -1.0 },
    Vec3f { x: -1.0, y: 1.0, z: -1.0 },
    Vec3f { x: -1.0, y: -1.0, z: 1.0 },
    Vec3f { x: 1.0, y: -1.0, z: 1.0 },
    Vec3f { x: 1.0, y: 1.0, z: 1.0 },
    Vec3f { x: -1.0, y: 1.0, z: 1.0 },
];

// Each quad is emitted as two triangles. Opaque faces are sorted from far to near because this
// simple example deliberately uses the UI colored-triangle path rather than a depth buffer.
const CUBE_FACES: [([usize; 4], u32); 6] = [
    ([0, 1, 2, 3], 0xffd26946),
    ([4, 5, 6, 7], 0xff5252dc),
    ([0, 4, 7, 3], 0xff70b448),
    ([1, 5, 6, 2], 0xff3eace6),
    ([0, 1, 5, 4], 0xffcd5c8e),
    ([3, 2, 6, 7], 0xffcdb93c),
];

/// Sorts opaque faces by average projected depth for this intentionally CPU-rendered example.
fn face_depth(corners: &[usize; 4], points: &[Vec3f; 8]) -> f32 {
    corners.iter().map(|&index| points[index].z).sum::<f32>() / 4.0
}

/// CPU-projects colored faces into local coordinates; the UI supplies its own atlas material.
fn build_cube_vertices(area: Rect, angle: f32) -> Vec<ColoredVertex> {
    let rotate_y = Quatf::of_axis_angle(&Vec3f::new(0.0, 1.0, 0.0), angle, EPS_F32).expect("valid Y rotation");
    let rotate_x = Quatf::of_axis_angle(&Vec3f::new(1.0, 0.0, 0.0), angle * 0.63, EPS_F32).expect("valid X rotation");
    let model = (rotate_x * rotate_y).mat4();
    let view = lookat(&Vec3f::new(0.0, 0.0, 4.0), &Vec3f::new(0.0, 0.0, 0.0), &Vec3f::new(0.0, 1.0, 0.0));
    let projection = perspective(std::f32::consts::FRAC_PI_3, area.width.max(1) as f32 / area.height.max(1) as f32, 0.1, 100.0);
    let model_view = view * model;

    // `project3` uses a bottom-left screen origin. Reversing the Y endpoints maps its result to
    // microui's top-left screen coordinates without hand-writing another projection transform.
    let screen_bottom_left = Vec2f::new(area.x as f32, (area.y + area.height) as f32);
    let screen_top_right = Vec2f::new((area.x + area.width) as f32, area.y as f32);
    let projected = CUBE_POINTS.map(|point| project3(&model_view, &projection, &screen_bottom_left, &screen_top_right, &point));

    let mut faces = CUBE_FACES;
    faces.sort_by(|(left, _), (right, _)| face_depth(right, &projected).total_cmp(&face_depth(left, &projected)));

    let mut vertices = Vec::with_capacity(6 * 6);
    for (corners, color) in faces {
        let [a, b, c, d] = corners;
        for index in [a, b, c, a, c, d] {
            vertices.push(ColoredVertex {
                x: projected[index].x,
                y: projected[index].y,
                rgba: color,
            });
        }
    }
    vertices
}

/// Application-owned animation is shared explicitly with the inline COM leaf content.
struct State {
    /// Rotation is a paint-only external value; the widget always reports unchanged measurement.
    angle: Rc<Cell<f32>>,
}
impl AppState for State {
    /// This example has no application messages.
    type Message = ();

    /// Time-based motion stays consistent across native and browser refresh rates.
    fn frame(&mut self, _host: &Host, _size: Size, seconds: f64) -> Result<bool, NcStatus> {
        self.angle.set((seconds * 1.08 % f64::from(std::f32::consts::TAU)) as f32);
        Ok(true)
    }
}

/// Creates one concrete COM leaf; standard host ownership handles all input/update/render phases.
fn run() -> Result<(), HostError> {
    neocogi_demo_host::initialize()?;
    let host = Host::new("COM solid cube", Size { width: 640, height: 480 }, neocogi_demo_assets::load_bundle()?)?;
    let angle = Rc::new(Cell::new(0.0));
    let cube = host.ui.widgets().leaf(Cube { angle: angle.clone() }, WidgetOptions::NO_INTERACT)?;
    host.ui.window("Solid painter cube", Rect { x: 40, y: 40, width: 360, height: 360 }, &cube)?;
    host.run(State { angle }, Events::new())
}

/// Reports startup and frame failures through formatting retained by the min-size profile.
fn main() -> std::process::ExitCode {
    report_exit(run())
}
