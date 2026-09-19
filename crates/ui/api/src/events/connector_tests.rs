//! Concrete foreign producers exercise direct replacement, ownership and deferred behavior.
use super::*;
use neocogi_com::{com_object, ComPtr, NcMessage};
use std::{cell::Cell, rc::Rc};

#[com_object(interfaces(IActivationEvents, __ActivationReceiver))]
/// Foreign producer uses only ordinary fields and mutable COM methods.
struct ForeignActivation {
    /// The complete delivery storage: one optional typed destination.
    destination: Option<ComPtr<__ActivationReceiverAbi>>,
    /// One failed setup proves mapper captures are released on installation errors.
    fail_once: bool,
}
impl IActivationEvents for ForeignActivation {
    /// Failed setup leaves the old destination intact; successful setup replaces it.
    fn __map_activated(&mut self, destination: ComRef<'_, __ActivationReceiverAbi>) -> Result<(), NcStatus> {
        match std::mem::replace(&mut self.fail_once, false) {
            true => Err(NC_E_INVALID_ARG),
            false => {
                self.destination = Some(destination.add_ref());
                Ok(())
            }
        }
    }
}
impl __ActivationReceiver for ForeignActivation {
    /// Acquires the input before publishing through the installed receiver.
    fn post(&mut self, message: NcMessage<'_, Activation>) -> Result<(), NcStatus> {
        message.to_owned()?.post_to(&mut self.destination)
    }
}
/// Creates one independently owned foreign producer, initially without a destination.
fn source(fail_once: bool) -> ComPtr<__ActivationReceiverAbi> {
    ForeignActivation::from_value(ForeignActivation { destination: None, fail_once })
        .query()
        .unwrap()
}
/// Uses the producer's generated output accessor, including its checked COM installation call.
fn output(source: &ComPtr<__ActivationReceiverAbi>) -> impl FnOnce(Box<dyn FnMut(Activation) -> Result<(), NcStatus>>) -> Result<(), NcStatus> + use<> {
    source.query::<IActivationEventsAbi>().unwrap().activated()
}
/// Lends an input string only for one fixture producer call.
fn emit(source: &ComPtr<__ActivationReceiverAbi>, label: &str) -> Result<(), NcStatus> {
    source.clone().post(NcMessage::new(&Activation { label: label.into() }))
}
/// Makes capture destruction observable independently of both producer and consumer ownership.
struct Lifetime(Rc<Cell<u32>>);
impl Drop for Lifetime {
    /// Counts the release caused by failed setup, replacement or producer destruction.
    fn drop(&mut self) {
        self.0.set(self.0.get() + 1);
    }
}
/// Rejects an invalid domain value before invoking application translation.
fn valid_label(event: &Activation) -> Result<(), NcStatus> {
    match event.label.is_empty() {
        true => Err(NC_E_INVALID_ARG),
        false => Ok(()),
    }
}

/// Failed setup releases captures; successful outputs preserve actual post order across producers.
#[test]
fn failure_validation_fifo_and_producer_lifetime() {
    let mut events = Events::new();
    let drops = Rc::new(Cell::new(0));
    let a = source(false);
    let b = source(true);
    let lifetime = Lifetime(drops.clone());
    assert_eq!(
        events.map(output(&b), move |event| {
            let _ = &lifetime;
            event.label
        }),
        Err(NC_E_INVALID_ARG)
    );
    assert_eq!(drops.get(), 1);
    let lifetime = Lifetime(drops.clone());
    events
        .map(output(&a), move |event| {
            let _ = &lifetime;
            event.label
        })
        .unwrap();
    events.map(checked(output(&b), valid_label), |event| event.label).unwrap();
    assert_eq!(emit(&b, ""), Err(NC_E_INVALID_ARG));
    assert!(events.is_empty());
    emit(&b, "second").unwrap();
    emit(&a, "first").unwrap();
    drop(a);
    assert_eq!(drops.get(), 2);
    assert_eq!(events.pop().as_deref(), Some("second"));
    assert_eq!(events.pop().as_deref(), Some("first"));
}

