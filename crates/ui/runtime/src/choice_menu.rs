//! Core choice-menu model and orchestration over the shared compact menu surface.
//!
//! The root forest stores only generic menu surfaces and popup ownership. This module connects
//! one public header, one authoritative typed state, and one private choice-popup definition.

use crate::menu::{Menu, MenuEntry};
use crate::window_manager::{PopupHandle, SurfaceMutationError, WindowManager};
use neocogi_com::{ComPtr, INcStringAbi, INcStringComPtrExt, NcCopyFromBufferOffset, NcOutputWriter, NcStatus, NcString, NC_E_INVALID_ARG, com_object};
use neocogi_ui::abi::core::{ChoiceMenuEntry, ChoiceSelection, IChoiceMenu, IChoiceMenuAbi, IChoiceMenuVtblFor, MenuTextRange, Rect};

/// Maximum accepted branch nesting, keeping foreign preorder input bounded on the native stack.
const MAX_CHOICE_MENU_DEPTH: usize = 32;

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

/// Complete runtime-private bindings compiled from the local IDL contract.
#[allow(dead_code, unused_imports)]
mod bindings {
    include!(env!("IDL_NEOCOGI_UI_RUNTIME_INTERNAL"));
}
pub(crate) use bindings::{ChoicePopupAction, ChoicePopupIntent, IChoiceMenuState, IChoiceMenuStateAbi, IChoiceMenuStateVtblFor};

/// Canonical checked menu data; its private index is derived, never separately mutable.
/// The index stores entry positions rather than labels, IDs or another topology. No public
/// capability/lifetime is added: ordinary label getters continue returning owned strings.
#[derive(PartialEq)]
pub(crate) struct ChoiceDefinition {
    /// Shared UTF-8 backing referenced by checked entry ranges.
    text: String,
    /// Sole authoritative preorder hierarchy, including submenu headers.
    entries: Vec<ChoiceMenuEntry>,
    /// Preorder ordinal -> entry position; all positions name Choice variants.
    choices: Vec<u32>,
}
impl ChoiceDefinition {
    /// Validates once and builds matching metadata before any live state can be replaced.
    fn new(text: String, entries: Vec<ChoiceMenuEntry>) -> Result<Self, NcStatus> {
        u32::try_from(text.len()).map_err(|_| neocogi_com::NC_E_OUT_OF_RANGE)?;
        u32::try_from(entries.len()).map_err(|_| neocogi_com::NC_E_OUT_OF_RANGE)?;
        validate_menu(&text, &entries, None)?;
        let count = entries.iter().filter(|entry| matches!(entry, ChoiceMenuEntry::Choice { .. })).count();
        #[cfg(test)]
        if probes::build() {
            return Err(neocogi_com::NC_E_OUT_OF_MEMORY);
        }
        let mut choices = Vec::new();
        choices.try_reserve_exact(count).map_err(|_| neocogi_com::NC_E_OUT_OF_MEMORY)?;
        for (position, entry) in entries.iter().enumerate() {
            if matches!(entry, ChoiceMenuEntry::Choice { .. }) {
                choices.push(u32::try_from(position).map_err(|_| neocogi_com::NC_E_OUT_OF_RANGE)?);
            }
        }
        Ok(Self { text, entries, choices })
    }

    /// Performs bounded metadata lookup before copying only this leaf's referenced label.
    fn label(&self, ordinal: usize) -> Option<&str> {
        let position = usize::try_from(*self.choices.get(ordinal)?).ok()?;
        let entry = self.entries.get(position)?;
        #[cfg(test)]
        probes::entry();
        match *entry {
            ChoiceMenuEntry::Choice { label, .. } => ranged_text(&self.text, label),
            ChoiceMenuEntry::Submenu { .. } => None,
        }
    }
}

