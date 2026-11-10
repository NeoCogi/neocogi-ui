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

use crate::nodes::ControlRole;

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

impl crate::nodes::LeafWidget for Combo {
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

#[neocogi_com::com_object(neocogi_ui::abi::nodes::Combo, ownership = exclusive)]
/// Concrete retained combo and dynamically constructible COM object.
pub struct Combo {
    /// Core-owned hierarchy, popup lifecycle, committed selection, and user event source.
    menu: ComPtr<IChoiceMenuAbi>,
    /// Header-only fallback displayed when the generalized choice menu has no selection.
    placeholder: String,
    /// Initialization-only font.
    font: FontRef,
    /// Private menu facts, promoted only during this composite's dispatch scope.
    events: crate::nodes::widget_object::CompositeEvents,
    /// Drops after private children/inboxes so their receivers are released first.
    runtime: crate::nodes::widget_object::WidgetRuntime,
}

impl Combo {
    /// Combines typed semantic state with already bound common widget runtime resources.
    pub(crate) fn from_runtime(
        runtime: crate::nodes::widget_object::WidgetRuntime,
        parameters: ComboParameters,
        menu: ComPtr<IChoiceMenuAbi>,
        placeholder: String,
        events: crate::nodes::widget_object::CompositeEvents,
    ) -> Self {
        Self {
            runtime,
            menu,
            placeholder,
            font: parameters.font,
            events,
        }
    }

    /// Copies the rendered header text for the infallible private paint algorithm.
    fn display_text(&self) -> String {
        // The public COM observation below preserves errors. Painting cannot return a status, and
        // the private state object remains alive and valid for the header's complete lifetime.
        match self.menu.selection() {
            Ok(ChoiceSelection::Selected(_)) => self.menu.selected_label().and_then(|text| text.to_string()).unwrap_or_default(),
            Ok(ChoiceSelection::None) | Err(_) => self.placeholder.clone(),
        }
    }

