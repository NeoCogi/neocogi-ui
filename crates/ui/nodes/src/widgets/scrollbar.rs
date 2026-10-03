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
//! Standalone retained one-axis scrollbar widget and its shared thumb geometry.
//!
//! The widget owns range state, input, events, and paint while its parent remains responsible for
//! allocating the track. Shared geometry keeps paint, dragging, and track clicks consistent.
use crate::{ControlRole};

use crate::math::{clamp_i64_to_i32, RectExt};
use crate::{Dimensioni, MouseButton, Recti, UiInputEvent, Vec2i, Widget, WidgetOption, WidgetPaintCtx, WidgetUpdateCtx};

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
/// Axis selector used by shared scrollbar geometry helpers.
pub enum ScrollbarAxis {
    /// Vertical scrollbar operating on y offsets.
    Vertical,
    /// Horizontal scrollbar operating on x offsets.
    Horizontal,
}

impl ScrollbarAxis {
    /// Returns the rectangle extent along this scrollbar's movement axis.
    fn rect_len(self, rect: Recti) -> i32 {
        match self {
            Self::Vertical => rect.height,
            Self::Horizontal => rect.width,
        }
    }

    /// Returns one point component along this scrollbar's movement axis.
    fn point(self, point: Vec2i) -> i32 {
        match self {
            Self::Vertical => point.y,
            Self::Horizontal => point.x,
        }
    }

    /// Returns the rectangle origin along this scrollbar's movement axis.
    fn origin(self, rect: Recti) -> i32 {
        match self {
            Self::Vertical => rect.y,
            Self::Horizontal => rect.x,
        }
    }

    /// Replaces the rectangle extent along this scrollbar's movement axis.
    fn with_len(self, mut rect: Recti, len: i32) -> Recti {
        match self {
            Self::Vertical => rect.height = len,
            Self::Horizontal => rect.width = len,
        }
        rect
    }

    /// Translates the rectangle origin along this scrollbar's movement axis.
    fn translate(self, mut rect: Recti, amount: i32) -> Recti {
        // translated_axis_origin = rectangle_axis_origin + amount.
        match self {
            Self::Vertical => rect.y = rect.y.saturating_add(amount),
            Self::Horizontal => rect.x = rect.x.saturating_add(amount),
        }
        rect
    }
}

/// Complete mapping between one scrollbar track and one content axis.
///
/// `thumb`, [`Self::drag_delta`], and [`Self::centered_offset`] all use the same scroll range and
/// thumb travel. Keeping those values together is important when the minimum thumb length is
/// larger than the proportional thumb.
#[derive(Copy, Clone, Debug)]
pub(crate) struct ScrollbarGeometry {
    /// Axis used to select rectangle and pointer components.
    axis: ScrollbarAxis,
    /// Complete local track rectangle.
    track: Recti,
    /// Proportional, minimum-bounded thumb rectangle inside `track`.
    thumb: Recti,
    /// Largest valid content offset.
    max_offset: i32,
    /// Number of pixels through which the thumb can move.
    thumb_travel: i32,
}

/// Multiplies before dividing in a wider domain and clamps the signed result to one coordinate.
fn scaled_ratio(value: i32, numerator: i32, denominator: i32) -> i32 {
    debug_assert!(numerator >= 0 && denominator > 0);
    // Every i32 product fits i64. Dividing before narrowing preserves proportional mapping at
    // large content ranges instead of saturating the intermediate product and destroying its ratio.
    clamp_i64_to_i32(i64::from(value) * i64::from(numerator) / i64::from(denominator))
}

