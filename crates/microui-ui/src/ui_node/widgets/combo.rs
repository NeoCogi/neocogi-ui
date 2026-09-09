//
// Copyright 2023-Present (c) Raja Lehtihet & Wael El Oraiby
//
// Redistribution and use in source and binary forms, with or without
// modification, are permitted provided that the following conditions are met:
//
// 1. Redistributions of source code must retain the above copyright notice,
// this list of conditions and the following disclaimer.
//
// 2. Redistributions in binary form must reproduce the above copyright notice,
// this list of conditions and the following disclaimer in the documentation
// and/or other materials provided with the distribution.
//
// 3. Neither the name of the copyright holder nor the names of its contributors
// may be used to endorse or promote products derived from this software without
// specific prior written permission.
//
// THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS "AS IS"
// AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE
// IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE
// ARE DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT HOLDER OR CONTRIBUTORS BE
// LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR
// CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF
// SUBSTITUTE GOODS OR SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS
// INTERRUPTION) HOWEVER CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN
// CONTRACT, STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE)
// ARISING IN ANY WAY OUT OF THE USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE
// POSSIBILITY OF SUCH DAMAGE.
//

//! Combo-box retained state.
//!
//! The combo widget tracks selected item text and popup-open state; the context root layer owns the
//! actual popup traversal.

use crate::ControlRole;

use super::*;

/// Compares rectangles structurally because the external geometry type does not implement equality.
fn same_rect(left: Recti, right: Recti) -> bool {
    (left.x, left.y, left.width, left.height) == (right.x, right.y, right.width, right.height)
}

/// One-shot construction input for a [`Combo`].
pub struct ComboParameters {
    /// Font used for the current label.
    pub font: FontRef,
    /// Base widget options.
    pub opt: WidgetOption,
}

impl crate::LeafWidget for Combo {
    fn measure(&self, style: &Skin, atlas: &AtlasHandle, constraints: Constraints) -> Dimensioni {
        self.preferred_size_widget(style, atlas, constraints)
    }
}

impl ComboParameters {
    /// Creates combo parameters with default widget options.
    pub const fn new() -> Self {
        Self {
            font: FontRef::Role(FontRole::Body),
            opt: WidgetOption::FRAME,
        }
    }
}

impl Default for ComboParameters {
    fn default() -> Self {
        Self::new()
    }
}

#[nc_com::com_object(microui_contracts::Combo)]
/// Concrete retained combo and dynamically constructible COM object.
pub struct Combo {
    /// Non-semantic COM lifecycle and host-backed phase resources.
    runtime: crate::ui_node::widget_object::WidgetRuntime,
    /// Currently selected item index.
    selected: usize,
    /// Whether the combo popup should be open.
    open: bool,
    /// Label text for the currently selected item.
    label: String,
    /// Initialization-only font.
    font: FontRef,
    /// Base widget options.
    /// Native algorithm fixture policy; production options live only in WidgetState.
    #[cfg(test)]
    opt: WidgetOption,
    /// Runtime-owned source for selection changes.
    changed_event: crate::EventSource<ComboChanged>,
    /// Runtime-owned source for header submissions.
    submitted_event: crate::EventSource<ComboSubmitted>,
}

impl Combo {
    /// Combines validated semantic parameters with one already bound common runtime.
    pub(crate) fn from_runtime(runtime: crate::ui_node::widget_object::WidgetRuntime, parameters: ComboParameters) -> Self {
        Self {
            runtime,
            selected: 0,
            open: false,
            label: String::new(),
            font: parameters.font,
            #[cfg(test)]
            opt: parameters.opt,
            changed_event: crate::EventSource::new(),
            submitted_event: crate::EventSource::new(),
        }
    }

    /// Returns the currently selected item index.
    pub fn selected(&self) -> usize {
        self.selected
    }

    /// Returns the current selected-item label.
    pub fn label(&self) -> &str {
        &self.label
    }

    /// Returns `true` while the combo popup should remain open.
    pub fn is_open(&self) -> bool {
        self.open
    }

    /// Opens the popup and marks the combo open.
    pub fn open_popup(&mut self) {
        self.open = true;
    }

