//! Full application checks use the actual plugin graph, including GPU images and all theme choices.
use super::*;
use neocogi_ui::events::testing::TestApp;

/// Exercise the same leaf constructor used by both scene viewports through actual UI routing.
#[cfg(not(target_arch = "wasm32"))]
#[test]
#[ignore = "requires the packaged native plugin graph and a native GPU window"]
fn packaged_full_viewport_wheel_routing() {
    /// Owns observations in a channel because the runtime exclusively owns the mounted content.
    struct WheelProbe(std::sync::mpsc::Sender<Point>);
    impl LeafContent for WheelProbe {
        /// Keep the leaf smaller than its window so layout cannot hide its hit target.
        fn measure(&self, _: LayoutLimits, _: ComRef<'_, ITextMetricsAbi>) -> Result<Size, NcStatus> {
            Ok(Size { width: 80, height: 24 })
        }
        /// Observe only scroll events admitted by the production pointer router and COM leaf.
        fn update(&mut self, state: WidgetUpdate, event: WidgetEvent, _: &str) -> Result<GeometryChange, NcStatus> {
            if let WidgetEvent::Scroll { delta, .. } = event {
                assert!(!state.focused, "hovered wheel routing must not require keyboard focus");
                self.0.send(delta).unwrap();
            }
            Ok(GeometryChange::NoChange)
        }
        /// Routing assertions require no fixture-specific rendering operations.
        fn paint(&mut self, _: WidgetPaint, _: &mut Painter<'_>) -> Result<(), NcStatus> {
            Ok(())
        }
    }
    let (host, _state, _events) = create(crate::host_runtime()).unwrap();
    let ui = &host.ui;
    let size = host.window.size().unwrap();
    let (send, received) = std::sync::mpsc::channel();
    let leaf = viewport(ui, WheelProbe(send)).unwrap();
    let mut observed = leaf.handle();
    let window = ui.window("Wheel routing", rect(10, 10, 200, 100), leaf).unwrap();
    window.set_options(WindowOptions::NO_TITLE | WindowOptions::NO_PADDING | WindowOptions::NO_RESIZE).unwrap();
    window.raise().unwrap();
    ui.update(size).unwrap();
    ui.input(UiInput::Pointer(Point { x: 30, y: 30 }), "").unwrap();
    for delta in [Point { x: 0, y: -30 }, Point { x: 0, y: 30 }, Point { x: 1, y: 0 }] {
        ui.input(UiInput::Scroll(delta), "").unwrap();
        ui.update(size).unwrap();
        assert_eq!(received.try_recv().unwrap(), delta);
        assert!(received.try_recv().is_err(), "one wheel event is delivered once");
    }
    // Removing the routing opt-in reproduces the original defect; restoring it recovers delivery.
    observed.clear_flags(WidgetOptions::GRAB_SCROLL).unwrap();
    ui.input(UiInput::Scroll(Point { x: 0, y: 30 }), "").unwrap();
    ui.update(size).unwrap();
    assert!(received.try_recv().is_err());
    observed.set_flags(WidgetOptions::GRAB_SCROLL).unwrap();
    ui.input(UiInput::Scroll(Point { x: 0, y: 1 }), "").unwrap();
    ui.update(size).unwrap();
    assert_eq!(received.try_recv().unwrap(), Point { x: 0, y: 1 });
    // Neither zero deltas nor a pointer outside the visible leaf may reach its scroll handler.
    ui.input(UiInput::Scroll(Point::default()), "").unwrap();
    ui.input(UiInput::Pointer(Point { x: 240, y: 140 }), "").unwrap();
    ui.input(UiInput::Scroll(Point { x: 0, y: 30 }), "").unwrap();
    ui.update(size).unwrap();
    assert!(received.try_recv().is_err());
    window.close().unwrap();
}

/// Compiles shipped themes through the deployed services without requiring a GPU window.
#[cfg(not(target_arch = "wasm32"))]
#[test]
#[ignore = "requires the packaged native plugin graph and adjacent theme assets"]
fn packaged_full_theme_loaders() {
    neocogi_demo_host::initialize(crate::host_runtime()).unwrap();
    let base = neocogi_demo_assets::load_bundle(crate::host_runtime()).unwrap();
    for theme in Theme::ALL {
        let loaded = theme
            .load(crate::host_runtime(), base.as_com_ref())
            .unwrap_or_else(|status| panic!("{} theme load failed: {status}", theme.label()));
        loaded.metrics().unwrap();
    }
}

