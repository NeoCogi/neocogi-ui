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
//! Shared native and browser application runner for examples.
//!
//! This module owns host setup, backend selection, input forwarding, and the example main loop used
//! by retained-mode demos. Native targets use SDL; `wasm32-unknown-unknown` uses the browser DOM and
//! WebGL 2 directly.
#[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
mod browser_input;
#[cfg(any(test, all(target_arch = "wasm32", target_os = "unknown")))]
mod wheel;

use microui_ui::{
    self as microui,
    prelude::{AtlasHandle, Dimensioni, FrameInfo, color},
};

#[cfg(any(feature = "webgl", feature = "glow"))]
use std::rc::Rc;

#[cfg(all(not(feature = "webgl"), feature = "glow"))]
use microui_redux_renderer_glow as glow_renderer;
#[cfg(all(not(feature = "webgl"), feature = "glow"))]
use microui_redux_renderer_glow::GlApplicationRenderer as PlatformRenderer;
#[cfg(all(feature = "webgl", target_arch = "wasm32", target_os = "unknown"))]
use microui_redux_renderer_webgl_canvas as webgl_canvas_renderer;
#[cfg(all(feature = "webgl", target_arch = "wasm32", target_os = "unknown"))]
use microui_redux_renderer_webgl_canvas::GlApplicationRenderer as PlatformRenderer;
#[cfg(all(not(feature = "webgl"), not(feature = "glow"), feature = "vulkan"))]
use microui_redux_renderer_vulkan as vulkan_renderer;
#[cfg(all(not(feature = "webgl"), not(feature = "glow"), feature = "vulkan"))]
use microui_redux_renderer_vulkan::VulkanApplicationRenderer as PlatformRenderer;
#[cfg(all(not(feature = "webgl"), not(feature = "glow"), not(feature = "vulkan"), feature = "wgpu"))]
use microui_redux_renderer_wgpu as wgpu_renderer;
#[cfg(all(not(feature = "webgl"), not(feature = "glow"), not(feature = "vulkan"), feature = "wgpu"))]
use microui_redux_renderer_wgpu::WgpuApplicationRenderer as PlatformRenderer;
#[cfg(not(all(target_arch = "wasm32", target_os = "unknown")))]
use sdl2::event::{Event, WindowEvent};
#[cfg(not(all(target_arch = "wasm32", target_os = "unknown")))]
use sdl2::keyboard::{Keycode, Mod};
#[cfg(all(not(all(target_arch = "wasm32", target_os = "unknown")), feature = "glow"))]
use sdl2::video::{GLContext, GLProfile, SwapInterval};
#[cfg(not(all(target_arch = "wasm32", target_os = "unknown")))]
use sdl2::video::Window;
#[cfg(not(all(target_arch = "wasm32", target_os = "unknown")))]
use sdl2::{Sdl, VideoSubsystem};

#[cfg(all(feature = "webgl", not(all(target_arch = "wasm32", target_os = "unknown"))))]
compile_error!("the `webgl` demo backend requires --target wasm32-unknown-unknown");
#[cfg(all(target_arch = "wasm32", target_os = "unknown", not(feature = "webgl")))]
compile_error!("browser demos require the `webgl` backend feature");

/// Renderer backend selected by Cargo features and specialized by application GPU state.
#[cfg(feature = "webgl")]
pub type SelectedBackend<ApplicationRenderer = ()> = webgl_canvas_renderer::WebGlCanvasRenderer<ApplicationRenderer>;
/// Renderer backend selected by Cargo features and specialized by application GPU state.
#[cfg(all(not(feature = "webgl"), feature = "glow"))]
pub type SelectedBackend<ApplicationRenderer = ()> = glow_renderer::GLRenderer<ApplicationRenderer>;
/// Renderer backend selected by Cargo features and specialized by application GPU state.
#[cfg(all(not(feature = "webgl"), not(feature = "glow"), feature = "vulkan"))]
pub type SelectedBackend<ApplicationRenderer = ()> = vulkan_renderer::VulkanRenderer<ApplicationRenderer>;
/// Renderer backend selected by Cargo features and specialized by application GPU state.
#[cfg(all(not(feature = "webgl"), not(feature = "glow"), not(feature = "vulkan"), feature = "wgpu"))]
pub type SelectedBackend<ApplicationRenderer = ()> = wgpu_renderer::WgpuRenderer<ApplicationRenderer>;

