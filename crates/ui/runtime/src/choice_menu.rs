//! Core choice-menu model and orchestration over the shared compact menu surface.
//!
//! The root forest stores only generic menu surfaces and popup ownership. This module connects
//! one public header, one authoritative typed state, and one private choice-popup definition.

use crate::menu::{Menu, MenuEntry};
use crate::window_manager::{PopupHandle, SurfaceMutationError, WindowManager};
use neocogi_com::{
    ComPtr, INcStringAbi, INcStringComPtrExt, NcCopyFromBufferOffset, NcCopyToBufferOffset, NcStatus, NcString, NC_E_INVALID_ARG, com_int_enum, com_interface,
    com_object,
};
use neocogi_ui_core_contracts::{
    ChoiceMenuEntry, ChoiceMenuEntryAbi, ChoiceSelection, ChoiceSelectionAbi, IChoiceMenu, IChoiceMenuAbi, IChoiceMenuInterfacesFor, MenuTextRange, Rect,
};

/// Maximum accepted branch nesting, keeping foreign preorder input bounded on the native stack.
const MAX_CHOICE_MENU_DEPTH: usize = 32;

com_int_enum! {
    /// Header intent understood only by the private choice menu state machine.
    pub(crate) enum ChoicePopupIntent: u8 {
        /// Toggle the current popup intent after ordinary pointer, Enter, or Space activation.
        Toggle = 0,
        /// Request opening without closing an already-open popup.
        Open = 1,
        /// Request closure without opening a currently closed popup.
        Close = 2,
    }
}

com_int_enum! {
    /// Concrete operation returned to the runtime-level choice menu coordinator.
    pub(crate) enum ChoicePopupAction: u8 {
        /// Popup visibility already agrees with semantic intent.
        None = 0,
        /// Prepare and show the ordinary widget popup at the retained anchor.
        Open = 1,
        /// Hide the ordinary widget popup without changing selection.
        Close = 2,
        /// Permanently remove the private popup because its header retired.
        Remove = 3,
    }
}

/// Complete private popup lifecycle; every transition is centralized in [`ChoiceMenuState`].
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
enum ChoicePopupLifecycle {
    /// The generic popup definition exists but is not in the active popup path.
    Closed,
    /// Header interaction requested opening at the separately retained anchor.
    OpenRequested,
    /// The generic popup is expected to be present in the active popup path.
    Open,
    /// Header or choice interaction requested ordinary popup dismissal.
    CloseRequested,
    /// Header closure requested permanent removal of the private popup definition.
    RemoveRequested,
    /// The runtime coordinator removed the popup and will no longer consult this state.
    Detached,
}

#[com_interface(
    auto_guid = "neocogi.ui.runtime.internal.ChoiceMenuState:v1",
    assigned_signature = b"00000000000000000000000000000051"
)]
/// Runtime-private typed access to one authoritative choice menu model.
pub(crate) trait IChoiceMenuState {
    /// Returns a revision changed by hierarchy labels or committed selection.
    fn revision(&self) -> Result<u64, NcStatus>;
    /// Returns the explicit optional committed leaf identity.
    fn selection(&self) -> Result<ChoiceSelectionAbi, NcStatus>;
    /// Copies the hierarchy's shared UTF-8 storage for rebuilding its menu projection.
    fn menu_text(&self) -> Result<ComPtr<INcStringAbi>, NcStatus>;
    /// Copies a requested preorder entry range and returns the complete entry count.
    fn menu_entries(&self, output: NcCopyToBufferOffset<'_, ChoiceMenuEntryAbi>) -> Result<u32, NcStatus>;
    /// Returns the number of retained display labels.
    fn choice_count(&self) -> Result<u64, NcStatus>;
    /// Copies one exact display label after strict range validation.
    fn choice_label(&self, ordinal: u64) -> Result<ComPtr<INcStringAbi>, NcStatus>;
    /// Atomically replaces every label and the explicit optional selection.
    fn replace_menu(
        &mut self,
        menu_text: NcCopyFromBufferOffset<'_, u8>,
        entries: NcCopyFromBufferOffset<'_, ChoiceMenuEntryAbi>,
        selection: ChoiceSelectionAbi,
    ) -> Result<bool, NcStatus>;
    /// Silently replaces the optional committed selection without clamping.
    fn set_selection(&mut self, selection: ChoiceSelectionAbi) -> Result<bool, NcStatus>;
    /// Applies one typed header intent and captures a valid screen-space anchor when opening.
    fn request_popup(&mut self, intent: ChoicePopupIntent, anchor: Rect) -> Result<(), NcStatus>;
    /// Reconciles semantic lifecycle with actual generic-popup visibility.
    fn reconcile_popup(&mut self, visible: bool) -> Result<ChoicePopupAction, NcStatus>;
    /// Returns the most recently captured header-bottom anchor.
    fn popup_anchor(&self) -> Result<Rect, NcStatus>;
    /// Commits one user choice, publishes a changed snapshot, and requests popup closure.
    fn submit(&mut self, id: u64) -> Result<u64, NcStatus>;
    /// Ends event delivery and requests permanent private-popup removal.
    fn retire(&mut self) -> Result<(), NcStatus>;
}

#[com_object(interfaces(IChoiceMenuState, IChoiceMenu), ports(neocogi_ui_core_contracts::IChoiceMenu::changed = {
        Ok(self.changed_event.as_ref())
    }))]
