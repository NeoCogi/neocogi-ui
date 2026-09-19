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
//! the core-owned `IChoiceMenu` also used by the ordinary widget-backed popup; all mutation crosses
//! that public typed boundary through an exclusive receiver rather than shared interior mutability.

use crate::ControlRole;

use super::*;

/// Contract-defined visible fallback for a combo with no committed selection.
pub(crate) const DEFAULT_COMBO_PLACEHOLDER: &str = "Select…";

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

#[neocogi_com::com_object(neocogi_ui_nodes_contracts::Combo, ports(neocogi_ui_nodes_contracts::Combo::changed = {
        self.runtime().state.writable()?;
        Ok(self.menu.changed())
    }))]
/// Concrete retained combo and dynamically constructible COM object.
pub struct Combo {
    /// Non-semantic COM lifecycle and host-backed phase resources.
    runtime: crate::widget_object::WidgetRuntime,
    /// Core-owned hierarchy, popup lifecycle, committed selection, and user event source.
    menu: ComPtr<IChoiceMenuAbi>,
    /// Header-only fallback displayed when the generalized choice menu has no selection.
    placeholder: String,
    /// Initialization-only font.
    font: FontRef,
}

impl Combo {
    /// Combines typed semantic state with already bound common widget runtime resources.
    pub(crate) fn from_runtime(
        runtime: crate::widget_object::WidgetRuntime,
        parameters: ComboParameters,
        menu: ComPtr<IChoiceMenuAbi>,
        placeholder: String,
    ) -> Self {
        Self {
            runtime,
            menu,
            placeholder,
            font: parameters.font,
        }
    }

    /// Copies the rendered header text for the infallible private paint algorithm.
    fn display_text(&self) -> String {
        // The public COM observation below preserves errors. Painting cannot return a status, and
        // the private state object remains alive and valid for the header's complete lifetime.
        match self
            .menu
            .selection()
            .and_then(|selection| ChoiceSelection::try_from(selection).map_err(|_| neocogi_com::NC_E_INVALID_ARG))
        {
            Ok(ChoiceSelection::Selected(_)) => self.menu.selected_label().and_then(|text| text.to_string()).unwrap_or_default(),
            Ok(ChoiceSelection::None) | Err(_) => self.placeholder.clone(),
        }
    }

