//! Typed constructor receivers preserve ownership, validation and queue ordering.
use super::*;
use neocogi_com::NC_E_INVALID_ARG;
use neocogi_com::events::EventPort;
use std::{cell::Cell, rc::Rc};

/// Counts queue destruction without relying on channel implementation details.
struct Lifetime(Rc<Cell<u32>>);
impl Drop for Lifetime {
    fn drop(&mut self) {
        self.0.set(self.0.get() + 1);
    }
}

/// Independent producer ports share an application FIFO without installing observers later.
#[test]
fn construction_queue_preserves_fifo_and_owned_snapshots() {
    let ui = nc_ui_nodes::bind_builtin_nodes(runtime()).unwrap();
    let menu = ui.menus().unwrap().create("origins").unwrap();
    let origin = menu.item("source").unwrap();
    let (sender, messages) = std::sync::mpsc::channel();
    let mut first = EventPort::<IMenuItemAbi>::with_queue(
        Some(neocogi_ui::test_event_queue!(activation, String, sender.clone(), |_queue, _origin, event| event.label).unwrap()),
        origin.as_interface().downgrade().unwrap(),
    );
    let mut second = EventPort::<IMenuItemAbi>::with_queue(
        Some(neocogi_ui::test_event_queue!(activation, String, sender, |_queue, _origin, event| event.label).unwrap()),
        menu.item("second").unwrap().as_interface().downgrade().unwrap(),
    );
    second.post(Activation { label: "second".into() }).unwrap();
    first.post(Activation { label: "first".into() }).unwrap();
    drop(first);
    drop(second);
    assert_eq!(messages.try_iter().collect::<Vec<_>>(), ["second", "first"]);
}

/// Logical closure releases the captured parent even while the producer remains retained.
#[test]
fn close_releases_queue_once_and_preserves_already_queued_facts() {
    let ui = nc_ui_nodes::bind_builtin_nodes(runtime()).unwrap();
    let menu = ui.menus().unwrap().create("origins").unwrap();
    let origin = menu.item("source").unwrap();
    let (sender, messages) = std::sync::mpsc::channel();
    let drops = Rc::new(Cell::new(0));
    let capture = Lifetime(drops.clone());
    let sink = neocogi_ui::test_event_queue!(activation, String, sender,
        |_queue, _origin, event| event.label, { _lifetime: Lifetime = capture })
    .unwrap();
    let mut port = EventPort::<IMenuItemAbi>::with_queue(Some(sink), origin.as_interface().downgrade().unwrap());
    port.post(Activation { label: "before close".into() }).unwrap();
    assert_eq!(drops.get(), 0);
    port.close();
    port.close();
    assert_eq!(drops.get(), 1);
    assert_eq!(port.post(Activation { label: "after close".into() }), Err(neocogi_com::NC_E_CLOSED));
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
    let sink = neocogi_ui::test_event_queue!(activation, String, sender,
        |queue, _origin, event| { queue.calls.set(queue.calls.get() + 1); event.label },
        { calls: Rc<Cell<u32>> = observed })
    .unwrap();
    let mut port = EventPort::<IMenuItemAbi>::with_queue(Some(sink), origin.as_interface().downgrade().unwrap());
    port.post(Activation { label: "first".into() }).unwrap();
    assert_eq!(calls.get(), 1);
    assert_eq!(messages.try_recv().unwrap(), "first");
    drop(messages);
    port.post(Activation { label: "abandoned".into() }).unwrap();
    assert_eq!(calls.get(), 2);
    port.close();
}

/// Permanently unobserved ports need no receiver allocation but still honor terminal closure.
#[test]
fn unobserved_port_has_no_delivery_and_cannot_emit_after_close() {
    let mut port = EventPort::<IMenuItemAbi>::new();
    port.post(Activation { label: "ignored".into() }).unwrap();
    port.close();
    assert_eq!(port.post(Activation { label: "closed".into() }), Err(neocogi_com::NC_E_CLOSED));
}