    /// Closes the popup.
    pub fn close_popup(&mut self) {
        self.open = false;
    }

    /// Updates the cached label and clamps the selected index to the provided items.
    #[cfg(test)]
    pub fn update_items<S: AsRef<str>>(&mut self, items: &[S]) {
        let previous_selected = self.selected;
        if items.is_empty() {
            self.selected = 0;
            self.label.clear();
            if self.selected != previous_selected {
                self.emit_changed();
            }
            return;
        }

        if self.selected >= items.len() {
            // Clamp stale selections after the backing item list changes.
            self.selected = items.len() - 1;
        }

        self.label.clear();
        if let Some(label) = items.get(self.selected) {
            self.label.push_str(label.as_ref());
        }
        if self.selected != previous_selected {
            self.emit_changed();
        }
    }

    /// Applies a submitted popup item selection and closes the popup.
    #[cfg(test)]
    pub fn select<S: AsRef<str>>(&mut self, index: usize, items: &[S]) -> Option<String> {
        let previous_selected = self.selected;
        let previous_label = self.label.clone();
        if items.is_empty() {
            self.selected = 0;
            self.label.clear();
            self.close_popup();
            self.emit_change_if_needed(previous_selected, &previous_label);
            return None;
        }

        self.selected = index.min(items.len() - 1);
        self.label.clear();
        self.label.push_str(items[self.selected].as_ref());
        let selected_label = self.label.clone();
        self.close_popup();
        self.emit_change_if_needed(previous_selected, &previous_label);
        Some(selected_label)
    }

    #[cfg(test)]
    fn emit_change_if_needed(&mut self, previous_selected: usize, previous_label: &str) {
        if self.selected == previous_selected && self.label == previous_label {
            return;
        }
        self.changed_event.emit(ComboChanged {
            selected: self.selected,
            label: self.label.clone(),
        });
    }

    fn emit_changed(&mut self) {
        self.changed_event.emit(ComboChanged {
            selected: self.selected,
            label: self.label.clone(),
        });
    }
}

#[cfg(test)]
impl TypedWidgetHandle<Combo> {
    /// Returns whether the retained combo popup is open.
    pub fn is_open(&self) -> Option<bool> {
        self.try_read(Combo::is_open)
    }

    /// Closes the retained combo popup.
    pub fn close_popup(&self) -> Option<()> {
        self.try_update(Combo::close_popup)
    }

    /// Returns the combo's native selection-change endpoint.
    pub fn changed(&self) -> WidgetEventPortHandle<ComboChanged> {
        self.widget_event()
    }

    /// Returns the combo's native header-submission endpoint.
    pub fn submitted(&self) -> WidgetEventPortHandle<ComboSubmitted> {
        self.widget_event()
    }
}

/// Selection snapshot emitted after a combo's selected value changes.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ComboChanged {
    /// Selected item index after applying the change.
    pub selected: usize,
    /// Selected item label after applying the change.
    pub label: String,
}

/// Popup-state snapshot emitted when the user submits the combo header.
#[derive(Copy, Clone, Debug)]
pub struct ComboSubmitted {
    /// Whether the triggering header submission left the popup open.
    pub open: bool,
    /// Screen-space popup anchor from the update that handled the submission.
    pub anchor: Recti,
}

impl PartialEq for ComboSubmitted {
    fn eq(&self, other: &Self) -> bool {
        self.open == other.open && same_rect(self.anchor, other.anchor)
    }
}

impl Combo {
    /// Constructs a retained node and a weak typed handle to its concrete combo.
    #[cfg(test)]
    pub fn create(parameters: ComboParameters) -> (TypedWidgetHandle<Self>, Node) {
        Node::typed_widget(Self::from(parameters))
    }

    /// Returns the native event endpoint emitted whenever the user submits the combo header.
    #[cfg(test)]
    pub fn submitted(&self) -> crate::WidgetEventPortHandle<ComboSubmitted> {
        <Self as crate::TypedWidget<ComboSubmitted>>::event(self)
    }