impl ScrollbarGeometry {
    /// Resolves one track and thumb for the supplied visible/content lengths and offset.
    pub(crate) fn new(axis: ScrollbarAxis, track: Recti, view_len: i32, content_len: i32, offset: i32, min_thumb_len: i32) -> Self {
        // Normalize every external length once so later mapping functions operate on valid ranges.
        let track_len = axis.rect_len(track).max(0);
        let view_len = view_len.max(0);
        let content_len = content_len.max(0);
        let max_offset = scrollbar_max_scroll(content_len, view_len);

        // Proportional length represents the visible fraction; the style minimum preserves usability.
        let proportional = if content_len > 0 {
            // proportional_thumb_length = track_length * visible_length / content_length.
            scaled_ratio(track_len, view_len, content_len)
        } else {
            track_len
        };
        let thumb_len = proportional.max(min_thumb_len.max(0)).min(track_len);
        // thumb_travel = track_length - thumb_length.
        let thumb_travel = track_len.saturating_sub(thumb_len).max(0);
        // Map clamped content offset into thumb travel using the same integer ratio inverted by drag.
        let thumb_offset = if max_offset > 0 && thumb_travel > 0 {
            // thumb_offset = clamped_content_offset * thumb_travel / maximum_content_offset.
            scaled_ratio(offset.clamp(0, max_offset), thumb_travel, max_offset)
        } else {
            0
        };
        let thumb = axis.translate(axis.with_len(track, thumb_len), thumb_offset);

        Self {
            axis,
            track,
            thumb,
            max_offset,
            thumb_travel,
        }
    }

    /// Returns the full scrollbar track.
    pub(crate) fn track(self) -> Recti {
        self.track
    }

    /// Returns the painted and hit-tested thumb rectangle.
    pub(crate) fn thumb(self) -> Recti {
        self.thumb
    }

    /// Converts pointer movement into the exactly inverse content-offset movement.
    pub(crate) fn drag_delta(self, delta: Vec2i) -> i32 {
        // A non-scrollable range or immobile thumb cannot produce a meaningful content delta.
        if self.thumb_travel <= 0 || self.max_offset <= 0 {
            return 0;
        }
        // content_delta = pointer_delta * maximum_content_offset / thumb_travel.
        scaled_ratio(self.axis.point(delta), self.max_offset, self.thumb_travel)
    }

    /// Returns the offset that centers the thumb on `pointer`, clamped to the track.
    pub(crate) fn centered_offset(self, pointer: Vec2i) -> i32 {
        // Track clicks use the same travel/range mapping as dragging, centered on the thumb.
        if self.thumb_travel <= 0 || self.max_offset <= 0 {
            return 0;
        }
        let thumb_len = self.axis.rect_len(self.thumb);
        // track_pointer = pointer_axis_position - track_axis_origin.
        let track_pointer = self.axis.point(pointer).saturating_sub(self.axis.origin(self.track));
        // centered_thumb = clamp(track_pointer - thumb_length / 2, 0, thumb_travel).
        let centered = track_pointer.saturating_sub(thumb_len / 2).clamp(0, self.thumb_travel);
        // content_offset = centered_thumb * maximum_content_offset / thumb_travel.
        scaled_ratio(centered, self.max_offset, self.thumb_travel)
    }
}

/// Returns the largest scroll offset needed to reveal all content.
pub(crate) fn scrollbar_max_scroll(content_len: i32, view_len: i32) -> i32 {
    // maximum_scroll = max(content_length - visible_length, 0).
    content_len.saturating_sub(view_len).max(0)
}

/// One-shot construction input for a standalone [`Scrollbar`].
pub struct ScrollbarParameters {
    /// Axis along which the scrollbar changes its content offset.
    pub axis: ScrollbarAxis,
    /// Initial visible content length represented by the complete track.
    pub viewport_len: i32,
    /// Initial total content length represented by the track and thumb.
    pub content_len: i32,
    /// Initial content offset, clamped to the configured range.
    pub offset: i32,
    /// Base widget options applied in addition to focus-preserving pointer interaction.
    pub opt: WidgetOption,
}

impl ScrollbarParameters {
    /// Creates an empty scrollbar on `axis`; callers or a containing layout can install lengths later.
    pub fn new(axis: ScrollbarAxis) -> Self {
        // An empty range keeps a newly mounted standalone scrollbar inert until its owner provides
        // meaningful viewport and content lengths.
        Self {
            axis,
            viewport_len: 0,
            content_len: 0,
            offset: 0,
            opt: WidgetOption::NONE,
        }
    }

    /// Replaces the initial viewport length, content length, and requested offset.
    pub const fn range(mut self, viewport_len: i32, content_len: i32, offset: i32) -> Self {
        // Preserve the caller's raw request here; Scrollbar construction performs one authoritative
        // normalization and clamp after all parameters have been assembled.
        self.viewport_len = viewport_len;
        self.content_len = content_len;
        self.offset = offset;
        self
    }
}

