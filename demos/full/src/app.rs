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
//! Demo composition keeps concrete application state and actions local to the host binary.
use super::{
    falloff::FalloffEditor,
    grid::{GridState, GridWidget as GridViewWidget},
    mesh::Suzanne,
    painter_demo::PainterDemo,
    style_editor::StyleEditor,
};
use neocogi_ui::{events::*, *};
use neocogi_demo_assets::themes::Theme;
use neocogi_demo_host::{AppState, Host, HostError};
use neocogi_com::{ComInterface, ComPtr, ComRef, NC_E_INVALID_ARG, NcCopyFromBufferOffset, NcStatus};
use std::{cell::RefCell, rc::Rc};

/// Application-owned combo values never cross the ABI as an erased generic collection.
const FRUIT: [&str; 4] = ["Apple", "Banana", "Cherry", "Date"];
/// Ordinary expanding edge reused by explicit tree construction.
const FLEX: LayoutTrack = LayoutTrack::Flex { weight: 1. };

/// Converts named channels explicitly to the shared packed-RGBA wire convention.
pub(super) fn rgba(color: Color) -> u32 {
    u32::from_le_bytes([color.r, color.g, color.b, color.a])
}
/// Small geometry constructor keeps the initial window arrangement readable.
fn rect(x: i32, y: i32, width: i32, height: i32) -> Rect {
    Rect { x, y, width, height }
}
/// A vertical line owns exactly the children inserted through its COM editor.
pub(super) fn column(ui: &Ui) -> Result<LinearWidget, NcStatus> {
    ui.widgets().linear(FlowDirection::TopToBottom, CrossSize::Stretch)
}
/// A row derives height from its actual children, so theme/font changes cannot stale a pixel estimate.
pub(super) fn row(ui: &Ui) -> Result<LinearWidget, NcStatus> {
    ui.widgets().linear(FlowDirection::LeftToRight, CrossSize::Content)
}
/// Appends through the authoritative child count; no second local list or builder closure is retained.
pub(super) fn append<I: ComInterface>(parent: &mut LinearWidget, child: &Widget<I>, track: LayoutTrack) -> Result<(), NcStatus> {
    parent.insert(parent.children()?.child_count()?, child, LinearEdge::new(track))?.finish()
}
/// Static display text remains a real plugin-owned TextBlock with no application callback.
pub(super) fn label(ui: &Ui, text: &str) -> Result<TextBlockWidget, NcStatus> {
    ui.widgets().text_block(text, TextWrapping::None)
}
/// Shared framed, centered numeric control used by the palette and background editors.
pub(super) fn slider(ui: &Ui, value: f32, high: f32) -> Result<SliderWidget, NcStatus> {
    let mut control = ui.widgets().slider(SliderOptions {
        value,
        low: 0.,
        high,
        step: 1.,
        precision: 0,
    })?;
    control.set_options(WidgetOptions::FRAME | WidgetOptions::ALIGN_CENTER)?;
    Ok(control)
}
/// Creates and immediately subscribes a concrete button; only its parent needs to retain it afterward.
fn button(ui: &Ui, events: &EventSession<State>, text: &str, action: Action) -> Result<ButtonWidget, NcStatus> {
    let mut control = ui.widgets().button(text)?;
    control.set_options(WidgetOptions::FRAME | WidgetOptions::ALIGN_CENTER)?;
    events.subscribe(control.activated(), move |state: &mut State, event: &Activation| state.activate(action, event))?;
    Ok(control)
}
/// A disclosure body is its actual editable Linear COM object, with no placeholder replacement tree.
fn section(ui: &Ui, parent: &mut LinearWidget, text: &str, style: DisclosureStyle, expanded: bool) -> Result<LinearWidget, NcStatus> {
    let section = ui.widgets().disclosure(text, style, expanded)?;
    append(parent, &section, LayoutTrack::Content)?;
    section.body()
}
/// Mounts one fully expanding child directly under a structural floating window.
fn floating<I: ComInterface>(ui: &Ui, desktop: ComRef<'_, IWindowAbi>, title: &str, area: Rect, content: &Widget<I>) -> Result<ComPtr<IWindowAbi>, NcStatus> {
    let window = ui.child_window(desktop, title, area, content)?;
    window.set_options(WindowOptions::FRAME)?;
    Ok(window)
}

