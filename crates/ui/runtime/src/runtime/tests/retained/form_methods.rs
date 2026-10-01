//! Compiled generated forms exercise the same concrete methods as handwritten widget clients.
use super::*;

/// This fixture is checked against current generator output by the document test suite.
#[allow(dead_code)]
mod generated {
    include!(concat!(env!("CARGO_MANIFEST_DIR"), "/../document/tests/fixtures/full_bindings.rs"));
}

/// A single-button generated contract permits a complete application handler in this fixture.
#[allow(dead_code)]
mod button_form {
    include!(concat!(env!("CARGO_MANIFEST_DIR"), "/../document/tests/fixtures/empty_bindings.rs"));
}

impl button_form::Handlers for button_form::Form {
    /// Application behavior edits its typed widget after the channel event has been consumed.
    fn on_root_0(&mut self, event: neocogi_ui::button::Event) -> Result<(), NcStatus> {
        match event {
            neocogi_ui::button::Event::Activated(_) => self.widget_root_0.set_label("Handled by application"),
        }
    }
}

/// All former named setter operations are callable through compiled concrete form fields.
#[test]
fn generated_form_fields_preserve_widget_validation_silence_and_reload_lifetime() {
    let mut fixture = Headless::standard();
    let mut slot = slot(&fixture.ui);
    let document = Document::from_json(include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../document/tests/fixtures/bindings.ui.json"
    )))
    .unwrap();
    let (send, events) = std::sync::mpsc::channel();
    let mut candidate = None;
    slot.reload(
        &document,
        Resources::new(),
        |generation| generated::events(generation, send.clone()),
        |component, _| {
            candidate = Some(generated::bind(component)?);
            Ok(())
        },
    )
    .unwrap();
    let mut form = candidate.unwrap();
    form.widget_button_1.set_label("Saved").unwrap();
    assert_eq!(form.widget_button_1.label().unwrap(), "Saved");
    form.widget_button_1.replace_flags(WidgetOptions::ALIGN_RIGHT).unwrap();
    form.widget_button_1.set_flags(WidgetOptions::NO_INTERACT).unwrap();
    form.widget_button_1.clear_flags(WidgetOptions::NO_INTERACT).unwrap();
    assert_eq!(form.widget_button_1.as_interface().properties().unwrap().options, WidgetOptions::ALIGN_RIGHT);
    form.widget_check_2.set_checked(true).unwrap();
    assert!(form.widget_check_2.checked().unwrap());
    form.widget_text_3.set_text("application text").unwrap();
    assert_eq!(form.widget_text_3.text().unwrap(), "application text");
    form.widget_slider_4.set_value(3.0).unwrap();
    assert_eq!(form.widget_slider_4.value().unwrap(), 1.0, "the authored upper bound still clamps setters");
    form.widget_scroll_5.set_offset(25).unwrap();
    assert_eq!(form.widget_scroll_5.range().unwrap().offset, 25);
    let split = form.widget_split_6.settings().unwrap();
    form.widget_split_6.set_position(175).unwrap();
    assert_eq!(form.widget_split_6.settings().unwrap(), SplitOptions { position: 175, ..split });
    form.widget_combo_7.replace_items(&["first", "second"], ChoiceSelection::Selected(1)).unwrap();
    assert_eq!(form.widget_combo_7.display_text().unwrap(), "second");
    form.widget_combo_7.set_selection(ChoiceSelection::None).unwrap();
    assert_eq!(form.widget_combo_7.selection().unwrap(), ChoiceSelection::None);
    form.widget_disclosure_8.set_expanded(true).unwrap();
    assert!(form.widget_disclosure_8.expanded().unwrap());

    // Valid foreign storage can still contain invalid UTF-8 or label ranges.
    // Exercise the widget ABI directly so validation cannot depend on document
    // command adapters or the Rust facade accepting only &str and valid slices.
    let mut combo = form.widget_combo_7.as_interface().clone();
    for (text, first, count) in [
        (&b"\xc3"[..], 0, 1),
        ("é".as_bytes(), 0, 1),
        (&b"label"[..], 5, 1),
        (&b"label"[..], u32::MAX, 2),
    ] {
        let entries: [ChoiceMenuEntry; 1] = [ChoiceMenuEntry::Choice {
            id: 0,
            label: MenuTextRange { first, count },
        }];
        assert!(
            combo
                .replace_menu(
                    neocogi_com::NcCopyFromBufferOffset::new(0, text),
                    neocogi_com::NcCopyFromBufferOffset::new(0, &entries),
                    ChoiceSelection::Selected(0),
                )
                .is_err()
        );
        assert_eq!(form.widget_combo_7.choice_count().unwrap(), 2);
        assert_eq!(form.widget_combo_7.choice_label(0).unwrap(), "first");
        assert_eq!(form.widget_combo_7.choice_label(1).unwrap(), "second");
        assert_eq!(form.widget_combo_7.selection().unwrap(), ChoiceSelection::None);
    }
    fixture.frame(VIEW).unwrap();
    assert!(matches!(events.try_recv(), Err(std::sync::mpsc::TryRecvError::Empty)));
    assert_eq!(Document::from_binary(&slot.component().unwrap().save().unwrap()).unwrap(), document);

    // The application's old Form cannot mutate a replacement generation even
    // though its concrete COM references keep the old allocations alive.
    slot.reload(&document, Resources::new(), |_| Ok(EventBindings::new()), |_, _| Ok(())).unwrap();
    assert_eq!(form.widget_button_1.label(), Err(neocogi_com::NC_E_DROPPED));
    assert_eq!(form.widget_text_3.set_text("stale"), Err(neocogi_com::NC_E_DROPPED));
    assert_eq!(form.widget_split_6.set_position(10), Err(neocogi_com::NC_E_DROPPED));
    assert_eq!(form.widget_combo_7.replace_items(&["stale"], ChoiceSelection::None), Err(neocogi_com::NC_E_DROPPED));
}

/// A setter is synchronous and silent; a real interaction queues the generated event for later handling.
#[test]
fn generated_event_handler_uses_widget_methods_after_receiving_the_user_event() {
    let mut fixture = Headless::standard();
    let mut slot = slot(&fixture.ui);
    let document = Document::from_json(include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/../document/tests/fixtures/empty.ui.json"))).unwrap();
    let (send, events) = std::sync::mpsc::channel();
    slot.reload(
        &document,
        Resources::new(),
        |generation| button_form::events(generation, send.clone()),
        |_, _| Ok(()),
    )
    .unwrap();
    let mut form = button_form::bind(slot.component().unwrap()).unwrap();
    form.widget_root_0.set_label("Before click").unwrap();
    fixture.frame(VIEW).unwrap();
    assert!(matches!(events.try_recv(), Err(std::sync::mpsc::TryRecvError::Empty)));
    pointer(&fixture.ui, Point { x: 30, y: 30 }, true);
    pointer(&fixture.ui, Point { x: 30, y: 30 }, false);
    fixture.frame(VIEW).unwrap();
    assert_eq!(form.widget_root_0.label().unwrap(), "Before click");
    let message = events.try_recv().unwrap();
    assert!(slot.accepts(message.generation));
    let event = message.event.resolve(&form).unwrap().unwrap();
    event.dispatch(&mut form).unwrap();
    assert_eq!(form.widget_root_0.label().unwrap(), "Handled by application");
    assert!(matches!(events.try_recv(), Err(std::sync::mpsc::TryRecvError::Empty)));
}
