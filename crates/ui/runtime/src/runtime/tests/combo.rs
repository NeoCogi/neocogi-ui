//! End-to-end regressions for the runtime-owned combo-box composite.

use super::*;
use neocogi_ui::{EventHandler, EventSession};

/// Application state observes only public committed-selection events.
struct Observations {
    /// Immutable events captured in dispatch order.
    changes: Vec<ComboSelection>,
}

/// Concrete changed-event recorder used by the runtime dispatch boundary.
struct RecordSelection;

impl EventHandler<Observations, ComboSelection> for RecordSelection {
    /// Retains the delivered snapshot without rereading the live combo model.
    fn handle(&mut self, state: &mut Observations, event: &ComboSelection) -> Result<(), NcStatus> {
        state.changes.push(*event);
        Ok(())
    }
}

/// Queues one non-repeated key press for the next production update cycle.
fn press(ui: &neocogi_ui::Ui, key: KeyCode) {
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

/// Explicit absence, strict replacement, popup fallback, and keyboard commit share one model.
#[test]
fn combo_owns_optional_selection_and_popup_state_end_to_end() {
    let ui = neocogi_ui::Ui::from_interface(runtime()).unwrap();
    let mut root = ui.widgets().linear(FlowDirection::TopToBottom, CrossSize::Stretch).unwrap();
    let window = ui.window("combo", RECT, &root).unwrap();
    let mut combo = ui.combo_box(window.as_com_ref(), &["first", "second"], ComboSelection::Selected(1)).unwrap();
    combo.set_placeholder("Choose a value").unwrap();
    root.insert(0, &combo, LinearEdge::new(LayoutTrack::Content)).unwrap().finish().unwrap();
    let session = EventSession::new(Observations { changes: Vec::new() });
    session.subscribe_combo_changed(&combo, RecordSelection).unwrap();

    ui.update_with_events(SIZE, &session).unwrap();
    assert_eq!(combo.selection().unwrap(), ComboSelection::Selected(1));
    assert_eq!(combo.display_text().unwrap(), "second");

    // Opening and cancelling cannot erase or transiently commit over the existing selection.
    for key in [KeyCode::Tab, KeyCode::Enter, KeyCode::Escape] {
        press(&ui, key);
    }
    ui.update_with_events(SIZE, &session).unwrap();
    assert_eq!(combo.selection().unwrap(), ComboSelection::Selected(1));
    assert!(session.state().unwrap().changes.is_empty());

    // Reopening initializes the keyboard candidate from item one; Down wraps to item zero and
    // Enter is the only transition that commits and publishes the changed snapshot.
    for key in [KeyCode::Enter, KeyCode::ArrowDown, KeyCode::Enter] {
        press(&ui, key);
    }
    ui.update_with_events(SIZE, &session).unwrap();
    assert_eq!(combo.selection().unwrap(), ComboSelection::Selected(0));
    assert_eq!(session.state().unwrap().changes, [ComboSelection::Selected(0)]);

    // Invalid candidates never clamp and never partially replace the valid live snapshot.
    assert_eq!(combo.replace_items(&["shorter"], ComboSelection::Selected(7)), Err(NC_E_INVALID_ARG));
    assert_eq!(combo.item_count(), Ok(2));
    assert_eq!(combo.display_text().unwrap(), "first");
    assert_eq!(combo.set_selection(ComboSelection::Selected(7)), Err(NC_E_INVALID_ARG));

    // A valid empty replacement has an explicit absent selection and a nonblank fallback.
    combo.replace_items::<&str>(&[], ComboSelection::None).unwrap();
    assert_eq!(combo.selection(), Ok(ComboSelection::None));
    assert_eq!(combo.display_text().unwrap(), "Choose a value");
    assert_eq!(session.dispatch(), Ok(0));
    let (renderer, _) = renderer();
    assert_eq!(ui.render(SIZE, renderer.as_com_ref()), Err(UI_E_UPDATE_REQUIRED));
    ui.update_with_events(SIZE, &session).unwrap();
    ui.render(SIZE, renderer.as_com_ref()).unwrap();

    window.close().unwrap();
    assert_eq!(combo.selection(), Err(UI_E_CLOSED));
}

/// Construction rejects an invalid initial selection without altering the owner window.
#[test]
fn combo_construction_is_atomic_for_invalid_selection() {
    let ui = neocogi_ui::Ui::from_interface(runtime()).unwrap();
    let root = ui.widgets().text_block("owner", TextWrapping::None).unwrap();
    let window = ui.window("combo owner", RECT, &root).unwrap();
    let defaulted = ui.combo_box(window.as_com_ref(), &["only"], ComboSelection::None).unwrap();
    assert_eq!(defaulted.placeholder().unwrap(), "Select…");
    assert_eq!(defaulted.display_text().unwrap(), "Select…");
    let result = ui.combo_box(window.as_com_ref(), &["only"], ComboSelection::Selected(1));
    assert!(matches!(result, Err(NC_E_INVALID_ARG)));
    assert_eq!(window.snapshot().unwrap().visibility, Visibility::Visible);
}