/// Application state owns only models and editing/event capabilities needed by its actions.
pub(super) struct State {
    /// A retained runtime projection reads during handlers and installs after dispatch returns.
    pub(super) ui: Ui,
    /// Named palette authoring and the actual retained editor controls.
    pub(super) style: StyleEditor,
    /// Background drawing and grid actions share this exact camera/color model.
    grid: Rc<RefCell<GridState>>,
    /// Fullscreen structural parent follows the physical platform drawable extent.
    desktop: ComPtr<IWindowAbi>,
    /// Movable main window can be hidden and restored through the persistent grid menu.
    demo: ComPtr<IWindowAbi>,
    /// Actual sampler scrolling is retained for navigation and complete-frame image verification.
    sample_scroll: ScrollAreaWidget,
    /// Position, dimensions and sampled frame-rate presentations belong to this main window.
    info: [TextBlockWidget; 3],
    /// Minimized geometry must not be clamped back to the normal minimum size.
    minimized: bool,
    /// Actual log text is the only stored log buffer; appends read it only when an action writes.
    log: TextBlockWidget,
    /// Scroll requests survive measurement changes caused by log appends and style edits.
    log_scroll: ScrollAreaWidget,
    /// Enter and Submit edit and clear this actual single-line control.
    entry: TextboxWidget,
    /// User-owned policy for following new output.
    auto_scroll: bool,
    /// Concrete check item displays the same auto-scroll policy.
    auto_item: MenuItem,
    /// Concrete density item reflects the grid's minor-line policy.
    minor_item: MenuItem,
    /// Theme radio selection changes only after successful installation.
    themes: Vec<(Theme, MenuItem)>,
    /// Comfortable and compact spacing radio items share the style editor path.
    spacing: [MenuItem; 2],
    /// Complete combo box owns its labels, optional selection, and private popup.
    combo: ComboWidget,
    /// Standalone compact menu opened by the Popup button.
    test_popup: ComPtr<IPopupAbi>,
    /// Color preview reflects the model used to paint the fullscreen background.
    background: ColorSwatchWidget,
    /// Native picker remains reusable across complete/cancel requests; browsers have no OS filesystem.
    file_dialog: Option<FileDialog>,
    /// Open menu eligibility follows the real component's active state.
    open_item: MenuItem,
    /// Last successful frame time supports frame-rate smoothing without wall-clock dependence.
    last_seconds: f64,
    /// Exponential frame-rate estimate avoids replacing text for each insignificant clock fluctuation.
    fps: f64,
    /// Next visible FPS refresh, limited to four times per second.
    next_fps: f64,
}
impl State {
    /// Updates one actual TextBlock and requests the next measured end when enabled.
    fn write_log(&mut self, message: &str) -> Result<(), NcStatus> {
        let mut text = self.log.text()?;
        if !text.is_empty() {
            text.push('\n');
        }
        text.push_str(message);
        self.log.set_text(&text)?;
        if self.auto_scroll {
            self.log_scroll.scroll_to_end()?;
        }
        Ok(())
    }
    /// Enter and the Submit button share one concrete operation and clear the actual input.
    fn submit(&mut self) -> Result<(), NcStatus> {
        self.write_log(&self.entry.text()?)?;
        self.entry.clear()
    }
    /// Native read failures remain visible in the log and do not stop the whole demo.
    fn open_file(&mut self) -> Result<(), NcStatus> {
        let Some(dialog) = &self.file_dialog else {
            return self.write_log("File browsing requires a native filesystem.");
        };
        #[cfg(not(target_arch = "wasm32"))]
        let directory = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
        #[cfg(target_arch = "wasm32")]
        let directory = std::path::PathBuf::from("/");
        if let Err(status) = dialog.open(&FileDialogRequest::new(&directory)) {
            let diagnostic = dialog.diagnostic()?;
            return self.write_log(&format!("Open failed: {status}: {diagnostic}"));
        }
        self.open_item.set_enabled(false)
    }
    /// Prepares a theme candidate before updating its corresponding application presentation.
    fn select_theme(&mut self, selected: Theme) -> Result<(), NcStatus> {
        self.style.select(selected)?;
        for (theme, item) in &self.themes {
            item.set_mark(MenuMark::Radio { selected: *theme == selected })?;
        }
        self.write_log(&format!("Selected {}", selected.label()))
    }
}
impl AppState for State {
    /// Publishes prepared assets after dispatch; geometry then follows the platform drawable.
    fn frame(&mut self, host: &Host, size: Size, seconds: f64) -> Result<bool, NcStatus> {
        self.style.install(&mut self.ui, host.renderer.as_com_ref())?;
        let area = rect(0, 0, size.width, size.height);
        if self.desktop.snapshot()?.rect != area {
            self.desktop.set_rect(area)?;
        }
        if seconds > self.last_seconds {
            let sample = 1. / (seconds - self.last_seconds);
            self.fps = if self.fps == 0. { sample } else { self.fps * 0.9 + sample * 0.1 };
        }
        self.last_seconds = seconds;
        if seconds >= self.next_fps {
            self.info[2].set_text(&format!("{:.1}", self.fps))?;
            self.next_fps = seconds + 0.25;
        }
        Ok(true)
    }
}