#[cfg(any(feature = "webgl", feature = "glow"))]
/// Native graphics handle supplied to application state initialization on GL backends.
pub type BackendInitContext = Rc<glow::Context>;
#[cfg(any(
    all(not(feature = "webgl"), not(feature = "glow"), feature = "vulkan"),
    all(not(feature = "webgl"), not(feature = "glow"), not(feature = "vulkan"), feature = "wgpu"),
))]
/// Unit initialization value for backends whose application renderer owns native setup.
pub type BackendInitContext = ();

#[cfg(not(all(target_arch = "wasm32", target_os = "unknown")))]
/// Native SDL application hosting microui and a concrete application GPU renderer.
pub struct Application<S: 'static, ApplicationRenderer = ()>
where
    ApplicationRenderer: PlatformRenderer,
{
    // Drop renderer-backed state before the SDL window/subsystem. Vulkan and wgpu surfaces
    // borrow native window resources, so the window must outlive `state`, `ctx`, and backend data.
    /// Application-owned retained state updated by the demo callbacks.
    state: S,
    /// Microui context containing the selected, application-specialized backend.
    ctx: microui::Context<SelectedBackend<ApplicationRenderer>, S>,
    #[cfg(feature = "glow")]
    /// Native GL context that must remain current while rendering and outlive GL resources.
    backend: BackendData,
    /// SDL window whose native handles back Vulkan/wgpu surfaces or the GL drawable.
    window: Window,
    /// SDL video subsystem retained for the lifetime of its window and GL configuration.
    _sdl_vid: VideoSubsystem,
    /// Root SDL owner used to construct the event pump.
    sdl_ctx: Sdl,
}

#[cfg(not(all(target_arch = "wasm32", target_os = "unknown")))]
impl<S: 'static> Application<S, ()> {
    /// Creates a UI-only application using the selected backend's uninhabited command type.
    pub fn new<F: FnMut(BackendInitContext, &mut microui::Context<SelectedBackend, S>) -> S>(atlas: AtlasHandle, mut init_state: F) -> Result<Self, String>
    where
        S: 'static,
    {
        Self::new_with_renderer(atlas, (), move |backend, context| init_state(backend, context))
    }
}

