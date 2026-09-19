//! Generated foreign connections exercise immediate mapping and deferred consumer behavior.
use super::*;
use neocogi_com::{com_object, ComPtr, NcCopyFromBufferOffset};
use std::{cell::Cell, rc::Rc};

/// Observable source state belongs to the producer, independently of its consumers.
struct SourceProbe {
    /// Production connection point; it has no pending-message storage.
    source: EventSource<Activation>,
    /// One rejected connection proves unsuccessful setup releases mapper captures.
    fail_once: Cell<bool>,
}
#[com_object(interfaces(IActivationEvents))]
/// Foreign source delegates to the same native connection used by widgets.
struct ForeignActivation {
    /// Fixture access observes source lifecycle without retaining any consumer state.
    probe: Rc<SourceProbe>,
}
impl IActivationEvents for ForeignActivation {
    /// Failure does not install a connection; successful setup owns exactly one receiver lease.
    fn subscribe_activated(&self, sink: ComRef<'_, ActivationPortAbi>) -> Result<ComPtr<IEventSubscriptionAbi>, NcStatus> {
        match self.probe.fail_once.replace(false) {
            true => Err(NC_E_INVALID_ARG),
            false => self.probe.source.subscribe(sink),
        }
    }
}
/// Records release of a concrete mapping capture, including failed connection attempts.
struct Lifetime(Rc<Cell<u32>>);
impl Drop for Lifetime {
    /// Destruction remains observable even while a foreign lease is separately retained.
    fn drop(&mut self) {
        self.0.set(self.0.get() + 1);
    }
}
/// Creates a real foreign source whose port can outlive a temporary borrowed projection.
fn source(fail_once: bool) -> (ComPtr<IActivationEventsAbi>, Rc<SourceProbe>) {
    let probe = Rc::new(SourceProbe {
        source: EventSource::new(),
        fail_once: Cell::new(fail_once),
    });
    (
        ForeignActivation::from_value(ForeignActivation { probe: probe.clone() }).query().unwrap(),
        probe,
    )
}
/// Domain validation runs before translation can post an invalid fact.
fn valid_label(event: &Activation) -> Result<(), NcStatus> {
    match event.label.is_empty() {
        true => Err(NC_E_INVALID_ARG),
        false => Ok(()),
    }
}

/// All protocols mapped to one inbox preserve actual emission order, not registration order.
#[test]
fn foreign_mapping_failure_validation_fifo_and_lifetime() {
    let mut events = Events::new();
    let drops = Rc::new(Cell::new(0));
    let (first, a) = source(false);
    let (second, b) = source(true);
    let lifetime = Lifetime(drops.clone());
    let rejected = events.map(second.activated(), move |event| {
        let _ = &lifetime;
        event.label
    });
    assert_eq!(rejected.unwrap_err(), NC_E_INVALID_ARG);
    assert_eq!(drops.get(), 1);
    assert!(!b.source.connected());
    let lifetime = Lifetime(drops.clone());
    let lease_a = events
        .map(first.as_com_ref().activated(), move |event| {
            let _ = &lifetime;
            event.label
        })
        .unwrap();
    let lease_b = events.map(second.activated().checked(valid_label), |event| event.label).unwrap();
    assert_eq!(b.source.emit(Activation { label: String::new() }), Err(NC_E_INVALID_ARG));
    assert!(events.is_empty());
    b.source.emit(Activation { label: "second".into() }).unwrap();
    a.source.emit(Activation { label: "first".into() }).unwrap();
    lease_b.cancel().unwrap();
    a.source.close_ordinary();
    assert_eq!(drops.get(), 2);
    // Cancellation and producer closure cannot retract already committed consumer facts.
    assert_eq!(events.pop().as_deref(), Some("second"));
    assert_eq!(events.pop().as_deref(), Some("first"));
    assert_eq!(lease_a.active(), Ok(false));
    drop(events);
    assert_eq!(lease_b.active(), Ok(false));
}