#[derive(Clone, Copy)]
/// Each stored action exposes exactly the data needed to interpret a captured activation.
enum Action {
    /// Append the captured producer label.
    Log,
    /// Append input and clear the textbox.
    Submit,
    /// Open a standalone compact popup at the latest consumed pointer position.
    Popup,
    /// Start a native file-picker request.
    Open,
    /// Reset log and input for a new demonstration session.
    New,
    /// Remove all displayed log text.
    Clear,
    /// Toggle following newly appended log lines.
    AutoScroll,
    /// Select one concrete base theme.
    Theme(Theme),
    /// Select comfortable (true) or compact (false) spacing.
    Spacing(bool),
    /// Reset the grid camera while preserving its chosen density and background.
    ResetGrid,
    /// Toggle unit-spaced grid lines.
    MinorLines,
    /// Restore the existing hidden demo window.
    ShowDemo,
    /// Scroll the sampler to its actual image controls.
    ShowTextures,
    /// Describe the active demo in its log.
    About,
}
impl State {
    /// Runs after the UI releases its context, permitting ordinary concrete edits and menu coordination.
    fn activate(&mut self, action: Action, event: &Activation) -> Result<(), NcStatus> {
        match action {
            Action::Log => self.write_log(&format!("Pressed {}", event.label)),
            Action::Submit => self.submit(),
            // The popup capability resolves the pointer captured by the triggering input boundary,
            // so moving the Demo Window does not require application-authored coordinate offsets.
            Action::Popup => self.test_popup.show(),
            Action::Open => self.open_file(),
            Action::New => {
                self.log.clear()?;
                self.entry.clear()?;
                self.write_log("New demo session")
            }
            Action::Clear => self.log.clear(),
            Action::AutoScroll => {
                self.auto_scroll = !self.auto_scroll;
                self.auto_item.set_mark(MenuMark::Checked { checked: self.auto_scroll })?;
                if self.auto_scroll {
                    self.log_scroll.scroll_to_end()?;
                }
                Ok(())
            }
            Action::Theme(theme) => self.select_theme(theme),
            Action::Spacing(comfortable) => {
                self.style.spacing(if comfortable { 8 } else { 2 }, &self.ui)?;
                self.spacing[0].set_mark(MenuMark::Radio { selected: comfortable })?;
                self.spacing[1].set_mark(MenuMark::Radio { selected: !comfortable })
            }
            Action::ResetGrid => {
                self.grid.borrow_mut().view = super::grid::create_grid_view_3d();
                Ok(())
            }
            Action::MinorLines => {
                let checked = {
                    let mut grid = self.grid.borrow_mut();
                    grid.minor_lines = !grid.minor_lines;
                    grid.minor_lines
                };
                self.minor_item.set_mark(MenuMark::Checked { checked })
            }
            Action::ShowDemo => {
                self.demo.set_visibility(Visibility::Visible)?;
                self.demo.raise()
            }
            Action::ShowTextures => self.sample_scroll.scroll_to_end(),
            Action::About => self.write_log("neocogi-ui COM demo: concrete widgets, typed events, shared layout, PNG images and renderer-owned 3D scenes."),
        }
    }

    /// Enter uses the captured submitted text, which may differ from a later silent edit.
    fn text_submitted(&mut self, event: &TextSubmitted) -> Result<(), NcStatus> {
        self.write_log(&event.text)?;
        self.entry.clear()
    }

    /// Resolves display text separately from the generated selection event.
    fn choice_changed(&mut self, event: &neocogi_ui::events::ChoiceChanged) -> Result<(), NcStatus> {
        let event = &event.selection;
        match *event {
            ChoiceSelection::None => self.write_log("Selection cleared"),
            ChoiceSelection::Selected(index) => {
                let index = usize::try_from(index).map_err(|_| NC_E_INVALID_ARG)?;
                let label = self.combo.choice_label(index)?;
                self.write_log(&format!("Selected {label}"))
            }
        }
    }