/// Plain authoritative choice menu state mutated exclusively through `IChoiceMenuState`'s mutable receiver.
pub(crate) struct ChoiceMenuState {
    /// Shared UTF-8 storage referenced by every checked preorder entry.
    menu_text: String,
    /// Authoritative preorder hierarchy with explicit direct-child counts.
    entries: Vec<ChoiceMenuEntryAbi>,
    /// Committed stable leaf identity; `None` remains distinct from every `u64` value.
    selection: Option<u64>,
    /// Monotonic semantic revision used to invalidate transient popup candidates.
    revision: u64,
    /// Last valid screen-space anchor captured by header activation.
    popup_anchor: Rect,
    /// Explicit popup lifecycle reconciled by the runtime composition owner.
    popup: ChoicePopupLifecycle,
    /// Runtime-owned source for actual user commits only.
    changed_event: crate::Port<ChoiceSelection>,
}

impl ChoiceMenuState {
    /// Validates selection before allocating the private COM state identity.
    pub(crate) fn create(menu_text: String, entries: Vec<ChoiceMenuEntryAbi>, selection: Option<u64>) -> Result<ComPtr<IChoiceMenuStateAbi>, NcStatus> {
        validate_menu(&menu_text, &entries, selection)?;
        let state = Self {
            menu_text,
            entries,
            selection,
            revision: 0,
            popup_anchor: Rect::default(),
            popup: ChoicePopupLifecycle::Closed,
            changed_event: crate::Port::new(),
        };
        Self::from_value(state).query()
    }

    /// Advances the semantic revision after one observable model change.
    fn advance_revision(&mut self) {
        self.revision = self.revision.wrapping_add(1);
    }

    /// Applies header popup intent using exhaustive lifecycle pattern matching.
    fn apply_popup_intent(&mut self, intent: ChoicePopupIntent, anchor: Rect) -> Result<(), NcStatus> {
        if !intent.is_known() || anchor.width < 0 || anchor.height < 0 {
            return Err(NC_E_INVALID_ARG);
        }
        if choice_count(&self.entries) == 0 && intent != ChoicePopupIntent::Close {
            return Ok(());
        }
        self.popup = match (self.popup, intent) {
            (ChoicePopupLifecycle::Closed | ChoicePopupLifecycle::CloseRequested, ChoicePopupIntent::Toggle | ChoicePopupIntent::Open) => {
                self.popup_anchor = anchor;
                ChoicePopupLifecycle::OpenRequested
            }
            (ChoicePopupLifecycle::Open | ChoicePopupLifecycle::OpenRequested, ChoicePopupIntent::Toggle) => ChoicePopupLifecycle::CloseRequested,
            (ChoicePopupLifecycle::OpenRequested, ChoicePopupIntent::Open) => {
                self.popup_anchor = anchor;
                ChoicePopupLifecycle::OpenRequested
            }
            (ChoicePopupLifecycle::Open, ChoicePopupIntent::Open) => ChoicePopupLifecycle::Open,
            (ChoicePopupLifecycle::Open | ChoicePopupLifecycle::OpenRequested, ChoicePopupIntent::Close) => ChoicePopupLifecycle::CloseRequested,
            (ChoicePopupLifecycle::Closed | ChoicePopupLifecycle::CloseRequested, ChoicePopupIntent::Close) => ChoicePopupLifecycle::Closed,
            (ChoicePopupLifecycle::RemoveRequested, _) => ChoicePopupLifecycle::RemoveRequested,
            (ChoicePopupLifecycle::Detached, _) => ChoicePopupLifecycle::Detached,
            (_, _) => return Err(NC_E_INVALID_ARG),
        };
        Ok(())
    }

