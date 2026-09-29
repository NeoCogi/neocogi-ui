//! Typed constructor receivers preserve ownership, validation and queue ordering.
use super::*;
use neocogi_ui::events::*;
use neocogi_com::NC_E_INVALID_ARG;
use neocogi_com::events::EventPort;
use neocogi_ui_core_contracts::IMenuItemEventQueueAbi;
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
    let ui = nc_ui_nodes::bind_builtin_nodes(runtime()).unwrap();
    let menu = ui.menus().unwrap().create("origins").unwrap();
    let origin = menu.item("source").unwrap();
    let (sender, messages) = std::sync::mpsc::channel();
    let mut first = EventPort::<IMenuItemEventQueueAbi>::with_queue(
        Some(activation::queue(sender.clone(), |event| event.label).unwrap()),
        origin.as_interface().downgrade().unwrap(),
    );
    let mut second = EventPort::<IMenuItemEventQueueAbi>::with_queue(
        Some(activation::queue(sender, |event| event.label).unwrap()),
        menu.item("second").unwrap().as_interface().downgrade().unwrap(),
    );
    second.post(&Activation { label: "second".into() }).unwrap();
    first.post(&Activation { label: "first".into() }).unwrap();
    drop(first);
    drop(second);
    assert_eq!(messages.try_iter().collect::<Vec<_>>(), ["second", "first"]);
}

/// Logical closure releases the captured parent even while the producer remains retained.
#[test]
fn close_releases_mapping_once_and_preserves_already_queued_facts() {
    let ui = nc_ui_nodes::bind_builtin_nodes(runtime()).unwrap();
    let menu = ui.menus().unwrap().create("origins").unwrap();
    let origin = menu.item("source").unwrap();
    let (sender, messages) = std::sync::mpsc::channel();
    let drops = Rc::new(Cell::new(0));
    let capture = Lifetime(drops.clone());
    let sink = activation::queue(sender, move |event| {
        let _ = &capture;
        event.label
    })
    .unwrap();
    let mut port = EventPort::<IMenuItemEventQueueAbi>::with_queue(Some(sink), origin.as_interface().downgrade().unwrap());
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
    let ui = nc_ui_nodes::bind_builtin_nodes(runtime()).unwrap();
    let menu = ui.menus().unwrap().create("origins").unwrap();
    let origin = menu.item("source").unwrap();
    let (sender, messages) = std::sync::mpsc::channel();
    let calls = Rc::new(Cell::new(0));
    let observed = calls.clone();
    let sink = activation::queue(sender, move |event| {
        observed.set(observed.get() + 1);
        event.label
    })
    .unwrap();
    let mut port = EventPort::<IMenuItemEventQueueAbi>::with_queue(Some(sink), origin.as_interface().downgrade().unwrap());
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
    let mut port = EventPort::<IMenuItemEventQueueAbi>::new();
    port.post(&Activation { label: "ignored".into() }).unwrap();
    port.close();
    assert_eq!(port.post(&Activation { label: "closed".into() }), Err(neocogi_com::NC_E_CLOSED));
}

/// The generated aggregate sink preserves facade domain checks before mapping.
#[test]
fn constructor_queues_reject_invalid_payloads_before_the_mapper_runs() {
    let ui = nc_ui_nodes::bind_builtin_nodes(runtime()).unwrap();
    let origin = ui.widgets().textbox_unobserved("").unwrap();
    let (sender, receiver) = std::sync::mpsc::channel();
    let mut sink = neocogi_ui::textbox::queue(sender, |event| event).unwrap();
    let invalid = neocogi_ui::textbox::Event::Changed(TextChanged { text: "é".into(), cursor: 1 });
    assert_eq!(
        sink.post(origin.as_interface(), neocogi_com::NcValueRef::new(&invalid).unwrap()),
        Err(NC_E_INVALID_ARG)
    );
    assert!(receiver.try_recv().is_err());
    let valid = neocogi_ui::textbox::Event::Changed(TextChanged { text: "é".into(), cursor: 2 });
    sink.post(origin.as_interface(), neocogi_com::NcValueRef::new(&valid).unwrap()).unwrap();
    sink.post(
        origin.as_interface(),
        neocogi_com::NcValueRef::new(&neocogi_ui::textbox::Event::Submitted(TextSubmitted { text: "é".into() })).unwrap(),
    )
    .unwrap();
    assert!(matches!(
        receiver.try_recv().unwrap(),
        neocogi_ui::textbox::Event::Changed(TextChanged { cursor: 2, .. })
    ));
    assert!(matches!(receiver.try_recv().unwrap(), neocogi_ui::textbox::Event::Submitted(TextSubmitted { text }) if text == "é"));

    let (sender, receiver) = std::sync::mpsc::channel();
    let origin = ui
        .widgets()
        .number_unobserved(neocogi_ui::NumberOptions { value: 0.0, step: 1.0, precision: 0 })
        .unwrap();
    let mut sink = neocogi_ui::number::queue(sender, |_| ()).unwrap();
    assert_eq!(
        sink.post(
            origin.as_interface(),
            neocogi_com::NcValueRef::new(&neocogi_ui::number::Event::Changed(NumericChanged { value: f32::INFINITY })).unwrap()
        ),
        Err(NC_E_INVALID_ARG)
    );
    assert!(receiver.try_recv().is_err());
}

