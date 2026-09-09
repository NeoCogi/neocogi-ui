//
// Copyright 2022-Present (c) Raja Lehtihet & Wael El Oraiby
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
// -----------------------------------------------------------------------------
// Ported to rust from https://github.com/rxi/microui/ and the original license
//
// Copyright (c) 2020 rxi
//
// Permission is hereby granted, free of charge, to any person obtaining a copy
// of this software and associated documentation files (the "Software"), to
// deal in the Software without restriction, including without limitation the
// rights to use, copy, modify, merge, publish, distribute, sublicense, and/or
// sell copies of the Software, and to permit persons to whom the Software is
// furnished to do so, subject to the following conditions:
//
// The above copyright notice and this permission notice shall be included in
// all copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
// IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING
// FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS
// IN THE SOFTWARE.
//
//! Retained text-block widget.
//!
//! Text blocks render static or application-mutated text while participating in the same
//! measurement and paint pipeline as interactive controls. CRLF and lone CR are normalized to LF
//! when text enters concrete widget storage. Visible glyph coverage and missing-character fallback
//! come from the selected atlas font.

use crate::{SurfaceRole, SurfaceState};

use crate::ui_node::text_layout::{baseline_aligned_top, build_display_text_lines, text_block_size};
use crate::*;

use super::text_edit::normalize_multiline_text;

/// One-shot construction input for a [`TextBlock`].
pub struct TextBlockParameters {
    /// Initial text rendered by the widget; line endings are normalized when mounted.
    pub text: String,
    /// Wrapping mode used for layout and rendering.
    pub wrap: TextWrap,
    /// Font used for text measurement and paint.
    pub font: FontRef,
    /// Base widget options.
    pub opt: WidgetOption,
}

impl crate::LeafWidget for TextBlock {
    fn measure(&self, style: &Skin, atlas: &AtlasHandle, constraints: Constraints) -> Dimensioni {
        self.preferred_size_widget(style, atlas, constraints)
    }
}

impl TextBlockParameters {
    /// Creates text-block parameters without wrapping.
    #[cfg(test)]
    pub fn new(text: impl Into<String>) -> Self {
        Self::with_wrap(text, TextWrap::None)
    }

    /// Creates text-block parameters with an explicit wrapping mode.
    pub fn with_wrap(text: impl Into<String>, wrap: TextWrap) -> Self {
        Self {
            text: text.into(),
            wrap,
            font: FontRef::Role(FontRole::Body),
            opt: WidgetOption::NO_INTERACT,
        }
    }

    /// Replaces the font used for text measurement and paint.
    #[cfg(test)]
    pub fn font(mut self, font: FontRef) -> Self {
        // Text layout resolves this stable value against whichever skin bundle is active later.
        self.font = font;
        self
    }
}

#[neocogi_com::com_object(microui_contracts::TextBlock)]
/// Concrete retained text block and dynamically constructible COM object.
pub struct TextBlock {
    /// Non-semantic COM lifecycle and host-backed phase resources.
    runtime: crate::ui_node::widget_object::WidgetRuntime,
    /// Initialization-only wrapping mode.
    wrap: TextWrap,
    /// Initialization-only font.
    font: FontRef,
    /// Base widget options.
    /// Native algorithm fixture policy; production options live only in WidgetState.
    #[cfg(test)]
    opt: WidgetOption,
    /// Mutable display text.
    text: String,
}

impl TextBlock {
    /// Constructs a retained node and a weak typed handle to its concrete text block.
    #[cfg(test)]
    pub fn create(parameters: TextBlockParameters) -> (TypedWidgetHandle<Self>, Node) {
        Node::typed_widget(Self::from(parameters))
    }

    /// Combines validated semantic parameters with one already bound common runtime.
    pub(crate) fn from_runtime(runtime: crate::ui_node::widget_object::WidgetRuntime, parameters: TextBlockParameters) -> Self {
        Self {
            runtime,
            wrap: parameters.wrap,
            font: parameters.font,
            #[cfg(test)]
            opt: parameters.opt,
            // Public construction data may arrive through a struct literal, so concrete storage
            // itself remains the single normalization boundary.
            text: normalize_multiline_text(parameters.text),
        }
    }

    /// Returns the current display text.
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Replaces display text after normalizing line endings to LF.
    pub fn set_text(&mut self, text: impl Into<String>) {
        // Static multiline text shares the same canonical LF representation as editable content,
        // keeping measured line ranges identical to the slices submitted during paint.
        self.text = normalize_multiline_text(text);
    }