/// Runs selection under the production host, including platform polling and native IME placement.
#[cfg(not(target_arch = "wasm32"))]
#[test]
#[ignore = "requires the packaged native plugin graph and a native GPU window"]
fn packaged_full_host_selects_win311() {
    /// Ordered input stages; the press remains held through installation on the release frame.
    #[derive(Clone, Copy)]
    enum Step {
        /// Bring the demo above its sibling windows after initial construction.
        Raise,
        /// Focus its title after the first frame has committed layout.
        Focus,
        /// Open the View menu using the default theme's committed geometry.
        OpenView,
        /// Open View's nested Theme menu.
        OpenTheme,
        /// Press Windows 3.11; leave the mouse held at the end of this frame.
        PressTheme,
        /// Release on the frame that installs the theme prepared by the preceding dispatch.
        ReleaseTheme,
        /// Exercise another full platform/IME/render iteration after installation.
        RenderInstalled,
        /// Check the committed selection and stop the native loop.
        Verify,
    }
    /// Owns the actual application and advances one pointer selection across frames.
    struct Selection {
        /// All controls and application behavior use the production state.
        state: State,
        /// A finite forward-only sequence ends by returning false from Verify.
        steps: std::array::IntoIter<Step, 8>,
    }
    impl AppState for Selection {
        type Message = neocogi_ui::events::EventMessage;
        /// Preserve the production application's queued COM event handling.
        fn update(&mut self, message: Self::Message) -> Result<(), NcStatus> {
            self.state.update(message)
        }
        /// Inject only through this window's UI interface, then run the actual frame callback.
        fn frame(&mut self, host: &Host, size: Size, seconds: f64) -> Result<bool, NcStatus> {
            let step = self.steps.next().expect("Verify must terminate the host loop");
            match step {
                Step::Raise => self.state.demo.raise()?,
                Step::Focus | Step::OpenView | Step::OpenTheme | Step::PressTheme | Step::ReleaseTheme => {
                    let area = self.state.demo.snapshot()?.rect;
                    // Coordinates belong to the shipped default theme: 24-pixel title,
                    // 28-pixel menu rows, and View's fourth row opens the Theme submenu.
                    let (x, y) = match step {
                        Step::Focus => (100, 10),
                        Step::OpenView => (45, 39),
                        Step::OpenTheme => (50, 152),
                        Step::PressTheme | Step::ReleaseTheme => (205, 182),
                        _ => unreachable!(),
                    };
                    let position = Point { x: area.x + x, y: area.y + y };
                    host.ui.input(UiInput::Pointer(position), "")?;
                    if !matches!(step, Step::ReleaseTheme) {
                        host.ui.input(UiInput::Down { position, buttons: 1 }, "")?;
                    }
                    if !matches!(step, Step::PressTheme) {
                        host.ui.input(UiInput::Up { position, buttons: 1 }, "")?;
                    }
                }
                Step::RenderInstalled => {}
                Step::Verify => {
                    assert_eq!(self.state.themes[1].1.mark()?, MenuMark::Radio { selected: true });
                    return Ok(false);
                }
            }
            self.state.frame(host, size, seconds)
        }
    }
    let (host, state, events) = create(crate::host_runtime()).unwrap();
    let steps = [
        Step::Raise,
        Step::Focus,
        Step::OpenView,
        Step::OpenTheme,
        Step::PressTheme,
        Step::ReleaseTheme,
        Step::RenderInstalled,
        Step::Verify,
    ]
    .into_iter();
    host.run(Selection { state, steps }, events).unwrap();
}