#[cfg(not(all(target_arch = "wasm32", target_os = "unknown")))]
impl<S, ApplicationRenderer> Application<S, ApplicationRenderer>
where
    S: 'static,
    ApplicationRenderer: PlatformRenderer,
{
    /// Creates an application with concrete GPU renderer state supplied by the caller.
    pub fn new_with_renderer<F>(atlas: AtlasHandle, application_renderer: ApplicationRenderer, mut init_state: F) -> Result<Self, String>
    where
        F: FnMut(BackendInitContext, &mut microui::Context<SelectedBackend<ApplicationRenderer>, S>) -> S,
    {
        // SDL/video/window setup is backend-dependent, but state construction always receives a
        // ready-to-use, statically specialized microui `Context` plus any backend-specific
        // initialization payload.
        let sdl_ctx = sdl2::init().map_err(|err| err.to_string())?;
        let video = sdl_ctx.video().map_err(|err| err.to_string())?;
        let (bundle, init_ctx) = init_backend(&video, atlas, application_renderer)?;
        #[cfg(feature = "glow")]
        let BackendBundle { window, backend, renderer } = bundle;
        #[cfg(any(
            all(not(feature = "glow"), feature = "vulkan"),
            all(not(feature = "glow"), not(feature = "vulkan"), feature = "wgpu"),
        ))]
        let BackendBundle { window, renderer } = bundle;

        let mut ctx = microui::Context::<_, S>::new(renderer);
        Ok(Self {
            state: init_state(init_ctx, &mut ctx),
            ctx,
            #[cfg(feature = "glow")]
            backend,
            window,
            _sdl_vid: video,
            sdl_ctx,
        })
    }

    /// Runs the SDL event loop, forwarding input into microui and invoking the user frame callback.
    #[allow(dead_code)] // Each example selects either polling or subscriber-driven updates.
    pub fn event_loop<F>(self, f: F)
    where
        F: Fn(&mut microui::Context<SelectedBackend<ApplicationRenderer>, S>, &mut S, Dimensioni) + 'static,
    {
        self.event_loop_with_update(|ctx, state, dimensions| ctx.update_ui_state(dimensions, state), f);
    }

    /// Runs the SDL loop with the context-owned application event session.
    #[allow(dead_code)] // Each example selects either polling or subscriber-driven updates.
    pub fn event_loop_events<Setup, F>(mut self, setup: Setup, f: F)
    where
        S: 'static,
        Setup: FnOnce(&S, &mut microui::Context<SelectedBackend<ApplicationRenderer>, S>),
        F: Fn(&mut microui::Context<SelectedBackend<ApplicationRenderer>, S>, &mut S, Dimensioni) + 'static,
    {
        setup(&self.state, &mut self.ctx);
        self.event_loop_with_update(|ctx, state, dimensions| ctx.update_ui_state(dimensions, state), f);
    }

    /// Shared SDL driver parameterized by the first retained update performed each frame.
    fn event_loop_with_update<Update, F>(self, update: Update, f: F)
    where
        Update: FnMut(&mut microui::Context<SelectedBackend<ApplicationRenderer>, S>, &mut S, Dimensioni) + 'static,
        F: Fn(&mut microui::Context<SelectedBackend<ApplicationRenderer>, S>, &mut S, Dimensioni) + 'static,
    {
        #[cfg(feature = "glow")]
        {
            self.window.gl_make_current(&self.backend.gl_ctx).unwrap();
            // The shared runner otherwise renders without a scheduling boundary on drivers whose
            // default swap interval is immediate. That can starve SDL event delivery in debug
            // examples and make an already-queued click appear delayed.
            let _ = self._sdl_vid.gl_set_swap_interval(SwapInterval::VSync);
        }

        let mut event_pump = self.sdl_ctx.event_pump().unwrap();
        let mut application = self;
        let mut update = update;
        while application.run_frame(&mut event_pump, &mut update, &f) {}
    }

    /// Processes input and renders one frame, returning false when the host requests shutdown.
    fn run_frame<Update, F>(&mut self, event_pump: &mut sdl2::EventPump, update: &mut Update, f: &F) -> bool
    where
        Update: FnMut(&mut microui::Context<SelectedBackend<ApplicationRenderer>, S>, &mut S, Dimensioni),
        F: Fn(&mut microui::Context<SelectedBackend<ApplicationRenderer>, S>, &mut S, Dimensioni),
    {
        let (width, height) = self.window.size();

        fn map_mouse_button(sdl_mb: sdl2::mouse::MouseButton) -> microui::MouseButton {
            match sdl_mb {
                sdl2::mouse::MouseButton::Left => microui::MouseButton::LEFT,
                sdl2::mouse::MouseButton::Right => microui::MouseButton::RIGHT,
                sdl2::mouse::MouseButton::Middle => microui::MouseButton::MIDDLE,
                _ => microui::MouseButton::NONE,
            }
        }

        /// Converts SDL's platform key identity into the crate's single logical key space.
        fn map_key(keycode: Keycode) -> Option<microui::Key> {
            let named = match keycode {
                Keycode::Backspace => Some(microui::Key::Backspace),
                Keycode::Delete => Some(microui::Key::Delete),
                Keycode::Return | Keycode::KpEnter => Some(microui::Key::Enter),
                Keycode::Escape => Some(microui::Key::Escape),
                Keycode::Space => Some(microui::Key::Space),
                Keycode::Tab => Some(microui::Key::Tab),
                Keycode::Insert => Some(microui::Key::Insert),
                Keycode::Home => Some(microui::Key::Home),
                Keycode::End => Some(microui::Key::End),
                Keycode::PageUp => Some(microui::Key::PageUp),
                Keycode::PageDown => Some(microui::Key::PageDown),
                Keycode::Up => Some(microui::Key::ArrowUp),
                Keycode::Down => Some(microui::Key::ArrowDown),
                Keycode::Left => Some(microui::Key::ArrowLeft),
                Keycode::Right => Some(microui::Key::ArrowRight),
                Keycode::F1 => Some(microui::Key::Function(1)),
                Keycode::F2 => Some(microui::Key::Function(2)),
                Keycode::F3 => Some(microui::Key::Function(3)),
                Keycode::F4 => Some(microui::Key::Function(4)),
                Keycode::F5 => Some(microui::Key::Function(5)),
                Keycode::F6 => Some(microui::Key::Function(6)),
                Keycode::F7 => Some(microui::Key::Function(7)),
                Keycode::F8 => Some(microui::Key::Function(8)),
                Keycode::F9 => Some(microui::Key::Function(9)),
                Keycode::F10 => Some(microui::Key::Function(10)),
                Keycode::F11 => Some(microui::Key::Function(11)),
                Keycode::F12 => Some(microui::Key::Function(12)),
                Keycode::LAlt | Keycode::RAlt => Some(microui::Key::Alt),
                Keycode::LCtrl | Keycode::RCtrl => Some(microui::Key::Control),
                Keycode::LShift | Keycode::RShift => Some(microui::Key::Shift),
                Keycode::LGui | Keycode::RGui => Some(microui::Key::Super),
                _ => None,
            };
            named.or_else(|| {
                // SDL assigns printable keycodes their Unicode scalar value. Preserve that
                // logical value for future accelerators while TextInput remains authoritative
                // for composed text.
                char::from_u32(keycode.into_i32() as u32)
                    .filter(|character| !character.is_control())
                    .map(microui::Key::Character)
            })
        }

        /// Converts SDL's complete modifier snapshot without retaining backend-specific bits.
        fn map_modifiers(keymod: Mod) -> microui::Modifiers {
            let mut modifiers = microui::Modifiers::NONE;
            if keymod.intersects(Mod::LALTMOD | Mod::RALTMOD) {
                modifiers |= microui::Modifiers::ALT;
            }
            if keymod.intersects(Mod::LCTRLMOD | Mod::RCTRLMOD) {
                modifiers |= microui::Modifiers::CTRL;
            }
            if keymod.intersects(Mod::LSHIFTMOD | Mod::RSHIFTMOD) {
                modifiers |= microui::Modifiers::SHIFT;
            }
            if keymod.intersects(Mod::LGUIMOD | Mod::RGUIMOD) {
                modifiers |= microui::Modifiers::SUPER;
            }
            modifiers
        }
        // SDL events are translated into the narrower microui input vocabulary here. This
        // keeps the rest of the demo code backend-agnostic.
        let mut pending_mouse_move = None;
        for event in event_pump.poll_iter() {
            // SDL can produce many consecutive motion samples between rendered frames. The
            // retained Context intentionally commits every event independently, so forwarding
            // all of those samples creates a queue of full update/layout transactions ahead
            // of a later click. Preserve the last position and flush it before any non-motion
            // event, keeping observable ordering while bounding motion work per host batch.
            let event = match event {
                Event::MouseMotion { x, y, .. } => {
                    pending_mouse_move = Some((x, y));
                    continue;
                }
                event => event,
            };
            if let Some((x, y)) = pending_mouse_move.take() {
                self.ctx.mousemove(x, y);
            }
            match event {
                Event::Quit { .. } => return false,
                Event::Window { win_event: WindowEvent::Close, .. } => return false,
                Event::Window { win_event: WindowEvent::FocusLost, .. } => self.ctx.cancel_input(),
                Event::MouseWheel { x, y, .. } => self.ctx.scroll(x * -30, y * -30),
                Event::MouseButtonDown { x, y, mouse_btn, .. } => {
                    let mb = map_mouse_button(mouse_btn);
                    self.ctx.mousedown(x, y, mb);
                }
                Event::MouseButtonUp { x, y, mouse_btn, .. } => {
                    let mb = map_mouse_button(mouse_btn);
                    self.ctx.mouseup(x, y, mb);
                }
                Event::KeyDown {
                    keycode: Some(keycode), keymod, repeat, ..
                } => {
                    // Forward every logical press with SDL's complete modifier and repeat
                    // snapshot. The UI manager can then scope popup/menu commands precisely,
                    // while ordinary focused widgets retain presses that no command accepts.
                    if let Some(key) = map_key(keycode) {
                        let mut event = microui::KeyEvent::pressed(key, map_modifiers(keymod));
                        event.repeat = repeat;
                        self.ctx.key(event);
                    }
                }
                Event::KeyUp { keycode: Some(keycode), keymod, .. } => {
                    // Releases are forwarded independently rather than inferred from held
                    // state. Popup dismissal consumes only its initial press, so the currently
                    // focused surface—including a restored popup parent—receives the release.
                    if let Some(key) = map_key(keycode) {
                        self.ctx.key(microui::KeyEvent::released(key, map_modifiers(keymod)));
                    }
                }
                Event::TextInput { text, .. } => {
                    self.ctx.text(text.as_str());
                }

                _ => {}
            }
        }
        if let Some((x, y)) = pending_mouse_move {
            self.ctx.mousemove(x, y);
        }

        let dimensions = Dimensioni::new(width as i32, height as i32);
        if let Ok(info) = FrameInfo::try_new(dimensions, color(0x7F, 0x7F, 0x7F, 255)) {
            // First commit queued host input so application polling observes this frame's
            // completed widget and Context-owned dialog actions.
            update(&mut self.ctx, &mut self.state, dimensions);
            f(&mut self.ctx, &mut self.state, dimensions);
            // Application mutations can affect retained state and layout, so synchronize once
            // more before the paint-only frame is submitted.
            update(&mut self.ctx, &mut self.state, dimensions);
            // Activate SDL's IME only for committed editable focus, using the same host-neutral
            // area as the browser. Avoid restarting an already active composition each frame.
            let text_input = self._sdl_vid.text_input();
            if let Some(area) = self.ctx.text_input_area() {
                text_input.set_rect(sdl2::rect::Rect::new(area.x, area.y, area.width as u32, area.height as u32));
                if !text_input.is_active() {
                    text_input.start();
                }
            } else if text_input.is_active() {
                text_input.stop();
            }
            if let Err(error) = self.ctx.frame(info).render_ui() {
                eprintln!("[microui-redux][example] frame failed: {error}");
            }
        }
        #[cfg(feature = "glow")]
        self.window.gl_swap_window();

        true
    }
}