/// Concrete routing IDs allow one protocol to serve multiple application destinations.
#[test]
fn direct_mapping_is_immediate_and_consumption_is_deferred() {
    let mut events = Events::new();
    let (first, a) = source(false);
    let (second, b) = source(false);
    let mut sequence = 0;
    events
        .map(first.activated(), move |event| {
            sequence += 1;
            (0, sequence, event.label)
        })
        .unwrap();
    events.map(second.activated(), |event| (1, 0, event.label)).unwrap();
    a.source.emit(Activation { label: "a".into() }).unwrap();
    b.source.emit(Activation { label: "b".into() }).unwrap();
    a.source.emit(Activation { label: "c".into() }).unwrap();
    let mut consumed = Vec::new();
    assert_eq!(events.len(), 3);
    assert_eq!(
        events.dispatch(2, |message| {
            consumed.push(message);
            Ok(())
        }),
        Ok(2)
    );
    assert_eq!(consumed, [(0, 1, "a".into()), (1, 0, "b".into())]);
    assert_eq!(events.pop(), Some((0, 2, "c".into())));
}

/// A cancelled old lease cannot disconnect a replacement, and source drop preserves posted messages.
#[test]
fn reconnect_and_source_drop_do_not_change_inbox_ownership() {
    let mut first = Events::new();
    let mut second = Events::new();
    let source = EventSource::<Activation>::new();
    let old = first.map(source.event(), |event| event.label).unwrap();
    assert_eq!(second.map(source.event(), |event| event.label).unwrap_err(), crate::UI_E_BUSY);
    old.cancel().unwrap();
    let current = second.map(source.event(), |event| event.label).unwrap();
    old.cancel().unwrap();
    source.emit(Activation { label: "survives".into() }).unwrap();
    drop(source);
    assert_eq!(current.active(), Ok(false));
    assert_eq!(second.pop().as_deref(), Some("survives"));
    assert!(first.is_empty());
}

/// ABI decoding and domain validation occur before mapping, without application-state borrow gates.
#[test]
fn generated_receivers_validate_before_posting_owned_snapshots() {
    let mut numeric = Events::new();
    let mut sink = create_numeric_changes_port_sink(numeric.sender(), |event| event.value, Some(valid_numeric)).unwrap();
    assert_eq!(sink.changed(f32::NAN), Err(NC_E_INVALID_ARG));
    sink.changed(1.0).unwrap();
    assert_eq!(numeric.pop(), Some(1.0));
    let mut text = Events::new();
    let mut sink = neocogi_ui_core_contracts::create_text_changes_port_sink(text.sender(), |event| event.text, Some(valid_text_cursor)).unwrap();
    assert_eq!(sink.changed(NcCopyFromBufferOffset::new(1, "é".as_bytes()), 2), Err(NC_E_INVALID_ARG));
    assert_eq!(sink.changed(NcCopyFromBufferOffset::new(0, "é".as_bytes()), 1), Err(NC_E_INVALID_ARG));
    sink.changed(NcCopyFromBufferOffset::new(0, "é".as_bytes()), 2).unwrap();
    assert_eq!(text.pop().as_deref(), Some("é"));
}

/// Immediate mappers cannot recursively post through the same source; cancellation during posting is safe.
#[test]
fn recursive_post_rejects_and_closure_during_post_keeps_the_committed_message() {
    let source = Rc::new(EventSource::<Activation>::new());
    let captured = source.clone();
    let mut events = Events::new();
    let lease = events
        .map(source.event(), move |event| {
            assert_eq!(captured.emit(Activation { label: "recursive".into() }), Err(neocogi_com_events::EVENT_E_BUSY));
            captured.close_ordinary();
            event.label
        })
        .unwrap();
    source.emit(Activation { label: "committed".into() }).unwrap();
    assert_eq!(lease.active(), Ok(false));
    assert_eq!(events.pop().as_deref(), Some("committed"));
}