#[neocogi_com::com_class(neocogi_ui_nodes_contracts::Scrollbar, ownership = exclusive)]
/// Standalone one-axis scrollbar and dynamically constructible COM object.
pub struct Scrollbar {
    /// Non-semantic COM lifecycle and host-backed phase resources.
    runtime: crate::widget_object::WidgetRuntime,
    /// Immutable axis used to project pointer and layout coordinates.
    axis: ScrollbarAxis,
    /// One authoritative range/offset allocation, also shared by composed scroll layout when present.
    pub(crate) axis_state: std::rc::Rc<ScrollAxisState>,
    /// Caller-authored options; dynamic range policy is resolved from the same shared state.
    /// Typed source for user-originated movement; programmatic requests remain silent.
    events: crate::widget_object::WidgetEvents<neocogi_ui_nodes_contracts::ScrollbarEventQueueAbi>,
}

/// Coherent copied values shared by a real scrollbar and its co-packaged scroll layout.
#[derive(Clone, Copy, Default, PartialEq, Eq)]
struct ScrollValues {
    /// Nonnegative length visible through the viewport.
    viewport: i32,
    /// Nonnegative full content extent.
    content: i32,
    /// Nonnegative requested offset, clamped when a new authoritative range installs.
    offset: i32,
}

/// Concrete scalar model has no widget, parent, runtime or child-collection references.
/// Sharing this data within the UI plugin permits derived range installation during layout
/// without invoking a semantic COM setter or creating a widget ownership cycle.
#[derive(Default)]
pub(crate) struct ScrollAxisState {
    /// The complete range publishes in one copied assignment.
    values: std::cell::Cell<ScrollValues>,
    /// Semantic requests persist until the containing scroll layout collects them.
    dirty: std::cell::Cell<bool>,
}
impl ScrollAxisState {
    /// Returns the current request; layout may still clamp it against newly measured content.
    pub(crate) fn offset(&self) -> i32 {
        self.values.get().offset
    }
    /// Derives the maximum instead of retaining a second copy of the range.
    pub(crate) fn maximum(&self) -> i32 {
        let values = self.values.get();
        scrollbar_max_scroll(values.content, values.viewport)
    }
    /// Records an ordinary semantic request, including requests above an old range.
    pub(crate) fn request_offset(&self, offset: i32) {
        let mut values = self.values.get();
        let offset = offset.max(0);
        if values.offset != offset {
            values.offset = offset;
            self.values.set(values);
            self.dirty.set(true);
        }
    }
    /// Installs parent-derived geometry without creating another layout request from its own output.
    pub(crate) fn install(&self, viewport: i32, content: i32, requested: i32) {
        let viewport = viewport.max(0);
        let content = content.max(0);
        let offset = requested.clamp(0, scrollbar_max_scroll(content, viewport));
        self.values.set(ScrollValues { viewport, content, offset });
    }
    /// Clears a hidden structural bar's derived range and interaction offset.
    pub(crate) fn deactivate(&self) {
        self.install(0, 0, 0);
    }
    /// Drains semantic shared-state invalidation independently of ordinary widget update participation.
    pub(crate) fn take_change(&self) -> bool {
        self.dirty.replace(false)
    }
}

impl Scrollbar {
    /// Normalizes a complete range and combines it with one already bound common runtime.
    pub(crate) fn from_runtime(runtime: crate::widget_object::WidgetRuntime, parameters: ScrollbarParameters) -> Self {
        // Normalize all range inputs before mounting so the first measure, update, and paint phases
        // observe one coherent range and offset.
        let viewport_len = parameters.viewport_len.max(0);
        let content_len = parameters.content_len.max(0);
        let axis_state = std::rc::Rc::new(ScrollAxisState::default());
        axis_state.install(viewport_len, content_len, parameters.offset);
        Self {
            runtime,
            axis: parameters.axis,
            axis_state,
            events: Default::default(),
        }
    }
}

#[cfg(test)]
impl From<ScrollbarParameters> for crate::test_support::algorithm::Algorithm<Scrollbar> {
    /// Builds the same concrete scrollbar against a valid native test host.
    fn from(parameters: ScrollbarParameters) -> Self {
        Self::new(parameters.opt, |runtime| Scrollbar::from_runtime(runtime, parameters))
    }
}

