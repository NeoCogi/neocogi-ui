//! Compact menu declarations, live row edits, and manager keyboard policy cross only concrete capabilities.
use super::*;
use neocogi_ui::support::test_support::events::TestApp;

/// Multi-level hides restore the first transition, while a retained menu keeps its text scope.
#[test]
fn hiding_nested_menu_suffix_routes_the_first_text_to_the_surviving_scope() {
    for depth in 1..=3 {
        let ui = neocogi_ui::nodes::bind_builtin_nodes(runtime()).unwrap();
        let owner = ui.widgets().textbox_unobserved("").unwrap();
        let editor = owner.handle();
        let window = ui.window("nested focus", RECT, owner).unwrap();
        key(&ui, KeyCode::Tab);
        ui.update(SIZE).unwrap();
        let menus = ui.menus().unwrap();
        let mut builders = vec![menus.create("Commands").unwrap()];
        for index in 1..depth {
            let child = builders.last().unwrap().submenu(&format!("Nested {index}")).unwrap();
            builders.push(child);
        }
        builders.last().unwrap().item("Leaf").unwrap();
        let mut popup = menus.popup(window.as_com_ref(), &builders[0]).unwrap();
        drop(builders);
        for partial in [false, true] {
            popup.show_at(RECT).unwrap();
            key(&ui, KeyCode::ArrowDown);
            for _ in 1..depth {
                key(&ui, KeyCode::ArrowRight);
            }
            ui.update(SIZE).unwrap();
            if partial && depth > 1 {
                key(&ui, KeyCode::ArrowLeft);
                ui.input(UiInput::Text, "CONSUMED").unwrap();
                ui.update(SIZE).unwrap();
                assert_eq!(editor.text().unwrap(), "FIRST");
                for _ in 1..depth {
                    key(&ui, KeyCode::Escape);
                }
            } else {
                popup.hide().unwrap();
            }
            ui.input(UiInput::Text, if partial { "SECOND" } else { "FIRST" }).unwrap();
            ui.update(SIZE).unwrap();
            assert_eq!(editor.text().unwrap(), if partial { "FIRSTSECOND" } else { "FIRST" }, "depth={depth}");
        }
        popup.close().unwrap();
        window.close().unwrap();
    }
}

/// An application callback hides a still-active popup before the next queued text.
#[test]
fn callback_hide_preserves_the_first_queued_text_after_popup_activation() {
    let ui = neocogi_ui::nodes::bind_builtin_nodes(runtime()).unwrap();
    let owner = ui.widgets().textbox_unobserved("").unwrap();
    let editor = owner.handle();
    let window = ui.window("callback focus", RECT, owner).unwrap();
    key(&ui, KeyCode::Tab);
    ui.update(SIZE).unwrap();
    let (sender, receiver) = std::sync::mpsc::channel();
    let events = neocogi_ui::test_event_queue!(neocogi_ui::ButtonAbi, (), sender, |_queue, _origin, event| {
        event.query_ref::<neocogi_ui::IActivatedEventAbi>()?;
    })
    .unwrap();
    let button = ui.widgets().button_with_queue("Hide active popup", Some(events.as_com_ref())).unwrap();
    let mut popup = ui.popup(window.as_com_ref(), "Actions", button).unwrap();
    popup.show_at(RECT).unwrap();
    ui.update(SIZE).unwrap();
    let app = TestApp::new((popup, 0), receiver, |state, ()| {
        assert_eq!(state.0.visibility()?, Visibility::Visible);
        state.0.hide()?;
        state.1 += 1;
        Ok(())
    });
    key(&ui, KeyCode::Enter);
    ui.input(UiInput::Text, "FIRST").unwrap();
    ui.update_with_events(SIZE, app.as_interface()).unwrap();
    assert_eq!(app.state().1, 1);
    assert_eq!(editor.text().unwrap(), "FIRST");
}

