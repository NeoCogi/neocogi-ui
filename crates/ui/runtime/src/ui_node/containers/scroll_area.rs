//
// Copyright 2023-Present (c) Raja Lehtihet & Wael El Oraiby
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

use crate::{SurfaceRole};

use std::rc::Rc;

use bitflags::bitflags;

use crate::ui_node::widgets::{Scrollbar, ScrollbarAxis, ScrollbarParameters};
use crate::{
    ChildParticipation, ContainerWidget, Dimensioni, FrameRole, MeasureCtx, Recti, UiInputEvent, Vec2i, Widget, WidgetOption, WidgetPaintCtx, WidgetUpdateCtx,
};

use super::{ContainerLayoutCtx};

/// Returns the child allocation for one scrollbar adjacent to the resolved content body.
fn scrollbar_track(axis: ScrollbarAxis, body: Recti, scrollbar_size: i32) -> Recti {
    // Start with the body so the track preserves its cross-axis origin and extent.
    let mut track = body;
    match axis {
        ScrollbarAxis::Vertical => {
            // vertical_track_x = body_x + body_width.
            track.x = body.x.saturating_add(body.width);
            track.width = scrollbar_size;
        }
        ScrollbarAxis::Horizontal => {
            // horizontal_track_y = body_y + body_height.
            track.y = body.y.saturating_add(body.height);
            track.height = scrollbar_size;
        }
    }
    track
}

bitflags! {
    #[derive(Copy, Clone)]
    /// Options fixed when a retained scroll area is constructed.
    pub struct ScrollAreaOption : u32 {
        /// Gives the scroll area a Skin-owned outer border and inset content area.
        const FRAME = 1024;
        /// Enables scrolling and scrollbars initially.
        const ENABLE_SCROLL = 32;
        /// No special options.
        const NONE = 0;
    }
}

/// Geometry retained only to describe the latest committed scroll surface.
#[derive(Copy, Clone, Debug, Default)]
struct ScrollAreaGeometry {
    /// Complete widget-local surface, excluding an optional generic frame.
    surface: Recti,
    /// Clamped content translation represented as positive scroll coordinates.
    offset: Vec2i,
    /// Content-local viewport extent used to resolve scroll-into-view requests.
    viewport: Dimensioni,
    /// Parent-local allocation of the vertical scrollbar child.
    vertical: Option<Recti>,
    /// Parent-local allocation of the horizontal scrollbar child.
    horizontal: Option<Recti>,
    /// Non-interactive gap between two visible scrollbar children.
    corner: Option<Recti>,
}

/// Insets a non-negative rectangle without allowing either axis to underflow.
fn inset_rect(rect: Recti, amount: i32) -> Recti {
    let amount = amount.max(0);
    // inset_x/y = min(requested_inset, non_negative_axis_extent).
    let inset_x = amount.min(rect.width.max(0));
    let inset_y = amount.min(rect.height.max(0));
    // inset_extent = leading_inset + trailing_inset = amount * 2.
    let inset_extent = amount.saturating_mul(2);
    // content_origin = rectangle_origin + clamped_inset.
    let x = rect.x.saturating_add(inset_x);
    let y = rect.y.saturating_add(inset_y);
    // content_extent = max(rectangle_extent - inset_extent, 0).
    let width = rect.width.saturating_sub(inset_extent).max(0);
    let height = rect.height.saturating_sub(inset_extent).max(0);
    Recti::new(x, y, width, height)
}

/// Returns the nearest non-negative offset that reveals one interval inside a viewport.
fn offset_to_reveal_interval(offset: i32, viewport_len: i32, interval_start: i32, interval_len: i32) -> i32 {
    // A zero-size viewport cannot reveal content. Preserve the existing request so a later layout
    // with usable geometry remains authoritative.
    if viewport_len <= 0 {
        return offset.max(0);
    }

    // Normalize the requested interval before comparing it with the currently visible range.
    let start = interval_start.max(0);
    let end = start.saturating_add(interval_len.max(0));
    let visible_start = offset.max(0);
    let visible_end = visible_start.saturating_add(viewport_len);

    if start < visible_start {
        // Reveal content before the viewport by aligning its leading edge.
        start
    } else if end > visible_end {
        // Reveal content after the viewport by aligning its trailing edge. Do not clamp against the
        // last committed content range because an editor may have expanded before the next layout.
        end.saturating_sub(viewport_len).max(0)
    } else {
        // Keep the current offset when the complete requested interval is already visible.
        visible_start
    }
}

