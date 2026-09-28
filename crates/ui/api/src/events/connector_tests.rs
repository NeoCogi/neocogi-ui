//! Typed constructor receivers preserve ownership, validation and queue ordering.
use super::*;
use neocogi_com::events::EventPort;
use neocogi_ui_core_contracts::IMenuItemSinkAbi;
use std::{cell::Cell, rc::Rc};

/// Counts mapping destruction without relying on channel implementation details.
struct Lifetime(Rc<Cell<u32>>);
impl Drop for Lifetime {
    fn drop(&mut self) {
        self.0.set(self.0.get() + 1);
    }
}

/// Independent producer ports share an application FIFO without installing observers later.
#[test]
fn construction_mapping_preserves_fifo_and_owned_snapshots() {
    let (sender, messages) = std::sync::mpsc::channel();
    let mut first = EventPort::<IMenuItemSinkAbi>::with_sink(Some(activation::sink(sender.clone(), |event| event.label).unwrap()));
    let mut second = EventPort::<IMenuItemSinkAbi>::with_sink(Some(activation::sink(sender, |event| event.label).unwrap()));
    second.post(&Activation { label: "second".into() }).unwrap();
    first.post(&Activation { label: "first".into() }).unwrap();
    drop(first);
    drop(second);
    assert_eq!(messages.try_iter().collect::<Vec<_>>(), ["second", "first"]);
}

/// Logical closure releases the captured parent even while the producer remains retained.
#[test]
fn close_releases_mapping_once_and_preserves_already_queued_facts() {
    let (sender, messages) = std::sync::mpsc::channel();
    let drops = Rc::new(Cell::new(0));
    let capture = Lifetime(drops.clone());
    let sink = activation::sink(sender, move |event| {
        let _ = &capture;
        event.label
    })
    .unwrap();
    let mut port = EventPort::<IMenuItemSinkAbi>::with_sink(Some(sink));
    port.post(&Activation { label: "before close".into() }).unwrap();
    assert_eq!(drops.get(), 0);
    port.close();
    port.close();
    assert_eq!(drops.get(), 1);
    assert_eq!(port.post(&Activation { label: "after close".into() }), Err(neocogi_com::NC_E_CLOSED));
    assert_eq!(messages.try_recv().unwrap(), "before close");
    drop(port);
    assert_eq!(drops.get(), 1);
}

/// Mapping is synchronous, application behavior is queued, and an abandoned inbox is harmless.
#[test]
fn translation_precedes_dispatch_and_disconnection_keeps_cleanup_deterministic() {
    let (sender, messages) = std::sync::mpsc::channel();
    let calls = Rc::new(Cell::new(0));
    let observed = calls.clone();
    let sink = activation::sink(sender, move |event| {
        observed.set(observed.get() + 1);
        event.label
    })
    .unwrap();
    let mut port = EventPort::<IMenuItemSinkAbi>::with_sink(Some(sink));
    port.post(&Activation { label: "first".into() }).unwrap();
    assert_eq!(calls.get(), 1);
    assert_eq!(messages.try_recv().unwrap(), "first");
    drop(messages);
    port.post(&Activation { label: "abandoned".into() }).unwrap();
    assert_eq!(calls.get(), 2);
    port.close();
}

/// Permanently unobserved ports need no receiver allocation but still honor terminal closure.
#[test]
fn unobserved_port_has_no_delivery_and_cannot_emit_after_close() {
    let mut port = EventPort::<IMenuItemSinkAbi>::new();
    port.post(&Activation { label: "ignored".into() }).unwrap();
    port.close();
    assert_eq!(port.post(&Activation { label: "closed".into() }), Err(neocogi_com::NC_E_CLOSED));
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
    sink.post(neocogi_com::NcValueRef::new(&super::textbox::Event::Submitted(super::TextSubmitted { text: "é".into() })).unwrap())
        .unwrap();
    assert!(matches!(
        receiver.try_recv().unwrap(),
        super::textbox::Event::Changed(super::TextChanged { cursor: 2, .. })
    ));
    assert!(matches!(receiver.try_recv().unwrap(), super::textbox::Event::Submitted(super::TextSubmitted { text }) if text == "é"));

    let (sender, receiver) = std::sync::mpsc::channel();
    let mut sink = super::number::sink(sender, |_| ()).unwrap();
    assert_eq!(
        sink.post(neocogi_com::NcValueRef::new(&super::number::Event::Changed(NumericChanged { value: f32::INFINITY })).unwrap()),
        Err(NC_E_INVALID_ARG)
    );
    assert!(receiver.try_recv().is_err());
}
