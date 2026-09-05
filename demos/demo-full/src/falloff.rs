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
//! Interactive curve editing over copied COM input and scoped drawing.
use super::graphics::*;

const FALLOFF_MIN_NODE_GAP: f32 = 0.08;
const FALLOFF_HANDLE_X_MAX: f32 = 0.5;
const FALLOFF_PICK_RADIUS: f32 = 9.0;
const FALLOFF_SEGMENT_STEPS: usize = 24;

#[derive(Clone, Copy)]
/// One normalized anchor and its monotone cubic tangent controls.
struct FalloffNode {
    /// Anchor in the unit square.
    pos: Vec2f,
    /// Incoming horizontal factor relative to the previous segment.
    in_x: f32,
    /// Incoming handle height in the unit square.
    in_y: f32,
    /// Outgoing horizontal factor relative to the next segment.
    out_x: f32,
    /// Outgoing handle height in the unit square.
    out_y: f32,
}

#[derive(Clone, Copy, PartialEq, Eq)]
/// Explicit hit target preserves which anchor owns the selected handle.
enum FalloffTarget {
    /// Move an interior curve anchor.
    Anchor(usize),
    /// Move its incoming tangent.
    InHandle(usize),
    /// Move its outgoing tangent.
    OutHandle(usize),
}

/// Interactive normalized curve stored inline in one local COM leaf.
pub(super) struct FalloffEditor {
    /// Ordered anchors, including the two pinned endpoints.
    nodes: Vec<FalloffNode>,
    /// Captured target remains stable through a pointer drag.
    active: Option<FalloffTarget>,
    /// Current hit candidate changes marker presentation only.
    hovered: Option<FalloffTarget>,
}

impl Default for FalloffEditor {
    /// Initializes four normalized anchors and enforces pinned endpoints and monotone handles.
    fn default() -> Self {
        let mut editor = FalloffEditor {
            nodes: vec![
                FalloffNode {
                    pos: Vec2f::new(0.0, 1.0),
                    in_x: 0.0,
                    in_y: 1.0,
                    out_x: 0.22,
                    out_y: 1.0,
                },
                FalloffNode {
                    pos: Vec2f::new(0.23, 0.94),
                    in_x: 0.18,
                    in_y: 0.97,
                    out_x: 0.28,
                    out_y: 0.70,
                },
                FalloffNode {
                    pos: Vec2f::new(0.57, 0.31),
                    in_x: 0.24,
                    in_y: 0.46,
                    out_x: 0.24,
                    out_y: 0.10,
                },
                FalloffNode {
                    pos: Vec2f::new(1.0, 0.0),
                    in_x: 0.20,
                    in_y: 0.0,
                    out_x: 0.0,
                    out_y: 0.0,
                },
            ],
            active: None,
            hovered: None,
        };
        editor.sanitize();
        editor
    }
}

impl FalloffEditor {
    // The editor keeps a small inner margin so markers and thick strokes can sit inside the
    // widget without fighting the outer container frame.
    /// Reserves a small inner margin for handles and thick strokes.
    fn graph_rect(bounds: Recti) -> Recti {
        rect(14, 14, (bounds.width - 28).max(0), (bounds.height - 28).max(0))
    }

