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

//! Retained list-box widget.

use crate::{ControlRole};

use super::*;

/// One-shot construction input for a [`ListBox`].
pub struct ListBoxParameters {
    /// Label displayed for the list box.
    pub label: String,
    /// Optional image rendered alongside the label.
    pub image: Option<microui_redux::Image>,
    /// Font used for the label.
    pub font: FontRef,
    /// Base widget options.
    pub opt: WidgetOption,
}

impl crate::LeafWidget for ListBox {
    fn measure(&self, style: &Skin, atlas: &AtlasHandle, constraints: Constraints) -> Dimensioni {
        self.preferred_size_widget(style, atlas, constraints)
    }
}

impl ListBoxParameters {
    /// Creates list-box parameters with default widget options.
    pub fn new(label: impl Into<String>, image: Option<microui_redux::Image>) -> Self {
        Self {
            label: label.into(),
            image,
            font: FontRef::Role(FontRole::Body),
            opt: WidgetOption::NONE,
        }
    }
}

/// Semantic payload emitted when the user submits a list box.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ListBoxSubmitted {
    /// Exact displayed label at user activation time, independent of later setter changes.
    pub label: String,
}

impl crate::event::ComEvent for ListBoxSubmitted {
    type Sink = microui_contracts::IActivationSinkAbi;

    /// Copies the captured selection label through the ListBox activation contract.
    fn deliver(&self, sink: neocogi_com::ComRef<'_, Self::Sink>) -> Result<(), neocogi_com::NcStatus> {
        crate::event::deliver_activation(&self.label, sink)
    }
}

#[neocogi_com::com_object(microui_contracts::ListBox)]
/// Concrete retained list box and dynamically constructible COM object.
pub struct ListBox {
    /// Non-semantic COM lifecycle and host-backed phase resources.
    runtime: crate::ui_node::widget_object::WidgetRuntime,
    /// Initialization-only label.
    label: String,
    /// Initialization-only image.
    image: Option<microui_redux::Image>,
    /// Width-fitting policy for the current immutable image.
    fit_width: bool,
    /// Initialization-only font.
    font: FontRef,
    /// Base widget options.
    /// Native algorithm fixture policy; production options live only in WidgetState.
    #[cfg(test)]
    opt: WidgetOption,
    /// Runtime-owned source for user submissions.
    submitted_event: crate::EventSource<ListBoxSubmitted>,
}

impl ListBox {
    /// Constructs a retained node and a weak typed handle to its concrete list box.
    #[cfg(test)]
    pub fn create(parameters: ListBoxParameters) -> (crate::TypedWidgetHandle<Self>, crate::Node) {
        let widget = ListBox::from(parameters);
        crate::Node::typed_widget(widget)
    }

    /// Combines validated semantic parameters with one already bound common runtime.
    pub(crate) fn from_runtime(runtime: crate::ui_node::widget_object::WidgetRuntime, parameters: ListBoxParameters) -> Self {
        Self {
            runtime,
            label: parameters.label,
            image: parameters.image,
            fit_width: false,
            font: parameters.font,
            #[cfg(test)]
            opt: parameters.opt,
            submitted_event: crate::EventSource::new(),
        }
    }

    /// Returns the native event endpoint emitted once for every user submission.
    #[cfg(test)]
    pub fn submitted(&self) -> crate::WidgetEventPortHandle<ListBoxSubmitted> {
        <Self as crate::TypedWidget<ListBoxSubmitted>>::event(self)
    }

    /// Measures list-box inline label and optional image.
    fn preferred_size_widget(&self, style: &Skin, atlas: &AtlasHandle, constraints: Constraints) -> Dimensioni {
        let visual = self.image.as_ref().map(image_size);
        if self.fit_width && visual.is_some() && constraints.width.bound().is_some() {
            scaled_visual_content_size(constraints, visual)
        } else {
            inline_content_size(style, atlas, &self.font, &self.label, visual)
        }
    }

