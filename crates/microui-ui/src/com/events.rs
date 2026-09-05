//! Typed COM delivery reuses native semantic queues; only the application-owned lease retains its sink.
use super::*;
use crate::event::{EventLease, WidgetEventPortHandle};
use nc_com::{ComInterface, INcStringAbi, NcString};

/// A concrete semantic snapshot defines its one concrete foreign delivery contract.
pub(crate) trait ComEvent: crate::WidgetEvent {
    /// Concrete sink interface shared by every snapshot in this native event family.
    type Sink: ComInterface;
    /// Delivers a committed snapshot without reading the live producer or retaining a queue borrow.
    fn deliver(&self, sink: ComRef<'_, Self::Sink>) -> Result<(), NcStatus>;
}
/// Labelled controls share transport while their source snapshot types remain concrete inside the plugin.
macro_rules! activation_event {
    ($event:ty) => {
        impl ComEvent for $event {
            type Sink = IActivationSinkAbi;
            /// Copies the label captured during activation, never the producer's current mutable value.
            fn deliver(&self, sink: ComRef<'_, IActivationSinkAbi>) -> Result<(), NcStatus> {
                let label: ComPtr<INcStringAbi> = NcString::from_utf8_str(&self.label);
                sink.submitted(label.as_com_ref())
            }
        }
    };
}
activation_event!(crate::ButtonSubmitted);
activation_event!(crate::ListBoxSubmitted);
activation_event!(crate::ListItemSubmitted);

/// Captured scalar values require no snapshot allocation or live-widget read at delivery time.
macro_rules! scalar_event {
    ($event:ty, $sink:ty, $field:ident) => {
        impl ComEvent for $event {
            type Sink = $sink;
            /// Delivers the actual committed user value through its concrete sink contract.
            fn deliver(&self, sink: ComRef<'_, $sink>) -> Result<(), NcStatus> {
                sink.changed(self.$field)
            }
        }
    };
}
scalar_event!(crate::CheckboxChanged, ICheckboxSinkAbi, checked);
scalar_event!(crate::SliderChanged, INumericSinkAbi, value);
scalar_event!(crate::NumberChanged, INumericSinkAbi, value);
scalar_event!(crate::ScrollbarChanged, IScrollbarSinkAbi, offset);

/// Both editor implementations already produce complete immutable semantic snapshots.
macro_rules! text_events {
    ($changed:ty, $submitted:ty) => {
        impl ComEvent for $changed {
            type Sink = ITextChangedSinkAbi;
            /// Delivers normalized text and its matching cursor from the same captured event.
            fn deliver(&self, sink: ComRef<'_, ITextChangedSinkAbi>) -> Result<(), NcStatus> {
                let text = NcString::from_utf8_str(&self.text);
                sink.changed(text.as_com_ref(), self.cursor as u64)
            }
        }
        impl ComEvent for $submitted {
            type Sink = ITextSubmittedSinkAbi;
            /// Converts one captured submission without consulting or retaining the source widget.
            fn deliver(&self, sink: ComRef<'_, ITextSubmittedSinkAbi>) -> Result<(), NcStatus> {
                let text = NcString::from_utf8_str(&self.text);
                sink.submitted(text.as_com_ref())
            }
        }
    };
}
text_events!(crate::TextboxChanged, crate::TextboxSubmitted);
text_events!(crate::TextAreaChanged, crate::TextAreaSubmitted);

#[com_class(interfaces(IEventSubscription))]
/// One generic local allocation specializes queue and sink types behind a concrete subscription interface.
struct Subscription<E: ComEvent> {
    /// Exclusive queue data and cancellation, shared with the existing native producer implementation.
    lease: EventLease<E>,
    /// Cancellation releases application ownership immediately, even if the lease itself is retained elsewhere.
    sink: RefCell<Option<ComPtr<E::Sink>>>,
    /// Direct reentrant dispatch must reject before detaching a second batch.
    dispatching: Cell<bool>,
}

/// Connects the native port before publishing a checked owned subscription interface.
pub(super) fn subscribe<E: ComEvent>(endpoint: WidgetEventPortHandle<E>, sink: ComRef<'_, E::Sink>) -> Result<ComPtr<IEventSubscriptionAbi>, NcStatus> {
    let lease = EventLease::connect(endpoint).map_err(|error| match error {
        crate::SubscribeError::WidgetExpired => UI_E_CLOSED,
        crate::SubscribeError::AlreadySubscribed => UI_E_BUSY,
    })?;
    Ok(Subscription::from_value(Subscription {
        lease,
        sink: RefCell::new(Some(sink.add_ref())),
        dispatching: Cell::new(false),
    })
    .query()
    .expect("subscription declares IEventSubscription"))
}

impl<E: ComEvent> IEventSubscription for Subscription<E> {
    /// Delivers a bounded detached prefix, preserving unattempted events after a callback error.
    fn dispatch(&self, limit: u32) -> Result<u32, NcStatus> {
        if self.dispatching.replace(true) {
            return Err(UI_E_BUSY);
        }
        let _guard = DispatchGuard(&self.dispatching);
        let batch = self.lease.drain(limit as usize)?;
        let mut remaining = batch.events.into_iter();
        // Take ownership instead of calling foreign AddRef while borrowing the slot.
        let sink = SinkAccess {
            subscription: self,
            value: self.sink.borrow_mut().take(),
        };
        let mut delivered = 0;
        if let Some(sink) = &sink.value {
            while let Some(event) = remaining.next() {
                if !self.lease.can_deliver(batch.terminal) {
                    break;
                }
                if let Err(error) = event.deliver(sink.as_com_ref()) {
                    self.lease.restore(remaining);
                    return Err(error);
                }
                delivered += 1;
            }
        }
        Ok(delivered)
    }
    /// Reports liveness without taking another source or sink reference.
    fn active(&self) -> Result<bool, NcStatus> {
        Ok(self.lease.active())
    }
    /// Cancels once and releases retained application state without invoking the handler.
    fn cancel(&self) -> Result<(), NcStatus> {
        self.lease.cancel();
        self.release_sink();
        Ok(())
    }
}
impl<E: ComEvent> Subscription<E> {
    /// Drops owned application state after releasing the short sink slot borrow, permitting safe destructor reentry.
    fn release_sink(&self) {
        let sink = self.sink.borrow_mut().take();
        drop(sink);
    }
}

/// Synchronous recursion guard owns no producer or callback borrow.
struct DispatchGuard<'a>(
    /// Restored after success, callback error or budget exhaustion.
    &'a Cell<bool>,
);
impl Drop for DispatchGuard<'_> {
    /// Failed delivery cannot leave the subscription permanently busy.
    fn drop(&mut self) {
        self.0.set(false);
    }
}