    /// Accepted paths remain native values until explicitly formatted for this human-readable log.
    fn file_selected(&mut self, event: &FileDialogCompletionMessage) -> Result<(), NcStatus> {
        self.open_item.set_enabled(true)?;
        match completed_file_path(event)? {
            Some(path) => self.write_log(&format!("Selected file: {}", path.display())),
            None => self.write_log("File dialog cancelled"),
        }
    }

    /// Preserves manager-owned minimize/maximize/restore rectangles and clamps only ordinary geometry.
    fn window_changed(&mut self, event: &neocogi_ui::events::WindowChanged) -> Result<(), NcStatus> {
        let event = &event.event;
        let (mut area, clamp) = match *event {
            WindowEvent::GeometryChanged { rect } => (rect, !self.minimized),
            WindowEvent::Minimized { .. } => {
                self.minimized = true;
                return self.write_log("Minimized Demo Window");
            }
            WindowEvent::Maximized { rect } | WindowEvent::Restored { rect } => {
                self.minimized = false;
                (rect, false)
            }
            WindowEvent::CloseRequested => return self.write_log("Closed Demo Window; use Grid > Show Demo Window to restore it"),
            WindowEvent::Hidden | WindowEvent::Destroyed => return Ok(()),
        };
        if clamp && (area.width < 240 || area.height < 300) {
            area.width = area.width.max(240);
            area.height = area.height.max(300);
            self.demo.set_rect(area)?;
        }
        self.info[0].set_text(&format!("{}, {}", area.x, area.y))?;
        self.info[1].set_text(&format!("{}, {}", area.width, area.height))
    }
    /// Paint-only external state reports NoChange; its intrinsic dimensions do not depend on color.
    /// The captured channel shift selects byte 0, 8, or 16 in the packed RGBA value.
    fn background_changed(&mut self, shift: u32, event: &NumericChanged) -> Result<(), NcStatus> {
        let value = {
            let mut grid = self.grid.borrow_mut();
            grid.background = (grid.background & !(255 << shift)) | ((event.value as u32) << shift);
            grid.background
        };
        self.background.set_fill(value)
    }
}

/// Colored triangle animation owns only its clock-derived angle and emits ordered solid geometry.
#[derive(Default)]
struct Triangle {
    /// Updated once per ordinary widget update; fixed intrinsic size needs no geometry invalidation.
    angle: f32,
}
impl LeafContent for Triangle {
    /// A flex window expands this modest intrinsic size.
    fn measure(&self, _: LayoutLimits, _: ComRef<'_, ITextMetricsAbi>) -> Result<Size, NcStatus> {
        Ok(Size { width: 80, height: 24 })
    }
    /// Time changes color geometry, not measurement; authors explicitly own this NoChange decision.
    fn update(&mut self, _: WidgetUpdate, _: WidgetEvent, _: &str) -> Result<GeometryChange, NcStatus> {
        self.angle = neocogi_demo_host::monotonic_seconds() as f32;
        Ok(GeometryChange::NoChange)
    }
    /// Rotates three colored points around the committed viewport center.
    fn paint(&self, state: WidgetPaint, painter: &mut Painter<'_>) -> Result<(), NcStatus> {
        let area = state.rect;
        let radius = area.width.min(area.height) as f32 * 0.4;
        let vertices = [(0., 0xff0000ff), (1., 0xff00ff00), (2., 0xffff0000)].map(|(index, rgba)| {
            let angle = self.angle + index * std::f32::consts::TAU / 3.;
            ColoredVertex {
                x: area.x as f32 + area.width as f32 * 0.5 + angle.cos() * radius,
                y: area.y as f32 + area.height as f32 * 0.5 + angle.sin() * radius,
                rgba,
            }
        });
        painter.colored_triangles(&vertices)
    }
}

/// Collects concrete menu actions only during construction, then transfers their subscriptions to the session.
fn menu_item(menu: &MenuBuilder, pending: &mut Vec<(MenuItem, Action)>, text: &str, action: Action) -> Result<MenuItem, NcStatus> {
    let item = menu.item(text)?;
    pending.push((item.clone(), action));
    Ok(item)
}

