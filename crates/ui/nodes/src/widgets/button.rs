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

use crate::ControlRole;
use crate::widget_object::{
    HostedWidget, ImageState, direct_leaf_layout_impl, direct_widget_phase_impl, set_direct_font_role, set_direct_icon_role, set_direct_named_font,
    set_direct_named_icon,
};
use neocogi_ui_core_contracts::*;
use neocogi_ui_nodes_contracts::{ButtonAbi, ContentIcon, FillStates};
use neocogi_com::{ComPtr, ComRef, INcStringAbi, NcCopyFromBufferOffset, NcStatus, NcString};

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
        image: Option<neocogi_ui::Image>,
    },
    /// An optional image scaled to the allocated button width while preserving aspect ratio.
    ScaledImage {
        /// Text displayed on the button.
        label: String,
        /// Optional image rendered on the button.
        image: Option<neocogi_ui::Image>,
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
    /// Fill behavior for the button background.
    pub fill: WidgetFillOption,
}

impl ButtonParameters {
    /// Creates text-button parameters with default options.
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            content: ButtonContent::Text { label: label.into(), icon: None },
            font: FontRef::Role(FontRole::Body),
            fill: WidgetFillOption::ALL,
        }
    }
}

#[neocogi_com::com_class(neocogi_ui_nodes_contracts::Button, ownership = exclusive)]
/// Concrete retained button and dynamically constructible COM object.
///
/// The allocation owns its semantic fields directly. `runtime` contains only common host,
/// lifecycle, option, resource, and rendering machinery; it cannot contain another Button.
pub struct Button {
    /// Non-semantic COM lifecycle and host-backed phase resources.
    runtime: crate::widget_object::WidgetRuntime,
    /// Initialization-only content.
    content: ButtonContent,
    /// Initialization-only font.
    font: FontRef,
    /// Base widget options.
    /// Initialization-only fill behavior.
    fill: WidgetFillOption,
    /// Single event slot; receives its destination during construction and is released when the widget is destroyed.
    events: crate::widget_object::WidgetEvents<neocogi_ui_nodes_contracts::ButtonEventQueueAbi>,
}

impl Button {
    /// Combines validated semantic parameters with one already bound common runtime.
    pub(crate) fn from_runtime(runtime: crate::widget_object::WidgetRuntime, parameters: ButtonParameters) -> Self {
        Self {
            runtime,
            content: parameters.content,
            font: parameters.font,
            fill: parameters.fill,
            events: Default::default(),
        }
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
    fn paint_widget(&self, ctx: &mut WidgetPaintCtx<'_>) {
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
                    ctx.draw_texture(visual, crate::color(255, 255, 255, 255), image.as_interface().add_ref());
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
                    ctx.draw_texture(visual, crate::color(255, 255, 255, 255), image.as_interface().add_ref());
                }
            }
        }
    }
}

impl Widget for Button {
    fn frame_appearance_role(&self) -> FrameRole {
        // Buttons use one stateful role for their complete runtime-owned outer frame.
        FrameRole::Control(ControlRole::Button)
    }

    fn update(&mut self, ctx: &mut WidgetUpdateCtx<'_>, input: Option<&UiInputEvent>) -> crate::Change {
        if ctx.action(input, self.keyboard_behavior()) != Some(KeyboardAction::Activate) {
            return crate::Change::NoChange;
        }
        // Pointer click, Enter, and Space converge on the button's single typed submission port.
        if self.events.is_bound() {
            ctx.posted(self.events.post(&neocogi_ui_nodes_contracts::ButtonEvent::Activated(crate::Activation {
                label: self.com_label().to_owned(),
            })));
        }

        // Submitting leaves the label and preferred dimensions unchanged.
        crate::Change::NoChange
    }

    fn paint(&self, ctx: &mut WidgetPaintCtx<'_>) {
        self.paint_widget(ctx);
    }

    fn keyboard_behavior(&self) -> KeyboardBehavior {
        // Buttons are ordinary sequential focus targets. Shared activation capabilities are added
        // independently so focus traversal does not depend on button-specific routing knowledge.
        KeyboardBehavior::TAB_STOP | KeyboardBehavior::ACTIVATE_ENTER | KeyboardBehavior::ACTIVATE_SPACE
    }
}

impl crate::widget_object::WidgetAlgorithm for Button {

}

impl crate::widget_object::HostedWidget for Button {
    /// Borrows the common runtime without exposing semantic fields through it.
    fn runtime(&self) -> &crate::widget_object::WidgetRuntime {
        &self.runtime
    }

