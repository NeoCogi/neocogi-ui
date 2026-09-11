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

//! Retained color-swatch widget.

use crate::{ControlRole};

use crate::*;

/// One-shot construction input for a [`ColorSwatch`].
pub struct ColorSwatchParameters {
    /// Initial fill color rendered inside the swatch.
    pub fill: Color,
    /// Initial optional label rendered on top of the swatch.
    pub label: String,
    /// Font used for the label.
    pub font: FontRef,
    /// Base widget options.
    pub opt: WidgetOption,
}

impl crate::LeafWidget for ColorSwatch {
    fn measure(&self, style: &Skin, atlas: &AtlasHandle, constraints: Constraints) -> Dimensioni {
        self.preferred_size_widget(style, atlas, constraints)
    }
}

impl ColorSwatchParameters {
    /// Creates swatch parameters with the provided fill color.
    pub fn new(fill: Color) -> Self {
        Self {
            fill,
            label: String::new(),
            font: FontRef::Role(FontRole::BODY),
            opt: WidgetOption::NO_INTERACT | WidgetOption::ALIGN_CENTER | WidgetOption::FRAME,
        }
    }

    /// Seeds the label rendered over the swatch.
    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = label.into();
        self
    }
}

#[neocogi_com::com_object(neocogi_ui_contracts::ColorSwatch)]
/// Concrete retained color swatch and dynamically constructible COM object.
pub struct ColorSwatch {
    /// Non-semantic COM lifecycle and host-backed phase resources.
    runtime: crate::ui_node::widget_object::WidgetRuntime,
    /// Initialization-only font.
    font: FontRef,
    /// Base widget options.
    /// Native algorithm fixture policy; production options live only in WidgetState.
    #[cfg(test)]
    opt: WidgetOption,
    /// Mutable fill color.
    fill: Color,
    /// Mutable label.
    label: String,
}

impl ColorSwatch {
    /// Constructs a retained node and a weak typed handle to its concrete color swatch.
    #[cfg(test)]
    pub fn create(parameters: ColorSwatchParameters) -> (crate::TypedWidgetHandle<Self>, crate::Node) {
        let widget = ColorSwatch::from(parameters);
        crate::Node::typed_widget(widget)
    }

    /// Combines validated semantic parameters with one already bound common runtime.
    pub(crate) fn from_runtime(runtime: crate::ui_node::widget_object::WidgetRuntime, parameters: ColorSwatchParameters) -> Self {
        Self {
            runtime,
            font: parameters.font,
            #[cfg(test)]
            opt: parameters.opt,
            fill: parameters.fill,
            label: parameters.label,
        }
    }

    /// Returns the current fill color.
    pub const fn fill(&self) -> Color {
        self.fill
    }

    /// Replaces the fill color.
    pub fn set_fill(&mut self, fill: Color) {
        self.fill = fill;
    }

    /// Returns the current label.
    pub fn label(&self) -> &str {
        &self.label
    }

    /// Replaces the label.
    pub fn set_label(&mut self, label: impl Into<String>) {
        self.label = label.into();
    }

    /// Measures a square-ish color swatch with a text-friendly default height.
    fn preferred_size_widget(&self, style: &Skin, atlas: &AtlasHandle, _constraints: Constraints) -> Dimensioni {
        let padding = style.metrics.padding.max(0);
        let font = style.resolve_font(atlas, &self.font);
        let label_width = if self.label.is_empty() {
            0
        } else {
            atlas.get_text_size(font, self.label.as_str()).width.max(0)
        };
        // Saturating extents keep an otherwise valid extreme font from wrapping intrinsic widget
        // geometry before layout has a chance to constrain it.
        let padding_extent = padding.saturating_mul(2);
        let height = (atlas.get_font_height(font) as i32).saturating_add(padding_extent).max(24);
        Dimensioni::new(label_width.saturating_add(padding_extent).max(24), height)
    }

    /// Paints the swatch fill, border, and optional label.
    fn paint_widget(&self, ctx: &mut WidgetPaintCtx<'_>) {
        let rect = ctx.local_rect();
        ctx.draw_rect(rect, self.fill);
        if !self.label.is_empty() {
            let font = ctx.skin().resolve_font(ctx.atlas(), &self.font);
            ctx.draw_control_text_with_font(font, self.label.as_str(), rect, ControlRole::BUTTON, ctx.options());
        }
    }
}

impl Widget for ColorSwatch {
    #[cfg(test)]
    fn widget_opt(&self) -> &WidgetOption {
        &self.opt
    }

