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

//! Header widget for the runtime-owned combo-box composite.
//!
//! This leaf owns widget lifecycle, font selection, and header interaction. Semantic state lives in
//! one private typed COM object also used by the ordinary widget-backed popup; all mutation crosses
//! that boundary through an exclusive receiver rather than shared interior mutability.

use crate::{
    ControlRole,
    combo_box::{ComboPopupIntent, IComboStateAbi},
};

use super::*;

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

#[neocogi_com::com_object(neocogi_ui_core_contracts::Combo)]
/// Concrete retained combo and dynamically constructible COM object.
pub struct Combo {
    /// Non-semantic COM lifecycle and host-backed phase resources.
    runtime: crate::ui_node::widget_object::WidgetRuntime,
    /// Typed capability for the sole authoritative semantic state allocation.
    state: ComPtr<IComboStateAbi>,
    /// Initialization-only font.
    font: FontRef,
}

impl Combo {
    /// Combines typed semantic state with already bound common widget runtime resources.
    pub(crate) fn from_runtime(runtime: crate::ui_node::widget_object::WidgetRuntime, parameters: ComboParameters, state: ComPtr<IComboStateAbi>) -> Self {
        Self { runtime, state, font: parameters.font }
    }

    /// Copies the rendered header text for the infallible private paint algorithm.
    fn display_text(&self) -> String {
        // The public COM observation below preserves errors. Painting cannot return a status, and
        // the private state object remains alive and valid for the header's complete lifetime.
        self.state.display_text().and_then(|text| text.to_string()).unwrap_or_default()
    }

    /// Reconstructs the header border box used as the popup's exact outer-width anchor.
    fn popup_anchor(&self, ctx: &WidgetUpdateCtx<'_>) -> neocogi_ui_core_contracts::Rect {
        // Traversal intentionally gives widgets their content rectangle. Recover this control's
        // border box with the same normal-state insets used by measurement and layout, including
        // the valid unframed case without introducing theme-dependent pixel constants.
        let options = self.runtime.state.options().expect("a mounted combo update has readable common state");
        let options = WidgetOption::from_bits_retain(u32::from(options.raw()));
        let insets = if options.intersects(WidgetOption::FRAME) {
            crate::ui_node::frame::normal_frame_visual(ctx.skin(), self.frame_appearance_role())
                .patch
                .insets
                .normalized()
        } else {
            crate::SliceInsets::ZERO
        };
        let header = crate::ui_node::frame::outer_from_content(ctx.screen_content_rect(), insets);
        neocogi_ui_core_contracts::Rect {
            x: header.x,
            y: header.y.saturating_add(header.height),
            width: header.width,
            height: 1,
        }
    }
}

impl Combo {
    /// Measures the widest possible header label so selection changes remain paint-only.
    fn preferred_size_widget(&self, style: &Skin, atlas: &AtlasHandle, _constraints: Constraints) -> Dimensioni {
        let padding = style.metrics().padding.max(0);
        let placeholder = self.state.placeholder().and_then(|text| text.to_string()).unwrap_or_default();
        let mut text_w = text_size(style, atlas, &self.font, &placeholder).width.max(0);
        let count = self.state.item_count().ok().and_then(|count| usize::try_from(count).ok()).unwrap_or(0);
        for index in 0..count {
            if let Ok(label) = self.state.item(index as u64).and_then(|text| text.to_string()) {
                text_w = text_w.max(text_size(style, atlas, &self.font, &label).width.max(0));
            }
        }
        let indicator = atlas.get_icon_size(style.resolve_icon_role(atlas, crate::IconRole::ExpandDown));
        let width = padding.saturating_mul(3).saturating_add(text_w).saturating_add(indicator.width.max(0));
        let height = content_height(style, atlas, &self.font, indicator.height);
        Dimensioni::new(width, height)
    }