    /// Converts one pending lifecycle transition into a generic-popup operation.
    fn reconcile_popup_value(&mut self, visible: bool) -> ChoicePopupAction {
        let (next, action) = match (self.popup, visible) {
            (ChoicePopupLifecycle::RemoveRequested, _) => (ChoicePopupLifecycle::Detached, ChoicePopupAction::Remove),
            (ChoicePopupLifecycle::Detached, _) => (ChoicePopupLifecycle::Detached, ChoicePopupAction::None),
            (ChoicePopupLifecycle::OpenRequested, _) => (ChoicePopupLifecycle::Open, ChoicePopupAction::Open),
            (ChoicePopupLifecycle::CloseRequested, true) => (ChoicePopupLifecycle::Closed, ChoicePopupAction::Close),
            (ChoicePopupLifecycle::CloseRequested, false) => (ChoicePopupLifecycle::Closed, ChoicePopupAction::None),
            (ChoicePopupLifecycle::Open, true) => (ChoicePopupLifecycle::Open, ChoicePopupAction::None),
            (ChoicePopupLifecycle::Open, false) => (ChoicePopupLifecycle::Closed, ChoicePopupAction::None),
            (ChoicePopupLifecycle::Closed, false) => (ChoicePopupLifecycle::Closed, ChoicePopupAction::None),
            (ChoicePopupLifecycle::Closed, true) => (ChoicePopupLifecycle::Closed, ChoicePopupAction::Close),
        };
        self.popup = next;
        action
    }

    /// Requests popup closure after replacing the item list with no choices.
    fn close_if_empty(&mut self) {
        if choice_count(&self.entries) != 0 {
            return;
        }
        self.popup = match self.popup {
            ChoicePopupLifecycle::Open | ChoicePopupLifecycle::OpenRequested => ChoicePopupLifecycle::CloseRequested,
            ChoicePopupLifecycle::Closed | ChoicePopupLifecycle::CloseRequested => ChoicePopupLifecycle::Closed,
            ChoicePopupLifecycle::RemoveRequested => ChoicePopupLifecycle::RemoveRequested,
            ChoicePopupLifecycle::Detached => ChoicePopupLifecycle::Detached,
        };
    }
}

impl IChoiceMenuState for ChoiceMenuState {
    /// Reads the scalar revision directly from the authoritative state.
    fn revision(&self) -> Result<u64, NcStatus> {
        Ok(self.revision)
    }

    /// Encodes structural absence without manufacturing an in-band index sentinel.
    fn selection(&self) -> Result<ChoiceSelectionAbi, NcStatus> {
        Ok(encode_selection(self.selection))
    }

    /// Copies the one authoritative UTF-8 backing store without exposing internal allocation.
    fn menu_text(&self) -> Result<ComPtr<INcStringAbi>, NcStatus> {
        Ok(NcString::from_utf8_str(&self.menu_text))
    }

    /// Copies only the requested suffix while reporting complete preorder cardinality.
    fn menu_entries(&self, mut output: NcCopyToBufferOffset<'_, ChoiceMenuEntryAbi>) -> Result<u32, NcStatus> {
        let offset = usize::try_from(output.offset()).map_err(|_| NC_E_INVALID_ARG)?;
        let source = self.entries.get(offset..).ok_or(NC_E_INVALID_ARG)?;
        for (target, entry) in output.as_mut_slice().iter_mut().zip(source) {
            *target = *entry;
        }
        u32::try_from(self.entries.len()).map_err(|_| neocogi_com::NC_E_OUT_OF_RANGE)
    }

    /// Reports the authoritative label count without a second collection.
    fn choice_count(&self) -> Result<u64, NcStatus> {
        Ok(choice_count(&self.entries) as u64)
    }

    /// Copies one strictly checked label from the authoritative collection.
    fn choice_label(&self, ordinal: u64) -> Result<ComPtr<INcStringAbi>, NcStatus> {
        let ordinal = usize::try_from(ordinal).map_err(|_| NC_E_INVALID_ARG)?;
        let label = choice_at(&self.menu_text, &self.entries, ordinal).ok_or(NC_E_INVALID_ARG)?;
        Ok(NcString::from_utf8_str(label))
    }

    /// Validates a complete candidate before committing either labels or selection.
    fn replace_menu(
        &mut self,
        menu_text: NcCopyFromBufferOffset<'_, u8>,
        entries: NcCopyFromBufferOffset<'_, ChoiceMenuEntryAbi>,
        selection: ChoiceSelectionAbi,
    ) -> Result<bool, NcStatus> {
        let (menu_text, entries) = copied_menu(menu_text, entries)?;
        let selection = decoded_selection(selection)?;
        validate_menu(&menu_text, &entries, selection)?;
        if self.menu_text == menu_text && self.entries == entries && self.selection == selection {
            return Ok(false);
        }
        self.menu_text = menu_text;
        self.entries = entries;
        self.selection = selection;
        self.advance_revision();
        self.close_if_empty();
        Ok(true)
    }

