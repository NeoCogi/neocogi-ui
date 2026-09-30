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

//! Checkbox widget state and rendering.
//!
//! The checkbox toggles persistent boolean state on pointer or keyboard activation and paints the
//! atlas check icon when selected.

use crate::{ControlRole};

use super::*;

/// One-shot construction input for a [`Checkbox`].
///
/// The label, font, and widget options configure the retained runtime. The checked value seeds
/// Applications change the checked value through the typed COM checkbox capability.
pub struct CheckboxParameters {
    /// Label displayed beside the checkbox square.
    pub label: String,
    /// Initial checked value.
    pub checked: bool,
    /// Font used to measure and paint the label.
    pub font: FontRef,
    /// Base widget options used for interaction and painting.
    pub opt: WidgetOption,
}

impl crate::LeafWidget for Checkbox {
    fn measure(&self, style: &Skin, atlas: &AtlasHandle, _constraints: Constraints) -> Dimensioni {
        self.preferred_size(style, atlas)
    }
}

impl CheckboxParameters {
    /// Creates checkbox parameters with the body font and no special widget options.
    pub fn new(label: impl Into<String>, checked: bool) -> Self {
        Self {
            label: label.into(),
            checked,
            font: FontRef::Role(FontRole::Body),
            opt: WidgetOption::NONE,
        }
    }
}

#[neocogi_com::com_class(neocogi_ui_nodes_contracts::Checkbox, ownership = exclusive)]
/// Concrete retained checkbox and dynamically constructible COM object.
pub struct Checkbox {
    /// Non-semantic COM lifecycle and host-backed phase resources.
    runtime: crate::widget_object::WidgetRuntime,
    /// Label displayed beside the checkbox square.
    label: String,
    /// Font used for the label.
    font: FontRef,
    /// Base widget options.
    /// Current checked value.
    checked: bool,
    /// Single lifetime event slot; the constructor fixes the parent receiver before publishing the widget.
    events: crate::widget_object::WidgetEvents<neocogi_ui_nodes_contracts::CheckboxEventQueueAbi>,
}

impl Checkbox {
    /// Combines validated semantic parameters with one already bound common runtime.
    pub(crate) fn from_runtime(runtime: crate::widget_object::WidgetRuntime, parameters: CheckboxParameters) -> Self {
        Self {
            runtime,
            label: parameters.label,
            font: parameters.font,
            checked: parameters.checked,
            events: Default::default(),
        }
    }

    /// Replaces the checked value without recording user interaction.
    pub fn set_checked(&mut self, checked: bool) {
        self.checked = checked;
    }

    /// Returns the current checked value.
    pub const fn checked(&self) -> bool {
        self.checked
    }

    /// Measures the checkbox square plus optional label.
    fn preferred_size(&self, style: &Skin, atlas: &AtlasHandle) -> Dimensioni {
        let padding = style.metrics().padding.max(0);
        let check_icon = atlas.get_icon_size(style.resolve_icon_role(atlas, crate::IconRole::Check));
        let height = content_height(style, atlas, &self.font, check_icon.height);
        let mut width = padding.saturating_mul(2).saturating_add(height.max(0));
        if !self.label.is_empty() {
            width = width
                .saturating_add(text_size(style, atlas, &self.font, &self.label).width.max(0))
                .saturating_add(padding);
        }
        Dimensioni::new(width.max(0), height)
    }

    /// Paints the checkbox square, checked icon, and label.
    fn paint_widget(&self, checked: bool, ctx: &mut WidgetPaintCtx<'_>) {
        let bounds = ctx.local_rect();
        let box_rect = rect(bounds.x, bounds.y, bounds.height, bounds.height);
        let box_content = ctx.draw_control(ControlRole::Checkbox, box_rect);
        if checked {
            // The check glyph is the complete persistent-state cue; the surrounding square remains
            // governed only by its ordinary pointer, keyboard, and disabled interaction state.
            let color = ctx.control_content_color(ControlRole::Checkbox);
            if let Some(box_content) = box_content {
                let check = ctx.skin().resolve_icon_role(ctx.atlas(), crate::IconRole::Check);
                ctx.draw_icon(check, box_content, color);
            }
        }
        let text_rect = rect(bounds.x + box_rect.width, bounds.y, bounds.width - box_rect.width, bounds.height);
        if !self.label.is_empty() {
            let font = ctx.skin().resolve_font(ctx.atlas(), &self.font);
            ctx.draw_control_text_with_font(font, &self.label, text_rect, ControlRole::Checkbox, ctx.options());
        }
    }
}

impl Widget for Checkbox {
    fn update(&mut self, ctx: &mut WidgetUpdateCtx<'_>, input: Option<&UiInputEvent>) -> crate::Change {
        if ctx.action(input, self.keyboard_behavior()) != Some(KeyboardAction::Activate) {
            return crate::Change::NoChange;
        }

        // Windows-style checkboxes toggle with Space; Enter remains available to a default button.
        self.checked = !self.checked;
        let checked = self.checked;
        ctx.posted(
            self.events
                .post(&neocogi_ui_nodes_contracts::CheckboxEvent::Changed(crate::CheckboxChanged { checked })),
        );

        // The check mark changes paint only; both states reserve the same icon space.
        crate::Change::NoChange
    }

    fn paint(&self, ctx: &mut WidgetPaintCtx<'_>) {
        self.paint_widget(self.checked, ctx);
    }

    fn keyboard_behavior(&self) -> KeyboardBehavior {
        KeyboardBehavior::TAB_STOP | KeyboardBehavior::ACTIVATE_SPACE
    }
}

impl crate::widget_object::WidgetAlgorithm for Checkbox {

}