impl Scrollbar {
    /// Returns the axis configured when this scrollbar was constructed.
    pub const fn axis(&self) -> ScrollbarAxis {
        // Axis is immutable semantic configuration, so returning it never depends on layout state.
        self.axis
    }

    /// Installs non-negative viewport and content lengths and clamps the previous offset.
    pub fn set_lengths(&mut self, viewport_len: i32, content_len: i32) {
        // Normalize both external lengths together before deriving a maximum. This prevents paint
        // and pointer input from observing a partially updated range.
        let previous = self.axis_state.values.get();
        self.axis_state.install(viewport_len, content_len, previous.offset);
        if self.axis_state.values.get() != previous {
            self.axis_state.dirty.set(true);
        }
    }

    /// Returns the current clamped content offset on this axis.
    pub fn offset(&self) -> i32 {
        // Offset is clamped by construction, range updates, and user interaction.
        self.axis_state.offset()
    }

    /// Requests a non-negative offset that the next range update can clamp further.
    pub fn set_offset(&mut self, offset: i32) {
        // Preserve an above-range request so a containing layout can set an offset before installing
        // newly measured lengths. The next set_lengths call performs the authoritative upper clamp.
        self.axis_state.request_offset(offset);
    }

    /// Returns the largest offset supported by the current viewport and content lengths.
    pub fn max_offset(&self) -> i32 {
        // Derive the maximum instead of retaining a second range value that could become stale.
        self.axis_state.maximum()
    }

    /// Resolves track and thumb geometry from the current allocation and style-owned minimum.
    fn geometry(&self, track: Recti, min_thumb_len: i32) -> ScrollbarGeometry {
        // The retained node allocation is the standalone widget's authoritative track. Range and
        // offset remain semantic state, so no containing layout must write widget-local rectangles.
        let values = self.axis_state.values.get();
        ScrollbarGeometry::new(self.axis, track, values.viewport, values.content, values.offset, min_thumb_len)
    }
}

impl Widget for Scrollbar {
    /// Applies track clicks and captured thumb dragging to the current offset.
    fn update(&mut self, ctx: &mut WidgetUpdateCtx<'_>, input: Option<&UiInputEvent>) -> crate::Change {
        // Resolve one geometry snapshot from this widget's local allocation so hit testing and drag
        // conversion use the exact same track and thumb for the complete event.
        let geometry = self.geometry(ctx.local_rect(), ctx.skin().metrics().thumb_size.max(0));
        let previous = self.offset();
        match input {
            Some(UiInputEvent::MouseDown { pos, button }) if button.intersects(MouseButton::LEFT) && geometry.track().contains_point(*pos) => {
                // Clicking outside the thumb recenters it. Runtime capture established for this
                // press supplies the complete drag lease through `ctx.active()` below.
                if !geometry.thumb().contains_point(*pos) {
                    self.axis_state.request_offset(geometry.centered_offset(*pos));
                }
            }
            Some(UiInputEvent::MouseDrag { delta, .. }) if ctx.active() => {
                // Only the router-owned capture recipient is active. This continues beyond
                // the track rectangle without retaining a second widget-local capture flag.
                // offset = clamp(previous_offset + drag_delta, 0, maximum_offset).
                self.axis_state
                    .request_offset(self.offset().saturating_add(geometry.drag_delta(*delta)).clamp(0, self.max_offset()));
            }
            _ => {}
        }
        if self.offset() != previous {
            // Publish only user-originated movement. Programmatic setters intentionally remain
            // silent so owners can synchronize range and offset without recursive notifications.
            ctx.posted(self.events.post(&neocogi_ui_nodes_contracts::ScrollbarEvent::Changed(crate::ScrollbarChanged {
                offset: self.offset(),
            })));
        }

        // A containing ScrollArea reads this offset when placing its content.
        crate::Change::from(self.offset() != previous)
    }