/// Checks phase attribution and the original error chain through the actual native driver.
#[cfg(not(target_arch = "wasm32"))]
#[test]
#[ignore = "requires the packaged native plugin graph and a native GPU window"]
fn packaged_full_host_preserves_rejecting_operation() {
    /// Each injected rejection belongs to a distinct production driver boundary.
    #[derive(Clone, Copy)]
    enum Failure {
        /// Direct application preparation before event dispatch.
        Preparation,
        /// Pending changes invoked through the UI's real COM application dispatcher.
        Dispatch,
        /// Application work after successful dispatch and layout.
        AfterUpdate,
        /// IME rectangle selection after the frame updates have completed.
        TextArea,
    }
    impl Failure {
        /// Expected context belongs to the host operation that invokes this hook.
        fn operation(self) -> &'static str {
            match self {
                Self::Preparation => "application frame preparation",
                Self::Dispatch => "event and layout update",
                Self::AfterUpdate => "application frame update",
                Self::TextArea => "text input placement",
            }
        }
    }
    /// Keeps production widget ownership and callbacks, rejecting only the selected hook.
    struct Reject {
        /// Owns every real application control until host teardown.
        state: State,
        /// The sole hook allowed to produce this test's NC_E_INVALID_ARG.
        failure: Failure,
    }
    impl AppState for Reject {
        type Message = neocogi_ui::events::EventMessage;
        /// Preserve ordinary event handling for hooks that are not under test.
        fn update(&mut self, message: Self::Message) -> Result<(), NcStatus> {
            self.state.update(message)
        }
        /// Direct frame rejection retains its status before any UI update.
        fn frame(&mut self, host: &Host, size: Size, seconds: f64) -> Result<bool, NcStatus> {
            match self.failure {
                Failure::Preparation => Err(NC_E_INVALID_ARG),
                _ => self.state.frame(host, size, seconds),
            }
        }
        /// Dispatch failure crosses the generated checked COM interface and returns to the host.
        fn apply_pending_changes(&mut self, host: &Host, size: Size) -> Result<ApplyOutcome, NcStatus> {
            match self.failure {
                Failure::Dispatch => Err(NC_E_INVALID_ARG),
                _ => self.state.apply_pending_changes(host, size),
            }
        }
        /// Post-layout rejection is distinguishable from the preceding COM dispatch failure.
        fn after_update(&mut self, host: &Host, size: Size) -> Result<(), NcStatus> {
            match self.failure {
                Failure::AfterUpdate => Err(NC_E_INVALID_ARG),
                _ => self.state.after_update(host, size),
            }
        }
        /// IME preparation keeps its context instead of masquerading as a renderer failure.
        fn text_input_area(&self, host: &Host) -> Result<TextInputArea, NcStatus> {
            match self.failure {
                Failure::TextArea => Err(NC_E_INVALID_ARG),
                _ => self.state.text_input_area(host),
            }
        }
    }
    for failure in [Failure::Preparation, Failure::Dispatch, Failure::AfterUpdate, Failure::TextArea] {
        let (host, state, events) = create(crate::host_runtime()).unwrap();
        let error = host
            .run(Reject { state, failure }, events)
            .expect_err("the selected hook must reject this frame");
        assert!(
            matches!(error, HostError::Operation { operation, status, .. } if operation == failure.operation() && status == NC_E_INVALID_ARG),
            "{error}"
        );
        assert_eq!(error.to_string(), format!("{} failed: {NC_E_INVALID_ARG}", failure.operation()));
        assert_eq!(std::error::Error::source(&error).unwrap().to_string(), NC_E_INVALID_ARG.to_string());
    }
}

/// Renders without exhausting the browser's finite streamed-buffer ring in one JavaScript task.
/// Native calls remain synchronous; browsers retire GPU fences between event-loop tasks.
async fn render_frame(host: &Host, size: Size) -> Result<(), HostError> {
    #[cfg(not(target_arch = "wasm32"))]
    {
        host.ui.render(size, host.renderer.as_com_ref()).map_err(Into::into)
    }

    #[cfg(target_arch = "wasm32")]
    {
        for attempt in 0..120 {
            // Yield before rendering: the caller must inspect the final framebuffer before the
            // browser can discard its unpreserved contents at the next animation boundary.
            browser::next_task().await?;
            match host.ui.render(size, host.renderer.as_com_ref()) {
                Ok(()) => return Ok(()),
                Err(error) if error.status == neocogi_render_core::abi::RENDER_E_BUSY && attempt < 119 => {},
                Err(error) => return Err(error.into()),
            }
        }
        unreachable!("the final attempt returns either the rendered frame or its original error")
    }
}

