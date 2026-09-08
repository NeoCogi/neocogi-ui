//! Runtime-owned paint state lets COM widgets append directly to the one frame display list.
use super::*;
use crate::{FontRole, WidgetOption, ControlRole};
use crate::math::RectExt;

#[com_object(interfaces(ITextMetrics))]
/// Immutable text metrics for one installed resource bundle.
pub(super) struct Metrics {
    /// Current imported assets shared with concrete built-ins in this runtime.
    pub(super) resources: super::widget::SharedAssets,
}

impl ITextMetrics for Metrics {
    /// Uses the existing inline-content algorithm so migration preserves sizing.
    fn button_size(&self, input: NcCopyFromBufferOffset<'_, u8>) -> Result<Size, NcStatus> {
        let label = text(input)?;
        let resources = self.resources.borrow();
        let size = crate::ui_node::widgets::control::inline_content_size(
            resources.bundle().skin(),
            resources.bundle().atlas(),
            &crate::FontRef::Role(FontRole::Body),
            label,
            None,
        );
        Ok(Size { width: size.width, height: size.height })
    }
}

/// The one frame display list while control is inside a COM widget's paint method.
struct PaintContext {
    /// Runtime-owned output buffers, moved here without moving or copying their allocations.
    display_list: crate::render::DisplayList,
    /// Widget content allocation used to translate local primitives into screen coordinates.
    content_rect: crate::Recti,
    /// Traversal-derived screen clip which caller-provided scopes may only narrow.
    screen_clip: crate::Recti,
}

/// One runtime paint context and its monotonically increasing invocation clock.
struct Recording {
    /// Zero means no caller currently has authority to record.
    active: Cell<u64>,
    /// Never wraps: exhaustion rejects the next invocation.
    next: Cell<u64>,
    /// Owned state is temporarily taken by each primitive, then restored before returning to COM.
    context: Cell<Option<PaintContext>>,
    /// Installed resources resolve the legacy convenience text operation without per-widget import.
    resources: super::widget::SharedAssets,
}

impl Recording {
    /// Creates an idle runtime paint context over the runtime's one installed asset value.
    fn new(resources: super::widget::SharedAssets) -> Self {
        Self {
            active: Cell::new(0),
            next: Cell::new(0),
            context: Cell::new(None),
            resources,
        }
    }
}

#[com_object(interfaces(IPainter))]
/// Retainable COM facade over the runtime-owned paint context.
struct Recorder {
    /// Shared scope authority and temporarily installed frame state.
    recording: Rc<Recording>,
}

/// Exclusive owned access to the active context, restored on every ordinary or error return.
struct ContextAccess<'a> {
    /// Slot from which the state was taken without creating a runtime borrow.
    slot: &'a Cell<Option<PaintContext>>,
    /// Owned active state; `Some` until [`Drop`] restores it.
    context: Option<PaintContext>,
}

impl<'a> ContextAccess<'a> {
    /// Takes the active state or rejects reentry while another primitive is recording.
    fn take(slot: &'a Cell<Option<PaintContext>>) -> Result<Self, NcStatus> {
        let context = slot.take().ok_or(UI_E_BUSY)?;
        Ok(Self {
            slot,
            context: Some(context),
        })
    }

    /// Borrows the exclusively owned state for one direct display-list operation.
    fn context(&mut self) -> &mut PaintContext {
        self.context.as_mut().expect("active paint context is restored only by Drop")
    }
}

impl Drop for ContextAccess<'_> {
    /// Returns the complete frame buffers before control crosses COM again.
    fn drop(&mut self) {
        self.slot.set(self.context.take());
    }
}

impl Recorder {
    /// Validates authority before touching the active frame, including empty operations.
    fn check(&self, scope: PaintScope) -> Result<(), NcStatus> {
        if scope.invocation == 0 || self.recording.active.get() != scope.invocation {
            Err(UI_E_EXPIRED_SCOPE)
        } else if scope.clip.width < 0 || scope.clip.height < 0 {
            Err(NC_E_INVALID_ARG)
        } else {
            Ok(())
        }
    }
}

