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
//! Local math-to-contract conversions used by the concrete graphics widgets.
pub(super) use neocogi_ui_contracts::*;
pub(super) use neocogi_com::{ComPtr, ComRef, NcStatus, NcCopyFromBufferOffset};
pub(super) use rs_math3d::{Vec2f, Vec2i, Vec3f, Vec4f, Mat4f, Quatf, Box3f, Recti, Dimensioni};
pub(super) use rs_math3d::{RealVector, Vector, CrossProduct};
pub(super) use std::f32::consts::PI;

/// Constructs a local mathematical rectangle, without retaining runtime geometry.
pub(super) fn rect(x: i32, y: i32, width: i32, height: i32) -> Recti {
    Recti::new(x, y, width, height)
}
/// Converts a copied wire allocation into the math library's ordinary value.
pub(super) fn local_rect(r: neocogi_ui_contracts::Rect) -> Recti {
    rect(r.x, r.y, r.width, r.height)
}
/// Copies geometry into its portable C-shaped representation.
pub(super) fn wire_rect(r: Recti) -> neocogi_ui_contracts::Rect {
    neocogi_ui_contracts::Rect {
        x: r.x,
        y: r.y,
        width: r.width,
        height: r.height,
    }
}
/// Maps an already local pointer coordinate into ordinary vector arithmetic.
pub(super) fn point(p: Point) -> Vec2i {
    Vec2i::new(p.x, p.y)
}
/// Packs RGBA independently of host byte order.
pub(super) const fn color(r: u8, g: u8, b: u8, a: u8) -> u32 {
    u32::from_le_bytes([r, g, b, a])
}
/// Converts a line's endpoints at the drawing boundary; tessellation belongs to the UI.
pub(super) fn stroke(p: &mut Painter<'_>, a: Vec2f, b: Vec2f, width: f32, rgba: u32) -> Result<(), NcStatus> {
    p.stroke_line_f32(PaintPoint { x: a.x, y: a.y }, PaintPoint { x: b.x, y: b.y }, width, rgba)
}
/// Copies a complete simple polygon to the UI's existing fill implementation.
pub(super) fn fill(p: &mut Painter<'_>, points: &[Vec2f], rgba: u32) -> Result<(), NcStatus> {
    let points: Vec<_> = points.iter().map(|v| PaintPoint { x: v.x, y: v.y }).collect();
    p.fill_polygon_f32(&points, rgba)
}

/// Returns the four clockwise outline segments only for a nonempty rectangle.
pub(super) fn rect_edges(rect: Recti) -> Option<[(Vec2f, Vec2f); 4]> {
    if rect.width <= 0 || rect.height <= 0 {
        return None;
    }

    let x0 = rect.x as f32;
    let y0 = rect.y as f32;
    let x1 = (rect.x + rect.width) as f32;
    let y1 = (rect.y + rect.height) as f32;
    Some([
        (Vec2f::new(x0, y0), Vec2f::new(x1, y0)),
        (Vec2f::new(x1, y0), Vec2f::new(x1, y1)),
        (Vec2f::new(x1, y1), Vec2f::new(x0, y1)),
        (Vec2f::new(x0, y1), Vec2f::new(x0, y0)),
    ])
}

/// Alternates inner and outer radii to produce one ordered, concave star polygon.
pub(super) fn build_star_polygon(center: Vec2f, outer_radius: f32, inner_radius: f32, spikes: usize, angle: f32) -> Vec<Vec2f> {
    let spikes = spikes.max(2);
    let mut points = Vec::with_capacity(spikes * 2);
    for idx in 0..spikes * 2 {
        let radius = if idx % 2 == 0 { outer_radius } else { inner_radius };
        let theta = angle + idx as f32 * PI / spikes as f32;
        points.push(Vec2f::new(center.x + theta.cos() * radius, center.y + theta.sin() * radius));
    }
    points
}

/// Returns an axis-aligned square around the supplied marker center.
pub(super) fn build_square_polygon(center: Vec2f, radius: f32) -> [Vec2f; 4] {
    [
        Vec2f::new(center.x - radius, center.y - radius),
        Vec2f::new(center.x + radius, center.y - radius),
        Vec2f::new(center.x + radius, center.y + radius),
        Vec2f::new(center.x - radius, center.y + radius),
    ]
}

/// Returns the four cardinal marker vertices in clockwise order.
pub(super) fn build_diamond_polygon(center: Vec2f, radius: f32) -> [Vec2f; 4] {
    [
        Vec2f::new(center.x, center.y - radius),
        Vec2f::new(center.x + radius, center.y),
        Vec2f::new(center.x, center.y + radius),
        Vec2f::new(center.x - radius, center.y),
    ]
}
