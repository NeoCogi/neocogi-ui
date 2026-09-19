//! Full application checks use the actual plugin graph, including GPU images and all theme choices.
use super::*;

/// Selects Win 3.11 through the real nested menu and event session, rather than calling its handler directly.
fn select_win311_from_menu(host: &Host, state: &mut State, events: &std::sync::mpsc::Receiver<Message>) -> Result<(), HostError> {
    let size = host.window.size()?;
    let base = host.ui.assets()?;
    let demo = state.demo.clone();
    demo.raise()?;
    host.ui.update_with_events(size, events, |message| state.update(message))?;
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
    host.ui.update_with_events(size, events, |message| state.update(message))?;
    // The application installs prepared theme assets in its frame phase after event dispatch.
    // Reproduce that production ordering before the synchronization update and render assertions.
    state.frame(host, size, 0.)?;
    host.ui.update_with_events(size, events, |message| state.update(message))?;
    assert_eq!(state.themes[1].1.mark()?, MenuMark::Radio { selected: true });
    assert_eq!(state.ui.assets()?.metrics()?, Theme::ALL[1].load(base.as_com_ref())?.metrics()?);
    host.ui.render(size, host.renderer.as_com_ref())?;
    Ok(())
}

/// Exercises application-level composition against real runtime, platform and decoder objects.
fn exercise() -> Result<(Host, State, std::sync::mpsc::Receiver<Message>), HostError> {
    let (host, mut state, events) = create()?;
    let size = host.window.size()?;
    let base = host.ui.assets()?;
    select_win311_from_menu(&host, &mut state, &events)?;
    for theme in Theme::ALL {
        state.select_theme(theme)?;
        // Drive the same deferred asset installation that Host performs between its two updates.
        state.frame(&host, size, 0.)?;
        // The photograph is below the initial viewport. Requesting the measured end guarantees
        // every theme crosses real image forwarding, clipping, validation and native GPU upload.
        state.sample_scroll.scroll_to_end()?;
        host.ui.update_with_events(size, &events, |message| state.update(message))?;
        host.ui.render(size, host.renderer.as_com_ref())?;
        assert_eq!(state.ui.assets()?.metrics()?, theme.load(base.as_com_ref())?.metrics()?);
        for (choice, item) in &state.themes {
            assert_eq!(item.mark()?, MenuMark::Radio { selected: *choice == theme });
        }
    }
    {
        state.select_theme(Theme::Default)?;
        state.activate(Action::Spacing(false), &Activation { label: String::new() })?;
        state.combo.set_selection(ChoiceSelection::Selected(2))?;
        assert_eq!(state.combo.selection()?, ChoiceSelection::Selected(2));
        assert_eq!(state.combo.display_text()?, "Cherry");
        state.entry.set_text("Unicode: 日本語")?;
        state.submit()?;
        assert!(state.entry.text()?.is_empty());
        assert!(state.log.text()?.ends_with("Unicode: 日本語"));
        state.demo.set_visibility(Visibility::Hidden)?;
        state.activate(Action::ShowDemo, &Activation { label: String::new() })?;
        assert_eq!(state.demo.snapshot()?.visibility, Visibility::Visible);
    }
    // Commit the final style edit before observing the installed immutable revision.
    state.frame(&host, size, 0.)?;
    assert_eq!(state.ui.assets()?.metrics()?.spacing, 2);
    host.ui.update_with_events(size, &events, |message| state.update(message))?;
    host.ui.render(size, host.renderer.as_com_ref())?;
    Ok((host, state, events))
}

/// Packaging supplies the actual adjacent libraries; ordinary unit runs explicitly report this as ignored.
#[cfg(not(target_arch = "wasm32"))]
#[test]
#[ignore = "run scripts/test-demo.sh to stage the independently built native plugins"]
fn packaged_full_application_themes_images_actions_and_close() {
    let (host, state, events) = exercise().unwrap();
    let retained = state.combo.clone();
    drop(host);
    assert_eq!(retained.selection(), Err(UI_E_CLOSED));
    drop(events);
}

#[cfg(target_arch = "wasm32")]
mod browser {
    use super::*;
    use std::cell::Cell;
    use wasm_bindgen::prelude::*;

    thread_local! {
        /// Independent observations do not keep the application, its session or its canvas alive.
        static RELEASED: RefCell<Option<[Rc<Cell<bool>>; 2]>> = const { RefCell::new(None) };
    }
    /// Concrete finite app checks the host's real RAF ownership paths after the full synchronous checks.
    struct Stop {
        /// Counts ordinary callbacks before an explicit stop or deliberate frame error.
        frames: u32,
        /// Selects the error path independently of the normal stop path.
        fail: bool,
        /// Test-owned flag observes final state destruction without retaining the state itself.
        released: Rc<Cell<bool>>,
    }
    impl AppState for Stop {
        /// The teardown fixture has no semantic messages.
        type Message = ();
        /// The successful run renders two frames; the failing run rejects its first callback.
        fn frame(&mut self, _: &Host, _: Size, _: f64) -> Result<bool, NcStatus> {
            self.frames += 1;
            if self.fail { Err(UI_E_RENDER) } else { Ok(self.frames < 3) }
        }
    }
    impl Drop for Stop {
        /// Final release proves the RAF closure/session cycle was broken.
        fn drop(&mut self) {
            self.released.set(true);
        }
    }
    /// Browser harness starts the real full demo checks followed by both asynchronous teardown paths.
    #[wasm_bindgen]
    pub fn full_demo_checks() -> Result<(), JsValue> {
        let (host, state, events) = exercise().map_err(|error| JsValue::from_str(&error.to_string()))?;
        drop(events);
        drop(state);
        let released = [Rc::new(Cell::new(false)), Rc::new(Cell::new(false))];
        RELEASED.with_borrow_mut(|stored| *stored = Some(released.clone()));
        host.run(
            Stop {
                frames: 0,
                fail: false,
                released: released[0].clone(),
            },
            std::sync::mpsc::channel().1,
        )
        .map_err(|error| JsValue::from_str(&error.to_string()))?;
        let host = Host::new(
            "Error teardown",
            Size { width: 160, height: 120 },
            neocogi_demo_assets::load_bundle().map_err(|error| JsValue::from_str(&error.to_string()))?,
        )
        .map_err(|error| JsValue::from_str(&error.to_string()))?;
        host.run(
            Stop {
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
        RELEASED.with_borrow(|stored| stored.as_ref().is_some_and(|flags| flags.iter().all(|flag| flag.get())))
    }
}
