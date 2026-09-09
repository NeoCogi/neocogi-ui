//! Application-owned typed handlers and registration-order delivery over concrete COM event contracts.
//!
//! A session owns its state and leases; producer queues own only immutable event
//! snapshots. The runtime borrows the dispatcher while returning its context
//! between input phases, so handlers can use ordinary widget/window methods.
//!
//! ```no_run
//! use microui_redux::{ButtonWidget, EventHandler, EventSession, Size, Ui};
//! use neocogi_com::NcStatus;
//! use microui_redux::events::Activation;
//!
//! struct Count;
//! impl EventHandler<u32, Activation> for Count {
//!     fn handle(&mut self, count: &mut u32, _event: &Activation) -> Result<(), NcStatus> {
//!         *count += 1;
//!         Ok(())
//!     }
//! }
//! fn update(ui: &Ui, button: &ButtonWidget, size: Size) -> Result<u32, NcStatus> {
//!     let events = EventSession::new(0u32);
//!     events.subscribe_activation(button, Count)?;
//!     ui.update_with_events(size, &events)?;
//!     let count = *events.state()?;
//!     Ok(count) // Dropping events cancels its leases, even when separately retained.
//! }
//! ```
//!
//! Delivery is FIFO within each source and registration ordered between sources.
//! Finish state borrows before dispatch. Handler errors consume the attempted
//! event, preserve unattempted events, and do not roll back prior callback edits.
//! Geometry is synchronized after handlers; behavior requiring an ordinary widget
//! update (for example a newly requested caret reveal) waits for the next eligible
//! update. Recursive update/render and session dispatch/registration report BUSY.
//! See `docs/COM_AUTHORS.md` for lifecycle, bounded cascades and author obligations.
/// Static producer queues shared by built-ins, independent plugins and application components.
mod source;
pub use source::{EventEndpoint, EventPayload, EventSource};

/// Internal queue access for isolated semantic-algorithm tests, absent from normal builds.
/// Applications and plugin authors subscribe through EventSource/EventEndpoint instead.
#[cfg(feature = "test-support")]
pub mod test_support {
    pub use super::source::{EventBatch, EventLease, SubscribeError};
}

use crate::com::*;
use neocogi_com::{com_object, ComPtr, ComRef, INcStringAbi, INcStringComPtrExt, NcStatus, NC_E_INVALID_ARG};
use std::{
    cell::{Cell, RefCell, Ref, RefMut},
    rc::Rc,
    ops::{Deref, DerefMut},
};

/// Application-local behavior stored inline in the concrete COM sink specialization.
pub trait EventHandler<State, Event> {
    /// Handles one owned semantic snapshot while borrowing only local application state.
    fn handle(&mut self, state: &mut State, event: &Event) -> Result<(), NcStatus>;
}
impl<State, Event, Callback> EventHandler<State, Event> for Callback
where
    Callback: FnMut(&mut State, &Event) -> Result<(), NcStatus>,
{
    /// Keeps the approved callback convenience local; neither its captures nor state borrow cross the ABI.
    fn handle(&mut self, state: &mut State, event: &Event) -> Result<(), NcStatus> {
        self(state, event)
    }
}

/// Default finite callback budget for one deterministic dispatch boundary.
pub use microui_contracts::EVENT_DISPATCH_LIMIT;

/// Static session owns application state and subscriptions; producers never retain this session.
pub struct EventSession<State: 'static> {
    /// The same owned state is borrowed by each concrete handler, never passed as a raw COM pointer.
    state: Rc<RefCell<State>>,
    /// One authoritative registration sequence shared with the retainable dispatcher object.
    registry: Rc<Registry>,
    /// Runtime borrows this projection synchronously and must not retain it.
    dispatcher: ComPtr<IEventDispatcherAbi>,
}
impl<State: 'static> EventSession<State> {
    /// Creates an empty session with owned application state and no subscriptions.
    pub fn new(state: State) -> Self {
        let registry = Rc::new(Registry {
            leases: RefCell::new(Some(Vec::new())),
            closed: Cell::new(false),
        });
        let dispatcher = Dispatcher::from_value(Dispatcher { registry: registry.clone() })
            .query()
            .expect("dispatcher declares IEventDispatcher");
        Self {
            state: Rc::new(RefCell::new(state)),
            registry,
            dispatcher,
        }
    }
    /// Reads local state; an incompatible active handler borrow reports BUSY.
    pub fn state(&self) -> Result<Ref<'_, State>, NcStatus> {
        self.state.try_borrow().map_err(|_| UI_E_BUSY)
    }
    /// Mutates local state between callbacks; finish this borrow before dispatching events.
    pub fn state_mut(&self) -> Result<RefMut<'_, State>, NcStatus> {
        self.state.try_borrow_mut().map_err(|_| UI_E_BUSY)
    }
    /// Borrows the concrete dispatcher without transferring application/session ownership to a runtime.
    pub fn as_dispatcher(&self) -> ComRef<'_, IEventDispatcherAbi> {
        self.dispatcher.as_com_ref()
    }
    /// Delivers queued events and finite cascades without processing UI input or committing layout.
    pub fn dispatch(&self) -> Result<u32, NcStatus> {
        self.dispatcher.dispatch(EVENT_DISPATCH_LIMIT)
    }
    /// Selects a finite explicit callback budget; unattempted events survive budget exhaustion.
    pub fn dispatch_bounded(&self, limit: u32) -> Result<u32, NcStatus> {
        self.dispatcher.dispatch(limit)
    }
    /// Cancels all subscriptions and releases their sinks; active registration or dispatch reports BUSY.
    pub fn cancel(&self) -> Result<(), NcStatus> {
        let mut leases = self.registry.take()?;
        for lease in leases.iter() {
            lease.cancel()?;
        }
        leases.clear();
        Ok(())
    }
}

