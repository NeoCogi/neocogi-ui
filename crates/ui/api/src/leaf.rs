//! Static application leaf implementations share lifecycle plumbing in one concrete COM allocation.
//!
//! Use this helper for local behavior whose editing state is already concrete application data.
//! Classes exposing their own editing interfaces can instead compose WidgetState and implement
//! IWidget directly. This helper creates no registered class, native node, erased value or Rust vtable.
use crate::*;
use crate::com::widget::WidgetState;
use neocogi_com::{com_object, ComPtr, ComRef, NcCopyFromBufferOffset, NcStatus};

/// Statically dispatched behavior stored inline in one application-owned COM widget.
pub trait LeafContent: 'static {
    /// Keyboard participation bits use the same copied contract as IWidget::properties.
    const KEYBOARD: u16 = 0;
    /// Theme frame role shared with ordinary controls; options choose whether to draw it.
    const FRAME: WidgetFrame = WidgetFrame::Generic;
    /// Computes preferred content dimensions under validated finite/unbounded constraints.
    fn measure(&self, bounds: LayoutLimits, metrics: ComRef<'_, ITextMetricsAbi>) -> Result<Size, NcStatus>;
    /// Processes a validated copied event or an ordinary eventless update. Report CHANGED whenever
    /// external/local state changes measurement inputs; skipped hidden updates retain the documented
    /// author obligation to synchronize through a setter or a later truthful update result.
    fn update(&mut self, _state: WidgetUpdate, _event: WidgetEvent, _text: &str) -> Result<GeometryChange, NcStatus> {
        Ok(GeometryChange::NoChange)
    }
    /// Records observational drawing through this invocation's checked painter capability.
    fn paint(&self, state: WidgetPaint, painter: &mut Painter<'_>) -> Result<(), NcStatus>;
    /// Handles at most one private message after traversal releases the widget.
    /// Passive content uses this no-op and allocates no event queue.
    fn dispatch_next(&mut self) -> Result<bool, NcStatus> {
        Ok(false)
    }

}

#[com_object(interfaces(IWidget, ILeafLayout), ownership = exclusive)]
/// The generic parameter specializes ordinary Rust calls; only concrete IWidget/ILeafLayout cross COM.
struct LeafObject<Content: LeafContent> {
    /// One common options, mount, closure and geometry-invalidation state.
    state: WidgetState,
    /// Statically dispatched application behavior owned directly by this COM object.
    content: Content,
}
impl<Content: LeafContent> IWidget for LeafObject<Content> {
    /// Supplies the same runtime authority used by built-in COM objects.
    fn host(&self) -> Result<ComPtr<IWidgetHostAbi>, NcStatus> {
        Ok(self.state.host().add_ref())
    }
    /// Mount-once semantics come from the single common state, not the content behavior.
    fn mount(&mut self) -> Result<(), NcStatus> {
        self.state.mount()
    }
    /// Shares the widget's COM borrow gate and lifecycle with its private behavior.
    fn dispatch_next(&mut self) -> Result<bool, NcStatus> {
        self.state.writable()?;
        self.content.dispatch_next()
    }

    /// Common observations read only lifecycle state and remain safe during shared content calls.
    fn properties(&self) -> Result<WidgetProperties, NcStatus> {
        Ok(WidgetProperties {
            options: self.state.requested_flags()?,
            keyboard: Content::KEYBOARD,
            frame: Content::FRAME,
        })
    }
    /// Options and their sticky geometry effect follow the standard widget contract.
    fn replace_flags(&mut self, flags: WidgetOptions) -> Result<(), NcStatus> {
        self.state.replace_flags(flags)
    }
    /// Sets current requested bits through the same widget-owned mutation authority.
    fn set_flags(&mut self, flags: WidgetOptions) -> Result<(), NcStatus> {
        self.state.set_flags(flags)
    }
    /// Clears requested bits without copying effective properties back into settings.
    fn clear_flags(&mut self, flags: WidgetOptions) -> Result<(), NcStatus> {
        self.state.clear_flags(flags)
    }
    /// Validates all copied input before mutating content; changes remain sticky until collected.
    fn update(&mut self, state: WidgetUpdate, event: WidgetEvent, text: NcCopyFromBufferOffset<'_, u8>) -> Result<GeometryChange, NcStatus> {
        self.state.writable()?;
        state.validate()?;
        let (event, text) = decode_widget_event(event, text)?;
        let change = self.content.update(state, event, text).inspect_err(|_| {
            // A failed behavior update may have committed part of its local state. Retain a
            // conservative geometry marker so recovery cannot reuse an earlier measurement.
            self.state.record_change(GeometryChange::Changed);
        })?;
        if change != GeometryChange::Changed && change != GeometryChange::NoChange {
            self.state.record_change(GeometryChange::Changed);
            return Err(neocogi_com::NC_E_INVALID_ARG);
        }
        self.state.record_change(change);
        Ok(change)
    }
    /// Setter and update invalidation use the same collection path.
    fn take_change(&mut self) -> Result<GeometryChange, NcStatus> {
        self.state.take_change()
    }
    /// Content receives a borrowing convenience; retaining raw paint authority still expires normally.
    fn paint(&mut self, scope: PaintScope, state: WidgetPaint, painter: ComRef<'_, IPainterAbi>) -> Result<(), NcStatus> {
        self.state.readable()?;
        state.validate()?;
        self.content.paint(state, &mut Painter::from_call(painter, scope))
    }
}
impl<Content: LeafContent> ILeafLayout for LeafObject<Content> {
    /// Observational preferred-size calls reject stale lifetime, malformed limits and content reentry.
    fn measure(&mut self, bounds: LayoutLimits, metrics: ComRef<'_, ITextMetricsAbi>) -> Result<Size, NcStatus> {
        self.state.readable()?;
        bounds.width.validate()?;
        bounds.height.validate()?;
        self.content.measure(bounds, metrics)
    }
}

impl WidgetFactory {
    /// Creates one local COM leaf with statically dispatched inline content and normal runtime authority.
    pub fn leaf<Content: LeafContent>(&self, content: Content, options: WidgetOptions) -> Result<OwnedWidget<IWidgetAbi>, NcStatus> {
        options.validate()?;
        let object = LeafObject::into_box(LeafObject {
            state: WidgetState::new(self.host()?, options),
            content,
        });
        Ok(OwnedWidget::from_interface(object.query()?))
    }
}
