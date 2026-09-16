//! Private combo model and composition over the shared compact choice-menu surface.
//!
//! The root forest stores only generic menu surfaces and popup ownership. This module connects
//! one public header, one authoritative typed state, and one private choice-popup definition.

use crate::menu::MenuSlot;
use crate::window_manager::{PopupHandle, SurfaceMutationError, WindowManager};
use neocogi_com::{
    ComPtr, ComRef, INcStringAbi, INcStringComPtrExt, NcCopyFromBufferOffset, NcStatus, NcString, NC_E_INVALID_ARG, com_int_enum, com_interface, com_object,
};
use neocogi_ui_core_contracts::{IEventSubscriptionAbi, Rect};
use neocogi_ui_nodes_contracts::{ComboItemRange, ComboSelection, ComboSelectionAbi, IComboChangedSinkAbi};

/// Contract-defined visible fallback for a combo with no committed selection.
pub(crate) const DEFAULT_COMBO_PLACEHOLDER: &str = "Select…";

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
    /// Returns the explicit optional committed item position.
    fn selection(&self) -> Result<ComboSelectionAbi, NcStatus>;
    /// Copies the selected label or placeholder currently displayed by the header.
    fn display_text(&self) -> Result<ComPtr<INcStringAbi>, NcStatus>;
    /// Returns the number of retained display labels.
    fn item_count(&self) -> Result<u64, NcStatus>;
    /// Copies one exact display label after strict range validation.
    fn item(&self, index: u64) -> Result<ComPtr<INcStringAbi>, NcStatus>;
    /// Atomically replaces every label and the explicit optional selection.
    fn replace_items(
        &mut self,
        item_text: NcCopyFromBufferOffset<'_, u8>,
        ranges: NcCopyFromBufferOffset<'_, ComboItemRange>,
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
    fn submit(&mut self, index: u64) -> Result<u64, NcStatus>;
    /// Connects the public changed sink to this model's sole event source.
    fn subscribe_changed(&mut self, sink: ComRef<'_, IComboChangedSinkAbi>) -> Result<ComPtr<IEventSubscriptionAbi>, NcStatus>;
    /// Ends event delivery and requests permanent private-popup removal.
    fn retire(&mut self) -> Result<(), NcStatus>;
}

#[com_object(interfaces(IComboState))]
/// Plain authoritative combo state mutated exclusively through `IComboState`'s mutable receiver.
pub(crate) struct ComboState {
    /// Display labels in stable application order.
    items: Vec<String>,
    /// Committed selection; `None` remains distinct from item zero.
    selection: Option<usize>,
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
    pub(crate) fn create(items: Vec<String>, selection: Option<usize>, placeholder: String) -> Result<ComPtr<IComboStateAbi>, NcStatus> {
        validate_selection(selection, items.len())?;
        let state = Self {
            items,
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
            Some(index) => self.items[index].as_str(),
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
        if self.items.is_empty() && intent != ComboPopupIntent::Close {
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
        if !self.items.is_empty() {
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

    /// Copies one immutable display snapshot for the public header or facade.
    fn display_text(&self) -> Result<ComPtr<INcStringAbi>, NcStatus> {
        Ok(NcString::from_utf8_str(self.display_text_value()))
    }

    /// Reports the authoritative label count without a second collection.
    fn item_count(&self) -> Result<u64, NcStatus> {
        Ok(self.items.len() as u64)
    }

    /// Copies one strictly checked label from the authoritative collection.
    fn item(&self, index: u64) -> Result<ComPtr<INcStringAbi>, NcStatus> {
        let index = usize::try_from(index).map_err(|_| NC_E_INVALID_ARG)?;
        Ok(NcString::from_utf8_str(self.items.get(index).ok_or(NC_E_INVALID_ARG)?))
    }

    /// Validates a complete candidate before committing either labels or selection.
    fn replace_items(
        &mut self,
        item_text: NcCopyFromBufferOffset<'_, u8>,
        ranges: NcCopyFromBufferOffset<'_, ComboItemRange>,
        selection: ComboSelectionAbi,
    ) -> Result<bool, NcStatus> {
        let items = copied_items(item_text, ranges)?;
        let selection = decoded_selection(selection)?;
        validate_selection(selection, items.len())?;
        if self.items == items && self.selection == selection {
            return Ok(false);
        }
        self.items = items;
        self.selection = selection;
        self.advance_revision();
        self.close_if_empty();
        Ok(true)
    }

    /// Replaces selection only after validating it against the current item sequence.
    fn set_selection(&mut self, selection: ComboSelectionAbi) -> Result<bool, NcStatus> {
        let selection = decoded_selection(selection)?;
        validate_selection(selection, self.items.len())?;
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
    fn submit(&mut self, index: u64) -> Result<u64, NcStatus> {
        let index = usize::try_from(index).map_err(|_| NC_E_INVALID_ARG)?;
        self.items.get(index).ok_or(NC_E_INVALID_ARG)?;
        if self.selection != Some(index) {
            self.selection = Some(index);
            self.advance_revision();
            self.changed_event.emit(ComboChangedPayload {
                selection: ComboSelection::Selected(index as u64),
            });
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

/// Builds one checked flat choice snapshot without a second retained item collection.
pub(crate) fn choice_rows(state: &ComPtr<IComboStateAbi>) -> Result<Vec<MenuSlot>, NcStatus> {
    let count = usize::try_from(state.item_count()?).map_err(|_| NC_E_INVALID_ARG)?;
    let mut rows = Vec::with_capacity(count);
    for index in 0..count {
        let label = state.item(index as u64)?.to_string()?;
        rows.push(MenuSlot::Choice { id: index as u64, label, enabled: true });
    }
    Ok(rows)
}

/// Runtime-owned association between private combo state and one shared menu popup handle.
struct ComboController {
    /// Typed semantic model shared by the header and composition coordinator.
    state: ComPtr<IComboStateAbi>,
    /// Generic popup capability; no combo policy is embedded in its forest node.
    popup: PopupHandle,
    /// Last model revision copied into the menu surface's compact leaf snapshot.
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
            let rows = choice_rows(&self.state)?;
            let committed = decoded_selection(self.state.selection()?)?.map(|index| index as u64);
            manager.replace_choice_popup(&self.popup, rows, committed).map_err(surface_status)?;
            self.observed_revision = revision;
        }
        let action = self.state.reconcile_popup(visible)?;
        match action {
            ComboPopupAction::None => Ok((ControllerDisposition::Keep, presentation_changed)),
            ComboPopupAction::Open => {
                manager.prepare_choice_popup(&self.popup).map_err(surface_status)?;
                let anchor = crate::boundary::local_rect(self.state.popup_anchor()?);
                manager.show_popup_at(&self.popup, anchor).map_err(surface_status)?;
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

/// Encodes optional selection with a tagged absence variant rather than a sentinel index.
pub(crate) fn encode_selection(selection: Option<usize>) -> ComboSelectionAbi {
    match selection {
        Some(index) => ComboSelection::Selected(index as u64),
        None => ComboSelection::None,
    }
    .into()
}

/// Decodes the public tagged selection and rejects unknown or unrepresentable payloads.
pub(crate) fn decoded_selection(selection: ComboSelectionAbi) -> Result<Option<usize>, NcStatus> {
    match ComboSelection::try_from(selection).map_err(|_| NC_E_INVALID_ARG)? {
        ComboSelection::None => Ok(None),
        ComboSelection::Selected(index) => usize::try_from(index).map(Some).map_err(|_| NC_E_INVALID_ARG),
    }
}

/// Validates one optional index against the complete candidate label count.
fn validate_selection(selection: Option<usize>, count: usize) -> Result<(), NcStatus> {
    match selection {
        Some(index) if index >= count => Err(NC_E_INVALID_ARG),
        Some(_) | None => Ok(()),
    }
}

/// Validates shared UTF-8 storage and copies each checked item range atomically.
pub(crate) fn copied_items(item_text: NcCopyFromBufferOffset<'_, u8>, ranges: NcCopyFromBufferOffset<'_, ComboItemRange>) -> Result<Vec<String>, NcStatus> {
    if item_text.offset() != 0 || ranges.offset() != 0 {
        return Err(NC_E_INVALID_ARG);
    }
    let text = std::str::from_utf8(item_text.as_slice()).map_err(|_| NC_E_INVALID_ARG)?;
    ranges
        .as_slice()
        .iter()
        .map(|range| {
            let first = usize::try_from(range.first).map_err(|_| NC_E_INVALID_ARG)?;
            let count = usize::try_from(range.count).map_err(|_| NC_E_INVALID_ARG)?;
            let end = first.checked_add(count).ok_or(NC_E_INVALID_ARG)?;
            text.get(first..end).map(str::to_owned).ok_or(NC_E_INVALID_ARG)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    //! Focused state-machine tests independent from generic root implementation details.

    use super::*;

    /// Creates a direct state value for exhaustive private transition tests.
    fn state(selection: Option<usize>) -> ComboState {
        ComboState {
            items: vec!["Apple".into(), "Banana".into(), "Cherry".into()],
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
        let ranges = [ComboItemRange { first: 0, count: 5 }];
        let result = state.replace_items(
            NcCopyFromBufferOffset::new(0, text),
            NcCopyFromBufferOffset::new(0, &ranges),
            encode_selection(Some(2)),
        );
        assert_eq!(result, Err(NC_E_INVALID_ARG));
        assert_eq!(state.items, ["Apple", "Banana", "Cherry"]);
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
        let ranges = [ComboItemRange { first: 0, count: 7 }, ComboItemRange { first: 7, count: 9 }];
        state
            .replace_items(
                NcCopyFromBufferOffset::new(0, text),
                NcCopyFromBufferOffset::new(0, &ranges),
                encode_selection(Some(1)),
            )
            .unwrap();
        assert_eq!(state.revision, 3);
    }
}