/// The generated aggregate sink preserves facade domain checks before queue.
#[test]
fn constructor_queues_reject_invalid_payloads_before_recording() {
    let ui = nc_ui_nodes::bind_builtin_nodes(runtime()).unwrap();
    let origin = ui.widgets().textbox_unobserved("").unwrap();
    let (sender, receiver) = std::sync::mpsc::channel();
    let mut sink = neocogi_ui::test_event_queue!(textbox, neocogi_ui::textbox::Event, sender, |_queue, _origin, event| event).unwrap();
    use neocogi_com::events::{EventOrigin, EventPayload};
    let invalid = neocogi_ui::textbox::Event::Changed(TextChanged { text: "é".into(), cursor: 1 });
    assert_eq!(invalid.into_event().unwrap_err(), NC_E_INVALID_ARG);
    assert!(receiver.try_recv().is_err());
    let origin = EventOrigin::Widget(origin.as_interface().downgrade().into_unknown());
    for value in [
        neocogi_ui::textbox::Event::Changed(TextChanged { text: "é".into(), cursor: 2 }),
        neocogi_ui::textbox::Event::Submitted(TextSubmitted { text: "é".into() }),
    ] {
        sink.post(neocogi_com::NcValueRef::new(&origin).unwrap(), value.into_event().unwrap()).unwrap();
    }
    assert!(matches!(receiver.try_recv().unwrap(), neocogi_ui::textbox::Event::Changed(TextChanged { cursor: 2, .. })));
    assert!(matches!(receiver.try_recv().unwrap(), neocogi_ui::textbox::Event::Submitted(TextSubmitted { text }) if text == "é"));
    assert_eq!(neocogi_ui::number::Event::Changed(NumericChanged { value: f32::INFINITY }).into_event().unwrap_err(), NC_E_INVALID_ARG);
}

#[neocogi_com::com_object(interfaces(neocogi_com::events::IEventReceiver))]
/// Queue owns immutable event boxes and never enters the emitting widget.
struct AppQueue {
    /// Posting access holds no application model or strong widget reference.
    sender: std::sync::mpsc::Sender<neocogi_com::events::EventMessage>,
}
impl neocogi_com::events::IEventReceiver for AppQueue {
    /// Confirms the source gate stays closed while acquiring a non-owning identity.
    fn post(&mut self, origin: neocogi_com::NcValueRef<'_, neocogi_com::events::EventOrigin>, event: neocogi_com::ComBox<neocogi_com::events::IEventAbi>) -> Result<(), (NcStatus, neocogi_com::ComBox<neocogi_com::events::IEventAbi>)> {
        let message = neocogi_com::events::EventMessage::capture(origin, event)?;
        if let Some(origin) = message.origin.widget::<ButtonAbi>().unwrap() {
            assert_eq!(origin.label().err(), Some(neocogi_com::NC_E_BUSY));
        }
        self.sender.send(message).map_err(|error| (neocogi_com::NC_E_CLOSED, error.0.event))
    }
}

/// Real routed input proves shared allocation, origin identity, captured ordering,
/// independent snapshot lifetime, and owner destruction despite retained weak observations.
#[test]
fn one_application_queue_handles_two_widget_types_without_retaining_origins() {
    let ui = nc_ui_nodes::bind_builtin_nodes(runtime()).unwrap();
    let (sender, messages) = std::sync::mpsc::channel();
    let queue = AppQueue::from_value(AppQueue { sender });
    let buttons = queue.query::<neocogi_com::events::IEventReceiverAbi>().unwrap();
    let texts = queue.query::<neocogi_com::events::IEventReceiverAbi>().unwrap();
    assert_eq!(buttons.as_raw().object, texts.as_raw().object);
    let button = ui.widgets().button_with_queue("captured", Some(buttons.as_com_ref())).unwrap();
    let textbox = ui.widgets().textbox_with_queue("", Some(texts.as_com_ref())).unwrap();
    let button_origin = button.as_interface().downgrade();
    let text_origin = textbox.as_interface().downgrade();
    let mut column = ui.widgets().linear(FlowDirection::TopToBottom, CrossSize::Stretch).unwrap();
    column.insert(0, button, LinearEdge::new(LayoutTrack::Content)).unwrap();
    column.insert(1, textbox, LinearEdge::new(LayoutTrack::Content)).unwrap();
    let window = ui.window("shared queue", RECT, column).unwrap();
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
    let retained = button_origin.clone();
    assert_eq!(retained.label().err(), Some(neocogi_com::NC_E_DROPPED));
    drop((retained, window, buttons, texts, queue));
    assert!(!button_origin.is_alive());
    assert!(!text_origin.is_alive());
    let button_message = messages.try_recv().unwrap();
    let (origin, neocogi_ui_nodes_contracts::ButtonEvent::Activated(event)) = button_message.widget::<ButtonAbi>().unwrap().unwrap();
    assert!(origin.same_object(&button_origin));
    assert!(!origin.is_alive());
    assert_eq!(event.label, "captured");
    assert!(button_message.downcast::<neocogi_ui::ITextChangedEventAbi>().unwrap().is_none());
    let text_message = messages.try_recv().unwrap();
    let (origin, neocogi_ui_nodes_contracts::TextboxEvent::Changed(event)) = text_message.widget::<TextboxAbi>().unwrap().unwrap() else { panic!("textbox should enqueue second") };
    assert!(origin.same_object(&text_origin));
    assert!(!origin.is_alive());
    assert_eq!(event.text, "owned snapshot");
    assert!(messages.try_recv().is_err());
}