    // Endpoints stay pinned to a classic brush falloff shape while interior anchors remain sorted
    // on x. Handle x values are stored as relative factors in [0, 0.5], which guarantees every
    // segment satisfies P0.x <= P1.x <= P2.x <= P3.x and therefore stays x-monotone.
    /// Pins endpoints, clamps values and keeps all cubic segments monotone along x.
    fn sanitize(&mut self) {
        if self.nodes.len() < 2 {
            return;
        }

        let last = self.nodes.len() - 1;
        self.nodes[0].pos = Vec2f::new(0.0, 1.0);
        self.nodes[last].pos = Vec2f::new(1.0, 0.0);

        for idx in 0..self.nodes.len() {
            self.nodes[idx].in_x = self.nodes[idx].in_x.clamp(0.0, FALLOFF_HANDLE_X_MAX);
            self.nodes[idx].out_x = self.nodes[idx].out_x.clamp(0.0, FALLOFF_HANDLE_X_MAX);
            self.nodes[idx].in_y = self.nodes[idx].in_y.clamp(0.0, 1.0);
            self.nodes[idx].out_y = self.nodes[idx].out_y.clamp(0.0, 1.0);
            self.nodes[idx].pos.y = self.nodes[idx].pos.y.clamp(0.0, 1.0);
        }

        for idx in 1..last {
            let left = self.nodes[idx - 1].pos.x + FALLOFF_MIN_NODE_GAP;
            let right = self.nodes[idx + 1].pos.x - FALLOFF_MIN_NODE_GAP;
            self.nodes[idx].pos.x = if left <= right {
                self.nodes[idx].pos.x.clamp(left, right)
            } else {
                (left + right) * 0.5
            };
        }

        self.nodes[0].in_x = 0.0;
        self.nodes[0].in_y = self.nodes[0].pos.y;
        self.nodes[last].out_x = 0.0;
        self.nodes[last].out_y = self.nodes[last].pos.y;
    }

    // Converts normalized falloff coordinates into widget-local pixels. The editor stores data in
    // normalized space so the same control logic works no matter how the window is resized.
    /// Maps normalized graph coordinates into the widget-local drawable rectangle.
    fn graph_to_local(graph: Recti, point: Vec2f) -> Vec2f {
        let width = graph.width.max(1) as f32;
        let height = graph.height.max(1) as f32;
        Vec2f::new(
            graph.x as f32 + point.x.clamp(0.0, 1.0) * width,
            graph.y as f32 + (1.0 - point.y.clamp(0.0, 1.0)) * height,
        )
    }

    // Converts widget-local pixels back into normalized falloff coordinates and clamps them into
    // the visible graph domain so dragging outside the rect still yields stable endpoint behavior.
    /// Maps a local pointer back into the clamped unit-square authoring space.
    fn local_to_graph(graph: Recti, point: Vec2f) -> Vec2f {
        let width = graph.width.max(1) as f32;
        let height = graph.height.max(1) as f32;
        Vec2f::new(
            ((point.x - graph.x as f32) / width).clamp(0.0, 1.0),
            (1.0 - (point.y - graph.y as f32) / height).clamp(0.0, 1.0),
        )
    }

    // Each incoming handle is parameterized relative to the span from the previous anchor. That
    // keeps the monotonicity invariant local to one segment and avoids cross-segment repair logic.
    /// Resolves the incoming handle relative to the preceding anchor interval.
    fn in_handle_graph(&self, idx: usize) -> Vec2f {
        let node = self.nodes[idx];
        let prev = self.nodes[idx - 1].pos;
        let span = (node.pos.x - prev.x).max(0.0);
        Vec2f::new(node.pos.x - span * node.in_x, node.in_y)
    }

    // Outgoing handles use the same relative-x representation against the next anchor. Limiting
    // the factor to 0.5 guarantees the two handles for a segment cannot cross on x.
    /// Resolves the outgoing handle relative to the following anchor interval.
    fn out_handle_graph(&self, idx: usize) -> Vec2f {
        let node = self.nodes[idx];
        let next = self.nodes[idx + 1].pos;
        let span = (next.x - node.pos.x).max(0.0);
        Vec2f::new(node.pos.x + span * node.out_x, node.out_y)
    }

    /// Locates one concrete anchor or tangent handle for hit testing and paint.
    fn target_local(&self, graph: Recti, target: FalloffTarget) -> Vec2f {
        match target {
            FalloffTarget::Anchor(idx) => Self::graph_to_local(graph, self.nodes[idx].pos),
            FalloffTarget::InHandle(idx) => Self::graph_to_local(graph, self.in_handle_graph(idx)),
            FalloffTarget::OutHandle(idx) => Self::graph_to_local(graph, self.out_handle_graph(idx)),
        }
    }