    /// Converts header activation into one private popup-state-machine transition.
    fn update_widget(&mut self, ctx: &mut WidgetUpdateCtx<'_>, input: Option<&UiInputEvent>) {
        if let Some(action) = ctx.action(input, self.keyboard_behavior()) {
            let anchor = self.popup_anchor(ctx);
            let intent = match action {
                KeyboardAction::Activate => Some(ComboPopupIntent::Toggle),
                KeyboardAction::Expand => Some(ComboPopupIntent::Open),
                KeyboardAction::Collapse => Some(ComboPopupIntent::Close),
                KeyboardAction::Decrease | KeyboardAction::Increase => None,
            };
            if let Some(intent) = intent {
                let _ = self.state.request_popup(intent, anchor);
            }
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
        let display_text = self.display_text();
        ctx.draw_control_text_with_font(font, &display_text, text_rect, ControlRole::Combo, ctx.options());

        let indicator_content = ctx.draw_control(ControlRole::Button, indicator);
        let icon_color = ctx.control_content_color(ControlRole::Combo);
        if let Some(indicator_content) = indicator_content {
            ctx.draw_icon(indicator_id, indicator_content, icon_color);
        }
    }
}

impl Widget for Combo {
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

impl crate::ui_node::widget_object::WidgetAlgorithm for Combo {
    /// Closes event delivery and retires the popup definition with the removed header.
    fn close_events(&mut self) {
        let _ = self.state.retire();
    }
}

impl Drop for Combo {
    /// Retires an unmounted combo's popup when its final application reference is released.
    fn drop(&mut self) {
        // Mounted headers request the same idempotent transition from `close_events`; this path
        // covers construction followed by release without insertion into the retained tree.
        let _ = self.state.retire();
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
use crate::ui_node::widget_object::{HostedWidget, direct_leaf_layout_impl, direct_widget_phase_impl, set_direct_font_role, set_direct_named_font};
use neocogi_ui_core_contracts::{
    ComboAbi, ComboItemRange, ComboSelectionAbi, FontRole, GeometryChange, IComboChangedSinkAbi, IEventSubscriptionAbi, IWidgetHostAbi,
};
use neocogi_com::{ComPtr, ComRef, INcStringAbi, INcStringComPtrExt, NcCopyFromBufferOffset, NcStatus};

direct_widget_phase_impl!(Combo);
direct_leaf_layout_impl!(Combo);

impl Combo {
    /// Constructs the header around the typed state also retained by its private popup widget.
    pub(crate) fn create_com(
        host: ComPtr<IWidgetHostAbi>,
        resources: crate::ui_node::widget_object::SharedAssets,
        state: ComPtr<IComboStateAbi>,
    ) -> Result<ComPtr<ComboAbi>, NcStatus> {
        let value = Self::build(host, &resources, state)?;
        Ok(Self::from_value(value).query().expect("Combo declares its callable class contract"))
    }

    /// Builds one header with runtime-shared resources and no semantic-state mirror.
    fn build(host: ComPtr<IWidgetHostAbi>, resources: &crate::ui_node::widget_object::SharedAssets, state: ComPtr<IComboStateAbi>) -> Result<Self, NcStatus> {
        let parameters = crate::ComboParameters::new();
        let runtime = crate::ui_node::widget_object::WidgetRuntime::for_resources(host, parameters.opt, Some(resources))?;
        Ok(Self::from_runtime(runtime, parameters, state))
    }
}

impl neocogi_ui_core_contracts::Combo for Combo {
    /// Returns an explicit tagged optional position from the authoritative model.
    fn selection(&self) -> Result<ComboSelectionAbi, NcStatus> {
        self.runtime().state.readable()?;
        self.state.selection()
    }

    /// Copies the selected label or fallback without exposing the plugin's string allocation.
    fn display_text(&self) -> Result<ComPtr<INcStringAbi>, NcStatus> {
        self.runtime().state.readable()?;
        self.state.display_text()
    }

    /// Returns the exact number of labels in the authoritative sequence.
    fn item_count(&self) -> Result<u64, NcStatus> {
        self.runtime().state.readable()?;
        self.state.item_count()
    }

    /// Copies one checked item without exposing shared-model storage.
    fn item(&self, index: u64) -> Result<ComPtr<INcStringAbi>, NcStatus> {
        self.runtime().state.readable()?;
        self.state.item(index)
    }

    /// Validates and copies a complete candidate before changing any live model field.
    fn replace_items(
        &mut self,
        item_text: NcCopyFromBufferOffset<'_, u8>,
        ranges: NcCopyFromBufferOffset<'_, ComboItemRange>,
        selection: ComboSelectionAbi,
    ) -> Result<(), NcStatus> {
        self.runtime().state.writable()?;
        let changed = self.state.replace_items(item_text, ranges, selection)?;
        self.runtime_mut()
            .state
            .record_change(if changed { GeometryChange::Changed } else { GeometryChange::NoChange });
        Ok(())
    }

    /// Silently changes selection with strict range validation and truthful geometry invalidation.
    fn set_selection(&mut self, selection: ComboSelectionAbi) -> Result<(), NcStatus> {
        self.runtime().state.writable()?;
        let _ = self.state.set_selection(selection)?;
        // Header measurement reserves the widest label, so changing only selection is paint-only.
        self.runtime_mut().state.record_change(GeometryChange::NoChange);
        Ok(())
    }

    /// Copies the fallback independently of whether the header currently needs it.
    fn placeholder(&self) -> Result<ComPtr<INcStringAbi>, NcStatus> {
        self.runtime().state.readable()?;
        self.state.placeholder()
    }

    /// Validates a complete fallback before mutating model state.
    fn set_placeholder(&mut self, placeholder: NcCopyFromBufferOffset<'_, u8>) -> Result<(), NcStatus> {
        self.runtime().state.writable()?;
        let changed = self.state.set_placeholder(placeholder)?;
        self.runtime_mut()
            .state
            .record_change(if changed { GeometryChange::Changed } else { GeometryChange::NoChange });
        Ok(())
    }

    /// Validates a semantic font role before changing the Combo's measurement input.
    fn set_font_role(&mut self, role: FontRole) -> Result<(), NcStatus> {
        set_direct_font_role(self, role)
    }

    /// Resolves one exact named font before replacing the Combo's selection.
    fn set_named_font(&mut self, name: NcCopyFromBufferOffset<'_, u8>) -> Result<(), NcStatus> {
        set_direct_named_font(self, name)
    }

    /// Connects the Combo's captured-selection source directly through its class contract.
    fn subscribe_changed(&mut self, sink: ComRef<'_, IComboChangedSinkAbi>) -> Result<ComPtr<IEventSubscriptionAbi>, NcStatus> {
        self.runtime().state.writable()?;
        self.state.subscribe_changed(sink)
    }
}
