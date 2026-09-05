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

//! Button widget state and rendering.
//!
//! Buttons support text, arbitrary atlas icons, and external textures through one shared inline
//! placement path.

use crate::{ControlRole};

use super::*;

#[derive(Clone)]
/// Describes the content rendered inside a button widget.
pub enum ButtonContent {
    /// A text label and optional icon from the atlas.
    Text {
        /// Text displayed on the button.
        label: String,
        /// Optional icon rendered on the button.
        icon: Option<IconRef>,
    },
    /// A text label and optional image.
    Image {
        /// Text displayed on the button.
        label: String,
        /// Optional image rendered on the button.
        image: Option<microui_redux::Image>,
    },
    /// An optional image scaled to the allocated button width while preserving aspect ratio.
    ScaledImage {
        /// Text displayed on the button.
        label: String,
        /// Optional image rendered on the button.
        image: Option<microui_redux::Image>,
    },
}

impl crate::LeafWidget for Button {
    fn measure(&self, style: &Skin, atlas: &AtlasHandle, constraints: Constraints) -> Dimensioni {
        self.preferred_size_widget(style, atlas, constraints)
    }
}

/// One-shot construction input for a [`Button`].
pub struct ButtonParameters {
    /// Content rendered inside the button.
    pub content: ButtonContent,
    /// Font used for text content.
    pub font: FontRef,
    /// Base widget options.
    pub opt: WidgetOption,
    /// Fill behavior for the button background.
    pub fill: WidgetFillOption,
}

impl ButtonParameters {
    /// Creates text-button parameters with default options.
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            content: ButtonContent::Text { label: label.into(), icon: None },
            font: FontRef::Role(FontRole::Body),
            opt: WidgetOption::FRAME,
            fill: WidgetFillOption::ALL,
        }
    }

    /// Creates text-button parameters with explicit widget options.
    #[cfg(test)]
    pub fn with_opt(label: impl Into<String>, opt: WidgetOption) -> Self {
        Self {
            content: ButtonContent::Text { label: label.into(), icon: None },
            font: FontRef::Role(FontRole::Body),
            opt,
            fill: WidgetFillOption::ALL,
        }
    }

    /// Creates image-button parameters with explicit options and fill behavior.
    #[cfg(test)]
    pub fn with_image(label: impl Into<String>, image: Option<microui_redux::Image>, opt: WidgetOption, fill: WidgetFillOption) -> Self {
        Self {
            content: ButtonContent::Image { label: label.into(), image },
            font: FontRef::Role(FontRole::Body),
            opt,
            fill,
        }
    }
}

/// Semantic payload emitted when the user submits a button.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ButtonSubmitted {
    /// Exact displayed label at user activation time, independent of later setter changes.
    pub label: String,
}

/// Concrete retained button, including its semantic and interaction state.
pub struct Button {
    /// Initialization-only content.
    content: ButtonContent,
    /// Initialization-only font.
    font: FontRef,
    /// Base widget options.
    /// Native algorithm fixture policy; production options live only in WidgetState.
    #[cfg(test)]
    opt: WidgetOption,
    /// Initialization-only fill behavior.
    fill: WidgetFillOption,
    /// Runtime-owned source for user submissions.
    submitted_event: crate::EventSource<ButtonSubmitted>,
}

impl Button {
    /// Constructs a retained node and a weak typed handle to its concrete button.
    #[cfg(test)]
    pub fn create(parameters: ButtonParameters) -> (crate::TypedWidgetHandle<Self>, crate::Node) {
        let widget = Button::from(parameters);
        crate::Node::typed_widget(widget)
    }

    /// Returns the native event endpoint emitted once for every user submission.
    #[cfg(test)]
    pub fn submitted(&self) -> crate::WidgetEventPortHandle<ButtonSubmitted> {
        <Self as crate::TypedWidget<ButtonSubmitted>>::event(self)
    }

    /// Measures the label and optional visual content.
    fn preferred_size_widget(&self, style: &Skin, atlas: &AtlasHandle, constraints: Constraints) -> Dimensioni {
        match &self.content {
            ButtonContent::Text { label, icon } => {
                let visual = icon.as_ref().map(|icon| atlas.get_icon_size(icon.resolve(style, atlas)));
                inline_content_size(style, atlas, &self.font, label, visual)
            }
            ButtonContent::Image { label, image } => {
                let visual = image.as_ref().map(image_size);
                inline_content_size(style, atlas, &self.font, label, visual)
            }
            ButtonContent::ScaledImage { label, image } => {
                let visual = image.as_ref().map(image_size);
                if visual.is_some() && constraints.width.bound().is_some() {
                    scaled_visual_content_size(constraints, visual)
                } else {
                    inline_content_size(style, atlas, &self.font, label, visual)
                }
            }
        }
    }

