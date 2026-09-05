//! Concrete button storage and its exclusive, typed event subscription.
use super::*;
use super::leaf::{Leaf, Resources};
use nc_com::{INcStringAbi, NcString};
use std::collections::VecDeque;

#[com_class(interfaces(IButton, ILeafLayout, IButtonEvents, IFontStyle, IIconContent, IButtonStyle))]
/// One authoritative button allocation shared by editing, layout, paint and input.
pub(super) struct Button {
    /// Shared mount, closure and geometry rules live inside this one allocation.
    leaf: Leaf<crate::Button>,
    /// Producer owns queue data, never the application's sink or subscription.
    queue: Rc<Queue>,
}

/// Shared source data contains only immutable event snapshots and connection state.
#[derive(Default)]
struct Queue {
    /// FIFO snapshots detached before any application callback.
    pending: RefCell<VecDeque<ComPtr<INcStringAbi>>>,
    /// Exclusive subscriber presence; cleared by cancel or final lease release.
    connected: Cell<bool>,
    /// Source closure prevents new ordinary events and discards queued ones.
    closed: Cell<bool>,
}

impl Button {
    /// Constructs an unmounted concrete button for the supplied runtime.
    pub(super) fn create(host: ComPtr<IWidgetHostAbi>, resources: Rc<Resources>) -> ComPtr<IButtonAbi> {
        Self::from_value(Self {
            leaf: Leaf::new(host, resources, crate::Button::from(crate::ButtonParameters::new(""))),
            queue: Rc::default(),
        })
        .query()
        .expect("Button declares IButton")
    }
}

impl IButton for Button {
    /// Validates before committing and only dirties geometry when text actually differs.
    fn set_label(&self, input: NcCopyFromBufferOffset<'_, u8>) -> Result<(), NcStatus> {
        self.leaf.state.writable()?;
        let input = text(input)?;
        let mut core = self.leaf.core.try_borrow_mut().map_err(|_| UI_E_BUSY)?;
        if core.com_label() != input {
            core.com_set_label(input);
            self.leaf.state.record_change(GeometryChange::CHANGED);
        }
        Ok(())
    }
    /// Returns an owned immutable snapshot, never a borrow into widget storage.
    fn label(&self) -> Result<ComPtr<INcStringAbi>, NcStatus> {
        self.leaf.state.readable()?;
        Ok(NcString::from_utf8_str(self.leaf.core.try_borrow().map_err(|_| UI_E_BUSY)?.com_label()))
    }
}

leaf_widget_impl!(Button);

impl Button {
    /// Closes the inherited widget and clears ordinary queued submissions.
    fn close_widget(&self) -> Result<(), NcStatus> {
        self.leaf.state.close()?;
        self.queue.closed.set(true);
        self.queue.pending.try_borrow_mut().map_err(|_| UI_E_BUSY)?.clear();
        Ok(())
    }

    /// Uses the shared button algorithm and queues an immutable user-submission snapshot.
    fn update_widget(&self, state: WidgetUpdate, event: WidgetEventAbi, text: NcCopyFromBufferOffset<'_, u8>) -> Result<GeometryChange, NcStatus> {
        let change = self.leaf.update(state, event, text)?;
        if state.action == WidgetAction::ACTIVATE && self.queue.connected.get() && !self.queue.closed.get() {
            let snapshot = self.label()?;
            self.queue.pending.try_borrow_mut().map_err(|_| UI_E_BUSY)?.push_back(snapshot);
        }
        Ok(change)
    }
}

impl IButtonEvents for Button {
    /// Retains the typed sink only inside the returned application-owned lease.
    fn subscribe(&self, sink: ComRef<'_, IButtonSinkAbi>) -> Result<ComPtr<IEventSubscriptionAbi>, NcStatus> {
        self.leaf.state.writable()?;
        if self.queue.connected.replace(true) {
            return Err(UI_E_BUSY);
        }
        Ok(Subscription::from_value(Subscription {
            queue: self.queue.clone(),
            sink: sink.add_ref(),
            cancelled: Cell::new(false),
            dispatching: Cell::new(false),
        })
        .query()
        .expect("Subscription declares its interface"))
    }
}

#[com_class(interfaces(IEventSubscription))]
/// Application-owned lease; the producer never owns this object or its sink.
struct Subscription {
    /// Queue data is independent of the button's allocation.
    queue: Rc<Queue>,
    /// Concrete typed consumer retained until the lease is released.
    sink: ComPtr<IButtonSinkAbi>,
    /// Distinguishes an old cancelled lease from a later subscription to the same source.
    cancelled: Cell<bool>,
    /// Reentrant dispatch must fail rather than reorder an in-flight batch.
    dispatching: Cell<bool>,
}

impl IEventSubscription for Subscription {
    /// Detaches one FIFO batch before invoking any application code.
    fn dispatch(&self) -> Result<u32, NcStatus> {
        if self.cancelled.get() || self.queue.closed.get() {
            return Ok(0);
        }
        if self.dispatching.replace(true) {
            return Err(UI_E_BUSY);
        }
        let result = self.deliver();
        self.dispatching.set(false);
        result
    }
    /// Disconnects once; releasing an old lease cannot cancel a newer subscriber.
    fn cancel(&self) -> Result<(), NcStatus> {
        if !self.cancelled.replace(true) {
            self.queue.connected.set(false);
            self.queue.pending.try_borrow_mut().map_err(|_| UI_E_BUSY)?.clear();
        }
        Ok(())
    }
}

impl Subscription {
    /// Invokes the sink without a queue borrow, preserving cancellation during delivery.
    fn deliver(&self) -> Result<u32, NcStatus> {
        let batch = std::mem::take(&mut *self.queue.pending.try_borrow_mut().map_err(|_| UI_E_BUSY)?);
        let mut delivered = 0;
        for snapshot in batch {
            if self.cancelled.get() || self.queue.closed.get() {
                break;
            }
            self.sink.submitted(snapshot.as_com_ref())?;
            delivered += 1;
        }
        Ok(delivered)
    }
}

impl Drop for Subscription {
    /// Final release disconnects without calling user code.
    fn drop(&mut self) {
        let _ = self.cancel();
    }
}

impl Drop for Button {
    /// Producer destruction discards ordinary pending events without invoking a retained sink.
    fn drop(&mut self) {
        self.queue.closed.set(true);
        if let Ok(mut pending) = self.queue.pending.try_borrow_mut() {
            pending.clear();
        }
    }
}

font_style_impl!(Button);

icon_content_impl!(Button);
impl IButtonStyle for Button {
    /// Reports the current fill mask from the sole concrete button state.
    fn fill_states(&self) -> Result<FillStates, NcStatus> {
        self.leaf.state.readable()?;
        Ok(FillStates {
            bits: self.leaf.core.try_borrow().map_err(|_| UI_E_BUSY)?.com_fill().bits() as u8,
        })
    }
    /// Rejects unknown flags before committing a paint-only change.
    fn set_fill_states(&self, states: FillStates) -> Result<(), NcStatus> {
        self.leaf.state.writable()?;
        let fill = crate::WidgetFillOption::from_bits(u32::from(states.bits)).ok_or(NC_E_INVALID_ARG)?;
        self.leaf.core.try_borrow_mut().map_err(|_| UI_E_BUSY)?.com_set_fill(fill);
        Ok(())
    }
}
