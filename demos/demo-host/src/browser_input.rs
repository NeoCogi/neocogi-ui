//! DOM ownership and translation for the canvas host; widgets never depend on browser objects.

use std::{cell::RefCell, rc::Rc};
use microui_redux as microui;
use wasm_bindgen::{JsCast, closure::Closure};
use web_sys::{Event, EventTarget, FocusEvent, HtmlCanvasElement, HtmlTextAreaElement, KeyboardEvent, PointerEvent, WheelEvent};
use crate::wheel::{WheelAccumulator, WheelUnit};

/// Ordered, host-independent transitions drained once per animation frame.
pub(super) enum BrowserInput {
    /// Absolute framebuffer pointer position.
    MouseMove(i32, i32),
    /// Newly held buttons at the supplied framebuffer position.
    MouseDown(i32, i32, microui::MouseButton),
    /// Released buttons at the supplied framebuffer position.
    MouseUp(i32, i32, microui::MouseButton),
    /// Physical pixel scroll distance, positive right/down.
    Scroll(i32, i32),
    /// Logical command transition; never used to synthesize text.
    Key(microui::KeyEvent),
    /// Committed text, including pasted and composed Unicode.
    Text(String),
    /// Lost host focus or an interrupted pointer gesture.
    Cancel,
}

/// State shared only by DOM listeners and the host's frame drain.
#[derive(Default)]
struct InputQueue {
    /// FIFO transitions waiting for the core.
    events: Vec<BrowserInput>,
    /// One primary pointer emulates the core's mouse; other contacts are ignored.
    pointer: Option<i32>,
    /// DOM button mask used to recover chord changes delivered through pointermove.
    buttons: u16,
    /// Wheel fractions that have not yet accumulated to a physical pixel.
    wheel: WheelAccumulator,
    /// Composition owns the textarea until its final DOM value is available.
    composing: bool,
}

impl InputQueue {
    /// Revokes local gesture state and appends an ordered core cancellation.
    fn cancel(&mut self) {
        self.pointer = None;
        self.buttons = 0;
        self.composing = false;
        self.wheel = WheelAccumulator::default();
        self.events.push(BrowserInput::Cancel);
    }

    /// Transfers the committed DOM value once; composition updates remain browser-owned.
    fn commit_text(&mut self, text: &HtmlTextAreaElement) {
        if !self.composing {
            let value = text.value();
            text.set_value("");
            if !value.is_empty() {
                self.events.push(BrowserInput::Text(value));
            }
        }
    }
}

/// One registered DOM callback, removed before its Rust closure is released.
struct Listener {
    /// DOM object that owns the registration.
    target: EventTarget,
    /// Exact event name needed to remove the registration.
    name: &'static str,
    /// Browser-required closure boundary; its state is owned by the concrete input queue.
    callback: Closure<dyn FnMut(Event)>,
}

impl Drop for Listener {
    fn drop(&mut self) {
        // Removing a listener is infallible for a live EventTarget and matching registration.
        let _ = self
            .target
            .remove_event_listener_with_callback(self.name, self.callback.as_ref().unchecked_ref());
    }
}

/// Owns the canvas listeners and the native text input used for IME and paste.
pub(super) struct BrowserInputHost {
    /// Shared event queue, drained without keeping a RefCell borrow during application updates.
    queue: Rc<RefCell<InputQueue>>,
    /// Transparent input anchored inside the focused editor, outside pointer hit testing.
    text: HtmlTextAreaElement,
    /// Registrations are removed when the host is dropped, including partial construction failure.
    listeners: Vec<Listener>,
}

impl Drop for BrowserInputHost {
    fn drop(&mut self) {
        // Unregister before removing the focused element so teardown cannot enqueue new input.
        self.listeners.clear();
        self.text.remove();
    }
}