    /// Replaces selection only after validating it against the current item sequence.
    fn set_selection(&mut self, selection: ChoiceSelectionAbi) -> Result<bool, NcStatus> {
        let selection = decoded_selection(selection)?;
        validate_selection(selection, &self.menu_text, &self.entries)?;
        if self.selection == selection {
            return Ok(false);
        }
        self.selection = selection;
        self.advance_revision();
        Ok(true)
    }

    /// Delegates every intent to the exhaustive semantic state machine.
    fn request_popup(&mut self, intent: ChoicePopupIntent, anchor: Rect) -> Result<(), NcStatus> {
        self.apply_popup_intent(intent, anchor)
    }

    /// Synchronizes manager visibility without publishing application-facing lifecycle events.
    fn reconcile_popup(&mut self, visible: bool) -> Result<ChoicePopupAction, NcStatus> {
        Ok(self.reconcile_popup_value(visible))
    }

    /// Copies the anchor captured at the same header transition as the pending open request.
    fn popup_anchor(&self) -> Result<Rect, NcStatus> {
        Ok(self.popup_anchor)
    }

    /// Commits a checked item and closes the popup even when the value was already selected.
    fn submit(&mut self, id: u64) -> Result<u64, NcStatus> {
        validate_selection(Some(id), &self.menu_text, &self.entries)?;
        let changed = self.selection != Some(id);
        if changed {
            self.selection = Some(id);
            self.advance_revision();
        }
        self.popup = match self.popup {
            ChoicePopupLifecycle::Open | ChoicePopupLifecycle::OpenRequested => ChoicePopupLifecycle::CloseRequested,
            ChoicePopupLifecycle::Closed | ChoicePopupLifecycle::CloseRequested => ChoicePopupLifecycle::Closed,
            ChoicePopupLifecycle::RemoveRequested => ChoicePopupLifecycle::RemoveRequested,
            ChoicePopupLifecycle::Detached => ChoicePopupLifecycle::Detached,
        };
        // Commit the complete popup transition before entering even a pure mapper.
        if changed {
            self.changed_event.emit(ChoiceSelection::Selected(id))?;
        }
        Ok(self.revision)
    }

    /// Makes retirement idempotent for both logical widget closure and final unmounted release.
    fn retire(&mut self) -> Result<(), NcStatus> {
        self.changed_event.close_ordinary();
        self.popup = match self.popup {
            ChoicePopupLifecycle::Detached => ChoicePopupLifecycle::Detached,
            ChoicePopupLifecycle::Closed
            | ChoicePopupLifecycle::OpenRequested
            | ChoicePopupLifecycle::Open
            | ChoicePopupLifecycle::CloseRequested
            | ChoicePopupLifecycle::RemoveRequested => ChoicePopupLifecycle::RemoveRequested,
        };
        Ok(())
    }
}

impl IChoiceMenu for ChoiceMenuState {
    /// Returns the exact optional stable identity from the authoritative menu model.
    fn selection(&self) -> Result<ChoiceSelectionAbi, NcStatus> {
        <Self as IChoiceMenuState>::selection(self)
    }

    /// Copies the selected label, using empty text only for the explicit absent-selection case.
    fn selected_label(&self) -> Result<ComPtr<INcStringAbi>, NcStatus> {
        let label = match self.selection {
            Some(id) => choice_label(&self.menu_text, &self.entries, id).ok_or(NC_E_INVALID_ARG)?,
            None => "",
        };
        Ok(NcString::from_utf8_str(label))
    }

    /// Reports selectable leaf cardinality without flattening the hierarchy.
    fn choice_count(&self) -> Result<u64, NcStatus> {
        <Self as IChoiceMenuState>::choice_count(self)
    }

    /// Copies one checked preorder leaf label.
    fn choice_label(&self, ordinal: u64) -> Result<ComPtr<INcStringAbi>, NcStatus> {
        <Self as IChoiceMenuState>::choice_label(self, ordinal)
    }

    /// Validates the complete replacement before committing hierarchy or selection.
    fn replace(
        &mut self,
        menu_text: NcCopyFromBufferOffset<'_, u8>,
        entries: NcCopyFromBufferOffset<'_, ChoiceMenuEntryAbi>,
        selection: ChoiceSelectionAbi,
    ) -> Result<(), NcStatus> {
        <Self as IChoiceMenuState>::replace_menu(self, menu_text, entries, selection).map(|_| ())
    }

    /// Silently commits one checked optional leaf identity.
    fn set_selection(&mut self, selection: ChoiceSelectionAbi) -> Result<(), NcStatus> {
        <Self as IChoiceMenuState>::set_selection(self, selection).map(|_| ())
    }

    /// Records an activation request whose popup mutation occurs after traversal returns.
    fn show_at(&mut self, anchor: Rect) -> Result<(), NcStatus> {
        <Self as IChoiceMenuState>::request_popup(self, ChoicePopupIntent::Open, anchor)
    }