impl<State: 'static> Drop for EventSession<State> {
    /// Cancels retained leases even when application code keeps a lease or dispatcher pointer after session destruction.
    fn drop(&mut self) {
        let idle = self.registry.take();
        self.registry.closed.set(true);
        // If delivery owns the sequence, its transaction observes closed and
        // performs the same cancellation before returning the vector.
        drop(idle);
    }
}

/// Registration storage is taken by value before any foreign subscription or callback call.
struct Registry {
    /// Session destruction requests cancellation even if a retained dispatcher is currently delivering a batch.
    closed: Cell<bool>,
    /// None represents an active transaction, so reentry rejects instead of borrowing a live vector.
    leases: RefCell<Option<Vec<ComPtr<IEventSubscriptionAbi>>>>,
}
impl Registry {
    /// Takes the sole sequence; both registration and delivery use this same exclusion boundary.
    fn take(&self) -> Result<Registrations<'_>, NcStatus> {
        if self.closed.get() {
            return Err(UI_E_CLOSED);
        }
        let leases = self.leases.try_borrow_mut().map_err(|_| UI_E_BUSY)?.take().ok_or(UI_E_BUSY)?;
        Ok(Registrations { registry: self, leases: Some(leases) })
    }
}
/// Owned registration transaction restores its exact order on success and error.
struct Registrations<'a> {
    /// Vacant destination remains alive for this local synchronous operation.
    registry: &'a Registry,
    /// Sole sequence ownership; no RefCell guard crosses plugin calls or COM Release.
    leases: Option<Vec<ComPtr<IEventSubscriptionAbi>>>,
}
impl Deref for Registrations<'_> {
    type Target = Vec<ComPtr<IEventSubscriptionAbi>>;
    /// Borrows only the locally owned vector.
    fn deref(&self) -> &Self::Target {
        self.leases.as_ref().expect("active registration transaction owns its leases")
    }
}
impl DerefMut for Registrations<'_> {
    /// Mutates only the locally owned vector.
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.leases.as_mut().expect("active registration transaction owns its leases")
    }
}
impl Registrations<'_> {
    /// Releases inactive sinks explicitly before pruning their retained leases, without changing surviving order.
    fn prune(&mut self) -> Result<(), NcStatus> {
        let mut index = 0;
        while index < self.len() {
            if self[index].active()? {
                index += 1;
            } else {
                self[index].cancel()?;
                let retired = self.remove(index);
                drop(retired);
            }
        }
        Ok(())
    }
}
impl Drop for Registrations<'_> {
    /// Restores the sequence after all foreign calls; the destination never contains another live vector.
    fn drop(&mut self) {
        if self.registry.closed.get() {
            let leases = self.leases.as_mut().expect("active transaction owns its leases until drop");
            for lease in leases.iter() {
                let _ = lease.cancel();
            }
            leases.clear();
        }
        *self.registry.leases.borrow_mut() = self.leases.take();
    }
}

#[com_object(interfaces(IEventDispatcher))]
/// Concrete borrowed runtime capability knows only leases, not application state types or UI implementation storage.
struct Dispatcher {
    /// The static session and dispatcher share one registration sequence.
    registry: Rc<Registry>,
}
impl IEventDispatcher for Dispatcher {
    /// Sweeps in registration order and caps total successful callbacks across all FIFO sources and cascades.
    fn dispatch(&self, limit: u32) -> Result<u32, NcStatus> {
        let mut leases = self.registry.take()?;
        leases.prune()?;
        let mut delivered = 0;
        loop {
            let before = delivered;
            for lease in leases.iter() {
                if self.registry.closed.get() {
                    return Err(UI_E_CLOSED);
                }
                let remaining = limit - delivered;
                let count = lease.dispatch(remaining)?;
                if count > remaining {
                    return Err(NC_E_INVALID_ARG);
                }
                delivered += count;
            }
            if before == delivered {
                break;
            }
        }
        leases.prune()?;
        Ok(delivered)
    }
}

