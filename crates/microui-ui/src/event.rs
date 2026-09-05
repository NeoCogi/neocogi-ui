//
// Copyright 2023-Present (c) Raja Lehtihet & Wael El Oraiby
//
// Redistribution and use in source and binary forms, with or without
// modification, are permitted provided that the following conditions are met:
//
// 1. Redistributions of source code must retain the above copyright notice,
// this list of conditions and the following disclaimer.
//
// 2. Redistributions in binary form must reproduce the above copyright notice,
// this list of conditions and the following disclaimer in the documentation
// and/or other materials provided with the distribution.
//
// 3. Neither the name of the copyright holder nor the names of its contributors
// may be used to endorse or promote products derived from this software without
// specific prior written permission.
//
// THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS "AS IS"
// AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE
// IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE
// ARE DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT HOLDER OR CONTRIBUTORS BE
// LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR
// CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF
// SUBSTITUTE GOODS OR SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS
// INTERRUPTION) HOWEVER CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN
// CONTRACT, STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE)
// ARISING IN ANY WAY OUT OF THE USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE
// POSSIBILITY OF SUCH DAMAGE.

#![doc = include_str!("../docs/EVENTS.md")]

use std::cell::RefCell;
use std::fmt;
use std::rc::{Rc, Weak};

use crate::Widget;

/// Maximum number of native events handled in one dispatch transaction.
///
/// The limit applies across all subscriptions and all cascade sweeps initiated by one call to
/// [`WidgetEventDispatcher::dispatch_with_context`]. It is a correctness guard against mutually emitting handlers, not a
/// normal flow-control mechanism.
const MAX_EVENT_DISPATCHES: usize = 1_000_000;

/// Marker implemented by every typed event payload emitted by retained UI.
///
/// An event is an owned snapshot of the semantic or lifecycle fact its producer is reporting. For
/// example, a textbox change contains the text as it existed when the change occurred rather than a
/// reference into the mutable widget. This lets the port hold the payload until a safe context
/// dispatch boundary. Application-owned retained components use the same contract for lifecycle
/// events such as [`crate::FileDialogCompleted`]; no component-specific behavior enters the
/// dispatcher.
///
/// The `'static` bound is required because subscriptions of different concrete event types coexist
/// behind the dispatcher's `WidgetEventDispatch` boundary. It does not require event values to be `Clone`,
/// `Send`, or `Sync`.
pub trait WidgetEvent: 'static {}

/// A concrete retained widget that owns one typed native event source.
///
/// A widget may implement this trait more than once with different `E` types, such as separate
/// `Changed` and `Submitted` ports. The implementation returns a weak capability rather than the
/// port itself: callers can subscribe and inspect liveness, but cannot emit or drain widget events.
pub trait TypedWidget<E: WidgetEvent>: Widget {
    /// Returns a weak capability for this widget's `E` event source.
    ///
    /// Repeated calls identify the same widget-owned port. Each returned handle is weak and does
    /// not affect the lifetime of either the widget or port.
    fn event(&self) -> WidgetEventPortHandle<E>;
}

/// Producer-owned queue for one concrete semantic event type.
///
/// Store this in a widget or application component and expose only [`Self::event`] to subscribers.
/// Emission never invokes application code: Context delivers the owned payload after widget
/// traversal releases its borrows. Events emitted without a subscription are discarded.
///
/// The source is deliberately not cloneable. Several internal controls belonging to one component
/// may explicitly share an `Rc<EventSource<E>>`; ordinary widgets own the source directly. Neither
/// its weak endpoint nor a subscription extends the producer's lifetime. Sources belong to the UI
/// thread and are neither `Send` nor `Sync`.
pub struct EventSource<E: WidgetEvent> {
    /// Private queue ownership; neither callers nor subscribers can borrow or drain this storage.
    port: Rc<RefCell<WidgetEventPort<E>>>,
}

impl<E: WidgetEvent> EventSource<E> {
    /// Creates a source with no subscriber and no pending payload allocation.
    pub fn new() -> Self {
        Self {
            port: Rc::new(RefCell::new(WidgetEventPort::new())),
        }
    }

    /// Returns a weak, subscribable endpoint for this source without granting emission access.
    pub fn event(&self) -> WidgetEventPortHandle<E> {
        WidgetEventPortHandle::new(&self.port)
    }