    /// Paints list-box frame, label, and optional image.
    fn paint_widget(&self, ctx: &mut WidgetPaintCtx<'_>) {
        let rect = ctx.local_rect();
        if widget_fill_visible(ctx, WidgetFillOption::HOVER | WidgetFillOption::CLICK) {
            ctx.draw_control_center(ControlRole::Button, rect);
        }
        let visual_size = self.image.as_ref().map(image_size);
        let placement = if self.fit_width && visual_size.is_some() {
            place_scaled_visual_content(rect, visual_size)
        } else {
            place_inline_content(rect, ctx.skin(), &self.label, visual_size)
        };
        if !self.label.is_empty() {
            let font = ctx.skin().resolve_font(ctx.atlas(), &self.font);
            ctx.draw_control_text_with_font(font, &self.label, placement.text, ControlRole::Button, ctx.options());
        }
        if let (Some(image), Some(visual)) = (self.image.as_ref(), placement.visual) {
            // List-box labels remain theme-colored while external image content preserves the
            // exact source RGBA values supplied to the renderer.
            ctx.draw_texture(visual, crate::color(255, 255, 255, 255), image.as_interface().add_ref());
        }
    }
}

impl Widget for ListBox {
    #[cfg(test)]
    fn widget_opt(&self) -> &WidgetOption {
        &self.opt
    }

    fn update(&mut self, ctx: &mut WidgetUpdateCtx<'_>, input: Option<&UiInputEvent>) -> crate::Change {
        if ctx.action(input, self.keyboard_behavior()) != Some(KeyboardAction::Activate) {
            return crate::Change::NoChange;
        }
        // Pointer and keyboard activation publish the same application-facing submission.
        if self.submitted_event.connected() {
            self.submitted_event.emit(ListBoxSubmitted { label: self.com_label().to_owned() });
        }

        // The submitted event does not change this header's intrinsic inputs.
        crate::Change::NoChange
    }

    fn paint(&self, ctx: &mut WidgetPaintCtx<'_>) {
        self.paint_widget(ctx);
    }

    fn keyboard_behavior(&self) -> KeyboardBehavior {
        KeyboardBehavior::TAB_STOP | KeyboardBehavior::ACTIVATE_ENTER | KeyboardBehavior::ACTIVATE_SPACE
    }
}

impl crate::TypedWidget<ListBoxSubmitted> for ListBox {
    fn event(&self) -> crate::WidgetEventPortHandle<ListBoxSubmitted> {
        self.submitted_event.event()
    }
}

#[cfg(test)]
impl From<ListBoxParameters> for ListBox {
    /// Transfers one-shot parameters into the concrete ListBox runtime.
    fn from(parameters: ListBoxParameters) -> Self {
        let runtime = crate::ui_node::widget_object::WidgetRuntime::for_test(parameters.opt);
        Self::from_runtime(runtime, parameters)
    }
}

impl crate::ui_node::widget_object::WidgetAlgorithm for ListBox {
    /// Logical COM closure cancels ordinary submissions while retained widget references remain safe.
    fn close_events(&mut self) {
        self.submitted_event.close_ordinary();
    }
}

impl crate::ui_node::widget_object::HostedWidget for ListBox {
    /// Borrows common lifecycle and resource state without a second list-box representation.
    fn runtime(&self) -> &crate::ui_node::widget_object::WidgetRuntime {
        &self.runtime
    }

    /// Exclusively borrows common lifecycle and resource state for a COM mutation or phase.
    fn runtime_mut(&mut self) -> &mut crate::ui_node::widget_object::WidgetRuntime {
        &mut self.runtime
    }
}

impl ListBox {
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

impl crate::ui_node::widget_object::FontState for ListBox {
    /// Borrows the semantic font reference already used by layout and painting.
    fn font(&self) -> &crate::FontRef {
        &self.font
    }
    /// Replaces the same field after the COM resource owner validates the selection.
    fn set_font(&mut self, font: crate::FontRef) {
        self.font = font;
    }
}

impl crate::ui_node::widget_object::ImageState for ListBox {
    /// List-box image editing uses the same immutable image representation and layout helpers.
    fn set_image(&mut self, image: Option<microui_redux::Image>, fit_width: bool) -> bool {
        if same_image(&self.image, &image) && (image.is_none() || self.fit_width == fit_width) {
            return false;
        }
        self.image = image;
        self.fit_width = fit_width;
        true
    }
}

use crate::ui_node::widget_object::{HostedWidget, ImageState, direct_leaf_layout_impl, direct_widget_phase_impl, set_direct_font_role, set_direct_named_font};
use microui_contracts::{GeometryChange, IActivationSinkAbi, IEventSubscriptionAbi, ITextureAbi, IWidgetHostAbi, ListBoxAbi, TextFontRole};
use neocogi_com::*;

direct_widget_phase_impl!(ListBox);
direct_leaf_layout_impl!(ListBox);

impl ListBox {
    /// Constructs the class for the host-bound factory through the registry's typed contract.
    pub(crate) fn create_com(
        host: ComPtr<IWidgetHostAbi>,
        resources: crate::ui_node::widget_object::SharedAssets,
        label: NcCopyFromBufferOffset<'_, u8>,
    ) -> Result<ComPtr<ListBoxAbi>, NcStatus> {
        let value = Self::build(host, label, Some(&resources))?;
        Ok(Self::from_value(value).query().expect("ListBox declares its callable class contract"))
    }