/// Shared scrolling data contains no widget/parent references and never owns a child collection.
/// Actual scrollbar widgets and the transform/editor composition share these exact scalar values.
pub(crate) struct ScrollModel {
    /// The horizontal scrollbar's one authoritative range and requested offset.
    horizontal: Rc<crate::ui_node::widgets::ScrollAxisState>,
    /// The vertical scrollbar's one authoritative range and requested offset.
    vertical: Rc<crate::ui_node::widgets::ScrollAxisState>,
    /// Parent-controlled enablement determines wheel routing and structural participation.
    enabled: std::cell::Cell<bool>,
    /// One copied committed geometry summary, shared with caret reveal requests.
    geometry: std::cell::Cell<ScrollAreaGeometry>,
    /// Enablement changes persist independently of ordinary update participation.
    dirty: std::cell::Cell<bool>,
}
impl ScrollModel {
    /// Composes existing scrollbar state without creating another authority for either offset.
    pub(crate) fn new(
        horizontal: Rc<crate::ui_node::widgets::ScrollAxisState>,
        vertical: Rc<crate::ui_node::widgets::ScrollAxisState>,
        enabled: bool,
    ) -> Rc<Self> {
        Rc::new(Self {
            horizontal,
            vertical,
            enabled: std::cell::Cell::new(enabled),
            geometry: std::cell::Cell::default(),
            dirty: std::cell::Cell::new(false),
        })
    }
    /// Reads the actual axis requests, not a cached copy of their last placement.
    pub(crate) fn offset(&self) -> Vec2i {
        Vec2i::new(self.horizontal.offset(), self.vertical.offset())
    }
    /// Copies a coherent ABI snapshot from the actual axis requests and committed viewport.
    pub(crate) fn com_info(&self) -> neocogi_ui_core_contracts::ScrollInfo {
        let requested = self.offset();
        let maximum = self.maximum();
        let viewport = self.geometry.get().viewport;
        neocogi_ui_core_contracts::ScrollInfo {
            requested: neocogi_ui_core_contracts::Point { x: requested.x, y: requested.y },
            maximum: neocogi_ui_core_contracts::Point { x: maximum.x, y: maximum.y },
            viewport: neocogi_ui_core_contracts::Size {
                width: viewport.width,
                height: viewport.height,
            },
            enabled: self.enabled.get(),
        }
    }
    /// Requests offsets that the next placement clamps against newly measured content.
    pub(crate) fn set_offset(&self, offset: Vec2i) {
        let offset = if self.enabled.get() { offset } else { Vec2i::default() };
        self.horizontal.request_offset(offset.x);
        self.vertical.request_offset(offset.y);
    }
    /// Requests the newly measured vertical end, preserving the current horizontal position.
    pub(crate) fn scroll_to_end(&self) {
        if self.enabled.get() {
            self.vertical.request_offset(i32::MAX);
        }
    }
    /// Reveals a content-local interval using the single committed viewport and actual axis requests.
    pub(crate) fn reveal(&self, rect: Recti) {
        if !self.enabled.get() {
            return;
        }
        let offset = self.offset();
        let viewport = self.geometry.get().viewport;
        self.set_offset(Vec2i::new(
            offset_to_reveal_interval(offset.x, viewport.width, rect.x, rect.width),
            offset_to_reveal_interval(offset.y, viewport.height, rect.y, rect.height),
        ));
    }
    /// Changes responsive policy and synchronously clears disabled interaction requests.
    pub(crate) fn set_enabled(&self, enabled: bool) {
        if self.enabled.replace(enabled) != enabled {
            self.dirty.set(true);
        }
        if !enabled {
            self.horizontal.deactivate();
            self.vertical.deactivate();
            self.geometry.set(ScrollAreaGeometry {
                surface: self.geometry.get().surface,
                ..ScrollAreaGeometry::default()
            });
        }
    }
    /// Derives each maximum from the exact range used by its scrollbar.
    fn maximum(&self) -> Vec2i {
        Vec2i::new(self.horizontal.maximum(), self.vertical.maximum())
    }
    /// Accepts a wheel delta only if it can move at least one axis, allowing ancestor bubbling at boundaries.
    fn accepts_delta(&self, delta: Vec2i) -> bool {
        if !self.enabled.get() {
            return false;
        }
        let offset = self.offset();
        let maximum = self.maximum();
        let next = Vec2i::new(
            offset.x.saturating_add(delta.x).clamp(0, maximum.x),
            offset.y.saturating_add(delta.y).clamp(0, maximum.y),
        );
        next.x != offset.x || next.y != offset.y
    }
    /// Applies an accepted two-axis wheel movement against one coherent committed range.
    fn scroll_by(&self, delta: Vec2i) -> bool {
        if !self.accepts_delta(delta) {
            return false;
        }
        let offset = self.offset();
        let maximum = self.maximum();
        self.set_offset(Vec2i::new(
            offset.x.saturating_add(delta.x).clamp(0, maximum.x),
            offset.y.saturating_add(delta.y).clamp(0, maximum.y),
        ));
        true
    }
    /// Drains every shared semantic marker; layout range installation does not create a new request.
    fn take_change(&self) -> bool {
        self.dirty.replace(false) | self.horizontal.take_change() | self.vertical.take_change()
    }
}

