//! Runtime construction boundary for complete combo-box composites.

use super::*;
use crate::combo_box::{ComboState, DEFAULT_COMBO_PLACEHOLDER, choice_menu, copied_menu, decoded_selection};
use neocogi_ui_core_contracts::ChoiceMenuEntryAbi;
use neocogi_ui_nodes_contracts::{ComboAbi, ComboSelectionAbi};

/// Validates the complete composite before registering its private selection-menu popup.
pub(crate) fn create_combo_box(
    storage: &Rc<RuntimeStorage>,
    parent: ComRef<'_, IWindowAbi>,
    menu_text: NcCopyFromBufferOffset<'_, u8>,
    entries: NcCopyFromBufferOffset<'_, ChoiceMenuEntryAbi>,
    selection: ComboSelectionAbi,
) -> Result<ComPtr<ComboAbi>, NcStatus> {
    storage.open()?;
    storage.factory.host.check_mutation()?;

    // Copy every foreign input before any manager mutation, leaving validation failure atomic.
    let parent = storage.parent(parent)?;
    let (menu_text, entries) = copied_menu(menu_text, entries)?;
    let selection = decoded_selection(selection)?;
    let state = ComboState::create(menu_text, entries, selection, DEFAULT_COMBO_PLACEHOLDER.to_owned())?;
    let combo = crate::Combo::create_com(storage.factory.host.clone(), storage.factory.resources.clone(), state.clone())?;
    let menu = choice_menu(&state)?;
    let revision = state.revision()?;

    // Register the compact choice menu only after the entire foreign model was validated and
    // copied. The root forest owns generic menu rows, popup, and parent relationships only.
    let mut context = storage.context()?;
    storage.prune_mounts(&context.window_manager);
    context.window_manager.window_visible(&parent).map_err(super::windows::status)?;
    let popup = context
        .window_manager
        .create_choice_popup(&parent, menu, selection)
        .expect("authenticated popup owner cannot disappear during an exclusive transaction");
    context.register_combo_box(state, popup, revision);
    storage.pending.set(true);
    Ok(combo)
}
