//! Semantic-algorithm fixture. Production delivery uses the facade's concrete COM subscriptions.
use crate::{EventSource, WidgetEventPortHandle};
use microui_redux::events::test_support::{EventLease, SubscribeError};
#[cfg(test)]
use std::rc::Rc;
/// Keep runaway test callbacks bounded by the same published runtime budget.
const MAX_EVENT_DISPATCHES: usize = microui_contracts::EVENT_DISPATCH_LIMIT as usize;

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
struct Subscription<E: 'static, Handler> {
    /// Shared exclusive queue lease; producer and callback ownership remain separate.
    lease: EventLease<E>,
    /// Concrete invocation data retained directly inside the existing subscription allocation.
    handler: Handler,
}

impl<E: 'static, Handler> Subscription<E, Handler> {
    /// Connects using the same ownership rules as a concrete COM subscription.
    fn new(port: WidgetEventPortHandle<E>, handler: Handler) -> Result<Self, SubscribeError> {
        Ok(Self {
            lease: EventLease::connect(port)?,
            handler,
        })
    }
}

impl<Target, E, Handler> WidgetEventDispatch<Target> for Subscription<E, Handler>
where
    E: 'static,
    Handler: FnMut(&mut Target, &mut crate::Ui<'_>, &E),
{
    /// Preserves a live source or one undelivered terminal fact without retaining its producer.
    fn needs_dispatch(&self) -> bool {
        self.lease.active()
    }

    /// Delivers detached snapshots, stopping if an earlier callback closes their ordinary producer.
    fn dispatch(&mut self, target: &mut Target, ui: &mut crate::Ui<'_>) -> usize {
        let batch = self.lease.drain(usize::MAX).expect("an unbounded local batch cannot exhaust its budget");
        let mut delivered = 0;
        for event in batch.events {
            if !self.lease.can_deliver(batch.terminal) {
                break;
            }
            (self.handler)(target, ui, &event);
            delivered += 1;
        }
        delivered
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
    pub(crate) fn subscribe<E: 'static>(
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
        assert!(!owner.connected());

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