/// Creates the complete demo with the same structural desktop and floating examples on every backend.
fn create() -> Result<(Host, EventSession<State>), HostError> {
    neocogi_demo_host::initialize()?;
    let host = Host::new("neocogi-ui COM Demo", Size { width: 1200, height: 800 }, neocogi_demo_assets::load_bundle()?)?;
    let ui = &host.ui;
    let grid = Rc::new(RefCell::new(GridState::default()));
    let grid_widget = ui.widgets().leaf(GridViewWidget::new(grid.clone()), WidgetOptions::NONE)?;
    let desktop = ui.window("X-Y Grid Surface", rect(0, 0, 1200, 800), &grid_widget)?;
    desktop.set_layer(0)?;
    desktop.set_child_clip(ChildClip::Content)?;
    desktop.set_options(WindowOptions::NO_TITLE | WindowOptions::NO_CLOSE | WindowOptions::NO_RESIZE | WindowOptions::NO_PADDING)?;
    let mut main_body = column(ui)?;
    let main_scroll = ui.widgets().scroll_area(&main_body, true)?;
    let demo = floating(ui, desktop.as_com_ref(), "Demo Window", rect(40, 40, 300, 450), &main_scroll)?;
    demo.set_options(WindowOptions::FRAME | WindowOptions::MINIMIZE_BUTTON | WindowOptions::MAXIMIZE_BUTTON)?;
    let info = [label(ui, "40, 40")?, label(ui, "300, 450")?, label(ui, "0.0")?];
    let combo = ui.combo_box(demo.as_com_ref(), &FRUIT, ChoiceSelection::Selected(0))?;
    let menus = ui.menus()?;
    let mut pending = Vec::new();
    let popup_menu = menus.create("Test Popup")?;
    menu_item(&popup_menu, &mut pending, "Hello", Action::Log)?;
    menu_item(&popup_menu, &mut pending, "World", Action::Log)?;
    let test_popup = menus.popup(demo.as_com_ref(), &popup_menu)?;
    let bar = menus.create("Demo Menu")?;
    let file = bar.submenu("File")?;
    menu_item(&file, &mut pending, "New Session", Action::New)?;
    let open_item = menu_item(&file, &mut pending, "Open File...", Action::Open)?;
    open_item.set_shortcut_hint("Ctrl+O")?;
    file.separator()?;
    menu_item(&file, &mut pending, "Clear Log", Action::Clear)?;
    let view = bar.submenu("View")?;
    let auto_item = menu_item(&view, &mut pending, "Auto-scroll Log", Action::AutoScroll)?;
    auto_item.set_mark(MenuMark::Checked { checked: true })?;
    menu_item(&view, &mut pending, "Show Textures", Action::ShowTextures)?;
    let spacing_menu = view.submenu("Spacing")?;
    let spacing = [
        menu_item(&spacing_menu, &mut pending, "Comfortable", Action::Spacing(true))?,
        menu_item(&spacing_menu, &mut pending, "Compact", Action::Spacing(false))?,
    ];
    let theme_menu = view.submenu("Theme")?;
    let mut themes = Vec::new();
    for theme in Theme::ALL {
        let item = menu_item(&theme_menu, &mut pending, theme.label(), Action::Theme(theme))?;
        item.set_mark(MenuMark::Radio { selected: theme == Theme::Default })?;
        themes.push((theme, item));
    }
    let help = bar.submenu("Help")?;
    menu_item(&help, &mut pending, "About", Action::About)?;
    menus.attach_bar(demo.as_com_ref(), &bar)?;
    let bar = menus.create("Grid Menu")?;
    let grid_menu = bar.submenu("Grid")?;
    menu_item(&grid_menu, &mut pending, "Reset View", Action::ResetGrid)?;
    let minor_item = menu_item(&grid_menu, &mut pending, "Minor Lines", Action::MinorLines)?;
    minor_item.set_mark(MenuMark::Checked { checked: true })?;
    grid_menu.separator()?;
    menu_item(&grid_menu, &mut pending, "Show Demo Window", Action::ShowDemo)?;
    menu_item(&grid_menu, &mut pending, "About", Action::About)?;
    menus.attach_bar(desktop.as_com_ref(), &bar)?;
    let mut log = label(ui, "Welcome to neocogi-ui.")?;
    log.set_font_role(FontRole::Mono)?;
    let log_scroll = ui.widgets().scroll_area(&log, true)?;
    let mut entry = ui.widgets().textbox("")?;
    entry.set_font_role(FontRole::Mono)?;
    let (style, style_root) = StyleEditor::new(ui)?;
    floating(ui, desktop.as_com_ref(), "Flat Palette / Skin Metrics", rect(350, 250, 300, 240), &style_root)?;
    let background = ui.widgets().color_swatch(grid.borrow().background, "#5A5F64")?;
    #[cfg(not(target_arch = "wasm32"))]
    let file_dialog = Some(ui.file_dialog(demo.as_com_ref())?);
    #[cfg(target_arch = "wasm32")]
    let file_dialog = None;
    open_item.set_enabled(file_dialog.is_some())?;
    let events = EventSession::new(State {
        ui: ui.clone(),
        style,
        grid,
        desktop,
        demo,
        info,
        minimized: false,
        sample_scroll: main_scroll,
        log,
        log_scroll,
        entry,
        auto_scroll: true,
        auto_item,
        minor_item,
        themes,
        spacing,
        combo,
        test_popup,
        background,
        file_dialog,
        open_item,
        last_seconds: 0.,
        fps: 0.,
        next_fps: 0.,
    });
    for (item, action) in pending {
        events.subscribe(item.activated()?, move |state: &mut State, event: &Activation| state.activate(action, event))?;
    }
    {
        let state = events.state()?;
        state.style.subscribe(&events)?;
        events.subscribe(neocogi_ui::events::window_events(state.demo.as_com_ref())?, State::window_changed)?;
        events.subscribe(state.entry.submitted(), State::text_submitted)?;
        events.subscribe(state.combo.changed(), State::choice_changed)?;
        if let Some(dialog) = &state.file_dialog {
            events.subscribe(dialog.completed(), State::file_selected)?;
        }
    }
    build_controls(&host, &events, &mut main_body)?;
    build_examples(&host, &events)?;
    Ok((host, events))
}