    /// Records a dismissal request without destroying the reusable popup definition.
    fn hide(&mut self) -> Result<(), NcStatus> {
        <Self as IChoiceMenuState>::request_popup(self, ChoicePopupIntent::Close, Rect::default())
    }

    /// Selects the next requested lifecycle state using the existing reconciled transition table.
    fn toggle_at(&mut self, anchor: Rect) -> Result<(), NcStatus> {
        <Self as IChoiceMenuState>::request_popup(self, ChoicePopupIntent::Toggle, anchor)
    }

    /// Permanently ends event delivery and asks the reconciler to remove the popup forest branch.
    fn close(&mut self) -> Result<(), NcStatus> {
        <Self as IChoiceMenuState>::retire(self)
    }
}

/// Builds one checked recursive choice-menu snapshot from the authoritative serialized model.
pub(crate) fn choice_menu(state: &ComPtr<IChoiceMenuStateAbi>) -> Result<Menu, NcStatus> {
    let menu_text = state.menu_text()?.to_string()?;
    let count = usize::try_from(state.menu_entries(NcCopyToBufferOffset::new(0, &mut []))?).map_err(|_| NC_E_INVALID_ARG)?;
    let placeholder: ChoiceMenuEntryAbi = ChoiceMenuEntry::Choice {
        id: 0,
        label: MenuTextRange { first: 0, count: 0 },
    }
    .into();
    let mut entries = vec![placeholder; count];
    let copied = state.menu_entries(NcCopyToBufferOffset::new(0, &mut entries))?;
    if usize::try_from(copied).ok() != Some(count) {
        return Err(NC_E_INVALID_ARG);
    }
    native_menu(&menu_text, &entries)
}

/// Runtime-owned association between private choice menu state and one shared menu popup handle.
struct ChoiceMenuController {
    /// Typed semantic model shared by the header and composition coordinator.
    state: ComPtr<IChoiceMenuStateAbi>,
    /// Generic popup capability; no choice menu policy is embedded in its forest node.
    popup: PopupHandle,
    /// Last model revision copied into the compact relational menu hierarchy.
    observed_revision: u64,
}

/// Whether one controller remains useful after a reconciliation step.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
enum ControllerDisposition {
    /// Keep the controller for later header or dismissal transitions.
    Keep,
    /// Drop the controller because its header or structural owner retired.
    Remove,
}

impl ChoiceMenuController {
    /// Reconciles one semantic lifecycle against generic popup visibility and operations.
    fn reconcile(&mut self, manager: &mut WindowManager) -> Result<(ControllerDisposition, bool), NcStatus> {
        let visible = match manager.popup_visible(&self.popup) {
            Ok(visible) => visible,
            Err(SurfaceMutationError::UnknownPopup) => {
                self.state.retire()?;
                return Ok((ControllerDisposition::Remove, false));
            }
            Err(error) => return Err(surface_status(error)),
        };
        // Menu routing only stages a stable leaf identity. Commit it here, after the root forest
        // released input borrows and before application event sinks can observe the new value.
        if let Some(choice) = manager.take_choice_submission(&self.popup).map_err(surface_status)? {
            self.state.submit(choice)?;
        }
        let revision = self.state.revision()?;
        let presentation_changed = revision != self.observed_revision;
        if presentation_changed {
            let menu = choice_menu(&self.state)?;
            let committed = decoded_selection(self.state.selection()?)?;
            manager.replace_choice_popup(&self.popup, menu, committed).map_err(surface_status)?;
            self.observed_revision = revision;
        }
        let action = self.state.reconcile_popup(visible)?;
        match action {
            ChoicePopupAction::None => Ok((ControllerDisposition::Keep, presentation_changed)),
            ChoicePopupAction::Open => {
                let anchor = crate::boundary::local_rect(self.state.popup_anchor()?);
                manager.show_choice_popup_at(&self.popup, anchor).map_err(surface_status)?;
                Ok((ControllerDisposition::Keep, true))
            }
            ChoicePopupAction::Close => {
                manager.hide_popup(&self.popup).map_err(surface_status)?;
                Ok((ControllerDisposition::Keep, true))
            }
            ChoicePopupAction::Remove => {
                match manager.destroy_popup(&self.popup) {
                    Ok(()) | Err(SurfaceMutationError::UnknownPopup) => {}
                    Err(error) => return Err(surface_status(error)),
                }
                Ok((ControllerDisposition::Remove, true))
            }
            _ => Err(NC_E_INVALID_ARG),
        }
    }
}

/// Composition-layer owner for choice menu controllers; mutation requires the runtime context's `&mut`.
#[derive(Default)]
pub(crate) struct ChoiceMenuManager {
    /// Live controller set, independent from generic root storage and traversal.
    controllers: Vec<ChoiceMenuController>,
}