/// Concrete application data and behavior shared by the small per-family COM sink declarations.
struct HandlerState<State, Handler> {
    /// Explicit owned state, shared only inside the application binary.
    state: Rc<RefCell<State>>,
    /// Taking the handler prevents recursive invocation from aliasing its concrete mutable data.
    handler: RefCell<Option<Handler>>,
}
impl<State, Handler> HandlerState<State, Handler> {
    /// Owns the concrete handler during the local call and restores it before reporting any recoverable failure.
    fn deliver<Event>(&self, event: Event) -> Result<(), NcStatus>
    where
        Handler: EventHandler<State, Event>,
    {
        let mut handler = self.handler.try_borrow_mut().map_err(|_| UI_E_BUSY)?.take().ok_or(UI_E_BUSY)?;
        let result = match self.state.try_borrow_mut() {
            Ok(mut state) => handler.handle(&mut state, &event),
            Err(_) => Err(UI_E_BUSY),
        };
        *self.handler.borrow_mut() = Some(handler);
        result
    }
}

/// Captured label of a Button, ListItem or ListBox activation.
#[derive(Clone, Debug, PartialEq)]
pub struct Activation {
    /// Copied semantic value owned by this event snapshot.
    pub label: String,
}

/// Static adapter for concrete widget types that publish labelled activation.
pub trait ActivationSource {
    /// Connects one sink through the concrete class or shared interface that owns the source.
    fn subscribe_activation_sink(
        &self,
        sink: ComRef<'_, IActivationSinkAbi>,
    ) -> Result<ComPtr<IEventSubscriptionAbi>, NcStatus>;
}

impl ActivationSource for ButtonWidget {
    /// Buttons expose subscription directly on their callable class contract.
    fn subscribe_activation_sink(
        &self,
        sink: ComRef<'_, IActivationSinkAbi>,
    ) -> Result<ComPtr<IEventSubscriptionAbi>, NcStatus> {
        self.as_interface().subscribe(sink)
    }
}

impl ActivationSource for ListItemWidget {
    /// List rows expose subscription directly on their callable class contract.
    fn subscribe_activation_sink(
        &self,
        sink: ComRef<'_, IActivationSinkAbi>,
    ) -> Result<ComPtr<IEventSubscriptionAbi>, NcStatus> {
        self.as_interface().subscribe(sink)
    }
}

impl ActivationSource for ListBoxWidget {
    /// List boxes expose subscription directly on their callable class contract.
    fn subscribe_activation_sink(
        &self,
        sink: ComRef<'_, IActivationSinkAbi>,
    ) -> Result<ComPtr<IEventSubscriptionAbi>, NcStatus> {
        self.as_interface().subscribe(sink)
    }
}

#[com_object(interfaces(IActivationSink))]
/// Concrete local sink specialization stores application state and its exact handler type directly.
struct ActivationSink<State: 'static, Handler: EventHandler<State, Activation> + 'static> {
    /// One shared local invocation helper; no erased handler or Rust value crosses COM.
    handler: HandlerState<State, Handler>,
}
impl<State: 'static, Handler: EventHandler<State, Activation> + 'static> IActivationSink for ActivationSink<State, Handler> {
    /// Validates and copies the concrete foreign snapshot before invoking application behavior.
    fn submitted(&self, label: ComRef<'_, INcStringAbi>) -> Result<(), NcStatus> {
        self.handler.deliver(Activation { label: label.to_string()? })
    }
}
impl<State: 'static> EventSession<State> {
    /// Connects the widget's concrete event capability and registers its lease in deterministic order.
    /// Registration during an active session transaction reports BUSY before claiming the producer's port.
    pub fn subscribe_activation<Source: ActivationSource, Handler: EventHandler<State, Activation> + 'static>(
        &self,
        widget: &Source,
        handler: Handler,
    ) -> Result<ComPtr<IEventSubscriptionAbi>, NcStatus> {
        let mut leases = self.registry.take()?;
        let sink: ComPtr<IActivationSinkAbi> = ActivationSink::from_value(ActivationSink {
            handler: HandlerState {
                state: self.state.clone(),
                handler: RefCell::new(Some(handler)),
            },
        })
        .query()?;
        let lease = widget.subscribe_activation_sink(sink.as_com_ref())?;
        leases.push(lease.clone());
        Ok(lease)
    }
}