/// The normal entry point owns the completed tree and session through the concrete demo host.
pub(super) fn run() -> Result<(), HostError> {
    let (host, events) = create()?;
    host.run(events)
}

/// Builds the main scrolling control sampler without duplicating widget state or storing authoring closures.
fn build_controls(host: &Host, events: &EventSession<State>, content: &mut LinearWidget) -> Result<(), HostError> {
    let state = events.state()?;
    let ui = &state.ui;
    let mut info = section(ui, content, "Window Info", DisclosureStyle::Header, false)?;
    for (name, value) in ["Position", "Size", "FPS"].into_iter().zip(&state.info) {
        let mut line = row(ui)?;
        append(&mut line, &label(ui, name)?, LayoutTrack::Fixed { extent: 54 })?;
        append(&mut line, value, FLEX)?;
        append(&mut info, &line, LayoutTrack::Content)?;
    }
    let mut tests = section(ui, content, "Test Buttons", DisclosureStyle::Header, true)?;
    for (name, a, b, action) in [
        ("Buttons", "Button 1", "Button 2", Action::Log),
        ("More", "Button 3", "Popup", Action::Popup),
        ("Dialog", "Button 4", "Dialog", Action::Open),
    ] {
        let mut line = row(ui)?;
        append(&mut line, &label(ui, name)?, LayoutTrack::Fixed { extent: 60 })?;
        append(&mut line, &button(ui, events, a, Action::Log)?, FLEX)?;
        let mut right = button(ui, events, b, action)?;
        if matches!(action, Action::Open) && state.file_dialog.is_none() {
            right.set_options(WidgetOptions::FRAME | WidgetOptions::NO_INTERACT)?;
        }
        append(&mut line, &right, FLEX)?;
        append(&mut tests, &line, LayoutTrack::Content)?;
    }
    let mut combo = section(ui, content, "Combo Box", DisclosureStyle::Header, true)?;
    append(&mut combo, &state.combo, LayoutTrack::Content)?;
    let mut trees = section(ui, content, "Tree and Text", DisclosureStyle::Header, true)?;
    let mut line = row(ui)?;
    let mut left = column(ui)?;
    let mut test = section(ui, &mut left, "Test 1", DisclosureStyle::Tree, false)?;
    let mut nested = section(ui, &mut test, "Test 1a", DisclosureStyle::Tree, false)?;
    append(&mut nested, &label(ui, "Hello")?, LayoutTrack::Content)?;
    append(&mut nested, &label(ui, "World")?, LayoutTrack::Content)?;
    let mut nested = section(ui, &mut test, "Test 1b", DisclosureStyle::Tree, false)?;
    for text in ["Button 1", "Button 2"] {
        append(&mut nested, &button(ui, events, text, Action::Log)?, LayoutTrack::Content)?;
    }
    let mut test = section(ui, &mut left, "Test 2", DisclosureStyle::Tree, false)?;
    for pair in [["Button 3", "Button 4"], ["Button 5", "Button 6"]] {
        let mut line = row(ui)?;
        for text in pair {
            append(&mut line, &button(ui, events, text, Action::Log)?, FLEX)?;
        }
        append(&mut test, &line, LayoutTrack::Content)?;
    }
    let mut test = section(ui, &mut left, "Test 3", DisclosureStyle::Tree, false)?;
    for text in ["Checkbox 1", "Checkbox 2", "Checkbox 3"] {
        append(&mut test, &ui.widgets().checkbox(text, false)?, LayoutTrack::Content)?;
    }
    append(&mut line, &left, LayoutTrack::Fixed { extent: 140 })?;
    append(&mut line, &ui.widgets().text_block("Lorem ipsum dolor sit amet, consectetur adipiscing elit. Maecenas lacinia, sem eu lacinia molestie, mi risus faucibus ipsum, eu varius magna felis a nulla.", TextWrapping::Word)?, FLEX)?;
    append(&mut trees, &line, LayoutTrack::Content)?;
    let mut text = section(ui, content, "TextArea", DisclosureStyle::Header, true)?;
    let editor = ui.widgets().text_area(
        "This is a multi-line TextArea.\nYou can type, scroll, and resize the window.\n\nTry adding more lines to see the scrollbars.",
        TextWrapping::Word,
        true,
    )?;
    append(&mut text, &editor, LayoutTrack::Fixed { extent: 120 })?;
    let mut background = section(ui, content, "Background Color", DisclosureStyle::Header, true)?;
    let mut line = row(ui)?;
    let mut controls = column(ui)?;
    for (name, shift) in [("Red", 0), ("Green", 8), ("Blue", 16)] {
        let mut line = row(ui)?;
        let control = slider(ui, ((state.grid.borrow().background >> shift) & 255) as f32, 255.)?;
        events.subscribe(control.changed(), move |state: &mut State, event: &NumericChanged| {
            state.background_changed(shift, event)
        })?;
        append(&mut line, &label(ui, name)?, LayoutTrack::Fixed { extent: 46 })?;
        append(&mut line, &control, FLEX)?;
        append(&mut controls, &line, LayoutTrack::Content)?;
    }
    append(&mut line, &controls, FLEX)?;
    append(&mut line, &state.background, LayoutTrack::Fixed { extent: 77 })?;
    append(&mut background, &line, LayoutTrack::Content)?;
    let mut textures = section(ui, content, "Textures", DisclosureStyle::Header, true)?;
    let factory = host.renderer.query::<ITextureFactoryAbi>()?;
    for (name, size, color) in [
        ("Texture 1 - Red", Size { width: 64, height: 64 }, [255, 0, 0, 255]),
        ("Texture 2 - Green", Size { width: 24, height: 32 }, [0, 255, 0, 255]),
        ("Texture 3 - Blue", Size { width: 64, height: 24 }, [0, 0, 255, 255]),
    ] {
        let pixels = color.repeat(size.width as usize * size.height as usize);
        let image = factory.create_texture(size, NcCopyFromBufferOffset::new(0, &pixels))?;
        let mut control = button(ui, events, name, Action::Log)?;
        control.set_image(image.as_com_ref(), false)?;
        append(&mut textures, &control, LayoutTrack::Content)?;
    }
    let image = host.png_texture(&neocogi_demo_assets::photo_png()?)?;
    let mut control = button(ui, events, "External Image", Action::Log)?;
    control.set_image(image.as_com_ref(), true)?;
    append(&mut textures, &control, LayoutTrack::Content)?;
    // A deterministic xorshift sample has no random-library or native renderer dependency.
    let mut noise = 0x12345678u32;
    let mut pixels = Vec::with_capacity(24 * 32 * 4);
    for _ in 0..24 * 32 {
        noise ^= noise << 13;
        noise ^= noise >> 17;
        noise ^= noise << 5;
        pixels.extend_from_slice(&[noise as u8, (noise >> 8) as u8, (noise >> 16) as u8, 255]);
    }
    let image = factory.create_texture(Size { width: 24, height: 32 }, NcCopyFromBufferOffset::new(0, &pixels))?;
    let mut control = button(ui, events, "Texture 4 - Noise", Action::Log)?;
    control.set_image(image.as_com_ref(), false)?;
    append(&mut textures, &control, LayoutTrack::Content)?;
    Ok(())
}