/// Selects Win 3.11 through the real nested menu and verifies publication on the following frame.
async fn select_win311_from_menu(host: &Host, app: &TestApp<State, neocogi_ui::events::EventMessage>) -> Result<(), HostError> {
    let mut state = app.state();
    let size = host.window.size()?;
    let base = host.ui.assets()?;
    let demo = state.demo.clone();
    demo.raise()?;
    state.frame(host, size, 0.)?;
    drop(state);
    app.update(&host.ui, size)?;
    state = app.state();
    let installed = host.ui.assets()?;
    let area = demo.snapshot()?.rect;
    let position = Point { x: area.x + 100, y: area.y + 10 };
    for event in [
        UiInput::Pointer(position),
        UiInput::Down { position, buttons: 1 },
        UiInput::Up { position, buttons: 1 },
    ] {
        host.ui.input(event, "")?;
    }
    // Focus the main window, open View -> Theme and activate its second radio item.
    for key in [
        KeyCode::Function,
        KeyCode::ArrowRight,
        KeyCode::ArrowDown,
        KeyCode::ArrowDown,
        KeyCode::ArrowDown,
        KeyCode::ArrowDown,
        KeyCode::ArrowRight,
        KeyCode::ArrowDown,
        KeyCode::Enter,
    ] {
        host.ui.input(
            UiInput::Key(KeyStroke {
                key,
                value: if key == KeyCode::Function { 10 } else { 0 },
                pressed: true,
                repeat: false,
                modifiers: 0,
            }),
            "",
        )?;
    }
    // This frame's callback precedes the input handlers. Selecting the theme prepares its assets,
    // but the current revision remains renderable after the single event/layout update completes.
    state.frame(host, size, 0.)?;
    drop(state);
    app.update(&host.ui, size)?;
    state = app.state();
    assert_eq!(state.themes[1].1.mark()?, MenuMark::Radio { selected: true });
    assert_eq!(state.ui.assets()?.metrics()?, installed.metrics()?);
    drop(state);
    render_frame(host, size).await?;
    state = app.state();
    // The following frame publishes the candidate and settles its geometry with one update too.
    state.frame(host, size, 0.)?;
    drop(state);
    app.update(&host.ui, size)?;
    state = app.state();
    assert_eq!(
        state.ui.assets()?.metrics()?,
        Theme::ALL[1].load(crate::host_runtime(), base.as_com_ref())?.metrics()?
    );
    drop(state);
    render_frame(host, size).await?;
    Ok(())
}

/// Construction helpers drop rejected candidates while preserving existing children.
/// Both parent storage forms must use the same real generated insertion boundary.
fn construction_rejection_checks(ui: &Ui) -> Result<(), NcStatus> {
    let mut parent = column(ui)?;
    let mut observed_parent = parent.handle();
    let child = label(ui, "retained child")?;
    let observed_child = child.handle();
    append(&mut parent, child, LayoutTrack::Content)?;

    let candidate = label(ui, "invalid owner-parent edge")?;
    let candidate_view = candidate.handle();
    assert_eq!(append(&mut parent, candidate, LayoutTrack::Fixed { extent: -1 }), Err(NC_E_INVALID_ARG));
    assert!(!candidate_view.as_interface().is_alive());
    assert_eq!(parent.children()?.child_count()?, 1);
    assert!(observed_child.as_interface().is_alive());

    let candidate = ui.widgets().button_unobserved("invalid weak-parent edge")?;
    let candidate_view = candidate.handle();
    assert_eq!(
        append(&mut observed_parent, candidate, LayoutTrack::Fixed { extent: -1 }),
        Err(NC_E_INVALID_ARG)
    );
    assert!(!candidate_view.as_interface().is_alive());
    assert_eq!(parent.children()?.child_count()?, 1);

    // A successful weak-parent insertion still moves the candidate into the tree.
    let child = ui.widgets().button_unobserved("accepted through weak parent")?;
    let child_view = child.handle();
    append(&mut observed_parent, child, LayoutTrack::Content)?;
    assert_eq!(parent.children()?.child_count()?, 2);
    assert!(child_view.as_interface().is_alive());
    drop(parent);
    assert!(!child_view.as_interface().is_alive());
    assert!(!observed_child.as_interface().is_alive());

    // A failure before insertion, while obtaining the parent's child count, also
    // disposes of the fresh candidate without attempting a reverse projection.
    let candidate = label(ui, "expired parent")?;
    let candidate_view = candidate.handle();
    assert_eq!(append(&mut observed_parent, candidate, LayoutTrack::Content), Err(neocogi_com::NC_E_DROPPED));
    assert!(!candidate_view.as_interface().is_alive());
    Ok(())
}