    /// Reconstructs the header border box used as the popup's exact outer-width anchor.
    fn popup_anchor(&self, ctx: &WidgetUpdateCtx<'_>) -> neocogi_ui_core_contracts::Rect {
        // Traversal intentionally gives widgets their content rectangle. Recover this control's
        // border box with the same normal-state insets used by measurement and layout, including
        // the valid unframed case without introducing theme-dependent pixel constants.
        let options = self.runtime.state.options().expect("a mounted combo update has readable common state");
        let options = WidgetOption::from_bits_retain(u32::from(options.raw()));
        let insets = if options.intersects(WidgetOption::FRAME) {
            crate::frame::normal_frame_visual(ctx.skin(), self.frame_appearance_role())
                .patch
                .insets
                .normalized()
        } else {
            crate::SliceInsets::ZERO
        };
        let header = crate::frame::outer_from_content(ctx.screen_content_rect(), insets);
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
        let mut text_w = text_size(style, atlas, &self.font, &self.placeholder).width.max(0);
        let count = self.menu.choice_count().ok().and_then(|count| usize::try_from(count).ok()).unwrap_or(0);
        for ordinal in 0..count {
            let ordinal = u64::try_from(ordinal).expect("usize fits the generated u64 choice ordinal on supported targets");
            if let Ok(label) = self.menu.choice_label(ordinal).and_then(|text| text.to_string()) {
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
            match action {
                KeyboardAction::Activate => {
                    let _ = self.menu.toggle_at(anchor);
                }
                KeyboardAction::Expand => {
                    let _ = self.menu.show_at(anchor);
                }
                KeyboardAction::Collapse => {
                    let _ = self.menu.hide();
                }
                KeyboardAction::Decrease | KeyboardAction::Increase => {}
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

impl crate::widget_object::WidgetAlgorithm for Combo {
    /// Closes event delivery and retires the popup definition with the removed header.
    fn close_events(&mut self) {
        let _ = self.menu.close();
    }
}

impl Drop for Combo {
    /// Retires an unmounted combo's popup when its final application reference is released.
    fn drop(&mut self) {
        // Mounted headers request the same idempotent transition from `close_events`; this path
        // covers construction followed by release without insertion into the retained tree.
        let _ = self.menu.close();
    }
}

impl crate::widget_object::HostedWidget for Combo {
    /// Borrows common lifecycle and resource state without a second combo representation.
    fn runtime(&self) -> &crate::widget_object::WidgetRuntime {
        &self.runtime
    }

    /// Exclusively borrows common lifecycle and resource state for a COM mutation or phase.
    fn runtime_mut(&mut self) -> &mut crate::widget_object::WidgetRuntime {
        &mut self.runtime
    }
}
impl crate::widget_object::FontState for Combo {
    /// Measurement and paint read this same existing font selection.
    fn font(&self) -> &FontRef {
        &self.font
    }
    /// The common COM helper validates ownership before installing this field.
    fn set_font(&mut self, font: FontRef) {
        self.font = font;
    }
}
use crate::widget_object::{HostedWidget, direct_leaf_layout_impl, direct_widget_phase_impl, set_direct_font_role, set_direct_named_font};
use neocogi_ui_core_contracts::{
    ChoiceMenuEntryAbi, ChoiceSelection, ChoiceSelectionAbi, FontRole, GeometryChange, IChoiceMenuAbi, IChoiceMenuPorts, IWidgetHostAbi,
};
use neocogi_ui_nodes_contracts::ComboAbi;
use neocogi_com::{ComPtr, INcStringAbi, INcStringComPtrExt, NcCopyFromBufferOffset, NcStatus, NcString};

direct_widget_phase_impl!(Combo);
direct_leaf_layout_impl!(Combo);

impl Combo {
    /// Constructs the header around the typed state also retained by its composition controller.
    pub(crate) fn create_com(
        host: ComPtr<IWidgetHostAbi>,
        resources: crate::widget_object::SharedAssets,
        menu: ComPtr<IChoiceMenuAbi>,
        placeholder: String,
    ) -> Result<ComPtr<ComboAbi>, NcStatus> {
        let value = Self::build(host, &resources, menu, placeholder)?;
        Ok(Self::from_value(value).query().expect("Combo declares its callable class contract"))
    }

    /// Builds one header with runtime-shared resources and no semantic-state mirror.
    fn build(
        host: ComPtr<IWidgetHostAbi>,
        resources: &crate::widget_object::SharedAssets,
        menu: ComPtr<IChoiceMenuAbi>,
        placeholder: String,
    ) -> Result<Self, NcStatus> {
        let parameters = crate::ComboParameters::new();
        let runtime = crate::widget_object::WidgetRuntime::for_resources(host, parameters.opt, Some(resources))?;
        Ok(Self::from_runtime(runtime, parameters, menu, placeholder))
    }
}

impl neocogi_ui_nodes_contracts::Combo for Combo {
    /// Returns an explicit tagged optional position from the authoritative model.
    fn selection(&self) -> Result<ChoiceSelectionAbi, NcStatus> {
        self.runtime().state.readable()?;
        self.menu.selection()
    }

    /// Copies the selected label or fallback without exposing the plugin's string allocation.
    fn display_text(&self) -> Result<ComPtr<INcStringAbi>, NcStatus> {
        self.runtime().state.readable()?;
        Ok(NcString::from_utf8_str(&self.display_text()))
    }

    /// Returns the exact number of labels in the authoritative sequence.
    fn choice_count(&self) -> Result<u64, NcStatus> {
        self.runtime().state.readable()?;
        self.menu.choice_count()
    }

    /// Copies one checked leaf label without exposing shared-model storage.
    fn choice_label(&self, ordinal: u64) -> Result<ComPtr<INcStringAbi>, NcStatus> {
        self.runtime().state.readable()?;
        self.menu.choice_label(ordinal)
    }

    /// Validates and copies a complete candidate before changing any live model field.
    fn replace_menu(
        &mut self,
        menu_text: NcCopyFromBufferOffset<'_, u8>,
        entries: NcCopyFromBufferOffset<'_, ChoiceMenuEntryAbi>,
        selection: ChoiceSelectionAbi,
    ) -> Result<(), NcStatus> {
        self.runtime().state.writable()?;
        self.menu.replace(menu_text, entries, selection)?;
        self.runtime_mut().state.record_change(GeometryChange::Changed);
        Ok(())
    }

    /// Silently changes selection with strict range validation and truthful geometry invalidation.
    fn set_selection(&mut self, selection: ChoiceSelectionAbi) -> Result<(), NcStatus> {
        self.runtime().state.writable()?;
        self.menu.set_selection(selection)?;
        // Header measurement reserves the widest label, so changing only selection is paint-only.
        self.runtime_mut().state.record_change(GeometryChange::NoChange);
        Ok(())
    }

    /// Copies the fallback independently of whether the header currently needs it.
    fn placeholder(&self) -> Result<ComPtr<INcStringAbi>, NcStatus> {
        self.runtime().state.readable()?;
        Ok(NcString::from_utf8_str(&self.placeholder))
    }

    /// Validates a complete fallback before mutating model state.
    fn set_placeholder(&mut self, placeholder: NcCopyFromBufferOffset<'_, u8>) -> Result<(), NcStatus> {
        self.runtime().state.writable()?;
        if placeholder.offset() != 0 {
            return Err(neocogi_com::NC_E_INVALID_ARG);
        }
        let placeholder = std::str::from_utf8(placeholder.as_slice()).map_err(|_| neocogi_com::NC_E_INVALID_ARG)?;
        if self.placeholder != placeholder {
            self.placeholder.clear();
            self.placeholder.push_str(placeholder);
            self.runtime_mut().state.record_change(GeometryChange::Changed);
        }
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
}