/// Popup ownership, frontmost order and each window's remembered editor are distinct facts.
#[test]
fn nested_hide_restores_its_owner_and_ineligible_owners_fall_back_on_the_first_text() {
    for policy in ["front-other", "hidden", "disabled", "modal"] {
        let ui = neocogi_ui::nodes::bind_builtin_nodes(runtime()).unwrap();
        let owner = ui.widgets().textbox_unobserved("").unwrap();
        let other_text = owner.handle();
        let other = ui.window("other", Rect { x: 430, y: 20, width: 180, height: 200 }, owner).unwrap();
        key(&ui, KeyCode::Tab);
        ui.update(SIZE).unwrap();
        let owner = ui.widgets().textbox_unobserved("").unwrap();
        let owner_text = owner.handle();
        let window = ui.window("owner", RECT, owner).unwrap();
        // Select the owner editor without replacing the other window's remembered focus.
        for event in [
            UiInput::Pointer(Point { x: 60, y: 100 }),
            UiInput::Down {
                position: Point { x: 60, y: 100 },
                buttons: 1,
            },
            UiInput::Up {
                position: Point { x: 60, y: 100 },
                buttons: 1,
            },
        ] {
            ui.input(event, "").unwrap();
        }
        ui.update(SIZE).unwrap();
        let menus = ui.menus().unwrap();
        let root = menus.create("Commands").unwrap();
        let child = root.submenu("Nested").unwrap();
        child.submenu("Deep").unwrap().item("Leaf").unwrap();
        let mut popup = menus.popup(window.as_com_ref(), &root).unwrap();
        popup.show_at(RECT).unwrap();
        key(&ui, KeyCode::ArrowDown);
        key(&ui, KeyCode::ArrowRight);
        key(&ui, KeyCode::ArrowRight);
        ui.update(SIZE).unwrap();
        other.raise().unwrap();
        let mut modal_text = None;
        let mut modal = None;
        match policy {
            "hidden" => window.set_visibility(Visibility::Hidden).unwrap(),
            "disabled" => {
                window.set_options(WindowOptions::DISABLED).unwrap();
            }
            "modal" => {
                let owner = ui.widgets().textbox_unobserved("").unwrap();
                modal_text = Some(owner.handle());
                let dialog = ui
                    .dialog(other.as_com_ref(), "modal", Rect { x: 430, y: 20, width: 180, height: 200 }, owner)
                    .unwrap();
                dialog.set_visibility(Visibility::Visible).unwrap();
                key(&ui, KeyCode::Tab);
                ui.update(SIZE).unwrap();
                modal = Some(dialog);
            }
            _ => (),
        }
        popup.hide().unwrap();
        ui.input(UiInput::Text, "FIRST").unwrap();
        ui.update(SIZE).unwrap();
        assert_eq!(owner_text.text().unwrap(), if policy == "front-other" { "FIRST" } else { "" }, "{policy}");
        assert_eq!(
            other_text.text().unwrap(),
            if matches!(policy, "hidden" | "disabled") { "FIRST" } else { "" },
            "{policy}"
        );
        if let Some(text) = modal_text {
            assert_eq!(text.text().unwrap(), "FIRST");
        }
        drop(modal);
    }
}

/// Queues a concrete key including the function-key number carried by FUNCTION.
fn key(ui: &neocogi_ui::Ui, key: KeyCode) {
    ui.input(
        UiInput::Key(KeyStroke {
            key,
            action: neocogi_ui::KeyAction::Pressed { repeat: false },
            modifiers: 0,
        }),
        "",
    )
    .unwrap();
}

