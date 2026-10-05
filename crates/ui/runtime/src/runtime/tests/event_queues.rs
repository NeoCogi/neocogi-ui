//! Typed constructor receivers preserve ownership, validation and queue ordering.
use super::*;
use neocogi_com::INcStringComPtrExt;
use neocogi_ui::events as snapshot;
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

#[com_object(interfaces(neocogi_com::events::IEventReceiver), ownership = exclusive)]
/// Valid exclusive receiver used to test rejection before UI ownership transfer.
struct ExclusiveReceiver;
impl neocogi_com::events::IEventReceiver for ExclusiveReceiver {
    fn post(
        &mut self,
        _origin: neocogi_com::NcValueRef<'_, neocogi_com::events::EventOrigin>,
        event: ComBox<neocogi_com::events::IEventAbi>,
    ) -> Result<(), (NcStatus, ComBox<neocogi_com::events::IEventAbi>)> {
        Err((neocogi_com::NC_E_CLOSED, event))
    }
}

/// Widget/menu capture is fallible, and all mounting facades return the same root for retry.
#[test]
fn exclusive_receiver_rejection_preserves_unmounted_roots() {
    let ui = nc_ui_nodes::bind_builtin_nodes(runtime()).unwrap();
    let receiver = ExclusiveReceiver::into_box(ExclusiveReceiver)
        .query::<neocogi_com::events::IEventReceiverAbi>()
        .unwrap();
    let queue = Some(receiver.as_com_ref());
    assert!(matches!(ui.widgets().button_with_queue("rejected", queue), Err(NC_E_INVALID_ARG)));
    assert!(matches!(ui.widgets().textbox_with_queue("rejected", queue), Err(NC_E_INVALID_ARG)));
    let menus = ui.menus().unwrap();
    let menu = menus.create("retry").unwrap();
    assert!(matches!(menu.item_with_queue("rejected", queue), Err(NC_E_INVALID_ARG)));
    menu.item("accepted").unwrap();

    let parent = ui.window("parent", RECT, ui.widgets().button_unobserved("parent").unwrap()).unwrap();
    for case in 0..4 {
        let root = ui.widgets().button_unobserved("root").unwrap();
        let origin = root.as_interface().downgrade();
        let result = match case {
            0 => ui.window_with_queue("rejected", RECT, root, queue).map(|_| ()),
            1 => ui.child_window_with_queue(parent.as_com_ref(), "rejected", RECT, root, queue).map(|_| ()),
            2 => ui.dialog_with_queue(parent.as_com_ref(), "rejected", RECT, root, queue).map(|_| ()),
            _ => ui.popup_with_queue(parent.as_com_ref(), "rejected", root, queue).map(|_| ()),
        };
        let (status, root) = result.unwrap_err();
        assert_eq!(status, NC_E_INVALID_ARG);
        assert!(origin.is_alive());
        // The rejected root still belongs to the caller and can be mounted normally.
        let window = ui.window("retry", RECT, root).unwrap();
        window.close().unwrap();
        assert!(!origin.is_alive());
    }
    assert!(receiver.downgrade().is_alive());
    parent.close().unwrap();
}

/// Independent producer ports share an application FIFO without installing observers later.
#[test]
fn construction_queue_preserves_fifo_and_owned_snapshots() {
    let ui = nc_ui_nodes::bind_builtin_nodes(runtime()).unwrap();
    let menu = ui.menus().unwrap().create("origins").unwrap();
    let origin = menu.item("source").unwrap();
    let (sender, messages) = std::sync::mpsc::channel();
    let mut first = EventPort::with_queue(
        Some(neocogi_ui::test_event_queue!(neocogi_ui::IMenuItemAbi, String, sender.clone(), |_queue, _origin, event| event.query_weak::<neocogi_ui::IActivatedEventAbi>()?.label()?.to_string()?).unwrap()),
        origin.as_interface().downgrade().unwrap(),
    );
    let mut second = EventPort::with_queue(
        Some(neocogi_ui::test_event_queue!(neocogi_ui::IMenuItemAbi, String, sender, |_queue, _origin, event| event.query_weak::<neocogi_ui::IActivatedEventAbi>()?.label()?.to_string()?).unwrap()),
        menu.item("second").unwrap().as_interface().downgrade().unwrap(),
    );
    second.post(snapshot::activated("second").unwrap()).unwrap();
    first.post(snapshot::activated("first").unwrap()).unwrap();
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
    let sink = neocogi_ui::test_event_queue!(neocogi_ui::IMenuItemAbi, String, sender,
        |_queue, _origin, event| event.query_weak::<neocogi_ui::IActivatedEventAbi>()?.label()?.to_string()?, { _lifetime: Lifetime = capture })
    .unwrap();
    let mut port = EventPort::with_queue(Some(sink), origin.as_interface().downgrade().unwrap());
    port.post(snapshot::activated("before close").unwrap()).unwrap();
    assert_eq!(drops.get(), 0);
    port.close();
    port.close();
    assert_eq!(drops.get(), 1);
    assert_eq!(port.post(snapshot::activated("after close").unwrap()), Err(neocogi_com::NC_E_CLOSED));
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
    let sink = neocogi_ui::test_event_queue!(neocogi_ui::IMenuItemAbi, String, sender,
        |queue, _origin, event| { queue.calls.set(queue.calls.get() + 1); event.query_weak::<neocogi_ui::IActivatedEventAbi>()?.label()?.to_string()? },
        { calls: Rc<Cell<u32>> = observed })
    .unwrap();
    let mut port = EventPort::with_queue(Some(sink), origin.as_interface().downgrade().unwrap());
    port.post(snapshot::activated("first").unwrap()).unwrap();
    assert_eq!(calls.get(), 1);
    assert_eq!(messages.try_recv().unwrap(), "first");
    drop(messages);
    port.post(snapshot::activated("abandoned").unwrap()).unwrap();
    assert_eq!(calls.get(), 2);
    port.close();
}

