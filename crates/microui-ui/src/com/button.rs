//! Concrete button storage and its exclusive, typed event subscription.
use super::*;
use microui_contracts::widget::WidgetState;
use nc_com::{INcStringAbi, NcString};
use std::collections::VecDeque;

#[com_class(interfaces(IButton, ILeafLayout, IButtonEvents))]
/// One authoritative button allocation shared by editing, layout, paint and input.
pub(super) struct Button {
    /// Shared mount, closure and geometry rules live inside this one allocation.
    state: WidgetState,
    /// Current UTF-8 label; snapshots own independent storage.
    label: RefCell<String>,
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
    pub(super) fn create(host: ComPtr<IWidgetHostAbi>) -> ComPtr<IButtonAbi> {
        Self::from_value(Self {
            state: WidgetState::new(host, WidgetOptions::FRAME),
            label: RefCell::new(String::new()),
            queue: Rc::default(),
        })
        .query()
        .expect("Button declares IButton")
    }
}

impl IButton for Button {
    /// Validates before committing and only dirties geometry when text actually differs.
    fn set_label(&self, input: NcCopyFromBufferOffset<'_, u8>) -> Result<(), NcStatus> {
        self.state.writable()?;
        let input = text(input)?;
        let mut label = self.label.try_borrow_mut().map_err(|_| UI_E_BUSY)?;
        if *label != input {
            label.clear();
            label.push_str(input);
            self.state.record_change(GeometryChange::CHANGED);
        }
        Ok(())
    }
    /// Returns an owned immutable snapshot, never a borrow into widget storage.
    fn label(&self) -> Result<ComPtr<INcStringAbi>, NcStatus> {
        self.state.readable()?;
        Ok(NcString::from_utf8_str(&self.label.try_borrow().map_err(|_| UI_E_BUSY)?))
    }
}

impl IWidget for Button {
    /// Returns the runtime's common identity service.
    fn host(&self) -> Result<ComPtr<IWidgetHostAbi>, NcStatus> {
        Ok(self.state.host().add_ref())
    }
    /// Commits the sole attachment after the manager has validated ownership/capabilities.
    fn mount(&self) -> Result<(), NcStatus> {
        self.state.mount()
    }
    /// Idempotently closes ordinary events and all widget capabilities.
    fn close(&self) -> Result<(), NcStatus> {
        self.state.close()?;
        self.queue.closed.set(true);
        self.queue.pending.try_borrow_mut().map_err(|_| UI_E_BUSY)?.clear();
        Ok(())
    }

    /// Declares button appearance and the shared Enter/Space/Tab routing policy.
    fn properties(&self) -> Result<WidgetProperties, NcStatus> {
        Ok(WidgetProperties {
            options: self.state.options()?,
            keyboard: 2 | 4 | 8,
            frame: WidgetFrame::BUTTON,
        })
    }
    /// Uses the same validated option mutation rules as every other widget class.
    fn set_options(&self, options: WidgetOptions) -> Result<(), NcStatus> {
        self.state.set_options(options)
    }

    /// Enqueues one immutable submission after the runtime resolves pointer/key activation.
    fn update(&self, state: WidgetUpdate, event: WidgetEventAbi, text: NcCopyFromBufferOffset<'_, u8>) -> Result<GeometryChange, NcStatus> {
        self.state.writable()?;
        decode_widget_event(event, text)?;
        if state.action == WidgetAction::ACTIVATE && self.queue.connected.get() && !self.queue.closed.get() {
            let snapshot = self.label()?;
            self.queue.pending.try_borrow_mut().map_err(|_| UI_E_BUSY)?.push_back(snapshot);
        }
        // Submission changes neither preferred geometry nor child placement.
        Ok(GeometryChange::NO_CHANGE)
    }

    /// Drains the accumulated programmatic geometry marker exactly once.
    fn take_change(&self) -> Result<GeometryChange, NcStatus> {
        self.state.take_change()
    }
    /// Records text synchronously; the painter owns all recorded storage.
    fn paint(&self, scope: PaintScope, rect: Rect, painter: ComRef<'_, IPainterAbi>) -> Result<(), NcStatus> {
        self.state.readable()?;
        let label = self.label.try_borrow().map_err(|_| UI_E_BUSY)?.clone();
        painter.text(scope, rect, NcCopyFromBufferOffset::new(0, label.as_bytes()))
    }
}

impl ILeafLayout for Button {
    /// Uses shared theme/font metrics without retaining the measurement transaction.
    fn measure(&self, bounds: LayoutLimits, metrics: ComRef<'_, ITextMetricsAbi>) -> Result<Size, NcStatus> {
        self.state.readable()?;
        let label = self.label.try_borrow().map_err(|_| UI_E_BUSY)?.clone();
        bounds.width.validate()?;
        bounds.height.validate()?;
        metrics.button_size(NcCopyFromBufferOffset::new(0, label.as_bytes()))
    }
}

impl IButtonEvents for Button {
    /// Retains the typed sink only inside the returned application-owned lease.
    fn subscribe(&self, sink: ComRef<'_, IButtonSinkAbi>) -> Result<ComPtr<IEventSubscriptionAbi>, NcStatus> {
        self.state.writable()?;
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