    /// Queues an owned snapshot after the producer commits the state described by it.
    ///
    /// The short queue borrow ends here. Delivery occurs at Context's next dispatch boundary;
    /// callers must not expect their subscribed application handler to have run on return.
    pub fn emit(&self, event: E) {
        self.port.borrow_mut().emit(event);
    }

    /// Permanently closes this source, preserving one terminal event for its current subscriber.
    ///
    /// Pending ordinary events are discarded. Handles immediately report expired, further emission
    /// is ignored, and subsequent close calls preserve the first terminal event. Without a current
    /// subscriber the terminal event is discarded too; closing never creates replay history.
    pub fn close(&self, terminal: E) {
        self.port.borrow_mut().close(Some(terminal));
    }
}

impl<E: WidgetEvent> Drop for EventSource<E> {
    /// Expires endpoints and discards ordinary events without erasing an explicit terminal event.
    fn drop(&mut self) {
        self.port.borrow_mut().close(None);
    }
}

impl<E: WidgetEvent> Default for EventSource<E> {
    /// Uses the same disconnected initial state as [`Self::new`].
    fn default() -> Self {
        Self::new()
    }
}

/// One retained-producer-owned typed event queue and its connection state.
///
/// Connected owns ordinary queued events; Closed may retain one explicit terminal fact.
/// Disconnected is distinct from both, with no redundant liveness or connection boolean.
enum WidgetEventPort<E: WidgetEvent> {
    /// No subscription exists; emitted events are discarded.
    Disconnected,
    /// Exactly one subscription exists and owns the right to drain this FIFO.
    Connected {
        /// Ordinary snapshots awaiting the next safe dispatch boundary.
        pending: Vec<E>,
    },
    /// The producer closed or was dropped; only an explicitly published terminal event survives.
    Closed {
        /// One final payload retained by the existing subscription until its next dispatch.
        terminal: Option<E>,
    },
}

impl<E: WidgetEvent> WidgetEventPort<E> {
    /// Creates a disconnected port.
    ///
    /// Widgets construct their ports before they are mounted or subscribed, so the initial state
    /// deliberately has no queue allocation and records no events.
    fn new() -> Self {
        Self::Disconnected
    }

    /// Appends an event when a subscription is connected, otherwise discards it.
    ///
    /// The widget calls this only after committing the semantic or lifecycle state described by
    /// `event`. Delivery is deferred; this method never invokes application code.
    fn emit(&mut self, event: E) {
        if let Self::Connected { pending } = self {
            pending.push(event);
        }
    }

    /// Transitions a disconnected port to a connected port with an empty queue.
    ///
    /// The exclusive connection enforces the design's one-port/one-state-method rule.
    fn connect(&mut self) -> Result<(), SubscribeError> {
        if matches!(self, Self::Closed { .. }) {
            return Err(SubscribeError::WidgetExpired);
        }
        if matches!(self, Self::Connected { .. }) {
            return Err(SubscribeError::AlreadySubscribed);
        }
        *self = Self::Connected { pending: Vec::new() };
        Ok(())
    }

    /// Transitions a connected port to the disconnected state and drops queued payloads.
    ///
    /// Calling this for an already disconnected port is harmless. In normal operation it is called
    /// by the exclusive subscription's [`Drop`] implementation.
    fn disconnect(&mut self) {
        // Disconnection cannot resurrect a closed source or make its terminal event replayable.
        *self = if matches!(self, Self::Closed { .. }) {
            Self::Closed { terminal: None }
        } else {
            Self::Disconnected
        };
    }

    /// Closes once, retaining a terminal payload only if a subscriber was connected.
    fn close(&mut self, terminal: Option<E>) {
        match self {
            Self::Connected { .. } => *self = Self::Closed { terminal },
            Self::Disconnected => *self = Self::Closed { terminal: None },
            Self::Closed { .. } => {} // A later producer drop must not overwrite its terminal fact.
        }
    }

    /// Moves the complete pending batch out while preserving the connected state.
    ///
    /// Returning an owned queue ensures the port's `RefCell` borrow ends before a handler runs. An
    /// event emitted recursively by that handler therefore enters the new empty queue and is
    /// observed by a subsequent dispatcher sweep.
    fn drain(&mut self) -> Vec<E> {
        match self {
            Self::Disconnected => Vec::new(),
            Self::Connected { pending } => std::mem::take(pending),
            Self::Closed { terminal } => terminal.take().into_iter().collect(),
        }
    }
}

