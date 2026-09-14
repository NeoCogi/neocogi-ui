//! Private runtime composition for combo-box state and popup presentation.
//!
//! The window manager sees the choice list as an ordinary widget-backed popup. Combo semantics
//! stay in this module: a private typed COM object owns the authoritative model, while the public
//! header and private popup widget reach that object through exclusive `&mut self` calls. No
//! combo-specific type, branch, or input policy enters the root/surface layer.

use crate::ui_node::widgets::content_height;
use crate::ui_node::widget_object::{HostedWidget, direct_leaf_layout_impl, direct_widget_phase_impl};
use crate::window_manager::{PopupHandle, SurfaceMutationError, WindowManager};
use crate::{
    AtlasHandle, Change, Constraints, ControlRole, ControlState, Dimensioni, FontRef, FontRole, KeyboardBehavior, MouseButton, PointerState, Recti, Skin,
    UiInputEvent, Vec2i, Widget, WidgetOption, WidgetPaintCtx, WidgetUpdateCtx,
};
use neocogi_com::{
    ComPtr, ComRef, INcStringAbi, INcStringComPtrExt, NcCopyFromBufferOffset, NcStatus, NcString, NC_E_INVALID_ARG, com_int_enum, com_interface, com_object,
};
#[allow(unused_imports)] // The object macro resolves these trait identifiers into generated types.
use neocogi_ui_core_contracts::{ILeafLayout, IWidget};
use neocogi_ui_core_contracts::{
    ComboItemRange, ComboSelection, ComboSelectionAbi, IComboChangedSinkAbi, IEventSubscriptionAbi, ILeafLayoutAbi, ILeafLayoutInterfacesFor, IWidgetAbi,
    IWidgetHostAbi, IWidgetInterfacesFor, Rect,
};

/// Contract-defined visible fallback for a combo with no committed selection.
pub(crate) const DEFAULT_COMBO_PLACEHOLDER: &str = "Select…";

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
    /// Returns a revision changed only when popup row count changes its vertical geometry.
    fn row_revision(&self) -> Result<u64, NcStatus>;
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
    /// Monotonic row-count revision kept distinct from paint-only model changes.
    row_revision: u64,
    /// Last valid screen-space anchor captured by header activation.
    popup_anchor: Rect,
    /// Explicit popup lifecycle reconciled by the runtime composition owner.
    popup: ComboPopupLifecycle,
    /// Runtime-owned source for actual user commits only.
    changed_event: crate::EventSource<ComboSelection>,
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
            row_revision: 0,
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

    /// Reads the narrower revision used by the popup's independent measurement cache.
    fn row_revision(&self) -> Result<u64, NcStatus> {
        Ok(self.row_revision)
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
        let row_count_changed = self.items.len() != items.len();
        self.items = items;
        self.selection = selection;
        self.advance_revision();
        if row_count_changed {
            self.row_revision = self.row_revision.wrapping_add(1);
        }
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
            self.changed_event.emit(ComboSelection::Selected(index as u64));
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
        crate::event::subscribe(self.changed_event.event(), sink)
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

#[com_interface(
    auto_guid = "neocogi.ui.runtime.internal.ComboPopupControl:v1",
    assigned_signature = b"00000000000000000000000000000052"
)]
/// Runtime-private lifecycle control for the ordinary widget used as popup content.
pub(crate) trait IComboPopupControl {
    /// Synchronizes popup-local candidates and invalidates geometry only when row count changed.
    fn synchronize_model(&mut self) -> Result<bool, NcStatus>;
    /// Resets hover and keyboard candidates immediately before generic popup activation.
    fn prepare_open(&mut self) -> Result<(), NcStatus>;
}

#[com_object(interfaces(IWidget, ILeafLayout, IComboPopupControl))]
/// Ordinary private widget that measures, updates, and paints combo choices inside a generic popup.
pub(crate) struct ComboPopup {
    /// Common widget lifecycle and phase resources used by every retained leaf.
    runtime: crate::ui_node::widget_object::WidgetRuntime,
    /// Typed model capability; all mutation enters its `&mut self` interface methods.
    state: ComPtr<IComboStateAbi>,
    /// Pointer candidate present only while the pointer is over a concrete row.
    hot: Option<usize>,
    /// Keyboard candidate initialized from the committed selection at each opening.
    active: Option<usize>,
    /// State revision reflected by the transient candidates.
    observed_revision: u64,
    /// Row revision reflected by the popup widget's independent measurement cache.
    observed_row_revision: u64,
}

