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

//! Retained list-item widget.
//!
//! `ListItem` represents one selectable row in a retained list. Application payloads stay in the
//! statically linked caller and are mapped from the row's typed activation event.

use crate::{ControlRole};

use super::*;
/// One-shot construction input for a [`ListItem`].
pub struct ListItemParameters {
    /// Initial label displayed for the list item.
    pub label: String,
    /// Optional atlas icon rendered alongside the label.
    pub icon: Option<IconRef>,
    /// Font used for the label.
    pub font: FontRef,
    /// Base widget options.
    pub opt: WidgetOption,
}

impl crate::LeafWidget for ListItem {
    fn measure(&self, style: &Skin, atlas: &AtlasHandle, constraints: Constraints) -> Dimensioni {
        self.preferred_size_widget(style, atlas, constraints)
    }
}

impl ListItemParameters {
    /// Creates list-item parameters with default widget options.
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            icon: None,
            font: FontRef::Role(FontRole::Body),
            opt: WidgetOption::NONE,
        }
    }

    /// Creates list-item parameters with an icon and default widget options.
    #[cfg(test)]
    pub fn with_icon(label: impl Into<String>, icon: IconRef) -> Self {
        Self {
            label: label.into(),
            icon: Some(icon),
            font: FontRef::Role(FontRole::Body),
            opt: WidgetOption::NONE,
        }
    }
}

#[neocogi_com::com_object(neocogi_ui_nodes_contracts::ListItem, ownership = exclusive)]
/// Concrete retained list item and dynamically constructible COM object.
pub struct ListItem {
    /// Non-semantic COM lifecycle and host-backed phase resources.
    runtime: crate::widget_object::WidgetRuntime,
    /// Initialization-only icon.
    icon: Option<IconRef>,
    /// Initialization-only font.
    font: FontRef,
    /// Base widget options.
    /// Mutable label displayed for the item.
    label: String,
}

impl ListItem {
    /// Combines validated semantic parameters with one already bound common runtime.
    pub(crate) fn from_runtime(runtime: crate::widget_object::WidgetRuntime, parameters: ListItemParameters) -> Self {
        Self {
            runtime,
            icon: parameters.icon,
            font: parameters.font,
            label: parameters.label,
        }
    }

    /// Returns the current label.
    pub fn label(&self) -> &str {
        &self.label
    }

    /// Replaces the label without recording a user submission.
    pub fn set_label(&mut self, label: impl Into<String>) {
        self.label = label.into();
    }

    /// Measures the row label and optional icon.
    fn preferred_size_widget(&self, style: &Skin, atlas: &AtlasHandle, _constraints: Constraints) -> Dimensioni {
        let padding = style.metrics().padding.max(0);
        let mut width = padding.saturating_mul(2);
        let mut visual_h = 0;
        if let Some(icon) = &self.icon {
            let size = atlas.get_icon_size(icon.resolve(style, atlas));
            width = width.saturating_add(size.width.max(0)).saturating_add(padding);
            visual_h = size.height;
        }
        if !self.label.is_empty() {
            width = width.saturating_add(text_size(style, atlas, &self.font, &self.label).width.max(0));
        }
        let height = content_height(style, atlas, &self.font, visual_h);
        Dimensioni::new(width.max(0), height)
    }

