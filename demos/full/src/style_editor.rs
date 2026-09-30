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
use super::app::{append, column, label, row, slider, rgba, NumericTarget};
use neocogi_ui::*;
use neocogi_demo_assets::themes::Theme;
use neocogi_com::{ComPtr, ComRef, NcStatus};

/// Palette and metric edits promoted into the application's single message inbox.
pub(super) enum Message {
    /// Exact palette role and channel with the committed byte value.
    Color(ColorRole, Channel, u8),
    /// Exact layout metric with the committed integer value.
    Metric(Metric, i32),
}

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
    channels: [SliderWidget; 4],
    /// Real color preview painted by the UI plugin.
    swatch: ColorSwatchWidget,
}

/// Complete authoring state for the demo; immutable revisions are published transactionally by Ui.
pub(super) struct StyleEditor {
    /// Original catalog makes all theme selections valid revisions of the same application assets.
    base: ComPtr<IAssetBundleAbi>,
    /// Asset-owned compiler remains independent of the currently selected UI tree.
    compiler: ComPtr<StyleCompilerAbi>,
    /// Selected pristine theme determines whether flat palette editing is meaningful.
    selected: Theme,
    /// Application-owned flat recipe, never a second live renderer appearance table.
    palette: FlatPalette,
    /// Current editable scalar geometry, copied from the installed revision.
    metrics: AssetMetrics,
    /// Latest immutable candidate awaits the exclusive runtime boundary after event dispatch.
    pending: Option<ComPtr<IAssetBundleAbi>>,
    /// Concrete rows retain only the controls needed by application handlers.
    colors: Vec<ColorRow>,
    /// Concrete scalar controls paired with their named metric.
    values: Vec<(Metric, SliderWidget)>,
}
impl StyleEditor {
    /// Builds the same 72 color sliders and five metrics inside one ordinary scrolling tree.
    pub(super) fn new(ui: &Ui, events_tx: &mut super::app::Inputs) -> Result<(Self, OwnedWidget<ScrollAreaAbi>), NcStatus> {
        let base = ui.assets()?;
        // Activation belongs to this application; the UI runtime has no asset-provider dependency.
        let runtime = neocogi_com::Runtime::global();
        let gfx = runtime.activate_plugin_factory::<GfxPluginFactoryAbi>(neocogi_com::guid_from_qualified_name("neocogi.gfx.runtime"))?;
        let publisher = gfx.asset_publisher()?;
        let factory = runtime.activate_plugin_factory::<SkinPluginFactoryAbi>(neocogi_com::guid_from_qualified_name("neocogi.skin.runtime"))?;
        let compiler = factory.style_compiler(publisher.as_com_ref())?;
        let mut metrics = base.metrics()?;
        let mut palette = FlatPalette::default();
        let mut content = column(ui)?;
        let mut colors = Vec::new();
        for role in ColorRole::ALL {
            let mut line = row(ui)?;
            append(&mut line, label(ui, role.label())?, LayoutTrack::Fixed { extent: 80 })?;
            let color = role.value(&mut palette);
            let mut channel_slider = |channel: Channel, value: u8| slider(ui, value as f32, 255., events_tx, NumericTarget::Color(role, channel));
            let channels = [
                channel_slider(Channel::Red, color.r)?,
                channel_slider(Channel::Green, color.g)?,
                channel_slider(Channel::Blue, color.b)?,
                channel_slider(Channel::Alpha, color.a)?,
            ];
            let channel_handles = channels.each_ref().map(|control| control.handle());
            for control in channels {
                append(&mut line, control, LayoutTrack::Flex { weight: 1. })?;
            }
            let swatch = ui.widgets().color_swatch(rgba(*color), "")?;
            let swatch_handle = swatch.handle();
            append(&mut line, swatch, LayoutTrack::Flex { weight: 1. })?;
            append(&mut content, line, LayoutTrack::Content)?;
            colors.push(ColorRow { role, channels: channel_handles, swatch: swatch_handle });
        }
        let mut values = Vec::new();
        for metric in Metric::ALL {
            let (name, high) = metric.presentation();
            let mut line = row(ui)?;
            let control = slider(ui, *metric.value(&mut metrics) as f32, high, events_tx, NumericTarget::Metric(metric))?;
            append(&mut line, label(ui, name)?, LayoutTrack::Fixed { extent: 80 })?;
            let handle = control.handle();
            append(&mut line, control, LayoutTrack::Flex { weight: 1. })?;
            append(&mut content, line, LayoutTrack::Content)?;
            values.push((metric, handle));
        }
        let root = ui.widgets().scroll_area(content, true).map_err(|(status, _candidate)| status)?;
        Ok((
            Self {
                base,
                compiler,
                selected: Theme::Default,
                palette,
                metrics,
                pending: None,
                colors,
                values,
            },
            root,
        ))
    }
    /// Applies one editor command through ordinary mutable fields and silent control setters.
    pub(super) fn update(&mut self, ui: &Ui, message: Message) -> Result<(), NcStatus> {
        match message {
            Message::Color(role, channel, value) => match self.selected {
                Theme::Default => *channel.value(role.value(&mut self.palette)) = value,
                _ => return self.refresh(),
            },
            Message::Metric(metric, value) => *metric.value(&mut self.metrics) = value,
        }
        self.prepare(ui)?;
        self.refresh()
    }
    /// Prepares a pristine theme in the original catalog and resets the editor's controls.
    pub(super) fn select(&mut self, selected: Theme) -> Result<(), NcStatus> {
        let candidate = selected.load(self.base.as_com_ref())?;
        let metrics = candidate.metrics()?;
        self.selected = selected;
        self.palette = FlatPalette::default();
        self.metrics = metrics;
        self.pending = Some(candidate);
        self.refresh()
    }
    /// Compiles one changed style against the latest prepared or installed revision.
    fn prepare(&mut self, ui: &Ui) -> Result<(), NcStatus> {
        let palette = if self.selected == Theme::Default {
            PaletteEdit::Flat(self.palette)
        } else {
            PaletteEdit::Preserve
        };
        let installed;
        let current = if let Some(candidate) = &self.pending {
            candidate.as_com_ref()
        } else {
            installed = ui.assets()?;
            installed.as_com_ref()
        };
        self.pending = Some(self.compiler.derive_style(current, self.metrics, palette.into())?);
        Ok(())
    }
    /// Installs the latest candidate only after the active event-dispatch call has returned.
    pub(super) fn install(&mut self, ui: &mut Ui, renderer: ComRef<'_, IRendererAbi>) -> Result<(), NcStatus> {
        let Some(candidate) = self.pending.take() else {
            return Ok(());
        };
        if let Err(error) = ui.install_assets(candidate.as_com_ref(), renderer) {
            self.pending = Some(candidate);
            return Err(error);
        }
        Ok(())
    }
    /// Spacing menu actions share the editor's exact metric and candidate path.
    pub(super) fn spacing(&mut self, spacing: i32, ui: &Ui) -> Result<(), NcStatus> {
        self.metrics.spacing = spacing;
        self.prepare(ui)?;
        self.refresh()
    }
    /// Silent control updates neither synthesize user events nor continuously reinstall a theme.
    fn refresh(&mut self) -> Result<(), NcStatus> {
        for row in &mut self.colors {
            let color = row.role.value(&mut self.palette);
            for (channel, control) in Channel::ALL.into_iter().zip(&mut row.channels) {
                control.set_value(*channel.value(color) as f32)?;
                let options = WidgetOptions::FRAME
                    | WidgetOptions::ALIGN_CENTER
                    | if self.selected == Theme::Default {
                        WidgetOptions::NONE
                    } else {
                        WidgetOptions::NO_INTERACT
                    };
                control.replace_flags(options)?;
            }
            row.swatch.set_fill(rgba(*color))?;
        }
        for (metric, control) in &mut self.values {
            control.set_value(*metric.value(&mut self.metrics) as f32)?;
        }
        Ok(())
    }
}