/// Mounts the log, typography, layout and custom drawing examples as ordinary structural children.
fn build_examples(host: &Host, events: &EventSession<State>) -> Result<(), HostError> {
    let state = events.state()?;
    let ui = &state.ui;
    let desktop = state.desktop.as_com_ref();
    let mut log = column(ui)?;
    append(&mut log, &state.log_scroll, FLEX)?;
    let mut line = row(ui)?;
    append(&mut line, &state.entry, FLEX)?;
    append(&mut line, &button(ui, events, "Submit", Action::Submit)?, LayoutTrack::Fixed { extent: 69 })?;
    append(&mut log, &line, LayoutTrack::Content)?;
    floating(ui, desktop, "Log Window", rect(350, 40, 300, 200), &log)?;
    let mut typography = column(ui)?;
    let mut heading = label(ui, "NORMAL.ttf at 18px")?;
    heading.set_font_role(FontRole::Heading)?;
    append(&mut typography, &heading, LayoutTrack::Content)?;
    append(
        &mut typography,
        &ui.widgets().text_block(
            "NORMAL.ttf at 12px remains the control font. Window titles use BOLD.ttf, and the log window uses CONSOLE.ttf for input and output.",
            TextWrapping::Word,
        )?,
        LayoutTrack::Content,
    )?;
    append(&mut typography, &button(ui, events, "Control Preview", Action::Log)?, LayoutTrack::Content)?;
    floating(ui, desktop, "Typography Demo", rect(40, 500, 300, 170), &typography)?;
    floating(
        ui,
        desktop,
        "Triangle Window",
        rect(200, 100, 200, 200),
        &ui.widgets().leaf(Triangle::default(), WidgetOptions::NO_INTERACT)?,
    )?;
    floating(
        ui,
        desktop,
        "Painter Window",
        rect(820, 40, 280, 240),
        &ui.widgets().leaf(PainterDemo::default(), WidgetOptions::NONE)?,
    )?;
    floating(
        ui,
        desktop,
        "Brush Falloff",
        rect(820, 300, 320, 260),
        &ui.widgets().leaf(FalloffEditor::default(), WidgetOptions::NONE)?,
    )?;
    floating(
        ui,
        desktop,
        "Suzanne Window",
        rect(220, 220, 300, 300),
        &ui.widgets().leaf(Suzanne::new(host.renderer.as_com_ref())?, WidgetOptions::NONE)?,
    )?;
    let mut stacks = row(ui)?;
    for (name, direction) in [("Top to bottom", FlowDirection::TopToBottom), ("Bottom to top", FlowDirection::BottomToTop)] {
        let mut column = column(ui)?;
        append(&mut column, &label(ui, name)?, LayoutTrack::Content)?;
        let mut stack = ui.widgets().linear(direction, CrossSize::Stretch)?;
        for text in ["First", "Second", "Third"] {
            append(&mut stack, &button(ui, events, text, Action::Log)?, LayoutTrack::Fixed { extent: 28 })?;
        }
        append(&mut column, &stack, LayoutTrack::Fixed { extent: 120 })?;
        append(&mut stacks, &column, FLEX)?;
    }
    floating(ui, desktop, "Stack Direction Demo", rect(530, 40, 280, 220), &stacks)?;
    let mut weights = column(ui)?;
    append(&mut weights, &label(ui, "Row weights: 1 : 2 : 3")?, LayoutTrack::Content)?;
    let mut line = row(ui)?;
    for (text, weight) in [("1x", 1.), ("2x", 2.), ("3x", 3.)] {
        append(&mut line, &button(ui, events, text, Action::Log)?, LayoutTrack::Flex { weight })?;
    }
    append(&mut weights, &line, LayoutTrack::Fixed { extent: 28 })?;
    append(&mut weights, &label(ui, "Grid row weights: 1 : 2")?, LayoutTrack::Content)?;
    let mut grid = ui.widgets().grid(&[FLEX; 3], &[FLEX, LayoutTrack::Flex { weight: 2. }])?;
    for index in 0..6 {
        let control = button(ui, events, &format!("Cell {}", index + 1), Action::Log)?;
        grid.insert(index, &control, GridExtent { columns: 1, rows: 1 })?.finish()?;
    }
    append(&mut weights, &grid, FLEX)?;
    floating(ui, desktop, "Weight Demo", rect(530, 270, 280, 260), &weights)?;
    Ok(())
}

#[cfg(test)]
#[path = "app_tests.rs"]
mod tests;