/// Captured boolean state after user activation.
#[derive(Clone, Debug, PartialEq)]
pub struct CheckboxChanged {
    /// Copied semantic value owned by this event snapshot.
    pub checked: bool,
}
#[com_object(interfaces(ICheckboxSink))]
/// Concrete local sink specialization stores application state and its exact handler type directly.
struct CheckboxSink<State: 'static, Handler: EventHandler<State, CheckboxChanged> + 'static> {
    /// One shared local invocation helper; no erased handler or Rust value crosses COM.
    handler: HandlerState<State, Handler>,
}
impl<State: 'static, Handler: EventHandler<State, CheckboxChanged> + 'static> ICheckboxSink for CheckboxSink<State, Handler> {
    /// Validates and copies the concrete foreign snapshot before invoking application behavior.
    fn changed(&self, checked: bool) -> Result<(), NcStatus> {
        self.handler.deliver(CheckboxChanged { checked })
    }
}
impl<State: 'static> EventSession<State> {
    /// Connects the widget's concrete event capability and registers its lease in deterministic order.
    /// Registration during an active session transaction reports BUSY before claiming the producer's port.
    pub fn subscribe_checkbox<Handler: EventHandler<State, CheckboxChanged> + 'static>(
        &self,
        widget: &crate::CheckboxWidget,
        handler: Handler,
    ) -> Result<ComPtr<IEventSubscriptionAbi>, NcStatus> {
        let mut leases = self.registry.take()?;
        let sink: ComPtr<ICheckboxSinkAbi> = CheckboxSink::from_value(CheckboxSink {
            handler: HandlerState {
                state: self.state.clone(),
                handler: RefCell::new(Some(handler)),
            },
        })
        .query()?;
        let lease = widget.as_interface().subscribe(sink.as_com_ref())?;
        leases.push(lease.clone());
        Ok(lease)
    }
}

/// Captured finite user value from a Slider or Number.
#[derive(Clone, Debug, PartialEq)]
pub struct NumericChanged {
    /// Copied semantic value owned by this event snapshot.
    pub value: f32,
}

/// Static facade hook shared by the concrete Slider and Number class wrappers.
pub trait NumericEventSource {
    /// Connects a numeric sink through the concrete class contract.
    fn subscribe_numeric_sink(&self, sink: ComRef<'_, INumericSinkAbi>) -> Result<ComPtr<IEventSubscriptionAbi>, NcStatus>;
}

impl NumericEventSource for crate::SliderWidget {
    /// Delegates directly to the Slider class projection without an event-interface query.
    fn subscribe_numeric_sink(&self, sink: ComRef<'_, INumericSinkAbi>) -> Result<ComPtr<IEventSubscriptionAbi>, NcStatus> {
        self.as_interface().subscribe(sink)
    }
}

impl NumericEventSource for crate::NumberWidget {
    /// Delegates directly to the Number class projection without an event-interface query.
    fn subscribe_numeric_sink(&self, sink: ComRef<'_, INumericSinkAbi>) -> Result<ComPtr<IEventSubscriptionAbi>, NcStatus> {
        self.as_interface().subscribe(sink)
    }
}

#[com_object(interfaces(INumericSink))]
/// Concrete local sink specialization stores application state and its exact handler type directly.
struct NumericSink<State: 'static, Handler: EventHandler<State, NumericChanged> + 'static> {
    /// One shared local invocation helper; no erased handler or Rust value crosses COM.
    handler: HandlerState<State, Handler>,
}
impl<State: 'static, Handler: EventHandler<State, NumericChanged> + 'static> INumericSink for NumericSink<State, Handler> {
    /// Validates and copies the concrete foreign snapshot before invoking application behavior.
    fn changed(&self, value: f32) -> Result<(), NcStatus> {
        if !value.is_finite() {
            return Err(NC_E_INVALID_ARG);
        }
        self.handler.deliver(NumericChanged { value })
    }
}
impl<State: 'static> EventSession<State> {
    /// Connects the widget's concrete event capability and registers its lease in deterministic order.
    /// Registration during an active session transaction reports BUSY before claiming the producer's port.
    pub fn subscribe_numeric<Source: NumericEventSource, Handler: EventHandler<State, NumericChanged> + 'static>(
        &self,
        widget: &Source,
        handler: Handler,
    ) -> Result<ComPtr<IEventSubscriptionAbi>, NcStatus> {
        let mut leases = self.registry.take()?;
        let sink: ComPtr<INumericSinkAbi> = NumericSink::from_value(NumericSink {
            handler: HandlerState {
                state: self.state.clone(),
                handler: RefCell::new(Some(handler)),
            },
        })
        .query()?;
        let lease = widget.subscribe_numeric_sink(sink.as_com_ref())?;
        leases.push(lease.clone());
        Ok(lease)
    }
}

/// Captured clamped user scrollbar offset.
#[derive(Clone, Debug, PartialEq)]
pub struct ScrollbarChanged {
    /// Copied semantic value owned by this event snapshot.
    pub offset: i32,
}
#[com_object(interfaces(IScrollbarSink))]
/// Concrete local sink specialization stores application state and its exact handler type directly.
struct ScrollbarSink<State: 'static, Handler: EventHandler<State, ScrollbarChanged> + 'static> {
    /// One shared local invocation helper; no erased handler or Rust value crosses COM.
    handler: HandlerState<State, Handler>,
}
impl<State: 'static, Handler: EventHandler<State, ScrollbarChanged> + 'static> IScrollbarSink for ScrollbarSink<State, Handler> {
    /// Validates and copies the concrete foreign snapshot before invoking application behavior.
    fn changed(&self, offset: i32) -> Result<(), NcStatus> {
        if offset < 0 {
            return Err(NC_E_INVALID_ARG);
        }
        self.handler.deliver(ScrollbarChanged { offset })
    }
}
impl<State: 'static> EventSession<State> {
    /// Connects the widget's concrete event capability and registers its lease in deterministic order.
    /// Registration during an active session transaction reports BUSY before claiming the producer's port.
    pub fn subscribe_scrollbar<Handler: EventHandler<State, ScrollbarChanged> + 'static>(
        &self,
        widget: &crate::ScrollbarWidget,
        handler: Handler,
    ) -> Result<ComPtr<IEventSubscriptionAbi>, NcStatus> {
        let mut leases = self.registry.take()?;
        let sink: ComPtr<IScrollbarSinkAbi> = ScrollbarSink::from_value(ScrollbarSink {
            handler: HandlerState {
                state: self.state.clone(),
                handler: RefCell::new(Some(handler)),
            },
        })
        .query()?;
        let lease = widget.as_interface().subscribe(sink.as_com_ref())?;
        leases.push(lease.clone());
        Ok(lease)
    }
}