impl BrowserInputHost {
    /// Installs the host's pointer, wheel, keyboard, composition, and focus boundaries.
    pub(super) fn new(canvas: &HtmlCanvasElement) -> Result<Self, String> {
        let window = web_sys::window().ok_or("browser window is unavailable")?;
        let document = window.document().ok_or("browser document is unavailable")?;
        let text = document
            .create_element("textarea")
            .map_err(js_error)?
            .dyn_into::<HtmlTextAreaElement>()
            .map_err(|_| "created element is not a textarea")?;
        text.set_tab_index(-1);
        text.set_attribute("aria-label", "Canvas text input").map_err(js_error)?;
        text.set_attribute("autocomplete", "off").map_err(js_error)?;
        text.set_attribute("autocapitalize", "off").map_err(js_error)?;
        text.set_attribute("spellcheck", "false").map_err(js_error)?;
        text.style()
            .set_css_text("position:fixed;opacity:0;pointer-events:none;resize:none;padding:0;border:0;width:1px;height:1px;font-size:16px");
        document.body().ok_or("browser body is unavailable")?.append_child(&text).map_err(js_error)?;
        let mut host = Self {
            queue: Rc::new(RefCell::new(InputQueue::default())),
            text,
            listeners: Vec::new(),
        };
        canvas.set_tab_index(0);
        canvas.style().set_property("touch-action", "none").map_err(js_error)?;

        // Pointer capture keeps drags alive outside the canvas. Comparing the complete button mask
        // also handles chord transitions, for which browsers do not emit another pointerdown/up.
        for name in ["pointerdown", "pointermove", "pointerup"] {
            let canvas = canvas.clone();
            let target: EventTarget = canvas.clone().into();
            let queue = host.queue.clone();
            host.listen(&target, name, move |event| {
                let Some(pointer) = event.dyn_ref::<PointerEvent>() else { return };
                if !pointer.is_primary() {
                    return;
                }
                let mut queue = queue.borrow_mut();
                if queue.pointer.is_some_and(|id| id != pointer.pointer_id()) {
                    return;
                }
                let bounds = canvas.get_bounding_client_rect();
                let scale = canvas_scale(&canvas);
                let x = ((pointer.client_x() as f64 - bounds.left()) * scale[0]).round() as i32;
                let y = ((pointer.client_y() as f64 - bounds.top()) * scale[1]).round() as i32;
                if name == "pointerdown" {
                    if canvas.set_pointer_capture(pointer.pointer_id()).is_err() {
                        queue.cancel();
                        return;
                    }
                    queue.pointer = Some(pointer.pointer_id());
                    let _ = canvas.focus();
                }
                queue.events.push(BrowserInput::MouseMove(x, y));
                let buttons = pointer.buttons();
                // Ignore held buttons from a gesture that began elsewhere; it has no local capture.
                if queue.pointer.is_some() {
                    for (mask, button) in [
                        (1, microui::MouseButton::LEFT),
                        (2, microui::MouseButton::RIGHT),
                        (4, microui::MouseButton::MIDDLE),
                    ] {
                        if (buttons ^ queue.buttons) & mask != 0 {
                            queue.events.push(if buttons & mask != 0 {
                                BrowserInput::MouseDown(x, y, button)
                            } else {
                                BrowserInput::MouseUp(x, y, button)
                            });
                        }
                    }
                    queue.buttons = buttons;
                    if buttons == 0 {
                        // Clear ownership before the browser's normal lostpointercapture event.
                        queue.pointer = None;
                    }
                }
                event.prevent_default();
            })?;
        }
        for name in ["pointercancel", "lostpointercapture"] {
            let queue = host.queue.clone();
            let text = host.text.clone();
            host.listen(canvas.unchecked_ref(), name, move |event| {
                let Some(pointer) = event.dyn_ref::<PointerEvent>() else { return };
                let mut queue = queue.borrow_mut();
                if queue.pointer == Some(pointer.pointer_id()) {
                    queue.cancel();
                    text.set_value("");
                }
            })?;
        }
        {
            let queue = host.queue.clone();
            let canvas = canvas.clone();
            let target = canvas.clone().into();
            host.listen(&target, "wheel", move |event| {
                let Some(wheel) = event.dyn_ref::<WheelEvent>() else { return };
                let unit = match wheel.delta_mode() {
                    1 => WheelUnit::Line,
                    2 => WheelUnit::Page,
                    _ => WheelUnit::Pixel,
                };
                let mut queue = queue.borrow_mut();
                let delta = queue.wheel.translate(
                    [wheel.delta_x(), wheel.delta_y()],
                    unit,
                    canvas_scale(&canvas),
                    [canvas.width() as f64, canvas.height() as f64],
                );
                if delta != [0, 0] {
                    queue.events.push(BrowserInput::Scroll(delta[0], delta[1]));
                }
                event.prevent_default();
            })?;
        }

        // Commands work on both focus targets. Printable keys are committed only through the DOM
        // textarea, allowing dead keys, IME, paste, and Unicode strings to share one text path.
        for target in [EventTarget::from(canvas.clone()), EventTarget::from(host.text.clone())] {
            for name in ["keydown", "keyup"] {
                let queue = host.queue.clone();
                let text = host.text.clone();
                host.listen(&target, name, move |event| {
                    let Some(keyboard) = event.dyn_ref::<KeyboardEvent>() else { return };
                    if keyboard.is_composing() || keyboard.key_code() == 229 || queue.borrow().composing {
                        return;
                    }
                    if let Some(key) = map_key(&keyboard.key()) {
                        let mut transition = if name == "keydown" {
                            microui::KeyEvent::pressed(key, map_modifiers(keyboard))
                        } else {
                            microui::KeyEvent::released(key, map_modifiers(keyboard))
                        };
                        transition.repeat = keyboard.repeat();
                        queue.borrow_mut().events.push(BrowserInput::Key(transition));
                        let editable = event.target().is_some_and(|target| target == EventTarget::from(text.clone()));
                        let printable = matches!(key, microui::Key::Character(_) | microui::Key::Space);
                        // Keep the native paste shortcut and printable defaults; navigation/editing
                        // commands belong to the retained editor and must not edit the input buffer.
                        if !(editable && printable) {
                            event.prevent_default();
                        }
                    }
                })?;
            }
            let queue = host.queue.clone();
            let canvas = canvas.clone();
            let text = host.text.clone();
            host.listen(&target, "blur", move |event| {
                let next = event.dyn_ref::<FocusEvent>().and_then(FocusEvent::related_target);
                if next == Some(canvas.clone().into()) || next == Some(text.clone().into()) {
                    return;
                }
                queue.borrow_mut().cancel();
                text.set_value("");
            })?;
        }
        for name in ["compositionstart", "compositionend", "input"] {
            let queue = host.queue.clone();
            let text = host.text.clone();
            host.listen(host.text.clone().unchecked_ref(), name, move |_| {
                let mut queue = queue.borrow_mut();
                match name {
                    "compositionstart" => queue.composing = true,
                    "compositionend" => {
                        queue.composing = false;
                        queue.commit_text(&text);
                    }
                    _ => queue.commit_text(&text),
                }
                // Clearing after commit makes a following final input event observe an empty value.
            })?;
        }
        {
            let queue = host.queue.clone();
            let text = host.text.clone();
            host.listen(window.unchecked_ref(), "blur", move |_| {
                queue.borrow_mut().cancel();
                text.set_value("");
            })?;
        }
        Ok(host)
    }