    /// Measures the combo header label plus dropdown indicator.
    fn preferred_size_widget(&self, style: &Skin, atlas: &AtlasHandle, _constraints: Constraints) -> Dimensioni {
        let padding = style.metrics.padding.max(0);
        let text_w = if self.label.is_empty() {
            0
        } else {
            text_size(style, atlas, &self.font, self.label.as_str()).width
        };
        let indicator = atlas.get_icon_size(style.resolve_icon_role(atlas, crate::IconRole::ExpandDown));
        let width = padding.saturating_mul(3).saturating_add(text_w.max(0)).saturating_add(indicator.width.max(0));
        let height = content_height(style, atlas, &self.font, indicator.height);
        Dimensioni::new(width, height)
    }

    /// Updates popup-open state and records header submissions with their routed geometry.
    fn update_widget(&mut self, ctx: &mut WidgetUpdateCtx<'_>, input: Option<&UiInputEvent>) {
        // The Combo owns only semantic open state. The application composes that state with whichever
        // retained root it selected as popup content through the typed submission event below.
        let action = ctx.action(input, self.keyboard_behavior());
        let submitted = if let Some(action) = action {
            let previous_open = self.open;
            let screen_header = ctx.screen_content_rect();
            let anchor = rect(screen_header.x, screen_header.y.saturating_add(screen_header.height), screen_header.width, 1);
            self.open = match action {
                KeyboardAction::Activate => !self.open,
                KeyboardAction::Expand => true,
                KeyboardAction::Collapse => false,
                // Combo headers declare no adjustment capability, so these branches remain a
                // defensive no-op if a custom caller supplies inconsistent flags.
                KeyboardAction::Decrease | KeyboardAction::Increase => self.open,
            };
            // Idempotent Expand/Collapse repeats do not ask the application to reapply identical
            // popup state; every pointer or activation toggle necessarily changes the value.
            (self.open != previous_open).then_some(ComboSubmitted { open: self.open, anchor })
        } else {
            None
        };
        if let Some(event) = submitted {
            self.submitted_event.emit(event);
        }
    }

    /// Paints the combo header from already-committed retained state.
    fn paint_widget(&self, ctx: &mut WidgetPaintCtx<'_>) {
        let header = ctx.local_rect();
        let indicator_id = ctx.skin().resolve_icon_role(ctx.atlas(), crate::IconRole::ExpandDown);
        let indicator_size = ctx.atlas().get_icon_size(indicator_id);
        let indicator_x = header.x + header.width - indicator_size.width;
        let indicator_y = header.y + ((header.height - indicator_size.height) / 2).max(0);
        let indicator = rect(indicator_x, indicator_y, indicator_size.width, indicator_size.height);

        let mut text_rect = header;
        let reserved_width = indicator_size.width;
        text_rect.width = (text_rect.width - reserved_width).max(0);
        let font = ctx.skin().resolve_font(ctx.atlas(), &self.font);
        ctx.draw_control_text_with_font(font, self.label.as_str(), text_rect, ControlRole::Combo, ctx.options());

        let indicator_content = ctx.draw_control(ControlRole::Button, indicator);
        let icon_color = ctx.control_content_color(ControlRole::Combo);
        if let Some(indicator_content) = indicator_content {
            ctx.draw_icon(indicator_id, indicator_content, icon_color);
        }
    }
}

impl crate::TypedWidget<ComboChanged> for Combo {
    fn event(&self) -> crate::WidgetEventPortHandle<ComboChanged> {
        self.changed_event.event()
    }
}

impl crate::TypedWidget<ComboSubmitted> for Combo {
    fn event(&self) -> crate::WidgetEventPortHandle<ComboSubmitted> {
        self.submitted_event.event()
    }
}

impl Widget for Combo {
    #[cfg(test)]
    fn widget_opt(&self) -> &WidgetOption {
        &self.opt
    }

    fn frame_appearance_role(&self) -> FrameRole {
        // Combo headers use a distinct role so themes can separate them from command buttons.
        FrameRole::Control(ControlRole::Combo)
    }

    fn update(&mut self, ctx: &mut WidgetUpdateCtx<'_>, input: Option<&UiInputEvent>) -> crate::Change {
        self.update_widget(ctx, input);

        // Popup coordination is deferred; the header retains the same geometry.
        crate::Change::NoChange
    }

    fn paint(&self, ctx: &mut WidgetPaintCtx<'_>) {
        self.paint_widget(ctx);
    }