    /// Paints the track and proportional thumb inside the complete local allocation.
    fn paint(&self, ctx: &mut WidgetPaintCtx<'_>) {
        // Rebuild geometry from the paint allocation and the same semantic range used by update.
        // Hidden composite children never reach this phase, while mounted zero-range bars display a
        // full-length thumb that communicates the absence of overflow.
        let geometry = self.geometry(ctx.local_rect(), ctx.skin().metrics().thumb_size.max(0));
        let _ = ctx.draw_control(ControlRole::ScrollbarTrack, geometry.track());
        let _ = ctx.draw_control(ControlRole::ScrollbarThumb, geometry.thumb());
    }
}

impl crate::LeafWidget for Scrollbar {
    /// Reports zero length along the scroll axis and style-owned thickness across it.
    fn measure(&self, style: &crate::Skin, _atlas: &crate::AtlasHandle, _constraints: crate::Constraints) -> Dimensioni {
        // A containing layout supplies track length explicitly; intrinsic measurement contributes
        // only the cross-axis thickness required by standalone layout composition.
        let thickness = style.metrics().scrollbar_size.max(0);
        match self.axis {
            ScrollbarAxis::Horizontal => Dimensioni::new(0, thickness),
            ScrollbarAxis::Vertical => Dimensioni::new(thickness, 0),
        }
    }
}

impl crate::widget_object::WidgetAlgorithm for Scrollbar {

    /// A zero range paints a full thumb but must not establish meaningless pointer capture.
    fn effective_options(&self, options: WidgetOption) -> WidgetOption {
        if self.max_offset() > 0 {
            options
        } else {
            options | WidgetOption::NO_INTERACT
        }
    }
}

impl crate::widget_object::HostedWidget for Scrollbar {
    /// Borrows common lifecycle and resource state without a second scrollbar representation.
    fn runtime(&self) -> &crate::widget_object::WidgetRuntime {
        &self.runtime
    }

    /// Exclusively borrows common lifecycle and resource state for a COM mutation or phase.
    fn runtime_mut(&mut self) -> &mut crate::widget_object::WidgetRuntime {
        &mut self.runtime
    }
}

impl Scrollbar {
    /// Copies range metadata without introducing a second source of scrollbar truth.
    pub(crate) fn com_lengths(&self) -> (i32, i32) {
        let values = self.axis_state.values.get();
        (values.viewport, values.content)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn minimum_thumb_drag_uses_the_same_inverse_range_as_paint() {
        let track = Recti::new(10, 20, 8, 100);
        let geometry = ScrollbarGeometry::new(ScrollbarAxis::Vertical, track, 20, 200, 90, 40);

        let thumb = geometry.thumb();
        assert_eq!((thumb.x, thumb.y, thumb.width, thumb.height), (10, 50, 8, 40));
        assert_eq!(geometry.drag_delta(Vec2i::new(0, 30)), 90);

        let moved = ScrollbarGeometry::new(ScrollbarAxis::Vertical, track, 20, 200, 180, 40);
        assert_eq!(moved.thumb().y, geometry.thumb().y + 30);
    }

    #[test]
    fn centered_track_click_clamps_to_both_ends() {
        let track = Recti::new(4, 6, 100, 8);
        let geometry = ScrollbarGeometry::new(ScrollbarAxis::Horizontal, track, 25, 100, 0, 20);

        assert_eq!(geometry.centered_offset(Vec2i::new(-100, 8)), 0);
        assert_eq!(geometry.centered_offset(Vec2i::new(104, 8)), 75);
    }

    /// Verifies large content ranges retain their proportional thumb position and inverse drag
    /// mapping instead of saturating the multiplication before division.
    #[test]
    fn extreme_content_range_preserves_scroll_ratios() {
        let track = Recti::new(10, 20, 300, 8);
        let view_len = 300;
        let content_len = i32::MAX;
        let max_offset = scrollbar_max_scroll(content_len, view_len);
        let offset = max_offset / 2;
        let geometry = ScrollbarGeometry::new(ScrollbarAxis::Horizontal, track, view_len, content_len, offset, 8);
        let thumb = geometry.thumb();

        assert_eq!(thumb.width, 8);
        assert!(
            (i64::from(thumb.x) - i64::from(10 + 146)).abs() <= 1,
            "integer division may place the half-range thumb one pixel before the exact midpoint"
        );
        let inverse = geometry.drag_delta(Vec2i::new(146, 0));
        assert!(
            (i64::from(inverse) - i64::from(offset)).abs() <= 1,
            "half-track drag must map back to half the content range"
        );
        assert!((i64::from(geometry.centered_offset(Vec2i::new(10 + 146 + 4, 20))) - i64::from(offset)).abs() <= 1);
    }
}

use crate::widget_object::{HostedWidget, direct_leaf_layout_impl, direct_widget_phase_impl};
use neocogi_ui_core_contracts::{IWidgetHostAbi};
use neocogi_ui_nodes_contracts::{ScrollAxis, ScrollRange, ScrollbarAbi};
use neocogi_com::*;

direct_widget_phase_impl!(Scrollbar);
direct_leaf_layout_impl!(Scrollbar);

impl Scrollbar {
    /// Converts an already host-bound structural scrollbar into its public COM capability.
    pub(crate) fn into_com(self) -> neocogi_com::ComBox<ScrollbarAbi> {
        Self::into_box(self).query().expect("Scrollbar declares its callable class contract")
    }