impl Recorder {
    /// Appends one operation directly to the active frame with an authoritative narrowed clip.
    fn with_painter(&self, scope: PaintScope, draw: impl FnOnce(&mut crate::render::Painter<'_>)) -> Result<(), NcStatus> {
        self.check(scope)?;
        let mut access = ContextAccess::take(&self.recording.context)?;
        let context = access.context();
        let requested = local_rect(scope.clip).translated(crate::Vec2i::new(context.content_rect.x, context.content_rect.y));
        let clip = context
            .screen_clip
            .positive_intersection(requested)
            .unwrap_or_else(|| crate::Recti::new(requested.x, requested.y, 0, 0));
        let mut painter = crate::render::Painter::for_widget(&mut context.display_list, context.content_rect, clip);
        draw(&mut painter);
        Ok(())
    }
}

impl IPainter for Recorder {
    /// Validates complete scalar geometry before appending anything to the frame.
    fn stroke_line(&self, scope: PaintScope, from: PaintPoint, to: PaintPoint, width: f32, rgba: u32) -> Result<(), NcStatus> {
        if !from.x.is_finite() || !from.y.is_finite() || !to.x.is_finite() || !to.y.is_finite() || !width.is_finite() || width < 0.0 {
            return Err(NC_E_INVALID_ARG);
        }
        self.with_painter(scope, |painter| {
            painter.stroke_line(crate::Vec2f::new(from.x, from.y), crate::Vec2f::new(to.x, to.y), width, super::render::color(rgba));
        })
    }
    /// Copies a whole finite boundary; native geometry code owns all triangulation and clipping.
    fn fill_polygon(&self, scope: PaintScope, input: NcCopyFromBufferOffset<'_, PaintPoint>, rgba: u32) -> Result<(), NcStatus> {
        if input.offset() != 0 || input.as_slice().iter().any(|p| !p.x.is_finite() || !p.y.is_finite()) {
            return Err(NC_E_INVALID_ARG);
        }
        let points = input.as_slice();
        self.with_painter(scope, |painter| painter.fill_contract_polygon(points, super::render::color(rgba)))
    }

    /// Validates a complete solid batch before appending; invalid or expired calls publish nothing.
    fn colored_triangles(&self, scope: PaintScope, input: NcCopyFromBufferOffset<'_, ColoredVertex>) -> Result<(), NcStatus> {
        let vertices = input.as_slice();
        if input.offset() != 0 || !vertices.len().is_multiple_of(3) || vertices.iter().any(|v| !v.x.is_finite() || !v.y.is_finite()) {
            return Err(NC_E_INVALID_ARG);
        }
        self.with_painter(scope, |painter| painter.colored_triangles(vertices))
    }