/// Concrete three-child scroll composite; its model owns data but no widget or tree backpointer.
#[neocogi_com::com_object(neocogi_ui_nodes_contracts::ScrollArea)]
pub struct ScrollArea {
    /// Non-semantic COM lifecycle and host-backed phase resources.
    runtime: crate::ui_node::widget_object::WidgetRuntime,
    /// Exactly three owned structural children: surface and both scrollbar objects.
    editor: neocogi_com::ComPtr<neocogi_ui_core_contracts::IChildrenEditorAbi>,
    /// Private writer for the surface's single replaceable application child.
    content: neocogi_com::ComPtr<neocogi_ui_core_contracts::IChildrenEditorAbi>,
    /// Same scrolling values used by the actual bars, transform surface and optional text editor.
    model: Rc<ScrollModel>,
}
impl ScrollArea {
    /// Combines the fixed structural tree and shared scroll model with one concrete runtime.
    fn from_runtime(
        runtime: crate::ui_node::widget_object::WidgetRuntime,
        editor: neocogi_com::ComPtr<neocogi_ui_core_contracts::IChildrenEditorAbi>,
        content: neocogi_com::ComPtr<neocogi_ui_core_contracts::IChildrenEditorAbi>,
        model: Rc<ScrollModel>,
    ) -> Self {
        Self { runtime, editor, content, model }
    }

    /// Requests nonnegative offsets; placement clamps against freshly measured content.
    pub fn set_offset(&mut self, offset: Vec2i) {
        self.model.set_offset(offset);
    }
    /// Requests the end of the next measured vertical range.
    pub fn scroll_to_end(&mut self) {
        self.model.scroll_to_end();
    }
    /// Requests the nearest offsets that reveal a content-local rectangle.
    pub fn scroll_rect_into_view(&mut self, rect: Recti) {
        self.model.reveal(rect);
    }
    /// Enables or disables scrolling and its two actual scrollbar children.
    pub fn set_scrolling_enabled(&mut self, enabled: bool) {
        self.model.set_enabled(enabled);
    }
}

impl Widget for ScrollArea {
    /// Shared offset/caret/enablement requests remain observable even if update skipped this subtree.
    fn take_shared_change(&self) -> bool {
        self.take_scroll_change()
    }

    fn frame_appearance_role(&self) -> FrameRole {
        self.scroll_frame_role()
    }

    /// Reports scrolling because child placement reads the scrollbar-owned offsets.
    fn update(&mut self, _ctx: &mut WidgetUpdateCtx<'_>, input: Option<&UiInputEvent>) -> crate::Change {
        self.update_scroll(input)
    }