/// Remapping changes only future delivery, even between different consumer message types.
#[test]
fn replacement_preserves_queued_values_and_routing_ids() {
    let producer = source(false);
    let mut first = Events::new();
    let mut second = Events::new();
    first.map(output(&producer), |event| (10, event.label)).unwrap();
    emit(&producer, "old").unwrap();
    second.map(output(&producer), |event| event.label.len()).unwrap();
    emit(&producer, "replacement").unwrap();
    drop(producer);
    assert_eq!(first.pop(), Some((10, "old".into())));
    assert_eq!(second.pop(), Some(11));
    assert!(first.is_empty());
}

/// Consumer destruction makes retained receivers inert; producer replacement releases their captures.
#[test]
fn abandoned_consumer_does_not_run_its_mapper_or_stay_alive() {
    let producer = source(false);
    let mut events = Events::new();
    let calls = Rc::new(Cell::new(0));
    let observed = calls.clone();
    let drops = Rc::new(Cell::new(0));
    let lifetime = Lifetime(drops.clone());
    events
        .map(output(&producer), move |_| {
            let _ = &lifetime;
            observed.set(observed.get() + 1);
        })
        .unwrap();
    emit(&producer, "live").unwrap();
    assert_eq!(calls.get(), 1);
    drop(events);
    emit(&producer, "abandoned").unwrap();
    assert_eq!(calls.get(), 1);
    assert_eq!(drops.get(), 0, "producer owns the adapter until replacement or destruction");
    let mut replacement = Events::new();
    replacement.map(output(&producer), |_| ()).unwrap();
    assert_eq!(drops.get(), 1);
}

/// Mapping runs synchronously while ordinary consumer behavior waits for a dispatch boundary.
#[test]
fn translation_is_immediate_and_behavior_is_deferred() {
    let producer = source(false);
    let mut events = Events::new();
    let mapped = Rc::new(Cell::new(0));
    let observed = mapped.clone();
    events
        .map(output(&producer), move |event| {
            observed.set(observed.get() + 1);
            event.label
        })
        .unwrap();
    emit(&producer, "a").unwrap();
    emit(&producer, "b").unwrap();
    assert_eq!(mapped.get(), 2);
    let mut consumed = Vec::new();
    assert_eq!(
        events.dispatch(1, |message| {
            consumed.push(message);
            Ok(())
        }),
        Ok(1)
    );
    assert_eq!(consumed, ["a"]);
    assert_eq!(events.pop().as_deref(), Some("b"));
}

/// Decoder-owned snapshots still pass domain checks before any app value enters its FIFO.
#[test]
fn generated_receiver_checks_domain_values_before_mapping() {
    let mut numeric = Events::new();
    let mut receiver = None;
    numeric
        .map(
            checked(
                |consume| {
                    receiver = Some(NumericChanged::__receiver(consume)?);
                    Ok(())
                },
                valid_numeric,
            ),
            |event| event.value,
        )
        .unwrap();
    let receiver = receiver.as_mut().unwrap();
    assert_eq!(receiver.post(NcMessage::new(&NumericChanged { value: f32::NAN })), Err(NC_E_INVALID_ARG));
    receiver.post(NcMessage::new(&NumericChanged { value: 1.0 })).unwrap();
    assert_eq!(numeric.pop(), Some(1.0));
    let mut text = Events::new();
    let mut receiver = None;
    text.map(
        checked(
            |consume| {
                receiver = Some(TextChanged::__receiver(consume)?);
                Ok(())
            },
            valid_text_cursor,
        ),
        |event| event.text,
    )
    .unwrap();
    let receiver = receiver.as_mut().unwrap();
    assert_eq!(
        receiver.post(NcMessage::new(&TextChanged { text: "é".into(), cursor: 1 })),
        Err(NC_E_INVALID_ARG)
    );
    receiver.post(NcMessage::new(&TextChanged { text: "é".into(), cursor: 2 })).unwrap();
    assert_eq!(text.pop().as_deref(), Some("é"));
}

/// The existing COM mutable gate rejects recursive producer calls without a posting phase machine.
#[test]
fn ordinary_com_gate_rejects_recursive_emission() {
    let producer = source(false);
    let captured = producer.clone();
    let mut events = Events::new();
    events
        .map(output(&producer), move |event| {
            assert_eq!(emit(&captured, "recursive"), Err(neocogi_com::NC_E_BUSY));
            event.label
        })
        .unwrap();
    emit(&producer, "outer").unwrap();
    assert_eq!(events.pop().as_deref(), Some("outer"));
    // Release the deliberately self-capturing test mapper by replacing it.
    events.map(output(&producer), |event| event.label).unwrap();
}