    /// Paints row highlight, optional icon, and label.
    fn paint_widget(&self, ctx: &mut WidgetPaintCtx<'_>) {
        let bounds = ctx.local_rect();

        // The default normal role is transparent; image themes may still provide ordinary row art.
        ctx.draw_control_center(ControlRole::Item, bounds);

        let mut text_rect = bounds;
        if let Some(icon) = &self.icon {
            let icon = icon.resolve(ctx.skin(), ctx.atlas());
            // Icons consume the left padding + icon width before the text region starts.
            let padding = ctx.skin().metrics().padding.max(0);
            let icon_size = ctx.atlas().get_icon_size(icon);
            // Skin values and retained allocations can independently reach coordinate limits.
            // Saturating the nonnegative extents keeps paint total while preserving ordinary
            // geometry exactly.
            let icon_width = icon_size.width.max(0);
            let icon_height = icon_size.height.max(0);
            let icon_x = bounds.x.saturating_add(padding);
            let icon_y = bounds.y.saturating_add(bounds.height.saturating_sub(icon_height).max(0) / 2);
            let icon_rect = rect(icon_x, icon_y, icon_width, icon_height);
            let consumed = icon_width.saturating_add(padding.saturating_mul(2));
            text_rect.x = text_rect.x.saturating_add(consumed);
            text_rect.width = text_rect.width.saturating_sub(consumed).max(0);
            let color = ctx.control_content_color(ControlRole::Item);
            ctx.draw_icon(icon, icon_rect, color);
        }

        if !self.label.is_empty() {
            let font = ctx.skin().resolve_font(ctx.atlas(), &self.font);
            ctx.draw_control_text_with_font(font, &self.label, text_rect, ControlRole::Item, ctx.options());
        }
    }
}

impl Widget for ListItem {
    fn update(&mut self, ctx: &mut WidgetUpdateCtx<'_>, input: Option<&UiInputEvent>) -> crate::Change {
        if ctx.action(input, self.keyboard_behavior()) != Some(KeyboardAction::Activate) {
            return crate::Change::NoChange;
        }
        if self.runtime.state.is_observed() {
            // The constructor captures the label before posting. Borrow it directly
            // so observed rows make one copy and unobserved rows do no snapshot work.
            ctx.posted(neocogi_ui_core_contracts::events::activated().and_then(|event| self.runtime.state.post(event)));
        }

        // Submitting preserves the label and icon used by intrinsic measurement.
        crate::Change::NoChange
    }

    fn paint(&self, ctx: &mut WidgetPaintCtx<'_>) {
        self.paint_widget(ctx);
    }

    fn keyboard_behavior(&self) -> KeyboardBehavior {
        KeyboardBehavior::TAB_STOP | KeyboardBehavior::ACTIVATE_ENTER | KeyboardBehavior::ACTIVATE_SPACE
    }
}

#[cfg(test)]
impl From<ListItemParameters> for crate::test_support::algorithm::Algorithm<ListItem> {
    /// Transfers construction data into one concrete row and its typed event source.
    fn from(parameters: ListItemParameters) -> Self {
        Self::new(parameters.opt, |runtime| ListItem::from_runtime(runtime, parameters))
    }
}

impl crate::widget_object::WidgetAlgorithm for ListItem {

}

impl crate::widget_object::HostedWidget for ListItem {
    /// Borrows common lifecycle and resource state without a second list-item representation.
    fn runtime(&self) -> &crate::widget_object::WidgetRuntime {
        &self.runtime
    }

    /// Exclusively borrows common lifecycle and resource state for a COM mutation or phase.
    fn runtime_mut(&mut self) -> &mut crate::widget_object::WidgetRuntime {
        &mut self.runtime
    }
}

impl crate::widget_object::FontState for ListItem {
    /// Borrows the semantic font reference already used by layout and painting.
    fn font(&self) -> &crate::FontRef {
        &self.font
    }
    /// Replaces the same field after the COM resource owner validates the selection.
    fn set_font(&mut self, font: crate::FontRef) {
        self.font = font;
    }
}

impl crate::widget_object::IconState for ListItem {
    /// Replaces the icon without changing row data, reporting only a changed selection.
    fn set_icon(&mut self, icon: Option<crate::IconRef>) -> bool {
        let changed = self.icon != icon;
        self.icon = icon;
        changed
    }
}

#[cfg(test)]
mod tests {
    //! Boundary tests for retained list-item paint geometry.

    use super::*;
    use crate::render::DisplayList;
    use crate::test_support::{test_atlas, test_skin};

