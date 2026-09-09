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
//! Lines, concave polygons and nested clipping through the public COM painter.
use super::graphics::*;

/// Animated geometry plus optional local pointer override.
pub(super) struct PainterDemo {
    /// Monotonic phase changes drawing without invalidating preferred size.
    phase: f32,
    /// Pointer position while hovered; otherwise the analytic orbit controls the star.
    star_center: Option<Vec2f>,
}

impl Default for PainterDemo {
    /// Starts the paint-only animation without a remembered pointer target.
    fn default() -> Self {
        PainterDemo { phase: 0.0, star_center: None }
    }
}

impl LeafContent for PainterDemo {
    /// Intrinsic size stays fixed while animation changes paint only.
    fn measure(&self, _: LayoutLimits, _: ComRef<'_, ITextMetricsAbi>) -> Result<Size, NcStatus> {
        Ok(Size { width: 240, height: 200 })
    }

    /// Samples time and the routed local pointer without changing intrinsic dimensions.
    fn update(&mut self, state: WidgetUpdate, event: WidgetEvent, _: &str) -> Result<GeometryChange, NcStatus> {
        let bounds = local_rect(state.rect);
        let local_width = bounds.width.max(0) as f32;
        let local_height = bounds.height.max(0) as f32;
        if local_width <= 0.0 || local_height <= 0.0 {
            return Ok(GeometryChange::NO_CHANGE);
        }

        self.phase = (microui_redux_demo_host::monotonic_seconds() as f32 * 1.5) % (PI * 2.0);
        if state.hovered {
            let position = match event {
                WidgetEvent::Move { position, .. }
                | WidgetEvent::Drag { position, .. }
                | WidgetEvent::Down { position, .. }
                | WidgetEvent::Up { position, .. }
                | WidgetEvent::Scroll { position, .. } => Some(position),
                _ => None,
            };
            if let Some(position) = position {
                self.star_center = Some(Vec2f::new(position.x as f32, position.y as f32));
            }
        } else {
            self.star_center = None;
        }

        // Geometry is unchanged here; mutations through other handles invalidate their owners.
        Ok(GeometryChange::NO_CHANGE)
    }

    /// Records ordered lines and concave fills through the shared painter and narrowed clips.
    fn paint(&self, state: WidgetPaint, painter: &mut Painter<'_>) -> Result<(), NcStatus> {
        let bounds = local_rect(state.rect);
        let local_width = bounds.width.max(0) as f32;
        let local_height = bounds.height.max(0) as f32;
        if local_width <= 0.0 || local_height <= 0.0 {
            return Ok(());
        }

        let clip_rect = rect(18, 18, (bounds.width - 36).max(0), (bounds.height - 36).max(0));
        let animated_center = Vec2f::new(
            local_width * 0.5 + self.phase.cos() * (local_width * 0.16),
            local_height * 0.5 + self.phase.sin() * (local_height * 0.12),
        );
        let star_center = if state.hovered {
            self.star_center.unwrap_or(animated_center)
        } else {
            animated_center
        };
        let star_center = Vec2f::new(star_center.x.clamp(0.0, local_width), star_center.y.clamp(0.0, local_height));

        {
            let g = painter;
            let local = local_rect(state.rect);
            let outer = rect(8, 8, (local.width - 16).max(0), (local.height - 16).max(0));
            let background = [
                Vec2f::new(0.0, 0.0),
                Vec2f::new(local_width, 0.0),
                Vec2f::new(local_width, local_height),
                Vec2f::new(0.0, local_height),
            ];

            fill(g, background.as_slice(), color(34, 38, 44, 255))?;
            for (start, end) in rect_edges(outer).into_iter().flatten() {
                stroke(g, start, end, 2.0, color(65, 70, 76, 255))?;
            }
            for (start, end) in rect_edges(clip_rect).into_iter().flatten() {
                stroke(g, start, end, 1.5, color(240, 210, 110, 255))?;
            }

            stroke(
                g,
                Vec2f::new(12.0, 12.0),
                Vec2f::new(local_width - 12.0, local_height - 12.0),
                3.0,
                color(70, 145, 220, 180),
            )?;
            stroke(
                g,
                Vec2f::new(local_width - 12.0, 12.0),
                Vec2f::new(12.0, local_height - 12.0),
                3.0,
                color(220, 95, 110, 180),
            )?;

            {
                let mut clipped = g.clipped(wire_rect(clip_rect))?;
                let g = &mut clipped;
                for idx in 0..4 {
                    let t = self.phase + idx as f32 * 0.45;
                    let y = clip_rect.y as f32 + clip_rect.height as f32 * (0.15 + idx as f32 * 0.2);
                    stroke(
                        g,
                        Vec2f::new(-32.0, y + t.sin() * 10.0),
                        Vec2f::new(local_width + 32.0, y + t.cos() * 26.0),
                        7.0 - idx as f32,
                        color(60 + idx as u8 * 30, 140 + idx as u8 * 18, 225, 130),
                    )?;
                }

                let star = build_star_polygon(star_center, 58.0, 26.0, 5, self.phase);
                fill(g, star.as_slice(), color(255, 180, 70, 225))?;

                let sweep = build_star_polygon(
                    Vec2f::new(local_width * 0.35 + self.phase.sin() * 18.0, local_height * 0.72),
                    34.0,
                    14.0,
                    4,
                    -self.phase * 1.3,
                );
                fill(g, sweep.as_slice(), color(90, 220, 180, 190))?;
            }
        }
        Ok(())
    }
}
