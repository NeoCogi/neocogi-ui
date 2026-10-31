//! End-to-end regressions for the runtime-owned combo-box composite.

use super::*;
use neocogi_ui_support::test_support::events::TestApp;

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
    let root_owner = ui.widgets().linear(FlowDirection::TopToBottom, CrossSize::Stretch).unwrap();
    let mut root = root_owner.handle();
    let window = ui.window("combo", RECT, root_owner).unwrap();
    let (session_tx, session) = std::sync::mpsc::channel();
    let combo_owner = ui
        .combo_box_with_queue(
            window.as_com_ref(),
            &["first", "second"],
            ChoiceSelection::Selected(1),
            Some(
                neocogi_ui::test_event_queue!(neocogi_ui::ComboAbi, ChoiceSelection, session_tx.clone(), |_queue, _origin, event| event
                    .query_weak::<IChoiceChangedEventAbi>()?
                    .selection()?)
                .unwrap()
                .as_com_ref(),
            ),
        )
        .unwrap();
    let mut combo = combo_owner.handle();
    combo.set_placeholder("Choose a value").unwrap();
    root.insert(0, combo_owner, LinearEdge::new(LayoutTrack::Content)).unwrap();
    let changes = Vec::new();

    let app = TestApp::new(changes, session, |state, message| {
        state.push(message);
        Ok(())
    });
    ui.update_with_events(SIZE, app.as_interface()).unwrap();
    assert_eq!(combo.selection().unwrap(), ChoiceSelection::Selected(1));
    assert_eq!(combo.display_text().unwrap(), "second");

    // Opening and cancelling cannot erase or transiently commit over the existing selection.
    for key in [KeyCode::Tab, KeyCode::Enter, KeyCode::Escape] {
        press(&ui, key);
    }
    ui.update_with_events(SIZE, app.as_interface()).unwrap();
    assert_eq!(combo.selection().unwrap(), ChoiceSelection::Selected(1));
    assert!(app.state().is_empty());

    // Reopening initializes the keyboard candidate from item one; Down wraps to item zero and
    // Enter is the only transition that commits and publishes the changed snapshot.
    for key in [KeyCode::Enter, KeyCode::ArrowDown, KeyCode::Enter] {
        press(&ui, key);
    }
    ui.update_with_events(SIZE, app.as_interface()).unwrap();
    assert_eq!(combo.selection().unwrap(), ChoiceSelection::Selected(0));
    assert_eq!(*app.state(), [ChoiceSelection::Selected(0)]);

    // Invalid candidates never clamp and never partially replace the valid live snapshot.
    assert_eq!(combo.replace_items(&["shorter"], ChoiceSelection::Selected(7)), Err(NC_E_INVALID_ARG));
    assert_eq!(combo.choice_count(), Ok(2));
    assert_eq!(combo.display_text().unwrap(), "first");
    assert_eq!(combo.set_selection(ChoiceSelection::Selected(7)), Err(NC_E_INVALID_ARG));

    // A valid empty replacement has an explicit absent selection and a nonblank fallback.
    combo.replace_items::<&str>(&[], ChoiceSelection::None).unwrap();
    assert_eq!(combo.selection(), Ok(ChoiceSelection::None));
    assert_eq!(combo.display_text().unwrap(), "Choose a value");
    assert_eq!(app.as_interface().dispatch_next(), Ok(ApplicationDispatch::Idle));
    let (renderer, _) = renderer();
    assert_eq!(ui.render(SIZE, renderer.as_com_ref()), Err(UI_E_UPDATE_REQUIRED));
    ui.update_with_events(SIZE, app.as_interface()).unwrap();
    ui.render(SIZE, renderer.as_com_ref()).unwrap();

    window.close().unwrap();
    assert_eq!(combo.selection(), Err(neocogi_com::NC_E_DROPPED));
}

/// Construction rejects an invalid initial selection without altering the owner window.
#[test]
fn combo_construction_is_atomic_for_invalid_selection() {
    let ui = nc_ui_nodes::bind_builtin_nodes(runtime()).unwrap();
    let root = ui.widgets().text_block("owner", TextWrapping::None).unwrap();
    let window = ui.window("combo owner", RECT, root).unwrap();
    let defaulted = ui.combo_box_unobserved(window.as_com_ref(), &["only"], ChoiceSelection::None).unwrap();
    assert_eq!(defaulted.placeholder().unwrap(), "Select…");
    assert_eq!(defaulted.display_text().unwrap(), "Select…");
    let result = ui.combo_box_unobserved(window.as_com_ref(), &["only"], ChoiceSelection::Selected(1));
    assert!(matches!(result, Err(NC_E_INVALID_ARG)));
    assert_eq!(window.snapshot().unwrap().visibility, Visibility::Visible);
}