/// Weak, typed capability identifying one event source owned by retained UI.
///
/// Holding or cloning this value does not keep the widget or its pending events alive.
/// The event type parameter prevents connecting a handler for one payload type to another port at
/// compile time.
pub struct WidgetEventPortHandle<E: WidgetEvent>(Weak<RefCell<WidgetEventPort<E>>>);

impl<E: WidgetEvent> WidgetEventPortHandle<E> {
    /// Creates a weak handle to a live retained-producer-owned port.
    fn new(port: &Rc<RefCell<WidgetEventPort<E>>>) -> Self {
        Self(Rc::downgrade(port))
    }

    /// Creates the same observable state as a handle whose widget has already been removed.
    ///
    /// Projection through an unavailable typed widget handle uses this value so event access stays
    /// weak and fallible without manufacturing an owner.
    fn expired() -> Self {
        Self(Weak::new())
    }

    /// Returns whether the concrete retained producer still owns this event source.
    ///
    /// This is a liveness observation, not an ownership claim. The handle remains weak, and a later
    /// attempt to subscribe can still report [`SubscribeError::WidgetExpired`] if the producer is
    /// removed or closed between operations.
    pub fn is_alive(&self) -> bool {
        self.0.upgrade().is_some_and(|port| !matches!(*port.borrow(), WidgetEventPort::Closed { .. }))
    }
}

impl<W: Widget + 'static> crate::TypedWidgetHandle<W> {
    /// Projects a concrete widget-owned event port through this weak typed widget handle.
    ///
    /// Access is intentionally brief: [`TypedWidget::event`] clones only the weak port capability,
    /// and the concrete widget borrow is released before the caller can subscribe. If the widget
    /// cell is currently unavailable or already dead, the result behaves as an expired event
    /// handle.
    pub fn widget_event<E: WidgetEvent>(&self) -> WidgetEventPortHandle<E>
    where
        W: TypedWidget<E>,
    {
        self.try_read(TypedWidget::event).unwrap_or_else(WidgetEventPortHandle::expired)
    }
}

impl<E: WidgetEvent> Clone for WidgetEventPortHandle<E> {
    /// Clones only the weak capability; this neither clones an event nor retains a widget.
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<E: WidgetEvent> fmt::Debug for WidgetEventPortHandle<E> {
    /// Reports current liveness without exposing port identity or queued payloads.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("WidgetEventPortHandle").field("alive", &self.is_alive()).finish_non_exhaustive()
    }
}

/// Failure to subscribe an application-state method to a retained UI event.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum SubscribeError {
    /// The retained producer that owns the event port has already been removed or was unavailable
    /// when its typed handle projected the event capability.
    WidgetExpired,
    /// The event port already has its exclusive subscription.
    ///
    /// In normal application code this means the port is already subscribed through its owning
    /// context.
    AlreadySubscribed,
}

impl fmt::Display for SubscribeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::WidgetExpired => "widget event owner has expired",
            Self::AlreadySubscribed => "widget event already has a subscriber",
        })
    }
}

impl std::error::Error for SubscribeError {}