    /// Adds a browser-required closure and immediately records its teardown ownership.
    fn listen<F: FnMut(Event) + 'static>(&mut self, target: &EventTarget, name: &'static str, callback: F) -> Result<(), String> {
        let callback = Closure::wrap(Box::new(callback) as Box<dyn FnMut(Event)>);
        target
            .add_event_listener_with_callback(name, callback.as_ref().unchecked_ref())
            .map_err(js_error)?;
        self.listeners.push(Listener { target: target.clone(), name, callback });
        Ok(())
    }

    /// Takes the current batch before application code can trigger additional DOM events.
    pub(super) fn drain(&mut self) -> Vec<BrowserInput> {
        std::mem::take(&mut self.queue.borrow_mut().events)
    }

    /// Places native input using the core's committed editor area without stealing external focus.
    pub(super) fn synchronize(&self, canvas: &HtmlCanvasElement, area: Option<microui::prelude::Recti>) {
        let active = canvas.owner_document().and_then(|document| document.active_element());
        let on_canvas = active == Some(canvas.clone().into());
        let on_text = active == Some(self.text.clone().into());
        if !on_canvas && !on_text {
            return;
        }
        if let Some(area) = area {
            let bounds = canvas.get_bounding_client_rect();
            let scale = canvas_scale(canvas);
            let style = self.text.style();
            let _ = style.set_property("left", &format!("{}px", bounds.left() + area.x as f64 / scale[0]));
            let _ = style.set_property("top", &format!("{}px", bounds.top() + area.y as f64 / scale[1]));
            let _ = style.set_property("height", &format!("{}px", area.height as f64 / scale[1]));
            if !on_text {
                let _ = self.text.focus();
            }
        } else if on_text {
            self.queue.borrow_mut().composing = false;
            self.text.set_value("");
            let _ = canvas.focus();
        }
    }
}