    /// Measures wrapped display text using the available width when requested.
    fn preferred_size_widget(&self, style: &Skin, atlas: &AtlasHandle, constraints: Constraints) -> Dimensioni {
        if self.text.is_empty() {
            return Dimensioni::new(0, 0);
        }

        let font = style.resolve_font(atlas, &self.font);
        let line_height = atlas.get_font_height(font) as i32;
        let max_width = if self.wrap == TextWrap::Word {
            constraints.width.bound().unwrap_or(i32::MAX / 4).max(1)
        } else {
            i32::MAX / 4
        };
        let lines = build_display_text_lines(self.text.as_str(), self.wrap, max_width, font, atlas);
        text_block_size(&lines, line_height)
    }

    /// Paints each measured display line with baseline alignment.
    fn paint_widget(&self, ctx: &mut WidgetPaintCtx<'_>) {
        if self.text.is_empty() {
            return;
        }

        let bounds = ctx.local_rect();
        let font = ctx.skin().resolve_font(ctx.atlas(), &self.font);
        let color = ctx
            .skin()
            .surface(SurfaceRole::GenericFrame, SurfaceState::from_enabled(ctx.enabled()))
            .content_color;
        let line_height = ctx.atlas().get_font_height(font) as i32;
        let baseline = ctx.atlas().get_font_baseline(font);
        let max_width = if self.wrap == TextWrap::Word { bounds.width.max(1) } else { i32::MAX / 4 };
        let lines = build_display_text_lines(self.text.as_str(), self.wrap, max_width, font, ctx.atlas());

        ctx.with_clip(bounds, |ctx| {
            for (idx, line) in lines.iter().enumerate() {
                // A large validated line height or long block must clamp at the geometry boundary
                // instead of overflowing while deriving a later row's origin.
                let line_index = i32::try_from(idx).unwrap_or(i32::MAX);
                let line_y = bounds.y.saturating_add(line_index.saturating_mul(line_height));
                let line_rect = rect(bounds.x, line_y, bounds.width, line_height);
                let line_top = baseline_aligned_top(line_rect, line_height, baseline);
                let slice = &self.text[line.start..line.end];
                if !slice.is_empty() {
                    ctx.draw_text(font, slice, vec2(line_rect.x, line_top), color);
                }
            }
        });
    }
}

#[cfg(test)]
impl TypedWidgetHandle<TextBlock> {
    /// Replaces retained display text after normalizing line endings to LF.
    pub fn set_text(&self, text: impl Into<String>) -> Option<()> {
        self.try_update_with(text.into(), |widget, text| widget.set_text(text)).ok()
    }
}

impl Widget for TextBlock {
    #[cfg(test)]
    fn widget_opt(&self) -> &WidgetOption {
        &self.opt
    }

    fn paint(&self, ctx: &mut WidgetPaintCtx<'_>) {
        self.paint_widget(ctx);
    }
}

#[cfg(test)]
impl From<TextBlockParameters> for TextBlock {
    /// Transfers one-shot parameters into the concrete TextBlock runtime.
    fn from(parameters: TextBlockParameters) -> Self {
        let runtime = crate::ui_node::widget_object::WidgetRuntime::for_test(parameters.opt);
        Self::from_runtime(runtime, parameters)
    }
}

impl crate::ui_node::widget_object::WidgetAlgorithm for TextBlock {}

impl crate::ui_node::widget_object::HostedWidget for TextBlock {
    /// Borrows common lifecycle and resource state without a second text-block representation.
    fn runtime(&self) -> &crate::ui_node::widget_object::WidgetRuntime {
        &self.runtime
    }

    /// Exclusively borrows common lifecycle and resource state for a COM mutation or phase.
    fn runtime_mut(&mut self) -> &mut crate::ui_node::widget_object::WidgetRuntime {
        &mut self.runtime
    }
}

impl TextBlock {
    /// Returns the existing concrete wrapping policy to its COM editing interface.
    pub(crate) fn com_wrap(&self) -> TextWrap {
        self.wrap
    }
    /// Replaces wrapping; the caller records the corresponding geometry marker.
    pub(crate) fn com_set_wrap(&mut self, wrap: TextWrap) {
        self.wrap = wrap;
    }
}

impl crate::ui_node::widget_object::FontState for TextBlock {
    /// Borrows the semantic font reference already used by layout and painting.
    fn font(&self) -> &crate::FontRef {
        &self.font
    }
    /// Replaces the same field after the COM resource owner validates the selection.
    fn set_font(&mut self, font: crate::FontRef) {
        self.font = font;
    }
}

#[cfg(test)]
mod tests {
    //! Storage-boundary regressions for static multiline text.

    use super::*;

