//! Shared document delivery needs no generated event enum or receiver implementation.
use super::*;
use neocogi_ui::events::{EventOrigin, EventMessage};
use neocogi_ui_document::{Document, DocumentEvent, EventBindings, CustomWidgetRegistry, ReloadIds, ReloadSlot, Resources, document_receiver};

/// Optional generated fields remain ordinary checked widget conveniences.
#[allow(dead_code)]
mod generated {
    include!(concat!(env!("CARGO_MANIFEST_DIR"), "/../document/tests/fixtures/shared_bindings.rs"));
}

fn key(ui: &neocogi_ui::Ui, key: KeyCode) {
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

/// Current generation and exact widget identity are independent admission checks.
#[test]
fn shared_document_receiver_rejects_foreign_and_replaced_forms_before_dispatch() {
    let ui = nc_ui_nodes::bind_builtin_nodes(runtime()).unwrap();
    let root_owner = ui.widgets().linear(FlowDirection::TopToBottom, CrossSize::Stretch).unwrap();
    let mut root = root_owner.handle();
    let window = ui.window("document events", RECT, root_owner).unwrap();
    let mut ids = ReloadIds::new();
    let limits = neocogi_ui::GridLimits { max_columns: 64, max_rows: 256 };
    let mut slot = ReloadSlot::new(&mut ids, ui.clone(), window.clone(), limits).unwrap();
    root.insert(0, slot.take_widget().unwrap(), LinearEdge::new(LayoutTrack::Flex { weight: 1.0 }))
        .unwrap();
    let document = Document::from_json(include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../document/tests/fixtures/shared.ui.json"
    )))
    .unwrap();
    let custom = CustomWidgetRegistry::new();
    let (send, messages) = std::sync::mpsc::channel();
    let mut receiver = None;
    let first = slot
        .reload(
            &document,
            Resources::new(),
            |generation| {
                let queue = document_receiver(generation, send.clone())?;
                let bindings = EventBindings::all(&document, &custom, queue.as_com_ref())?;
                receiver = Some(queue);
                Ok(bindings)
            },
            |_, _| Ok(()),
        )
        .unwrap()
        .generation;
    let mut form = generated::bind(slot.component().unwrap()).unwrap();
    let mut foreign_slot = ReloadSlot::new(&mut ids, ui.clone(), window.clone(), limits).unwrap();
    let foreign = foreign_slot
        .reload(
            &document,
            Resources::new(),
            |generation| EventBindings::all(&document, &custom, document_receiver(generation, send.clone())?.as_com_ref()),
            |_, _| Ok(()),
        )
        .unwrap()
        .generation;
    assert_ne!(first, foreign);
    let foreign_form = generated::bind(foreign_slot.component().unwrap()).unwrap();
    let foreign_event = DocumentEvent {
        // Even a current token cannot authenticate an unrelated live widget.
        generation: first,
        message: EventMessage {
            origin: EventOrigin::Widget(foreign_form.widget_first_1.as_interface().clone().into_unknown()),
            event: neocogi_ui::events::activated().unwrap(),
        },
    };
    assert!(foreign_event.event_for(&slot, "first").unwrap().is_none());
    for input in [KeyCode::Tab, KeyCode::Space, KeyCode::Tab, KeyCode::Space, KeyCode::Tab] {
        key(&ui, input);
    }
    ui.input(UiInput::Text, "captured text").unwrap();
    ui.update(SIZE).unwrap();
    assert_eq!(form.widget_first_1.label().unwrap(), "first", "posting cannot enter application behavior");
    let mut observed = Vec::new();
    for message in messages.try_iter() {
        assert!(slot.accepts(message.generation));
        assert!(!foreign_slot.accepts(message.generation));
        if let Some(event) = message.event_for(&slot, "first").unwrap() {
            observed.push((1, {
                assert!(event.query::<IActivatedEventAbi>().unwrap().is_some());
                "first".to_owned()
            }));
            form.widget_first_1.set_label("handled first").unwrap();
        } else if let Some(event) = message.event_for(&slot, "second").unwrap() {
            observed.push((2, {
                assert!(event.query::<IActivatedEventAbi>().unwrap().is_some());
                "second".to_owned()
            }));
        } else if let Some(event) = message.event_for(&slot, "text").unwrap() {
            observed.push((3, event.query::<ITextChangedEventAbi>().unwrap().unwrap().text().unwrap().to_string().unwrap()));
        }
    }
    assert_eq!(observed, [(1, "first".into()), (2, "second".into()), (3, "captured text".into())]);
    assert_eq!(form.widget_first_1.label().unwrap(), "handled first");
    ui.input(UiInput::Text, " stale").unwrap();
    ui.update(SIZE).unwrap();
    let old = messages.try_recv().unwrap();
    let next = slot
        .reload(
            &document,
            Resources::new(),
            |generation| EventBindings::all(&document, &custom, document_receiver(generation, send.clone())?.as_com_ref()),
            |_, _| Ok(()),
        )
        .unwrap()
        .generation;
    assert!(!slot.accepts(old.generation));
    assert!(
        old.event_for(&slot, "missing key").unwrap().is_none(),
        "generation check precedes even widget lookup"
    );
    assert!(!form.widget_text_3.as_interface().is_alive());
    assert_eq!(form.widget_text_3.text().unwrap_err(), neocogi_com::NC_E_DROPPED);
    let outsider = ui.widgets().button_unobserved("outsider").unwrap();
    let outside = DocumentEvent {
        generation: next,
        message: EventMessage {
            origin: EventOrigin::Widget(outsider.as_interface().downgrade().into_unknown()),
            event: neocogi_ui::events::activated().unwrap(),
        },
    };
    assert!(outside.event_for(&slot, "first").unwrap().is_none());
    drop(outsider);
    assert!(outside.event_for(&slot, "first").unwrap().is_none());
    let mut receiver = receiver.unwrap();
    let service = DocumentEvent {
        generation: next,
        message: EventMessage {
            origin: EventOrigin::Service(receiver.downgrade().unwrap().into_unknown()),
            event: neocogi_ui::events::activated().unwrap(),
        },
    };
    assert!(service.event_for(&slot, "first").unwrap().is_none());
    drop(messages);
    let event = neocogi_ui::events::activated().unwrap();
    let allocation = event.as_raw();
    let (status, returned) = receiver
        .post(neocogi_com::NcValueRef::new(&outside.message.origin).unwrap(), event)
        .unwrap_err();
    assert_eq!(status, neocogi_com::NC_E_CLOSED);
    assert_eq!(returned.as_raw(), allocation);
    window.close().unwrap();
}
