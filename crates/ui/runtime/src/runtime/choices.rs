//! Runtime construction boundary for core-owned hierarchical choice menus.

use super::*;
use crate::choice_menu::{ChoiceMenuState, choice_menu, copied_menu, decoded_selection};
use neocogi_ui::abi::core::{ChoiceMenuEntry, ChoiceSelection, IChoiceMenuAbi};

/// Creates the core-owned choice model and popup without constructing any presentation widget.
pub(crate) fn create_choice_menu(
    storage: &Rc<RuntimeStorage>,
    parent: ComRef<'_, IWindowAbi>,
    menu_text: NcCopyFromBufferOffset<'_, u8>,
    entries: NcCopyFromBufferOffset<'_, ChoiceMenuEntry>,
    selection: ChoiceSelection,
    queue: Option<ComPtr<neocogi_ui::abi::core::IEventReceiverAbi>>,
) -> Result<ComPtr<IChoiceMenuAbi>, NcStatus> {
    storage.open()?;
    storage.factory.host.check_mutation()?;

    // Copy every foreign input before any manager mutation, leaving validation failure atomic.
    let parent = storage.parent(parent)?;
    let definition = copied_menu(menu_text, entries)?;
    let selection = decoded_selection(selection);
    let state = ChoiceMenuState::create(definition, selection, queue)?;
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
    context.register_choice_menu(state.clone(), popup, revision);
    storage.pending.set(true);
    state.query()
}