    /// Verifies both public parameter forms and later replacements canonicalize line endings.
    #[test]
    fn text_block_stores_only_lf_line_endings() {
        let mut block = TextBlock::from(TextBlockParameters {
            text: "a\r\nb\rc\n".to_owned(),
            wrap: TextWrap::None,
            font: FontRef::Role(FontRole::Body),
            opt: WidgetOption::NO_INTERACT,
        });
        assert_eq!(block.text(), "a\nb\nc\n");

        block.set_text("d\r\ne\rf\n");
        assert_eq!(block.text(), "d\ne\nf\n");
    }
}

use crate::ui_node::widget_object::{HostedWidget, direct_leaf_layout_impl, direct_widget_phase_impl, set_direct_font_role, set_direct_named_font};
use microui_contracts::{GeometryChange, IWidgetHostAbi, TextBlockAbi, TextFontRole, TextWrapping};
use neocogi_com::*;

direct_widget_phase_impl!(TextBlock);
direct_leaf_layout_impl!(TextBlock);

impl TextBlock {
    /// Constructs the class for the host-bound factory through the registry's typed contract.
    pub(crate) fn create_com(
        host: ComPtr<IWidgetHostAbi>,
        resources: crate::ui_node::widget_object::SharedAssets,
        text: NcCopyFromBufferOffset<'_, u8>,
        wrapping: TextWrapping,
    ) -> Result<ComPtr<TextBlockAbi>, NcStatus> {
        let value = Self::build(host, text, wrapping, Some(&resources))?;
        Ok(Self::from_value(value).query().expect("TextBlock declares its callable class contract"))
    }

    /// Builds one text block with either directly imported or runtime-shared resources.
    fn build(
        host: ComPtr<IWidgetHostAbi>,
        input: NcCopyFromBufferOffset<'_, u8>,
        wrapping: TextWrapping,
        resources: Option<&crate::ui_node::widget_object::SharedAssets>,
    ) -> Result<Self, NcStatus> {
        let parameters = crate::TextBlockParameters::with_wrap(crate::boundary::text(input)?, crate::ui_node::widget_object::local_wrap(wrapping)?);
        let runtime = crate::ui_node::widget_object::WidgetRuntime::for_resources(host, parameters.opt, resources)?;
        Ok(Self::from_runtime(runtime, parameters))
    }
}

impl microui_contracts::TextBlock for TextBlock {
    /// Validates text and wrapping before allocating the established TextBlock defaults.
    fn new(host: ComRef<'_, IWidgetHostAbi>, input: NcCopyFromBufferOffset<'_, u8>, wrapping: TextWrapping) -> Result<Self, NcStatus> {
        Self::build(host.add_ref(), input, wrapping, None)
    }
    /// Returns an immutable snapshot without exposing the semantic String allocation.
    fn text(&self) -> Result<ComPtr<INcStringAbi>, NcStatus> {
        self.runtime().state.readable()?;
        Ok(NcString::from_utf8_str(self.text()))
    }
    /// Replaces complete text and records only an actual geometry change.
    fn set_text(&mut self, input: NcCopyFromBufferOffset<'_, u8>) -> Result<(), NcStatus> {
        self.runtime().state.writable()?;
        let input = crate::boundary::text(input)?;
        let previous = self.text().to_owned();
        self.set_text(input);
        if self.text() != previous {
            self.runtime_mut().state.record_change(GeometryChange::CHANGED);
        }
        Ok(())
    }
    /// Encodes exactly the wrapping behavior used by existing text measurement.
    fn wrapping(&self) -> Result<TextWrapping, NcStatus> {
        self.runtime().state.readable()?;
        Ok(match self.com_wrap() {
            crate::TextWrap::None => TextWrapping::NONE,
            crate::TextWrap::Word => TextWrapping::WORD,
        })
    }
    /// Rejects unknown policies before mutating semantic state or its geometry marker.
    fn set_wrapping(&mut self, wrapping: TextWrapping) -> Result<(), NcStatus> {
        self.runtime().state.writable()?;
        let wrap = crate::ui_node::widget_object::local_wrap(wrapping)?;
        if self.com_wrap() != wrap {
            self.com_set_wrap(wrap);
            self.runtime_mut().state.record_change(GeometryChange::CHANGED);
        }
        Ok(())
    }
    /// Validates a semantic font role before changing the TextBlock's measurement input.
    fn set_font_role(&mut self, role: TextFontRole) -> Result<(), NcStatus> {
        set_direct_font_role(self, role)
    }
    /// Resolves one exact named font before replacing the TextBlock's selection.
    fn set_named_font(&mut self, name: NcCopyFromBufferOffset<'_, u8>) -> Result<(), NcStatus> {
        set_direct_named_font(self, name)
    }
}