/// Native and WebGL exercise the same bounded Combo input and resize path.
async fn popup_scrolling_checks(host: &Host, size: Size) -> Result<(), HostError> {
    let column = host.ui.widgets().linear(FlowDirection::TopToBottom, CrossSize::Stretch)?;
    let mut children = column.handle();
    let window = host
        .ui
        .window("Popup scrolling", Rect { x: 600, y: 400, width: 240, height: 180 }, column)
        .map_err(|(status, _candidate)| status)?;
    window.set_options(WindowOptions::NO_TITLE | WindowOptions::NO_PADDING | WindowOptions::NO_RESIZE)?;
    let combo = host.ui.combo_box_unobserved(window.as_com_ref(), &FRUIT, ChoiceSelection::Selected(0))?;
    let mut control = combo.handle();
    assert_eq!(control.max_visible_items()?, 10);
    control.set_max_visible_items(8)?;
    children
        .insert(0, combo, LinearEdge::new(LayoutTrack::Content))
        .map_err(|(status, _candidate)| status)?;
    host.ui.update(size)?;
    // The fresh unframed window has one header at its exact top-left. Route real
    // public pointer/keyboard events, leaving all menu state and fitting to core.
    let header = Point { x: 605, y: 405 };
    for event in [
        UiInput::Pointer(header),
        UiInput::Down { position: header, buttons: 1 },
        UiInput::Up { position: header, buttons: 1 },
    ] {
        host.ui.input(event, "")?;
    }
    host.ui.update(size)?;
    let key = |key| {
        host.ui.input(
            UiInput::Key(KeyStroke {
                key,
                value: 0,
                pressed: true,
                repeat: false,
                modifiers: 0,
            }),
            "",
        )
    };
    key(KeyCode::End)?;
    host.ui.update(size)?;
    render_frame(host, size).await?;
    // Shrink a live popup, then refit after a drawable-layout change; the complete
    // candidate/model remains usable through the exact same generated interfaces.
    control.set_max_visible_items(3)?;
    host.ui.update(Size { width: 300, height: 180 })?;
    host.ui.update(size)?;
    key(KeyCode::Enter)?;
    host.ui.update(size)?;
    assert_eq!(control.selection()?, ChoiceSelection::Selected(15));
    assert_eq!(control.display_text()?, "Tangerine");
    for event in [UiInput::Down { position: header, buttons: 1 }, UiInput::Up { position: header, buttons: 1 }] {
        host.ui.input(event, "")?;
    }
    key(KeyCode::Home)?;
    key(KeyCode::PageDown)?;
    host.ui.input(UiInput::Pointer(Point { x: 610, y: 450 }), "")?;
    host.ui.input(UiInput::Scroll(Point { x: 0, y: 60 }), "")?;
    host.ui.update(size)?;
    render_frame(host, size).await?;
    key(KeyCode::Escape)?;
    host.ui.update(size)?;
    assert_eq!(control.selection()?, ChoiceSelection::Selected(15), "scroll/cancel do not commit a candidate");
    window.close()?;
    host.ui.update(size)?;
    render_frame(host, size).await
}