/// Captured normalized editor text and matching UTF-8 byte cursor.
#[derive(Clone, Debug, PartialEq)]
pub struct TextChanged {
    /// Copied semantic value owned by this event snapshot.
    pub text: String,
    /// Copied semantic value owned by this event snapshot.
    pub cursor: u64,
}

/// Static facade hook shared by the concrete Textbox and TextArea class wrappers.
pub trait TextEventSource {
    /// Connects a changed-text sink through the concrete class contract.
    fn subscribe_changed_sink(&self, sink: ComRef<'_, ITextChangedSinkAbi>) -> Result<ComPtr<IEventSubscriptionAbi>, NcStatus>;
    /// Connects a submitted-text sink through the concrete class contract.
    fn subscribe_submitted_sink(&self, sink: ComRef<'_, ITextSubmittedSinkAbi>) -> Result<ComPtr<IEventSubscriptionAbi>, NcStatus>;
}

impl TextEventSource for crate::TextboxWidget {
    /// Delegates directly to the Textbox class projection without an event-interface query.
    fn subscribe_changed_sink(&self, sink: ComRef<'_, ITextChangedSinkAbi>) -> Result<ComPtr<IEventSubscriptionAbi>, NcStatus> {
        self.as_interface().subscribe_changed(sink)
    }
    /// Delegates directly to the Textbox class projection without an event-interface query.
    fn subscribe_submitted_sink(&self, sink: ComRef<'_, ITextSubmittedSinkAbi>) -> Result<ComPtr<IEventSubscriptionAbi>, NcStatus> {
        self.as_interface().subscribe_submitted(sink)
    }
}

impl TextEventSource for crate::TextAreaWidget {
    /// Delegates directly to the TextArea class projection without an event-interface query.
    fn subscribe_changed_sink(&self, sink: ComRef<'_, ITextChangedSinkAbi>) -> Result<ComPtr<IEventSubscriptionAbi>, NcStatus> {
        self.as_interface().subscribe_changed(sink)
    }
    /// Delegates directly to the TextArea class projection without an event-interface query.
    fn subscribe_submitted_sink(&self, sink: ComRef<'_, ITextSubmittedSinkAbi>) -> Result<ComPtr<IEventSubscriptionAbi>, NcStatus> {
        self.as_interface().subscribe_submitted(sink)
    }
}
#[com_object(interfaces(ITextChangedSink))]
/// Concrete local sink specialization stores application state and its exact handler type directly.
struct TextChangedSink<State: 'static, Handler: EventHandler<State, TextChanged> + 'static> {
    /// One shared local invocation helper; no erased handler or Rust value crosses COM.
    handler: HandlerState<State, Handler>,
}
impl<State: 'static, Handler: EventHandler<State, TextChanged> + 'static> ITextChangedSink for TextChangedSink<State, Handler> {
    /// Validates and copies the concrete foreign snapshot before invoking application behavior.
    fn changed(&self, text: ComRef<'_, INcStringAbi>, cursor: u64) -> Result<(), NcStatus> {
        let text = text.to_string()?;
        let position = usize::try_from(cursor).map_err(|_| NC_E_INVALID_ARG)?;
        if !text.is_char_boundary(position) {
            return Err(NC_E_INVALID_ARG);
        }
        self.handler.deliver(TextChanged { text, cursor })
    }
}
impl<State: 'static> EventSession<State> {
    /// Connects the widget's concrete event capability and registers its lease in deterministic order.
    /// Registration during an active session transaction reports BUSY before claiming the producer's port.
    pub fn subscribe_text_changed<Source: TextEventSource, Handler: EventHandler<State, TextChanged> + 'static>(
        &self,
        widget: &Source,
        handler: Handler,
    ) -> Result<ComPtr<IEventSubscriptionAbi>, NcStatus> {
        let mut leases = self.registry.take()?;
        let sink: ComPtr<ITextChangedSinkAbi> = TextChangedSink::from_value(TextChangedSink {
            handler: HandlerState {
                state: self.state.clone(),
                handler: RefCell::new(Some(handler)),
            },
        })
        .query()?;
        let lease = widget.subscribe_changed_sink(sink.as_com_ref())?;
        leases.push(lease.clone());
        Ok(lease)
    }
}

