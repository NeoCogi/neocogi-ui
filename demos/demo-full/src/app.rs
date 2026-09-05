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
    grid::{GridState, GridWidget},
    mesh::Suzanne,
    painter_demo::PainterDemo,
    style_editor::StyleEditor,
};
use microui_redux::{events::*, *};
use microui_redux_demo_assets::themes::Theme;
use microui_redux_demo_host::{AppState, Host, HostError};
use nc_com::{ComInterface, ComPtr, ComRef, NcCopyFromBufferOffset, NcStatus};
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
pub(super) fn column(ui: &Ui) -> Result<Linear, NcStatus> {
    ui.widgets().linear(FlowDirection::TOP_TO_BOTTOM, CrossSize::Stretch)
}
/// A row derives height from its actual children, so theme/font changes cannot stale a pixel estimate.
pub(super) fn row(ui: &Ui) -> Result<Linear, NcStatus> {
    ui.widgets().linear(FlowDirection::LEFT_TO_RIGHT, CrossSize::Content)
}
/// Appends through the authoritative child count; no second local list or builder closure is retained.
pub(super) fn append<I: ComInterface>(parent: &Linear, child: &Widget<I>, track: LayoutTrack) -> Result<(), NcStatus> {
    parent.insert(parent.children()?.child_count()?, child, LinearEdge::new(track))?.finish()
}
/// Static display text remains a real plugin-owned TextBlock with no application callback.
pub(super) fn label(ui: &Ui, text: &str) -> Result<TextBlock, NcStatus> {
    ui.widgets().text_block(text, TextWrapping::NONE)
}
/// Shared framed, centered numeric control used by the palette and background editors.
pub(super) fn slider(ui: &Ui, value: f32, high: f32) -> Result<Slider, NcStatus> {
    let control = ui.widgets().slider(SliderOptions {
        value,
        low: 0.,
        high,
        step: 1.,
        precision: 0,
    })?;
    control.set_options(WidgetOptions {
        bits: WidgetOptions::FRAME.bits | WidgetOptions::ALIGN_CENTER.bits,
    })?;
    Ok(control)
}
/// Creates and immediately subscribes a concrete button; only its parent needs to retain it afterward.
fn button(ui: &Ui, events: &EventSession<State>, text: &str, action: Action) -> Result<Button, NcStatus> {
    let control = ui.widgets().button(text)?;
    control.set_options(WidgetOptions {
        bits: WidgetOptions::FRAME.bits | WidgetOptions::ALIGN_CENTER.bits,
    })?;
    events.subscribe_activation(&control, action)?;
    Ok(control)
}
/// A disclosure body is its actual editable Linear COM object, with no placeholder replacement tree.
fn section(ui: &Ui, parent: &Linear, text: &str, style: DisclosureStyle, expanded: bool) -> Result<Linear, NcStatus> {
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
    /// A retained runtime projection permits typed handlers to create and install UI values.
    pub(super) ui: Ui,
    /// The selected renderer accepts theme revisions through one transactional capability.
    pub(super) renderer: ComPtr<IRendererAbi>,
    /// Named palette authoring and the actual retained editor controls.
    pub(super) style: StyleEditor,
    /// Background drawing and grid actions share this exact camera/color model.
    grid: Rc<RefCell<GridState>>,
    /// Fullscreen structural parent follows the physical platform drawable extent.
    desktop: ComPtr<IWindowAbi>,
    /// Movable main window can be hidden and restored through the persistent grid menu.
    demo: ComPtr<IWindowAbi>,
    /// Actual sampler scrolling is retained for navigation and complete-frame image verification.
    sample_scroll: ScrollArea,
    /// Position, dimensions and sampled frame-rate presentations belong to this main window.
    info: [TextBlock; 3],
    /// Minimized geometry must not be clamped back to the normal minimum size.
    minimized: bool,
    /// Actual log text is the only stored log buffer; appends read it only when an action writes.
    log: TextBlock,
    /// Scroll requests survive measurement changes caused by log appends and style edits.
    log_scroll: ScrollArea,
    /// Enter and Submit edit and clear this actual single-line control.
    entry: Textbox,
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
    /// Combo header owns the selected index and label.
    combo: Combo,
    /// Actual generic-list popup is coordinated by captured submission/dismissal events.
    combo_popup: ComPtr<IPopupAbi>,
    /// Standalone compact menu opened by the Popup button.
    test_popup: ComPtr<IPopupAbi>,
    /// Color preview reflects the model used to paint the fullscreen background.
    background: ColorSwatch,
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
    /// Successful theme publication precedes every radio/check presentation update.
    fn select_theme(&mut self, selected: Theme) -> Result<(), NcStatus> {
        self.style.select(selected, &self.ui, self.renderer.as_com_ref())?;
        for (theme, item) in &self.themes {
            item.set_mark(MenuMark::Radio { selected: *theme == selected })?;
        }
        self.write_log(&format!("Selected {}", selected.label()))
    }
}
impl AppState for State {
    /// Geometry follows the platform; animations keep fixed intrinsic size and only repaint.
    fn frame(&mut self, _: &Host, size: Size, seconds: f64) -> Result<bool, NcStatus> {
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
    /// Open a standalone compact popup near the main window.
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
    /// Select one application-owned combo value.
    Fruit(usize),
}
impl EventHandler<State, Activation> for Action {
    /// Runs after the UI releases its context, permitting ordinary concrete edits and menu coordination.
    fn handle(&mut self, state: &mut State, event: &Activation) -> Result<(), NcStatus> {
        match *self {
            Self::Log => state.write_log(&format!("Pressed {}", event.label)),
            Self::Submit => state.submit(),
            Self::Popup => {
                let area = state.demo.snapshot()?.rect;
                state.test_popup.show(rect(area.x + 50, area.y + 100, 120, 24))
            }
            Self::Open => state.open_file(),
            Self::New => {
                state.log.clear()?;
                state.entry.clear()?;
                state.write_log("New demo session")
            }
            Self::Clear => state.log.clear(),
            Self::AutoScroll => {
                state.auto_scroll = !state.auto_scroll;
                state.auto_item.set_mark(MenuMark::Checked { checked: state.auto_scroll })?;
                if state.auto_scroll {
                    state.log_scroll.scroll_to_end()?;
                }
                Ok(())
            }
            Self::Theme(theme) => state.select_theme(theme),
            Self::Spacing(comfortable) => {
                state.style.spacing(if comfortable { 8 } else { 2 }, &state.ui, state.renderer.as_com_ref())?;
                state.spacing[0].set_mark(MenuMark::Radio { selected: comfortable })?;
                state.spacing[1].set_mark(MenuMark::Radio { selected: !comfortable })
            }
            Self::ResetGrid => {
                state.grid.borrow_mut().view = super::grid::create_grid_view_3d();
                Ok(())
            }
            Self::MinorLines => {
                let checked = {
                    let mut grid = state.grid.borrow_mut();
                    grid.minor_lines = !grid.minor_lines;
                    grid.minor_lines
                };
                state.minor_item.set_mark(MenuMark::Checked { checked })
            }
            Self::ShowDemo => {
                state.demo.set_visibility(Visibility::VISIBLE)?;
                state.demo.raise()
            }
            Self::ShowTextures => state.sample_scroll.scroll_to_end(),
            Self::About => state.write_log("microui-redux COM demo: concrete widgets, typed events, shared layout, PNG images and renderer-owned 3D scenes."),
            Self::Fruit(index) => {
                state.combo.select(index, &FRUIT)?;
                state.combo_popup.hide()?;
                state.write_log(&format!("Selected {}", FRUIT[index]))
            }
        }
    }
}

/// Stateless concrete observer coordinates typed component events through the application's owned capabilities.
struct Observe;
impl EventHandler<State, TextSubmitted> for Observe {
    /// Enter uses the captured submitted text, which may differ from a later silent edit.
    fn handle(&mut self, state: &mut State, event: &TextSubmitted) -> Result<(), NcStatus> {
        state.write_log(&event.text)?;
        state.entry.clear()
    }
}
impl EventHandler<State, ComboSubmitted> for Observe {
    /// Popup placement uses the exact routed screen-space header anchor.
    fn handle(&mut self, state: &mut State, event: &ComboSubmitted) -> Result<(), NcStatus> {
        if event.open {
            state.combo_popup.show(event.anchor)
        } else {
            state.combo_popup.hide()
        }
    }
}
impl EventHandler<State, PopupDismissed> for Observe {
    /// Dismissal synchronizes the header without synthesizing another selection event.
    fn handle(&mut self, state: &mut State, _: &PopupDismissed) -> Result<(), NcStatus> {
        state.combo.set_open(false)
    }
}
impl EventHandler<State, FileDialogCompleted> for Observe {
    /// Accepted paths remain native values until explicitly formatted for this human-readable log.
    fn handle(&mut self, state: &mut State, event: &FileDialogCompleted) -> Result<(), NcStatus> {
        state.open_item.set_enabled(true)?;
        match event {
            FileDialogCompleted::Accepted(path) => state.write_log(&format!("Selected file: {}", path.display())),
            FileDialogCompleted::Cancelled => state.write_log("File dialog cancelled"),
        }
    }
}
impl EventHandler<State, WindowEvent> for Observe {
    /// Preserves manager-owned minimize/maximize/restore rectangles and clamps only ordinary geometry.
    fn handle(&mut self, state: &mut State, event: &WindowEvent) -> Result<(), NcStatus> {
        let (mut area, clamp) = match *event {
            WindowEvent::GeometryChanged { rect } => (rect, !state.minimized),
            WindowEvent::Minimized { .. } => {
                state.minimized = true;
                return state.write_log("Minimized Demo Window");
            }
            WindowEvent::Maximized { rect } | WindowEvent::Restored { rect } => {
                state.minimized = false;
                (rect, false)
            }
            WindowEvent::CloseRequested => return state.write_log("Closed Demo Window; use Grid > Show Demo Window to restore it"),
            WindowEvent::Hidden | WindowEvent::Destroyed => return Ok(()),
        };
        if clamp && (area.width < 240 || area.height < 300) {
            area.width = area.width.max(240);
            area.height = area.height.max(300);
            state.demo.set_rect(area)?;
        }
        state.info[0].set_text(&format!("{}, {}", area.x, area.y))?;
        state.info[1].set_text(&format!("{}, {}", area.width, area.height))
    }
}

/// One explicit RGB channel edits the same model consumed by the grid's observational paint.
struct Background {
    /// Channel offset in the documented packed-RGBA value, limited by construction to 0, 8 or 16.
    shift: u32,
}
impl EventHandler<State, NumericChanged> for Background {
    /// Paint-only external state reports NoChange; its intrinsic dimensions do not depend on color.
    fn handle(&mut self, state: &mut State, event: &NumericChanged) -> Result<(), NcStatus> {
        let value = {
            let mut grid = state.grid.borrow_mut();
            grid.background = (grid.background & !(255 << self.shift)) | ((event.value as u32) << self.shift);
            grid.background
        };
        state.background.set_fill(value)
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
        self.angle = microui_redux_demo_host::monotonic_seconds() as f32;
        Ok(GeometryChange::NO_CHANGE)
    }
    /// Rotates three colored points around the committed viewport center.
    fn paint(&self, state: WidgetPaint, painter: &Painter<'_>) -> Result<(), NcStatus> {
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
    let host = Host::new(
        "microui-redux COM Demo",
        Size { width: 1200, height: 800 },
        microui_redux_demo_assets::load_bundle()?,
    )?;
    let ui = &host.ui;
    let grid = Rc::new(RefCell::new(GridState::default()));
    let grid_widget = ui.widgets().leaf(GridWidget { data: grid.clone() }, WidgetOptions::NONE)?;
    let desktop = ui.window("X-Y Grid Surface", rect(0, 0, 1200, 800), &grid_widget)?;
    desktop.set_layer(0)?;
    desktop.set_child_clip(ChildClip::CONTENT)?;
    desktop.set_options(WindowOptions::NO_TITLE | WindowOptions::NO_CLOSE | WindowOptions::NO_RESIZE | WindowOptions::NO_PADDING)?;
    let main_body = column(ui)?;
    let main_scroll = ui.widgets().scroll_area(&main_body, true)?;
    let demo = floating(ui, desktop.as_com_ref(), "Demo Window", rect(40, 40, 300, 450), &main_scroll)?;
    demo.set_options(WindowOptions::FRAME | WindowOptions::MINIMIZE_BUTTON | WindowOptions::MAXIMIZE_BUTTON)?;
    let info = [label(ui, "40, 40")?, label(ui, "300, 450")?, label(ui, "0.0")?];
    let combo = ui.widgets().combo()?;
    combo.update_items(&FRUIT)?;
    let combo_content = column(ui)?;
    let combo_popup = ui.popup(demo.as_com_ref(), "Combo Box Popup", &combo_content)?;
    combo_popup.set_options(WindowOptions::FRAME | WindowOptions::AUTO_HEIGHT | WindowOptions::NO_RESIZE | WindowOptions::NO_TITLE)?;
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
    let log = label(ui, "Welcome to microui-redux.")?;
    log.set_font_role(TextFontRole::MONO)?;
    let log_scroll = ui.widgets().scroll_area(&log, true)?;
    let entry = ui.widgets().textbox("")?;
    entry.set_font_role(TextFontRole::MONO)?;
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
        renderer: host.renderer.clone(),
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
        combo_popup,
        test_popup,
        background,
        file_dialog,
        open_item,
        last_seconds: 0.,
        fps: 0.,
        next_fps: 0.,
    });
    for (item, action) in pending {
        events.subscribe_menu_item(&item, action)?;
    }
    {
        let state = events.state()?;
        state.style.subscribe(&events)?;
        events.subscribe_window(state.demo.as_com_ref(), Observe)?;
        events.subscribe_text_submitted(&state.entry, Observe)?;
        events.subscribe_combo_popup(&state.combo, Observe)?;
        events.subscribe_popup(state.combo_popup.as_com_ref(), Observe)?;
        if let Some(dialog) = &state.file_dialog {
            events.subscribe_file_dialog(dialog, Observe)?;
        }
    }
    for (index, text) in FRUIT.into_iter().enumerate() {
        let item = ui.widgets().list_item(text)?;
        events.subscribe_activation(&item, Action::Fruit(index))?;
        append(&combo_content, &item, LayoutTrack::Content)?;
    }
    build_controls(&host, &events, &main_body)?;
    build_examples(&host, &events)?;
    Ok((host, events))
}