#[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
mod browser {
    use std::{cell::RefCell, rc::Rc};

    use wasm_bindgen::{JsCast, closure::Closure};
    use web_sys::{HtmlCanvasElement, HtmlElement};
    use crate::browser_input::{BrowserInput, BrowserInputHost};

    use super::{AtlasHandle, BackendInitContext, Dimensioni, FrameInfo, PlatformRenderer, SelectedBackend, color, microui, webgl_canvas_renderer};

    /// Browser application runner backed directly by an HTML canvas and WebGL 2.
    pub struct Application<S: 'static, ApplicationRenderer = ()>
    where
        ApplicationRenderer: PlatformRenderer,
    {
        /// Application-owned retained state updated by the browser frame callbacks.
        state: S,
        /// Microui context containing the application-specialized WebGL backend.
        ctx: microui::Context<SelectedBackend<ApplicationRenderer>, S>,
        /// Canvas that owns the active WebGL context and determines physical frame dimensions.
        canvas: HtmlCanvasElement,
        /// Buffered DOM input translated into the narrower microui event vocabulary.
        input: BrowserInputHost,
    }

    impl<S: 'static> Application<S, ()> {
        /// Creates a UI-only browser application from the page's `canvas` element.
        pub fn new<F: FnMut(BackendInitContext, &mut microui::Context<SelectedBackend, S>) -> S>(
            atlas: AtlasHandle,
            mut init_state: F,
        ) -> Result<Self, String> {
            Self::new_with_renderer(atlas, (), move |backend, context| init_state(backend, context))
        }
    }

    impl<S, ApplicationRenderer> Application<S, ApplicationRenderer>
    where
        S: 'static,
        ApplicationRenderer: PlatformRenderer,
    {
        /// Creates a browser application with caller-owned WebGL renderer state.
        pub fn new_with_renderer<F>(atlas: AtlasHandle, application_renderer: ApplicationRenderer, mut init_state: F) -> Result<Self, String>
        where
            F: FnMut(BackendInitContext, &mut microui::Context<SelectedBackend<ApplicationRenderer>, S>) -> S,
        {
            let window = web_sys::window().ok_or("browser window is unavailable")?;
            let document = window.document().ok_or("browser document is unavailable")?;
            let canvas = document
                .get_element_by_id("canvas")
                .ok_or("expected an HTML canvas with id `canvas`")?
                .dyn_into::<HtmlCanvasElement>()
                .map_err(|_| "element with id `canvas` is not an HTML canvas")?;
            resize_canvas(&canvas);

            let webgl = canvas
                .get_context("webgl2")
                .map_err(js_error)?
                .ok_or("WebGL 2 is unavailable in this browser")?
                .dyn_into::<web_sys::WebGl2RenderingContext>()
                .map_err(|_| "canvas returned a non-WebGL 2 context")?;
            let gl = Rc::new(glow::Context::from_webgl2_context(webgl));
            let renderer =
                webgl_canvas_renderer::WebGlCanvasRenderer::new_with_application(gl.clone(), atlas, canvas.width(), canvas.height(), application_renderer)?;
            let mut ctx = microui::Context::new(renderer);
            let state = init_state(gl, &mut ctx);
            let input = BrowserInputHost::new(&canvas)?;
            let _ = canvas.unchecked_ref::<HtmlElement>().focus();

            Ok(Self { state, ctx, canvas, input })
        }

        /// Starts a `requestAnimationFrame` loop using the normal retained-state update.
        #[allow(dead_code)]
        pub fn event_loop<F>(self, f: F)
        where
            F: Fn(&mut microui::Context<SelectedBackend<ApplicationRenderer>, S>, &mut S, Dimensioni) + 'static,
        {
            self.event_loop_with_update(|ctx, state, dimensions| ctx.update_ui_state(dimensions, state), f);
        }

        /// Starts a `requestAnimationFrame` loop after registering context event subscriptions.
        #[allow(dead_code)]
        pub fn event_loop_events<Setup, F>(mut self, setup: Setup, f: F)
        where
            Setup: FnOnce(&S, &mut microui::Context<SelectedBackend<ApplicationRenderer>, S>),
            F: Fn(&mut microui::Context<SelectedBackend<ApplicationRenderer>, S>, &mut S, Dimensioni) + 'static,
        {
            setup(&self.state, &mut self.ctx);
            self.event_loop_with_update(|ctx, state, dimensions| ctx.update_ui_state(dimensions, state), f);
        }

        fn event_loop_with_update<Update, F>(self, update: Update, frame: F)
        where
            Update: FnMut(&mut microui::Context<SelectedBackend<ApplicationRenderer>, S>, &mut S, Dimensioni) + 'static,
            F: Fn(&mut microui::Context<SelectedBackend<ApplicationRenderer>, S>, &mut S, Dimensioni) + 'static,
        {
            /// State retained by the self-scheduling browser animation callback.
            struct Driver<S: 'static, ApplicationRenderer, Update, Frame>
            where
                ApplicationRenderer: PlatformRenderer,
            {
                /// Browser application whose canvas and renderer survive between callbacks.
                application: Application<S, ApplicationRenderer>,
                /// Retained-state update invoked before and after application frame construction.
                update: Update,
                /// Application UI construction callback invoked once per animation frame.
                frame: Frame,
            }

            let driver = Rc::new(RefCell::new(Driver { application: self, update, frame }));
            let animation_frame: Rc<RefCell<Option<Closure<dyn FnMut(f64)>>>> = Rc::new(RefCell::new(None));
            let callback_slot = animation_frame.clone();
            let callback_driver = driver.clone();
            *animation_frame.borrow_mut() = Some(Closure::wrap(Box::new(move |_timestamp: f64| {
                {
                    let mut driver = callback_driver.borrow_mut();
                    let Driver { application, update, frame } = &mut *driver;
                    application.run_frame(update, frame);
                }
                if let (Some(window), Some(callback)) = (web_sys::window(), callback_slot.borrow().as_ref()) {
                    let _ = window.request_animation_frame(callback.as_ref().unchecked_ref());
                }
            }) as Box<dyn FnMut(f64)>));

            if let (Some(window), Some(callback)) = (web_sys::window(), animation_frame.borrow().as_ref()) {
                let _ = window.request_animation_frame(callback.as_ref().unchecked_ref());
            }
        }

        fn run_frame<Update, F>(&mut self, update: &mut Update, frame: &F)
        where
            Update: FnMut(&mut microui::Context<SelectedBackend<ApplicationRenderer>, S>, &mut S, Dimensioni),
            F: Fn(&mut microui::Context<SelectedBackend<ApplicationRenderer>, S>, &mut S, Dimensioni),
        {
            resize_canvas(&self.canvas);
            for event in self.input.drain() {
                match event {
                    BrowserInput::MouseMove(x, y) => self.ctx.mousemove(x, y),
                    BrowserInput::MouseDown(x, y, button) => self.ctx.mousedown(x, y, button),
                    BrowserInput::MouseUp(x, y, button) => self.ctx.mouseup(x, y, button),
                    BrowserInput::Scroll(x, y) => self.ctx.scroll(x, y),
                    BrowserInput::Key(event) => self.ctx.key(event),
                    BrowserInput::Text(text) => self.ctx.text(&text),
                    BrowserInput::Cancel => self.ctx.cancel_input(),
                }
            }

            let dimensions = Dimensioni::new(self.canvas.width() as i32, self.canvas.height() as i32);
            if let Ok(info) = FrameInfo::try_new(dimensions, color(0x7F, 0x7F, 0x7F, 255)) {
                update(&mut self.ctx, &mut self.state, dimensions);
                frame(&mut self.ctx, &mut self.state, dimensions);
                update(&mut self.ctx, &mut self.state, dimensions);
                self.input.synchronize(&self.canvas, self.ctx.text_input_area());
                if let Err(error) = self.ctx.frame(info).render_ui() {
                    eprintln!("[microui-redux][example] frame failed: {error}");
                }
            }
        }
    }

    fn resize_canvas(canvas: &HtmlCanvasElement) {
        let scale = web_sys::window().map(|window| window.device_pixel_ratio()).unwrap_or(1.0);
        let width = ((canvas.client_width() as f64 * scale).round() as u32).max(1);
        let height = ((canvas.client_height() as f64 * scale).round() as u32).max(1);
        if canvas.width() != width {
            canvas.set_width(width);
        }
        if canvas.height() != height {
            canvas.set_height(height);
        }
    }

    fn js_error(error: wasm_bindgen::JsValue) -> String {
        error.as_string().unwrap_or_else(|| format!("{error:?}"))
    }
}

