//! COM implementation boundary. Rust UI storage never crosses this module's ABI.
//!
//! Retained nodes directly own COM widget capabilities. Runtime geometry remains
//! private and foreign calls receive only copied values or owned scoped services.

use microui_contracts::*;
use nc_com::{ComPtr, ComRef, IUnknown, NcCopyFromBufferOffset, NcStatus, NC_E_INVALID_ARG, com_class};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

#[macro_use]
pub(crate) mod leaf;
mod button;
mod leaves;
mod node;
mod input;
pub(crate) use node::ComNode;
mod paint;
mod render;
mod runtime;
pub use runtime::create_runtime;

/// Validates a whole UTF-8 replacement before touching any live object state.
fn text(input: NcCopyFromBufferOffset<'_, u8>) -> Result<&str, NcStatus> {
    if input.offset() != 0 {
        return Err(NC_E_INVALID_ARG);
    }
    std::str::from_utf8(input.as_slice()).map_err(|_| NC_E_INVALID_ARG)
}

/// Converts the canonical ABI rectangle at the temporary retained-engine boundary.
fn local_rect(rect: Rect) -> crate::prelude::Recti {
    crate::rect(rect.x, rect.y, rect.width, rect.height)
}

/// Copies legacy geometry without passing a foreign crate's Rust type through COM.
fn copied_rect(rect: crate::prelude::Recti) -> Rect {
    Rect {
        x: rect.x,
        y: rect.y,
        width: rect.width,
        height: rect.height,
    }
}

/// Phase state contains no tree or application references, so widget retention cannot cycle.
#[derive(Default)]
struct HostState {
    /// Logical runtime lifetime, independent of reference counts.
    closed: Cell<bool>,
    /// Layout and paint temporarily forbid setters even on otherwise accessible widgets.
    observing: Cell<bool>,
    /// First component failure in the current legacy traversal; never submitted as a valid frame.
    error: Cell<Option<NcStatus>>,
}

impl HostState {
    /// Preserves the first failure so later callbacks cannot accidentally clear it.
    fn fail(&self, error: NcStatus) {
        if self.error.get().is_none() {
            self.error.set(Some(error));
        }
    }
}

#[com_class(interfaces(IWidgetHost))]
/// Lightweight owned authority shared by every widget in one runtime.
struct WidgetHost {
    /// Shared phase/lifetime flags; deliberately no reference to the owning runtime.
    state: Rc<HostState>,
}

impl IWidgetHost for WidgetHost {
    /// Read-only operations remain legal in observational phases.
    fn check_open(&self) -> Result<(), NcStatus> {
        if self.state.closed.get() { Err(UI_E_CLOSED) } else { Ok(()) }
    }
    /// Authorizes mutation before a concrete setter borrows its own state.
    fn check_mutation(&self) -> Result<(), NcStatus> {
        if self.state.closed.get() {
            Err(UI_E_CLOSED)
        } else if self.state.observing.get() {
            Err(UI_E_BUSY)
        } else {
            Ok(())
        }
    }
}

/// Restores the surrounding phase even when a component returns an error.
struct Observation<'a> {
    /// Host authority remains alive for the complete synchronous call.
    host: &'a HostState,
    /// Recursive calls restore their parent's state, not an assumed idle state.
    previous: bool,
}

impl<'a> Observation<'a> {
    /// Begins an observational measurement or paint call.
    fn begin(host: &'a HostState) -> Self {
        Self {
            host,
            previous: host.observing.replace(true),
        }
    }
}

impl Drop for Observation<'_> {
    /// Ends the scope on every exit path.
    fn drop(&mut self) {
        self.host.observing.set(self.previous);
    }
}

#[cfg(test)]
mod tests;