/// The normal entry point owns the completed tree and session through the concrete demo host.
pub(super) fn run() -> Result<(), HostError> {
    let (host, events) = create()?;
    host.run(events)
}

/// Builds the main scrolling control sampler without duplicating widget state or storing authoring closures.
fn build_controls(host: &Host, events: &EventSession<State>, content: &Linear) -> Result<(), HostError> {
    let state = events.state()?;
    let ui = &state.ui;
    let info = section(ui, content, "Window Info", DisclosureStyle::HEADER, false)?;
    for (name, value) in ["Position", "Size", "FPS"].into_iter().zip(&state.info) {
        let line = row(ui)?;
        append(&line, &label(ui, name)?, LayoutTrack::Fixed { extent: 54 })?;
        append(&line, value, FLEX)?;
        append(&info, &line, LayoutTrack::Content)?;
    }
    let tests = section(ui, content, "Test Buttons", DisclosureStyle::HEADER, true)?;
    for (name, a, b, action) in [
        ("Buttons", "Button 1", "Button 2", Action::Log),
        ("More", "Button 3", "Popup", Action::Popup),
        ("Dialog", "Button 4", "Dialog", Action::Open),
    ] {
        let line = row(ui)?;
        append(&line, &label(ui, name)?, LayoutTrack::Fixed { extent: 60 })?;
        append(&line, &button(ui, events, a, Action::Log)?, FLEX)?;
        let right = button(ui, events, b, action)?;
        if matches!(action, Action::Open) && state.file_dialog.is_none() {
            right.set_options(WidgetOptions {
                bits: WidgetOptions::FRAME.bits | WidgetOptions::NO_INTERACT.bits,
            })?;
        }
        append(&line, &right, FLEX)?;
        append(&tests, &line, LayoutTrack::Content)?;
    }
    let combo = section(ui, content, "Combo Box", DisclosureStyle::HEADER, true)?;
    append(&combo, &state.combo, LayoutTrack::Content)?;
    let trees = section(ui, content, "Tree and Text", DisclosureStyle::HEADER, true)?;
    let line = row(ui)?;
    let left = column(ui)?;
    let test = section(ui, &left, "Test 1", DisclosureStyle::TREE, false)?;
    let nested = section(ui, &test, "Test 1a", DisclosureStyle::TREE, false)?;
    append(&nested, &label(ui, "Hello")?, LayoutTrack::Content)?;
    append(&nested, &label(ui, "World")?, LayoutTrack::Content)?;
    let nested = section(ui, &test, "Test 1b", DisclosureStyle::TREE, false)?;
    for text in ["Button 1", "Button 2"] {
        append(&nested, &button(ui, events, text, Action::Log)?, LayoutTrack::Content)?;
    }
    let test = section(ui, &left, "Test 2", DisclosureStyle::TREE, false)?;
    for pair in [["Button 3", "Button 4"], ["Button 5", "Button 6"]] {
        let line = row(ui)?;
        for text in pair {
            append(&line, &button(ui, events, text, Action::Log)?, FLEX)?;
        }
        append(&test, &line, LayoutTrack::Content)?;
    }
    let test = section(ui, &left, "Test 3", DisclosureStyle::TREE, false)?;
    for text in ["Checkbox 1", "Checkbox 2", "Checkbox 3"] {
        append(&test, &ui.widgets().checkbox(text, false)?, LayoutTrack::Content)?;
    }
    append(&line, &left, LayoutTrack::Fixed { extent: 140 })?;
    append(&line, &ui.widgets().text_block("Lorem ipsum dolor sit amet, consectetur adipiscing elit. Maecenas lacinia, sem eu lacinia molestie, mi risus faucibus ipsum, eu varius magna felis a nulla.", TextWrapping::WORD)?, FLEX)?;
    append(&trees, &line, LayoutTrack::Content)?;
    let text = section(ui, content, "TextArea", DisclosureStyle::HEADER, true)?;
    let editor = ui.widgets().text_area(
        "This is a multi-line TextArea.\nYou can type, scroll, and resize the window.\n\nTry adding more lines to see the scrollbars.",
        TextWrapping::WORD,
        true,
    )?;
    append(&text, &editor, LayoutTrack::Fixed { extent: 120 })?;
    let background = section(ui, content, "Background Color", DisclosureStyle::HEADER, true)?;
    let line = row(ui)?;
    let controls = column(ui)?;
    for (name, shift) in [("Red", 0), ("Green", 8), ("Blue", 16)] {
        let line = row(ui)?;
        let control = slider(ui, ((state.grid.borrow().background >> shift) & 255) as f32, 255.)?;
        events.subscribe_numeric(&control, Background { shift })?;
        append(&line, &label(ui, name)?, LayoutTrack::Fixed { extent: 46 })?;
        append(&line, &control, FLEX)?;
        append(&controls, &line, LayoutTrack::Content)?;
    }
    append(&line, &controls, FLEX)?;
    append(&line, &state.background, LayoutTrack::Fixed { extent: 77 })?;
    append(&background, &line, LayoutTrack::Content)?;
    let textures = section(ui, content, "Textures", DisclosureStyle::HEADER, true)?;
    let factory = host.renderer.query::<ITextureFactoryAbi>()?;
    for (name, size, color) in [
        ("Texture 1 - Red", Size { width: 64, height: 64 }, [255, 0, 0, 255]),
        ("Texture 2 - Green", Size { width: 24, height: 32 }, [0, 255, 0, 255]),
        ("Texture 3 - Blue", Size { width: 64, height: 24 }, [0, 0, 255, 255]),
    ] {
        let pixels = color.repeat(size.width as usize * size.height as usize);
        let image = factory.create_texture(size, NcCopyFromBufferOffset::new(0, &pixels))?;
        let control = button(ui, events, name, Action::Log)?;
        control.set_image(image.as_com_ref(), false)?;
        append(&textures, &control, LayoutTrack::Content)?;
    }
    let image = host.png_texture(microui_redux_demo_assets::FACEPALM_PNG)?;
    let control = button(ui, events, "External Image", Action::Log)?;
    control.set_image(image.as_com_ref(), true)?;
    append(&textures, &control, LayoutTrack::Content)?;
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
    let control = button(ui, events, "Texture 4 - Noise", Action::Log)?;
    control.set_image(image.as_com_ref(), false)?;
    append(&textures, &control, LayoutTrack::Content)?;
    Ok(())
}