    fn keyboard_behavior(&self) -> KeyboardBehavior {
        KeyboardBehavior::TAB_STOP | KeyboardBehavior::ACTIVATE_ENTER | KeyboardBehavior::ACTIVATE_SPACE | KeyboardBehavior::POPUP
    }
}

#[cfg(test)]
impl From<ComboParameters> for Combo {
    /// Transfers one-shot parameters into the concrete Combo runtime.
    fn from(parameters: ComboParameters) -> Self {
        let runtime = crate::ui_node::widget_object::WidgetRuntime::for_test(parameters.opt);
        Self::from_runtime(runtime, parameters)
    }
}

impl crate::ui_node::widget_object::WidgetAlgorithm for Combo {
    /// Retained COM pointers cannot extend event delivery after logical widget closure.
    fn close_events(&mut self) {
        self.changed_event.close_ordinary();
        self.submitted_event.close_ordinary();
    }
}

impl crate::ui_node::widget_object::HostedWidget for Combo {
    /// Borrows common lifecycle and resource state without a second combo representation.
    fn runtime(&self) -> &crate::ui_node::widget_object::WidgetRuntime {
        &self.runtime
    }

    /// Exclusively borrows common lifecycle and resource state for a COM mutation or phase.
    fn runtime_mut(&mut self) -> &mut crate::ui_node::widget_object::WidgetRuntime {
        &mut self.runtime
    }
}
impl crate::ui_node::widget_object::FontState for Combo {
    /// Measurement and paint read this same existing font selection.
    fn font(&self) -> &FontRef {
        &self.font
    }
    /// The common COM helper validates ownership before installing this field.
    fn set_font(&mut self, font: FontRef) {
        self.font = font;
    }
}
impl Combo {
    /// Installs one complete selection; only an explicit user submission closes and publishes changed.
    pub(crate) fn com_set_selection(&mut self, selected: usize, label: &str, submit: bool) {
        let changed = self.selected != selected || self.label != label;
        self.selected = selected;
        self.label.clear();
        self.label.push_str(label);
        if submit {
            self.close_popup();
            if changed {
                self.emit_changed();
            }
        }
    }
}
impl Eq for ComboSubmitted {}

use crate::ui_node::widget_object::{HostedWidget, direct_leaf_layout_impl, direct_widget_phase_impl, set_direct_font_role, set_direct_named_font};
use microui_contracts::{
    ComboAbi, GeometryChange, IComboChangedSinkAbi, IComboPopupSinkAbi, IEventSubscriptionAbi, IWidgetHostAbi, TextFontRole,
};
use nc_com::{ComPtr, ComRef, INcStringAbi, NcCopyFromBufferOffset, NcStatus, NcString, NC_E_INVALID_ARG};

direct_widget_phase_impl!(Combo);
direct_leaf_layout_impl!(Combo);

impl Combo {
    /// Constructs the class for the host-bound factory through the registry's typed contract.
    pub(crate) fn create_com(
        host: ComPtr<IWidgetHostAbi>,
        resources: crate::ui_node::widget_object::SharedAssets,
    ) -> Result<ComPtr<ComboAbi>, NcStatus> {
        let value = Self::build(host, Some(&resources))?;
        Ok(Self::from_value(value).query().expect("Combo declares its callable class contract"))
    }

    /// Builds one combo header with either directly imported or runtime-shared resources.
    fn build(host: ComPtr<IWidgetHostAbi>, resources: Option<&crate::ui_node::widget_object::SharedAssets>) -> Result<Self, NcStatus> {
        let parameters = crate::ComboParameters::new();
        let runtime = crate::ui_node::widget_object::WidgetRuntime::for_resources(host, parameters.opt, resources)?;
        Ok(Self::from_runtime(runtime, parameters))
    }
}

impl microui_contracts::Combo for Combo {
    /// Initializes the established empty Combo state after validating host mutation authority.
    fn new(host: ComRef<'_, IWidgetHostAbi>) -> Result<Self, NcStatus> {
        Self::build(host.add_ref(), None)
    }

