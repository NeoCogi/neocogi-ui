//! Owned recording services keep COM retention independent of stack borrows.
use super::*;
use crate::{FontRole, WidgetOption, ControlRole};

#[com_class(interfaces(ITextMetrics))]
/// Immutable text metrics for one installed resource bundle.
pub(super) struct Metrics {
    /// Exact immutable revision also shared by the concrete built-in objects.
    pub(super) resources: Rc<super::widget::Resources>,
}

impl ITextMetrics for Metrics {
    /// Uses the existing inline-content algorithm so migration preserves sizing.
    fn button_size(&self, input: NcCopyFromBufferOffset<'_, u8>) -> Result<Size, NcStatus> {
        let label = text(input)?;
        let resources = self.resources.current();
        let size = crate::ui_node::widgets::control::inline_content_size(
            resources.assets.bundle().skin(),
            resources.assets.bundle().atlas(),
            &crate::FontRef::Role(FontRole::Body),
            label,
            None,
        );
        Ok(Size { width: size.width, height: size.height })
    }
}

/// Commands contain only owned values and are replayed after the component call.
enum Drawing {
    /// Solid geometry in the current widget's local coordinates.
    Fill(Rect, u32),
    /// Finite endpoints, nonnegative width and packed RGBA for UI-owned line tessellation.
    Line(PaintPoint, PaintPoint, f32, u32),
    /// Owned finite polygon boundary uses the existing shared UI triangulation algorithm.
    Polygon(Vec<crate::Vec2f>, u32),
    /// Complete, validated local atlas triangles copied before the call returns.
    Triangles(Vec<RenderVertex>),
    /// Solid vertices retain their material meaning until the UI executor resolves the current atlas.
    ColoredTriangles(Vec<ColoredVertex>),
    /// Texture capability retained until the prepared frame has finished or been abandoned.
    Texture(Rect, TextureRegion, u32, ComPtr<ITextureAbi>),
    /// Application GPU work retains its precise position among ordinary primitives.
    Item(Rect, ComPtr<IRenderItemAbi>),
    /// Owned UTF-8 text and its local allocation.
    Text(Rect, String),
}

/// Each operation owns its clip by value; later nested drawing cannot mutate earlier commands.
struct Command {
    /// Requested local clip is intersected with the authoritative widget clip during replay.
    clip: Rect,
    /// Owned typed drawing payload, including retained resources where needed.
    drawing: Drawing,
}

/// One reusable command allocation and monotonically increasing invocation clock.
#[derive(Default)]
struct Recording {
    /// Zero means no caller currently has authority to record.
    active: Cell<u64>,
    /// Never wraps: exhaustion rejects the next invocation.
    next: Cell<u64>,
    /// Commands are drained without retaining any painter/context references.
    commands: RefCell<Vec<Command>>,
}

#[com_class(interfaces(IPainter))]
/// Retainable COM service owning only recording storage.
struct Recorder {
    /// Shared storage also retained by the runtime's private driver.
    recording: Rc<Recording>,
}

impl Recorder {
    /// Validates authority before touching the queue, including empty operations.
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
    /// Publishes one fully owned operation after validation, without calling out under the queue borrow.
    fn push(&self, scope: PaintScope, drawing: Drawing) -> Result<(), NcStatus> {
        self.recording
            .commands
            .try_borrow_mut()
            .map_err(|_| UI_E_BUSY)?
            .push(Command { clip: scope.clip, drawing });
        Ok(())
    }
}

impl IPainter for Recorder {
    /// Validates complete scalar geometry before any command is visible to replay.
    fn stroke_line(&self, scope: PaintScope, from: PaintPoint, to: PaintPoint, width: f32, rgba: u32) -> Result<(), NcStatus> {
        self.check(scope)?;
        if !from.x.is_finite() || !from.y.is_finite() || !to.x.is_finite() || !to.y.is_finite() || !width.is_finite() || width < 0.0 {
            return Err(NC_E_INVALID_ARG);
        }
        self.push(scope, Drawing::Line(from, to, width, rgba))
    }
    /// Copies a whole finite boundary; native geometry code owns all triangulation and clipping.
    fn fill_polygon(&self, scope: PaintScope, input: NcCopyFromBufferOffset<'_, PaintPoint>, rgba: u32) -> Result<(), NcStatus> {
        self.check(scope)?;
        if input.offset() != 0 || input.as_slice().iter().any(|p| !p.x.is_finite() || !p.y.is_finite()) {
            return Err(NC_E_INVALID_ARG);
        }
        let points = input.as_slice().iter().map(|p| crate::Vec2f::new(p.x, p.y)).collect();
        self.push(scope, Drawing::Polygon(points, rgba))
    }

    /// Validates a complete solid batch before recording; invalid or expired calls publish nothing.
    fn colored_triangles(&self, scope: PaintScope, input: NcCopyFromBufferOffset<'_, ColoredVertex>) -> Result<(), NcStatus> {
        self.check(scope)?;
        let vertices = input.as_slice();
        if input.offset() != 0 || !vertices.len().is_multiple_of(3) || vertices.iter().any(|v| !v.x.is_finite() || !v.y.is_finite()) {
            return Err(NC_E_INVALID_ARG);
        }
        self.push(scope, Drawing::ColoredTriangles(vertices.to_vec()))
    }