#[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
pub use browser::Application;

/// Returns a monotonic timestamp in seconds on both native and browser targets.
#[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
pub fn monotonic_seconds() -> f64 {
    web_sys::window()
        .and_then(|window| window.performance())
        .map(|performance| performance.now() / 1000.0)
        .unwrap_or(0.0)
}

/// Returns a monotonic timestamp in seconds on both native and browser targets.
#[cfg(not(all(target_arch = "wasm32", target_os = "unknown")))]
pub fn monotonic_seconds() -> f64 {
    use std::{sync::OnceLock, time::Instant};

    static START: OnceLock<Instant> = OnceLock::new();
    START.get_or_init(Instant::now).elapsed().as_secs_f64()
}

#[cfg(all(not(all(target_arch = "wasm32", target_os = "unknown")), feature = "glow"))]
/// Initializes the OpenGL example backend and returns the window, renderer, and GL init context.
fn init_backend<ApplicationRenderer>(
    video: &VideoSubsystem,
    atlas: AtlasHandle,
    application_renderer: ApplicationRenderer,
) -> Result<(BackendBundle<ApplicationRenderer>, BackendInitContext), String>
where
    ApplicationRenderer: PlatformRenderer,
{
    // The GL example owns an explicit SDL GL context in addition to the microui renderer.
    let gl_attr = video.gl_attr();
    gl_attr.set_context_profile(GLProfile::GLES);
    gl_attr.set_context_version(3, 0);
    gl_attr.set_depth_size(24);

    let window = video.window("Window", 1024, 768).resizable().opengl().build().map_err(|err| err.to_string())?;
    let gl_ctx = window.gl_create_context().map_err(|err| err.to_string())?;
    window.gl_make_current(&gl_ctx).map_err(|err| err.to_string())?;

    let gl = unsafe { glow::Context::from_loader_function(|s| video.gl_get_proc_address(s) as *const _) };
    debug_assert_eq!(gl_attr.context_profile(), GLProfile::GLES);
    debug_assert_eq!(gl_attr.context_version(), (3, 0));

    let (width, height) = window.size();
    let gl = Rc::new(gl);
    let renderer = glow_renderer::GLRenderer::new_with_application(gl.clone(), atlas, width, height, application_renderer)?;

    Ok((
        BackendBundle {
            window,
            backend: BackendData { gl_ctx },
            renderer,
        },
        gl,
    ))
}