    // Exposes one cubic segment in Bernstein control-point form so sampling and drawing both reuse
    // the same handle reconstruction logic.
    /// Collects one cubic segment in endpoint/control/control/endpoint order.
    fn segment_points(&self, seg: usize) -> [Vec2f; 4] {
        [
            self.nodes[seg].pos,
            self.out_handle_graph(seg),
            self.in_handle_graph(seg + 1),
            self.nodes[seg + 1].pos,
        ]
    }

    // Standard cubic Bezier evaluation. The editor relies on dense line sampling rather than
    // adding a dedicated curve primitive to the renderer.
    /// Evaluates the cubic Bernstein basis at the supplied interpolation parameter.
    fn eval_segment(&self, seg: usize, t: f32) -> Vec2f {
        let [p0, p1, p2, p3] = self.segment_points(seg);
        let omt = 1.0 - t;
        let omt2 = omt * omt;
        let t2 = t * t;
        p0 * (omt2 * omt) + p1 * (3.0 * omt2 * t) + p2 * (3.0 * omt * t2) + p3 * (t2 * t)
    }

    // Samples the full piecewise curve in local pixels. A single sampled polyline feeds both the
    // filled-under-curve polygon and the visible stroke, which keeps draw work coherent.
    /// Samples each segment into one ordered polyline for the shared painter.
    fn sample_curve_local(&self, graph: Recti, steps_per_segment: usize) -> Vec<Vec2f> {
        let steps = steps_per_segment.max(4);
        let mut points = Vec::with_capacity((self.nodes.len() - 1) * steps + 1);
        points.push(Self::graph_to_local(graph, self.nodes[0].pos));
        for seg in 0..self.nodes.len() - 1 {
            for step in 1..=steps {
                let t = step as f32 / steps as f32;
                points.push(Self::graph_to_local(graph, self.eval_segment(seg, t)));
            }
        }
        points
    }

    // Hit testing is resolved in local pixel space because markers are displayed in pixels, not in
    // normalized graph units. Only draggable controls participate.
    /// Chooses a nearby handle or anchor using the existing marker hit priorities.
    fn pick_target(&self, graph: Recti, mouse_local: Vec2f) -> Option<FalloffTarget> {
        let mut best = None;
        let mut best_dist_sq = FALLOFF_PICK_RADIUS * FALLOFF_PICK_RADIUS;

        for idx in 0..self.nodes.len() {
            if idx > 0 && idx + 1 < self.nodes.len() {
                let target = FalloffTarget::Anchor(idx);
                let pos = self.target_local(graph, target);
                let dist_sq = (pos - mouse_local).length_squared();
                if dist_sq <= best_dist_sq {
                    best = Some(target);
                    best_dist_sq = dist_sq;
                }
            }

            if idx > 0 {
                let target = FalloffTarget::InHandle(idx);
                let pos = self.target_local(graph, target);
                let dist_sq = (pos - mouse_local).length_squared();
                if dist_sq <= best_dist_sq {
                    best = Some(target);
                    best_dist_sq = dist_sq;
                }
            }

            if idx + 1 < self.nodes.len() {
                let target = FalloffTarget::OutHandle(idx);
                let pos = self.target_local(graph, target);
                let dist_sq = (pos - mouse_local).length_squared();
                if dist_sq <= best_dist_sq {
                    best = Some(target);
                    best_dist_sq = dist_sq;
                }
            }
        }

        best
    }

