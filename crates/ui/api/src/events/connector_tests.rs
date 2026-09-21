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
    let (events_tx, events) = std::sync::mpsc::channel();
    let drops = Rc::new(Cell::new(0));
    let a = source(false);
    let b = source(true);
    let lifetime = Lifetime(drops.clone());
    assert_eq!(
        neocogi_com::events::map(output(&b), events_tx.clone(), move |event| {
            let _ = &lifetime;
            event.label
        }),
        Err(NC_E_INVALID_ARG)
    );
    assert_eq!(drops.get(), 1);
    let lifetime = Lifetime(drops.clone());
    neocogi_com::events::map(output(&a), events_tx.clone(), move |event| {
        let _ = &lifetime;
        event.label
    })
    .unwrap();
    neocogi_com::events::map(checked(output(&b), valid_label), events_tx.clone(), |event| event.label).unwrap();
    assert_eq!(emit(&b, ""), Err(NC_E_INVALID_ARG));
    assert!(events.try_recv().is_err());
    emit(&b, "second").unwrap();
    emit(&a, "first").unwrap();
    drop(a);
    assert_eq!(drops.get(), 2);
    assert_eq!(events.try_recv().ok().as_deref(), Some("second"));
    assert_eq!(events.try_recv().ok().as_deref(), Some("first"));
}

/// Remapping changes only future delivery, even between different consumer message types.
#[test]
fn replacement_preserves_queued_values_and_routing_ids() {
    let producer = source(false);
    let (first_tx, first) = std::sync::mpsc::channel();
    let (second_tx, second) = std::sync::mpsc::channel();
    neocogi_com::events::map(output(&producer), first_tx.clone(), |event| (10, event.label)).unwrap();
    emit(&producer, "old").unwrap();
    neocogi_com::events::map(output(&producer), second_tx.clone(), |event| event.label.len()).unwrap();
    emit(&producer, "replacement").unwrap();
    drop(producer);
    assert_eq!(first.try_recv().ok(), Some((10, "old".into())));
    assert_eq!(second.try_recv().ok(), Some(11));
    assert!(first.try_recv().is_err());
}

/// Consumer destruction discards translated messages; producer replacement releases mapper captures.
#[test]
fn abandoned_consumer_discards_delivery_and_mapper_lives_until_replacement() {
    let producer = source(false);
    let (events_tx, events) = std::sync::mpsc::channel();
    let calls = Rc::new(Cell::new(0));
    let observed = calls.clone();
    let drops = Rc::new(Cell::new(0));
    let lifetime = Lifetime(drops.clone());
    neocogi_com::events::map(output(&producer), events_tx.clone(), move |_| {
        let _ = &lifetime;
        observed.set(observed.get() + 1);
    })
    .unwrap();
    emit(&producer, "live").unwrap();
    assert_eq!(calls.get(), 1);
    drop(events);
    emit(&producer, "abandoned").unwrap();
    assert_eq!(calls.get(), 2, "standard send discovers disconnection after translation");
    assert_eq!(drops.get(), 0, "producer owns the adapter until replacement or destruction");
    let (replacement_tx, _replacement) = std::sync::mpsc::channel();
    neocogi_com::events::map(output(&producer), replacement_tx.clone(), |_| ()).unwrap();
    assert_eq!(drops.get(), 1);
}

/// Mapping runs synchronously while ordinary consumer behavior waits for a dispatch boundary.
#[test]
fn translation_is_immediate_and_behavior_is_deferred() {
    let producer = source(false);
    let (events_tx, events) = std::sync::mpsc::channel();
    let mapped = Rc::new(Cell::new(0));
    let observed = mapped.clone();
    neocogi_com::events::map(output(&producer), events_tx.clone(), move |event| {
        observed.set(observed.get() + 1);
        event.label
    })
    .unwrap();
    emit(&producer, "a").unwrap();
    emit(&producer, "b").unwrap();
    assert_eq!(mapped.get(), 2);
    let consumed = [events.try_recv().unwrap()];
    assert_eq!(consumed, ["a"]);
    assert_eq!(events.try_recv().ok().as_deref(), Some("b"));
}

/// Decoder-owned snapshots still pass domain checks before any app value enters its FIFO.
#[test]
fn generated_receiver_checks_domain_values_before_mapping() {
    let (numeric_tx, numeric) = std::sync::mpsc::channel();
    let mut receiver = None;
    neocogi_com::events::map(
        checked(
            |consume| {
                receiver = Some(NumericChanged::__receiver(consume)?);
                Ok(())
            },
            valid_numeric,
        ),
        numeric_tx.clone(),
        |event| event.value,
    )
    .unwrap();
    let receiver = receiver.as_mut().unwrap();
    assert_eq!(receiver.post(NcMessage::new(&NumericChanged { value: f32::NAN })), Err(NC_E_INVALID_ARG));
    receiver.post(NcMessage::new(&NumericChanged { value: 1.0 })).unwrap();
    assert_eq!(numeric.try_recv().ok(), Some(1.0));
    let (text_tx, text) = std::sync::mpsc::channel();
    let mut receiver = None;
    neocogi_com::events::map(
        checked(
            |consume| {
                receiver = Some(TextChanged::__receiver(consume)?);
                Ok(())
            },
            valid_text_cursor,
        ),
        text_tx.clone(),
        |event| event.text,
    )
    .unwrap();
    let receiver = receiver.as_mut().unwrap();
    assert_eq!(
        receiver.post(NcMessage::new(&TextChanged { text: "é".into(), cursor: 1 })),
        Err(NC_E_INVALID_ARG)
    );
    receiver.post(NcMessage::new(&TextChanged { text: "é".into(), cursor: 2 })).unwrap();
    assert_eq!(text.try_recv().ok().as_deref(), Some("é"));
}

/// The existing COM mutable gate rejects recursive producer calls without a posting phase machine.
#[test]
fn ordinary_com_gate_rejects_recursive_emission() {
    let producer = source(false);
    let captured = producer.clone();
    let (events_tx, events) = std::sync::mpsc::channel();
    neocogi_com::events::map(output(&producer), events_tx.clone(), move |event| {
        assert_eq!(emit(&captured, "recursive"), Err(neocogi_com::NC_E_BUSY));
        event.label
    })
    .unwrap();
    emit(&producer, "outer").unwrap();
    assert_eq!(events.try_recv().ok().as_deref(), Some("outer"));
    // Release the deliberately self-capturing test mapper by replacing it.
    neocogi_com::events::map(output(&producer), events_tx.clone(), |event| event.label).unwrap();
}