    /// Reconstructs the header border box used as the popup's exact outer-width anchor.
    fn popup_anchor(&self, ctx: &WidgetUpdateCtx<'_>) -> neocogi_ui::abi::core::Rect {
        // Traversal intentionally gives widgets their content rectangle. Recover this control's
        // border box with the same normal-state insets used by measurement and layout, including
        // the valid unframed case without introducing theme-dependent pixel constants.
        let options = self.runtime.state.requested_flags().expect("a mounted combo update has readable common state");
        let options = WidgetOption::from_bits_retain(u32::from(options.raw()));
        let insets = if options.intersects(WidgetOption::FRAME) {
            crate::nodes::frame::normal_frame_visual(ctx.skin(), self.frame_appearance_role())
                .patch
                .insets
                .normalized()
        } else {
            crate::nodes::SliceInsets::ZERO
        };
        let header = crate::nodes::frame::outer_from_content(ctx.screen_content_rect(), insets);
        neocogi_ui::abi::core::Rect {
            x: header.x,
            y: header.y,
            width: header.width,
            height: header.height,
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
        let indicator = atlas.get_icon_size(style.resolve_icon_role(atlas, crate::nodes::IconRole::ExpandDown));
        let width = padding.saturating_mul(3).saturating_add(text_w).saturating_add(indicator.width.max(0));
        let height = content_height(style, atlas, &self.font, indicator.height);
        Dimensioni::new(width, height)
    }

    /// Converts header activation into one private popup-state-machine transition.
    fn update_widget(&mut self, ctx: &mut WidgetUpdateCtx<'_>, input: Option<&UiInputEvent>) -> Result<(), neocogi_com::NcStatus> {
        if let Some(action) = ctx.action(input, self.keyboard_behavior()) {
            let anchor = self.popup_anchor(ctx);
            match action {
                KeyboardAction::Activate => {
                    self.menu.toggle_at(anchor)?;
                }
                KeyboardAction::Expand => {
                    self.menu.show_at(anchor)?;
                }
                KeyboardAction::Collapse => {
                    self.menu.hide()?;
                }
                KeyboardAction::Decrease | KeyboardAction::Increase => {}
            }
        }
        Ok(())
    }

    /// Paints the combo header from already-committed retained state.
    fn paint_widget(&self, ctx: &mut WidgetPaintCtx<'_>) {
        let header = ctx.local_rect();
        let indicator_id = ctx.skin().resolve_icon_role(ctx.atlas(), crate::nodes::IconRole::ExpandDown);
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

    fn update(&mut self, ctx: &mut WidgetUpdateCtx<'_>, input: Option<&UiInputEvent>) -> Result<crate::nodes::Change, neocogi_com::NcStatus> {
        self.update_widget(ctx, input)?;

        // Popup coordination is deferred; the header retains the same geometry.
        Ok(crate::nodes::Change::NoChange)
    }

    fn paint(&self, ctx: &mut WidgetPaintCtx<'_>) {
        self.paint_widget(ctx);
    }

    fn keyboard_behavior(&self) -> KeyboardBehavior {
        KeyboardBehavior::TAB_STOP | KeyboardBehavior::ACTIVATE_ENTER | KeyboardBehavior::ACTIVATE_SPACE | KeyboardBehavior::POPUP
    }
}

impl crate::nodes::widget_object::WidgetAlgorithm for Combo {}

impl crate::nodes::widget_object::HostedWidget for Combo {
    /// Borrows common lifecycle and resource state without a second combo representation.
    fn runtime(&self) -> &crate::nodes::widget_object::WidgetRuntime {
        &self.runtime
    }

    /// Exclusively borrows common lifecycle and resource state for a COM mutation or phase.
    fn runtime_mut(&mut self) -> &mut crate::nodes::widget_object::WidgetRuntime {
        &mut self.runtime
    }
}
impl crate::nodes::widget_object::FontState for Combo {
    /// Measurement and paint read this same existing font selection.
    fn font(&self) -> &FontRef {
        &self.font
    }
    /// The common COM helper validates ownership before installing this field.
    fn set_font(&mut self, font: FontRef) {
        self.font = font;
    }
}
use crate::nodes::widget_object::{HostedWidget, direct_leaf_layout_impl, direct_widget_phase_impl, set_direct_font_role, set_direct_named_font};
use neocogi_ui::abi::core::{ChoiceMenuEntry, ChoiceSelection, FontRole, GeometryChange, IChoiceMenuAbi, IWidgetHostAbi};
use neocogi_ui::abi::nodes::ComboAbi;
use neocogi_com::{ComPtr, INcStringAbi, INcStringComPtrExt, NcCopyFromBufferOffset, NcStatus, NcString};

direct_widget_phase_impl!(Combo);
direct_leaf_layout_impl!(Combo);

impl Combo {
    /// Constructs the header around the typed state also retained by its composition controller.
    pub(crate) fn create_com(
        host: ComPtr<IWidgetHostAbi>,
        resources: crate::nodes::widget_object::SharedAssets,
        menu: ComPtr<IChoiceMenuAbi>,
        placeholder: String,
        events: crate::nodes::widget_object::CompositeEvents,
        queue: Option<ComPtr<neocogi_ui::events::IEventReceiverAbi>>,
    ) -> Result<neocogi_com::ComBox<ComboAbi>, NcStatus> {
        let parameters = crate::nodes::ComboParameters::new();
        let mut runtime = crate::nodes::widget_object::WidgetRuntime::for_resources(host, parameters.opt, Some(&resources))?;
        runtime.state = runtime.state.with_events(queue);
        Self::into_box(Self::from_runtime(runtime, parameters, menu, placeholder, events))
            .query()
            .map_err(|(status, _unpublished)| status)
    }
}

impl neocogi_ui::abi::nodes::Combo for Combo {
    /// Observes the core-owned row policy and preserves provider errors.
    fn max_visible_items(&self) -> Result<u32, NcStatus> {
        self.runtime().state.readable()?;
        self.menu.max_visible_items()
    }

    /// Admits mutation before forwarding and invalidates only a real policy edit.
    fn set_max_visible_items(&mut self, count: u32) -> Result<(), NcStatus> {
        self.runtime().state.writable()?;
        let previous = self.menu.max_visible_items()?;
        self.menu.set_max_visible_items(count)?;
        if previous != count {
            self.runtime_mut().state.record_change(GeometryChange::Changed);
        }
        Ok(())
    }

    /// Returns an explicit tagged optional position from the authoritative model.
    fn selection(&self) -> Result<ChoiceSelection, NcStatus> {
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
        entries: NcCopyFromBufferOffset<'_, ChoiceMenuEntry>,
        selection: ChoiceSelection,
    ) -> Result<(), NcStatus> {
        self.runtime().state.writable()?;
        self.menu.replace(menu_text, entries, selection)?;
        self.runtime_mut().state.record_change(GeometryChange::Changed);
        Ok(())
    }

    /// Silently changes selection with strict range validation and truthful geometry invalidation.
    fn set_selection(&mut self, selection: ChoiceSelection) -> Result<(), NcStatus> {
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

/// Menu input is private; applications observe the public combo identity.
impl neocogi_ui::abi::core::IWidgetDispatcher for Combo {
    /// Relays one immutable menu snapshot without copying its payload.
    fn dispatch_next(&mut self) -> Result<bool, NcStatus> {
        self.events.dispatch_next(&mut self.runtime.state)
    }
}

#[cfg(all(test, feature = "test-support"))]
mod limit_provider_tests {
    use super::*;
    use crate::nodes::test_support::algorithm::Algorithm;
    use neocogi_ui::abi::core::{IChoiceMenu, IChoiceMenuVtblFor};

    #[neocogi_com::com_object(interfaces(IChoiceMenu))]
    /// A valid alternate provider makes configuration failures observable across COM.
    struct FailingChoice {
        /// Fixed failure injection needs no shared mutation or test-only production path.
        read_error: Option<NcStatus>,
    }
    impl IChoiceMenu for FailingChoice {
        /// Either returns its installed policy or an explicit observation failure.
        fn max_visible_items(&self) -> Result<u32, NcStatus> {
            self.read_error.map_or(Ok(10), Err)
        }
        /// Fails independently after a successful observation, without changing policy.
        fn set_max_visible_items(&mut self, _: u32) -> Result<(), NcStatus> {
            Err(neocogi_com::NC_E_NOT_FOUND)
        }
        /// This fixture has no selected leaf.
        fn selection(&self) -> Result<ChoiceSelection, NcStatus> {
            Ok(ChoiceSelection::None)
        }
        /// Explicit absence has an independently owned empty label.
        fn selected_label(&self) -> Result<ComPtr<INcStringAbi>, NcStatus> {
            Ok(NcString::from_utf8_str(""))
        }
        /// Configuration failure is independent of its empty choice hierarchy.
        fn choice_count(&self) -> Result<u64, NcStatus> {
            Ok(0)
        }
        /// No ordinal is valid in the empty fixture.
        fn choice_label(&self, _: u64) -> Result<ComPtr<INcStringAbi>, NcStatus> {
            Err(neocogi_com::NC_E_INVALID_ARG)
        }
        /// Model edits are outside this provider's deliberately limited test contract.
        fn replace(&mut self, _: NcCopyFromBufferOffset<'_, u8>, _: NcCopyFromBufferOffset<'_, ChoiceMenuEntry>, _: ChoiceSelection) -> Result<(), NcStatus> {
            Err(neocogi_com::NC_E_INVALID_ARG)
        }
        /// No nonempty selection is supported by this fixture.
        fn set_selection(&mut self, _: ChoiceSelection) -> Result<(), NcStatus> {
            Err(neocogi_com::NC_E_INVALID_ARG)
        }
        /// This empty test model cannot open a popup.
        fn show_at(&mut self, _: neocogi_ui::abi::core::Rect) -> Result<(), NcStatus> {
            Ok(())
        }
        /// There is no open branch to dismiss.
        fn hide(&mut self) -> Result<(), NcStatus> {
            Ok(())
        }
        /// Empty-model toggles leave it closed.
        fn toggle_at(&mut self, _: neocogi_ui::abi::core::Rect) -> Result<(), NcStatus> {
            Ok(())
        }
        /// Retiring this configuration fixture owns no forest resources.
        fn close(&mut self) -> Result<(), NcStatus> {
            Ok(())
        }
    }

    /// Both dependencies and the Combo entrypoints are real COM objects with ordinary call gates.
    #[test]
    fn visible_limit_preserves_alternate_provider_errors() {
        for read_error in [Some(neocogi_com::NC_E_INVALID_ARG), None] {
            let mut combo = Algorithm::new(WidgetOption::NONE, |runtime| {
                let menu = FailingChoice::from_value(FailingChoice { read_error }).query::<IChoiceMenuAbi>().unwrap();
                Combo::from_runtime(
                    runtime,
                    ComboParameters::new(),
                    menu,
                    String::new(),
                    crate::nodes::widget_object::CompositeEvents::new(false).0,
                )
            })
            .map(|value| Combo::into_box(value).query::<ComboAbi>().unwrap());
            if let Some(status) = read_error {
                assert_eq!(combo.max_visible_items(), Err(status));
                assert_eq!(combo.set_max_visible_items(5), Err(status));
            } else {
                assert_eq!(combo.max_visible_items(), Ok(10));
                assert_eq!(combo.set_max_visible_items(5), Err(neocogi_com::NC_E_NOT_FOUND));
                assert_eq!(combo.max_visible_items(), Ok(10));
            }
        }
    }
}
