//! Runtime construction boundary for complete combo-box composites.

use super::*;
use crate::{
    combo_box::{ComboPopup, ComboState, copied_items, decoded_selection},
    window_manager::WindowOption,
};
use neocogi_ui_core_contracts::{ComboAbi, ComboItemRange, ComboSelectionIndexAbi, IComboBoxes};

impl IComboBoxes for Runtime {
    /// Validates the complete initial model before registering its private popup or returning a header.
    fn create_combo_box(
        &self,
        parent: ComRef<'_, IWindowAbi>,
        placeholder: NcCopyFromBufferOffset<'_, u8>,
        item_text: NcCopyFromBufferOffset<'_, u8>,
        ranges: NcCopyFromBufferOffset<'_, ComboItemRange>,
        selection: ComboSelectionIndexAbi,
    ) -> Result<ComPtr<ComboAbi>, NcStatus> {
        let storage = &self.storage;
        storage.open()?;
        storage.factory.host.check_mutation()?;

        // Copy every foreign input before any manager mutation, leaving validation failure atomic.
        let parent = storage.parent(parent)?;
        let placeholder = text(placeholder)?.to_owned();
        let items = copied_items(item_text, ranges)?;
        let selection = decoded_selection(selection)?;
        let state = ComboState::create(items, selection, placeholder)?;
        let combo = crate::Combo::create_com(storage.factory.host.clone(), storage.factory.resources.clone(), state.clone())?;
        let (popup_widget, presentation) = ComboPopup::create(storage.factory.host.clone(), storage.factory.resources.clone(), state.clone())?;

        // Mount the private presentation as an ordinary widget popup. The root forest owns only
        // generic node, popup, and parent relationships; combo semantics stop at this module.
        let mut context = storage.context()?;
        storage.prune_mounts(&context.window_manager);
        context.window_manager.window_visible(&parent).map_err(super::windows::status)?;
        let node = storage.factory.mount_node(popup_widget.as_com_ref(), None)?;
        let popup = context
            .window_manager
            .create_popup(&parent, "", node)
            .expect("authenticated popup owner cannot disappear during an exclusive transaction");
        context
            .window_manager
            .set_popup_options(
                &popup,
                WindowOption::FRAME | WindowOption::NO_PADDING | WindowOption::NO_TITLE | WindowOption::NO_RESIZE | WindowOption::AUTO_HEIGHT,
            )
            .expect("newly registered popup must accept its presentation options");
        context.register_combo_box(state, popup, presentation);
        storage.pending.set(true);
        Ok(combo)
    }
}