impl ChoiceMenuManager {
    /// Registers one fully constructed controller after its generic menu popup exists.
    pub(crate) fn register(&mut self, state: ComPtr<IChoiceMenuStateAbi>, popup: PopupHandle, observed_revision: u64) {
        self.controllers.push(ChoiceMenuController { state, popup, observed_revision });
    }

    /// Reconciles all controllers using exclusive manager and coordinator access.
    pub(crate) fn reconcile(&mut self, manager: &mut WindowManager) -> Result<bool, NcStatus> {
        let mut changed = false;
        let mut index = 0;
        while index < self.controllers.len() {
            let (disposition, controller_changed) = self.controllers[index].reconcile(manager)?;
            changed |= controller_changed;
            match disposition {
                ControllerDisposition::Keep => index += 1,
                ControllerDisposition::Remove => {
                    self.controllers.remove(index);
                }
            }
        }
        Ok(changed)
    }

    /// Drops private controller capabilities after runtime closure removed every root family.
    pub(crate) fn clear(&mut self) {
        self.controllers.clear();
    }
}

/// Converts generic surface policy failures at this private composition boundary.
fn surface_status(error: SurfaceMutationError) -> NcStatus {
    match error {
        SurfaceMutationError::UnknownWindow | SurfaceMutationError::UnknownPopup => neocogi_ui_core_contracts::UI_E_CLOSED,
        SurfaceMutationError::InvalidLayer(_) => NC_E_INVALID_ARG,
        SurfaceMutationError::InvalidDialogOwner
        | SurfaceMutationError::InvalidChildWindowParent
        | SurfaceMutationError::ManagedLayer
        | SurfaceMutationError::InvalidPopupParent => neocogi_ui_core_contracts::UI_E_POLICY,
    }
}

/// Encodes optional selection with a tagged absence variant rather than a sentinel identity.
pub(crate) fn encode_selection(selection: Option<u64>) -> ChoiceSelectionAbi {
    match selection {
        Some(id) => ChoiceSelection::Selected(id),
        None => ChoiceSelection::None,
    }
    .into()
}

/// Decodes the public tagged selection and rejects unknown discriminants.
pub(crate) fn decoded_selection(selection: ChoiceSelectionAbi) -> Result<Option<u64>, NcStatus> {
    match ChoiceSelection::try_from(selection).map_err(|_| NC_E_INVALID_ARG)? {
        ChoiceSelection::None => Ok(None),
        ChoiceSelection::Selected(id) => Ok(Some(id)),
    }
}

/// Validates one optional identity against every selectable leaf in the hierarchy.
fn validate_selection(selection: Option<u64>, text: &str, entries: &[ChoiceMenuEntryAbi]) -> Result<(), NcStatus> {
    match selection {
        Some(id) if choice_label(text, entries, id).is_none() => Err(NC_E_INVALID_ARG),
        Some(_) | None => Ok(()),
    }
}

/// Validates shared UTF-8 storage and copies the complete serialized hierarchy atomically.
pub(crate) fn copied_menu(
    menu_text: NcCopyFromBufferOffset<'_, u8>,
    entries: NcCopyFromBufferOffset<'_, ChoiceMenuEntryAbi>,
) -> Result<(String, Vec<ChoiceMenuEntryAbi>), NcStatus> {
    if menu_text.offset() != 0 || entries.offset() != 0 {
        return Err(NC_E_INVALID_ARG);
    }
    let text = std::str::from_utf8(menu_text.as_slice()).map_err(|_| NC_E_INVALID_ARG)?;
    u32::try_from(text.len()).map_err(|_| neocogi_com::NC_E_OUT_OF_RANGE)?;
    let entries = entries.as_slice().to_vec();
    u32::try_from(entries.len()).map_err(|_| neocogi_com::NC_E_OUT_OF_RANGE)?;
    validate_menu(text, &entries, None)?;
    Ok((text.to_owned(), entries))
}

/// Returns one checked UTF-8 label slice from the shared model storage.
fn ranged_text(text: &str, range: MenuTextRange) -> Option<&str> {
    let first = usize::try_from(range.first).ok()?;
    let count = usize::try_from(range.count).ok()?;
    text.get(first..first.checked_add(count)?)
}

/// Counts selectable leaves without creating a parallel flattened collection.
fn choice_count(entries: &[ChoiceMenuEntryAbi]) -> usize {
    entries
        .iter()
        .filter(|entry| matches!(ChoiceMenuEntry::try_from(**entry), Ok(ChoiceMenuEntry::Choice { .. })))
        .count()
}