/// Application-owned messages retain snapshots and weak producer identities only.
enum AppMessage {
    /// Complete button stream; the origin does not keep a removed button alive.
    Button(neocogi_com::WeakComPtr<ButtonAbi>, neocogi_ui_nodes_contracts::ButtonEvent),
    /// Complete textbox stream, delivered through the same queue allocation.
    Textbox(neocogi_com::WeakComPtr<TextboxAbi>, neocogi_ui_nodes_contracts::TextboxEvent),
}

#[neocogi_com::com_object(interfaces(neocogi_ui_nodes_contracts::ButtonEventQueue, neocogi_ui_nodes_contracts::TextboxEventQueue))]
/// One concrete application ABI object implements two closed receiver contracts.
struct AppQueue {
    /// The sole captured application resource; no parent model or widget is retained.
    sender: std::sync::mpsc::Sender<AppMessage>,
}
impl neocogi_ui_nodes_contracts::ButtonEventQueue for AppQueue {
    /// Acquires data and identity synchronously; behavior remains in the consumer.
    fn post(
        &mut self,
        origin: neocogi_com::ComRef<'_, ButtonAbi>,
        event: neocogi_com::NcValueRef<'_, neocogi_ui_nodes_contracts::ButtonEvent>,
    ) -> Result<(), NcStatus> {
        // Posting happens while the emitter owns its exclusive call gate. Even
        // observations must wait for dispatch; weak acquisition touches no state.
        assert_eq!(origin.label().err(), Some(neocogi_com::NC_E_BUSY));
        let _ = self.sender.send(AppMessage::Button(origin.downgrade()?, event.to_owned()?));
        Ok(())
    }
}
impl neocogi_ui_nodes_contracts::TextboxEventQueue for AppQueue {
    /// Uses ordinary mutable queue ownership with the textbox's exact ABI type.
    fn post(
        &mut self,
        origin: neocogi_com::ComRef<'_, TextboxAbi>,
        event: neocogi_com::NcValueRef<'_, neocogi_ui_nodes_contracts::TextboxEvent>,
    ) -> Result<(), NcStatus> {
        let event = event.to_owned()?;
        neocogi_ui::textbox::validate(&event)?;
        let _ = self.sender.send(AppMessage::Textbox(origin.downgrade()?, event));
        Ok(())
    }
}

/// Real routed input proves shared allocation, origin identity, captured ordering,
/// independent snapshot lifetime, and destruction versus retained logical closure.
#[test]
fn one_application_queue_handles_two_widget_types_without_retaining_origins() {
    let ui = nc_ui_nodes::bind_builtin_nodes(runtime()).unwrap();
    let (sender, messages) = std::sync::mpsc::channel();
    let queue = AppQueue::from_value(AppQueue { sender });
    let buttons = queue.query::<ButtonEventQueueAbi>().unwrap();
    let texts = queue.query::<TextboxEventQueueAbi>().unwrap();
    assert_eq!(buttons.as_raw().object, texts.as_raw().object);
    let button = ui.widgets().button_with_queue("captured", Some(buttons.as_com_ref())).unwrap();
    let textbox = ui.widgets().textbox_with_queue("", Some(texts.as_com_ref())).unwrap();
    let button_origin = button.as_interface().downgrade().unwrap();
    let text_origin = textbox.as_interface().downgrade().unwrap();
    let mut column = ui.widgets().linear(FlowDirection::TopToBottom, CrossSize::Stretch).unwrap();
    column.insert(0, &button, LinearEdge::new(LayoutTrack::Content)).unwrap().finish().unwrap();
    column.insert(1, &textbox, LinearEdge::new(LayoutTrack::Content)).unwrap().finish().unwrap();
    let window = ui.window("shared queue", RECT, &column).unwrap();
    for key in [KeyCode::Tab, KeyCode::Space, KeyCode::Tab] {
        ui.input(
            UiInput::Key(KeyStroke {
                key,
                value: 0,
                pressed: true,
                repeat: false,
                modifiers: 0,
            }),
            "",
        )
        .unwrap();
    }
    ui.input(UiInput::Text, "owned snapshot").unwrap();
    ui.update(SIZE).unwrap();
    window.close().unwrap();
    let retained = button_origin.upgrade().unwrap().unwrap();
    assert_eq!(retained.label().err(), Some(UI_E_CLOSED));
    drop((retained, button, textbox, column, window, buttons, texts, queue));
    assert!(button_origin.upgrade().unwrap().is_none());
    assert!(text_origin.upgrade().unwrap().is_none());
    match messages.try_recv().unwrap() {
        AppMessage::Button(origin, neocogi_ui_nodes_contracts::ButtonEvent::Activated(event)) => {
            assert!(origin.same_object(&button_origin));
            assert!(origin.upgrade().unwrap().is_none());
            assert_eq!(event.label, "captured");
        }
        _ => panic!("button should enqueue first"),
    }
    match messages.try_recv().unwrap() {
        AppMessage::Textbox(origin, neocogi_ui_nodes_contracts::TextboxEvent::Changed(event)) => {
            assert!(origin.same_object(&text_origin));
            assert!(origin.upgrade().unwrap().is_none());
            assert_eq!(event.text, "owned snapshot");
        }
        _ => panic!("textbox should enqueue second"),
    }
    assert!(messages.try_recv().is_err());
}
