//! Runtime-owned paint state lets COM widgets append directly to the one frame display list.
use crate::boundary::{copied_rect, local_rect, text};
use neocogi_ui_contracts::*;
use neocogi_com::{ComPtr, ComRef, NcCopyFromBufferOffset, NcStatus, NC_E_INVALID_ARG, com_object};
use crate::{FontRole, WidgetOption, ControlRole};
use crate::math::RectExt;

#[com_object(interfaces(ITextMetrics))]
/// Immutable text metrics for one installed resource bundle.
pub(crate) struct Metrics {
    /// Current imported assets shared with concrete built-ins in this runtime.
    pub(crate) resources: super::widget_object::SharedAssets,
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

#[com_object(interfaces(IPainter))]
/// Runtime-owned mutable paint context exposed through one checked COM capability.
struct Recorder {
    /// Zero means no caller currently has authority to record.
    active: u64,
    /// Never wraps: exhaustion rejects the next invocation.
    next: u64,
    /// The active frame state exists only between this recorder's begin and finish operations.
    context: Option<PaintContext>,
    /// Installed resources resolve the legacy convenience text operation without per-widget import.
    resources: super::widget_object::SharedAssets,
}

impl Recorder {
    /// Creates an idle runtime paint context over the runtime's one installed asset value.
    fn new(resources: super::widget_object::SharedAssets) -> Self {
        Self {
            active: 0,
            next: 0,
            context: None,
            resources,
        }
    }

    /// Validates authority before touching the active frame, including empty operations.
    fn check(&self, scope: PaintScope) -> Result<(), NcStatus> {
        if scope.invocation == 0 || self.active != scope.invocation {
            Err(UI_E_EXPIRED_SCOPE)
        } else if scope.clip.width < 0 || scope.clip.height < 0 {
            Err(NC_E_INVALID_ARG)
        } else {
            Ok(())
        }
    }
    /// Appends one operation directly to the active frame with an authoritative narrowed clip.
    fn with_painter<T>(&mut self, scope: PaintScope, draw: impl FnOnce(&mut crate::render::Painter<'_>) -> T) -> Result<T, NcStatus> {
        self.check(scope)?;
        let context = self.context.as_mut().ok_or(UI_E_BUSY)?;
        let requested = local_rect(scope.clip).translated(crate::Vec2i::new(context.content_rect.x, context.content_rect.y));
        let clip = context
            .screen_clip
            .positive_intersection(requested)
            .unwrap_or_else(|| crate::Recti::new(requested.x, requested.y, 0, 0));
        let mut painter = crate::render::Painter::for_widget(&mut context.display_list, context.content_rect, clip);
        Ok(draw(&mut painter))
    }

    /// Installs the runtime's existing frame buffers and opens one unique invocation.
    fn begin(&mut self, context: PaintContext) -> Result<u64, NcStatus> {
        if self.active != 0 || self.context.is_some() {
            return Err(UI_E_BUSY);
        }
        let invocation = self.next.checked_add(1).ok_or(neocogi_com::NC_E_OUT_OF_RANGE)?;
        self.next = invocation;
        self.context = Some(context);
        self.active = invocation;
        Ok(invocation)
    }

    /// Expires the matching invocation and returns the runtime's frame buffers to traversal.
    fn finish(&mut self, invocation: u64) -> Result<PaintContext, NcStatus> {
        if invocation == 0 || self.active != invocation {
            return Err(UI_E_EXPIRED_SCOPE);
        }
        self.active = 0;
        self.context.take().ok_or(UI_E_BUSY)
    }
}

/// Decodes one exhaustive contract state for a control appearance lookup.
fn local_control_state(state: WidgetVisualState) -> Result<crate::ControlState, NcStatus> {
    crate::ControlState::ALL.get(usize::from(state.raw())).copied().ok_or(NC_E_INVALID_ARG)
}

/// Decodes the only two states meaningful for structural surfaces.
fn local_surface_state(state: WidgetVisualState) -> Result<crate::SurfaceState, NcStatus> {
    match state {
        WidgetVisualState::DISABLED => Ok(crate::SurfaceState::Disabled),
        WidgetVisualState::NORMAL => Ok(crate::SurfaceState::Normal),
        _ => Err(NC_E_INVALID_ARG),
    }
}

impl IPainter for Recorder {
    /// Validates complete scalar geometry before appending anything to the frame.
    fn stroke_line(&mut self, scope: PaintScope, from: PaintPoint, to: PaintPoint, width: f32, rgba: u32) -> Result<(), NcStatus> {
        if !from.x.is_finite() || !from.y.is_finite() || !to.x.is_finite() || !to.y.is_finite() || !width.is_finite() || width < 0.0 {
            return Err(NC_E_INVALID_ARG);
        }
        self.with_painter(scope, |painter| {
            painter.stroke_line(crate::Vec2f::new(from.x, from.y), crate::Vec2f::new(to.x, to.y), width, crate::render::external::color(rgba));
        })
    }
    /// Copies a whole finite boundary; native geometry code owns all triangulation and clipping.
    fn fill_polygon(&mut self, scope: PaintScope, input: NcCopyFromBufferOffset<'_, PaintPoint>, rgba: u32) -> Result<(), NcStatus> {
        if input.offset() != 0 || input.as_slice().iter().any(|p| !p.x.is_finite() || !p.y.is_finite()) {
            return Err(NC_E_INVALID_ARG);
        }
        let points = input.as_slice();
        self.with_painter(scope, |painter| painter.fill_contract_polygon(points, crate::render::external::color(rgba)))
    }