/// Object-safe boundary allowing one dispatcher to store heterogeneous event subscriptions.
///
/// `Target` remains common to the whole vector; the implementation retains each concrete event and
/// handler type. This is the module's only dynamic dispatch boundary.
trait WidgetEventDispatch<Target> {
    /// Reports whether a live source or undelivered terminal fact still needs this subscription.
    fn needs_dispatch(&self) -> bool;
    /// Drains one port batch into `target` and returns the number of delivered payloads.
    fn dispatch(&mut self, target: &mut Target, ui: &mut crate::Ui<'_>) -> usize;
}

/// Concrete binding between one typed event port and one typed target handler.
///
/// `Handler` is the concrete callback supplied to `Context::subscribe`, including its captures.
/// It is stored directly inside this subscription; only the outer dispatch boundary is erased.
/// The application state and UI capability are borrowed anew for each event and cannot escape.
/// Queue ownership lets an explicit terminal event survive producer removal. The source closes its
/// queue on drop, so this storage never retains the producer or ordinary events after removal.
struct Subscription<E: WidgetEvent, Handler> {
    /// Queue storage only; producer lifetime is represented by its explicit Closed state.
    port: Rc<RefCell<WidgetEventPort<E>>>,
    /// Concrete invocation data, independent of producer ownership.
    handler: Handler,
}

impl<E: WidgetEvent, Handler> Subscription<E, Handler> {
    /// Connects a live port and creates its sole subscription.
    fn new(port: WidgetEventPortHandle<E>, handler: Handler) -> Result<Self, SubscribeError> {
        let Some(inner) = port.0.upgrade() else {
            return Err(SubscribeError::WidgetExpired);
        };
        inner.borrow_mut().connect()?;
        Ok(Self { port: inner, handler })
    }
}

impl<Target, E, Handler> WidgetEventDispatch<Target> for Subscription<E, Handler>
where
    E: WidgetEvent,
    Handler: FnMut(&mut Target, &mut crate::Ui<'_>, &E),
{
    /// Reports whether the subscription has a live producer or an undelivered terminal event.
    fn needs_dispatch(&self) -> bool {
        // A dead producer gets one final dispatch opportunity only if it explicitly closed with
        // a terminal event; ordinary queues disappear immediately on producer drop.
        !matches!(*self.port.borrow(), WidgetEventPort::Closed { terminal: None })
    }

    /// Detaches one complete batch, then invokes the concrete handler in port FIFO order.
    ///
    /// Detaching before the first invocation is essential: handler code may cause widgets to emit
    /// without colliding with a live mutable borrow of this port.
    fn dispatch(&mut self, target: &mut Target, ui: &mut crate::Ui<'_>) -> usize {
        let events = self.port.borrow_mut().drain();
        let count = events.len();
        for event in events {
            // The port borrow ended when drain returned, so a context-aware handler may safely
            // mutate or destroy retained roots before the next payload is delivered.
            (self.handler)(target, ui, &event);
        }
        count
    }
}

impl<E: WidgetEvent, Handler> Drop for Subscription<E, Handler> {
    /// Disconnects a still-live port and clears its queue.
    ///
    /// Closed queues stay closed; dropping a subscription also discards an undelivered terminal fact.
    fn drop(&mut self) {
        self.port.borrow_mut().disconnect();
    }
}

/// A strongly typed semantic widget-event dispatcher for one mutable target type.
///
/// Vector position is subscription order and therefore the deterministic cross-port sweep order.
/// Each boxed element retains concrete payload and handler types behind [`WidgetEventDispatch`]. This
/// type is crate-private because Context owns the application-state widget-event dispatcher for
/// the lifetime of the corresponding retained forest. Raw pointer, keyboard, and text input are
/// routed independently by the retained [`crate::UiRuntime`].
pub(crate) struct WidgetEventDispatcher<Target> {
    /// Heterogeneous subscriptions in deterministic registration order.
    subscriptions: Vec<Box<dyn WidgetEventDispatch<Target>>>,
}

impl<Target: 'static> WidgetEventDispatcher<Target> {
    /// Creates an empty dispatcher before its retained event sources are subscribed.
    pub(crate) fn new() -> Self {
        // Subscription storage remains unallocated until the owning Context connects its first
        // typed event source.
        Self { subscriptions: Vec::new() }
    }

    /// Connects one typed callback and appends its subscription in deterministic sweep order.
    ///
    /// Connection is validated before publication. Each callback retains its concrete captures
    /// inside the existing subscription allocation; there is no separately boxed callable.
    pub(crate) fn subscribe<E: WidgetEvent>(
        &mut self,
        port: WidgetEventPortHandle<E>,
        handler: impl FnMut(&mut Target, &mut crate::Ui<'_>, &E) + 'static,
    ) -> Result<(), SubscribeError> {
        self.subscriptions.push(Box::new(Subscription::new(port, handler)?));
        Ok(())
    }

    /// Delivers all pending events and finite cascades into the dispatch target.
    ///
    /// Dead widget subscriptions are removed first. The remaining subscriptions are swept in
    /// vector order until one complete sweep delivers nothing. The return value is `true` when at
    /// least one event was delivered; callers can use that result to decide whether target-handler
    /// effects require lifecycle reconciliation or another layout commit.
    ///
    /// The cumulative count is checked after every drained port batch. Arithmetic overflow and a
    /// transaction exceeding [`MAX_EVENT_DISPATCHES`] both panic because either indicates a broken
    /// event feedback loop rather than recoverable input.
    pub(crate) fn dispatch_with_context(&mut self, target: &mut Target, ui: &mut crate::Ui<'_>) -> bool {
        // Prune closed empty queues before lending UI access. Closed queues with a terminal fact
        // survive this sweep long enough to deliver it without retaining their removed producer.
        self.subscriptions.retain(|subscription| subscription.needs_dispatch());

        let mut dispatched = 0usize;
        loop {
            let before = dispatched;
            for subscription in &mut self.subscriptions {
                dispatched = dispatched
                    .checked_add(subscription.dispatch(target, ui))
                    .expect("widget event dispatch count overflowed");
                assert!(
                    dispatched <= MAX_EVENT_DISPATCHES,
                    "widget event cascade exceeded {MAX_EVENT_DISPATCHES} native events in one input transaction"
                );
            }
            if dispatched == before {
                break;
            }
        }
        dispatched != 0
    }

    /// Delivers pending events with an otherwise unused UI capability in focused unit tests.
    ///
    /// Production dispatch always uses [`Self::dispatch_with_context`] and the live context-owned
    /// window manager. Tests for isolated widget ports do not exercise root mutation, so constructing
    /// an empty manager here keeps their scope narrow without weakening the production signature.
    #[cfg(test)]
    pub(crate) fn dispatch(&mut self, target: &mut Target) -> bool {
        // Bind the temporary manager's skin to one concrete atlas ownership domain, just as a
        // production Context does. Keeping the handle alive for the complete dispatch also makes
        // the otherwise-unused test capability explicit instead of manufacturing ownerless IDs.
        let atlas = crate::test_support::test_atlas();
        let style = crate::test_support::test_skin(&atlas);
        let bundle = crate::SkinBundle::new(atlas, style);
        let mut window_manager = crate::window_manager::WindowManager::new(bundle);
        let mut ui = crate::Ui::new(&mut window_manager);
        self.dispatch_with_context(target, &mut ui)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    impl WidgetEvent for i32 {}

    #[derive(Default)]
    struct State {
        values: Vec<i32>,
        source: Option<Rc<EventSource<i32>>>,
    }

    impl State {
        fn record_and_cascade(&mut self, event: &i32) {
            self.values.push(*event);
            if *event < 10 {
                self.source.as_ref().unwrap().emit(event + 10);
            }
        }

        fn record_with_offset(&mut self, offset: &i32, event: &i32) {
            self.values.push(offset + event);
        }
    }

    #[test]
    fn port_queues_native_events_until_the_dispatcher_dispatches() {
        let owner = Rc::new(EventSource::new());
        let mut dispatcher = WidgetEventDispatcher::new();
        dispatcher
            .subscribe(owner.event(), |state, _, event| State::record_and_cascade(state, event))
            .unwrap();

        owner.emit(3);
        owner.emit(4);
        let mut state = State {
            source: Some(Rc::clone(&owner)),
            ..State::default()
        };
        assert!(state.values.is_empty());
        assert!(dispatcher.dispatch(&mut state));
        assert_eq!(state.values, [3, 4, 13, 14]);
    }

    #[test]
    fn bound_subscriber_receives_registration_context() {
        let owner = Rc::new(EventSource::new());
        let mut dispatcher = WidgetEventDispatcher::new();
        dispatcher
            .subscribe(owner.event(), move |state, _, event| State::record_with_offset(state, &40, event))
            .unwrap();

        owner.emit(2);
        let mut state = State::default();
        assert!(dispatcher.dispatch(&mut state));
        assert_eq!(state.values, [42]);
    }

    #[test]
    fn one_port_accepts_only_one_context_subscription() {
        let owner = Rc::new(EventSource::new());
        let port = owner.event();
        let mut dispatcher = WidgetEventDispatcher::new();
        dispatcher
            .subscribe(port.clone(), |state, _, event| State::record_and_cascade(state, event))
            .unwrap();

        assert_eq!(
            dispatcher.subscribe(port, |state, _, event| State::record_and_cascade(state, event)),
            Err(SubscribeError::AlreadySubscribed)
        );
    }

    #[test]
    fn dropping_the_context_dispatcher_disconnects_and_clears_its_ports() {
        let owner = Rc::new(EventSource::new());
        let port = owner.event();
        let mut dispatcher = WidgetEventDispatcher::new();
        dispatcher
            .subscribe(port.clone(), |state, _, event| State::record_and_cascade(state, event))
            .unwrap();

        owner.emit(1);
        drop(dispatcher);
        owner.emit(2);
        assert!(matches!(&*owner.port.borrow(), WidgetEventPort::Disconnected));

        let mut replacement = WidgetEventDispatcher::new();
        replacement.subscribe(port, |state, _, event| State::record_and_cascade(state, event)).unwrap();
    }

    #[test]
    fn expired_subscriptions_are_pruned_during_dispatch() {
        let owner = Rc::new(EventSource::new());
        let mut dispatcher = WidgetEventDispatcher::new();
        dispatcher
            .subscribe(owner.event(), |state, _, event| State::record_and_cascade(state, event))
            .unwrap();
        drop(owner);

        assert!(!dispatcher.dispatch(&mut State::default()));
        assert!(dispatcher.subscriptions.is_empty());
    }

    #[test]
    fn ports_discard_events_without_a_subscription() {
        let owner = Rc::new(EventSource::new());
        owner.emit(1);

        let mut dispatcher = WidgetEventDispatcher::new();
        dispatcher
            .subscribe(owner.event(), |state, _, event| State::record_and_cascade(state, event))
            .unwrap();
        assert!(!dispatcher.dispatch(&mut State::default()));
    }

    /// Closing preserves exactly one terminal fact while immediately expiring public endpoints.
    #[test]
    fn terminal_event_survives_source_drop_without_preserving_ordinary_events() {
        let source = EventSource::new();
        let endpoint = source.event();
        let mut dispatcher = WidgetEventDispatcher::new();
        dispatcher
            .subscribe(endpoint.clone(), |state, _, event| State::record_and_cascade(state, event))
            .unwrap();
        source.emit(1);
        source.close(99);
        source.close(100); // Repeated closure cannot replace the first terminal fact.
        source.emit(2);
        assert!(!endpoint.is_alive());
        assert_eq!(
            dispatcher.subscribe(endpoint, |state, _, event| State::record_and_cascade(state, event)),
            Err(SubscribeError::WidgetExpired)
        );
        drop(source);

        let mut state = State::default();
        assert!(dispatcher.dispatch(&mut state));
        assert_eq!(state.values, [99]);
        assert!(!dispatcher.dispatch(&mut state));
        assert!(dispatcher.subscriptions.is_empty());
    }

    /// Ordinary destruction deliberately has no terminal history or retained payload batch.
    #[test]
    fn dropping_an_open_source_discards_pending_events() {
        let source = EventSource::new();
        let mut dispatcher = WidgetEventDispatcher::new();
        dispatcher
            .subscribe(source.event(), |state, _, event| State::record_and_cascade(state, event))
            .unwrap();
        source.emit(1);
        drop(source);
        assert!(!dispatcher.dispatch(&mut State::default()));
    }

    /// A terminal fact is delivered only to the subscription that existed at closure time.
    #[test]
    fn closing_without_a_subscriber_never_creates_replay_history() {
        let source = EventSource::new();
        source.close(99);
        let mut dispatcher = WidgetEventDispatcher::new();
        assert_eq!(
            dispatcher.subscribe(source.event(), |state, _, event| State::record_and_cascade(state, event)),
            Err(SubscribeError::WidgetExpired)
        );
    }
    /// Mutable owned captures persist across batches and are dropped when the producer expires.
    #[test]
    fn callback_capture_state_persists_and_is_released_with_the_subscription() {
        let source = EventSource::new();
        let label = Rc::new(String::from("counter"));
        let lifetime = Rc::downgrade(&label);
        let mut dispatcher = WidgetEventDispatcher::<Vec<String>>::new();
        let mut history = Vec::new();
        dispatcher
            .subscribe(source.event(), move |values, _, event: &i32| {
                history.push(*event);
                values.push(format!("{label}:{}={event}", history.len()));
            })
            .unwrap();

        let mut values = Vec::new();
        source.emit(10);
        source.emit(20);
        assert!(dispatcher.dispatch(&mut values));
        source.emit(30);
        assert!(dispatcher.dispatch(&mut values));
        assert_eq!(values, ["counter:1=10", "counter:2=20", "counter:3=30"]);
        assert!(lifetime.upgrade().is_some());

        drop(source);
        assert!(!dispatcher.dispatch(&mut values));
        assert!(lifetime.upgrade().is_none(), "pruning releases the concrete callback captures");
    }
}