    /// Reads the same position used by the semantic selection algorithm.
    fn selected(&self) -> Result<u64, NcStatus> {
        self.runtime().state.readable()?;
        Ok(self.selected() as u64)
    }

    /// Copies the current header label without exposing the plugin's string allocation.
    fn label(&self) -> Result<ComPtr<INcStringAbi>, NcStatus> {
        self.runtime().state.readable()?;
        Ok(NcString::from_utf8_str(self.label()))
    }

    /// Reports semantic popup intent independently of the application's actual popup capability.
    fn is_open(&self) -> Result<bool, NcStatus> {
        self.runtime().state.readable()?;
        Ok(self.is_open())
    }

    /// Ordinary popup dismissal can synchronize intent without fabricating another request.
    fn set_open(&mut self, open: bool) -> Result<(), NcStatus> {
        self.runtime().state.writable()?;
        if open {
            self.open_popup();
        } else {
            self.close_popup();
        }
        Ok(())
    }

    /// Programmatic list synchronization remains silent and validates before changing either field.
    fn set_selection(&mut self, index: u64, label: NcCopyFromBufferOffset<'_, u8>) -> Result<(), NcStatus> {
        self.selection(index, label, false)
    }

    /// An actual popup action explicitly publishes the captured user selection after committing it.
    fn submit_selection(&mut self, index: u64, label: NcCopyFromBufferOffset<'_, u8>) -> Result<(), NcStatus> {
        self.selection(index, label, true)
    }

    /// Validates a semantic font role before changing the Combo's measurement input.
    fn set_font_role(&mut self, role: TextFontRole) -> Result<(), NcStatus> {
        set_direct_font_role(self, role)
    }

    /// Resolves one exact named font before replacing the Combo's selection.
    fn set_named_font(&mut self, name: NcCopyFromBufferOffset<'_, u8>) -> Result<(), NcStatus> {
        set_direct_named_font(self, name)
    }

    /// Connects the Combo's captured-selection source directly through its class contract.
    fn subscribe_changed(&mut self, sink: ComRef<'_, IComboChangedSinkAbi>) -> Result<ComPtr<IEventSubscriptionAbi>, NcStatus> {
        self.runtime().state.writable()?;
        let endpoint = <Self as crate::TypedWidget<crate::ComboChanged>>::event(self);
        crate::event::subscribe(endpoint, sink)
    }

    /// Connects popup coordination independently from captured selection delivery.
    fn subscribe_popup(&mut self, sink: ComRef<'_, IComboPopupSinkAbi>) -> Result<ComPtr<IEventSubscriptionAbi>, NcStatus> {
        self.runtime().state.writable()?;
        let endpoint = <Self as crate::TypedWidget<crate::ComboSubmitted>>::event(self);
        crate::event::subscribe(endpoint, sink)
    }
}

impl Combo {
    /// Replaces one complete selection after validating its copied text and representable index.
    fn selection(&mut self, index: u64, label: NcCopyFromBufferOffset<'_, u8>, submit: bool) -> Result<(), NcStatus> {
        self.runtime().state.writable()?;
        let index = usize::try_from(index).map_err(|_| NC_E_INVALID_ARG)?;
        let label = crate::com::text(label)?;
        let changed_label = self.label() != label;
        self.com_set_selection(index, label, submit);
        self.runtime_mut().state.record_change(if changed_label {
            GeometryChange::CHANGED
        } else {
            GeometryChange::NO_CHANGE
        });
        Ok(())
    }
}

impl crate::event::ComEvent for ComboChanged {
    type Sink = IComboChangedSinkAbi;

    /// Delivers the captured label rather than rereading possibly newer widget state.
    fn deliver(&self, sink: ComRef<'_, Self::Sink>) -> Result<(), NcStatus> {
        let label = NcString::from_utf8_str(&self.label);
        sink.changed(self.selected as u64, label.as_com_ref())
    }
}

impl crate::event::ComEvent for ComboSubmitted {
    type Sink = IComboPopupSinkAbi;

    /// Converts the existing routed anchor once at the ABI boundary.
    fn deliver(&self, sink: ComRef<'_, Self::Sink>) -> Result<(), NcStatus> {
        sink.submitted(self.open, crate::com::copied_rect(self.anchor))
    }
}