/// Exercises application-level composition against real runtime, platform and decoder objects.
async fn exercise() -> Result<(Host, TestApp<State, neocogi_ui::events::EventMessage>), HostError> {
    let (host, state, events) = create(crate::host_runtime())?;
    construction_rejection_checks(&host.ui)?;
    let app = TestApp::new(state, events, State::update);
    let size = host.window.size()?;
    let base = host.ui.assets()?;
    select_win311_from_menu(&host, &app).await?;
    let mut state = app.state();
    for theme in Theme::ALL.into_iter().cycle().take(2 * Theme::ALL.len()) {
        state.select_theme(theme)?;
        // Publish the prepared assets before the host's single update for this frame.
        state.frame(&host, size, 0.)?;
        // The photograph is below the initial viewport. Requesting the measured end guarantees
        // every theme crosses real image forwarding, clipping, validation and native GPU upload.
        state.sample_scroll.scroll_to_end()?;
        drop(state);
        app.update(&host.ui, size)?;
        render_frame(&host, size).await?;
        state = app.state();
        assert_eq!(state.ui.assets()?.metrics()?, theme.load(crate::host_runtime(), base.as_com_ref())?.metrics()?);
        for (choice, item) in &state.themes {
            assert_eq!(item.mark()?, MenuMark::Radio { selected: *choice == theme });
        }
        // Keep running after installation: lazy imports and timer-driven text edits must
        // remain renderable after the frame that first publishes a theme revision.
        for frame in 1..=4 {
            state.frame(&host, size, f64::from(frame) / 4.)?;
            drop(state);
            app.update(&host.ui, size)?;
            render_frame(&host, size).await?;
            state = app.state();
        }
        let original = state.demo.snapshot()?.rect;
        for (width, height) in [(240, 300), (400, 300), (640, 480)] {
            state.demo.set_rect(Rect { width, height, ..original })?;
            state.frame(&host, size, 2.)?;
            drop(state);
            app.update(&host.ui, size)?;
            render_frame(&host, size).await?;
            state = app.state();
        }
        state.demo.set_rect(original)?;
    }
    {
        state.select_theme(Theme::Default)?;
        state.activate(Action::Spacing(false))?;
        state.combo.set_selection(ChoiceSelection::Selected(2))?;
        assert_eq!(state.combo.selection()?, ChoiceSelection::Selected(2));
        assert_eq!(state.combo.display_text()?, "Cherry");
        state.entry.set_text("Unicode: 日本語")?;
        state.submit()?;
        assert!(state.entry.text()?.is_empty());
        assert!(state.log.text()?.ends_with("Unicode: 日本語"));
        state.demo.set_visibility(Visibility::Hidden)?;
        state.activate(Action::ShowDemo)?;
        assert_eq!(state.demo.snapshot()?.visibility, Visibility::Visible);
    }
    // Commit the final style edit before observing the installed immutable revision.
    state.frame(&host, size, 0.)?;
    assert_eq!(state.ui.assets()?.metrics()?.spacing, 2);
    drop(state);
    app.update(&host.ui, size)?;
    render_frame(&host, size).await?;
    popup_scrolling_checks(&host, size).await?;
    Ok((host, app))
}

/// Packaging supplies the actual adjacent libraries; ordinary unit runs explicitly report this as ignored.
#[cfg(not(target_arch = "wasm32"))]
#[test]
#[ignore = "run scripts/test-demo.sh to stage the independently built native plugins"]
fn packaged_full_application_themes_images_actions_and_close() {
    let (host, app) = pollster::block_on(exercise()).unwrap();
    let retained = app.state().combo.clone();
    drop(host);
    assert_eq!(retained.selection(), Err(neocogi_com::NC_E_DROPPED));
    drop(app);
}

#[cfg(target_arch = "wasm32")]
mod browser {
    use super::*;
    use std::cell::Cell;
    use wasm_bindgen::prelude::*;

    #[wasm_bindgen(inline_js = "export function nextTask() { return new Promise(resolve => setTimeout(resolve, 0)); }")]
    extern "C" {
        /// Resolves in a new event-loop task, after previous GPU work can retire.
        #[wasm_bindgen(js_name = nextTask)]
        fn next_task_promise() -> js_sys::Promise;
    }

