//! Concrete button storage and its exclusive, typed event subscription.
use super::*;
use nc_com::{INcStringAbi, NcString};
use std::collections::VecDeque;

#[com_class(interfaces(IWidget, ILeafLayout, IActivatable, IButton, IButtonEvents, IWidgetLifetime))]
/// One authoritative button allocation shared by editing, layout, paint and input.
pub(super) struct Button {
    /// Runtime authority has no path back to the owning tree.
    host: ComPtr<IWidgetHostAbi>,
    /// Current UTF-8 label; snapshots own independent storage.
    label: RefCell<String>,
    /// Programmatic geometry changes survive skipped update calls.
    dirty: Cell<bool>,
    /// A widget is mounted at most once during its allocation's lifetime.
    mounted: Cell<bool>,
    /// Logical closure is independent of COM allocation lifetime.
    closed: Cell<bool>,
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
            host,
            label: RefCell::new(String::new()),
            dirty: Cell::new(false),
            mounted: Cell::new(false),
            closed: Cell::new(false),
            queue: Rc::default(),
        })
        .query()
        .expect("Button declares IButton")
    }

    /// Reads remain legal during layout/paint, but never after runtime or widget closure.
    fn readable(&self) -> Result<(), NcStatus> {
        if self.closed.get() {
            return Err(UI_E_CLOSED);
        }
        self.host.check_open()
    }
    /// Checks logical lifetime before entering a mutating operation.
    fn writable(&self) -> Result<(), NcStatus> {
        if self.closed.get() {
            return Err(UI_E_CLOSED);
        }
        self.host.check_mutation()
    }
}

impl IButton for Button {
    /// Validates before committing and only dirties geometry when text actually differs.
    fn set_label(&self, input: NcCopyFromBufferOffset<'_, u8>) -> Result<(), NcStatus> {
        self.writable()?;
        let input = text(input)?;
        let mut label = self.label.try_borrow_mut().map_err(|_| UI_E_BUSY)?;
        if *label != input {
            label.clear();
            label.push_str(input);
            self.dirty.set(true);
        }
        Ok(())
    }
    /// Returns an owned immutable snapshot, never a borrow into widget storage.
    fn label(&self) -> Result<ComPtr<INcStringAbi>, NcStatus> {
        self.readable()?;
        Ok(NcString::from_utf8_str(&self.label.try_borrow().map_err(|_| UI_E_BUSY)?))
    }
}

impl IWidget for Button {
    /// Drains the accumulated programmatic geometry marker exactly once.
    fn take_change(&self) -> Result<GeometryChange, NcStatus> {
        self.readable()?;
        Ok(if self.dirty.replace(false) {
            GeometryChange::CHANGED
        } else {
            GeometryChange::NO_CHANGE
        })
    }
    /// Records text synchronously; the painter owns all recorded storage.
    fn paint(&self, scope: PaintScope, rect: Rect, painter: ComRef<'_, IPainterAbi>) -> Result<(), NcStatus> {
        self.readable()?;
        let label = self.label.try_borrow().map_err(|_| UI_E_BUSY)?.clone();
        painter.text(scope, rect, NcCopyFromBufferOffset::new(0, label.as_bytes()))
    }
}

impl ILeafLayout for Button {
    /// Uses shared theme/font metrics without retaining the measurement transaction.
    fn measure(&self, bounds: LayoutLimits, metrics: ComRef<'_, ITextMetricsAbi>) -> Result<Size, NcStatus> {
        self.readable()?;
        let label = self.label.try_borrow().map_err(|_| UI_E_BUSY)?.clone();
        bounds.width.validate()?;
        bounds.height.validate()?;
        metrics.button_size(NcCopyFromBufferOffset::new(0, label.as_bytes()))
    }
}

impl IActivatable for Button {
    /// Enqueues one immutable label snapshot without invoking application code.
    fn activate(&self) -> Result<GeometryChange, NcStatus> {
        self.writable()?;
        if self.queue.connected.get() && !self.queue.closed.get() {
            let snapshot = self.label()?;
            self.queue.pending.try_borrow_mut().map_err(|_| UI_E_BUSY)?.push_back(snapshot);
        }
        Ok(GeometryChange::NO_CHANGE)
    }
}

impl IWidgetLifetime for Button {
    /// Returns the runtime's common identity service.
    fn host(&self) -> Result<ComPtr<IWidgetHostAbi>, NcStatus> {
        Ok(self.host.clone())
    }
    /// Commits the sole attachment after the manager has validated ownership/capabilities.
    fn mount(&self) -> Result<(), NcStatus> {
        self.writable()?;
        if self.mounted.replace(true) { Err(UI_E_OWNER) } else { Ok(()) }
    }
    /// Idempotently closes ordinary events and all widget capabilities.
    fn close(&self) -> Result<(), NcStatus> {
        self.closed.set(true);
        self.queue.closed.set(true);
        self.queue.pending.try_borrow_mut().map_err(|_| UI_E_BUSY)?.clear();
        Ok(())
    }
}

impl IButtonEvents for Button {
    /// Retains the typed sink only inside the returned application-owned lease.
    fn subscribe(&self, sink: ComRef<'_, IButtonSinkAbi>) -> Result<ComPtr<IEventSubscriptionAbi>, NcStatus> {
        self.writable()?;
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
        let _ = self.close();
    }
}