    fn paint(&self, ctx: &mut WidgetPaintCtx<'_>) {
        self.paint_scroll(ctx);
    }
}

impl ScrollComposition for ScrollArea {
    /// Supplies the exact model used by ScrollArea's input, paint, and public scrolling methods.
    fn scroll_model(&self) -> &ScrollModel {
        &self.model
    }
}

/// The transform boundary between the viewport and its ordinary content node.
#[neocogi_com::com_object(interfaces(neocogi_ui_core_contracts::IWidget, neocogi_ui_core_contracts::IContainerLayout))]
struct ScrollSurface {
    /// Non-semantic COM lifecycle and host-backed phase resources.
    runtime: crate::ui_node::widget_object::WidgetRuntime,
    /// Sole writer for the one application content child mounted through COM.
    editor: neocogi_com::ComPtr<neocogi_ui_core_contracts::IChildrenEditorAbi>,
    /// Shares actual axis requests without a reference to the owning parent widget.
    model: Rc<ScrollModel>,
}

impl ScrollSurface {
    /// Combines one fixed content writer with the shared scroll scalar model.
    fn from_runtime(
        runtime: crate::ui_node::widget_object::WidgetRuntime,
        editor: neocogi_com::ComPtr<neocogi_ui_core_contracts::IChildrenEditorAbi>,
        model: Rc<ScrollModel>,
    ) -> Self {
        Self { runtime, editor, model }
    }
}

impl ContainerWidget for ScrollSurface {
    /// Measures the ordinary content with bounded width and unbounded height.
    fn measure(&self, ctx: &mut MeasureCtx<'_>, constraints: crate::Constraints) -> Dimensioni {
        // Forward the width contract so wrapping content resolves against its eventual viewport;
        // height remains intrinsic so the owning ScrollArea can detect vertical overflow.
        ctx.measure_child(0, crate::Constraints::new(constraints.width, crate::AvailableSpace::Unbounded))
            .unwrap_or_default()
    }

    /// Places content at least as large as the viewport and applies the scrollbar translation.
    fn place(&mut self, ctx: &mut ContainerLayoutCtx<'_>, rect: Recti) {
        // Re-measure with the exact viewport width selected after scrollbar convergence so wrapped
        // content and the final allocation use the same horizontal constraint.
        let preferred = ctx
            .measure_child(
                0,
                crate::Constraints::new(crate::AvailableSpace::bounded(rect.width), crate::AvailableSpace::Unbounded),
            )
            .unwrap_or_default();
        // Fill both viewport axes so an interactive child owns blank trailing space. Desired extents
        // still win when larger, preserving overflow and its associated scrollbar range.
        let content_rect = Recti::new(0, 0, rect.width.max(preferred.width).max(0), rect.height.max(preferred.height).max(0));
        let _ = ctx.layout_child(0, content_rect);
        // Read clamped values only after range configuration and translate descendants without
        // changing their stable content-local allocations.
        let offset = self.model.offset();
        ctx.set_children_viewport(rect, Vec2i::new(-offset.x, -offset.y));
        ctx.set_content_size(Dimensioni::new(content_rect.width, content_rect.height));
        ctx.set_child_overflow_propagation(false);
    }
}

impl Widget for ScrollSurface {
    /// Emits no paint operations because clipping and translation are traversal geometry.
    fn paint(&self, _ctx: &mut WidgetPaintCtx<'_>) {}
}

/// Statically shares fixed scroll-composition layout without storing another widget value.
pub(crate) trait ScrollComposition: Widget {
    /// Transform surface containing the application-provided content node.
    const SURFACE: usize = 0;
    /// Independently targetable horizontal scrollbar widget.
    const HORIZONTAL: usize = 1;
    /// Independently targetable vertical scrollbar widget.
    const VERTICAL: usize = 2;

    /// Borrows the scalar model already shared with the surface and both scrollbar children.
    fn scroll_model(&self) -> &ScrollModel;

    /// Drains requests emitted by the parent, editor, or either actual scrollbar child.
    fn take_scroll_change(&self) -> bool {
        self.scroll_model().take_change()
    }