/// Exact effects of synchronizing transient popup presentation with its semantic model.
#[derive(Copy, Clone, Debug, Default, Eq, PartialEq)]
struct PresentationChange {
    /// Whether hover or keyboard candidates were refreshed for a semantic change.
    semantic: bool,
    /// Whether current row labels require invalidating cached preferred geometry.
    geometry: bool,
}

impl ComboPopup {
    /// Builds both interfaces over one private widget allocation for mounting and coordination.
    pub(crate) fn create(
        host: ComPtr<IWidgetHostAbi>,
        resources: crate::ui_node::widget_object::SharedAssets,
        state: ComPtr<IComboStateAbi>,
    ) -> Result<(ComPtr<IWidgetAbi>, ComPtr<IComboPopupControlAbi>), NcStatus> {
        let runtime = crate::ui_node::widget_object::WidgetRuntime::for_resources(host, WidgetOption::NONE, Some(&resources))?;
        let observed_revision = state.revision()?;
        let observed_row_revision = state.row_revision()?;
        let unknown = Self::from_value(Self {
            runtime,
            state,
            hot: None,
            active: None,
            observed_revision,
            observed_row_revision,
        });
        Ok((unknown.query()?, unknown.query()?))
    }

    /// Reads and validates optional selection through the private typed state boundary.
    fn selection(&self) -> Result<Option<usize>, NcStatus> {
        decoded_selection(self.state.selection()?)
    }

    /// Reads a representable item count, treating impossible internal ABI failure as no rows.
    fn item_count(&self) -> usize {
        self.state.item_count().ok().and_then(|count| usize::try_from(count).ok()).unwrap_or(0)
    }

    /// Recomputes the shared row height from current immutable theme resources.
    fn row_height(style: &Skin, atlas: &AtlasHandle) -> i32 {
        content_height(style, atlas, &FontRef::Role(FontRole::Body), 0).max(0)
    }

    /// Resolves one local pointer position against the same row geometry used by paint.
    fn row_at(&self, position: Vec2i, row_height: i32) -> Option<usize> {
        if row_height <= 0 || position.y < 0 {
            return None;
        }
        let index = usize::try_from(position.y / row_height).ok()?;
        (index < self.item_count()).then_some(index)
    }

    /// Resets stale presentation candidates from the latest authoritative selection.
    fn synchronize_revision(&mut self) -> Result<PresentationChange, NcStatus> {
        let revision = self.state.revision()?;
        let row_revision = self.state.row_revision()?;
        let change = PresentationChange {
            semantic: self.observed_revision != revision,
            geometry: self.observed_row_revision != row_revision,
        };
        if change.semantic {
            self.hot = None;
            self.active = self.selection()?;
            self.observed_revision = revision;
        }
        if change.geometry {
            self.observed_row_revision = row_revision;
        }
        Ok(change)
    }

    /// Moves the keyboard candidate by one row with explicit wrapping behavior.
    fn move_active(&mut self, forward: bool) {
        let count = self.item_count();
        self.active = match (count, self.active, forward) {
            (0, _, _) => None,
            (_, Some(index), true) => Some((index + 1) % count),
            (_, Some(0), false) => Some(count - 1),
            (_, Some(index), false) => Some(index - 1),
            (_, None, true) => Some(0),
            (_, None, false) => Some(count - 1),
        };
    }

    /// Selects the first or last row as a keyboard candidate without committing it.
    fn move_active_to_edge(&mut self, first: bool) {
        self.active = match (self.item_count(), first) {
            (0, _) => None,
            (_, true) => Some(0),
            (count, false) => Some(count - 1),
        };
    }

    /// Commits one checked choice and keeps local presentation coherent with the new revision.
    fn submit(&mut self, index: usize) {
        if let Ok(revision) = self.state.submit(index as u64) {
            self.active = Some(index);
            self.observed_revision = revision;
        }
    }

    /// Returns the visible row with pointer hover taking precedence only while it exists.
    fn visible_index(&self) -> Option<usize> {
        let selected = self.selection().ok().flatten();
        match self.state.revision() {
            Ok(revision) if revision == self.observed_revision => visible_popup_index(self.hot, self.active, selected),
            Ok(_) => selected,
            Err(_) => None,
        }
    }

    /// Extracts a local pointer position from one normalized pointer transition.
    fn pointer_position(input: Option<&UiInputEvent>) -> Option<Vec2i> {
        match input {
            Some(
                UiInputEvent::MouseMove { pos, .. }
                | UiInputEvent::MouseDrag { pos, .. }
                | UiInputEvent::MouseDown { pos, .. }
                | UiInputEvent::MouseUp { pos, .. }
                | UiInputEvent::Scroll { pos, .. },
            ) => Some(*pos),
            Some(UiInputEvent::Cancel | UiInputEvent::Key { .. } | UiInputEvent::Text { .. }) | None => None,
        }
    }
}

