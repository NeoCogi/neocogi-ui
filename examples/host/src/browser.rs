//! requestAnimationFrame is the sole JavaScript closure boundary; application state stays concrete.
use super::*;
use std::{cell::RefCell, rc::Rc};
use wasm_bindgen::{closure::Closure, JsCast};

/// Owned JavaScript scheduling boundary; each invocation supplies the browser's timestamp.
type AnimationCallback = Closure<dyn FnMut(f64)>;

#[derive(Default)]
/// One browser callback owns the running application until an explicit stop or error breaks the loop.
struct Animation {
    /// Keeping this Rust closure alive keeps its JavaScript function callable between frames.
    callback: RefCell<Option<AnimationCallback>>,
}
impl Animation {
    /// Schedules exactly one asynchronous frame; failure is handled by the caller's teardown path.
    fn schedule(&self) -> Result<(), HostError> {
        let window = web_sys::window().ok_or_else(|| HostError::Message("browser window unavailable".to_owned()))?;
        let callback = self.callback.borrow();
        let callback = callback.as_ref().expect("live animation owns its callback");
        window
            .request_animation_frame(callback.as_ref().unchecked_ref())
            .map_err(|error| HostError::Message(format!("{error:?}")))?;
        Ok(())
    }
    /// Releases the callback and its application owner; no pending frame exists on this exit path.
    fn stop(&self) {
        self.callback.borrow_mut().take();
    }
}

/// Installs one self-scheduling callback, explicitly releasing its ownership cycle on every stop/error.
pub(super) fn run<State: AppState>(driver: Driver<State>) -> Result<(), HostError> {
    let animation = Rc::new(Animation::default());
    let owner = animation.clone();
    let mut driver = Some(driver);
    *animation.callback.borrow_mut() = Some(Closure::wrap(Box::new(move |milliseconds: f64| {
        let result = driver.as_mut().expect("active callback owns the application").frame(milliseconds / 1000.0);
        let result = match result {
            // Browser animation callbacks already pace both drawable and suspended windows.
            Ok(FrameOutcome::Presented | FrameOutcome::Suspended) => owner.schedule(),
            Ok(FrameOutcome::Stopped) => {
                driver.take();
                Ok(())
            }
            Err(error) => Err(error),
        };
        if let Err(error) = result {
            web_sys::console::error_1(&error.to_string().into());
            driver.take();
        }
        if driver.is_none() {
            // wasm-bindgen keeps the current JS invocation alive while its Rust Closure owner is
            // dropped. Its final invocation cleanup releases the callback data after this returns.
            owner.stop();
        }
    }) as Box<dyn FnMut(f64)>));
    if let Err(error) = animation.schedule() {
        animation.stop();
        return Err(error);
    }
    Ok(())
}
