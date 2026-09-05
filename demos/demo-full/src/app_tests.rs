//! Full application checks use the actual plugin graph, including GPU images and all compiled themes.
use super::*;

/// Selects Win 3.11 through the real nested menu and event session, rather than calling its handler directly.
fn select_win311_from_menu(host: &Host, events: &EventSession<State>) -> Result<(), HostError> {
    let size = host.window.size()?;
    let demo = events.state()?.demo.clone();
    demo.raise()?;
    host.ui.update_with_events(size, events)?;
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
        KeyCode::FUNCTION,
        KeyCode::ARROW_RIGHT,
        KeyCode::ARROW_DOWN,
        KeyCode::ARROW_DOWN,
        KeyCode::ARROW_DOWN,
        KeyCode::ARROW_DOWN,
        KeyCode::ARROW_RIGHT,
        KeyCode::ARROW_DOWN,
        KeyCode::ENTER,
    ] {
        host.ui.input(
            UiInput::Key(KeyStroke {
                key,
                value: if key == KeyCode::FUNCTION { 10 } else { 0 },
                pressed: true,
                repeat: false,
                modifiers: 0,
            }),
            "",
        )?;
    }
    host.ui.update_with_events(size, events)?;
    let state = events.state()?;
    assert_eq!(state.themes[1].1.mark()?, MenuMark::Radio { selected: true });
    assert_eq!(state.ui.assets()?.metrics()?, Theme::ALL[1].data()?.metrics);
    host.ui.render(size, host.renderer.as_com_ref())?;
    Ok(())
}

/// Exercises application-level composition against real runtime, platform and decoder objects.
fn exercise() -> Result<(Host, EventSession<State>), HostError> {
    let (host, events) = create()?;
    let size = host.window.size()?;
    select_win311_from_menu(&host, &events)?;
    for theme in Theme::ALL {
        events.state_mut()?.select_theme(theme)?;
        // The photograph is below the initial viewport. Requesting the measured end guarantees
        // every theme crosses real image forwarding, clipping, validation and native GPU upload.
        events.state()?.sample_scroll.scroll_to_end()?;
        host.ui.update_with_events(size, &events)?;
        host.ui.render(size, host.renderer.as_com_ref())?;
        let state = events.state()?;
        assert_eq!(state.ui.assets()?.metrics()?, theme.data()?.metrics);
        for (choice, item) in &state.themes {
            assert_eq!(item.mark()?, MenuMark::Radio { selected: *choice == theme });
        }
    }
    {
        let mut state = events.state_mut()?;
        state.select_theme(Theme::Default)?;
        Action::Spacing(false).handle(&mut state, &Activation { label: String::new() })?;
        assert_eq!(state.ui.assets()?.metrics()?.spacing, 2);
        Action::Fruit(2).handle(&mut state, &Activation { label: "Cherry".to_owned() })?;
        assert_eq!(state.combo.selected()?, 2);
        assert_eq!(state.combo.label()?, "Cherry");
        assert!(!state.combo.is_open()?);
        state.entry.set_text("Unicode: 日本語")?;
        state.submit()?;
        assert!(state.entry.text()?.is_empty());
        assert!(state.log.text()?.ends_with("Unicode: 日本語"));
        state.demo.set_visibility(Visibility::HIDDEN)?;
        Action::ShowDemo.handle(&mut state, &Activation { label: String::new() })?;
        assert_eq!(state.demo.snapshot()?.visibility, Visibility::VISIBLE);
    }
    host.ui.update_with_events(size, &events)?;
    host.ui.render(size, host.renderer.as_com_ref())?;
    Ok((host, events))
}

/// Packaging supplies the actual adjacent libraries; ordinary unit runs explicitly report this as ignored.
#[cfg(not(target_arch = "wasm32"))]
#[test]
#[ignore = "run scripts/test-demo.sh to stage the independently built native plugins"]
fn packaged_full_application_themes_images_actions_and_close() {
    let (host, events) = exercise().unwrap();
    let retained = events.state().unwrap().combo.clone();
    drop(host);
    assert_eq!(retained.selected(), Err(UI_E_CLOSED));
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
        let (host, events) = exercise().map_err(|error| JsValue::from_str(&error.to_string()))?;
        drop(events);
        let released = [Rc::new(Cell::new(false)), Rc::new(Cell::new(false))];
        RELEASED.with_borrow_mut(|stored| *stored = Some(released.clone()));
        host.run(EventSession::new(Stop {
            frames: 0,
            fail: false,
            released: released[0].clone(),
        }))
        .map_err(|error| JsValue::from_str(&error.to_string()))?;
        let host = Host::new(
            "Error teardown",
            Size { width: 160, height: 120 },
            microui_redux_demo_assets::load_bundle().map_err(|error| JsValue::from_str(&error.to_string()))?,
        )
        .map_err(|error| JsValue::from_str(&error.to_string()))?;
        host.run(EventSession::new(Stop {
            frames: 0,
            fail: true,
            released: released[1].clone(),
        }))
        .map_err(|error| JsValue::from_str(&error.to_string()))
    }
    /// The HTML harness also checks that both platform-owned canvases and input textareas disappeared.
    #[wasm_bindgen]
    pub fn demo_loops_released() -> bool {
        RELEASED.with_borrow(|stored| stored.as_ref().is_some_and(|flags| flags.iter().all(|flag| flag.get())))
    }
}
