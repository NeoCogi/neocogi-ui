//! The static generic combo adapter coordinates a real COM popup through ordinary typed events.
use super::*;
use microui_redux::{EventHandler, EventSession};

/// All application list values and coordination capabilities remain local to the caller.
struct Selection {
    /// Actual inherited combo widget, never a copy of its semantic state.
    combo: microui_redux::Combo,
    /// Actual manager-owned popup created independently from its header.
    popup: ComPtr<IPopupAbi>,
    /// Application-only generic list data resolved before any selection crosses COM.
    items: Vec<String>,
    /// Captured changed events retained to verify immutable selection values.
    changes: Vec<microui_redux::events::ComboChanged>,
}
/// Header coordinator uses the captured screen anchor at a context-free callback boundary.
struct CoordinatePopup;
impl EventHandler<Selection, microui_redux::events::ComboSubmitted> for CoordinatePopup {
    /// Ordinary show/hide operations retain the window manager's existing popup and keyboard policy.
    fn handle(&mut self, state: &mut Selection, event: &microui_redux::events::ComboSubmitted) -> Result<(), NcStatus> {
        if event.open { state.popup.show(event.anchor) } else { state.popup.hide() }
    }
}
/// One concrete row action owns its local list index rather than converting labels back to data.
struct SelectRow(usize);
impl EventHandler<Selection, microui_redux::events::Activation> for SelectRow {
    /// Commits the actual row's selection, then closes its actual popup before subsequent input.
    fn handle(&mut self, state: &mut Selection, _event: &microui_redux::events::Activation) -> Result<(), NcStatus> {
        state.combo.select(self.0, &state.items)?;
        state.popup.hide()
    }
}
/// Dismissal synchronizes semantic intent without emitting another header activation.
struct DismissPopup;
impl EventHandler<Selection, microui_redux::events::PopupDismissed> for DismissPopup {
    /// Escape, pointer dismissal and row selection converge on the same silent state setter.
    fn handle(&mut self, state: &mut Selection, _event: &microui_redux::events::PopupDismissed) -> Result<(), NcStatus> {
        state.combo.set_open(false)
    }
}
/// Changed delivery records the submitted snapshot independently of later programmatic label edits.
struct RecordSelection;
impl EventHandler<Selection, microui_redux::events::ComboChanged> for RecordSelection {
    /// Owns the captured index and label without reading current mutable header data.
    fn handle(&mut self, state: &mut Selection, event: &microui_redux::events::ComboChanged) -> Result<(), NcStatus> {
        state.changes.push(event.clone());
        Ok(())
    }
}

/// Keyboard opening, popup routing, typed selection and dismissal use the same inherited widget allocation.
#[test]
fn combo_coordinates_real_popup_and_keeps_generic_list_data_in_the_static_caller() {
    let ui = microui_redux::Ui::from_interface(runtime()).unwrap();
    let combo = ui.widgets().combo().unwrap();
    let items = vec!["first".to_owned(), "second".to_owned()];
    combo.update_items(&items).unwrap();
    let window = ui.window("combo", RECT, &combo).unwrap();
    let row = ui.widgets().list_item("second").unwrap();
    let popup = ui.popup(window.as_com_ref(), "choice", &row).unwrap();
    let session = EventSession::new(Selection {
        combo: combo.clone(),
        popup: popup.clone(),
        items,
        changes: Vec::new(),
    });
    session.subscribe_combo_popup(&combo, CoordinatePopup).unwrap();
    session.subscribe_activation(&row, SelectRow(1)).unwrap();
    session.subscribe_combo_changed(&combo, RecordSelection).unwrap();
    session.subscribe_popup(popup.as_com_ref(), DismissPopup).unwrap();
    combo.set_selection(0, "silent").unwrap();
    ui.update_with_events(SIZE, &session).unwrap();
    assert!(session.state().unwrap().changes.is_empty());
    for key in [KeyCode::TAB, KeyCode::ENTER, KeyCode::SPACE] {
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
    ui.update_with_events(SIZE, &session).unwrap();
    assert_eq!(combo.selected(), Ok(1));
    assert_eq!(combo.label().unwrap(), "second");
    assert_eq!(combo.is_open(), Ok(false));
    assert_eq!(popup.visibility(), Ok(Visibility::HIDDEN));
    assert_eq!(
        session.state().unwrap().changes,
        [microui_redux::events::ComboChanged { selected: 1, label: "second".to_owned() }]
    );
    let (renderer, _) = renderer();
    ui.render(SIZE, renderer.as_com_ref()).unwrap();
    combo.update_items(&["shorter"]).unwrap();
    assert_eq!(combo.selected(), Ok(0));
    assert_eq!(combo.label().unwrap(), "shorter");
    assert_eq!(session.dispatch(), Ok(0));
    assert_eq!(ui.render(SIZE, renderer.as_com_ref()), Err(UI_E_UPDATE_REQUIRED));
    assert_eq!(
        combo.as_interface().set_selection(7, NcCopyFromBufferOffset::new(0, &[0xff])),
        Err(NC_E_INVALID_ARG)
    );
    assert_eq!(combo.selected(), Ok(0));
    window.close().unwrap();
    assert_eq!(combo.label(), Err(UI_E_CLOSED));
}