    /// Returns the common panel appearance used by every fixed scrolling composition.
    fn scroll_frame_role(&self) -> FrameRole {
        FrameRole::Surface(SurfaceRole::Panel)
    }

    /// Applies wheel input to the shared model and reports whether placement must run again.
    fn update_scroll(&self, input: Option<&UiInputEvent>) -> crate::Change {
        match input {
            Some(UiInputEvent::Scroll { delta, .. }) => crate::Change::from(self.scroll_model().scroll_by(*delta)),
            _ => crate::Change::NoChange,
        }
    }

    /// Paints the noninteractive panel and any corner left between visible scrollbar children.
    fn paint_scroll(&self, ctx: &mut WidgetPaintCtx<'_>) {
        let fallback = ctx.local_rect();
        let geometry = self.scroll_model().geometry.get();
        // Before first placement the summary is empty; direct widget tests use the supplied local
        // rectangle while ordinary traversal always paints committed geometry.
        let surface = if geometry.surface.width > 0 || geometry.surface.height > 0 {
            geometry.surface
        } else {
            fallback
        };
        ctx.draw_surface_center_state(SurfaceRole::Panel, crate::SurfaceState::Normal, surface);
        if let Some(corner) = geometry.corner {
            ctx.draw_surface_center_state(SurfaceRole::Panel, crate::SurfaceState::Normal, corner);
        }
    }

    /// Adds dynamic wheel routing without duplicating the caller-authored common options.
    fn scroll_options(&self, options: WidgetOption) -> WidgetOption {
        if self.scroll_model().enabled.get() {
            options | WidgetOption::GRAB_SCROLL
        } else {
            options | WidgetOption::NO_INTERACT
        }
    }
}

impl<T: ScrollComposition> ContainerWidget for T {
    /// Measures content through the surface and includes Skin-owned viewport padding.
    fn measure(&self, ctx: &mut MeasureCtx<'_>, constraints: crate::Constraints) -> Dimensioni {
        // Scrollbars are responsive affordances and do not inflate intrinsic composite size.
        let padding = ctx.skin().metrics().padding.max(0);
        // inset = leading_padding + trailing_padding = padding * 2.
        let inset = padding.saturating_mul(2);
        let content_width = match constraints.width {
            crate::AvailableSpace::Bounded(width) => crate::AvailableSpace::bounded(width.saturating_sub(inset)),
            crate::AvailableSpace::Unbounded => crate::AvailableSpace::Unbounded,
        };
        let content = ctx
            .measure_child(Self::SURFACE, crate::Constraints::new(content_width, crate::AvailableSpace::Unbounded))
            .unwrap_or_default();
        // preferred_extent = content_extent + leading_padding + trailing_padding.
        Dimensioni::new(content.width.saturating_add(inset), content.height.saturating_add(inset))
    }