/// Owns the callback capability across delivery; no RefCell borrow or per-batch AddRef crosses user code.
struct SinkAccess<'a, E: ComEvent> {
    /// Existing lease and vacant sink slot remain alive for this synchronous call.
    subscription: &'a Subscription<E>,
    /// Sole sink reference is returned only if cancellation or terminal delivery has not ended it.
    value: Option<ComPtr<E::Sink>>,
}
impl<E: ComEvent> Drop for SinkAccess<'_, E> {
    /// Restores active callback ownership after success/error; inactive sinks drop after all slot borrows end.
    fn drop(&mut self) {
        if self.subscription.lease.active() {
            *self.subscription.sink.borrow_mut() = self.value.take();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nc_com::com_interface;

    #[com_interface(auto_guid = "microui.test.EventLeaseSink:v1")]
    /// Independent typed consumer used to exercise generic lease transport.
    trait ITestSink {
        /// Consumes one concrete test value and may return a recoverable application error.
        fn receive(&self, value: i32) -> Result<(), NcStatus>;
    }
    /// One immutable event with no producer or application reference.
    struct TestEvent(i32);
    impl crate::WidgetEvent for TestEvent {}
    impl ComEvent for TestEvent {
        type Sink = ITestSinkAbi;
        /// Forwards the actual captured value through the independent contract.
        fn deliver(&self, sink: ComRef<'_, ITestSinkAbi>) -> Result<(), NcStatus> {
            sink.receive(self.0)
        }
    }
    /// Application-owned observations deliberately retain a lease for callback cancellation/reentry.
    #[derive(Default)]
    struct Probe {
        /// Complete callback attempts, including the deliberately failing one.
        values: RefCell<Vec<i32>>,
        /// Retaining this lease would cycle without explicit cancellation releasing its sink.
        lease: RefCell<Option<ComPtr<IEventSubscriptionAbi>>>,
        /// Actual sink destruction count independent of retained lease references.
        drops: Cell<u32>,
        /// Value whose callback publishes another event before failing once.
        fail: Cell<Option<i32>>,
        /// Value whose callback cancels its own subscription during detached-batch delivery.
        cancel: Cell<Option<i32>>,
    }
    #[com_class(interfaces(ITestSink))]
    /// Concrete application callback owns its producer capability, an allowed direction of ownership.
    struct Sink {
        /// Producer does not own this sink, so only the deliberate application lease reference can cycle.
        source: Rc<crate::EventSource<TestEvent>>,
        /// Shared test observations with no borrowed stack state stored in COM.
        probe: Rc<Probe>,
    }
    impl ITestSink for Sink {
        /// Exercises direct recursion, failure and cancellation without a borrowed subscription sink slot.
        fn receive(&self, value: i32) -> Result<(), NcStatus> {
            let lease = self.probe.lease.borrow().clone().unwrap();
            assert_eq!(lease.dispatch(1), Err(UI_E_BUSY));
            self.probe.values.borrow_mut().push(value);
            if self.probe.fail.get() == Some(value) {
                self.probe.fail.set(None);
                self.source.emit(TestEvent(4));
                return Err(NC_E_INVALID_ARG);
            }
            if self.probe.cancel.get() == Some(value) {
                lease.cancel()?;
            }
            Ok(())
        }
    }
    impl Drop for Sink {
        /// Confirms cancellation/terminal delivery can release application state before final lease release.
        fn drop(&mut self) {
            self.probe.drops.set(self.probe.drops.get() + 1);
        }
    }
    /// Builds an application-owned sink and lease over the same native source used by widgets.
    fn connected() -> (Rc<crate::EventSource<TestEvent>>, Rc<Probe>, ComPtr<IEventSubscriptionAbi>) {
        let source = Rc::new(crate::EventSource::new());
        let probe = Rc::new(Probe::default());
        let sink: ComPtr<ITestSinkAbi> = Sink::from_value(Sink {
            source: source.clone(),
            probe: probe.clone(),
        })
        .query()
        .unwrap();
        let lease = subscribe(source.event(), sink.as_com_ref()).unwrap();
        *probe.lease.borrow_mut() = Some(lease.clone());
        (source, probe, lease)
    }
    /// Errors preserve unattempted FIFO snapshots; in-callback cancellation discards the remainder and releases the sink.
    #[test]
    fn callback_errors_and_cancellation_preserve_order_and_release_application_ownership() {
        let (source, probe, lease) = connected();
        for value in [1, 2, 3] {
            source.emit(TestEvent(value));
        }
        probe.fail.set(Some(2));
        assert_eq!(lease.dispatch(0), Err(UI_E_EVENT_LIMIT));
        assert_eq!(lease.dispatch(3), Err(NC_E_INVALID_ARG));
        assert_eq!(*probe.values.borrow(), [1, 2]);
        assert_eq!(lease.dispatch(1), Ok(1));
        assert_eq!(*probe.values.borrow(), [1, 2, 3]);
        source.emit(TestEvent(5));
        probe.cancel.set(Some(4));
        assert_eq!(lease.dispatch(10), Ok(1));
        assert_eq!(*probe.values.borrow(), [1, 2, 3, 4]);
        assert_eq!(probe.drops.get(), 1);
        assert_eq!(lease.active(), Ok(false));
        assert_eq!(lease.dispatch(10), Ok(0));
        // A still-retained cancelled lease no longer owns a sink and cannot
        // disconnect a newer subscriber to the same producer.
        let replacement = EventLease::connect(source.event()).unwrap();
        drop(lease);
        probe.lease.borrow_mut().take();
        source.emit(TestEvent(6));
        assert_eq!(replacement.drain(1).unwrap().events[0].0, 6);
    }
    /// Explicit terminal facts survive ordinary queue closure, deliver once, and release the retained application sink.
    #[test]
    fn terminal_delivery_survives_logical_source_closure_once() {
        let (source, probe, lease) = connected();
        source.emit(TestEvent(1));
        source.close(TestEvent(9));
        source.close(TestEvent(10));
        assert_eq!(lease.active(), Ok(true));
        assert_eq!(lease.dispatch(1), Ok(1));
        assert_eq!(*probe.values.borrow(), [9]);
        assert_eq!(lease.active(), Ok(false));
        assert_eq!(probe.drops.get(), 1);
        assert_eq!(lease.dispatch(1), Ok(0));
        probe.lease.borrow_mut().take();
    }
}

impl ComEvent for crate::WindowEvent {
    type Sink = IWindowSinkAbi;
    /// Encodes the already captured manager event, including its explicit terminal destruction fact.
    fn deliver(&self, sink: ComRef<'_, IWindowSinkAbi>) -> Result<(), NcStatus> {
        let event = match *self {
            crate::WindowEvent::GeometryChanged { rect } => WindowEvent::GeometryChanged { rect: copied_rect(rect) },
            crate::WindowEvent::CloseRequested => WindowEvent::CloseRequested,
            crate::WindowEvent::Hidden => WindowEvent::Hidden,
            crate::WindowEvent::Destroyed => WindowEvent::Destroyed,
            crate::WindowEvent::Minimized { rect } => WindowEvent::Minimized { rect: copied_rect(rect) },
            crate::WindowEvent::Maximized { rect } => WindowEvent::Maximized { rect: copied_rect(rect) },
            crate::WindowEvent::Restored { rect } => WindowEvent::Restored { rect: copied_rect(rect) },
        };
        sink.event(event.into())
    }
}
impl ComEvent for crate::PopupEvent {
    type Sink = IPopupSinkAbi;
    /// The current native popup event family contains exactly one ordinary dismissal fact.
    fn deliver(&self, sink: ComRef<'_, IPopupSinkAbi>) -> Result<(), NcStatus> {
        match self {
            crate::PopupEvent::Dismissed => sink.dismissed(),
        }
    }
}
