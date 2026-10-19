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
//! Statically dispatched algorithms owned only by the built-in nodes provider.
//!
//! The phase projections in widget_object invoke these traits on concrete node values.
//! Core receives only generated IWidget/ILeafLayout/IContainerLayout capabilities, never
//! these Rust algorithms, their fields, or a native widget representation.
use crate::{AtlasHandle, Change, Constraints, Dimensioni, FrameRole, KeyboardBehavior, Skin, SurfaceRole, UiInputEvent, WidgetPaintCtx, WidgetUpdateCtx};
use bitflags::bitflags;

bitflags! {
    #[derive(Copy, Clone)]
    /// Controls which widget states should draw a filled background.
    pub struct WidgetFillOption : u32 {
        /// Fill the background for the idle/normal state.
        const NORMAL = 1;
        /// Fill the background while hovered.
        const HOVER = 2;
        /// Fill the background while the widget owns focus.
        const CLICK = 4;
        /// Fill the background for every interaction state.
        const ALL = Self::NORMAL.bits() | Self::HOVER.bits() | Self::CLICK.bits();
    }
}

/// Common retained runtime phase contract implemented by concrete widgets.
///
/// Widgets participate in the retained update and paint phases. Geometry is deliberately split by
/// node kind: [`LeafWidget`] measures intrinsic content, while a scoped container algorithm measures
/// and places an authoritative child collection. A branch therefore has no meaningless leaf
/// measurement implementation.
///
/// The runtime routes an event before running that event's complete update traversal, then commits
/// layout before considering the next queued event. [`Widget::keyboard_behavior`] is the sole
/// keyboard-eligibility query. Event-kind filtering belongs to the dispatcher for ordinary widgets
/// and to a scoped container algorithm for an overloaded container surface.
pub trait Widget {
    /// Updates retained widget state for exactly one normalized input event.
    ///
    /// Pointer positions in `input` are relative to the widget's derived content rectangle, using
    /// the same origin as [`WidgetUpdateCtx::local_rect`]. Outer frame pixels remain part of the
    /// runtime hit target, so a pointer event on the border may lie just outside the local content
    /// bounds. At most one eligible widget receives `Some(input)` during a traversal; every other
    /// eligible widget receives `None`. Held state is available from [`WidgetUpdateCtx`]. The
    /// update context intentionally cannot record drawing commands. Implementations that retain a
    /// local drag mode must reconcile it from [`WidgetUpdateCtx::active`] on every call; pointer
    /// capture is runtime-owned and has no separate widget lifecycle callback. Delivery of a
    /// consumed or captured event alone does not invalidate measurement. Return [`Change::Changed`]
    /// whenever this call changes inputs read by measurement or child placement, including on a
    /// `None` update (animation, shared-state synchronization, or committing an editor on blur).
    /// The runtime propagates that invalidation upward before the next input is routed.
    ///
    /// [`Change::NoChange`] does not suppress event dispatch, painting, descendant updates, or
    /// invalidation already recorded through typed handles. Resize constraints, skin revision,
    /// topology edits, and handle mutations are tracked independently. Widget authors must report
    /// their own changes truthfully; app authors must synchronize external/shared state through a
    /// typed handle or a widget update that reports `Changed`. Out-of-band interior mutation is
    /// not observable by the runtime. Hidden/clipped widgets are not guaranteed an update, so
    /// shared geometry state should be synchronized through handles before it must affect layout.
    /// Return errors directly with `?`, including snapshot construction and delivery failures.
    /// Already committed semantic edits are not rolled back. Before fallible work, record any
    /// committed geometry change in the widget's `WidgetState` so `IWidget::take_change` can
    /// recover it even when this call returns an error. Stop emitting after the first failure;
    /// the context has no separate error accumulator or completion operation.
    /// The default is suitable for widgets with no update behavior.
    fn update(&mut self, _ctx: &mut WidgetUpdateCtx<'_>, _input: Option<&UiInputEvent>) -> Result<Change, neocogi_com::NcStatus> {
        // Static widgets retain valid measurement until an independently tracked input changes.
        Ok(Change::NoChange)
    }
    /// Records paint commands through paint-only capabilities.
    ///
    /// Paint is observational with respect to application-authored semantic state, topology,
    /// interaction, and committed layout. Rendering caches belong to the owning COM object's
    /// phase machinery rather than the semantic widget implementation. Painting may therefore
    /// neither alter the current commit nor publish application-coordination events. Mutating
    /// retained UI through an independently captured typed widget handle is a contract violation
    /// rather than a deferred-next-frame operation.
    fn paint(&self, ctx: &mut WidgetPaintCtx<'_>);
    /// Drains geometry changes from explicit shared semantic models owned by a composite.
    /// This hook only consumes notification flags; it must not edit UI state or topology.
    /// Ordinary widgets report changes through update/setters and keep the default false.
    fn take_shared_change(&self) -> bool {
        false
    }

    /// Returns the semantic appearance used when [`crate::WidgetOption::FRAME`] is effective.
    ///
    /// The runtime uses this one role for measurement, layout, input localization, and paint. A
    /// custom widget therefore changes themed border geometry without duplicating or bypassing the
    /// retained outer/content-box contract.
    fn frame_appearance_role(&self) -> FrameRole {
        // Generic framing remains the neutral default for application widgets that request FRAME
        // without opting into one of the built-in control meanings.
        FrameRole::Surface(SurfaceRole::GenericFrame)
    }
    /// Returns declarative keyboard routing capabilities for this widget surface.
    ///
    /// Keyboard focus is persistent and owned by retained routing rather than by pointer capture.
    /// Override this method for focusable custom widgets; the default keeps structural drawing and
    /// structural containers out of Tab order without another opt-out flag.
    fn keyboard_behavior(&self) -> KeyboardBehavior {
        KeyboardBehavior::NONE
    }
}

/// Intrinsic geometry contract for a retained leaf widget.
///
/// Each constraint axis explicitly distinguishes a finite maximum from an unbounded preferred-size
/// query. Returned components are clamped to zero before the node's frame is applied.
pub trait LeafWidget: Widget {
    /// Returns this leaf's preferred content size.
    fn measure(&self, skin: &Skin, atlas: &AtlasHandle, constraints: Constraints) -> Dimensioni;
}