    // Dragging writes directly back into the normalized representation. Anchor x is clamped
    // between neighboring anchors, while handle x updates only the relative factor that belongs to
    // its segment.
    /// Applies a normalized drag to the selected field and restores graph invariants.
    fn drag_target(&mut self, target: FalloffTarget, point: Vec2f) {
        match target {
            FalloffTarget::Anchor(idx) => {
                if idx == 0 || idx + 1 == self.nodes.len() {
                    return;
                }

                let left = self.nodes[idx - 1].pos.x + FALLOFF_MIN_NODE_GAP;
                let right = self.nodes[idx + 1].pos.x - FALLOFF_MIN_NODE_GAP;
                self.nodes[idx].pos.x = if left <= right { point.x.clamp(left, right) } else { (left + right) * 0.5 };
                self.nodes[idx].pos.y = point.y.clamp(0.0, 1.0);
            }
            FalloffTarget::InHandle(idx) => {
                if idx == 0 {
                    return;
                }
                let anchor = self.nodes[idx].pos;
                let prev = self.nodes[idx - 1].pos;
                let span = (anchor.x - prev.x).max(FALLOFF_MIN_NODE_GAP * 0.25);
                self.nodes[idx].in_x = ((anchor.x - point.x) / span).clamp(0.0, FALLOFF_HANDLE_X_MAX);
                self.nodes[idx].in_y = point.y.clamp(0.0, 1.0);
            }
            FalloffTarget::OutHandle(idx) => {
                if idx + 1 >= self.nodes.len() {
                    return;
                }
                let anchor = self.nodes[idx].pos;
                let next = self.nodes[idx + 1].pos;
                let span = (next.x - anchor.x).max(FALLOFF_MIN_NODE_GAP * 0.25);
                self.nodes[idx].out_x = ((point.x - anchor.x) / span).clamp(0.0, FALLOFF_HANDLE_X_MAX);
                self.nodes[idx].out_y = point.y.clamp(0.0, 1.0);
            }
        }

        self.sanitize();
    }
}

impl LeafContent for FalloffEditor {
    /// Pointer capture participates without adding an actionless Tab stop.
    const KEYBOARD: u16 = 1;
    /// The curve is normalized, so model edits never change preferred dimensions.
    fn measure(&self, _: LayoutLimits, _: ComRef<'_, ITextMetricsAbi>) -> Result<Size, NcStatus> {
        Ok(Size { width: 300, height: 220 })
    }

    /// Maintains capture and hit state from copied input; edits change paint only.
    fn update(&mut self, state: WidgetUpdate, event: WidgetEvent, _: &str) -> Result<GeometryChange, NcStatus> {
        let bounds = local_rect(state.rect);
        let graph = Self::graph_rect(bounds);
        if graph.width <= 0 || graph.height <= 0 {
            return Ok(GeometryChange::NO_CHANGE);
        }

        if !state.focused && !state.active {
            self.active = None;
        }

        let pointer = match event {
            WidgetEvent::Move { position, delta } | WidgetEvent::Drag { position, delta, .. } => Some((point(position), point(delta))),
            WidgetEvent::Down { position, .. } | WidgetEvent::Up { position, .. } | WidgetEvent::Scroll { position, .. } => {
                Some((point(position), Vec2i::default()))
            }
            _ => None,
        };
        let Some((mouse_pos, mouse_delta)) = pointer else {
            return Ok(GeometryChange::NO_CHANGE);
        };
        let mouse_local = Vec2f::new(mouse_pos.x as f32, mouse_pos.y as f32);
        self.hovered = if state.hovered { self.pick_target(graph, mouse_local) } else { None };

        if state.clicked {
            self.active = if graph.contains(&Vec2i::new(mouse_local.x as i32, mouse_local.y as i32)) {
                self.pick_target(graph, mouse_local)
            } else {
                None
            };
        } else if !state.active {
            self.active = None;
        }

        if state.active
            && (mouse_delta.x != 0 || mouse_delta.y != 0)
            && let Some(target) = self.active
        {
            let point = Self::local_to_graph(graph, mouse_local);
            self.drag_target(target, point);
        }

        // Geometry is unchanged here; mutations through other handles invalidate their owners.
        Ok(GeometryChange::NO_CHANGE)
    }