    /// Paints the button frame, text, and optional visual payload.
    fn paint_widget(&mut self, ctx: &mut WidgetPaintCtx<'_>) {
        let rect = ctx.local_rect();
        if !ctx.options().intersects(WidgetOption::FRAME) && widget_fill_visible(ctx, self.fill) {
            // Framed buttons are painted once by retained traversal across their complete outer
            // allocation. An unframed button asks for only the role's stretchable center payload.
            ctx.draw_control_center(ControlRole::Button, rect);
        }
        let font = ctx.skin().resolve_font(ctx.atlas(), &self.font);
        match &self.content {
            ButtonContent::Text { label, icon } => {
                // Text/icon buttons use atlas icon metrics when placing the inline visual.
                let icon_id = icon.as_ref().map(|icon| icon.resolve(ctx.skin(), ctx.atlas()));
                let visual_size = icon_id.map(|icon| ctx.atlas().get_icon_size(icon));
                let placement = place_inline_content(rect, ctx.skin(), label, visual_size);
                if !label.is_empty() {
                    ctx.draw_control_text_with_font(font, label, placement.text, ControlRole::Button, ctx.options());
                }
                if let (Some(icon), Some(visual)) = (icon_id, placement.visual) {
                    let color = ctx.control_content_color(ControlRole::Button);
                    ctx.draw_icon(icon, visual, color);
                }
            }
            ButtonContent::Image { label, image } => {
                let visual_size = image.as_ref().map(image_size);
                let placement = place_inline_content(rect, ctx.skin(), label, visual_size);
                if !label.is_empty() {
                    ctx.draw_control_text_with_font(font, label, placement.text, ControlRole::Button, ctx.options());
                }
                if let (Some(image), Some(visual)) = (image.as_ref(), placement.visual) {
                    // External image pixels carry their own color; the adjacent label alone uses
                    // the stateful Button content color selected by the active theme.
                    ctx.painter()
                        .com_texture(visual, crate::color(255, 255, 255, 255), image.as_interface().add_ref());
                }
            }
            ButtonContent::ScaledImage { label, image } => {
                let visual_size = image.as_ref().map(image_size);
                let placement = if visual_size.is_some() {
                    place_scaled_visual_content(rect, visual_size)
                } else {
                    place_inline_content(rect, ctx.skin(), label, visual_size)
                };
                if !label.is_empty() {
                    ctx.draw_control_text_with_font(font, label, placement.text, ControlRole::Button, ctx.options());
                }
                if let (Some(image), Some(visual)) = (image.as_ref(), placement.visual) {
                    // Scaling changes only destination geometry; it must not introduce a theme
                    // content-color tint that destroys the source image's RGB channels.
                    ctx.painter()
                        .com_texture(visual, crate::color(255, 255, 255, 255), image.as_interface().add_ref());
                }
            }
        }
    }
}

#[cfg(test)]
impl crate::TypedWidgetHandle<Button> {
    /// Returns the button's native submission endpoint.
    pub fn submitted(&self) -> crate::WidgetEventPortHandle<ButtonSubmitted> {
        self.widget_event()
    }
}

impl Widget for Button {
    #[cfg(test)]
    fn widget_opt(&self) -> &WidgetOption {
        &self.opt
    }

    fn frame_appearance_role(&self) -> FrameRole {
        // Buttons use one stateful role for their complete runtime-owned outer frame.
        FrameRole::Control(ControlRole::Button)
    }

    fn update(&mut self, ctx: &mut WidgetUpdateCtx<'_>, input: Option<&UiInputEvent>) -> crate::Change {
        if ctx.action(input, self.keyboard_behavior()) != Some(KeyboardAction::Activate) {
            return crate::Change::NoChange;
        }
        // Pointer click, Enter, and Space converge on the button's single typed submission port.
        if self.submitted_event.connected() {
            self.submitted_event.emit(ButtonSubmitted { label: self.com_label().to_owned() });
        }

        // Submitting leaves the label and preferred dimensions unchanged.
        crate::Change::NoChange
    }