    /// Builds one list box with either directly imported or runtime-shared resources.
    fn build(
        host: ComPtr<IWidgetHostAbi>,
        label: NcCopyFromBufferOffset<'_, u8>,
        resources: Option<&crate::ui_node::widget_object::SharedAssets>,
    ) -> Result<Self, NcStatus> {
        let parameters = crate::ListBoxParameters::new(crate::boundary::text(label)?, None);
        let runtime = crate::ui_node::widget_object::WidgetRuntime::for_resources(host, parameters.opt, resources)?;
        Ok(Self::from_runtime(runtime, parameters))
    }
}

impl microui_contracts::ListBox for ListBox {
    /// Validates the label before allocating the established label-only entry state.
    fn new(host: ComRef<'_, IWidgetHostAbi>, label: NcCopyFromBufferOffset<'_, u8>) -> Result<Self, NcStatus> {
        Self::build(host.add_ref(), label, None)
    }
    /// Returns an immutable label snapshot from the concrete list box.
    fn text(&self) -> Result<ComPtr<INcStringAbi>, NcStatus> {
        self.runtime().state.readable()?;
        Ok(NcString::from_utf8_str(self.com_label()))
    }
    /// Validates UTF-8 and records geometry only when the concrete label changes.
    fn set_text(&mut self, input: NcCopyFromBufferOffset<'_, u8>) -> Result<(), NcStatus> {
        self.runtime().state.writable()?;
        let label = crate::boundary::text(input)?;
        if self.com_label() != label {
            self.com_set_label(label);
            self.runtime_mut().state.record_change(GeometryChange::CHANGED);
        }
        Ok(())
    }
    /// Validates a semantic role before changing the list-box label's font.
    fn set_font_role(&mut self, role: TextFontRole) -> Result<(), NcStatus> {
        set_direct_font_role(self, role)
    }
    /// Resolves one exact named font before changing the list-box label's font.
    fn set_named_font(&mut self, name: NcCopyFromBufferOffset<'_, u8>) -> Result<(), NcStatus> {
        set_direct_named_font(self, name)
    }
    /// Validates and installs one immutable renderer-owned image before committing the edit.
    fn set_image(&mut self, image: ComRef<'_, ITextureAbi>, fit_width: bool) -> Result<(), NcStatus> {
        self.runtime().state.writable()?;
        let image = microui_redux::Image::from_interface(image.add_ref())?;
        // Revalidate lifetime authority after the foreign size query before committing the
        // prepared image; the generated object gate rejects recursive mutable entry.
        self.runtime().state.writable()?;
        if ImageState::set_image(self, Some(image), fit_width) {
            self.runtime_mut().state.record_change(GeometryChange::CHANGED);
        }
        Ok(())
    }
    /// Removes image content while preserving the label, font, and activation source.
    fn clear_image(&mut self) -> Result<(), NcStatus> {
        self.runtime().state.writable()?;
        if ImageState::set_image(self, None, false) {
            self.runtime_mut().state.record_change(GeometryChange::CHANGED);
        }
        Ok(())
    }
    /// Connects the entry's existing semantic event source directly through the class contract.
    fn subscribe(&mut self, sink: ComRef<'_, IActivationSinkAbi>) -> Result<ComPtr<IEventSubscriptionAbi>, NcStatus> {
        self.runtime().state.writable()?;
        let endpoint = <Self as crate::TypedWidget<crate::ListBoxSubmitted>>::event(self);
        crate::event::subscribe(endpoint, sink)
    }
}