    /// Retains the resource outside the command borrow; validation at submission authenticates its owner.
    fn texture(&self, scope: PaintScope, rect: Rect, source: TextureRegion, rgba: u32, texture: ComRef<'_, ITextureAbi>) -> Result<(), NcStatus> {
        self.check(scope)?;
        if rect.width < 0 || rect.height < 0 {
            return Err(NC_E_INVALID_ARG);
        }
        source.validate()?;
        let texture = texture.add_ref();
        self.push(scope, Drawing::Texture(rect, source, rgba, texture))
    }
    /// Records a capability rather than a callback lending mutable GPU state across COM.
    fn item(&self, scope: PaintScope, rect: Rect, item: ComRef<'_, IRenderItemAbi>) -> Result<(), NcStatus> {
        self.check(scope)?;
        if rect.width < 0 || rect.height < 0 {
            return Err(NC_E_INVALID_ARG);
        }
        let item = item.add_ref();
        self.push(scope, Drawing::Item(rect, item))
    }
    /// Validates the whole batch before publishing any geometry to this scope.
    fn triangles(&self, scope: PaintScope, input: NcCopyFromBufferOffset<'_, RenderVertex>) -> Result<(), NcStatus> {
        self.check(scope)?;
        let vertices = input.as_slice();
        if input.offset() != 0
            || !vertices.len().is_multiple_of(3)
            || vertices
                .iter()
                .any(|v| !v.x.is_finite() || !v.y.is_finite() || !v.u.is_finite() || !v.v.is_finite())
        {
            return Err(NC_E_INVALID_ARG);
        }
        self.push(scope, Drawing::Triangles(vertices.to_vec()))
    }
    /// Stores a checked fill command without accessing the calling stack's painter.
    fn fill(&self, scope: PaintScope, rect: Rect, rgba: u32) -> Result<(), NcStatus> {
        self.check(scope)?;
        if rect.width < 0 || rect.height < 0 {
            return Err(NC_E_INVALID_ARG);
        }
        self.push(scope, Drawing::Fill(rect, rgba))
    }
    /// Copies text while its input buffer is valid; replay uses only the owned copy.
    fn text(&self, scope: PaintScope, rect: Rect, input: NcCopyFromBufferOffset<'_, u8>) -> Result<(), NcStatus> {
        self.check(scope)?;
        if rect.width < 0 || rect.height < 0 {
            return Err(NC_E_INVALID_ARG);
        }
        let label = text(input)?.to_owned();
        self.push(scope, Drawing::Text(rect, label))
    }
}

/// Reusable driver; one instance serves sequential widget paint calls in a runtime.
#[derive(Clone)]
pub(super) struct PaintService {
    /// Storage is independent of the COM object's allocation and every stack frame.
    recording: Rc<Recording>,
    /// Checked capability passed to widgets without an AddRef per primitive.
    painter: ComPtr<IPainterAbi>,
}

/// Private scope guard expires authority on both success and error paths.
struct Invocation<'a>(&'a Recording);
impl Drop for Invocation<'_> {
    /// Invalidate before commands are replayed or a later widget is invoked.
    fn drop(&mut self) {
        self.0.active.set(0);
    }
}

impl PaintService {
    /// Allocates one service for the runtime, not one object per primitive.
    pub(super) fn new() -> Self {
        let recording = Rc::new(Recording::default());
        let painter = Recorder::from_value(Recorder { recording: recording.clone() })
            .query()
            .expect("Recorder declares IPainter");
        Self { recording, painter }
    }
    /// Calls a component and then replays its complete command batch into the legacy executor.
    pub(super) fn paint(&self, widget: &ComPtr<IWidgetAbi>, ctx: &mut crate::WidgetPaintCtx<'_>) -> Result<(), NcStatus> {
        if self.recording.active.get() != 0 {
            return Err(UI_E_BUSY);
        }
        let invocation = self.recording.next.get().checked_add(1).ok_or(nc_com::NC_E_OUT_OF_RANGE)?;
        self.recording.next.set(invocation);
        // Resource Release may reenter. Detach old values before destruction and preserve the allocation.
        let mut commands = std::mem::take(&mut *self.recording.commands.borrow_mut());
        commands.clear();
        *self.recording.commands.borrow_mut() = commands;
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
        let result = widget.paint(PaintScope { invocation, clip: state.clip }, state, self.painter.as_com_ref());
        drop(guard);
        // Failed paint abandons every retained resource as well; never run Release under the queue borrow.
        let mut commands = std::mem::take(&mut *self.recording.commands.borrow_mut());
        if let Err(error) = result {
            commands.clear();
            *self.recording.commands.borrow_mut() = commands;
            return Err(error);
        }
        for Command { clip, drawing } in commands.drain(..) {
            let mut clipped = ctx.clipped(local_rect(clip));
            let ctx = &mut clipped;
            match drawing {
                Drawing::Line(from, to, width, rgba) => ctx.painter().stroke_line(
                    crate::Vec2f::new(from.x, from.y),
                    crate::Vec2f::new(to.x, to.y),
                    width,
                    super::render::color(rgba),
                ),
                Drawing::Polygon(points, rgba) => ctx.painter().fill_polygon(&points, super::render::color(rgba)),
                Drawing::Texture(rect, source, rgba, texture) => {
                    ctx.painter().com_texture_region(local_rect(rect), source, super::render::color(rgba), texture)
                }
                Drawing::Item(rect, item) => ctx.painter().com_item(local_rect(rect), item),
                Drawing::ColoredTriangles(vertices) => ctx.painter().colored_triangles(&vertices),
                Drawing::Triangles(vertices) => ctx.painter().atlas_triangles(&vertices),
                Drawing::Fill(rect, rgba) => ctx.painter().fill_rect(local_rect(rect), super::render::color(rgba)),
                Drawing::Text(rect, label) => {
                    let font = ctx.skin().resolve_font_role(ctx.atlas(), FontRole::Body);
                    ctx.draw_control_text_with_font(font, &label, local_rect(rect), ControlRole::Button, WidgetOption::NONE);
                }
            }
        }
        *self.recording.commands.borrow_mut() = commands;
        Ok(())
    }
}