    /// Validates a complete solid batch before appending; invalid or expired calls publish nothing.
    fn colored_triangles(&mut self, scope: PaintScope, input: NcCopyFromBufferOffset<'_, ColoredVertex>) -> Result<(), NcStatus> {
        let vertices = input.as_slice();
        if input.offset() != 0 || !vertices.len().is_multiple_of(3) || vertices.iter().any(|v| !v.x.is_finite() || !v.y.is_finite()) {
            return Err(NC_E_INVALID_ARG);
        }
        self.with_painter(scope, |painter| painter.colored_triangles(vertices))
    }

    /// Retains the resource outside the command borrow; validation at submission authenticates its owner.
    fn texture(&mut self, scope: PaintScope, rect: Rect, source: TextureRegion, rgba: u32, texture: ComRef<'_, ITextureAbi>) -> Result<(), NcStatus> {
        if rect.width < 0 || rect.height < 0 {
            return Err(NC_E_INVALID_ARG);
        }
        source.validate()?;
        self.check(scope)?;
        // AddRef can execute foreign code, so retain the texture before taking the paint context.
        let texture = texture.add_ref();
        self.with_painter(scope, |painter| painter.com_texture_region(local_rect(rect), source, crate::render::external::color(rgba), texture))
    }
    /// Records a capability rather than a callback lending mutable GPU state across COM.
    fn item(&mut self, scope: PaintScope, rect: Rect, item: ComRef<'_, IRenderItemAbi>) -> Result<(), NcStatus> {
        if rect.width < 0 || rect.height < 0 {
            return Err(NC_E_INVALID_ARG);
        }
        self.check(scope)?;
        // AddRef can execute foreign code, so retain the item before taking the paint context.
        let item = item.add_ref();
        self.with_painter(scope, |painter| painter.com_item(local_rect(rect), item))
    }
    /// Validates the whole batch before publishing any geometry to this scope.
    fn triangles(&mut self, scope: PaintScope, input: NcCopyFromBufferOffset<'_, RenderVertex>) -> Result<(), NcStatus> {
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
    fn fill(&mut self, scope: PaintScope, rect: Rect, rgba: u32) -> Result<(), NcStatus> {
        if rect.width < 0 || rect.height < 0 {
            return Err(NC_E_INVALID_ARG);
        }
        self.with_painter(scope, |painter| painter.fill_rect(local_rect(rect), crate::render::external::color(rgba)))
    }
    /// Appends text while its borrowed UTF-8 input remains valid for this synchronous call.
    fn text(&mut self, scope: PaintScope, rect: Rect, input: NcCopyFromBufferOffset<'_, u8>) -> Result<(), NcStatus> {
        if rect.width < 0 || rect.height < 0 {
            return Err(NC_E_INVALID_ARG);
        }
        let label = text(input)?;
        // Resource lookup is local and immutable; the borrow ends before returning across COM.
        let rect = local_rect(rect);
        let (font, color, position) = {
            let resources = self.resources.borrow();
            let skin = resources.bundle().skin();
            let atlas = resources.bundle().atlas();
            let font = skin.resolve_font_role(atlas, FontRole::Body);
            let color = skin.control(ControlRole::Button, crate::ControlState::Enabled(crate::PointerState::Normal)).content_color;
            let position = crate::ui_node::text_layout::control_text_position_with_font(skin, atlas, font, label, rect, WidgetOption::NONE);
            (font, color, position)
        };
        self.with_painter(scope, |painter| {
            painter.with_clip(rect, |painter| painter.text(font, label, position, color));
        })
    }

    /// Resolves one compact semantic patch in the runtime and records it without widget-local geometry.
    fn frame(
        &mut self,
        scope: PaintScope,
        rect: Rect,
        role: WidgetFrame,
        state: WidgetVisualState,
        center_only: bool,
    ) -> Result<Rect, NcStatus> {
        if rect.width < 0 || rect.height < 0 {
            return Err(NC_E_INVALID_ARG);
        }
        let role = super::mounted_node::local_frame(role)?;
        let mut patch = {
            let resources = self.resources.borrow();
            let skin = resources.bundle().skin();
            match role {
                crate::FrameRole::Control(role) => skin.control(role, local_control_state(state)?).patch,
                crate::FrameRole::Surface(role) => skin.surface(role, local_surface_state(state)?).patch,
            }
        };
        if center_only {
            patch = patch.with_insets(crate::SliceInsets::ZERO);
        }
        let content = self.with_painter(scope, |painter| crate::ui_node::frame::paint_internal_frame(painter, local_rect(rect), patch))?;
        Ok(content.map(copied_rect).unwrap_or_default())
    }

    /// Authenticates and records one atlas icon directly in the current frame operation sequence.
    fn icon(&mut self, scope: PaintScope, icon: PaintIcon, rect: Rect, rgba: u32) -> Result<(), NcStatus> {
        if rect.width < 0 || rect.height < 0 {
            return Err(NC_E_INVALID_ARG);
        }
        let icon = self.resources.borrow().bundle().atlas().icon_at_index(icon.index).ok_or(NC_E_INVALID_ARG)?;
        self.with_painter(scope, |painter| painter.icon(icon, local_rect(rect), crate::render::external::color(rgba)))
    }

    /// Authenticates and records one exact-font UTF-8 run without an intermediate text command.
    fn text_run(
        &mut self,
        scope: PaintScope,
        font: PaintFont,
        position: Point,
        rgba: u32,
        input: NcCopyFromBufferOffset<'_, u8>,
    ) -> Result<(), NcStatus> {
        let label = text(input)?;
        let font = self.resources.borrow().bundle().atlas().font_at_index(font.index).ok_or(NC_E_INVALID_ARG)?;
        self.with_painter(scope, |painter| {
            painter.text(font, label, crate::Vec2i::new(position.x, position.y), crate::render::external::color(rgba));
        })
    }
}

/// Reusable driver; one instance serves sequential widget paint calls in a runtime.
#[derive(Clone)]
pub(crate) struct PaintService {
    /// Checked capability whose concrete object owns the runtime's mutable paint state.
    painter: ComPtr<IPainterAbi>,
}

impl PaintService {
    /// Allocates one service for the runtime, sharing its one installed resource value.
    pub(crate) fn new(resources: super::widget_object::SharedAssets) -> Self {
        let painter = Recorder::from_value(Recorder::new(resources)).query().expect("Recorder declares IPainter");
        Self { painter }
    }