/// Composite implementation children must never escape as the public event origin.
/// The weak message remains readable after closing and dropping the whole composite.
#[test]
fn composite_queues_report_outer_origins_without_retaining_their_children() {
    let ui = nc_ui_nodes::bind_builtin_nodes(runtime()).unwrap();
    let (send, messages) = std::sync::mpsc::channel();
    let queue = neocogi_ui::test_event_queue!(
        text_area,
        (neocogi_com::WeakComBox<TextAreaAbi>, neocogi_ui::text_area::Event),
        send,
        |_queue, origin, event| (origin.downgrade_box()?, event)
    )
    .unwrap();
    let area = ui
        .widgets()
        .text_area_with_queue("", TextWrapping::Word, true, Some(queue.as_com_ref()))
        .unwrap();
    let origin = area.as_interface().downgrade();
    let window = ui.window("area origin", RECT, area).unwrap();
    key_for_origin_test(&ui, KeyCode::Tab);
    ui.input(UiInput::Text, "outer text").unwrap();
    ui.update(SIZE).unwrap();
    window.close().unwrap();
    drop((window, queue));
    let (captured, event) = messages.try_recv().unwrap();
    assert!(captured.same_object(&origin));
    assert!(!captured.is_alive());
    assert!(matches!(event, TextAreaEvent::Changed(TextChanged { text, .. }) if text == "outer text"));

    let root = ui.widgets().linear(FlowDirection::TopToBottom, CrossSize::Stretch).unwrap();
    let root_owner = root;
    let mut root = root_owner.handle();
    let window = ui.window("combo origin", RECT, root_owner).unwrap();
    let (send, messages) = std::sync::mpsc::channel();
    let queue = neocogi_ui::test_event_queue!(
        combo,
        (neocogi_com::WeakComBox<ComboAbi>, neocogi_ui::combo::Event),
        send,
        |_queue, origin, event| (origin.downgrade_box()?, event)
    )
    .unwrap();
    let combo = ui
        .combo_box_with_queue(
            window.as_com_ref(),
            &["first", "second"],
            ChoiceSelection::Selected(0),
            Some(queue.as_com_ref()),
        )
        .unwrap();
    let origin = combo.as_interface().downgrade();
    root.insert(0, combo, LinearEdge::new(LayoutTrack::Content)).unwrap();
    for key in [KeyCode::Tab, KeyCode::Enter, KeyCode::ArrowDown, KeyCode::Enter] {
        key_for_origin_test(&ui, key);
    }
    ui.update(SIZE).unwrap();
    window.close().unwrap();
    drop((window, root, queue));
    let (captured, event) = messages.try_recv().unwrap();
    assert!(captured.same_object(&origin));
    assert!(!captured.is_alive());
    let neocogi_ui::combo::Event::Changed(selection) = event;
    assert_eq!(selection, ChoiceSelection::Selected(1));
}

/// Sends an ordinary key to the production runtime without directly invoking a port.
fn key_for_origin_test(ui: &neocogi_ui::Ui, key: KeyCode) {
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

/// A retained port distinguishes an expired COM origin from deliberate service closure.
#[test]
fn expired_origin_and_explicit_port_close_have_distinct_statuses() {
    let ui = nc_ui_nodes::bind_builtin_nodes(runtime()).unwrap();
    let menu = ui.menus().unwrap().create("short-lived producer").unwrap();
    let origin = menu.item("origin").unwrap();
    let (send, messages) = std::sync::mpsc::channel();
    let queue = neocogi_ui::test_event_queue!(activation, String, send, |_queue, _origin, event| event.label).unwrap();
    let mut port = EventPort::<IMenuItemAbi>::with_queue(Some(queue), origin.as_interface().downgrade().unwrap());
    // The unmounted declaration owns the other strong origin reference.
    // Drop both owners while leaving this separate port and its weak origin alive.
    drop(origin);
    drop(menu);
    let event = Activation { label: "expired origin".into() };
    assert_eq!(port.post(event.clone()), Err(neocogi_com::NC_E_DROPPED));
    assert!(matches!(messages.try_recv(), Err(std::sync::mpsc::TryRecvError::Empty)));
    port.close();
    assert_eq!(port.post(event.clone()), Err(neocogi_com::NC_E_CLOSED));
    assert!(matches!(messages.try_recv(), Err(std::sync::mpsc::TryRecvError::Disconnected)));
}