/// A declared nested menu transfers once, remains compact, and edits/submits its same native records after builder destruction.
#[test]
fn nested_menu_keyboard_navigation_and_live_item_edits_use_the_authoritative_forest() {
    let ui = neocogi_ui::nodes::bind_builtin_nodes(runtime()).unwrap();
    let editor_owner = ui.widgets().textbox_unobserved("").unwrap();
    let editor = editor_owner.handle();
    let window = ui.window("menus", RECT, editor_owner).unwrap();
    let menus = ui.menus().unwrap();
    let bar = menus.create("").unwrap();
    let file = bar.submenu("File").unwrap();
    let (session_tx, session) = std::sync::mpsc::channel();
    let disabled = file
        .item_with_queue(
            "Disabled",
            Some(
                neocogi_ui::test_event_queue!(neocogi_ui::IMenuItemAbi, String, session_tx.clone(), |_queue, _origin, event| {
                    event.query_ref::<neocogi_ui::IActivatedEventAbi>()?;
                    "disabled-command".to_owned()
                })
                .unwrap()
                .as_com_ref(),
            ),
        )
        .unwrap();
    disabled.set_enabled(false).unwrap();
    file.separator().unwrap();
    let recent = file.submenu("Recent").unwrap();
    let item = recent
        .item_with_queue(
            "old",
            Some(
                neocogi_ui::test_event_queue!(neocogi_ui::IMenuItemAbi, String, session_tx.clone(), |_queue, _origin, event| {
                    event.query_ref::<neocogi_ui::IActivatedEventAbi>()?;
                    "menu-command".to_owned()
                })
                .unwrap()
                .as_com_ref(),
            ),
        )
        .unwrap();
    item.set_label("Before mount").unwrap();
    item.set_mark(MenuMark::Checked { checked: false }).unwrap();
    item.set_shortcut_hint("Ctrl+O").unwrap();
    item.set_font_role(FontRole::Body).unwrap();
    assert_eq!(item.mark().unwrap(), MenuMark::Checked { checked: false });
    assert_eq!(item.shortcut_hint().unwrap(), "Ctrl+O");
    let mut labels = Vec::new();
    menus.attach_bar(window.as_com_ref(), &bar).unwrap();
    assert_eq!(bar.separator(), Err(UI_E_CLOSED));
    drop(bar);
    drop(file);
    drop(recent);
    item.set_label("Captured").unwrap();
    key(&ui, KeyCode::Tab);
    key(&ui, KeyCode::Function(10));
    key(&ui, KeyCode::ArrowDown);
    key(&ui, KeyCode::ArrowRight);
    key(&ui, KeyCode::Enter);
    ui.input(UiInput::Text, "focus restored").unwrap();
    ui.update(SIZE).unwrap();
    item.set_label("Later").unwrap();
    // Moving the menu into the runtime preserves the original consumer slot.
    // Neither a queued snapshot nor the mapping for future activations is replaced.
    labels.extend(session.try_iter());
    assert_eq!(&labels, &["menu-command"]);
    assert_eq!(editor.text().unwrap(), "focus restored");
    for code in [KeyCode::Function(10), KeyCode::ArrowDown, KeyCode::ArrowRight, KeyCode::Enter] {
        key(&ui, code);
    }
    let app = TestApp::new(labels, session, |state, message| {
        state.push(message);
        Ok(())
    });
    ui.update_with_events(SIZE, app.as_interface()).unwrap();
    assert_eq!(&*app.state(), &["menu-command", "menu-command"]);
    let (renderer, _) = renderer();
    ui.render(SIZE, renderer.as_com_ref()).unwrap();
    item.set_shortcut_hint("A longer accelerator").unwrap();
    assert_eq!(ui.render(SIZE, renderer.as_com_ref()).map_err(|error| error.status), Err(UI_E_UPDATE_REQUIRED));
    window.close().unwrap();
    assert_eq!(item.label(), Err(UI_E_CLOSED));
}

