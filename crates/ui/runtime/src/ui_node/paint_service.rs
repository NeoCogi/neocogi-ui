//! Runtime-owned paint state lets COM widgets append directly to the one frame display list.
use crate::boundary::{copied_rect, local_rect, text};
use neocogi_ui::abi::core::*;
use neocogi_com::{ComPtr, ComRef, NcCopyFromBufferOffset, NcStatus, NC_E_INVALID_ARG, com_object};
use crate::{FontRole, WidgetOption, ControlRole};
use crate::math::RectExt;
#[path = "paint_service/gpu.rs"]
mod gpu;
use neocogi_render_core::abi::{IRenderResourcesAbi, IRenderViewAbi, IRenderViewSessionAbi, IRenderViewBrokerAbi, RenderViewRoot, RenderViewState, RENDER_E_STATE};
use neocogi_com::ComBox;

/// One reserved list position and its private revocation/transfer controller.
struct PendingView {
    /// Index fixed before client recording starts.
    slot: usize,
    /// Kept by the runtime, never delegated to the widget.
    session: ComBox<IRenderViewSessionAbi>,
}

#[com_object(interfaces(ITextMetrics))]
/// Immutable text metrics for one installed resource bundle.
pub(crate) struct Metrics {
    /// Engine-local imported assets shared only with core metrics and scoped services.
    pub(crate) resources: super::SharedAssets,
}