impl crate::widget_object::HostedWidget for Checkbox {
    /// Borrows common lifecycle and resource state without a second checkbox representation.
    fn runtime(&self) -> &crate::widget_object::WidgetRuntime {
        &self.runtime
    }

    /// Exclusively borrows common lifecycle and resource state for a COM mutation or phase.
    fn runtime_mut(&mut self) -> &mut crate::widget_object::WidgetRuntime {
        &mut self.runtime
    }
}

impl Checkbox {
    /// Borrows the existing semantic label for a synchronous immutable snapshot.
    pub(crate) fn com_label(&self) -> &str {
        &self.label
    }
    /// Replaces the existing label without publishing a user submission.
    pub(crate) fn com_set_label(&mut self, text: &str) {
        self.label.clear();
        self.label.push_str(text);
    }
}

impl crate::widget_object::FontState for Checkbox {
    /// Borrows the semantic font reference already used by layout and painting.
    fn font(&self) -> &crate::FontRef {
        &self.font
    }
    /// Replaces the same field after the COM resource owner validates the selection.
    fn set_font(&mut self, font: crate::FontRef) {
        self.font = font;
    }
}

use crate::widget_object::{HostedWidget, direct_leaf_layout_impl, direct_widget_phase_impl, set_direct_font_role, set_direct_named_font};
use neocogi_ui_core_contracts::{GeometryChange, IWidgetHostAbi, FontRole};
use neocogi_ui_nodes_contracts::{CheckboxAbi};
use neocogi_com::*;

direct_widget_phase_impl!(Checkbox);
direct_leaf_layout_impl!(Checkbox);

impl Checkbox {
    /// Constructs the class for the host-bound factory through the same typed contract used by the registry.
    pub(crate) fn create_com(
        host: ComPtr<IWidgetHostAbi>,
        resources: crate::widget_object::SharedAssets,
        checked: bool,
        label: NcCopyFromBufferOffset<'_, u8>,
        queue: Option<ComPtr<neocogi_ui_nodes_contracts::CheckboxEventQueueAbi>>,
    ) -> Result<neocogi_com::ComBox<CheckboxAbi>, NcStatus> {
        let mut value = Self::build(host, checked, label, Some(&resources))?;
        // Fix the sole destination before publishing the completed widget.
        Self::into_box_cyclic::<CheckboxAbi>(move |origin| {
            value.events = crate::widget_object::WidgetEvents::with_queue(queue, origin);
            value
        })?
        .query()
    }

    /// Builds one checkbox with either directly imported or runtime-shared resources.
    fn build(
        host: ComPtr<IWidgetHostAbi>,
        checked: bool,
        label: NcCopyFromBufferOffset<'_, u8>,
        resources: Option<&crate::widget_object::SharedAssets>,
    ) -> Result<Self, NcStatus> {
        let parameters = crate::CheckboxParameters::new(crate::boundary::text(label)?, checked);
        let runtime = crate::widget_object::WidgetRuntime::for_resources(host, parameters.opt, resources)?;
        Ok(Self::from_runtime(runtime, parameters))
    }
}

impl neocogi_ui_nodes_contracts::Checkbox for Checkbox {
    /// Reads the same boolean state used by pointer/Space updates and paint.
    fn checked(&self) -> Result<bool, NcStatus> {
        self.runtime().state.readable()?;
        Ok(self.checked())
    }
    /// Checked state changes paint only; both states reserve the same glyph space.
    fn set_checked(&mut self, checked: bool) -> Result<(), NcStatus> {
        self.runtime().state.writable()?;
        self.set_checked(checked);
        Ok(())
    }
    /// Copies a label into an independently owned immutable COM string.
    fn label(&self) -> Result<ComPtr<INcStringAbi>, NcStatus> {
        self.runtime().state.readable()?;
        Ok(NcString::from_utf8_str(self.com_label()))
    }
    /// Commits valid UTF-8 atomically and preserves a prior pending change.
    fn set_label(&mut self, input: NcCopyFromBufferOffset<'_, u8>) -> Result<(), NcStatus> {
        self.runtime().state.writable()?;
        let input = crate::boundary::text(input)?;
        if self.com_label() != input {
            self.com_set_label(input);
            self.runtime_mut().state.record_change(GeometryChange::Changed);
        }
        Ok(())
    }
    /// Validates a semantic font role before changing the Checkbox's measurement input.
    fn set_font_role(&mut self, role: FontRole) -> Result<(), NcStatus> {
        set_direct_font_role(self, role)
    }
    /// Resolves one exact named font before replacing the Checkbox's selection.
    fn set_named_font(&mut self, name: NcCopyFromBufferOffset<'_, u8>) -> Result<(), NcStatus> {
        set_direct_named_font(self, name)
    }
}

/// Ordinary leaves reserve no duplicate ancestors or per-instance descriptor tables.
#[cfg(all(test, target_pointer_width = "64"))]
#[test]
fn com_object_control_is_independent_of_interface_count() {
    // The common header occupies three words, including the combined borrow state.
    // Box owns exactly this storage; there is no hidden Arc header to add.
    // No per-interface fields remain; handles carry their selected tables.
    // The constructor receiver is not an interface implemented by the widget.
    assert_eq!(Checkbox::COM_INTERFACE_COUNT, 4);
    println!(
        "Checkbox: state={}, allocation={}",
        core::mem::size_of::<Checkbox>(),
        core::mem::size_of::<CheckboxComObject>()
    );
    assert_eq!(core::mem::offset_of!(CheckboxComObject, value), 24);
    assert_eq!(core::mem::size_of::<CheckboxComObject>(), core::mem::size_of::<Checkbox>() + 24);
}