    /// Yields only the test driver; production rendering and resource ownership stay unchanged.
    pub(super) async fn next_task() -> Result<(), HostError> {
        wasm_bindgen_futures::JsFuture::from(next_task_promise())
            .await
            .map(|_| ())
            .map_err(|error| HostError::Message(format!("browser task failed: {error:?}")))
    }

    /// A drop-time callback count shared with the harness without retaining application state.
    type ReleasedFrames = Rc<Cell<Option<u32>>>;

    thread_local! {
        /// Independent observations do not keep the application, its session or its canvas alive.
        static RELEASED: RefCell<Option<[ReleasedFrames; 2]>> = const { RefCell::new(None) };
    }
    /// Concrete finite app checks the host's real RAF ownership paths after the full demo checks.
    struct Stop<Resources> {
        /// Keeps client-owned scene resources alive until the rendering loop itself stops.
        _resources: Resources,
        /// Counts ordinary callbacks before an explicit stop or deliberate frame error.
        frames: u32,
        /// Selects the error path independently of the normal stop path.
        fail: bool,
        /// Test-owned observation records the final callback count without retaining the state.
        released: ReleasedFrames,
    }
    impl<Resources: 'static> AppState for Stop<Resources> {
        /// The teardown fixture has no semantic messages.
        type Message = ();
        /// The successful run renders two frames; the failing run rejects its first callback.
        fn frame(&mut self, _: &Host, _: Size, _: f64) -> Result<bool, NcStatus> {
            self.frames += 1;
            if self.fail { Err(UI_E_RENDER) } else { Ok(self.frames < 3) }
        }
    }
    impl<Resources> Drop for Stop<Resources> {
        /// Final release proves the RAF closure/session cycle was broken at the expected callback.
        fn drop(&mut self) {
            self.released.set(Some(self.frames));
        }
    }
    /// Browser harness starts the real full demo checks followed by both asynchronous teardown paths.
    #[wasm_bindgen]
    pub async fn full_demo_checks() -> Result<(), JsValue> {
        let (host, app) = exercise().await.map_err(|error| JsValue::from_str(&error.to_string()))?;
        let released = [Rc::new(Cell::new(None)), Rc::new(Cell::new(None))];
        RELEASED.with_borrow_mut(|stored| *stored = Some(released.clone()));
        host.run(
            Stop {
                _resources: app,
                frames: 0,
                fail: false,
                released: released[0].clone(),
            },
            std::sync::mpsc::channel().1,
        )
        .map_err(|error| JsValue::from_str(&error.to_string()))?;
        let host = Host::new(
            crate::host_runtime(),
            "Error teardown",
            Size { width: 160, height: 120 },
            neocogi_demo_assets::load_bundle(crate::host_runtime()).map_err(|error| JsValue::from_str(&error.to_string()))?,
        )
        .map_err(|error| JsValue::from_str(&error.to_string()))?;
        host.run(
            Stop {
                _resources: (),
                frames: 0,
                fail: true,
                released: released[1].clone(),
            },
            std::sync::mpsc::channel().1,
        )
        .map_err(|error| JsValue::from_str(&error.to_string()))
    }
    /// Exercises the real binary reporting boundary with a deliberately failed startup result.
    /// The JavaScript fixture observes console output and the returned exit status; it does not
    /// replace initialization, a runtime, a provider, or an application implementation.
    #[wasm_bindgen]
    pub fn report_failed_startup() -> bool {
        neocogi_demo_host::report_exit(Err(HostError::Status(neocogi_com::NC_E_NOT_FOUND))) == std::process::ExitCode::FAILURE
    }
    /// The HTML harness also checks that both platform-owned canvases and input textareas disappeared.
    #[wasm_bindgen]
    pub fn demo_loops_released() -> bool {
        RELEASED.with_borrow(|stored| {
            stored.as_ref().is_some_and(|counts| counts[0].get() == Some(3) && counts[1].get() == Some(1))
        })
    }
    /// Diagnoses early render failures separately from a loop that still owns its state.
    #[wasm_bindgen]
    pub fn demo_loop_frames() -> String {
        RELEASED.with_borrow(|stored| match stored {
            Some(counts) => format!("normal={:?}, error={:?}", counts[0].get(), counts[1].get()),
            None => "loops not started".to_owned(),
        })
    }
}