/// Mounts the log, typography, layout and custom drawing examples as ordinary structural children.
fn build_examples(host: &Host, events: &EventSession<State>) -> Result<(), HostError> {
    let state = events.state()?;
    let ui = &state.ui;
    let desktop = state.desktop.as_com_ref();
    let log = column(ui)?;
    append(&log, &state.log_scroll, FLEX)?;
    let line = row(ui)?;
    append(&line, &state.entry, FLEX)?;
    append(&line, &button(ui, events, "Submit", Action::Submit)?, LayoutTrack::Fixed { extent: 69 })?;
    append(&log, &line, LayoutTrack::Content)?;
    floating(ui, desktop, "Log Window", rect(350, 40, 300, 200), &log)?;
    let typography = column(ui)?;
    let heading = label(ui, "NORMAL.ttf at 18px")?;
    heading.set_font_role(TextFontRole::HEADING)?;
    append(&typography, &heading, LayoutTrack::Content)?;
    append(
        &typography,
        &ui.widgets().text_block(
            "NORMAL.ttf at 12px remains the control font. Window titles use BOLD.ttf, and the log window uses CONSOLE.ttf for input and output.",
            TextWrapping::WORD,
        )?,
        LayoutTrack::Content,
    )?;
    append(&typography, &button(ui, events, "Control Preview", Action::Log)?, LayoutTrack::Content)?;
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
    let stacks = row(ui)?;
    for (name, direction) in [("Top to bottom", FlowDirection::TOP_TO_BOTTOM), ("Bottom to top", FlowDirection::BOTTOM_TO_TOP)] {
        let column = column(ui)?;
        append(&column, &label(ui, name)?, LayoutTrack::Content)?;
        let stack = ui.widgets().linear(direction, CrossSize::Stretch)?;
        for text in ["First", "Second", "Third"] {
            append(&stack, &button(ui, events, text, Action::Log)?, LayoutTrack::Fixed { extent: 28 })?;
        }
        append(&column, &stack, LayoutTrack::Fixed { extent: 120 })?;
        append(&stacks, &column, FLEX)?;
    }
    floating(ui, desktop, "Stack Direction Demo", rect(530, 40, 280, 220), &stacks)?;
    let weights = column(ui)?;
    append(&weights, &label(ui, "Row weights: 1 : 2 : 3")?, LayoutTrack::Content)?;
    let line = row(ui)?;
    for (text, weight) in [("1x", 1.), ("2x", 2.), ("3x", 3.)] {
        append(&line, &button(ui, events, text, Action::Log)?, LayoutTrack::Flex { weight })?;
    }
    append(&weights, &line, LayoutTrack::Fixed { extent: 28 })?;
    append(&weights, &label(ui, "Grid row weights: 1 : 2")?, LayoutTrack::Content)?;
    let grid = ui.widgets().grid(&[FLEX; 3], &[FLEX, LayoutTrack::Flex { weight: 2. }])?;
    for index in 0..6 {
        let control = button(ui, events, &format!("Cell {}", index + 1), Action::Log)?;
        grid.insert(index, &control, GridExtent { columns: 1, rows: 1 })?.finish()?;
    }
    append(&weights, &grid, FLEX)?;
    floating(ui, desktop, "Weight Demo", rect(530, 270, 280, 260), &weights)?;
    Ok(())
}

#[cfg(test)]
#[path = "app_tests.rs"]
mod tests;
