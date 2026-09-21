//! End-to-end regressions for the runtime-owned combo-box composite.

use super::*;

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
    let ui = nc_ui_nodes::bind_builtin_nodes(runtime()).unwrap();
    let mut root = ui.widgets().linear(FlowDirection::TopToBottom, CrossSize::Stretch).unwrap();
    let window = ui.window("combo", RECT, &root).unwrap();
    let mut combo = ui.combo_box(window.as_com_ref(), &["first", "second"], ChoiceSelection::Selected(1)).unwrap();
    combo.set_placeholder("Choose a value").unwrap();
    root.insert(0, &combo, LinearEdge::new(LayoutTrack::Content)).unwrap().finish().unwrap();
    let (session_tx, session) = std::sync::mpsc::channel();
    let mut changes = Vec::new();
    neocogi_ui::events::map(combo.changed(), session_tx.clone(), |event| event).unwrap();

    ui.update_with_events(SIZE, &session, |message| {
        changes.push(message);
        Ok(())
    })
    .unwrap();
    assert_eq!(combo.selection().unwrap(), ChoiceSelection::Selected(1));
    assert_eq!(combo.display_text().unwrap(), "second");

    // Opening and cancelling cannot erase or transiently commit over the existing selection.
    for key in [KeyCode::Tab, KeyCode::Enter, KeyCode::Escape] {
        press(&ui, key);
    }
    ui.update_with_events(SIZE, &session, |message| {
        changes.push(message);
        Ok(())
    })
    .unwrap();
    assert_eq!(combo.selection().unwrap(), ChoiceSelection::Selected(1));
    assert!(changes.is_empty());

    // Reopening initializes the keyboard candidate from item one; Down wraps to item zero and
    // Enter is the only transition that commits and publishes the changed snapshot.
    for key in [KeyCode::Enter, KeyCode::ArrowDown, KeyCode::Enter] {
        press(&ui, key);
    }
    ui.update_with_events(SIZE, &session, |message| {
        changes.push(message);
        Ok(())
    })
    .unwrap();
    assert_eq!(combo.selection().unwrap(), ChoiceSelection::Selected(0));
    assert_eq!(changes, [ChoiceSelection::Selected(0)]);

    // Invalid candidates never clamp and never partially replace the valid live snapshot.
    assert_eq!(combo.replace_items(&["shorter"], ChoiceSelection::Selected(7)), Err(NC_E_INVALID_ARG));
    assert_eq!(combo.choice_count(), Ok(2));
    assert_eq!(combo.display_text().unwrap(), "first");
    assert_eq!(combo.set_selection(ChoiceSelection::Selected(7)), Err(NC_E_INVALID_ARG));

    // A valid empty replacement has an explicit absent selection and a nonblank fallback.
    combo.replace_items::<&str>(&[], ChoiceSelection::None).unwrap();
    assert_eq!(combo.selection(), Ok(ChoiceSelection::None));
    assert_eq!(combo.display_text().unwrap(), "Choose a value");
    assert!(session.try_recv().is_err());
    let (renderer, _) = renderer();
    assert_eq!(ui.render(SIZE, renderer.as_com_ref()), Err(UI_E_UPDATE_REQUIRED));
    ui.update_with_events(SIZE, &session, |message| {
        changes.push(message);
        Ok(())
    })
    .unwrap();
    ui.render(SIZE, renderer.as_com_ref()).unwrap();

    window.close().unwrap();
    assert_eq!(combo.selection(), Err(UI_E_CLOSED));
}