    fn paint(&self, ctx: &mut WidgetPaintCtx<'_>) {
        self.paint_widget(ctx);
    }
}

#[cfg(test)]
impl From<ColorSwatchParameters> for ColorSwatch {
    /// Transfers one-shot parameters into the concrete ColorSwatch runtime.
    fn from(parameters: ColorSwatchParameters) -> Self {
        let runtime = crate::ui_node::widget_object::WidgetRuntime::for_test(parameters.opt);
        Self::from_runtime(runtime, parameters)
    }
}

impl crate::ui_node::widget_object::WidgetAlgorithm for ColorSwatch {}

impl crate::ui_node::widget_object::HostedWidget for ColorSwatch {
    /// Borrows common lifecycle and resource state without a second swatch representation.
    fn runtime(&self) -> &crate::ui_node::widget_object::WidgetRuntime {
        &self.runtime
    }

    /// Exclusively borrows common lifecycle and resource state for a COM mutation or phase.
    fn runtime_mut(&mut self) -> &mut crate::ui_node::widget_object::WidgetRuntime {
        &mut self.runtime
    }
}

impl crate::ui_node::widget_object::FontState for ColorSwatch {
    /// Borrows the semantic font reference already used by layout and painting.
    fn font(&self) -> &crate::FontRef {
        &self.font
    }
    /// Replaces the same field after the COM resource owner validates the selection.
    fn set_font(&mut self, font: crate::FontRef) {
        self.font = font;
    }
}

use crate::ui_node::widget_object::{HostedWidget, direct_leaf_layout_impl, direct_widget_phase_impl, set_direct_font_role, set_direct_named_font};
use neocogi_ui_contracts::{Color, ColorSwatchAbi, GeometryChange, IWidgetHostAbi, FontRole};
use neocogi_com::*;

direct_widget_phase_impl!(ColorSwatch);
direct_leaf_layout_impl!(ColorSwatch);

impl ColorSwatch {
    /// Constructs the class for the host-bound factory through the registry's typed contract.
    pub(crate) fn create_com(
        host: ComPtr<IWidgetHostAbi>,
        resources: crate::ui_node::widget_object::SharedAssets,
        rgba: u32,
        label: NcCopyFromBufferOffset<'_, u8>,
    ) -> Result<ComPtr<ColorSwatchAbi>, NcStatus> {
        let value = Self::build(host, rgba, label, Some(&resources))?;
        Ok(Self::from_value(value).query().expect("ColorSwatch declares its callable class contract"))
    }

    /// Builds one color swatch with either directly imported or runtime-shared resources.
    fn build(
        host: ComPtr<IWidgetHostAbi>,
        rgba: u32,
        label: NcCopyFromBufferOffset<'_, u8>,
        resources: Option<&crate::ui_node::widget_object::SharedAssets>,
    ) -> Result<Self, NcStatus> {
        let parameters = crate::ColorSwatchParameters::new(crate::render::external::color(rgba)).label(crate::boundary::text(label)?);
        let runtime = crate::ui_node::widget_object::WidgetRuntime::for_resources(host, parameters.opt, resources)?;
        Ok(Self::from_runtime(runtime, parameters))
    }
}

impl neocogi_ui_contracts::ColorSwatch for ColorSwatch {
    /// Returns an immutable snapshot without exposing the semantic String allocation.
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
            self.runtime_mut().state.record_change(GeometryChange::CHANGED);
        }
        Ok(())
    }
    /// Packs canonical RGBA without relying on machine endianness.
    fn fill(&self) -> Result<u32, NcStatus> {
        self.runtime().state.readable()?;
        let c = self.fill();
        Ok(u32::from_le_bytes([c.r, c.g, c.b, c.a]))
    }
    /// Color is a paint-only input and emits no user event.
    fn set_fill(&mut self, rgba: u32) -> Result<(), NcStatus> {
        self.runtime().state.writable()?;
        self.set_fill(crate::render::external::color(rgba));
        Ok(())
    }
    /// Validates a semantic role before changing the swatch label's font.
    fn set_font_role(&mut self, role: FontRole) -> Result<(), NcStatus> {
        set_direct_font_role(self, role)
    }
    /// Resolves one exact named font before changing the swatch label's font.
    fn set_named_font(&mut self, name: NcCopyFromBufferOffset<'_, u8>) -> Result<(), NcStatus> {
        set_direct_named_font(self, name)
    }
}