    /// Verifies application-provided maximum padding cannot overflow icon or label placement.
    #[test]
    fn extreme_padding_keeps_icon_paint_total() {
        let atlas = test_atlas();
        let mut style = test_skin(&atlas);
        style.metrics_mut().padding = i32::MAX;
        let check = atlas.icon_id("check").expect("test atlas must contain the exact check resource");
        let item = crate::test_support::algorithm::Algorithm::<ListItem>::from(ListItemParameters::with_icon("item", IconRef::named(check)));
        let bounds = rect(0, 0, 20, 20);
        let mut display_list = DisplayList::new();
        let mut ctx = WidgetPaintCtx::new_with_content_geometry(
            bounds,
            &mut display_list,
            bounds,
            &style,
            &atlas,
            true,
            true,
            false,
            false,
            false,
            true,
            WidgetOption::from_bits_retain(item.runtime.state.requested_flags().unwrap().raw() as u32),
        );

        item.paint(&mut ctx);

        // Hover fill remains visible; extreme icon/text geometry may be clipped completely.
        assert!(display_list.debug_operation_count() >= 1);
    }
}

use crate::widget_object::{
    HostedWidget, direct_leaf_layout_impl, direct_widget_phase_impl, set_direct_font_role, set_direct_icon_role, set_direct_named_font, set_direct_named_icon,
};
use neocogi_ui_core_contracts::{GeometryChange, IWidgetHostAbi, FontRole};
use neocogi_ui_nodes_contracts::{ContentIcon, ListItemAbi};
use neocogi_com::*;

direct_widget_phase_impl!(ListItem);
direct_leaf_layout_impl!(ListItem);

impl ListItem {
    /// Constructs the class for the host-bound factory through the registry's typed contract.
    pub(crate) fn create_com(
        host: ComPtr<IWidgetHostAbi>,
        resources: crate::widget_object::SharedAssets,
        label: NcCopyFromBufferOffset<'_, u8>,
        queue: Option<ComPtr<neocogi_com::events::IEventReceiverAbi>>,
    ) -> Result<neocogi_com::ComBox<ListItemAbi>, NcStatus> {
        let mut value = Self::build(host, label, Some(&resources))?;
        // Fix the sole destination before publishing the completed widget.
        value.runtime.state = value.runtime.state.with_events(queue);
        Self::into_box(value).query().map_err(|(status, _unpublished)| status)
    }

    /// Builds one list item with either directly imported or runtime-shared resources.
    fn build(
        host: ComPtr<IWidgetHostAbi>,
        label: NcCopyFromBufferOffset<'_, u8>,
        resources: Option<&crate::widget_object::SharedAssets>,
    ) -> Result<Self, NcStatus> {
        let parameters = crate::ListItemParameters::new(crate::boundary::text(label)?);
        let runtime = crate::widget_object::WidgetRuntime::for_resources(host, parameters.opt, resources)?;
        Ok(Self::from_runtime(runtime, parameters))
    }
}

impl neocogi_ui_nodes_contracts::ListItem for ListItem {
    /// Returns an immutable label snapshot from the concrete row.
    fn text(&self) -> Result<ComPtr<INcStringAbi>, NcStatus> {
        self.runtime().state.readable()?;
        Ok(NcString::from_utf8_str(self.label()))
    }
    /// Validates UTF-8 and records geometry only when the concrete label changes.
    fn set_text(&mut self, input: NcCopyFromBufferOffset<'_, u8>) -> Result<(), NcStatus> {
        self.runtime().state.writable()?;
        let label = crate::boundary::text(input)?;
        if self.label() != label {
            self.set_label(label);
            self.runtime_mut().state.record_change(GeometryChange::Changed);
        }
        Ok(())
    }
    /// Validates a semantic role before changing the row label's font.
    fn set_font_role(&mut self, role: FontRole) -> Result<(), NcStatus> {
        set_direct_font_role(self, role)
    }
    /// Resolves one exact named font before changing the row label's font.
    fn set_named_font(&mut self, name: NcCopyFromBufferOffset<'_, u8>) -> Result<(), NcStatus> {
        set_direct_named_font(self, name)
    }
    /// Validates and applies one semantic icon selection directly through the class contract.
    fn set_icon_role(&mut self, icon: ContentIcon) -> Result<(), NcStatus> {
        set_direct_icon_role(self, icon)
    }
    /// Resolves one exact named icon before replacing the row's existing selection.
    fn set_named_icon(&mut self, name: NcCopyFromBufferOffset<'_, u8>) -> Result<(), NcStatus> {
        set_direct_named_icon(self, name)
    }
}