/// Construction rejects an invalid initial selection without altering the owner window.
#[test]
fn combo_construction_is_atomic_for_invalid_selection() {
    let ui = nc_ui_nodes::bind_builtin_nodes(runtime()).unwrap();
    let root = ui.widgets().text_block("owner", TextWrapping::None).unwrap();
    let window = ui.window("combo owner", RECT, &root).unwrap();
    let defaulted = ui.combo_box(window.as_com_ref(), &["only"], ChoiceSelection::None).unwrap();
    assert_eq!(defaulted.placeholder().unwrap(), "Select…");
    assert_eq!(defaulted.display_text().unwrap(), "Select…");
    let result = ui.combo_box(window.as_com_ref(), &["only"], ChoiceSelection::Selected(1));
    assert!(matches!(result, Err(NC_E_INVALID_ARG)));
    assert_eq!(window.snapshot().unwrap().visibility, Visibility::Visible);
}

/// Nested combo choices traverse the shared submenu forest and commit stable leaf identities.
#[test]
fn combo_choice_menu_uses_shared_submenu_navigation() {
    let ui = nc_ui_nodes::bind_builtin_nodes(runtime()).unwrap();
    let mut root = ui.widgets().linear(FlowDirection::TopToBottom, CrossSize::Stretch).unwrap();
    let window = ui.window("nested combo", RECT, &root).unwrap();
    let menu_text = "FruitAppleBanana";
    let menu = [
        neocogi_ui::ChoiceMenuEntry::Submenu {
            label: neocogi_ui::MenuTextRange { first: 0, count: 5 },
            children: 2,
        },
        neocogi_ui::ChoiceMenuEntry::Choice {
            id: 10,
            label: neocogi_ui::MenuTextRange { first: 5, count: 5 },
        },
        neocogi_ui::ChoiceMenuEntry::Choice {
            id: 11,
            label: neocogi_ui::MenuTextRange { first: 10, count: 6 },
        },
    ];
    let mut combo = ui.combo_menu(window.as_com_ref(), menu_text, &menu, ChoiceSelection::Selected(10)).unwrap();
    root.insert(0, &combo, LinearEdge::new(LayoutTrack::Content)).unwrap().finish().unwrap();
    let (session_tx, session) = std::sync::mpsc::channel();
    let mut changes = Vec::new();
    neocogi_ui::events::map(combo.changed(), session_tx.clone(), |event| event).unwrap();
    ui.update_with_events(SIZE, &session, |message| {
        changes.push(message);
        Ok(())
    })
    .unwrap();

    // Opening restores the committed Apple branch automatically; Down then moves directly to
    // Banana inside that already active submenu, and Enter commits its stable identity.
    for key in [KeyCode::Tab, KeyCode::Enter, KeyCode::ArrowDown, KeyCode::Enter] {
        press(&ui, key);
    }
    ui.update_with_events(SIZE, &session, |message| {
        changes.push(message);
        Ok(())
    })
    .unwrap();
    assert_eq!(combo.selection().unwrap(), ChoiceSelection::Selected(11));
    assert_eq!(combo.display_text().unwrap(), "Banana");
    assert_eq!(combo.choice_count().unwrap(), 2);
    assert_eq!(changes, [ChoiceSelection::Selected(11)]);

    // A silent selection edit while the hierarchy is open rebuilds and restores the selected
    // branch instead of closing the popup. Down therefore moves from Apple back to Banana.
    press(&ui, KeyCode::Enter);
    ui.update_with_events(SIZE, &session, |message| {
        changes.push(message);
        Ok(())
    })
    .unwrap();
    combo.set_selection(ChoiceSelection::Selected(10)).unwrap();
    ui.update_with_events(SIZE, &session, |message| {
        changes.push(message);
        Ok(())
    })
    .unwrap();
    assert_eq!(changes, [ChoiceSelection::Selected(11)]);
    for key in [KeyCode::ArrowDown, KeyCode::Enter] {
        press(&ui, key);
    }
    ui.update_with_events(SIZE, &session, |message| {
        changes.push(message);
        Ok(())
    })
    .unwrap();
    assert_eq!(combo.selection().unwrap(), ChoiceSelection::Selected(11));
    assert_eq!(changes, [ChoiceSelection::Selected(11), ChoiceSelection::Selected(11)]);
}