#[cfg(all(not(all(target_arch = "wasm32", target_os = "unknown")), not(feature = "glow"), feature = "vulkan"))]
/// Initializes the Vulkan example backend and returns the window, renderer, and init marker.
fn init_backend<ApplicationRenderer>(
    video: &VideoSubsystem,
    atlas: AtlasHandle,
    application_renderer: ApplicationRenderer,
) -> Result<(BackendBundle<ApplicationRenderer>, BackendInitContext), String>
where
    ApplicationRenderer: PlatformRenderer,
{
    // Vulkan and wgpu derive their native surfaces from the SDL window itself, so the renderer is
    // created immediately from that window handle and then stored inside the microui `Context`.
    let window = video.window("Window", 1024, 768).resizable().vulkan().build().map_err(|err| err.to_string())?;
    let (width, height) = window.size();
    let renderer = vulkan_renderer::VulkanRenderer::new_with_application(&window, atlas, width, height, application_renderer)?;
    let init_ctx = ();

    Ok((BackendBundle { window, renderer }, init_ctx))
}

#[cfg(all(
    not(all(target_arch = "wasm32", target_os = "unknown")),
    not(feature = "glow"),
    not(feature = "vulkan"),
    feature = "wgpu"
))]
/// Initializes the wgpu example backend and returns the window, renderer, and init marker.
fn init_backend<ApplicationRenderer>(
    video: &VideoSubsystem,
    atlas: AtlasHandle,
    application_renderer: ApplicationRenderer,
) -> Result<(BackendBundle<ApplicationRenderer>, BackendInitContext), String>
where
    ApplicationRenderer: PlatformRenderer,
{
    let window = video.window("Window", 1024, 768).resizable().build().map_err(|err| err.to_string())?;
    let (width, height) = window.size();
    let renderer = wgpu_renderer::WgpuRenderer::new_with_application(&window, atlas, width, height, application_renderer)?;
    let init_ctx = ();

    Ok((BackendBundle { window, renderer }, init_ctx))
}

#[cfg(not(all(target_arch = "wasm32", target_os = "unknown")))]
/// Backend-specific objects produced as one initialization transaction.
struct BackendBundle<ApplicationRenderer>
where
    ApplicationRenderer: PlatformRenderer,
{
    /// SDL window whose native handles remain valid for the renderer lifetime.
    window: Window,
    #[cfg(feature = "glow")]
    /// Explicit native GL context retained outside the Glow renderer.
    backend: BackendData,
    /// Feature-selected backend specialized by concrete application renderer state.
    renderer: SelectedBackend<ApplicationRenderer>,
}

#[cfg(all(not(all(target_arch = "wasm32", target_os = "unknown")), feature = "glow"))]
/// Native GL objects owned by the host rather than by the renderer backend.
struct BackendData {
    /// SDL GL context made current before entering the frame loop.
    gl_ctx: GLContext,
}