/// Captured normalized editor text at submission.
#[derive(Clone, Debug, PartialEq)]
pub struct TextSubmitted {
    /// Copied semantic value owned by this event snapshot.
    pub text: String,
}
#[com_object(interfaces(ITextSubmittedSink))]
/// Concrete local sink specialization stores application state and its exact handler type directly.
struct TextSubmittedSink<State: 'static, Handler: EventHandler<State, TextSubmitted> + 'static> {
    /// One shared local invocation helper; no erased handler or Rust value crosses COM.
    handler: HandlerState<State, Handler>,
}
impl<State: 'static, Handler: EventHandler<State, TextSubmitted> + 'static> ITextSubmittedSink for TextSubmittedSink<State, Handler> {
    /// Validates and copies the concrete foreign snapshot before invoking application behavior.
    fn submitted(&self, text: ComRef<'_, INcStringAbi>) -> Result<(), NcStatus> {
        self.handler.deliver(TextSubmitted { text: text.to_string()? })
    }
}
impl<State: 'static> EventSession<State> {
    /// Connects the widget's concrete event capability and registers its lease in deterministic order.
    /// Registration during an active session transaction reports BUSY before claiming the producer's port.
    pub fn subscribe_text_submitted<Source: TextEventSource, Handler: EventHandler<State, TextSubmitted> + 'static>(
        &self,
        widget: &Source,
        handler: Handler,
    ) -> Result<ComPtr<IEventSubscriptionAbi>, NcStatus> {
        let mut leases = self.registry.take()?;
        let sink: ComPtr<ITextSubmittedSinkAbi> = TextSubmittedSink::from_value(TextSubmittedSink {
            handler: HandlerState {
                state: self.state.clone(),
                handler: RefCell::new(Some(handler)),
            },
        })
        .query()?;
        let lease = widget.subscribe_submitted_sink(sink.as_com_ref())?;
        leases.push(lease.clone());
        Ok(lease)
    }
}

#[com_object(interfaces(IWindowSink))]
/// Concrete application window handler uses the same generic ownership as ordinary widget handlers.
struct WindowSink<State: 'static, Handler: EventHandler<State, WindowEvent> + 'static> {
    /// Stored application state and handler remain local to this binary.
    handler: HandlerState<State, Handler>,
}
impl<State: 'static, Handler: EventHandler<State, WindowEvent> + 'static> IWindowSink for WindowSink<State, Handler> {
    /// Validates the complete tagged event and copied geometry before invoking application code.
    fn event(&self, event: WindowEventAbi) -> Result<(), NcStatus> {
        let event = WindowEvent::try_from(event).map_err(|_| NC_E_INVALID_ARG)?;
        match event {
            WindowEvent::GeometryChanged { rect } | WindowEvent::Minimized { rect } | WindowEvent::Maximized { rect } | WindowEvent::Restored { rect }
                if rect.width < 0 || rect.height < 0 =>
            {
                return Err(NC_E_INVALID_ARG);
            }
            _ => {}
        }
        self.handler.deliver(event)
    }
}
/// Captured removal from the active popup branch; it does not imply permanent popup destruction.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PopupDismissed;
#[com_object(interfaces(IPopupSink))]
/// Concrete application popup handler stores its actual callback type inline.
struct PopupSink<State: 'static, Handler: EventHandler<State, PopupDismissed> + 'static> {
    /// Explicit application ownership has no reference from the popup's producer queue.
    handler: HandlerState<State, Handler>,
}
impl<State: 'static, Handler: EventHandler<State, PopupDismissed> + 'static> IPopupSink for PopupSink<State, Handler> {
    /// Delivers the one concrete ordinary popup event without querying current visibility.
    fn dismissed(&self) -> Result<(), NcStatus> {
        self.handler.deliver(PopupDismissed)
    }
}
impl<State: 'static> EventSession<State> {
    /// Registers the actual window's lifecycle source, including terminal delivery after its closure.
    pub fn subscribe_window<Handler: EventHandler<State, WindowEvent> + 'static>(
        &self,
        window: ComRef<'_, IWindowAbi>,
        handler: Handler,
    ) -> Result<ComPtr<IEventSubscriptionAbi>, NcStatus> {
        let mut leases = self.registry.take()?;
        let sink: ComPtr<IWindowSinkAbi> = WindowSink::from_value(WindowSink {
            handler: HandlerState {
                state: self.state.clone(),
                handler: RefCell::new(Some(handler)),
            },
        })
        .query()?;
        let lease = window.add_ref().query::<IWindowEventsAbi>()?.subscribe(sink.as_com_ref())?;
        leases.push(lease.clone());
        Ok(lease)
    }
    /// Registers the actual popup's dismissal source; later visibility is read independently from this snapshot.
    pub fn subscribe_popup<Handler: EventHandler<State, PopupDismissed> + 'static>(
        &self,
        popup: ComRef<'_, IPopupAbi>,
        handler: Handler,
    ) -> Result<ComPtr<IEventSubscriptionAbi>, NcStatus> {
        let mut leases = self.registry.take()?;
        let sink: ComPtr<IPopupSinkAbi> = PopupSink::from_value(PopupSink {
            handler: HandlerState {
                state: self.state.clone(),
                handler: RefCell::new(Some(handler)),
            },
        })
        .query()?;
        let lease = popup.add_ref().query::<IPopupEventsAbi>()?.subscribe(sink.as_com_ref())?;
        leases.push(lease.clone());
        Ok(lease)
    }
}