/// Resolves one preorder leaf ordinal directly from the authoritative serialized entries.
fn choice_at<'a>(text: &'a str, entries: &[ChoiceMenuEntryAbi], ordinal: usize) -> Option<&'a str> {
    entries
        .iter()
        .filter_map(|entry| match ChoiceMenuEntry::try_from(*entry).ok()? {
            ChoiceMenuEntry::Choice { label, .. } => ranged_text(text, label),
            ChoiceMenuEntry::Submenu { .. } => None,
        })
        .nth(ordinal)
}

/// Resolves a stable leaf identity independently from its label and hierarchy position.
fn choice_label<'a>(text: &'a str, entries: &[ChoiceMenuEntryAbi], selected: u64) -> Option<&'a str> {
    entries.iter().find_map(|entry| match ChoiceMenuEntry::try_from(*entry).ok()? {
        ChoiceMenuEntry::Choice { id, label } if id == selected => ranged_text(text, label),
        ChoiceMenuEntry::Choice { .. } | ChoiceMenuEntry::Submenu { .. } => None,
    })
}

/// Validates tags, ranges, direct-child counts, unique leaf identities, and selection together.
fn validate_menu(text: &str, entries: &[ChoiceMenuEntryAbi], selection: Option<u64>) -> Result<(), NcStatus> {
    let mut ids = std::collections::HashSet::new();
    let mut index = 0;
    while index < entries.len() {
        index = validate_node(text, entries, index, 0, &mut ids)?;
    }
    validate_selection(selection, text, entries)
}

/// Validates one preorder node and returns the first entry after all of its descendants.
fn validate_node(text: &str, entries: &[ChoiceMenuEntryAbi], index: usize, depth: usize, ids: &mut std::collections::HashSet<u64>) -> Result<usize, NcStatus> {
    let entry = ChoiceMenuEntry::try_from(*entries.get(index).ok_or(NC_E_INVALID_ARG)?).map_err(|_| NC_E_INVALID_ARG)?;
    match entry {
        ChoiceMenuEntry::Choice { id, label } => {
            ranged_text(text, label).ok_or(NC_E_INVALID_ARG)?;
            if !ids.insert(id) {
                return Err(NC_E_INVALID_ARG);
            }
            index.checked_add(1).ok_or(NC_E_INVALID_ARG)
        }
        ChoiceMenuEntry::Submenu { label, children } => {
            ranged_text(text, label).ok_or(NC_E_INVALID_ARG)?;
            if children == 0 || depth >= MAX_CHOICE_MENU_DEPTH {
                return Err(NC_E_INVALID_ARG);
            }
            let mut next = index.checked_add(1).ok_or(NC_E_INVALID_ARG)?;
            for _ in 0..children {
                next = validate_node(text, entries, next, depth + 1, ids)?;
            }
            Ok(next)
        }
    }
}

/// Converts the checked preorder model into the manager's shared recursive menu declaration.
fn native_menu(text: &str, entries: &[ChoiceMenuEntryAbi]) -> Result<Menu, NcStatus> {
    validate_menu(text, entries, None)?;
    let mut menu = Menu::new("");
    let mut index = 0;
    while index < entries.len() {
        let (entry, next) = native_entry(text, entries, index)?;
        menu.entries.push(entry);
        index = next;
    }
    Ok(menu)
}

/// Converts one node and its exact descendants without reconstructing parent indices.
fn native_entry(text: &str, entries: &[ChoiceMenuEntryAbi], index: usize) -> Result<(MenuEntry, usize), NcStatus> {
    match ChoiceMenuEntry::try_from(*entries.get(index).ok_or(NC_E_INVALID_ARG)?).map_err(|_| NC_E_INVALID_ARG)? {
        ChoiceMenuEntry::Choice { id, label } => Ok((
            MenuEntry::Choice {
                id,
                label: ranged_text(text, label).ok_or(NC_E_INVALID_ARG)?.to_owned(),
            },
            index.checked_add(1).ok_or(NC_E_INVALID_ARG)?,
        )),
        ChoiceMenuEntry::Submenu { label, children } => {
            let mut menu = Menu::new(ranged_text(text, label).ok_or(NC_E_INVALID_ARG)?);
            let mut next = index.checked_add(1).ok_or(NC_E_INVALID_ARG)?;
            for _ in 0..children {
                let (child, after) = native_entry(text, entries, next)?;
                menu.entries.push(child);
                next = after;
            }
            Ok((MenuEntry::Submenu(menu), next))
        }
    }
}

#[cfg(test)]
mod tests {
    //! Focused state-machine tests independent from generic root implementation details.

    use super::*;