/// Resolves popup emphasis without allowing absent hover to erase committed selection.
fn visible_popup_index(hot: Option<usize>, active: Option<usize>, selected: Option<usize>) -> Option<usize> {
    match (hot, active, selected) {
        (Some(index), _, _) | (None, Some(index), _) | (None, None, Some(index)) => Some(index),
        (None, None, None) => None,
    }
}

impl crate::LeafWidget for ComboPopup {
    /// Measures every current label without retaining a second item collection.
    fn measure(&self, style: &Skin, atlas: &AtlasHandle, _constraints: Constraints) -> Dimensioni {
        let count = i32::try_from(self.item_count()).unwrap_or(i32::MAX);
        // Width belongs to the header's exact outer anchor; only row count contributes intrinsic
        // popup geometry now that the generic surface fixes the horizontal axis.
        Dimensioni::new(0, Self::row_height(style, atlas).saturating_mul(count))
    }
}

impl Widget for ComboPopup {
    /// Updates hover and keyboard candidates before committing only explicit row submissions.
    fn update(&mut self, ctx: &mut WidgetUpdateCtx<'_>, input: Option<&UiInputEvent>) -> Change {
        // The state capability is private, retained by this object, and populated only with
        // validated values. A failed internal observation cannot be represented by `Widget::update`;
        // retain the last complete presentation and let the next coordinator pass retry it.
        let state_changed = self.synchronize_revision().unwrap_or_default();
        let row_height = Self::row_height(ctx.skin(), ctx.atlas());
        self.hot = match (ctx.hovered(), Self::pointer_position(input)) {
            (true, Some(position)) => self.row_at(position, row_height),
            (false, _) => None,
            (true, None) => self.hot,
        };

        match input {
            Some(UiInputEvent::MouseDown { button, .. }) if button.intersects(MouseButton::LEFT) => {
                if let Some(index) = self.hot {
                    self.submit(index);
                }
            }
            Some(UiInputEvent::Key { event }) if ctx.focused() && event.is_pressed() => match event.key {
                crate::Key::ArrowDown => self.move_active(true),
                crate::Key::ArrowUp => self.move_active(false),
                crate::Key::Home => self.move_active_to_edge(true),
                crate::Key::End => self.move_active_to_edge(false),
                crate::Key::Enter | crate::Key::Space if !event.repeat => {
                    let selected = self.selection().ok().flatten();
                    if let Some(index) = visible_popup_index(self.hot, self.active, selected) {
                        self.submit(index);
                    }
                }
                _ => {}
            },
            Some(
                UiInputEvent::MouseMove { .. }
                | UiInputEvent::MouseDrag { .. }
                | UiInputEvent::MouseDown { .. }
                | UiInputEvent::MouseUp { .. }
                | UiInputEvent::Scroll { .. }
                | UiInputEvent::Cancel
                | UiInputEvent::Key { .. }
                | UiInputEvent::Text { .. },
            )
            | None => {}
        }
        Change::from(state_changed.geometry)
    }

    /// Paints rows from current model values and transient presentation candidates.
    fn paint(&self, ctx: &mut WidgetPaintCtx<'_>) {
        let row_height = Self::row_height(ctx.skin(), ctx.atlas());
        let font = ctx.skin().resolve_font(ctx.atlas(), &FontRef::Role(FontRole::Body));
        let visible = self.visible_index();
        for index in 0..self.item_count() {
            let Ok(label) = self.state.item(index as u64).and_then(|label| label.to_string()) else {
                continue;
            };
            let y = i32::try_from(index).unwrap_or(i32::MAX).saturating_mul(row_height);
            let row = Recti::new(0, y, ctx.local_rect().width.max(0), row_height);
            let hovered = self.hot == Some(index);
            let selected = visible == Some(index);
            let state = match (ctx.enabled(), selected, hovered, ctx.active() && hovered) {
                (false, _, _, _) => ControlState::Disabled,
                (true, true, true, true) => ControlState::Focused(PointerState::Pressed),
                (true, true, true, false) => ControlState::Focused(PointerState::Hovered),
                (true, true, false, _) => ControlState::Focused(PointerState::Normal),
                (true, false, true, true) => ControlState::Enabled(PointerState::Pressed),
                (true, false, true, false) => ControlState::Enabled(PointerState::Hovered),
                (true, false, false, _) => ControlState::Enabled(PointerState::Normal),
            };
            ctx.draw_control_center_state(ControlRole::Item, state, row);
            let color = ctx.skin().control(ControlRole::Item, state).content_color;
            ctx.draw_control_text_color_with_font(font, &label, row, color, WidgetOption::NONE);
        }
    }