impl ITextMetrics for Metrics {
    /// Uses the existing inline-content algorithm so migration preserves sizing.
    fn button_size(&self, input: NcCopyFromBufferOffset<'_, u8>) -> Result<Size, NcStatus> {
        let label = text(input)?;
        let resources = self.resources.borrow();
        let size = neocogi_ui_support::text_layout::inline_content_size(
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
    /// Unharvested contribution, if the widget began generic recording.
    view: Option<PendingView>,
    /// First unhandled view failure cannot be erased by attempting another begin.
    gpu_error: Option<NcStatus>,
}

#[com_object(interfaces(IPainter))]
/// One invocation's paint capability; its identity is never reused by another widget.
struct Recorder {
    /// Zero means no caller currently has authority to record.
    active: u64,
    /// Never wraps: exhaustion rejects the next invocation.
    next: u64,
    /// The active frame state exists only between this recorder's begin and finish operations.
    context: Option<PaintContext>,
    /// Installed resources resolve the legacy convenience text operation without per-widget import.
    resources: super::SharedAssets,
    /// Entire-traversal recording lifetime installed only by trusted runtime code.
    frames: std::rc::Rc<gpu::Frames>,
}

impl Recorder {
    /// Creates an idle runtime paint context over the runtime's one installed asset value.
    fn new(resources: super::SharedAssets) -> Self {
        Self {
            active: 0,
            next: 0,
            context: None,
            resources,
            frames: std::rc::Rc::default(),
        }
    }

    /// Transfers a completed view at its original slot or rejects unfinished/rejected work.
    fn harvest(&mut self) -> Result<(), NcStatus> {
        let context = self.context.as_mut().ok_or(UI_E_BUSY)?;
        if let Some(error) = context.gpu_error {
            return Err(error);
        }
        let Some(mut pending) = context.view.take() else {
            return Ok(());
        };
        let result = (|| match pending.session.state()? {
            RenderViewState::Committed => {
                let block = pending.session.take_block()?;
                context.display_list.complete_gpu_view(pending.slot, block)
            }
            RenderViewState::Cancelled => Ok(()),
            _ => Err(RENDER_E_STATE),
        })();
        // Host return always revokes the facade, even if the widget retained its owner.
        let _ = pending.session.expire();
        if let Err(error) = result {
            context.gpu_error = Some(error);
        }
        result
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
    /// Validates scope and reserves aggregate intake before any caller-buffer scan or allocation.
    fn reserve(&mut self, scope: PaintScope, bytes: usize, work: usize) -> Result<(), NcStatus> {
        self.check(scope)?;
        self.context.as_mut().ok_or(UI_E_BUSY)?.display_list.reserve_draw(bytes, work)
    }

    /// Appends one operation directly to the active frame with an authoritative narrowed clip.
    fn with_painter<T>(&mut self, scope: PaintScope, draw: impl FnOnce(&mut crate::render::Painter<'_>) -> Result<T, NcStatus>) -> Result<T, NcStatus> {
        self.check(scope)?;
        let context = self.context.as_mut().ok_or(UI_E_BUSY)?;
        let requested = local_rect(scope.clip).translated(crate::Vec2i::new(context.content_rect.x, context.content_rect.y));
        let clip = context
            .screen_clip
            .positive_intersection(requested)
            .unwrap_or_else(|| crate::Recti::new(requested.x, requested.y, 0, 0));
        let mut painter = crate::render::Painter::for_widget(&mut context.display_list, context.content_rect, clip);
        draw(&mut painter)
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
    match state {
        WidgetVisualState::Disabled => Ok(crate::ControlState::Disabled),
        WidgetVisualState::Normal => Ok(crate::ControlState::Enabled(crate::PointerState::Normal)),
        WidgetVisualState::Hovered => Ok(crate::ControlState::Enabled(crate::PointerState::Hovered)),
        WidgetVisualState::Pressed => Ok(crate::ControlState::Enabled(crate::PointerState::Pressed)),
        WidgetVisualState::Focused => Ok(crate::ControlState::Focused(crate::PointerState::Normal)),
        WidgetVisualState::FocusedHovered => Ok(crate::ControlState::Focused(crate::PointerState::Hovered)),
        WidgetVisualState::FocusedPressed => Ok(crate::ControlState::Focused(crate::PointerState::Pressed)),
        _ => Err(NC_E_INVALID_ARG),
    }
}

/// Decodes the only two states meaningful for structural surfaces.
fn local_surface_state(state: WidgetVisualState) -> Result<crate::SurfaceState, NcStatus> {
    match state {
        WidgetVisualState::Disabled => Ok(crate::SurfaceState::Disabled),
        WidgetVisualState::Normal => Ok(crate::SurfaceState::Normal),
        _ => Err(NC_E_INVALID_ARG),
    }
}

impl IPainter for Recorder {
    /// The root comes from committed visible geometry rather than a supplied rectangle/token.
    fn begin_render_view(&mut self, scope: PaintScope, resources: ComRef<'_, IRenderResourcesAbi>) -> Result<ComBox<IRenderViewAbi>, NcStatus> {
        self.check(scope)?;
        self.harvest()?;
        let context = self.context.as_mut().ok_or(UI_E_BUSY)?;
        let visible = context.screen_clip.positive_intersection(context.content_rect).ok_or(NC_E_INVALID_ARG)?;
        let local = visible.relative_to(crate::Vec2i::new(context.content_rect.x, context.content_rect.y));
        let root = RenderViewRoot {
            client: copied_rect(local),
            target: copied_rect(visible),
        };
        let mut session = self.frames.view(root, resources)?;
        let view = session.view()?;
        let slot = context.display_list.reserve_gpu_view(visible)?;
        context.view = Some(PendingView { slot, session });
        Ok(view)
    }
    /// Validates complete scalar geometry before appending anything to the frame.
    fn stroke_line(&mut self, scope: PaintScope, from: PaintPoint, to: PaintPoint, width: f32, rgba: u32) -> Result<(), NcStatus> {
        self.reserve(scope, 0, 6)?;
        if !from.x.is_finite() || !from.y.is_finite() || !to.x.is_finite() || !to.y.is_finite() || !width.is_finite() || width < 0.0 {
            return Err(NC_E_INVALID_ARG);
        }
        self.with_painter(scope, |painter| {
            painter.stroke_line(
                crate::Vec2f::new(from.x, from.y),
                crate::Vec2f::new(to.x, to.y),
                width,
                crate::render::contract_color(rgba),
            )
        })
    }
    /// Copies a whole finite boundary; native geometry code owns all triangulation and clipping.
    fn fill_polygon(&mut self, scope: PaintScope, input: NcCopyFromBufferOffset<'_, PaintPoint>, rgba: u32) -> Result<(), NcStatus> {
        self.check(scope)?;
        self.context.as_mut().ok_or(UI_E_BUSY)?.display_list.reserve_polygon(input.len())?;
        if input.offset() != 0 || input.as_slice().iter().any(|p| !p.x.is_finite() || !p.y.is_finite()) {
            return Err(NC_E_INVALID_ARG);
        }
        let points = input.as_slice();
        self.with_painter(scope, |painter| painter.fill_contract_polygon(points, crate::render::contract_color(rgba)))
    }

    /// Validates a complete solid batch before appending; invalid or expired calls publish nothing.
    fn colored_triangles(&mut self, scope: PaintScope, input: NcCopyFromBufferOffset<'_, ColoredVertex>) -> Result<(), NcStatus> {
        self.reserve(scope, core::mem::size_of_val(input.as_slice()), input.len())?;
        let vertices = input.as_slice();
        if input.offset() != 0 || !vertices.len().is_multiple_of(3) || vertices.iter().any(|v| !v.x.is_finite() || !v.y.is_finite()) {
            return Err(NC_E_INVALID_ARG);
        }
        self.with_painter(scope, |painter| painter.colored_triangles(vertices))
    }

    /// Retains the resource outside the command borrow; validation at submission authenticates its owner.
    fn texture(&mut self, scope: PaintScope, rect: Rect, source: TextureRegion, rgba: u32, texture: ComRef<'_, ITextureAbi>) -> Result<(), NcStatus> {
        self.reserve(scope, 0, 4)?;
        if rect.width < 0 || rect.height < 0 {
            return Err(NC_E_INVALID_ARG);
        }
        source.validate()?;
        self.check(scope)?;
        // AddRef can execute foreign code, so retain the texture before taking the paint context.
        let texture = texture.try_add_ref()?;
        self.with_painter(scope, |painter| {
            painter.com_texture_region(local_rect(rect), source, crate::render::contract_color(rgba), texture)
        })
    }
    /// Validates the whole batch before publishing any geometry to this scope.
    fn triangles(&mut self, scope: PaintScope, input: NcCopyFromBufferOffset<'_, RenderVertex>) -> Result<(), NcStatus> {
        self.reserve(scope, core::mem::size_of_val(input.as_slice()), input.len())?;
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
        self.reserve(scope, 0, 54)?;
        if rect.width < 0 || rect.height < 0 {
            return Err(NC_E_INVALID_ARG);
        }
        self.with_painter(scope, |painter| painter.fill_rect(local_rect(rect), crate::render::contract_color(rgba)))
    }
    /// Appends text while its borrowed UTF-8 input remains valid for this synchronous call.
    fn text(&mut self, scope: PaintScope, rect: Rect, input: NcCopyFromBufferOffset<'_, u8>) -> Result<(), NcStatus> {
        self.reserve(scope, input.len(), input.len())?;
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
            let color = skin
                .control(ControlRole::Button, crate::ControlState::Enabled(crate::PointerState::Normal))
                .content_color;
            let position = crate::ui_node::text_layout::control_text_position_with_font(skin, atlas, font, label, rect, WidgetOption::NONE);
            (font, color, position)
        };
        self.with_painter(scope, |painter| painter.with_clip(rect, |painter| painter.text(font, label, position, color)))
    }

    /// Resolves one compact semantic patch in the runtime and records it without widget-local geometry.
    fn frame(&mut self, scope: PaintScope, rect: Rect, role: WidgetFrame, state: WidgetVisualState, center_only: bool) -> Result<Rect, NcStatus> {
        self.reserve(scope, 0, 54)?;
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
        self.reserve(scope, 0, 6)?;
        if rect.width < 0 || rect.height < 0 {
            return Err(NC_E_INVALID_ARG);
        }
        let icon = self.resources.borrow().bundle().atlas().icon_at_index(icon.index).ok_or(NC_E_INVALID_ARG)?;
        self.with_painter(scope, |painter| painter.icon(icon, local_rect(rect), crate::render::contract_color(rgba)))
    }

    /// Authenticates and records one exact-font UTF-8 run without an intermediate text command.
    fn text_run(&mut self, scope: PaintScope, font: PaintFont, position: Point, rgba: u32, input: NcCopyFromBufferOffset<'_, u8>) -> Result<(), NcStatus> {
        self.reserve(scope, input.len(), input.len())?;
        let label = text(input)?;
        let font = self.resources.borrow().bundle().atlas().font_at_index(font.index).ok_or(NC_E_INVALID_ARG)?;
        self.with_painter(scope, |painter| {
            painter.text(font, label, crate::Vec2i::new(position.x, position.y), crate::render::contract_color(rgba))
        })
    }
}

/// Runtime-owned allocator of permanently distinct paint capabilities.
#[derive(Clone)]
pub(crate) struct PaintService {
    /// Resource changes remain visible to newly created invocation objects.
    resources: super::SharedAssets,
    /// Shared by mounted widgets; exhaustion fails before taking their display list.
    next: std::rc::Rc<std::cell::Cell<u64>>,
    /// Private per-traversal GPU frame shared with invocation recorders.
    frames: std::rc::Rc<gpu::Frames>,
}

impl PaintService {
    /// Shares resources and the checked sequence, never a reusable public painter identity.
    pub(crate) fn new(resources: super::SharedAssets) -> Self {
        Self {
            resources,
            next: std::rc::Rc::new(std::cell::Cell::new(0)),
            frames: std::rc::Rc::default(),
        }
    }

    /// Installs the trusted broker without returning its capability to widget services.
    pub(crate) fn install_broker(&self, broker: ComRef<'_, IRenderViewBrokerAbi>) -> Result<(), NcStatus> {
        self.frames.install(broker)
    }

    /// Opens one quota/accounting frame around all widget paint invocations.
    pub(crate) fn begin_frame(&self) -> Result<gpu::Guard, NcStatus> {
        self.frames.begin()
    }

    /// Moves the existing buffers through one unique COM allocation and revokes it on return.
    pub(super) fn paint(&self, widget: &neocogi_com::ComBox<IWidgetAbi>, ctx: &mut crate::WidgetPaintCtx<'_>) -> Result<(), NcStatus> {
        let invocation = self.next.get().checked_add(1).ok_or(neocogi_com::NC_E_OUT_OF_RANGE)?;
        self.next.set(invocation);
        let mut value = Recorder::new(self.resources.clone());
        value.next = invocation - 1;
        value.frames = self.frames.clone();
        value.begin(PaintContext {
            display_list: ctx.take_display_list(),
            content_rect: ctx.paint_content_rect(),
            screen_clip: ctx.paint_screen_clip(),
            view: None,
            gpu_error: None,
        })?;
        let recorder = Recorder::into_local(value);
        let painter: ComPtr<IPainterAbi> = recorder.query()?;
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
        let mut widget = widget.as_com_ref();
        let result = widget.paint(PaintScope { invocation, clip: state.clip }, state, painter.as_com_ref());
        // Typed construction proves the implementation; local access uses the same gate as COM.
        // No raw implementation cast or capability from a different invocation participates.
        let harvested = recorder.try_borrow_mut()?.harvest();
        let context = recorder.try_borrow_mut()?.finish(invocation)?;
        ctx.restore_display_list(context.display_list);
        result.and(harvested)
    }
}

#[cfg(test)]
#[path = "paint_service/tests.rs"]
mod gpu_tests;

#[cfg(test)]
mod intake_tests {
    use super::*;
    use neocogi_render_core::abi::{MAX_DRAW_INPUT_BYTES, MAX_POLYGON_POINTS};

    /// Budget rejection precedes input parsing/tessellation and survives changes of widget scope.
    #[test]
    fn oversized_inputs_are_rejected_before_work_and_budget_follows_the_frame() {
        let bundle = crate::SkinBundle::from_atlas(crate::test_support::test_atlas());
        let source = crate::test_support::assets::export_bundle(&bundle).unwrap();
        let resources = crate::assets::ImportedAssets::read(source.as_com_ref(), None).unwrap();
        let mut recorder = Recorder::new(std::rc::Rc::new(std::cell::RefCell::new(std::rc::Rc::new(resources))));
        let context = PaintContext {
            display_list: crate::render::DisplayList::new(),
            content_rect: crate::Recti::new(0, 0, 32, 32),
            screen_clip: crate::Recti::new(0, 0, 32, 32),
            view: None,
            gpu_error: None,
        };
        let invocation = recorder.begin(context).unwrap();
        let scope = PaintScope {
            invocation,
            clip: Rect { x: 0, y: 0, width: 32, height: 32 },
        };
        let points = vec![PaintPoint { x: f32::NAN, y: 0.0 }; MAX_POLYGON_POINTS + 1];
        assert_eq!(
            recorder.fill_polygon(scope, NcCopyFromBufferOffset::new(0, &points), 0),
            Err(neocogi_com::NC_E_OUT_OF_RANGE)
        );
        recorder.reserve(scope, MAX_DRAW_INPUT_BYTES, 0).unwrap();
        // Invalid UTF-8 would normally report INVALID_ARG, but the byte budget rejects it first.
        assert_eq!(
            recorder.text(scope, scope.clip, NcCopyFromBufferOffset::new(0, &[255])),
            Err(neocogi_com::NC_E_OUT_OF_RANGE)
        );
        let context = recorder.finish(invocation).unwrap();
        assert!(context.display_list.is_empty());
        let invocation = recorder.begin(context).unwrap();
        let next = PaintScope { invocation, ..scope };
        assert_eq!(
            recorder.text(next, next.clip, NcCopyFromBufferOffset::new(0, b"x")),
            Err(neocogi_com::NC_E_OUT_OF_RANGE)
        );
        let mut context = recorder.finish(invocation).unwrap();
        context.display_list.clear();
        let invocation = recorder.begin(context).unwrap();
        let next = PaintScope { invocation, ..scope };
        recorder.text(next, next.clip, NcCopyFromBufferOffset::new(0, b"x")).unwrap();
        assert!(!recorder.finish(invocation).unwrap().display_list.is_empty());
    }
}
