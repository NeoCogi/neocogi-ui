//! End-to-end subscription regressions through generated ports and COM source calls.

use super::*;
use neocogi_com::{com_object, ComPtr};
use std::{cell::Cell, rc::Rc};

/// Foreign producer observations shared with the test, not with application state.
struct SourceProbe {
    /// Actual production queue; this fixture does not replace event transport.
    queue: EventSource<Activation>,
    /// Counts foreign calls so failure and retry cannot be hidden by adapter behavior.
    attempts: Cell<usize>,
    /// The shared COM method consumes one injected subscription failure.
    fail_once: Cell<bool>,
}

#[com_object(interfaces(IActivationEvents))]
/// Foreign source exposing the real generated activation port and producer queue.
struct ForeignActivation {
    /// Producer-side ownership and observable shared-method state.
    probe: Rc<SourceProbe>,
    /// Retained projection checks the production registration gate during the call.
    dispatcher: ComPtr<IEventDispatcherAbi>,
}

impl IActivationEvents for ForeignActivation {
    /// Rejects dispatch reentry before failure injection or real queue subscription.
    fn subscribe_activated(&self, sink: ComRef<'_, IActivationSinkAbi>) -> Result<ComPtr<IEventSubscriptionAbi>, NcStatus> {
        assert_eq!(self.dispatcher.dispatch(10), Err(neocogi_com_events::EVENT_E_BUSY));
        self.probe.attempts.set(self.probe.attempts.get() + 1);
        match self.probe.fail_once.replace(false) {
            true => Err(NC_E_INVALID_ARG),
            false => self.probe.queue.subscribe(sink),
        }
    }
}

/// Concrete application handler with explicit lifetime observations.
struct Record {
    /// External counter proves failed setup and session destruction release handlers.
    drops: Rc<Cell<usize>>,
}

impl EventHandler<Vec<String>, Activation> for Record {
    /// Stores the immutable captured label in application registration order.
    fn handle(&mut self, labels: &mut Vec<String>, event: &Activation) -> Result<(), NcStatus> {
        labels.push(event.label.clone());
        Ok(())
    }
}

impl Drop for Record {
    /// Observes actual handler release independently of retained source and lease pointers.
    fn drop(&mut self) {
        self.drops.set(self.drops.get() + 1);
    }
}

/// Creates a foreign producer without borrowing or retaining application state.
fn source(session: &neocogi_com_events::EventSession<Vec<String>>, fail_once: bool) -> (ComPtr<IActivationEventsAbi>, Rc<SourceProbe>) {
    let probe = Rc::new(SourceProbe {
        queue: EventSource::new(),
        attempts: Cell::new(0),
        fail_once: Cell::new(fail_once),
    });
    let source = ForeignActivation::from_value(ForeignActivation {
        probe: probe.clone(),
        dispatcher: session.as_dispatcher().add_ref(),
    })
    .query()
    .unwrap();
    (source, probe)
}

/// Existing validator-function policy remains installed by the checked port.
fn valid_label(event: &Activation) -> Result<(), NcStatus> {
    match event.label.is_empty() {
        true => Err(NC_E_INVALID_ARG),
        false => Ok(()),
    }
}

/// Plain and checked ports preserve failed-call cleanup, retry, order, and cancellation.
#[test]
fn generated_ports_preserve_foreign_failure_and_checked_delivery() {
    let session = neocogi_com_events::EventSession::new(Vec::<String>::new());
    let drops = Rc::new(Cell::new(0));
    let (first_source, first) = source(&session, false);
    let (second_source, second) = source(&session, true);
    // Constructing a port is infallible and must not attempt a subscription.
    // The borrowed-handle accessor owns its projection after this block ends.
    let first_port = { first_source.as_com_ref().activated() };
    assert_eq!(first.attempts.get(), 0);
    drop(first_source);
    let first_lease = session.subscribe(first_port, Record { drops: drops.clone() }).unwrap();

    // A foreign failure drops the uncommitted sink/handler and preserves old registrations.
    let failure = session.subscribe(second_source.activated().checked(valid_label), Record { drops: drops.clone() });
    assert_eq!(failure.unwrap_err(), NC_E_INVALID_ARG);
    assert_eq!(drops.get(), 1);
    assert_eq!(second.attempts.get(), 1);
    assert!(!second.queue.connected());
    assert_eq!(first_lease.active(), Ok(true));

    let second_lease = session
        .subscribe(second_source.activated().checked(valid_label), Record { drops: drops.clone() })
        .unwrap();
    assert_eq!(first.attempts.get(), 1);
    assert_eq!(second.attempts.get(), 2);

    // The port must install validation before delivery can borrow local state.
    second.queue.emit(Activation { label: String::new() });
    let held = session.state_mut().unwrap();
    assert_eq!(second_lease.dispatch(10), Err(NC_E_INVALID_ARG));
    drop(held);
    assert!(session.state().unwrap().is_empty());

    // Reverse emission order does not change registration-ordered dispatch.
    second.queue.emit(Activation { label: "second".to_owned() });
    first.queue.emit(Activation { label: "first".to_owned() });
    assert_eq!(session.dispatch(), Ok(2));
    assert_eq!(&*session.state().unwrap(), &["first", "second"]);

    // Retained foreign capabilities and leases cannot keep cancelled handlers alive.
    drop(session);
    assert_eq!(drops.get(), 3);
    assert_eq!(first_lease.active(), Ok(false));
    assert_eq!(second_lease.active(), Ok(false));
}
