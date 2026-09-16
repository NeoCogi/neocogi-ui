//! Private combo model and composition over the shared compact choice-menu surface.
//!
//! The root forest stores only generic menu surfaces and popup ownership. This module connects
//! one public header, one authoritative typed state, and one private choice-popup definition.

use crate::menu::{Menu, MenuEntry};
use crate::window_manager::{PopupHandle, SurfaceMutationError, WindowManager};
use neocogi_com::{
    ComPtr, ComRef, INcStringAbi, INcStringComPtrExt, NcCopyFromBufferOffset, NcCopyToBufferOffset, NcStatus, NcString, NC_E_INVALID_ARG, com_int_enum,
    com_interface, com_object,
};
use neocogi_ui_core_contracts::{ChoiceMenuEntry, ChoiceMenuEntryAbi, IEventSubscriptionAbi, MenuTextRange, Rect};
use neocogi_ui_nodes_contracts::{ComboSelection, ComboSelectionAbi, IComboChangedSinkAbi};

/// Contract-defined visible fallback for a combo with no committed selection.
pub(crate) const DEFAULT_COMBO_PLACEHOLDER: &str = "Select…";

/// Maximum accepted branch nesting, keeping foreign preorder input bounded on the native stack.
const MAX_COMBO_MENU_DEPTH: usize = 32;

/// Canonical generated selection snapshot stored in this producer's event queue.
use neocogi_ui_nodes_contracts::IComboChangedSinkEvent as ComboChangedPayload;

com_int_enum! {
    /// Header intent understood only by the private combo state machine.
    pub(crate) enum ComboPopupIntent: u8 {
        /// Toggle the current popup intent after ordinary pointer, Enter, or Space activation.
        Toggle = 0,
        /// Request opening without closing an already-open popup.
        Open = 1,
        /// Request closure without opening a currently closed popup.
        Close = 2,
    }
}

