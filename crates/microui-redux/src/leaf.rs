//! Static application leaf implementations share lifecycle plumbing in one concrete COM allocation.
//!
//! Use this helper for local behavior whose editing state is already concrete application data.
//! Classes exposing their own editing interfaces can instead compose WidgetState and implement
//! IWidget directly. This helper creates no registered class, native node, erased value or Rust vtable.
use crate::*;
use microui_contracts::widget::WidgetState;
use nc_com::{com_class, ComPtr, ComRef, NcCopyFromBufferOffset, NcStatus};
use std::{
    cell::RefCell,
    ops::{Deref, DerefMut},
};

/// Statically dispatched behavior stored inline in one application-owned COM widget.
pub trait LeafContent: 'static {
    /// Keyboard participation bits use the same copied contract as IWidget::properties.
    const KEYBOARD: u16 = 0;
    /// Theme frame role shared with ordinary controls; options choose whether to draw it.
    const FRAME: WidgetFrame = WidgetFrame::GENERIC;
    /// Computes preferred content dimensions under validated finite/unbounded constraints.
    fn measure(&self, bounds: LayoutLimits, metrics: ComRef<'_, ITextMetricsAbi>) -> Result<Size, NcStatus>;
    /// Processes a validated copied event or an ordinary eventless update. Report CHANGED whenever
    /// external/local state changes measurement inputs; skipped hidden updates retain the documented
    /// author obligation to synchronize through a setter or a later truthful update result.
    fn update(&mut self, _state: WidgetUpdate, _event: WidgetEvent, _text: &str) -> Result<GeometryChange, NcStatus> {
        Ok(GeometryChange::NO_CHANGE)
    }
    /// Records observational drawing through this invocation's checked painter capability.
    fn paint(&self, state: WidgetPaint, painter: &Painter<'_>) -> Result<(), NcStatus>;
    /// Releases logical component activity once, including closing any locally owned event producers.
    /// The COM allocation and ordinary retained editing data may survive logical closure.
    fn close(&mut self) -> Result<(), NcStatus> {
        Ok(())
    }
}

#[com_class(interfaces(IWidget, ILeafLayout))]
/// The generic parameter specializes ordinary Rust calls; only concrete IWidget/ILeafLayout cross COM.
struct LeafObject<Content: LeafContent> {
    /// One common options, mount, closure and geometry-invalidation state.
    state: WidgetState,
    /// Behavior moves out for each call so no RefCell guard survives an outward interface call.
    content: RefCell<Option<Content>>,
}
impl<Content: LeafContent> LeafObject<Content> {
    /// Takes exclusive behavior ownership, rejecting nested measurement/update/painting as BUSY.
    fn content(&self) -> Result<ContentAccess<'_, Content>, NcStatus> {
        let value = self.content.try_borrow_mut().map_err(|_| UI_E_BUSY)?.take().ok_or(UI_E_BUSY)?;
        Ok(ContentAccess { slot: &self.content, value: Some(value) })
    }
}
impl<Content: LeafContent> IWidget for LeafObject<Content> {
    /// Supplies the same runtime authority used by built-in COM objects.
    fn host(&self) -> Result<ComPtr<IWidgetHostAbi>, NcStatus> {
        Ok(self.state.host().add_ref())
    }
    /// Mount-once semantics come from the single common state, not the content behavior.
    fn mount(&self) -> Result<(), NcStatus> {
        self.state.mount()
    }
    /// Commits logical closure once and then lets the concrete content terminate its own activity.
    fn close(&self) -> Result<(), NcStatus> {
        let mut content = self.content()?;
        if self.state.close()? {
            content.close()?;
        }
        Ok(())
    }
    /// Common observations need no behavior borrow and remain safe during content call-outs.
    fn properties(&self) -> Result<WidgetProperties, NcStatus> {
        Ok(WidgetProperties {
            options: self.state.options()?,
            keyboard: Content::KEYBOARD,
            frame: Content::FRAME,
        })
    }
    /// Options and their sticky geometry effect follow the standard widget contract.
    fn set_options(&self, options: WidgetOptions) -> Result<(), NcStatus> {
        self.state.set_options(options)
    }
    /// Validates all copied input before taking behavior; changes remain sticky until collected.
    fn update(&self, state: WidgetUpdate, event: WidgetEventAbi, text: NcCopyFromBufferOffset<'_, u8>) -> Result<GeometryChange, NcStatus> {
        self.state.writable()?;
        state.validate()?;
        let (event, text) = decode_widget_event(event, text)?;
        let change = self.content()?.update(state, event, text).inspect_err(|_| {
            // A failed behavior update may have committed part of its local state. Retain a
            // conservative geometry marker so recovery cannot reuse an earlier measurement.
            self.state.record_change(GeometryChange::CHANGED);
        })?;
        if change != GeometryChange::CHANGED && change != GeometryChange::NO_CHANGE {
            self.state.record_change(GeometryChange::CHANGED);
            return Err(nc_com::NC_E_INVALID_ARG);
        }
        self.state.record_change(change);
        Ok(change)
    }
    /// Setter and update invalidation use the same collection path.
    fn take_change(&self) -> Result<GeometryChange, NcStatus> {
        self.state.take_change()
    }
    /// Content receives a borrowing convenience; retaining raw paint authority still expires normally.
    fn paint(&self, scope: PaintScope, state: WidgetPaint, painter: ComRef<'_, IPainterAbi>) -> Result<(), NcStatus> {
        self.state.readable()?;
        state.validate()?;
        self.content()?.paint(state, &Painter::from_call(painter, scope))
    }
}
impl<Content: LeafContent> ILeafLayout for LeafObject<Content> {
    /// Observational preferred-size calls reject stale lifetime, malformed limits and content reentry.
    fn measure(&self, bounds: LayoutLimits, metrics: ComRef<'_, ITextMetricsAbi>) -> Result<Size, NcStatus> {
        self.state.readable()?;
        bounds.width.validate()?;
        bounds.height.validate()?;
        self.content()?.measure(bounds, metrics)
    }
}

/// Stack-owned behavior transaction restores the value on ordinary errors and unwinding alike.
struct ContentAccess<'a, Content> {
    /// The object's slot remains empty while foreign code can reenter it.
    slot: &'a RefCell<Option<Content>>,
    /// Exactly one live behavior value is owned by this transaction.
    value: Option<Content>,
}
impl<Content> Deref for ContentAccess<'_, Content> {
    type Target = Content;
    /// Observational methods borrow only the detached value owned by this stack transaction.
    fn deref(&self) -> &Content {
        self.value.as_ref().expect("live content transaction")
    }
}
impl<Content> DerefMut for ContentAccess<'_, Content> {
    /// Update/closure receives exclusive access without a guard into the COM allocation's slot.
    fn deref_mut(&mut self) -> &mut Content {
        self.value.as_mut().expect("live content transaction")
    }
}
impl<Content> Drop for ContentAccess<'_, Content> {
    /// Restores without calling content code or dropping another value under a slot borrow.
    fn drop(&mut self) {
        *self.slot.borrow_mut() = self.value.take();
    }
}

impl WidgetFactory {
    /// Creates one local COM leaf with statically dispatched inline content and normal runtime authority.
    pub fn leaf<Content: LeafContent>(&self, content: Content, options: WidgetOptions) -> Result<Widget<IWidgetAbi>, NcStatus> {
        options.validate()?;
        let object = LeafObject::from_value(LeafObject {
            state: WidgetState::new(self.host()?, options),
            content: RefCell::new(Some(content)),
        });
        Ok(Widget::from_interface(object.query()?))
    }
}