/// Matching application-list index and label after an explicitly submitted combo selection.
#[derive(Clone, Debug, PartialEq)]
pub struct ComboChanged {
    /// Application-owned list position, independent of the displayed text.
    pub selected: u64,
    /// Captured display value; generic list data never travels through this string.
    pub label: String,
}
#[com_object(interfaces(IComboChangedSink))]
/// Application-local specialization owns its concrete selection handler and shared state.
struct ComboChangedSink<State: 'static, Handler: EventHandler<State, ComboChanged> + 'static> {
    /// Shared owned invocation discipline used by every event family.
    handler: HandlerState<State, Handler>,
}
impl<State: 'static, Handler: EventHandler<State, ComboChanged> + 'static> IComboChangedSink for ComboChangedSink<State, Handler> {
    /// Validates the immutable UTF-8 snapshot before borrowing application state.
    fn changed(&self, selected: u64, label: ComRef<'_, INcStringAbi>) -> Result<(), NcStatus> {
        self.handler.deliver(ComboChanged { selected, label: label.to_string()? })
    }
}
/// Desired popup state and routed screen anchor captured by the combo header.
#[derive(Clone, Debug, PartialEq)]
pub struct ComboSubmitted {
    /// True requests opening; false requests ordinary popup hiding.
    pub open: bool,
    /// Complete screen-space anchor, independent of subsequent header movement.
    pub anchor: Rect,
}
#[com_object(interfaces(IComboPopupSink))]
/// Concrete application coordinator can show/hide its selected popup at the safe runtime boundary.
struct ComboPopupSink<State: 'static, Handler: EventHandler<State, ComboSubmitted> + 'static> {
    /// One owned handler without a borrowed native state pointer.
    handler: HandlerState<State, Handler>,
}
impl<State: 'static, Handler: EventHandler<State, ComboSubmitted> + 'static> IComboPopupSink for ComboPopupSink<State, Handler> {
    /// Rejects malformed geometry before application popup coordination runs.
    fn submitted(&self, open: bool, anchor: Rect) -> Result<(), NcStatus> {
        if anchor.width < 0 || anchor.height < 0 {
            return Err(NC_E_INVALID_ARG);
        }
        self.handler.deliver(ComboSubmitted { open, anchor })
    }
}
impl<State: 'static> EventSession<State> {
    /// Registers one captured-selection source without claiming the header's independent popup port.
    pub fn subscribe_combo_changed<Handler: EventHandler<State, ComboChanged> + 'static>(
        &self,
        combo: &crate::ComboWidget,
        handler: Handler,
    ) -> Result<ComPtr<IEventSubscriptionAbi>, NcStatus> {
        let mut leases = self.registry.take()?;
        let sink: ComPtr<IComboChangedSinkAbi> = ComboChangedSink::from_value(ComboChangedSink {
            handler: HandlerState {
                state: self.state.clone(),
                handler: RefCell::new(Some(handler)),
            },
        })
        .query()?;
        let lease = combo.as_interface().subscribe_changed(sink.as_com_ref())?;
        leases.push(lease.clone());
        Ok(lease)
    }
    /// Registers the header-to-popup coordinator; ordinary popup dismissal uses its separate family.
    pub fn subscribe_combo_popup<Handler: EventHandler<State, ComboSubmitted> + 'static>(
        &self,
        combo: &crate::ComboWidget,
        handler: Handler,
    ) -> Result<ComPtr<IEventSubscriptionAbi>, NcStatus> {
        let mut leases = self.registry.take()?;
        let sink: ComPtr<IComboPopupSinkAbi> = ComboPopupSink::from_value(ComboPopupSink {
            handler: HandlerState {
                state: self.state.clone(),
                handler: RefCell::new(Some(handler)),
            },
        })
        .query()?;
        let lease = combo.as_interface().subscribe_popup(sink.as_com_ref())?;
        leases.push(lease.clone());
        Ok(lease)
    }
}

impl<State: 'static> EventSession<State> {
    /// Registers a compact menu row's actual activation queue in the same deterministic sequence as widget events.
    pub fn subscribe_menu_item<Handler: EventHandler<State, Activation> + 'static>(
        &self,
        item: &crate::MenuItem,
        handler: Handler,
    ) -> Result<ComPtr<IEventSubscriptionAbi>, NcStatus> {
        let mut leases = self.registry.take()?;
        let sink: ComPtr<IActivationSinkAbi> = ActivationSink::from_value(ActivationSink {
            handler: HandlerState {
                state: self.state.clone(),
                handler: RefCell::new(Some(handler)),
            },
        })
        .query()?;
        let lease = item.as_interface().add_ref().query::<IActivationEventsAbi>()?.subscribe(sink.as_com_ref())?;
        leases.push(lease.clone());
        Ok(lease)
    }
}