    /// Retains the resource outside the command borrow; validation at submission authenticates its owner.
    fn texture(&self, scope: PaintScope, rect: Rect, source: TextureRegion, rgba: u32, texture: ComRef<'_, ITextureAbi>) -> Result<(), NcStatus> {
        if rect.width < 0 || rect.height < 0 {
            return Err(NC_E_INVALID_ARG);
        }
        source.validate()?;
        self.check(scope)?;
        // AddRef can execute foreign code, so retain the texture before taking the paint context.
        let texture = texture.add_ref();
        self.with_painter(scope, |painter| painter.com_texture_region(local_rect(rect), source, super::render::color(rgba), texture))
    }
    /// Records a capability rather than a callback lending mutable GPU state across COM.
    fn item(&self, scope: PaintScope, rect: Rect, item: ComRef<'_, IRenderItemAbi>) -> Result<(), NcStatus> {
        if rect.width < 0 || rect.height < 0 {
            return Err(NC_E_INVALID_ARG);
        }
        self.check(scope)?;
        // AddRef can execute foreign code, so retain the item before taking the paint context.
        let item = item.add_ref();
        self.with_painter(scope, |painter| painter.com_item(local_rect(rect), item))
    }
    /// Validates the whole batch before publishing any geometry to this scope.
    fn triangles(&self, scope: PaintScope, input: NcCopyFromBufferOffset<'_, RenderVertex>) -> Result<(), NcStatus> {
        let vertices = input.as_slice();
        if input.offset() != 0
            || !vertices.len().is_multiple_of(3)
            || vertices
                .iter()
                .any(|v| !v.x.is_finite() || !v.y.is_finite() || !v.u.is_finite() || !v.v.is_finite())
        {
            return Err(NC_E_INVALID_ARG);
        }
        self.with_painter(scope, |painter| painter.atlas_triangles(vertices))
    }
    /// Appends a checked fill without accessing the caller's stack.
    fn fill(&self, scope: PaintScope, rect: Rect, rgba: u32) -> Result<(), NcStatus> {
        if rect.width < 0 || rect.height < 0 {
            return Err(NC_E_INVALID_ARG);
        }
        self.with_painter(scope, |painter| painter.fill_rect(local_rect(rect), super::render::color(rgba)))
    }
    /// Appends text while its borrowed UTF-8 input remains valid for this synchronous call.
    fn text(&self, scope: PaintScope, rect: Rect, input: NcCopyFromBufferOffset<'_, u8>) -> Result<(), NcStatus> {
        if rect.width < 0 || rect.height < 0 {
            return Err(NC_E_INVALID_ARG);
        }
        let label = text(input)?;
        // Resource lookup is local and immutable; the borrow ends before returning across COM.
        let resources = self.recording.resources.borrow();
        let skin = resources.bundle().skin();
        let atlas = resources.bundle().atlas();
        let font = skin.resolve_font_role(atlas, FontRole::Body);
        let color = skin.control(ControlRole::Button, crate::ControlState::Enabled(crate::PointerState::Normal)).content_color;
        let rect = local_rect(rect);
        let position = crate::ui_node::text_layout::control_text_position_with_font(skin, atlas, font, label, rect, WidgetOption::NONE);
        self.with_painter(scope, |painter| {
            painter.with_clip(rect, |painter| painter.text(font, label, position, color));
        })
    }
}

/// Reusable driver; one instance serves sequential widget paint calls in a runtime.
#[derive(Clone)]
pub(super) struct PaintService {
    /// Runtime-owned context and scope authority shared with the COM facade.
    recording: Rc<Recording>,
    /// Checked capability passed to widgets without an AddRef per primitive.
    painter: ComPtr<IPainterAbi>,
}

/// Private scope guard expires authority on both success and error paths.
struct Invocation<'a>(&'a Recording);
impl Drop for Invocation<'_> {
    /// Invalidates authority before the frame list returns to traversal or a later widget runs.
    fn drop(&mut self) {
        self.0.active.set(0);
    }
}

impl PaintService {
    /// Allocates one service for the runtime, sharing its one installed resource value.
    pub(super) fn new(resources: super::widget::SharedAssets) -> Self {
        let recording = Rc::new(Recording::new(resources));
        let painter = Recorder::from_value(Recorder { recording: recording.clone() })
            .query()
            .expect("Recorder declares IPainter");
        Self { recording, painter }
    }
    /// Moves the frame list into the scoped context, calls a component, then restores the list.
    pub(super) fn paint(&self, widget: &ComPtr<IWidgetAbi>, ctx: &mut crate::WidgetPaintCtx<'_>) -> Result<(), NcStatus> {
        if self.recording.active.get() != 0 || self.recording.context.take().is_some() {
            return Err(UI_E_BUSY);
        }
        let invocation = self.recording.next.get().checked_add(1).ok_or(nc_com::NC_E_OUT_OF_RANGE)?;
        self.recording.next.set(invocation);
        self.recording.context.set(Some(PaintContext {
            display_list: ctx.take_display_list(),
            content_rect: ctx.paint_content_rect(),
            screen_clip: ctx.paint_screen_clip(),
        }));
        self.recording.active.set(invocation);
        let guard = Invocation(&self.recording);
        let state = WidgetPaint {
            rect: copied_rect(ctx.local_rect()),
            clip: copied_rect(ctx.local_clip()),
            enabled: ctx.enabled(),
            hovered: ctx.hovered(),
            focused: ctx.focused(),
            clicked: ctx.clicked(),
            active: ctx.active(),
            window_active: ctx.window_active(),
        };
        let mut widget = widget.clone();
        let result = widget.paint(PaintScope { invocation, clip: state.clip }, state, self.painter.as_com_ref());
        drop(guard);
        let context = self.recording.context.take().ok_or(UI_E_BUSY)?;
        ctx.restore_display_list(context.display_list);
        result
    }
}