/// CSS-to-framebuffer scale follows the actual canvas bounds, including nonuniform CSS sizing.
fn canvas_scale(canvas: &HtmlCanvasElement) -> [f64; 2] {
    let bounds = canvas.get_bounding_client_rect();
    [
        canvas.width() as f64 / bounds.width().max(1.0),
        canvas.height() as f64 / bounds.height().max(1.0),
    ]
}

/// Preserves a browser exception's useful message at the host construction boundary.
fn js_error(error: wasm_bindgen::JsValue) -> String {
    error.as_string().unwrap_or_else(|| format!("{error:?}"))
}

#[cfg(test)]
#[path = "browser_input_tests.rs"]
mod tests;

/// Maps logical commands without deriving committed text from key events.
fn map_key(key: &str) -> Option<microui::Key> {
    match key {
        "Backspace" => Some(microui::Key::Backspace),
        "Delete" => Some(microui::Key::Delete),
        "Enter" => Some(microui::Key::Enter),
        "Escape" | "Esc" => Some(microui::Key::Escape),
        " " | "Spacebar" => Some(microui::Key::Space),
        "Tab" => Some(microui::Key::Tab),
        "Insert" => Some(microui::Key::Insert),
        "Home" => Some(microui::Key::Home),
        "End" => Some(microui::Key::End),
        "PageUp" => Some(microui::Key::PageUp),
        "PageDown" => Some(microui::Key::PageDown),
        "ArrowUp" => Some(microui::Key::ArrowUp),
        "ArrowDown" => Some(microui::Key::ArrowDown),
        "ArrowLeft" => Some(microui::Key::ArrowLeft),
        "ArrowRight" => Some(microui::Key::ArrowRight),
        "Alt" => Some(microui::Key::Alt),
        "Control" => Some(microui::Key::Control),
        "Shift" => Some(microui::Key::Shift),
        "Meta" | "OS" => Some(microui::Key::Super),
        _ if key.starts_with('F') && key.len() > 1 => key[1..]
            .parse::<u8>()
            .ok()
            .filter(|number| (1..=24).contains(number))
            .map(microui::Key::Function),
        _ => {
            let mut characters = key.chars();
            let character = characters.next()?;
            characters.next().is_none().then_some(microui::Key::Character(character))
        }
    }
}

/// Copies the browser's authoritative modifier snapshot for this transition.
fn map_modifiers(event: &KeyboardEvent) -> microui::Modifiers {
    let mut modifiers = microui::Modifiers::NONE;
    if event.alt_key() {
        modifiers |= microui::Modifiers::ALT;
    }
    if event.ctrl_key() {
        modifiers |= microui::Modifiers::CTRL;
    }
    if event.shift_key() {
        modifiers |= microui::Modifiers::SHIFT;
    }
    if event.meta_key() {
        modifiers |= microui::Modifiers::SUPER;
    }
    modifiers
}