/// Nested combo choices traverse the shared submenu forest and commit stable leaf identities.
#[test]
fn combo_choice_menu_uses_shared_submenu_navigation() {
    let ui = nc_ui_nodes::bind_builtin_nodes(runtime()).unwrap();
    let root = ui.widgets().linear(FlowDirection::TopToBottom, CrossSize::Stretch).unwrap();
    let root_owner = root;
    let mut root = root_owner.handle();
    let window = ui.window("nested combo", RECT, root_owner).unwrap();
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
    let (session_tx, session) = std::sync::mpsc::channel();
    let combo = ui
        .combo_menu_with_queue(
            window.as_com_ref(),
            menu_text,
            &menu,
            ChoiceSelection::Selected(10),
            Some(
                neocogi_ui::test_event_queue!(neocogi_ui::ComboAbi, ChoiceSelection, session_tx.clone(), |_queue, _origin, event| event
                    .query_weak::<IChoiceChangedEventAbi>()?
                    .selection()?)
                .unwrap()
                .as_com_ref(),
            ),
        )
        .unwrap();
    let combo_owner = combo;
    let mut combo = combo_owner.handle();
    root.insert(0, combo_owner, LinearEdge::new(LayoutTrack::Content)).unwrap();
    let changes = Vec::new();

    let app = TestApp::new(changes, session, |state, message| {
        state.push(message);
        Ok(())
    });
    ui.update_with_events(SIZE, app.as_interface()).unwrap();

    // Opening restores the committed Apple branch automatically; Down then moves directly to
    // Banana inside that already active submenu, and Enter commits its stable identity.
    for key in [KeyCode::Tab, KeyCode::Enter, KeyCode::ArrowDown, KeyCode::Enter] {
        press(&ui, key);
    }
    ui.update_with_events(SIZE, app.as_interface()).unwrap();
    assert_eq!(combo.selection().unwrap(), ChoiceSelection::Selected(11));
    assert_eq!(combo.display_text().unwrap(), "Banana");
    assert_eq!(combo.choice_count().unwrap(), 2);
    assert_eq!(*app.state(), [ChoiceSelection::Selected(11)]);

    // A silent selection edit while the hierarchy is open rebuilds and restores the selected
    // branch instead of closing the popup. Down therefore moves from Apple back to Banana.
    press(&ui, KeyCode::Enter);
    ui.update_with_events(SIZE, app.as_interface()).unwrap();
    combo.set_selection(ChoiceSelection::Selected(10)).unwrap();
    ui.update_with_events(SIZE, app.as_interface()).unwrap();
    assert_eq!(*app.state(), [ChoiceSelection::Selected(11)]);
    for key in [KeyCode::ArrowDown, KeyCode::Enter] {
        press(&ui, key);
    }
    ui.update_with_events(SIZE, app.as_interface()).unwrap();
    assert_eq!(combo.selection().unwrap(), ChoiceSelection::Selected(11));
    assert_eq!(*app.state(), [ChoiceSelection::Selected(11), ChoiceSelection::Selected(11)]);
}

#[neocogi_com::com_object(interfaces(neocogi_ui::events::IEventReceiver))]
/// Deliberately reentrant queue attempts removal during composite output.
/// Application queues normally only enqueue; the active widget gate must reject this.
struct RemoveComboDuringPost {
    /// Weak parent access cannot preserve the emitted control after clear.
    root: neocogi_ui::LinearWidget,
    /// Independent observation proves one real submission crossed the ABI.
    calls: Rc<Cell<u32>>,
}
impl neocogi_ui::events::IEventReceiver for RemoveComboDuringPost {
    /// A receiver cannot destroy the composite whose dispatch call is active.
    fn post(
        &mut self,
        _: neocogi_com::NcValueRef<'_, neocogi_ui::events::EventOrigin>,
        event_owner: neocogi_com::ComBox<neocogi_ui::events::IEventAbi>,
    ) -> Result<(), (NcStatus, neocogi_com::ComBox<neocogi_ui::events::IEventAbi>)> {
        let result = (|| -> Result<(), NcStatus> {
            event_owner.query_weak::<neocogi_ui::IChoiceChangedEventAbi>()?;
            self.calls.set(self.calls.get() + 1);
            self.root.clear()
        })();
        result.map_err(|status| (status, event_owner))
    }
}