    /// Records the graph, curve and explicit control markers under a narrowed graph clip.
    fn paint(&self, state: WidgetPaint, painter: &Painter<'_>) -> Result<(), NcStatus> {
        let bounds = local_rect(state.rect);
        let graph = Self::graph_rect(bounds);
        if graph.width <= 0 || graph.height <= 0 {
            return Ok(());
        }

        let curve = self.sample_curve_local(graph, FALLOFF_SEGMENT_STEPS);

        {
            let g = painter;
            let local = local_rect(state.rect);
            let background = [
                Vec2f::new(0.0, 0.0),
                Vec2f::new(local.width as f32, 0.0),
                Vec2f::new(local.width as f32, local.height as f32),
                Vec2f::new(0.0, local.height as f32),
            ];
            fill(g, background.as_slice(), color(25, 29, 34, 255))?;
            for (start, end) in rect_edges(rect(6, 6, (local.width - 12).max(0), (local.height - 12).max(0)))
                .into_iter()
                .flatten()
            {
                stroke(g, start, end, 1.5, color(62, 68, 76, 255))?;
            }
            for (start, end) in rect_edges(graph).into_iter().flatten() {
                stroke(g, start, end, 1.5, color(88, 96, 106, 255))?;
            }

            {
                let clipped = g.clipped(wire_rect(graph))?;
                let g = &clipped;
                for idx in 1..4 {
                    let x = graph.x as f32 + graph.width as f32 * idx as f32 / 4.0;
                    let y = graph.y as f32 + graph.height as f32 * idx as f32 / 4.0;
                    stroke(
                        g,
                        Vec2f::new(x, graph.y as f32),
                        Vec2f::new(x, (graph.y + graph.height) as f32),
                        1.0,
                        color(46, 53, 60, 255),
                    )?;
                    stroke(
                        g,
                        Vec2f::new(graph.x as f32, y),
                        Vec2f::new((graph.x + graph.width) as f32, y),
                        1.0,
                        color(46, 53, 60, 255),
                    )?;
                }

                let mut area = Vec::with_capacity(curve.len() + 2);
                area.extend(curve.iter().copied());
                area.push(Vec2f::new((graph.x + graph.width) as f32, (graph.y + graph.height) as f32));
                area.push(Vec2f::new(graph.x as f32, (graph.y + graph.height) as f32));
                fill(g, area.as_slice(), color(74, 156, 216, 70))?;

                for pair in curve.windows(2) {
                    stroke(g, pair[0], pair[1], 3.0, color(111, 205, 251, 255))?;
                }
            }

            for seg in 0..self.nodes.len() - 1 {
                let anchor_a = Self::graph_to_local(graph, self.nodes[seg].pos);
                let anchor_b = Self::graph_to_local(graph, self.nodes[seg + 1].pos);
                let handle_a = Self::graph_to_local(graph, self.out_handle_graph(seg));
                let handle_b = Self::graph_to_local(graph, self.in_handle_graph(seg + 1));
                stroke(g, anchor_a, handle_a, 1.5, color(154, 122, 88, 255))?;
                stroke(g, handle_b, anchor_b, 1.5, color(154, 122, 88, 255))?;
            }

            for idx in 0..self.nodes.len() {
                if idx > 0 {
                    let target = FalloffTarget::InHandle(idx);
                    let center = self.target_local(graph, target);
                    let radius = if self.active == Some(target) {
                        6.0
                    } else if self.hovered == Some(target) {
                        5.0
                    } else {
                        4.0
                    };
                    let marker = build_diamond_polygon(center, radius);
                    fill(g, marker.as_slice(), color(235, 194, 92, 255))?;
                }

                if idx + 1 < self.nodes.len() {
                    let target = FalloffTarget::OutHandle(idx);
                    let center = self.target_local(graph, target);
                    let radius = if self.active == Some(target) {
                        6.0
                    } else if self.hovered == Some(target) {
                        5.0
                    } else {
                        4.0
                    };
                    let marker = build_diamond_polygon(center, radius);
                    fill(g, marker.as_slice(), color(235, 194, 92, 255))?;
                }

                let center = Self::graph_to_local(graph, self.nodes[idx].pos);
                let target = FalloffTarget::Anchor(idx);
                let radius = if idx == 0 || idx + 1 == self.nodes.len() {
                    4.5
                } else if self.active == Some(target) {
                    6.5
                } else if self.hovered == Some(target) {
                    5.5
                } else {
                    5.0
                };
                let marker = build_square_polygon(center, radius);
                let color = if idx == 0 || idx + 1 == self.nodes.len() {
                    color(220, 228, 236, 255)
                } else {
                    color(250, 250, 250, 255)
                };
                fill(g, marker.as_slice(), color)?;
            }
        }
        Ok(())
    }
}