    /// Places Skin-padded content and any required scrollbars inside the local surface.
    fn place(&mut self, ctx: &mut ContainerLayoutCtx<'_>, rect: Recti) {
        let model = self.scroll_model();
        // All committed summary geometry is parent-local; child scrollbar geometry passed to each
        // widget is normalized to that child's own local track by configure_bar.
        let surface = Recti::new(0, 0, rect.width.max(0), rect.height.max(0));
        let padding = ctx.skin().metrics().padding.max(0);
        let bar_size = ctx.skin().metrics().scrollbar_size.max(0);
        let enabled = model.enabled.get();
        let requested = model.offset();
        let bars_usable = enabled && bar_size > 0 && surface.width > 0 && surface.height > 0;
        let mut has_horizontal = bars_usable && model.geometry.get().horizontal.is_some();
        let mut has_vertical = bars_usable && model.geometry.get().vertical.is_some();
        let mut visited_states = 0_u8;

        // Start from the last committed state so an unchanged retained tree keeps the same width
        // constraint and reuses its child-measurement cache. Candidate states measure the surface;
        // no child is placed until the final scrollbar presence has converged. There are only four
        // states; if perpendicular induction creates a two-state cycle, commit their union.
        let mut resolved = None;
        for _ in 0..5 {
            let state_bit = 1_u8 << ((has_horizontal as u8) | ((has_vertical as u8) << 1));
            visited_states |= state_bit;
            let vertical_width = if has_vertical { bar_size.min(surface.width) } else { 0 };
            let horizontal_height = if has_horizontal { bar_size.min(surface.height) } else { 0 };
            // body_extent = surface_extent - occupied_scrollbar_extent.
            let body = Recti::new(
                0,
                0,
                surface.width.saturating_sub(vertical_width).max(0),
                surface.height.saturating_sub(horizontal_height).max(0),
            );
            // Inset content by the global Skin padding while keeping scrollbar tracks adjacent to
            // the complete body.
            let view = inset_rect(body, padding);
            let preferred = ctx
                .measure_child(
                    Self::SURFACE,
                    crate::Constraints::new(crate::AvailableSpace::bounded(view.width), crate::AvailableSpace::Unbounded),
                )
                .unwrap_or_default();
            // Fill the viewport on both axes while preserving larger desired extents. Comparing this
            // filled extent with the view still detects overflow exactly when preferred content is
            // larger, and it matches ScrollSurface's final allocation contract.
            let extent = Dimensioni::new(view.width.max(preferred.width).max(0), view.height.max(preferred.height).max(0));

            let next_vertical = bars_usable && extent.height > view.height;
            let next_horizontal = bars_usable && extent.width > view.width;
            if next_vertical != has_vertical || next_horizontal != has_horizontal {
                let next_bit = 1_u8 << ((next_horizontal as u8) | ((next_vertical as u8) << 1));
                if visited_states & next_bit != 0 {
                    let previous = (has_horizontal, has_vertical);
                    has_horizontal |= next_horizontal;
                    has_vertical |= next_vertical;
                    if (has_horizontal, has_vertical) != previous {
                        continue;
                    }
                } else {
                    has_horizontal = next_horizontal;
                    has_vertical = next_vertical;
                    continue;
                }
            }

            resolved = Some((body, view, extent));
            break;
        }

        let (body, view, extent) = resolved.expect("ScrollArea scrollbar presence must converge across four possible states");
        let vertical_width = if has_vertical { bar_size.min(surface.width) } else { 0 };
        let horizontal_height = if has_horizontal { bar_size.min(surface.height) } else { 0 };
        let vertical_track = scrollbar_track(ScrollbarAxis::Vertical, body, vertical_width);
        let horizontal_track = scrollbar_track(ScrollbarAxis::Horizontal, body, horizontal_height);
        let vertical_visible = has_vertical && vertical_track.width > 0 && vertical_track.height > 0;
        let horizontal_visible = has_horizontal && horizontal_track.width > 0 && horizontal_track.height > 0;

        if vertical_visible {
            // The scrollbar remains a real independently targetable child when active.
            model.vertical.install(view.height, extent.height, requested.y);
            let _ = ctx.set_child_participation(Self::VERTICAL, ChildParticipation::Active);
            let _ = ctx.layout_child(
                Self::VERTICAL,
                Recti::new(
                    rect.x + vertical_track.x,
                    rect.y + vertical_track.y,
                    vertical_track.width,
                    vertical_track.height,
                ),
            );
        } else {
            // Hidden participation removes the retained bar from all ordinary runtime phases.
            model.vertical.deactivate();
            let _ = ctx.set_child_participation(Self::VERTICAL, ChildParticipation::Hidden);
        }

        if horizontal_visible {
            model.horizontal.install(view.width, extent.width, requested.x);
            let _ = ctx.set_child_participation(Self::HORIZONTAL, ChildParticipation::Active);
            let _ = ctx.layout_child(
                Self::HORIZONTAL,
                Recti::new(
                    rect.x + horizontal_track.x,
                    rect.y + horizontal_track.y,
                    horizontal_track.width,
                    horizontal_track.height,
                ),
            );
        } else {
            model.horizontal.deactivate();
            let _ = ctx.set_child_participation(Self::HORIZONTAL, ChildParticipation::Hidden);
        }

        let offset = model.offset();
        // The surface reads the clamped scrollbar offsets while assigning its content. Mark only a
        // changed translation dirty so an unchanged exact allocation may still use the retained
        // layout fast path.
        if offset.x != model.geometry.get().offset.x || offset.y != model.geometry.get().offset.y {
            ctx.child_placement_changed(Self::SURFACE);
        }
        let child_rect = Recti::new(rect.x + view.x, rect.y + view.y, view.width, view.height);
        let _ = ctx.layout_child(Self::SURFACE, child_rect);

        let corner = (vertical_visible && horizontal_visible)
            .then(|| Recti::new(body.x + body.width, body.y + body.height, vertical_track.width, horizontal_track.height));
        model.geometry.set(ScrollAreaGeometry {
            surface,
            offset,
            viewport: Dimensioni::new(view.width, view.height),
            vertical: vertical_visible.then_some(vertical_track),
            horizontal: horizontal_visible.then_some(horizontal_track),
            corner,
        });
        // The parent clips the three structural children to its surface. Scrolling translation is
        // owned exclusively by the nested scroll surface.
        ctx.set_children_viewport(rect, Vec2i::default());
        ctx.set_content_size(Dimensioni::new(surface.width, surface.height));
        ctx.set_child_overflow_propagation(false);
    }