com_int_enum! {
    /// Concrete operation returned to the runtime-level combo coordinator.
    pub(crate) enum ComboPopupAction: u8 {
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

/// Complete private popup lifecycle; every transition is centralized in [`ComboState`].
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
enum ComboPopupLifecycle {
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
    auto_guid = "neocogi.ui.runtime.internal.ComboState:v1",
    assigned_signature = b"00000000000000000000000000000051"
)]
/// Runtime-private typed access to one authoritative combo model.
pub(crate) trait IComboState {
    /// Returns a revision changed by labels, placeholder, or committed selection.
    fn revision(&self) -> Result<u64, NcStatus>;
    /// Returns the explicit optional committed leaf identity.
    fn selection(&self) -> Result<ComboSelectionAbi, NcStatus>;
    /// Copies the hierarchy's shared UTF-8 storage for rebuilding its menu projection.
    fn menu_text(&self) -> Result<ComPtr<INcStringAbi>, NcStatus>;
    /// Copies a requested preorder entry range and returns the complete entry count.
    fn menu_entries(&self, output: NcCopyToBufferOffset<'_, ChoiceMenuEntryAbi>) -> Result<u32, NcStatus>;
    /// Copies the selected label or placeholder currently displayed by the header.
    fn display_text(&self) -> Result<ComPtr<INcStringAbi>, NcStatus>;
    /// Returns the number of retained display labels.
    fn choice_count(&self) -> Result<u64, NcStatus>;
    /// Copies one exact display label after strict range validation.
    fn choice_label(&self, ordinal: u64) -> Result<ComPtr<INcStringAbi>, NcStatus>;
    /// Atomically replaces every label and the explicit optional selection.
    fn replace_menu(
        &mut self,
        menu_text: NcCopyFromBufferOffset<'_, u8>,
        entries: NcCopyFromBufferOffset<'_, ChoiceMenuEntryAbi>,
        selection: ComboSelectionAbi,
    ) -> Result<bool, NcStatus>;
    /// Silently replaces the optional committed selection without clamping.
    fn set_selection(&mut self, selection: ComboSelectionAbi) -> Result<bool, NcStatus>;
    /// Copies the fallback independently from current selection.
    fn placeholder(&self) -> Result<ComPtr<INcStringAbi>, NcStatus>;
    /// Replaces the fallback without changing or publishing selection.
    fn set_placeholder(&mut self, placeholder: NcCopyFromBufferOffset<'_, u8>) -> Result<bool, NcStatus>;
    /// Applies one typed header intent and captures a valid screen-space anchor when opening.
    fn request_popup(&mut self, intent: ComboPopupIntent, anchor: Rect) -> Result<(), NcStatus>;
    /// Reconciles semantic lifecycle with actual generic-popup visibility.
    fn reconcile_popup(&mut self, visible: bool) -> Result<ComboPopupAction, NcStatus>;
    /// Returns the most recently captured header-bottom anchor.
    fn popup_anchor(&self) -> Result<Rect, NcStatus>;
    /// Commits one user choice, publishes a changed snapshot, and requests popup closure.
    fn submit(&mut self, id: u64) -> Result<u64, NcStatus>;
    /// Connects the public changed sink to this model's sole event source.
    fn subscribe_changed(&mut self, sink: ComRef<'_, IComboChangedSinkAbi>) -> Result<ComPtr<IEventSubscriptionAbi>, NcStatus>;
    /// Ends event delivery and requests permanent private-popup removal.
    fn retire(&mut self) -> Result<(), NcStatus>;
}

#[com_object(interfaces(IComboState))]
/// Plain authoritative combo state mutated exclusively through `IComboState`'s mutable receiver.
pub(crate) struct ComboState {
    /// Shared UTF-8 storage referenced by every checked preorder entry.
    menu_text: String,
    /// Authoritative preorder hierarchy with explicit direct-child counts.
    entries: Vec<ChoiceMenuEntryAbi>,
    /// Committed stable leaf identity; `None` remains distinct from every `u64` value.
    selection: Option<u64>,
    /// Visible header fallback when selection is absent.
    placeholder: String,
    /// Monotonic semantic revision used to invalidate transient popup candidates.
    revision: u64,
    /// Last valid screen-space anchor captured by header activation.
    popup_anchor: Rect,
    /// Explicit popup lifecycle reconciled by the runtime composition owner.
    popup: ComboPopupLifecycle,
    /// Runtime-owned source for actual user commits only.
    changed_event: crate::EventSource<ComboChangedPayload>,
}

impl ComboState {
    /// Validates selection before allocating the private COM state identity.
    pub(crate) fn create(
        menu_text: String,
        entries: Vec<ChoiceMenuEntryAbi>,
        selection: Option<u64>,
        placeholder: String,
    ) -> Result<ComPtr<IComboStateAbi>, NcStatus> {
        validate_menu(&menu_text, &entries, selection)?;
        let state = Self {
            menu_text,
            entries,
            selection,
            placeholder,
            revision: 0,
            popup_anchor: Rect::default(),
            popup: ComboPopupLifecycle::Closed,
            changed_event: crate::EventSource::new(),
        };
        Self::from_value(state).query()
    }

    /// Borrows the exact text rendered by the header from the current valid selection.
    fn display_text_value(&self) -> &str {
        match self.selection {
            Some(id) => choice_label(&self.menu_text, &self.entries, id).expect("validated selection must retain its choice label"),
            None => self.placeholder.as_str(),
        }
    }

    /// Advances the semantic revision after one observable model change.
    fn advance_revision(&mut self) {
        self.revision = self.revision.wrapping_add(1);
    }

    /// Applies header popup intent using exhaustive lifecycle pattern matching.
    fn apply_popup_intent(&mut self, intent: ComboPopupIntent, anchor: Rect) -> Result<(), NcStatus> {
        if !intent.is_known() || anchor.width < 0 || anchor.height < 0 {
            return Err(NC_E_INVALID_ARG);
        }
        if choice_count(&self.entries) == 0 && intent != ComboPopupIntent::Close {
            return Ok(());
        }
        self.popup = match (self.popup, intent) {
            (ComboPopupLifecycle::Closed | ComboPopupLifecycle::CloseRequested, ComboPopupIntent::Toggle | ComboPopupIntent::Open) => {
                self.popup_anchor = anchor;
                ComboPopupLifecycle::OpenRequested
            }
            (ComboPopupLifecycle::Open | ComboPopupLifecycle::OpenRequested, ComboPopupIntent::Toggle) => ComboPopupLifecycle::CloseRequested,
            (ComboPopupLifecycle::OpenRequested, ComboPopupIntent::Open) => {
                self.popup_anchor = anchor;
                ComboPopupLifecycle::OpenRequested
            }
            (ComboPopupLifecycle::Open, ComboPopupIntent::Open) => ComboPopupLifecycle::Open,
            (ComboPopupLifecycle::Open | ComboPopupLifecycle::OpenRequested, ComboPopupIntent::Close) => ComboPopupLifecycle::CloseRequested,
            (ComboPopupLifecycle::Closed | ComboPopupLifecycle::CloseRequested, ComboPopupIntent::Close) => ComboPopupLifecycle::Closed,
            (ComboPopupLifecycle::RemoveRequested, _) => ComboPopupLifecycle::RemoveRequested,
            (ComboPopupLifecycle::Detached, _) => ComboPopupLifecycle::Detached,
            (_, _) => return Err(NC_E_INVALID_ARG),
        };
        Ok(())
    }

    /// Converts one pending lifecycle transition into a generic-popup operation.
    fn reconcile_popup_value(&mut self, visible: bool) -> ComboPopupAction {
        let (next, action) = match (self.popup, visible) {
            (ComboPopupLifecycle::RemoveRequested, _) => (ComboPopupLifecycle::Detached, ComboPopupAction::Remove),
            (ComboPopupLifecycle::Detached, _) => (ComboPopupLifecycle::Detached, ComboPopupAction::None),
            (ComboPopupLifecycle::OpenRequested, _) => (ComboPopupLifecycle::Open, ComboPopupAction::Open),
            (ComboPopupLifecycle::CloseRequested, true) => (ComboPopupLifecycle::Closed, ComboPopupAction::Close),
            (ComboPopupLifecycle::CloseRequested, false) => (ComboPopupLifecycle::Closed, ComboPopupAction::None),
            (ComboPopupLifecycle::Open, true) => (ComboPopupLifecycle::Open, ComboPopupAction::None),
            (ComboPopupLifecycle::Open, false) => (ComboPopupLifecycle::Closed, ComboPopupAction::None),
            (ComboPopupLifecycle::Closed, false) => (ComboPopupLifecycle::Closed, ComboPopupAction::None),
            (ComboPopupLifecycle::Closed, true) => (ComboPopupLifecycle::Closed, ComboPopupAction::Close),
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
            ComboPopupLifecycle::Open | ComboPopupLifecycle::OpenRequested => ComboPopupLifecycle::CloseRequested,
            ComboPopupLifecycle::Closed | ComboPopupLifecycle::CloseRequested => ComboPopupLifecycle::Closed,
            ComboPopupLifecycle::RemoveRequested => ComboPopupLifecycle::RemoveRequested,
            ComboPopupLifecycle::Detached => ComboPopupLifecycle::Detached,
        };
    }
}

impl IComboState for ComboState {
    /// Reads the scalar revision directly from the authoritative state.
    fn revision(&self) -> Result<u64, NcStatus> {
        Ok(self.revision)
    }

    /// Encodes structural absence without manufacturing an in-band index sentinel.
    fn selection(&self) -> Result<ComboSelectionAbi, NcStatus> {
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

    /// Copies one immutable display snapshot for the public header or facade.
    fn display_text(&self) -> Result<ComPtr<INcStringAbi>, NcStatus> {
        Ok(NcString::from_utf8_str(self.display_text_value()))
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
        selection: ComboSelectionAbi,
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
    fn set_selection(&mut self, selection: ComboSelectionAbi) -> Result<bool, NcStatus> {
        let selection = decoded_selection(selection)?;
        validate_selection(selection, &self.menu_text, &self.entries)?;
        if self.selection == selection {
            return Ok(false);
        }
        self.selection = selection;
        self.advance_revision();
        Ok(true)
    }

    /// Copies the configured fallback even when a selected label is currently visible.
    fn placeholder(&self) -> Result<ComPtr<INcStringAbi>, NcStatus> {
        Ok(NcString::from_utf8_str(&self.placeholder))
    }

    /// Mutates fallback text exclusively and reports whether the stored value changed.
    fn set_placeholder(&mut self, placeholder: NcCopyFromBufferOffset<'_, u8>) -> Result<bool, NcStatus> {
        let placeholder = crate::boundary::text(placeholder)?;
        if self.placeholder == placeholder {
            return Ok(false);
        }
        self.placeholder.clear();
        self.placeholder.push_str(placeholder);
        self.advance_revision();
        Ok(true)
    }

    /// Delegates every intent to the exhaustive semantic state machine.
    fn request_popup(&mut self, intent: ComboPopupIntent, anchor: Rect) -> Result<(), NcStatus> {
        self.apply_popup_intent(intent, anchor)
    }

    /// Synchronizes manager visibility without publishing application-facing lifecycle events.
    fn reconcile_popup(&mut self, visible: bool) -> Result<ComboPopupAction, NcStatus> {
        Ok(self.reconcile_popup_value(visible))
    }

    /// Copies the anchor captured at the same header transition as the pending open request.
    fn popup_anchor(&self) -> Result<Rect, NcStatus> {
        Ok(self.popup_anchor)
    }

    /// Commits a checked item and closes the popup even when the value was already selected.
    fn submit(&mut self, id: u64) -> Result<u64, NcStatus> {
        validate_selection(Some(id), &self.menu_text, &self.entries)?;
        if self.selection != Some(id) {
            self.selection = Some(id);
            self.advance_revision();
            self.changed_event.emit(ComboChangedPayload { selection: ComboSelection::Selected(id) });
        }
        self.popup = match self.popup {
            ComboPopupLifecycle::Open | ComboPopupLifecycle::OpenRequested => ComboPopupLifecycle::CloseRequested,
            ComboPopupLifecycle::Closed | ComboPopupLifecycle::CloseRequested => ComboPopupLifecycle::Closed,
            ComboPopupLifecycle::RemoveRequested => ComboPopupLifecycle::RemoveRequested,
            ComboPopupLifecycle::Detached => ComboPopupLifecycle::Detached,
        };
        Ok(self.revision)
    }

    /// Uses the existing typed event bridge while keeping its source in this sole model.
    fn subscribe_changed(&mut self, sink: ComRef<'_, IComboChangedSinkAbi>) -> Result<ComPtr<IEventSubscriptionAbi>, NcStatus> {
        self.changed_event.event().subscribe(sink)
    }

    /// Makes retirement idempotent for both logical widget closure and final unmounted release.
    fn retire(&mut self) -> Result<(), NcStatus> {
        self.changed_event.close_ordinary();
        self.popup = match self.popup {
            ComboPopupLifecycle::Detached => ComboPopupLifecycle::Detached,
            ComboPopupLifecycle::Closed
            | ComboPopupLifecycle::OpenRequested
            | ComboPopupLifecycle::Open
            | ComboPopupLifecycle::CloseRequested
            | ComboPopupLifecycle::RemoveRequested => ComboPopupLifecycle::RemoveRequested,
        };
        Ok(())
    }
}

/// Builds one checked recursive choice-menu snapshot from the authoritative serialized model.
pub(crate) fn choice_menu(state: &ComPtr<IComboStateAbi>) -> Result<Menu, NcStatus> {
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

/// Runtime-owned association between private combo state and one shared menu popup handle.
struct ComboController {
    /// Typed semantic model shared by the header and composition coordinator.
    state: ComPtr<IComboStateAbi>,
    /// Generic popup capability; no combo policy is embedded in its forest node.
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

impl ComboController {
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
            let committed = decoded_selection(self.state.selection()?)?.map(|index| index as u64);
            manager.replace_choice_popup(&self.popup, menu, committed).map_err(surface_status)?;
            self.observed_revision = revision;
        }
        let action = self.state.reconcile_popup(visible)?;
        match action {
            ComboPopupAction::None => Ok((ControllerDisposition::Keep, presentation_changed)),
            ComboPopupAction::Open => {
                let anchor = crate::boundary::local_rect(self.state.popup_anchor()?);
                manager.show_choice_popup_at(&self.popup, anchor).map_err(surface_status)?;
                Ok((ControllerDisposition::Keep, true))
            }
            ComboPopupAction::Close => {
                manager.hide_popup(&self.popup).map_err(surface_status)?;
                Ok((ControllerDisposition::Keep, true))
            }
            ComboPopupAction::Remove => {
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

/// Composition-layer owner for combo controllers; mutation requires the runtime context's `&mut`.
#[derive(Default)]
pub(crate) struct ComboBoxManager {
    /// Live controller set, independent from generic root storage and traversal.
    controllers: Vec<ComboController>,
}

impl ComboBoxManager {
    /// Registers one fully constructed controller after its generic menu popup exists.
    pub(crate) fn register(&mut self, state: ComPtr<IComboStateAbi>, popup: PopupHandle, observed_revision: u64) {
        self.controllers.push(ComboController { state, popup, observed_revision });
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
pub(crate) fn encode_selection(selection: Option<u64>) -> ComboSelectionAbi {
    match selection {
        Some(id) => ComboSelection::Selected(id),
        None => ComboSelection::None,
    }
    .into()
}

/// Decodes the public tagged selection and rejects unknown discriminants.
pub(crate) fn decoded_selection(selection: ComboSelectionAbi) -> Result<Option<u64>, NcStatus> {
    match ComboSelection::try_from(selection).map_err(|_| NC_E_INVALID_ARG)? {
        ComboSelection::None => Ok(None),
        ComboSelection::Selected(id) => Ok(Some(id)),
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
            if children == 0 || depth >= MAX_COMBO_MENU_DEPTH {
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
    fn state(selection: Option<u64>) -> ComboState {
        let menu_text = "AppleBananaCherry".to_owned();
        let entries = [
            (0, MenuTextRange { first: 0, count: 5 }),
            (1, MenuTextRange { first: 5, count: 6 }),
            (2, MenuTextRange { first: 11, count: 6 }),
        ]
        .map(|(id, label)| ChoiceMenuEntry::Choice { id, label }.into())
        .to_vec();
        ComboState {
            menu_text,
            entries,
            selection,
            placeholder: "Select…".into(),
            revision: 0,
            popup_anchor: Rect::default(),
            popup: ComboPopupLifecycle::Closed,
            changed_event: crate::EventSource::new(),
        }
    }

    /// `None` and item zero remain structurally distinct in the authoritative plain state.
    #[test]
    fn optional_selection_has_no_zero_sentinel() {
        let mut state = state(None);
        assert_eq!(state.display_text_value(), "Select…");
        state.set_selection(encode_selection(Some(0))).unwrap();
        assert_eq!(state.display_text_value(), "Apple");
    }

    /// Opening, outside dismissal, and retirement produce explicit typed transitions.
    #[test]
    fn popup_lifecycle_reconciles_without_root_specific_state() {
        let mut state = state(Some(1));
        state
            .apply_popup_intent(ComboPopupIntent::Open, Rect { x: 10, y: 20, width: 100, height: 1 })
            .unwrap();
        assert_eq!(state.reconcile_popup_value(false), ComboPopupAction::Open);
        assert_eq!(state.reconcile_popup_value(true), ComboPopupAction::None);
        assert_eq!(state.reconcile_popup_value(false), ComboPopupAction::None);
        state.retire().unwrap();
        assert_eq!(state.reconcile_popup_value(false), ComboPopupAction::Remove);
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
    fn menu_model_revision_tracks_selection_fallback_and_items() {
        let mut state = state(Some(1));
        state.set_selection(encode_selection(Some(2))).unwrap();
        state.set_placeholder(NcCopyFromBufferOffset::new(0, "Another fallback".as_bytes())).unwrap();
        assert_eq!(state.revision, 2);

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
        assert_eq!(state.revision, 3);
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
