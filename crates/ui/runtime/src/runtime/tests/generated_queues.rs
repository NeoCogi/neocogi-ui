//! Generated form queues run against actual COM widgets and transactional reloads.
use super::*;
use neocogi_ui_document::{Document, ReloadSlot, Resources};

/// This exact generated fixture is compared with fresh output in document tests.
#[allow(dead_code)]
mod generated {
    include!(concat!(env!("CARGO_MANIFEST_DIR"), "/../document/tests/fixtures/shared_bindings.rs"));
}

/// Application behavior owns its checked form and ordinary mutable observations.
struct App {
    /// Typed fields remain available to application handlers without a parent pointer.
    form: generated::Form,
    /// Handler identity and captured payload prove same-class origins stay distinct.
    observed: Vec<(u8, String)>,
}
impl generated::Handlers for App {
    /// Receives the first button through its own required application method.
    fn on_first_1(&mut self, event: neocogi_ui::button::Event) -> Result<(), NcStatus> {
        let neocogi_ui::button::Event::Activated(event) = event;
        self.observed.push((1, event.label));
        // Resolution finished borrowing form before entering this mutable handler.
        self.form.widget_first_1.set_label("handled first")?;
        Ok(())
    }
    /// The second button uses the same queue interface and a different handler.
    fn on_second_2(&mut self, event: neocogi_ui::button::Event) -> Result<(), NcStatus> {
        let neocogi_ui::button::Event::Activated(event) = event;
        self.observed.push((2, event.label));
        Ok(())
    }
    /// Another queue projection shares the same concrete application allocation.
    fn on_text_3(&mut self, event: neocogi_ui::textbox::Event) -> Result<(), NcStatus> {
        match event {
            neocogi_ui::textbox::Event::Changed(event) => self.observed.push((3, event.text)),
            neocogi_ui::textbox::Event::Submitted(_) => {}
        }
        Ok(())
    }
}

/// Supplies actual input; tests never call an application handler during post.
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

/// Shared queues distinguish repeated classes, preserve deferred mutable handlers,
/// and reject old/foreign origins independently of whether weak promotion succeeds.
#[test]
fn generated_shared_queue_resolves_origins_and_rejects_replaced_forms() {
    let ui = nc_ui_nodes::bind_builtin_nodes(runtime()).unwrap();
    let mut root = ui.widgets().linear(FlowDirection::TopToBottom, CrossSize::Stretch).unwrap();
    let window = ui.window("generated", RECT, &root).unwrap();
    let mut slot = ReloadSlot::new(ui.clone(), window.clone()).unwrap();
    root.insert(0, slot.widget(), LinearEdge::new(LayoutTrack::Flex { weight: 1.0 }))
        .unwrap()
        .finish()
        .unwrap();
    let document = Document::from_json(include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../document/tests/fixtures/shared.ui.json"
    )))
    .unwrap();
    let (send, messages) = std::sync::mpsc::channel();
    let mut form = None;
    slot.reload(
        &document,
        Resources::new(),
        |generation| generated::events(generation, send.clone()),
        |component, _| {
            form = Some(generated::bind(component)?);
            Ok(())
        },
    )
    .unwrap();
    let mut app = App {
        form: form.unwrap(),
        observed: Vec::new(),
    };
    for input in [KeyCode::Tab, KeyCode::Space, KeyCode::Tab, KeyCode::Space, KeyCode::Tab] {
        key(&ui, input);
    }
    ui.input(UiInput::Text, "captured text").unwrap();
    ui.update(SIZE).unwrap();
    assert!(app.observed.is_empty(), "enqueueing must not run application behavior");
    for message in messages.try_iter() {
        assert!(slot.accepts(message.generation));
        let event = message.event.resolve(&app.form).unwrap().unwrap();
        event.dispatch(&mut app).unwrap();
    }
    assert_eq!(app.observed, [(1, "first".into()), (2, "second".into()), (3, "captured text".into())]);
    assert_eq!(app.form.widget_first_1.label().unwrap(), "handled first");

    // Capture an actual fact, then replace the component while retaining all old
    // fields. Weak promotion alone deliberately cannot authenticate form membership.
    ui.input(UiInput::Text, " stale").unwrap();
    ui.update(SIZE).unwrap();
    let old = messages.try_recv().unwrap();
    let mut replacement = None;
    slot.reload(
        &document,
        Resources::new(),
        |generation| generated::events(generation, send.clone()),
        |component, _| {
            replacement = Some(generated::bind(component)?);
            Ok(())
        },
    )
    .unwrap();
    let replacement = replacement.unwrap();
    assert!(!slot.accepts(old.generation));
    let old_origin = app.form.widget_text_3.as_interface().downgrade().unwrap();
    assert!(old_origin.upgrade().unwrap().is_some(), "old form deliberately retains a closed widget");
    assert!(old.event.resolve(&replacement).unwrap().is_none());
    assert_eq!(app.form.widget_text_3.text().unwrap_err(), UI_E_CLOSED);

    // An unrelated live widget with the right interface cannot select a handler.
    let outsider = ui.widgets().button_unobserved("outsider").unwrap();
    let unrelated = generated::QueuedEvent::Source0 {
        origin: outsider.as_interface().downgrade().unwrap(),
        event: neocogi_ui::button::Event::Activated(Activation { label: "outside".into() }),
    };
    assert!(unrelated.resolve(&replacement).unwrap().is_none());
    let expired = generated::QueuedEvent::Source0 {
        origin: outsider.as_interface().downgrade().unwrap(),
        event: neocogi_ui::button::Event::Activated(Activation { label: "expired".into() }),
    };
    drop(outsider);
    assert!(expired.resolve(&replacement).unwrap().is_none());
    drop(app);
    assert!(old_origin.upgrade().unwrap().is_none());
    window.close().unwrap();
}