/// Thread-local observations belong only to unit-test execution; no counters or fault state ship.
#[cfg(test)]
pub(crate) mod probes {
    use std::cell::Cell;
    /// Distinguishes candidate construction and actual reads from interface-call counts.
    #[derive(Clone, Copy, Default, Debug, PartialEq)]
    pub(crate) struct Counts {
        /// Candidate-admission attempts reaching index reservation.
        pub(crate) builds: usize,
        /// Actual authoritative entries resolved by ordinal metadata.
        pub(crate) entries: usize,
        /// Actual range resolutions, including each prefix visited by the original nth path.
        pub(crate) ranges: usize,
    }
    thread_local! {
        /// Tests run independent state without affecting another test thread.
        static COUNTS: Cell<Counts> = const { Cell::new(Counts { builds: 0, entries: 0, ranges: 0 }) };
        /// One deterministic failure at the real new-index reservation boundary.
        static FAIL_NEXT: Cell<bool> = const { Cell::new(false) };
    }
    /// Resets probes after admission/popup work or returns one operation's observed totals.
    pub(crate) fn take() -> Counts {
        COUNTS.with(|value| value.replace(Counts::default()))
    }
    /// Records index admission and consumes an optional test allocation refusal.
    pub(super) fn build() -> bool {
        COUNTS.with(|value| {
            let mut counts = value.get();
            counts.builds += 1;
            value.set(counts);
        });
        FAIL_NEXT.with(|value| value.replace(false))
    }
    /// Records the actual indexed entry access, not the public getter call.
    pub(super) fn entry() {
        COUNTS.with(|value| {
            let mut counts = value.get();
            counts.entries += 1;
            value.set(counts);
        });
    }
    /// Records slicing at its common owner so the original scan's extra work is observable.
    pub(super) fn range() {
        COUNTS.with(|value| {
            let mut counts = value.get();
            counts.ranges += 1;
            value.set(counts);
        });
    }
    /// Refuses the next candidate before new index publication, without exhausting process memory.
    pub(crate) fn fail_next() {
        FAIL_NEXT.with(|value| value.set(true));
    }
}

#[com_object(interfaces(IChoiceMenuState, IChoiceMenu))]
/// Plain authoritative choice menu state mutated exclusively through `IChoiceMenuState`'s mutable receiver.
pub(crate) struct ChoiceMenuState {
    /// Canonical text/preorder and its inseparable derived ordinal index.
    definition: ChoiceDefinition,
    /// Positive visible-row limit, distinct from model/selection rebuild revision.
    max_visible_items: u32,
    /// Committed stable leaf identity; `None` remains distinct from every `u64` value.
    selection: Option<u64>,
    /// Monotonic semantic revision used to invalidate transient popup candidates.
    revision: u64,
    /// Last valid screen-space anchor captured by header activation.
    popup_anchor: Rect,
    /// Explicit popup lifecycle reconciled by the runtime composition owner.
    popup: ChoicePopupLifecycle,
    /// Runtime-owned source for actual user commits only.
    changed_event: neocogi_ui::abi::core::EventPort,
}

