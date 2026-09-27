//! Concrete foreign producers exercise direct replacement, ownership and deferred behavior.
use super::*;
use neocogi_com::{com_object, NcValueRef};
use std::{cell::Cell, rc::Rc};

#[com_object(interfaces(IActivationEvents, IActivationEventsSink))]
/// Foreign producer uses only ordinary fields and mutable COM methods.
struct ForeignActivation {
    /// The complete delivery storage: one optional typed destination.
    destination: IActivationEventsEventPort,
    /// One failed setup proves mapper captures are released on installation errors.
    fail_once: bool,
}
impl IActivationEvents for ForeignActivation {}
impl IActivationEventsStream for ForeignActivation {
    /// Failed setup leaves the old destination intact; successful setup replaces it.
    fn attach(&mut self, destination: ComRef<'_, IActivationEventsSinkAbi>) -> Result<(), NcStatus> {
        match std::mem::replace(&mut self.fail_once, false) {
            true => Err(NC_E_INVALID_ARG),
            false => self.destination.attach(destination),
        }
    }
}
impl IActivationEventsSink for ForeignActivation {
    /// Acquires the input before publishing through the installed receiver.
    fn post(&mut self, message: NcValueRef<'_, Activation>) -> Result<(), NcStatus> {
        self.destination.post(&message.to_owned()?)
    }
}
/// Creates one independently owned foreign producer, initially without a destination.
fn source(fail_once: bool) -> neocogi_com::LocalCom<ForeignActivation> {
    ForeignActivation::into_local(ForeignActivation {
        destination: Default::default(),
        fail_once,
    })
}
/// Uses the producer's generated output accessor, including its checked COM installation call.
fn output(
    source: &neocogi_com::LocalCom<ForeignActivation>,
) -> impl FnOnce(Box<dyn FnMut(Activation) -> Result<(), NcStatus>>) -> Result<(), NcStatus> + use<> {
    install_stream(source.query::<IActivationEventsAbi>().unwrap())
}
/// Lends an input string only for one fixture producer call.
fn emit(source: &neocogi_com::LocalCom<ForeignActivation>, label: &str) -> Result<(), NcStatus> {
    source
        .query::<IActivationEventsSinkAbi>()?
        .post(NcValueRef::new(&Activation { label: label.into() }).unwrap())
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
        neocogi_com::events::bind(output(&b), events_tx.clone(), move |event| {
            let _ = &lifetime;
            event.label
        }),
        Err(NC_E_INVALID_ARG)
    );
    assert_eq!(drops.get(), 1);
    let lifetime = Lifetime(drops.clone());
    neocogi_com::events::bind(output(&a), events_tx.clone(), move |event| {
        let _ = &lifetime;
        event.label
    })
    .unwrap();
    neocogi_com::events::bind(checked(output(&b), valid_label), events_tx.clone(), |event| event.label).unwrap();
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
fn rejected_replacement_preserves_queued_values_and_original_consumer() {
    let producer = source(false);
    let (first_tx, first) = std::sync::mpsc::channel();
    let (second_tx, second) = std::sync::mpsc::channel();
    neocogi_com::events::bind(output(&producer), first_tx.clone(), |event| (10, event.label)).unwrap();
    emit(&producer, "old").unwrap();
    assert_eq!(
        neocogi_com::events::bind(output(&producer), second_tx.clone(), |event| event.label.len()),
        Err(neocogi_com::NC_E_ALREADY_BOUND)
    );
    emit(&producer, "replacement").unwrap();
    drop(producer);
    assert_eq!(first.try_recv().ok(), Some((10, "old".into())));
    assert!(second.try_recv().is_err());
    assert_eq!(first.try_recv().ok(), Some((10, "replacement".into())));
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
    neocogi_com::events::bind(output(&producer), events_tx.clone(), move |_| {
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
    assert_eq!(
        neocogi_com::events::bind(output(&producer), replacement_tx.clone(), |_| ()),
        Err(neocogi_com::NC_E_ALREADY_BOUND)
    );
    assert_eq!(drops.get(), 0);
    producer.try_borrow_mut().unwrap().destination.close();
    assert_eq!(drops.get(), 1);
}

/// Mapping runs synchronously while ordinary consumer behavior waits for a dispatch boundary.
#[test]
fn translation_is_immediate_and_behavior_is_deferred() {
    let producer = source(false);
    let (events_tx, events) = std::sync::mpsc::channel();
    let mapped = Rc::new(Cell::new(0));
    let observed = mapped.clone();
    neocogi_com::events::bind(output(&producer), events_tx.clone(), move |event| {
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
    neocogi_com::events::bind(
        checked(
            |mut consume| {
                receiver = Some(<neocogi_ui_nodes_contracts::NumberSinkAbi as neocogi_com::events::MessageSinkInterface>::receiver(move |number::Event::Changed(event)| consume(event))?);
                Ok(())
            },
            valid_numeric,
        ),
        numeric_tx.clone(),
        |event| event.value,
    )
    .unwrap();
    let receiver = receiver.as_mut().unwrap();
    assert_eq!(receiver.post(NcValueRef::new(&number::Event::Changed(NumericChanged { value: f32::NAN })).unwrap()), Err(NC_E_INVALID_ARG));
    receiver.post(NcValueRef::new(&number::Event::Changed(NumericChanged { value: 1.0 })).unwrap()).unwrap();
    assert_eq!(numeric.try_recv().ok(), Some(1.0));
    let (text_tx, text) = std::sync::mpsc::channel();
    let mut receiver = None;
    neocogi_com::events::bind(
        checked(
            |mut consume| {
                receiver = Some(<neocogi_ui_nodes_contracts::TextboxSinkAbi as neocogi_com::events::MessageSinkInterface>::receiver(move |event| match event { textbox::Event::Changed(event) => consume(event), textbox::Event::Submitted(_) => Ok(()) })?);
                Ok(())
            },
            |event: &TextChanged| valid_text_cursor(&event.text, event.cursor),
        ),
        text_tx.clone(),
        |event| event.text,
    )
    .unwrap();
    let receiver = receiver.as_mut().unwrap();
    assert_eq!(
        receiver.post(NcValueRef::new(&textbox::Event::Changed(TextChanged { text: "é".into(), cursor: 1 })).unwrap()),
        Err(NC_E_INVALID_ARG)
    );
    receiver.post(NcValueRef::new(&textbox::Event::Changed(TextChanged { text: "é".into(), cursor: 2 })).unwrap()).unwrap();
    assert_eq!(text.try_recv().ok().as_deref(), Some("é"));
}

/// The existing COM mutable gate rejects recursive producer calls without a posting phase machine.
#[test]
fn ordinary_com_gate_rejects_recursive_emission() {
    let producer = source(false);
    let captured = producer.clone();
    let (events_tx, events) = std::sync::mpsc::channel();
    neocogi_com::events::bind(output(&producer), events_tx.clone(), move |event| {
        assert_eq!(emit(&captured, "recursive"), Err(neocogi_com::NC_E_BUSY));
        event.label
    })
    .unwrap();
    emit(&producer, "outer").unwrap();
    assert_eq!(events.try_recv().ok().as_deref(), Some("outer"));
    // Close the fixture producer to release its deliberately self-capturing mapper.
    producer.try_borrow_mut().unwrap().destination.close();
}
/// The generated aggregate sink preserves facade domain checks before mapping.
#[test]
fn constructor_sinks_reject_invalid_payloads_before_the_mapper_runs() {
    let (sender, receiver) = std::sync::mpsc::channel();
    let mut sink = super::textbox::sink(sender, |event| event).unwrap();
    let invalid = super::textbox::Event::Changed(super::TextChanged { text: "é".into(), cursor: 1 });
    assert_eq!(sink.post(neocogi_com::NcValueRef::new(&invalid).unwrap()), Err(NC_E_INVALID_ARG));
    assert!(receiver.try_recv().is_err());
    let valid = super::textbox::Event::Changed(super::TextChanged { text: "é".into(), cursor: 2 });
    sink.post(neocogi_com::NcValueRef::new(&valid).unwrap()).unwrap();
    sink.post(neocogi_com::NcValueRef::new(&super::textbox::Event::Submitted(super::TextSubmitted {
        text: "é".into(),
    })).unwrap())
    .unwrap();
    assert!(matches!(
        receiver.try_recv().unwrap(),
        super::textbox::Event::Changed(super::TextChanged { cursor: 2, .. })
    ));
    assert!(matches!(receiver.try_recv().unwrap(), super::textbox::Event::Submitted(super::TextSubmitted { text }) if text == "é"));

    let (sender, receiver) = std::sync::mpsc::channel();
    let mut sink = super::number::sink(sender, |_| ()).unwrap();
    assert_eq!(
        sink.post(neocogi_com::NcValueRef::new(&super::number::Event::Changed(NumericChanged {
            value: f32::INFINITY
        })).unwrap()),
        Err(NC_E_INVALID_ARG)
    );
    assert!(receiver.try_recv().is_err());
}