    fn accepts_event(&self, event: &UiInputEvent) -> bool {
        let UiInputEvent::Scroll { delta, .. } = event else { return false };
        self.scroll_model().accepts_delta(*delta)
    }
}

impl ScrollArea {}

impl crate::ui_node::widget_object::WidgetAlgorithm for ScrollArea {
    /// Dynamic wheel eligibility follows the same shared policy used by placement.
    fn effective_options(&self, options: WidgetOption) -> WidgetOption {
        self.scroll_options(options)
    }
}

impl crate::ui_node::widget_object::HostedWidget for ScrollArea {
    /// Borrows common phase state stored directly beside ScrollArea's model and child writers.
    fn runtime(&self) -> &crate::ui_node::widget_object::WidgetRuntime {
        &self.runtime
    }

    /// Exclusively borrows common phase state for mutation or resource refresh.
    fn runtime_mut(&mut self) -> &mut crate::ui_node::widget_object::WidgetRuntime {
        &mut self.runtime
    }

    /// Closes the surface and both actual scrollbar children through their fixed writer.
    fn close_contents(&mut self) -> Result<(), neocogi_com::NcStatus> {
        self.editor.close()?.finish()
    }
}
impl crate::ui_node::widget_object::WidgetAlgorithm for ScrollSurface {}

impl crate::ui_node::widget_object::HostedWidget for ScrollSurface {
    /// Borrows common phase state stored directly beside the transform model.
    fn runtime(&self) -> &crate::ui_node::widget_object::WidgetRuntime {
        &self.runtime
    }

    /// Exclusively borrows common phase state for mutation or resource refresh.
    fn runtime_mut(&mut self) -> &mut crate::ui_node::widget_object::WidgetRuntime {
        &mut self.runtime
    }