    /// Uses neocogi-com's object-wide exclusive gate to access the recorder implementation it created.
    fn with_recorder_mut<T>(&mut self, call: impl FnOnce(&mut Recorder) -> T) -> Result<T, NcStatus> {
        let header = <IPainterAbi as neocogi_com::ComInterface>::iunknown(self.painter.as_mut_ptr());
        // SAFETY: `painter` was created exclusively by `Recorder::from_value` in `new` and keeps
        // that allocation alive. Its generated interface header therefore carries the matching
        // Recorder storage pointer. `with_abi_mut` applies the same exclusive call gate used by
        // normal mutable COM dispatch before lending the concrete value to this closure.
        unsafe { <Recorder as neocogi_com::ComAbiObject>::with_abi_mut((*header).object, call) }
    }

    /// Moves the frame list into the scoped context, calls a component, then restores the list.
    pub(super) fn paint(&mut self, widget: &ComPtr<IWidgetAbi>, ctx: &mut crate::WidgetPaintCtx<'_>) -> Result<(), NcStatus> {
        let invocation = self.with_recorder_mut(|recorder| {
            recorder.begin(PaintContext {
            display_list: ctx.take_display_list(),
            content_rect: ctx.paint_content_rect(),
            screen_clip: ctx.paint_screen_clip(),
            })
        })??;
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
        let context = self.with_recorder_mut(|recorder| recorder.finish(invocation))??;
        ctx.restore_display_list(context.display_list);
        result
    }
}
