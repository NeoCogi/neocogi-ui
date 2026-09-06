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
//! Concrete palette/metric controls compile immutable style revisions only after an edit.
use super::app::{append, column, label, row, slider, rgba, State};
use microui_redux::{events::NumericChanged, *};
use microui_redux_demo_assets::themes::Theme;
use nc_com::{ComPtr, ComRef, NcStatus};

/// One exhaustive declaration shares color names, field access and editor presentation.
macro_rules! color_roles {
    ($($role:ident => $field:ident, $label:literal;)*) => {
        #[derive(Clone, Copy)]
        /// Named authoring role, never a positional index into renderer appearance tables.
        pub(super) enum ColorRole { $(#[doc = $label] $role,)* }
        impl ColorRole {
            /// All editable roles in presentation order; disabled colors retain their defaults.
            const ALL: [Self; 18] = [$(Self::$role,)*];
            /// Human-readable label paired with its exact named palette field.
            fn label(self) -> &'static str { match self { $(Self::$role => $label,)* } }
            /// Edits the authoring value without exposing an untyped property bag.
            fn value(self, palette: &mut FlatPalette) -> &mut Color { match self { $(Self::$role => &mut palette.$field,)* } }
        }
    };
}
color_roles! {
    Text => text, "Text"; Border => border, "Border";
    Window => window_background, "Window"; Title => title_background, "Title";
    TitleText => title_foreground, "Title text"; Panel => panel_background, "Panel";
    Button => button, "Button"; ButtonHover => button_hovered, "Button hover";
    Input => input, "Input"; InputHover => input_hovered, "Input hover";
    Track => scrollbar_track, "Scroll track"; Thumb => scrollbar_thumb, "Scroll thumb";
    Focus => control_focus, "Focus"; Selection => selection_background, "Selection";
    SelectedText => selection_foreground, "Selected text"; Active => window_active, "Active window";
    MenuText => menu_foreground, "Menu text"; Menu => menu_background, "Menu";
}

#[derive(Clone, Copy)]
/// Four explicit channels keep an RGBA edit independent of any memory-layout assumption.
pub(super) enum Channel {
    /// Red channel.
    Red,
    /// Green channel.
    Green,
    /// Blue channel.
    Blue,
    /// Opacity channel.
    Alpha,
}
impl Channel {
    /// Matches the editor's visual left-to-right channel order.
    const ALL: [Self; 4] = [Self::Red, Self::Green, Self::Blue, Self::Alpha];
    /// Returns the named byte for a silent refresh or an accepted numeric edit.
    fn value(self, color: &mut Color) -> &mut u8 {
        match self {
            Self::Red => &mut color.r,
            Self::Green => &mut color.g,
            Self::Blue => &mut color.b,
            Self::Alpha => &mut color.a,
        }
    }
}

#[derive(Clone, Copy)]
/// Layout metrics demonstrated by the original live editor.
pub(super) enum Metric {
    /// Inner control padding.
    Padding,
    /// Space between neighboring cells.
    Spacing,
    /// Window title height.
    Title,
    /// Minimum draggable thumb extent.
    Thumb,
    /// Scrollbar transverse size.
    Scrollbar,
}
impl Metric {
    /// Preserves the original metric editor order.
    const ALL: [Self; 5] = [Self::Padding, Self::Spacing, Self::Title, Self::Thumb, Self::Scrollbar];
    /// Pairs display text with the original useful slider range.
    fn presentation(self) -> (&'static str, f32) {
        match self {
            Self::Padding => ("Padding", 16.),
            Self::Spacing => ("Spacing", 16.),
            Self::Title => ("Title height", 128.),
            Self::Thumb => ("Thumb size", 128.),
            Self::Scrollbar => ("Scrollbar", 128.),
        }
    }
    /// Selects the exact shared metric field; the wire record is also the authoring value.
    fn value(self, metrics: &mut AssetMetrics) -> &mut i32 {
        match self {
            Self::Padding => &mut metrics.padding,
            Self::Spacing => &mut metrics.spacing,
            Self::Title => &mut metrics.title_height,
            Self::Thumb => &mut metrics.thumb_size,
            Self::Scrollbar => &mut metrics.scrollbar_size,
        }
    }
}

/// A retained editor row owns capabilities, while palette state remains one named authoring value.
struct ColorRow {
    /// The palette field controlled by this row.
    role: ColorRole,
    /// Real RGBA sliders used for both subscriptions and silent refreshes.
    channels: [Slider; 4],
    /// Real color preview painted by the UI plugin.
    swatch: ColorSwatch,
}

/// Complete authoring state for the demo; immutable revisions are published transactionally by Ui.
pub(super) struct StyleEditor {
    /// Original catalog makes all theme selections valid revisions of the same application assets.
    base: ComPtr<IAssetBundleAbi>,
    /// Asset-owned compiler remains independent of the currently selected UI tree.
    compiler: ComPtr<IStyleCompilerAbi>,
    /// Selected pristine theme determines whether flat palette editing is meaningful.
    selected: Theme,
    /// Application-owned flat recipe, never a second live renderer appearance table.
    palette: FlatPalette,
    /// Current editable scalar geometry, copied from the installed revision.
    metrics: AssetMetrics,
    /// Concrete rows retain only the controls needed by application handlers.
    colors: Vec<ColorRow>,
    /// Concrete scalar controls paired with their named metric.
    values: Vec<(Metric, Slider)>,
}
impl StyleEditor {
    /// Builds the same 72 color sliders and five metrics inside one ordinary scrolling tree.
    pub(super) fn new(ui: &Ui) -> Result<(Self, ScrollArea), NcStatus> {
        let base = ui.assets()?;
        // Activation belongs to this application; the UI runtime has no asset-provider dependency.
        let compiler = unsafe { nc_com::Runtime::global().construct_class_as(STYLE_COMPILER_CLASS, std::ptr::null_mut())? };
        let mut metrics = base.metrics()?;
        let mut palette = FlatPalette::default();
        let content = column(ui)?;
        let mut colors = Vec::new();
        for role in ColorRole::ALL {
            let line = row(ui)?;
            append(&line, &label(ui, role.label())?, LayoutTrack::Fixed { extent: 80 })?;
            let color = role.value(&mut palette);
            let channels = [
                slider(ui, color.r as f32, 255.)?,
                slider(ui, color.g as f32, 255.)?,
                slider(ui, color.b as f32, 255.)?,
                slider(ui, color.a as f32, 255.)?,
            ];
            for control in &channels {
                append(&line, control, LayoutTrack::Flex { weight: 1. })?;
            }
            let swatch = ui.widgets().color_swatch(rgba(*color), "")?;
            append(&line, &swatch, LayoutTrack::Flex { weight: 1. })?;
            append(&content, &line, LayoutTrack::Content)?;
            colors.push(ColorRow { role, channels, swatch });
        }
        let mut values = Vec::new();
        for metric in Metric::ALL {
            let (name, high) = metric.presentation();
            let line = row(ui)?;
            let control = slider(ui, *metric.value(&mut metrics) as f32, high)?;
            append(&line, &label(ui, name)?, LayoutTrack::Fixed { extent: 80 })?;
            append(&line, &control, LayoutTrack::Flex { weight: 1. })?;
            append(&content, &line, LayoutTrack::Content)?;
            values.push((metric, control));
        }
        let root = ui.widgets().scroll_area(&content, true)?;
        Ok((
            Self {
                base,
                compiler,
                selected: Theme::Default,
                palette,
                metrics,
                colors,
                values,
            },
            root,
        ))
    }
    /// Registers typed handlers after State has been completely constructed.
    pub(super) fn subscribe(&self, events: &EventSession<State>) -> Result<(), NcStatus> {
        for row in &self.colors {
            for (channel, control) in Channel::ALL.into_iter().zip(&row.channels) {
                events.subscribe_numeric(control, Edit::Color(row.role, channel))?;
            }
        }
        for (metric, control) in &self.values {
            events.subscribe_numeric(control, Edit::Metric(*metric))?;
        }
        Ok(())
    }
    /// Installs a pristine theme in the original catalog and resets the editor's controls.
    pub(super) fn select(&mut self, selected: Theme, ui: &Ui, renderer: ComRef<'_, IRendererAbi>) -> Result<(), NcStatus> {
        let candidate = selected.load(self.base.as_com_ref())?;
        let metrics = candidate.metrics()?;
        ui.install_assets(candidate.as_com_ref(), renderer)?;
        self.selected = selected;
        self.palette = FlatPalette::default();
        self.metrics = metrics;
        self.refresh()
    }
    /// Publishes one changed style immediately so later input sees its committed layout.
    fn publish(&self, ui: &Ui, renderer: ComRef<'_, IRendererAbi>) -> Result<(), NcStatus> {
        let palette = if self.selected == Theme::Default {
            PaletteEdit::Flat(self.palette)
        } else {
            PaletteEdit::Preserve
        };
        let current = ui.assets()?;
        let candidate = self.compiler.derive_style(current.as_com_ref(), self.metrics, palette.into())?;
        ui.install_assets(candidate.as_com_ref(), renderer)
    }
    /// Spacing menu actions share the editor's exact metric and revision path.
    pub(super) fn spacing(&mut self, spacing: i32, ui: &Ui, renderer: ComRef<'_, IRendererAbi>) -> Result<(), NcStatus> {
        self.metrics.spacing = spacing;
        self.publish(ui, renderer)?;
        self.refresh()
    }
    /// Silent control updates neither synthesize user events nor continuously reinstall a theme.
    fn refresh(&mut self) -> Result<(), NcStatus> {
        for row in &self.colors {
            let color = row.role.value(&mut self.palette);
            for (channel, control) in Channel::ALL.into_iter().zip(&row.channels) {
                control.set_value(*channel.value(color) as f32)?;
                let options = WidgetOptions::FRAME.bits
                    | WidgetOptions::ALIGN_CENTER.bits
                    | if self.selected == Theme::Default {
                        0
                    } else {
                        WidgetOptions::NO_INTERACT.bits
                    };
                control.set_options(WidgetOptions { bits: options })?;
            }
            row.swatch.set_fill(rgba(*color))?;
        }
        for (metric, control) in &self.values {
            control.set_value(*metric.value(&mut self.metrics) as f32)?;
        }
        Ok(())
    }
}

/// Every numeric event identifies its actual named authoring target.
enum Edit {
    /// One color channel; image themes reject flat edits and restore their slider display.
    Color(ColorRole, Channel),
    /// One complete scalar geometry field.
    Metric(Metric),
}
impl EventHandler<State, NumericChanged> for Edit {
    /// Applies the candidate, installs it, and refreshes previews before returning to input routing.
    fn handle(&mut self, state: &mut State, event: &NumericChanged) -> Result<(), NcStatus> {
        let editor = &mut state.style;
        match *self {
            Self::Color(role, channel) if editor.selected == Theme::Default => {
                *channel.value(role.value(&mut editor.palette)) = event.value as u8;
            }
            Self::Color(..) => return editor.refresh(),
            Self::Metric(metric) => *metric.value(&mut editor.metrics) = event.value as i32,
        }
        editor.publish(&state.ui, state.renderer.as_com_ref())?;
        editor.refresh()
    }
}
