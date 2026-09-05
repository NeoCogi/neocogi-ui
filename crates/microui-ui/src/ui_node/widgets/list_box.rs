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

    /// Creates list-box parameters with explicit widget options.
    pub fn with_opt(label: impl Into<String>, image: Option<microui_redux::Image>, opt: WidgetOption) -> Self {
        Self {
            label: label.into(),
            image,
            font: FontRef::Role(FontRole::Body),
            opt,
        }
    }

    /// Replaces the font used for the label.
    pub fn font(mut self, font: FontRef) -> Self {
        // Store the reference itself so a replacement skin can supply a new concrete font ID.
        self.font = font;
        self
    }
}

/// Semantic payload emitted when the user submits a list box.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ListBoxSubmitted {
    /// Exact displayed label at user activation time, independent of later setter changes.
    pub label: String,
}

impl crate::WidgetEvent for ListBoxSubmitted {}

/// Concrete retained list box, including its semantic and interaction state.
pub struct ListBox {
    /// Initialization-only label.
    label: String,
    /// Initialization-only image.
    image: Option<microui_redux::Image>,
    /// Width-fitting policy for the current immutable image.
    fit_width: bool,
    /// Initialization-only font.
    font: FontRef,
    /// Base widget options.
    opt: WidgetOption,
    /// Runtime-owned source for user submissions.
    submitted_event: crate::EventSource<ListBoxSubmitted>,
}

impl ListBox {
    /// Constructs a retained node and a weak typed handle to its concrete list box.
    pub fn create(parameters: ListBoxParameters) -> (crate::TypedWidgetHandle<Self>, crate::Node) {
        let widget = ListBox::from(parameters);
        crate::Node::typed_widget(widget)
    }

    /// Returns the native event endpoint emitted once for every user submission.
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
    fn paint_widget(&mut self, ctx: &mut WidgetPaintCtx<'_>) {
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
            ctx.draw_control_text_with_font(font, &self.label, placement.text, ControlRole::Button, self.opt);
        }
        if let (Some(image), Some(visual)) = (self.image.as_ref(), placement.visual) {
            // List-box labels remain theme-colored while external image content preserves the
            // exact source RGBA values supplied to the renderer.
            ctx.painter().com_texture(visual, crate::color(255, 255, 255, 255), image.as_interface().add_ref());
        }
    }
}

impl crate::TypedWidgetHandle<ListBox> {
    /// Returns the list box's native submission endpoint.
    pub fn submitted(&self) -> crate::WidgetEventPortHandle<ListBoxSubmitted> {
        self.widget_event()
    }
}

impl Widget for ListBox {
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

    fn paint(&mut self, ctx: &mut WidgetPaintCtx<'_>) {
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

impl From<ListBoxParameters> for ListBox {
    /// Transfers one-shot parameters into the concrete ListBox runtime.
    fn from(parameters: ListBoxParameters) -> Self {
        ListBox {
            label: parameters.label,
            image: parameters.image,
            fit_width: false,
            font: parameters.font,
            opt: parameters.opt,
            submitted_event: crate::EventSource::new(),
        }
    }
}

impl crate::com::widget::WidgetAlgorithm for ListBox {
    /// Logical COM closure cancels ordinary submissions while retained widget references remain safe.
    fn close_events(&mut self) {
        self.submitted_event.close_ordinary();
    }

    /// Synchronizes options inside the one concrete semantic state of a COM leaf.
    fn set_options(&mut self, options: crate::WidgetOption) {
        self.opt = options;
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

impl crate::com::widget::FontState for ListBox {
    /// Borrows the semantic font reference already used by layout and painting.
    fn font(&self) -> &crate::FontRef {
        &self.font
    }
    /// Replaces the same field after the COM resource owner validates the selection.
    fn set_font(&mut self, font: crate::FontRef) {
        self.font = font;
    }
}

impl crate::com::widget::ImageState for ListBox {
    /// List-box image editing uses the same immutable image representation and layout helpers.
    fn set_image(&mut self, image: Option<microui_redux::Image>, fit_width: bool) -> bool {
        if same_image(&self.image, &image) && (image.is_none() || self.fit_width == fit_width) { return false; }
        self.image = image;
        self.fit_width = fit_width;
        true
    }
}