    /// Makes the private popup leaf the sole keyboard target inside its generic popup tree.
    fn keyboard_behavior(&self) -> KeyboardBehavior {
        KeyboardBehavior::TAB_STOP
    }
}

impl crate::ui_node::widget_object::WidgetAlgorithm for ComboPopup {}

impl HostedWidget for ComboPopup {
    /// Borrows the ordinary widget runtime used by generic root traversal.
    fn runtime(&self) -> &crate::ui_node::widget_object::WidgetRuntime {
        &self.runtime
    }

    /// Mutably borrows ordinary widget runtime state for phase and lifecycle operations.
    fn runtime_mut(&mut self) -> &mut crate::ui_node::widget_object::WidgetRuntime {
        &mut self.runtime
    }
}

impl IComboPopupControl for ComboPopup {
    /// Applies semantic changes after callbacks, before roots perform their next generic layout.
    fn synchronize_model(&mut self) -> Result<bool, NcStatus> {
        self.runtime.state.writable()?;
        let change = self.synchronize_revision()?;
        if change.geometry {
            self.runtime.state.record_change(neocogi_ui_core_contracts::GeometryChange::Changed);
        }
        Ok(change.semantic)
    }

    /// Resets transient candidates from the authoritative selection before each opening.
    fn prepare_open(&mut self) -> Result<(), NcStatus> {
        self.runtime.state.writable()?;
        self.hot = None;
        self.active = self.selection()?;
        self.observed_revision = self.state.revision()?;
        self.observed_row_revision = self.state.row_revision()?;
        Ok(())
    }
}

direct_widget_phase_impl!(ComboPopup);
direct_leaf_layout_impl!(ComboPopup);

/// Runtime-owned association between private combo state and one ordinary popup handle.
struct ComboController {
    /// Typed semantic model shared by the header and popup widget through COM calls.
    state: ComPtr<IComboStateAbi>,
    /// Generic manager capability; no combo policy is embedded in its root node.
    popup: PopupHandle,
    /// Typed control used only to reset transient presentation before generic activation.
    presentation: ComPtr<IComboPopupControlAbi>,
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
        let presentation_changed = self.presentation.synchronize_model()?;
        let action = self.state.reconcile_popup(visible)?;
        match action {
            ComboPopupAction::None => Ok((ControllerDisposition::Keep, presentation_changed)),
            ComboPopupAction::Open => {
                self.presentation.prepare_open()?;
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
    /// Registers one fully constructed controller after its generic popup exists.
    pub(crate) fn register(&mut self, state: ComPtr<IComboStateAbi>, popup: PopupHandle, presentation: ComPtr<IComboPopupControlAbi>) {
        self.controllers.push(ComboController { state, popup, presentation });
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
            row_revision: 0,
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

    /// A committed row remains visible until a concrete pointer or keyboard candidate supersedes it.
    #[test]
    fn popup_visibility_falls_back_to_committed_selection() {
        assert_eq!(visible_popup_index(None, None, Some(1)), Some(1));
        assert_eq!(visible_popup_index(Some(2), Some(0), Some(1)), Some(2));
        assert_eq!(visible_popup_index(None, Some(0), Some(1)), Some(0));
        assert_eq!(visible_popup_index(None, None, None), None);
    }

    /// Popup geometry revision changes for row-count replacements but not paint-only edits.
    #[test]
    fn row_revision_tracks_only_popup_measurement_inputs() {
        let mut state = state(Some(1));
        state.set_selection(encode_selection(Some(2))).unwrap();
        state.set_placeholder(NcCopyFromBufferOffset::new(0, "Another fallback".as_bytes())).unwrap();
        assert_eq!(state.row_revision, 0);

        let text = b"ApricotBlueberry";
        let ranges = [ComboItemRange { first: 0, count: 7 }, ComboItemRange { first: 7, count: 9 }];
        state
            .replace_items(
                NcCopyFromBufferOffset::new(0, text),
                NcCopyFromBufferOffset::new(0, &ranges),
                encode_selection(Some(1)),
            )
            .unwrap();
        assert_eq!(state.row_revision, 1);
    }
}