impl ChoiceMenuState {
    /// Validates selection before allocating the private COM state identity.
    pub(crate) fn create(
        definition: ChoiceDefinition,
        selection: Option<u64>,
        queue: Option<ComPtr<neocogi_ui::abi::core::IEventReceiverAbi>>,
    ) -> Result<ComPtr<IChoiceMenuStateAbi>, NcStatus> {
        validate_selection(selection, &definition.text, &definition.entries)?;
        let local = Self::into_local(Self {
            definition,
            selection,
            max_visible_items: crate::menu::DEFAULT_CHOICE_VISIBLE_ITEMS,
            revision: 0,
            popup_anchor: Rect::default(),
            popup: ChoicePopupLifecycle::Closed,
            changed_event: neocogi_ui::abi::core::EventPort::new(),
        });
        // No caller can submit a choice until this factory returns. Connect its
        // weak public identity through checked local access before that boundary.
        let origin = local.query::<IChoiceMenuAbi>()?.downgrade()?;
        local.try_borrow_mut()?.changed_event = neocogi_ui::abi::core::EventPort::with_queue(queue, origin);
        local.query()
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
        if self.definition.choices.is_empty() && intent != ChoicePopupIntent::Close {
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
            (ChoicePopupLifecycle::Open, ChoicePopupIntent::Open) => {
                self.popup_anchor = anchor;
                ChoicePopupLifecycle::OpenRequested
            }
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
        if !self.definition.choices.is_empty() {
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
    /// Reads presentation policy without manufacturing a second model revision.
    fn max_visible_items(&self) -> Result<u32, NcStatus> {
        Ok(self.max_visible_items)
    }

    /// Reads the scalar revision directly from the authoritative state.
    fn revision(&self) -> Result<u64, NcStatus> {
        Ok(self.revision)
    }

    /// Encodes structural absence without manufacturing an in-band index sentinel.
    fn selection(&self) -> Result<ChoiceSelection, NcStatus> {
        Ok(encode_selection(self.selection))
    }

    /// Copies the one authoritative UTF-8 backing store without exposing internal allocation.
    fn menu_text(&self) -> Result<ComPtr<INcStringAbi>, NcStatus> {
        Ok(NcString::from_utf8_str(&self.definition.text))
    }

    /// Copies only the requested suffix while reporting complete preorder cardinality.
    fn menu_entries(&self, mut output: NcOutputWriter<'_, ChoiceMenuEntry>) -> Result<u32, NcStatus> {
        let offset = usize::try_from(output.offset()).map_err(|_| NC_E_INVALID_ARG)?;
        let source = self.definition.entries.get(offset..).ok_or(NC_E_INVALID_ARG)?;
        let count = source.len().min(output.capacity());
        output.extend_from_slice(&source[..count])?;
        u32::try_from(self.definition.entries.len()).map_err(|_| neocogi_com::NC_E_OUT_OF_RANGE)
    }

    /// Reads cardinality from the same derived index used by ordinal resolution.
    fn choice_count(&self) -> Result<u64, NcStatus> {
        Ok(self.definition.choices.len() as u64)
    }

    /// Copies one strictly checked label from the authoritative collection.
    fn choice_label(&self, ordinal: u64) -> Result<ComPtr<INcStringAbi>, NcStatus> {
        let ordinal = usize::try_from(ordinal).map_err(|_| NC_E_INVALID_ARG)?;
        let label = self.definition.label(ordinal).ok_or(NC_E_INVALID_ARG)?;
        Ok(NcString::from_utf8_str(label))
    }

    /// Validates a complete candidate before committing either labels or selection.
    fn replace_menu(
        &mut self,
        menu_text: NcCopyFromBufferOffset<'_, u8>,
        entries: NcCopyFromBufferOffset<'_, ChoiceMenuEntry>,
        selection: ChoiceSelection,
    ) -> Result<bool, NcStatus> {
        let definition = copied_menu(menu_text, entries)?;
        let selection = decoded_selection(selection);
        validate_selection(selection, &definition.text, &definition.entries)?;
        if self.definition == definition && self.selection == selection {
            return Ok(false);
        }
        // The COM mutable gate spans this publication, including old-data destruction.
        // No observation can see new entries paired with the previous ordinal index.
        self.definition = definition;
        self.selection = selection;
        self.advance_revision();
        self.close_if_empty();
        Ok(true)
    }

    /// Replaces selection only after validating it against the current item sequence.
    fn set_selection(&mut self, selection: ChoiceSelection) -> Result<bool, NcStatus> {
        let selection = decoded_selection(selection);
        validate_selection(selection, &self.definition.text, &self.definition.entries)?;
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
        validate_selection(Some(id), &self.definition.text, &self.definition.entries)?;
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
            self.changed_event
                .post(neocogi_ui::abi::core::events::choice_changed(ChoiceSelection::Selected(id))?)?;
        }
        Ok(self.revision)
    }

    /// Makes explicit menu-service retirement idempotent. Header destruction needs no call.
    fn retire(&mut self) -> Result<(), NcStatus> {
        self.changed_event.close();
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
    /// Shares the authoritative cap with the private coordinator.
    fn max_visible_items(&self) -> Result<u32, NcStatus> {
        Ok(self.max_visible_items)
    }

    /// Validates before silent mutation; the coordinator installs geometry separately.
    fn set_max_visible_items(&mut self, count: u32) -> Result<(), NcStatus> {
        if count == 0 {
            return Err(NC_E_INVALID_ARG);
        }
        self.max_visible_items = count;
        Ok(())
    }

    /// Returns the exact optional stable identity from the authoritative menu model.
    fn selection(&self) -> Result<ChoiceSelection, NcStatus> {
        <Self as IChoiceMenuState>::selection(self)
    }

    /// Copies the selected label, using empty text only for the explicit absent-selection case.
    fn selected_label(&self) -> Result<ComPtr<INcStringAbi>, NcStatus> {
        let label = match self.selection {
            Some(id) => choice_label(&self.definition.text, &self.definition.entries, id).ok_or(NC_E_INVALID_ARG)?,
            None => "",
        };
        Ok(NcString::from_utf8_str(label))
    }

    /// Reports leaf cardinality without scanning or duplicating the authoritative hierarchy.
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
        entries: NcCopyFromBufferOffset<'_, ChoiceMenuEntry>,
        selection: ChoiceSelection,
    ) -> Result<(), NcStatus> {
        <Self as IChoiceMenuState>::replace_menu(self, menu_text, entries, selection).map(|_| ())
    }

    /// Silently commits one checked optional leaf identity.
    fn set_selection(&mut self, selection: ChoiceSelection) -> Result<(), NcStatus> {
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
    let mut entries = neocogi_com::NcOutputBuffer::new();
    let count = usize::try_from(state.menu_entries(entries.output(0, 0))?).map_err(|_| NC_E_INVALID_ARG)?;
    let copied = state.menu_entries(entries.output(0, count))?;
    if usize::try_from(copied).ok() != Some(count) || entries.len() != count {
        return Err(NC_E_INVALID_ARG);
    }
    native_menu(&menu_text, entries.as_slice())
}

/// Runtime-owned association between private choice menu state and one shared menu popup handle.
struct ChoiceMenuController {
    /// The header owns the menu model. Coordination observes it weakly so header
    /// destruction needs no fallible callback, including during menu submission.
    state: neocogi_com::WeakComPtr<IChoiceMenuStateAbi>,
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
        // Temporary access protects this reconciliation only. If forwarding
        // removes the header, this lease ends on return and the next pass removes
        // the orphan popup. No destructor reenters the menu's mutable call gate.
        let Some(mut state) = self.state.upgrade()? else {
            match manager.destroy_popup(&self.popup) {
                Ok(()) | Err(SurfaceMutationError::UnknownPopup) => {}
                Err(error) => return Err(surface_status(error)),
            }
            return Ok((ControllerDisposition::Remove, true));
        };
        let visible = match manager.popup_visible(&self.popup) {
            Ok(visible) => visible,
            Err(SurfaceMutationError::UnknownPopup) => {
                state.retire()?;
                return Ok((ControllerDisposition::Remove, false));
            }
            Err(error) => return Err(surface_status(error)),
        };
        // Menu routing only stages a stable leaf identity. Commit it here, after the root forest
        // released input borrows and before application event queues can observe the new value.
        if let Some(choice) = manager.take_choice_submission(&self.popup).map_err(surface_status)? {
            state.submit(choice)?;
        }
        let revision = state.revision()?;
        let mut presentation_changed = revision != self.observed_revision;
        if presentation_changed {
            let menu = choice_menu(&state)?;
            let committed = decoded_selection(state.selection()?);
            manager.replace_choice_popup(&self.popup, menu, committed).map_err(surface_status)?;
            self.observed_revision = revision;
        }
        presentation_changed |= manager
            .set_choice_popup_limit(&self.popup, state.max_visible_items()?)
            .map_err(surface_status)?;
        let action = state.reconcile_popup(visible)?;
        match action {
            ChoicePopupAction::None => Ok((ControllerDisposition::Keep, presentation_changed)),
            ChoicePopupAction::Open => {
                let anchor = crate::boundary::local_rect(state.popup_anchor()?);
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
        // State is created by this provider and supports the common shared weak ABI.
        let state = state.downgrade().expect("provider choice state supports weak observation");
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
}

/// Converts generic surface policy failures at this private composition boundary.
fn surface_status(error: SurfaceMutationError) -> NcStatus {
    match error {
        SurfaceMutationError::Busy => neocogi_ui::abi::core::UI_E_BUSY,
        SurfaceMutationError::UnknownWindow | SurfaceMutationError::UnknownPopup => neocogi_ui::abi::core::UI_E_CLOSED,
        SurfaceMutationError::InvalidLayer(_) => NC_E_INVALID_ARG,
        SurfaceMutationError::InvalidDialogOwner
        | SurfaceMutationError::InvalidChildWindowParent
        | SurfaceMutationError::ManagedLayer
        | SurfaceMutationError::InvalidPopupParent => neocogi_ui::abi::core::UI_E_POLICY,
    }
}

/// Encodes optional selection with a tagged absence variant rather than a sentinel identity.
pub(crate) fn encode_selection(selection: Option<u64>) -> ChoiceSelection {
    match selection {
        Some(id) => ChoiceSelection::Selected(id),
        None => ChoiceSelection::None,
    }
}

/// Converts a checked native selection to the runtime optional identity.
/// Foreign discriminants have already been validated by the COM thunk.
pub(crate) fn decoded_selection(selection: ChoiceSelection) -> Option<u64> {
    match selection {
        ChoiceSelection::None => None,
        ChoiceSelection::Selected(id) => Some(id),
    }
}

/// Validates one optional identity against every selectable leaf in the hierarchy.
fn validate_selection(selection: Option<u64>, text: &str, entries: &[ChoiceMenuEntry]) -> Result<(), NcStatus> {
    match selection {
        Some(id) if choice_label(text, entries, id).is_none() => Err(NC_E_INVALID_ARG),
        Some(_) | None => Ok(()),
    }
}

/// Validates shared UTF-8 storage and copies the complete serialized hierarchy atomically.
pub(crate) fn copied_menu(
    menu_text: NcCopyFromBufferOffset<'_, u8>,
    entries: NcCopyFromBufferOffset<'_, ChoiceMenuEntry>,
) -> Result<ChoiceDefinition, NcStatus> {
    if menu_text.offset() != 0 || entries.offset() != 0 {
        return Err(NC_E_INVALID_ARG);
    }
    let text = std::str::from_utf8(menu_text.as_slice()).map_err(|_| NC_E_INVALID_ARG)?;
    u32::try_from(text.len()).map_err(|_| neocogi_com::NC_E_OUT_OF_RANGE)?;
    u32::try_from(entries.len()).map_err(|_| neocogi_com::NC_E_OUT_OF_RANGE)?;
    ChoiceDefinition::new(text.to_owned(), entries.as_slice().to_vec())
}

/// Returns one checked UTF-8 label slice from the shared model storage.
fn ranged_text(text: &str, range: MenuTextRange) -> Option<&str> {
    #[cfg(test)]
    probes::range();
    let first = usize::try_from(range.first).ok()?;
    let count = usize::try_from(range.count).ok()?;
    text.get(first..first.checked_add(count)?)
}

/// Resolves a stable leaf identity independently from its label and hierarchy position.
fn choice_label<'a>(text: &'a str, entries: &[ChoiceMenuEntry], selected: u64) -> Option<&'a str> {
    entries.iter().find_map(|entry| match *entry {
        ChoiceMenuEntry::Choice { id, label } if id == selected => ranged_text(text, label),
        ChoiceMenuEntry::Choice { .. } | ChoiceMenuEntry::Submenu { .. } => None,
    })
}

/// Validates ranges, direct-child counts, unique leaf identities, and selection together.
fn validate_menu(text: &str, entries: &[ChoiceMenuEntry], selection: Option<u64>) -> Result<(), NcStatus> {
    let mut ids = std::collections::HashSet::new();
    let mut index = 0;
    while index < entries.len() {
        index = validate_node(text, entries, index, 0, &mut ids)?;
    }
    validate_selection(selection, text, entries)
}

/// Validates one preorder node and returns the first entry after all of its descendants.
fn validate_node(text: &str, entries: &[ChoiceMenuEntry], index: usize, depth: usize, ids: &mut std::collections::HashSet<u64>) -> Result<usize, NcStatus> {
    let entry = *entries.get(index).ok_or(NC_E_INVALID_ARG)?;
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
fn native_menu(text: &str, entries: &[ChoiceMenuEntry]) -> Result<Menu, NcStatus> {
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
fn native_entry(text: &str, entries: &[ChoiceMenuEntry], index: usize) -> Result<(MenuEntry, usize), NcStatus> {
    match *entries.get(index).ok_or(NC_E_INVALID_ARG)? {
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
        .map(|(id, label)| ChoiceMenuEntry::Choice { id, label })
        .to_vec();
        ChoiceMenuState {
            definition: ChoiceDefinition::new(menu_text, entries).unwrap(),
            selection,
            max_visible_items: crate::menu::DEFAULT_CHOICE_VISIBLE_ITEMS,
            revision: 0,
            popup_anchor: Rect::default(),
            popup: ChoicePopupLifecycle::Closed,
            changed_event: Default::default(),
        }
    }

    /// Generated COM getters resolve one entry/range per ordinal, including the admitted ceiling.
    #[test]
    fn indexed_labels_have_linear_resolution_through_com() {
        let _host = crate::test_support::host_runtime();
        for count in [512, 2_000, 4_000, 8_000, 65_536] {
            let entries = (0..count)
                .map(|i| ChoiceMenuEntry::Choice {
                    id: 100_000 + i as u64,
                    label: MenuTextRange { first: 0, count: 1 },
                })
                .collect();
            probes::take();
            let definition = ChoiceDefinition::new("a".into(), entries).unwrap();
            let mut state = ChoiceMenuState::create(definition, None, None).unwrap();
            let menu = state.query::<IChoiceMenuAbi>().unwrap();
            assert_eq!(probes::take().builds, 1);
            assert_eq!(menu.choice_count().unwrap(), count as u64);
            for ordinal in 0..count {
                assert_eq!(menu.choice_label(ordinal as u64).unwrap().to_string().unwrap(), "a");
            }
            assert_eq!(probes::take(), probes::Counts { builds: 0, entries: count, ranges: count });
            assert!(matches!(menu.choice_label(count as u64), Err(NC_E_INVALID_ARG)));
            assert!(menu.choice_label(u64::MAX).is_err());
            state.set_selection(ChoiceSelection::Selected(100_000)).unwrap();
            assert_eq!(probes::take().builds, 0);
        }
    }

    /// Nested metadata keeps preorder ordinal distinct from stable identity and UTF-8 byte ranges.
    #[test]
    fn nested_index_and_retained_strings_survive_replacement() {
        let _host = crate::test_support::host_runtime();
        let range = MenuTextRange { first: 1, count: 2 };
        let definition = ChoiceDefinition::new(
            "xéunused".into(),
            vec![
                ChoiceMenuEntry::Choice { id: 80, label: range },
                ChoiceMenuEntry::Submenu { label: range, children: 2 },
                ChoiceMenuEntry::Choice { id: 7, label: range },
                ChoiceMenuEntry::Submenu { label: range, children: 1 },
                ChoiceMenuEntry::Choice { id: 999, label: range },
            ],
        )
        .unwrap();
        assert_eq!(definition.choices, [0, 2, 4]);
        let mut state = ChoiceMenuState::create(definition, Some(999), None).unwrap();
        let menu = state.query::<IChoiceMenuAbi>().unwrap();
        let retained = menu.choice_label(2).unwrap();
        for ordinal in 0..3 {
            assert_eq!(menu.choice_label(ordinal).unwrap().to_string().unwrap(), "é");
        }
        state
            .replace_menu(NcCopyFromBufferOffset::new(0, b""), NcCopyFromBufferOffset::new(0, &[]), ChoiceSelection::None)
            .unwrap();
        assert_eq!(menu.choice_count(), Ok(0));
        drop(menu);
        drop(state);
        assert_eq!(retained.to_string().unwrap(), "é");
    }

    /// Refusal at the new allocation boundary cannot publish mismatched data/index or popup state.
    #[test]
    fn index_allocation_failure_preserves_complete_live_state() {
        let mut state = state(Some(2));
        state.apply_popup_intent(ChoicePopupIntent::Open, Rect::default()).unwrap();
        let old = (state.revision, state.popup, state.selection);
        let entries = [ChoiceMenuEntry::Choice {
            id: 42,
            label: MenuTextRange { first: 0, count: 1 },
        }];
        probes::fail_next();
        assert_eq!(
            state.replace_menu(
                NcCopyFromBufferOffset::new(0, b"z"),
                NcCopyFromBufferOffset::new(0, &entries),
                ChoiceSelection::Selected(42)
            ),
            Err(neocogi_com::NC_E_OUT_OF_MEMORY)
        );
        assert_eq!((state.revision, state.popup, state.selection), old);
        assert_eq!(state.definition.text, "AppleBananaCherry");
        assert_eq!(state.definition.choices, [0, 1, 2]);
        state
            .replace_menu(
                NcCopyFromBufferOffset::new(0, b"z"),
                NcCopyFromBufferOffset::new(0, &entries),
                ChoiceSelection::Selected(42),
            )
            .unwrap();
        assert_eq!(state.definition.choices, [0]);
        assert_eq!(state.definition.label(0), Some("z"));
        assert_eq!(state.revision, old.0 + 1);
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
        }];
        let result = state.replace_menu(
            NcCopyFromBufferOffset::new(0, text),
            NcCopyFromBufferOffset::new(0, &entries),
            encode_selection(Some(2)),
        );
        assert_eq!(result, Err(NC_E_INVALID_ARG));
        assert_eq!(state.definition.text, "AppleBananaCherry");
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
            },
            ChoiceMenuEntry::Choice {
                id: 1,
                label: MenuTextRange { first: 7, count: 9 },
            },
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
            },
            ChoiceMenuEntry::Choice {
                id: 9,
                label: MenuTextRange { first: 6, count: 4 },
            },
        ];
        assert_eq!(validate_menu(text, &missing_child, Some(9)), Err(NC_E_INVALID_ARG));

        let empty_branch = [ChoiceMenuEntry::Submenu {
            label: MenuTextRange { first: 0, count: 6 },
            children: 0,
        }];
        assert_eq!(validate_menu(text, &empty_branch, None), Err(NC_E_INVALID_ARG));

        let duplicate_ids = [
            ChoiceMenuEntry::Choice {
                id: 9,
                label: MenuTextRange { first: 6, count: 4 },
            },
            ChoiceMenuEntry::Choice {
                id: 9,
                label: MenuTextRange { first: 6, count: 4 },
            },
        ];
        assert_eq!(validate_menu(text, &duplicate_ids, Some(9)), Err(NC_E_INVALID_ARG));
    }
}
