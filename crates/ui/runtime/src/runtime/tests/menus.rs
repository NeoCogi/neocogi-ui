//! Compact menu declarations, live row edits, and manager keyboard policy cross only concrete capabilities.
use super::*;
use neocogi_ui_support::test_support::events::TestApp;

/// Queues a concrete key including the function-key number carried by FUNCTION.
fn key(ui: &neocogi_ui::Ui, key: KeyCode, value: u32) {
    ui.input(
        UiInput::Key(KeyStroke {
            key,
            value,
            pressed: true,
            repeat: false,
            modifiers: 0,
        }),
        "",
    )
    .unwrap();
}

/// A declared nested menu transfers once, remains compact, and edits/submits its same native records after builder destruction.
#[test]
fn nested_menu_keyboard_navigation_and_live_item_edits_use_the_authoritative_forest() {
    let ui = nc_ui_nodes::bind_builtin_nodes(runtime()).unwrap();
    let editor = ui.widgets().textbox_unobserved("").unwrap();
    let window = ui.window("menus", RECT, &editor).unwrap();
    let menus = ui.menus().unwrap();
    let bar = menus.create("").unwrap();
    let file = bar.submenu("File").unwrap();
    let (session_tx, session) = std::sync::mpsc::channel();
    let disabled = file
        .item_with_queue(
            "Disabled",
            Some(
                neocogi_ui::test_event_queue!(activation, String, session_tx.clone(), |_queue, _origin, event| event.label)
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
                neocogi_ui::test_event_queue!(activation, String, session_tx.clone(), |_queue, _origin, event| event.label)
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
    key(&ui, KeyCode::Tab, 0);
    key(&ui, KeyCode::Function, 10);
    key(&ui, KeyCode::ArrowDown, 0);
    key(&ui, KeyCode::ArrowRight, 0);
    key(&ui, KeyCode::Enter, 0);
    ui.input(UiInput::Text, "focus restored").unwrap();
    ui.update(SIZE).unwrap();
    item.set_label("Later").unwrap();
    // Moving the menu into the runtime preserves the original consumer slot.
    // Neither a queued snapshot nor the mapping for future activations is replaced.
    labels.extend(session.try_iter());
    assert_eq!(&labels, &["Captured"]);
    assert_eq!(editor.text().unwrap(), "focus restored");
    for code in [KeyCode::Function, KeyCode::ArrowDown, KeyCode::ArrowRight, KeyCode::Enter] {
        key(&ui, code, if code == KeyCode::Function { 10 } else { 0 });
    }
    let app = TestApp::new(labels, session, |state, message| {
        state.push(message);
        Ok(())
    });
    ui.update_with_events(SIZE, app.as_interface()).unwrap();
    assert_eq!(&*app.state(), &["Captured", "Later"]);
    let (renderer, _) = renderer();
    ui.render(SIZE, renderer.as_com_ref()).unwrap();
    item.set_shortcut_hint("A longer accelerator").unwrap();
    assert_eq!(ui.render(SIZE, renderer.as_com_ref()), Err(UI_E_UPDATE_REQUIRED));
    window.close().unwrap();
    assert_eq!(item.label(), Err(UI_E_CLOSED));
}

/// Wrong owner, invalid bar shape and duplicate bar attachment reject before consuming a candidate declaration.
#[test]
fn failed_menu_attachment_preserves_unique_declaration_and_scoped_builder_authority() {
    let ui = nc_ui_nodes::bind_builtin_nodes(runtime()).unwrap();
    let foreign = nc_ui_nodes::bind_builtin_nodes(runtime()).unwrap();
    let first = ui.widgets().button_unobserved("first").unwrap();
    let window = ui.window("first", RECT, &first).unwrap();
    let second = foreign.widgets().button_unobserved("second").unwrap();
    let foreign_window = foreign.window("second", RECT, &second).unwrap();
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
    let other_window = ui.window("other", RECT, &other).unwrap();
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
    let ui = nc_ui_nodes::bind_builtin_nodes(runtime()).unwrap();
    let body = ui.widgets().button_unobserved("body").unwrap();
    let window = ui.window("popup menu", RECT, &body).unwrap();
    let menus = ui.menus().unwrap();
    let builder = menus.create("Actions").unwrap();
    let (session_tx, session) = std::sync::mpsc::channel();
    let item = builder
        .item_with_queue(
            "Run",
            Some(
                neocogi_ui::test_event_queue!(activation, String, session_tx.clone(), |_queue, _origin, event| event.label)
                    .unwrap()
                    .as_com_ref(),
            ),
        )
        .unwrap();
    let mut popup = menus.popup(window.as_com_ref(), &builder).unwrap();
    let labels = Vec::new();
    popup.show_at(Rect { x: 60, y: 60, width: 20, height: 20 }).unwrap();
    // Standalone menus begin without row selection; Down selects their first actionable row.
    key(&ui, KeyCode::ArrowDown, 0);
    key(&ui, KeyCode::Enter, 0);
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