    /// Closes the sole content child when this structural surface is retired.
    fn close_contents(&mut self) -> Result<(), neocogi_com::NcStatus> {
        self.editor.close()?.finish()
    }
}

use crate::ui_node::widget_object::{HostedWidget, WidgetRuntime, direct_container_layout_impl, direct_scrolling_impl, direct_widget_phase_impl};
use neocogi_ui_core_contracts::{ChildEditResult, IChildrenEditorAbi, IWidgetAbi, IWidgetHostAbi};
use neocogi_ui_nodes_contracts::{ScrollAreaAbi};
use neocogi_com::{ComPtr, ComRef, NcStatus};

direct_widget_phase_impl!(ScrollSurface);
direct_container_layout_impl!(ScrollSurface);
direct_widget_phase_impl!(ScrollArea);
direct_container_layout_impl!(ScrollArea);
direct_scrolling_impl!(ScrollArea);

/// Prepared fixed scroll topology shared by ScrollArea and TextArea construction.
///
/// This value is consumed into one final object; it never becomes a second widget or live owner.
pub(crate) struct ScrollParts {
    /// Fixed surface and actual scrollbar topology.
    pub(crate) editor: ComPtr<IChildrenEditorAbi>,
    /// Private insertion authority for the surface's single content child.
    pub(crate) content: ComPtr<IChildrenEditorAbi>,
    /// Scalar model supplied to editable content before attachment.
    pub(crate) model: Rc<ScrollModel>,
}

impl ScrollParts {
    /// Builds every structural role before attaching application or editor content.
    pub(crate) fn prepare(
        host: ComPtr<IWidgetHostAbi>,
        enabled: bool,
        resources: Option<&crate::ui_node::widget_object::SharedAssets>,
    ) -> Result<Self, NcStatus> {
        host.check_mutation()?;
        let horizontal_parameters = ScrollbarParameters::new(ScrollbarAxis::Horizontal);
        let horizontal_options = horizontal_parameters.opt;
        let horizontal_runtime = WidgetRuntime::for_resources(host.clone(), horizontal_options, resources)?;
        let horizontal = Scrollbar::from_runtime(horizontal_runtime, horizontal_parameters);
        let vertical_parameters = ScrollbarParameters::new(ScrollbarAxis::Vertical);
        let vertical_options = vertical_parameters.opt;
        let vertical_runtime = WidgetRuntime::for_resources(host.clone(), vertical_options, resources)?;
        let vertical = Scrollbar::from_runtime(vertical_runtime, vertical_parameters);
        let model = ScrollModel::new(horizontal.axis_state.clone(), vertical.axis_state.clone(), enabled);
        let horizontal = horizontal.into_com();
        let vertical = vertical.into_com();
        let content = host.create_children()?;
        let surface_runtime = WidgetRuntime::for_resources(host.clone(), crate::WidgetOption::NO_INTERACT, resources)?;
        let surface = ScrollSurface::from_runtime(surface_runtime, content.clone(), model.clone());
        let surface: ComPtr<IWidgetAbi> = ScrollSurface::from_value(surface)
            .query()
            .expect("ScrollSurface declares its widget and container-layout capabilities");
        let editor = host.create_children()?;
        editor.insert(0, surface.as_com_ref())?.finish()?;
        editor.insert(1, horizontal.query::<IWidgetAbi>()?.as_com_ref())?.finish()?;
        editor.insert(2, vertical.query::<IWidgetAbi>()?.as_com_ref())?.finish()?;
        Ok(Self { editor, content, model })
    }
}

impl ScrollArea {
    /// Constructs the class for the host-bound factory through the registry's typed contract.
    pub(crate) fn create_com(
        host: ComPtr<IWidgetHostAbi>,
        resources: crate::ui_node::widget_object::SharedAssets,
        content: ComRef<'_, IWidgetAbi>,
        enabled: bool,
    ) -> Result<ComPtr<ScrollAreaAbi>, NcStatus> {
        let value = Self::build(host, content, enabled, Some(&resources))?;
        Ok(Self::from_value(value).query().expect("ScrollArea declares its callable class contract"))
    }

    /// Builds the fixed scroll composition with either direct or runtime-shared resources.
    fn build(
        host: ComPtr<IWidgetHostAbi>,
        content: ComRef<'_, IWidgetAbi>,
        enabled: bool,
        resources: Option<&crate::ui_node::widget_object::SharedAssets>,
    ) -> Result<Self, NcStatus> {
        let parts = ScrollParts::prepare(host.clone(), enabled, resources)?;
        parts.content.insert(0, content)?.finish()?;
        let runtime = WidgetRuntime::for_resources(host, crate::WidgetOption::FRAME, resources)?;
        Ok(Self::from_runtime(runtime, parts.editor, parts.content, parts.model))
    }
}

impl neocogi_ui_nodes_contracts::ScrollArea for ScrollArea {
    /// Reads the actual content identity through the fixed one-child surface writer.
    fn content(&self) -> Result<ComPtr<IWidgetAbi>, NcStatus> {
        self.runtime().state.readable()?;
        self.content.view()?.child(0)
    }

    /// Replaces only application content while preserving every fixed structural role.
    fn replace_content(&mut self, content: ComRef<'_, IWidgetAbi>) -> Result<ChildEditResult, NcStatus> {
        self.runtime().state.writable()?;
        self.content.replace(0, content)
    }
}