    /// Creates a direct state value for exhaustive private transition tests.
    fn state(selection: Option<u64>) -> ChoiceMenuState {
        let menu_text = "AppleBananaCherry".to_owned();
        let entries = [
            (0, MenuTextRange { first: 0, count: 5 }),
            (1, MenuTextRange { first: 5, count: 6 }),
            (2, MenuTextRange { first: 11, count: 6 }),
        ]
        .map(|(id, label)| ChoiceMenuEntry::Choice { id, label }.into())
        .to_vec();
        ChoiceMenuState {
            menu_text,
            entries,
            selection,
            revision: 0,
            popup_anchor: Rect::default(),
            popup: ChoicePopupLifecycle::Closed,
            changed_event: crate::Port::new(),
        }
    }

    /// `None` and item zero remain structurally distinct in the authoritative plain state.
    #[test]
    fn optional_selection_has_no_zero_sentinel() {
        let mut state = state(None);
        assert_eq!(state.selection, None);
        IChoiceMenuState::set_selection(&mut state, encode_selection(Some(0))).unwrap();
        assert_eq!(state.selection, Some(0));
    }

    /// Opening, outside dismissal, and retirement produce explicit typed transitions.
    #[test]
    fn popup_lifecycle_reconciles_without_root_specific_state() {
        let mut state = state(Some(1));
        state
            .apply_popup_intent(ChoicePopupIntent::Open, Rect { x: 10, y: 20, width: 100, height: 1 })
            .unwrap();
        assert_eq!(state.reconcile_popup_value(false), ChoicePopupAction::Open);
        assert_eq!(state.reconcile_popup_value(true), ChoicePopupAction::None);
        assert_eq!(state.reconcile_popup_value(false), ChoicePopupAction::None);
        state.retire().unwrap();
        assert_eq!(state.reconcile_popup_value(false), ChoicePopupAction::Remove);
    }

    /// Invalid replacement fails before either labels or selection can change.
    #[test]
    fn replacement_is_atomic_and_strict() {
        let mut state = state(Some(2));
        let text = b"short";
        let entries = [ChoiceMenuEntry::Choice {
            id: 7,
            label: MenuTextRange { first: 0, count: 5 },
        }
        .into()];
        let result = state.replace_menu(
            NcCopyFromBufferOffset::new(0, text),
            NcCopyFromBufferOffset::new(0, &entries),
            encode_selection(Some(2)),
        );
        assert_eq!(result, Err(NC_E_INVALID_ARG));
        assert_eq!(state.menu_text, "AppleBananaCherry");
        assert_eq!(state.selection, Some(2));
    }

    /// The menu coordinator observes every semantic edit through one authoritative revision.
    #[test]
    fn menu_model_revision_tracks_selection_and_items() {
        let mut state = state(Some(1));
        IChoiceMenuState::set_selection(&mut state, encode_selection(Some(2))).unwrap();
        assert_eq!(state.revision, 1);

        let text = b"ApricotBlueberry";
        let entries = [
            ChoiceMenuEntry::Choice {
                id: 0,
                label: MenuTextRange { first: 0, count: 7 },
            }
            .into(),
            ChoiceMenuEntry::Choice {
                id: 1,
                label: MenuTextRange { first: 7, count: 9 },
            }
            .into(),
        ];
        state
            .replace_menu(
                NcCopyFromBufferOffset::new(0, text),
                NcCopyFromBufferOffset::new(0, &entries),
                encode_selection(Some(1)),
            )
            .unwrap();
        assert_eq!(state.revision, 2);
    }

    /// Foreign preorder input cannot smuggle ambiguous topology or duplicate stable identities.
    #[test]
    fn serialized_menu_rejects_malformed_hierarchy_and_duplicate_ids() {
        let text = "BranchLeaf";
        let missing_child = [
            ChoiceMenuEntry::Submenu {
                label: MenuTextRange { first: 0, count: 6 },
                children: 2,
            }
            .into(),
            ChoiceMenuEntry::Choice {
                id: 9,
                label: MenuTextRange { first: 6, count: 4 },
            }
            .into(),
        ];
        assert_eq!(validate_menu(text, &missing_child, Some(9)), Err(NC_E_INVALID_ARG));

        let empty_branch = [ChoiceMenuEntry::Submenu {
            label: MenuTextRange { first: 0, count: 6 },
            children: 0,
        }
        .into()];
        assert_eq!(validate_menu(text, &empty_branch, None), Err(NC_E_INVALID_ARG));

        let duplicate_ids = [
            ChoiceMenuEntry::Choice {
                id: 9,
                label: MenuTextRange { first: 6, count: 4 },
            }
            .into(),
            ChoiceMenuEntry::Choice {
                id: 9,
                label: MenuTextRange { first: 6, count: 4 },
            }
            .into(),
        ];
        assert_eq!(validate_menu(text, &duplicate_ids, Some(9)), Err(NC_E_INVALID_ARG));
    }
}