/// One completed file-picker request; accepted native identity remains independent of display text.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FileDialogCompleted {
    /// Exact native path selected or lexically entered by the user.
    Accepted(std::path::PathBuf),
    /// The active request ended without a selected file.
    Cancelled,
}
#[com_object(interfaces(IFileDialogSink))]
/// Concrete local completion adapter owns the application's exact state and handler specialization.
struct FileDialogSink<State: 'static, Handler: EventHandler<State, FileDialogCompleted> + 'static> {
    /// Existing owned invocation helper; native Rust paths are decoded before borrowing application state.
    handler: HandlerState<State, Handler>,
}
impl<State: 'static, Handler: EventHandler<State, FileDialogCompleted> + 'static> IFileDialogSink for FileDialogSink<State, Handler> {
    /// Rejects an incompatible encoding and preserves exact native units when constructing the local event.
    fn accepted(&self, path: ComRef<'_, INativePathAbi>) -> Result<(), NcStatus> {
        self.handler.deliver(FileDialogCompleted::Accepted(crate::path::decode(path)?))
    }
    /// Emits the concrete cancellation outcome with no fabricated empty-path sentinel.
    fn cancelled(&self) -> Result<(), NcStatus> {
        self.handler.deliver(FileDialogCompleted::Cancelled)
    }
}
impl<State: 'static> EventSession<State> {
    /// Registers the picker's actual completion queue in the same deterministic application sequence.
    pub fn subscribe_file_dialog<Handler: EventHandler<State, FileDialogCompleted> + 'static>(
        &self,
        dialog: &crate::FileDialog,
        handler: Handler,
    ) -> Result<ComPtr<IEventSubscriptionAbi>, NcStatus> {
        let mut leases = self.registry.take()?;
        let sink: ComPtr<IFileDialogSinkAbi> = FileDialogSink::from_value(FileDialogSink {
            handler: HandlerState {
                state: self.state.clone(),
                handler: RefCell::new(Some(handler)),
            },
        })
        .query()?;
        let lease = dialog.as_interface().add_ref().query::<IFileDialogEventsAbi>()?.subscribe(sink.as_com_ref())?;
        leases.push(lease.clone());
        Ok(lease)
    }
}
impl source::EventPayload for FileDialogCompleted {
    type Sink = IFileDialogSinkAbi;
    /// Encodes native identity only at the boundary; lossy display text never becomes a selected path.
    fn deliver(&self, sink: ComRef<'_, Self::Sink>) -> Result<(), NcStatus> {
        match self {
            FileDialogCompleted::Accepted(path) => sink.accepted(crate::NativePath::new(path).as_interface()),
            FileDialogCompleted::Cancelled => sink.cancelled(),
        }
    }
}


/// One concrete serializer per standard snapshot; native Rust payloads never cross the ABI.
macro_rules! event_payload {
    ($event:ty, $sink:ty, $value:ident, $target:ident, $body:block) => {
        impl source::EventPayload for $event {
            type Sink = $sink;
            /// Delivers the captured snapshot without reading the live producer.
            fn deliver(&$value, $target: ComRef<'_, Self::Sink>) -> Result<(), NcStatus> $body
        }
    };
}
event_payload!(Activation, IActivationSinkAbi, self, sink, {
    sink.submitted(neocogi_com::NcString::from_utf8_str(&self.label).as_com_ref())
});
event_payload!(CheckboxChanged, ICheckboxSinkAbi, self, sink, { sink.changed(self.checked) });
event_payload!(NumericChanged, INumericSinkAbi, self, sink, { sink.changed(self.value) });
event_payload!(ScrollbarChanged, IScrollbarSinkAbi, self, sink, { sink.changed(self.offset) });
event_payload!(TextChanged, ITextChangedSinkAbi, self, sink, {
    sink.changed(neocogi_com::NcString::from_utf8_str(&self.text).as_com_ref(), self.cursor)
});
event_payload!(TextSubmitted, ITextSubmittedSinkAbi, self, sink, {
    sink.submitted(neocogi_com::NcString::from_utf8_str(&self.text).as_com_ref())
});
event_payload!(WindowEvent, IWindowSinkAbi, self, sink, { sink.event((*self).into()) });
event_payload!(PopupDismissed, IPopupSinkAbi, self, sink, { sink.dismissed() });
event_payload!(ComboChanged, IComboChangedSinkAbi, self, sink, {
    sink.changed(self.selected, neocogi_com::NcString::from_utf8_str(&self.label).as_com_ref())
});
event_payload!(ComboSubmitted, IComboPopupSinkAbi, self, sink, { sink.submitted(self.open, self.anchor) });