    /// Exclusively borrows the common runtime while leaving semantic ownership on `Button`.
    fn runtime_mut(&mut self) -> &mut crate::widget_object::WidgetRuntime {
        &mut self.runtime
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

impl crate::widget_object::FontState for Button {
    /// Borrows the semantic font reference already used by layout and painting.
    fn font(&self) -> &crate::FontRef {
        &self.font
    }
    /// Replaces the same field after the COM resource owner validates the selection.
    fn set_font(&mut self, font: crate::FontRef) {
        self.font = font;
    }
}

impl crate::widget_object::IconState for Button {
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

impl crate::widget_object::ImageState for Button {
    /// Replaces the visual kind while keeping the single label allocation and selected font.
    fn set_image(&mut self, image: Option<neocogi_ui::Image>, fit_width: bool) -> bool {
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

impl Button {
    /// Constructs the class directly for the factory shorthand without bypassing contract validation.
    pub(crate) fn create_com(
        host: ComPtr<IWidgetHostAbi>,
        resources: crate::widget_object::SharedAssets,
        label: NcCopyFromBufferOffset<'_, u8>,
        options: WidgetOptions,
        queue: Option<ComPtr<neocogi_ui_nodes_contracts::ButtonEventQueueAbi>>,
    ) -> Result<neocogi_com::ComBox<ButtonAbi>, NcStatus> {
        let mut value = Self::build(host, label, options, Some(&resources))?;
        // Fix the sole destination before publishing the completed widget.
        Self::into_box_cyclic::<ButtonAbi>(move |origin| {
            value.events = crate::widget_object::WidgetEvents::with_queue(queue, origin);
            value
        })?
        .query()
    }

    /// Builds one button with either directly imported or runtime-shared resources.
    fn build(
        host: ComPtr<IWidgetHostAbi>,
        label: NcCopyFromBufferOffset<'_, u8>,
        options: WidgetOptions,
        resources: Option<&crate::widget_object::SharedAssets>,
    ) -> Result<Self, NcStatus> {
        options.validate()?;
        let label = crate::boundary::text(label)?;
        let native_options = crate::WidgetOption::from_bits_retain(u32::from(options.raw()));
        let runtime = crate::widget_object::WidgetRuntime::for_resources(host, native_options, resources)?;
        Ok(Self::from_runtime(runtime, crate::ButtonParameters::new(label)))
    }
}

direct_widget_phase_impl!(Button);
direct_leaf_layout_impl!(Button);

impl neocogi_ui_nodes_contracts::Button for Button {
    /// Validates before committing and only dirties geometry when text actually differs.
    fn set_label(&mut self, input: NcCopyFromBufferOffset<'_, u8>) -> Result<(), NcStatus> {
        self.runtime().state.writable()?;
        let input = crate::boundary::text(input)?;
        if self.com_label() != input {
            self.com_set_label(input);
            self.runtime_mut().state.record_change(GeometryChange::Changed);
        }
        Ok(())
    }

    /// Returns an owned immutable snapshot, never a borrow into widget storage.
    fn label(&self) -> Result<ComPtr<INcStringAbi>, NcStatus> {
        self.runtime().state.readable()?;
        Ok(NcString::from_utf8_str(self.com_label()))
    }

    /// Validates a semantic role before changing the Button's font selection.
    fn set_font_role(&mut self, role: FontRole) -> Result<(), NcStatus> {
        set_direct_font_role(self, role)
    }

    /// Resolves and retains a stable named font identity at the setter boundary.
    fn set_named_font(&mut self, name: NcCopyFromBufferOffset<'_, u8>) -> Result<(), NcStatus> {
        set_direct_named_font(self, name)
    }

    /// Validates a semantic icon before replacing the Button's adjacent visual.
    fn set_icon_role(&mut self, icon: ContentIcon) -> Result<(), NcStatus> {
        set_direct_icon_role(self, icon)
    }

    /// Resolves one exact named icon before replacing the Button's adjacent visual.
    fn set_named_icon(&mut self, name: NcCopyFromBufferOffset<'_, u8>) -> Result<(), NcStatus> {
        set_direct_named_icon(self, name)
    }

    /// Reports the current fill mask from the sole concrete Button state.
    fn fill_states(&self) -> Result<FillStates, NcStatus> {
        self.runtime().state.readable()?;
        Ok(FillStates::from_raw(self.com_fill().bits() as u8))
    }

    /// Rejects unknown flags before committing a paint-only change.
    fn set_fill_states(&mut self, states: FillStates) -> Result<(), NcStatus> {
        self.runtime().state.writable()?;
        let fill = crate::WidgetFillOption::from_bits(u32::from(states.raw())).ok_or(neocogi_com::NC_E_INVALID_ARG)?;
        self.com_set_fill(fill);
        Ok(())
    }

    /// Validates and retains the renderer-owned image before changing semantic content.
    fn set_image(&mut self, image: ComRef<'_, ITextureAbi>, fit_width: bool) -> Result<(), NcStatus> {
        self.runtime().state.writable()?;
        let image = neocogi_ui::Image::from_interface(image.add_ref())?;
        // A hostile dimension provider may reenter through another interface while this mutable
        // call is active; revalidate host authority before committing the prepared image.
        self.runtime().state.writable()?;
        if ImageState::set_image(self, Some(image), fit_width) {
            self.runtime_mut().state.record_change(GeometryChange::Changed);
        }
        Ok(())
    }

    /// Clears owned image content while retaining the same label and event source.
    fn clear_image(&mut self) -> Result<(), NcStatus> {
        self.runtime().state.writable()?;
        if ImageState::set_image(self, None, false) {
            self.runtime_mut().state.record_change(GeometryChange::Changed);
        }
        Ok(())
    }
}

/// Ordinary leaves reserve no duplicate ancestors or per-instance descriptor tables.
#[cfg(all(test, target_pointer_width = "64"))]
#[test]
fn com_object_control_is_independent_of_interface_count() {
    // The common control header occupies three words and the call gate one.
    // Box owns exactly this storage; there is no hidden Arc header to add.
    // No per-interface fields remain; handles carry their selected tables.
    // The constructor receiver is not an interface implemented by the widget.
    assert_eq!(Button::COM_INTERFACE_COUNT, 4);
    println!(
        "Button: state={}, allocation={}",
        core::mem::size_of::<Button>(),
        core::mem::size_of::<ButtonComObject>()
    );
    assert_eq!(core::mem::offset_of!(ButtonComObject, value), 32);
    assert_eq!(core::mem::size_of::<ButtonComObject>(), core::mem::size_of::<Button>() + 32);
}