    fn paint(&mut self, ctx: &mut WidgetPaintCtx<'_>) {
        self.paint_widget(ctx);
    }

    fn keyboard_behavior(&self) -> KeyboardBehavior {
        // Buttons are ordinary sequential focus targets. Shared activation capabilities are added
        // independently so focus traversal does not depend on button-specific routing knowledge.
        KeyboardBehavior::TAB_STOP | KeyboardBehavior::ACTIVATE_ENTER | KeyboardBehavior::ACTIVATE_SPACE
    }
}

impl crate::TypedWidget<ButtonSubmitted> for Button {
    fn event(&self) -> crate::WidgetEventPortHandle<ButtonSubmitted> {
        self.submitted_event.event()
    }
}

impl From<ButtonParameters> for Button {
    /// Transfers one-shot parameters into the concrete Button runtime.
    fn from(parameters: ButtonParameters) -> Self {
        Button {
            content: parameters.content,
            font: parameters.font,
            #[cfg(test)]
            opt: parameters.opt,
            fill: parameters.fill,
            submitted_event: crate::EventSource::new(),
        }
    }
}

impl crate::com::widget::WidgetAlgorithm for Button {
    /// Logical COM closure cancels ordinary submissions while retained widget references remain safe.
    fn close_events(&mut self) {
        self.submitted_event.close_ordinary();
    }
}

impl Button {
    /// Borrows the label independently of whether its adjacent visual is an icon or image.
    pub(crate) fn com_label(&self) -> &str {
        match &self.content {
            ButtonContent::Text { label, .. } | ButtonContent::Image { label, .. } | ButtonContent::ScaledImage { label, .. } => label,
        }
    }
    /// Replaces only text, preserving the concrete visual and scaling selection.
    pub(crate) fn com_set_label(&mut self, text: &str) {
        let (ButtonContent::Text { label, .. } | ButtonContent::Image { label, .. } | ButtonContent::ScaledImage { label, .. }) = &mut self.content;
        label.clear();
        label.push_str(text);
    }
}

impl crate::com::widget::FontState for Button {
    /// Borrows the semantic font reference already used by layout and painting.
    fn font(&self) -> &crate::FontRef {
        &self.font
    }
    /// Replaces the same field after the COM resource owner validates the selection.
    fn set_font(&mut self, font: crate::FontRef) {
        self.font = font;
    }
}

impl crate::com::widget::IconState for Button {
    /// Changes visual kind while preserving the sole owned label allocation.
    fn set_icon(&mut self, icon: Option<crate::IconRef>) -> bool {
        if let ButtonContent::Text { icon: previous, .. } = &self.content
            && *previous == icon
        {
            return false;
        }
        let (ButtonContent::Text { label, .. } | ButtonContent::Image { label, .. } | ButtonContent::ScaledImage { label, .. }) = &mut self.content;
        self.content = ButtonContent::Text { label: std::mem::take(label), icon };
        true
    }
}
impl Button {
    /// Reads the existing unframed center-fill policy.
    pub(crate) fn com_fill(&self) -> WidgetFillOption {
        self.fill
    }
    /// Changes the paint-only fill policy after COM validation.
    pub(crate) fn com_set_fill(&mut self, fill: WidgetFillOption) {
        self.fill = fill;
    }
}

impl crate::com::widget::ImageState for Button {
    /// Replaces the visual kind while keeping the single label allocation and selected font.
    fn set_image(&mut self, image: Option<microui_redux::Image>, fit_width: bool) -> bool {
        match &self.content {
            // Clearing image content must not erase an independently selected atlas icon.
            ButtonContent::Text { .. } if image.is_none() => return false,
            ButtonContent::Image { image: previous, .. } if !fit_width && same_image(previous, &image) => return false,
            ButtonContent::ScaledImage { image: previous, .. } if fit_width && same_image(previous, &image) => return false,
            _ => {}
        }
        let (ButtonContent::Text { label, .. } | ButtonContent::Image { label, .. } | ButtonContent::ScaledImage { label, .. }) = &mut self.content;
        let label = std::mem::take(label);
        self.content = match (image, fit_width) {
            (None, _) => ButtonContent::Text { label, icon: None },
            (image, false) => ButtonContent::Image { label, image },
            (image, true) => ButtonContent::ScaledImage { label, image },
        };
        true
    }
}