/// Permanently unobserved ports need no receiver allocation but still honor terminal closure.
#[test]
fn unobserved_port_has_no_delivery_and_cannot_emit_after_close() {
    let mut port = EventPort::new();
    port.post(snapshot::activated("ignored").unwrap()).unwrap();
    port.close();
    assert_eq!(port.post(snapshot::activated("closed").unwrap()), Err(neocogi_com::NC_E_CLOSED));
}

/// The generated aggregate sink preserves facade domain checks before queue.
#[test]
fn constructor_queues_reject_invalid_payloads_before_recording() {
    let ui = nc_ui_nodes::bind_builtin_nodes(runtime()).unwrap();
    let origin = ui.widgets().textbox_unobserved("").unwrap();
    let (mut sink, receiver) = neocogi_com::events::channel();
    use neocogi_com::events::EventOrigin;
    assert_eq!(snapshot::text_changed("é", 1).unwrap_err(), NC_E_INVALID_ARG);
    assert!(receiver.try_recv().is_err());
    let origin = EventOrigin::Widget(origin.as_interface().downgrade().into_unknown());
    for value in [snapshot::text_changed("é", 2).unwrap(), snapshot::text_submitted("é").unwrap()] {
        sink.post(neocogi_com::NcValueRef::new(&origin).unwrap(), value).unwrap();
    }
    assert_eq!(receiver.try_recv().unwrap().downcast::<neocogi_ui::ITextChangedEventAbi>().unwrap().unwrap().cursor(), Ok(2));
    assert_eq!(receiver.try_recv().unwrap().downcast::<neocogi_ui::ITextSubmittedEventAbi>().unwrap().unwrap().text().unwrap().to_string().unwrap(), "é");
    assert_eq!(snapshot::numeric_changed(f32::INFINITY).unwrap_err(), NC_E_INVALID_ARG);

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
    let origin = button_message.origin.widget::<ButtonAbi>().unwrap().unwrap();
    let event = button_message.downcast::<neocogi_ui::IActivatedEventAbi>().unwrap().unwrap();
    assert!(origin.same_object(&button_origin));
    assert!(!origin.is_alive());
    assert_eq!(event.label().unwrap().to_string().unwrap(), "captured");
    assert!(button_message.downcast::<neocogi_ui::ITextChangedEventAbi>().unwrap().is_none());
    let text_message = messages.try_recv().unwrap();
    let origin = text_message.origin.widget::<TextboxAbi>().unwrap().unwrap();
    let event = text_message.downcast::<neocogi_ui::ITextChangedEventAbi>().unwrap().unwrap();
    assert!(origin.same_object(&text_origin));
    assert!(!origin.is_alive());
    assert_eq!(event.text().unwrap().to_string().unwrap(), "owned snapshot");
    assert!(messages.try_recv().is_err());
}

/// Composite implementation children must never escape as the public event origin.
/// The weak message remains readable after closing and dropping the whole composite.
#[test]
fn composite_queues_report_outer_origins_without_retaining_their_children() {
    let ui = nc_ui_nodes::bind_builtin_nodes(runtime()).unwrap();
    let (queue, messages) = neocogi_com::events::channel();
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
    let message = messages.try_recv().unwrap();
    let captured = message.origin.widget::<TextAreaAbi>().unwrap().unwrap();
    assert!(captured.same_object(&origin));
    assert!(!captured.is_alive());
    assert_eq!(message.downcast::<neocogi_ui::ITextChangedEventAbi>().unwrap().unwrap().text().unwrap().to_string().unwrap(), "outer text");

    let root = ui.widgets().linear(FlowDirection::TopToBottom, CrossSize::Stretch).unwrap();
    let root_owner = root;
    let mut root = root_owner.handle();
    let window = ui.window("combo origin", RECT, root_owner).unwrap();
    let (queue, messages) = neocogi_com::events::channel();
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
    let message = messages.try_recv().unwrap();
    let captured = message.origin.widget::<ComboAbi>().unwrap().unwrap();
    assert!(captured.same_object(&origin));
    assert!(!captured.is_alive());
    let selection = message.downcast::<neocogi_ui::IChoiceChangedEventAbi>().unwrap().unwrap().selection().unwrap();
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
    let queue = neocogi_ui::test_event_queue!(neocogi_ui::IMenuItemAbi, String, send, |_queue, _origin, event| event.query_weak::<neocogi_ui::IActivatedEventAbi>()?.label()?.to_string()?).unwrap();
    let mut port = EventPort::with_queue(Some(queue), origin.as_interface().downgrade().unwrap());
    // The unmounted declaration owns the other strong origin reference.
    // Drop both owners while leaving this separate port and its weak origin alive.
    drop(origin);
    drop(menu);
    assert_eq!(port.post(snapshot::activated("expired origin").unwrap()), Err(neocogi_com::NC_E_DROPPED));
    assert!(matches!(messages.try_recv(), Err(std::sync::mpsc::TryRecvError::Empty)));
    port.close();
    assert_eq!(port.post(snapshot::activated("expired origin").unwrap()), Err(neocogi_com::NC_E_CLOSED));
    assert!(matches!(messages.try_recv(), Err(std::sync::mpsc::TryRecvError::Disconnected)));
}