/// Wrong owner, invalid bar shape and duplicate bar attachment reject before consuming a candidate declaration.
#[test]
fn failed_menu_attachment_preserves_unique_declaration_and_scoped_builder_authority() {
    let ui = neocogi_ui::nodes::bind_builtin_nodes(runtime()).unwrap();
    let foreign = neocogi_ui::nodes::bind_builtin_nodes(runtime()).unwrap();
    let first = ui.widgets().button_unobserved("first").unwrap();
    let window = ui.window("first", RECT, first).unwrap();
    let second = foreign.widgets().button_unobserved("second").unwrap();
    let foreign_window = foreign.window("second", RECT, second).unwrap();
    let menus = ui.menus().unwrap();
    let bar = menus.create("bar").unwrap();
    let child = bar.submenu("File").unwrap();
    assert_eq!(menus.attach_bar(foreign_window.as_com_ref(), &bar), Err(UI_E_OWNER));
    child.item("still appendable").unwrap();
    assert_eq!(menus.attach_bar(window.as_com_ref(), &child), Err(UI_E_OWNER));
    menus.attach_bar(window.as_com_ref(), &bar).unwrap();
    let candidate = menus.create("popup").unwrap();
    let item = candidate.item("Action").unwrap();
    assert_eq!(menus.attach_bar(window.as_com_ref(), &candidate), Err(UI_E_POLICY));
    item.set_label("Still owned").unwrap();
    let mut popup = menus.popup(window.as_com_ref(), &candidate).unwrap();
    assert_eq!(candidate.separator(), Err(UI_E_CLOSED));
    popup.close().unwrap();
    assert_eq!(item.label(), Err(UI_E_CLOSED));
    let malformed = menus.create("not a bar").unwrap();
    let abandoned = malformed.item("wrong root row").unwrap();
    let other = ui.widgets().button_unobserved("other").unwrap();
    let other_window = ui.window("other", RECT, other).unwrap();
    assert_eq!(menus.attach_bar(other_window.as_com_ref(), &malformed), Err(NC_E_INVALID_ARG));
    assert_eq!(abandoned.label().unwrap(), "wrong root row");
    let scoped = malformed.submenu("abandoned child").unwrap();
    drop(malformed);
    assert_eq!(scoped.separator(), Err(UI_E_CLOSED));
    assert_eq!(abandoned.label(), Err(UI_E_CLOSED));
    window.close().unwrap();
    other_window.close().unwrap();
    foreign_window.close().unwrap();
}

/// Standalone menus remain ordinary popup capabilities with the same nested keyboard, dismissal and source lifetime policy.
#[test]
fn standalone_menu_popup_submits_then_hides_without_destroying_its_live_rows() {
    let ui = neocogi_ui::nodes::bind_builtin_nodes(runtime()).unwrap();
    let body = ui.widgets().button_unobserved("body").unwrap();
    let window = ui.window("popup menu", RECT, body).unwrap();
    let menus = ui.menus().unwrap();
    let builder = menus.create("Actions").unwrap();
    let (session_tx, session) = std::sync::mpsc::channel();
    let item = builder
        .item_with_queue(
            "Run",
            Some(
                neocogi_ui::test_event_queue!(neocogi_ui::IMenuItemAbi, String, session_tx.clone(), |_queue, _origin, event| {
                    event.query_ref::<neocogi_ui::IActivatedEventAbi>()?;
                    "Run".to_owned()
                })
                .unwrap()
                .as_com_ref(),
            ),
        )
        .unwrap();
    let mut popup = menus.popup(window.as_com_ref(), &builder).unwrap();
    let labels = Vec::new();
    popup.show_at(Rect { x: 60, y: 60, width: 20, height: 20 }).unwrap();
    // Standalone menus begin without row selection; Down selects their first actionable row.
    key(&ui, KeyCode::ArrowDown);
    key(&ui, KeyCode::Enter);
    let app = TestApp::new(labels, session, |state, message| {
        state.push(message);
        Ok(())
    });
    ui.update_with_events(SIZE, app.as_interface()).unwrap();
    assert_eq!(&*app.state(), &["Run"]);
    assert_eq!(popup.visibility(), Ok(Visibility::Hidden));
    assert_eq!(item.label().unwrap(), "Run");
    popup.close().unwrap();
    assert_eq!(item.label(), Err(UI_E_CLOSED));
    window.close().unwrap();
}

