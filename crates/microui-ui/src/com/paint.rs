//! Owned recording services keep COM retention independent of stack borrows.
use super::*;
use crate::prelude::{FontRole, WidgetOption, ControlRole};

#[com_class(interfaces(ITextMetrics))]
/// Immutable text metrics for one installed resource bundle.
pub(super) struct Metrics {
    /// Exact immutable revision also shared by the concrete built-in objects.
    pub(super) resources: Rc<super::leaf::Resources>,
}

impl ITextMetrics for Metrics {
    /// Uses the existing inline-content algorithm so migration preserves sizing.
    fn button_size(&self, input: NcCopyFromBufferOffset<'_, u8>) -> Result<Size, NcStatus> {
        let label = text(input)?;
        let size = crate::ui_node::widgets::control::inline_content_size(
            &self.resources.skin,
            &self.resources.atlas,
            &crate::FontRef::Role(FontRole::Body),
            label,
            None,
        );
        Ok(Size { width: size.width, height: size.height })
    }
}

/// Commands contain only owned values and are replayed after the component call.
enum Command {
    /// Solid geometry in the current widget's local coordinates.
    Fill(Rect, u32),
    /// Complete, validated local atlas triangles copied before the call returns.
    Triangles(Vec<RenderVertex>),
    /// Owned UTF-8 text and its local allocation.
    Text(Rect, String),
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
        } else {
            Ok(())
        }
    }
}

impl IPainter for Recorder {
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
        self.recording
            .commands
            .try_borrow_mut()
            .map_err(|_| UI_E_BUSY)?
            .push(Command::Triangles(vertices.to_vec()));
        Ok(())
    }
    /// Stores a checked fill command without accessing the calling stack's painter.
    fn fill(&self, scope: PaintScope, rect: Rect, rgba: u32) -> Result<(), NcStatus> {
        self.check(scope)?;
        if rect.width < 0 || rect.height < 0 {
            return Err(NC_E_INVALID_ARG);
        }
        self.recording.commands.try_borrow_mut().map_err(|_| UI_E_BUSY)?.push(Command::Fill(rect, rgba));
        Ok(())
    }
    /// Copies text while its input buffer is valid; replay uses only the owned copy.
    fn text(&self, scope: PaintScope, rect: Rect, input: NcCopyFromBufferOffset<'_, u8>) -> Result<(), NcStatus> {
        self.check(scope)?;
        if rect.width < 0 || rect.height < 0 {
            return Err(NC_E_INVALID_ARG);
        }
        let label = text(input)?.to_owned();
        self.recording
            .commands
            .try_borrow_mut()
            .map_err(|_| UI_E_BUSY)?
            .push(Command::Text(rect, label));
        Ok(())
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
        self.recording.commands.borrow_mut().clear();
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
        let result = widget.paint(PaintScope { invocation }, state, self.painter.as_com_ref());
        drop(guard);
        result?;
        // No component is called while the queue is borrowed; replay is private Rust code.
        for command in self.recording.commands.borrow_mut().drain(..) {
            match command {
                Command::Triangles(vertices) => ctx.painter().atlas_triangles(&vertices),
                Command::Fill(rect, rgba) => ctx.painter().fill_rect(local_rect(rect), super::render::color(rgba)),
                Command::Text(rect, label) => {
                    let font = ctx.skin().resolve_font_role(ctx.atlas(), FontRole::Body);
                    ctx.draw_control_text_with_font(font, &label, local_rect(rect), ControlRole::Button, WidgetOption::NONE);
                }
            }
        }
        Ok(())
    }
}