    /// Constructs the class for the host-bound factory through the same typed contract used by the registry.
    pub(crate) fn create_com(
        host: ComPtr<IWidgetHostAbi>,
        resources: crate::widget_object::SharedAssets,
        range: ScrollRange,
        queue: Option<ComPtr<neocogi_ui_nodes_contracts::ScrollbarEventQueueAbi>>,
    ) -> Result<neocogi_com::ComBox<ScrollbarAbi>, NcStatus> {
        let mut value = Self::build(host, range, Some(&resources))?;
        // Fix the sole destination before publishing the completed widget.
        Self::into_box_cyclic::<ScrollbarAbi>(move |origin| {
            value.events = crate::widget_object::WidgetEvents::with_queue(queue, origin);
            value
        })?
        .query().map_err(|(status, _unpublished)| status)
    }

    /// Builds one scrollbar with either directly imported or runtime-shared resources.
    fn build(host: ComPtr<IWidgetHostAbi>, range: ScrollRange, resources: Option<&crate::widget_object::SharedAssets>) -> Result<Self, NcStatus> {
        let axis = match range.axis {
            ScrollAxis::Horizontal => crate::ScrollbarAxis::Horizontal,
            ScrollAxis::Vertical => crate::ScrollbarAxis::Vertical,
            _ => return Err(NC_E_INVALID_ARG),
        };
        if range.viewport < 0 || range.content < 0 {
            return Err(NC_E_INVALID_ARG);
        }
        let parameters = crate::ScrollbarParameters::new(axis).range(range.viewport, range.content, range.offset);
        let runtime = crate::widget_object::WidgetRuntime::for_resources(host, parameters.opt, resources)?;
        Ok(Self::from_runtime(runtime, parameters))
    }
}

impl neocogi_ui_nodes_contracts::Scrollbar for Scrollbar {
    /// Reads all range fields from one consistent semantic snapshot.
    fn range(&self) -> Result<ScrollRange, NcStatus> {
        self.runtime().state.readable()?;
        let (viewport, content) = self.com_lengths();
        let axis = match self.axis() {
            crate::ScrollbarAxis::Horizontal => ScrollAxis::Horizontal,
            crate::ScrollbarAxis::Vertical => ScrollAxis::Vertical,
        };
        Ok(ScrollRange {
            axis,
            viewport,
            content,
            offset: self.offset(),
        })
    }
    /// Validates both lengths before clamping the existing offset in one commit.
    fn set_lengths(&mut self, viewport: i32, content: i32) -> Result<(), NcStatus> {
        self.runtime().state.writable()?;
        if viewport < 0 || content < 0 {
            return Err(NC_E_INVALID_ARG);
        }
        let previous = self.axis_state.values.get();
        self.set_lengths(viewport, content);
        if self.axis_state.values.get() != previous {
            self.runtime_mut().state.record_change(neocogi_ui_core_contracts::GeometryChange::Changed);
        }
        Ok(())
    }
    /// Offset affects thumb paint only; the control's preferred size is fixed by its axis.
    fn set_offset(&mut self, offset: i32) -> Result<(), NcStatus> {
        self.runtime().state.writable()?;
        let offset = offset.clamp(0, self.max_offset());
        if self.offset() != offset {
            self.set_offset(offset);
            let _ = self.runtime().state.host().mark_geometry_changed();
        }
        Ok(())
    }
}