/// All command branches spend the same bounded tree budget, and rejection leaves builders usable.
#[test]
fn command_menus_enforce_total_entry_and_depth_limits_through_com() {
    let runtime = runtime();
    let ui = neocogi_ui::nodes::bind_builtin_nodes(runtime.clone()).unwrap();
    let owner = mount(&runtime, button(&runtime).query().unwrap()).unwrap();
    let menus = ui.menus().unwrap();
    let root = menus.create("entry limit").unwrap();
    for _ in 0..63 {
        root.separator().unwrap();
    }
    let branch = root.submenu("the 64th entry").unwrap();
    assert_eq!(root.separator(), Err(neocogi_com::NC_E_OUT_OF_RANGE));
    assert_eq!(root.item("overflow").err(), Some(neocogi_com::NC_E_OUT_OF_RANGE));
    assert_eq!(branch.item("overflow in descendant").err(), Some(neocogi_com::NC_E_OUT_OF_RANGE));
    let mut popup = menus.popup(owner.as_com_ref(), &root).unwrap();
    popup.show_at(Rect { x: 0, y: 0, width: 120, height: 1 }).unwrap();
    ui.update(SIZE).unwrap();
    popup.hide().unwrap();

    let root = menus.create("depth limit").unwrap();
    let mut deepest = root.clone();
    for _ in 1..neocogi_ui::MAX_MENU_DEPTH {
        deepest = deepest.submenu("branch").unwrap();
    }
    assert_eq!(deepest.submenu("ninth level").err(), Some(neocogi_com::NC_E_OUT_OF_RANGE));
    deepest.item("still a valid eighth-level item").unwrap();
    root.item("root remains usable").unwrap();
    menus.popup(owner.as_com_ref(), &root).unwrap();
}

/// Choice creation/replacement and depth admission share the same exact limits as command menus.
#[test]
fn choice_menu_limits_preserve_the_previous_model_and_selection() {
    let runtime = runtime();
    let owner = mount(&runtime, button(&runtime).query().unwrap()).unwrap();
    let service = runtime.query::<IChoiceMenusAbi>().unwrap();
    let label = MenuTextRange { first: 0, count: 1 };
    let entries: Vec<_> = (0..64).map(|id| ChoiceMenuEntry::Choice { id, label }).collect();
    let mut menu = service
        .create_choice_menu(
            owner.as_com_ref(),
            NcCopyFromBufferOffset::new(0, b"a"),
            NcCopyFromBufferOffset::new(0, &entries),
            ChoiceSelection::Selected(63),
            neocogi_com::NcValueRef::new(&EventBinding::Unobserved).unwrap(),
        )
        .unwrap();
    let mut oversized = entries.clone();
    oversized.push(ChoiceMenuEntry::Choice { id: 64, label });
    assert_eq!(
        service
            .create_choice_menu(
                owner.as_com_ref(),
                NcCopyFromBufferOffset::new(0, b"a"),
                NcCopyFromBufferOffset::new(0, &oversized),
                ChoiceSelection::None,
                neocogi_com::NcValueRef::new(&EventBinding::Unobserved).unwrap()
            )
            .err(),
        Some(neocogi_com::NC_E_OUT_OF_RANGE)
    );
    assert_eq!(
        menu.replace(
            NcCopyFromBufferOffset::new(0, b"a"),
            NcCopyFromBufferOffset::new(0, &oversized),
            ChoiceSelection::None
        ),
        Err(neocogi_com::NC_E_OUT_OF_RANGE)
    );
    assert_eq!(menu.choice_count().unwrap(), 64);
    assert_eq!(menu.selection().unwrap(), ChoiceSelection::Selected(63));
    for branches in [7, 8] {
        let mut hierarchy = vec![ChoiceMenuEntry::Submenu { label, children: 1 }; branches];
        hierarchy.push(ChoiceMenuEntry::Choice { id: 42, label });
        let result = menu.replace(
            NcCopyFromBufferOffset::new(0, b"a"),
            NcCopyFromBufferOffset::new(0, &hierarchy),
            ChoiceSelection::Selected(42),
        );
        if branches == 7 {
            result.unwrap();
        } else {
            assert_eq!(result, Err(neocogi_com::NC_E_OUT_OF_RANGE));
        }
        assert_eq!(menu.choice_count().unwrap(), 1);
        assert_eq!(menu.selection().unwrap(), ChoiceSelection::Selected(42));
    }
}