/// Composite forwarding runs under its own gate; removal retries after return
/// and releases both the menu and application queue without a second notification.
#[test]
fn combo_forwarding_rejects_active_removal_then_releases_menu_and_queue() {
    let ui = nc_ui_nodes::bind_builtin_nodes(runtime()).unwrap();
    let root_owner = ui.widgets().linear(FlowDirection::TopToBottom, CrossSize::Stretch).unwrap();
    let mut root = root_owner.handle();
    let window = ui.window("reentrant combo destruction", RECT, root_owner).unwrap();
    let calls = Rc::new(Cell::new(0));
    let queue = RemoveComboDuringPost::from_value(RemoveComboDuringPost { root: root.clone(), calls: calls.clone() })
        .query::<neocogi_ui::events::IEventReceiverAbi>()
        .unwrap();
    let weak_queue = queue.downgrade().unwrap();
    let combo_owner = ui
        .combo_box_with_queue(
            window.as_com_ref(),
            &["first", "second"],
            ChoiceSelection::Selected(0),
            Some(queue.as_com_ref()),
        )
        .unwrap();
    let combo = combo_owner.handle();
    root.insert(0, combo_owner, LinearEdge::new(LayoutTrack::Content)).unwrap();
    drop(queue);
    for key in [KeyCode::Tab, KeyCode::Enter, KeyCode::ArrowDown, KeyCode::Enter] {
        press(&ui, key);
    }
    assert_eq!(ui.update(SIZE), Err(UI_E_BUSY));
    assert_eq!(calls.get(), 1);
    assert_eq!(combo.selection(), Ok(ChoiceSelection::Selected(1)));
    root.clear().unwrap();
    assert_eq!(combo.selection(), Err(neocogi_com::NC_E_DROPPED));
    assert!(
        weak_queue.upgrade().unwrap().is_none(),
        "the popup coordinator must not retain the dead header's destination"
    );
    assert_eq!(root.children().unwrap().child_count(), Ok(0));
    let (renderer, _) = renderer();
    ui.update(SIZE).unwrap();
    ui.render(SIZE, renderer.as_com_ref()).unwrap();
    assert_eq!(calls.get(), 1);
}

/// Large accepted definitions exercise the real core-to-node layout without lowering document limits.
#[test]
fn large_combo_layout_keeps_admitted_document_ceiling() {
    let mut document = neocogi_ui_document::Document::new("large admitted combo");
    document.root = neocogi_ui_document::Node::new(
        "combo",
        neocogi_ui_document::WidgetSpec::Combo {
            labels: vec!["a".to_owned(); 65_536],
            selected: None,
            placeholder: "".into(),
        },
    );
    document.outputs.clear();
    document.validate_binary().unwrap();
    assert_eq!(document.to_binary().unwrap().len(), 327_803);
    for count in [2_000, 4_000, 8_000, 65_536] {
        let ui = nc_ui_nodes::bind_builtin_nodes(runtime()).unwrap();
        let root = ui.widgets().linear(FlowDirection::TopToBottom, CrossSize::Stretch).unwrap();
        let mut root_handle = root.handle();
        let window = ui.window("large combo", RECT, root).unwrap();
        let entries: Vec<_> = (0..count)
            .map(|id| ChoiceMenuEntry::Choice {
                id,
                label: MenuTextRange { first: 0, count: 1 },
            })
            .collect();
        let combo = ui.combo_menu_unobserved(window.as_com_ref(), "a", &entries, ChoiceSelection::None).unwrap();
        let mut handle = combo.handle();
        root_handle.insert(0, combo, LinearEdge::new(LayoutTrack::Content)).unwrap();
        ui.update(SIZE).unwrap();
        assert_eq!(handle.choice_count().unwrap(), count as usize);
        handle.set_selection(ChoiceSelection::Selected(count - 1)).unwrap();
        ui.update(SIZE).unwrap();
        assert_eq!(handle.display_text().unwrap(), "a");
        ui.close().unwrap();
    }
}
