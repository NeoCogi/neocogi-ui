//! COM implementation boundary. Rust UI storage never crosses this module's ABI.
//!
//! Retained nodes directly own COM widget capabilities. Runtime geometry remains
//! private and foreign calls receive only copied values or owned scoped services.

use microui_contracts::*;
use nc_com::{ComPtr, ComRef, IUnknown, NcCopyFromBufferOffset, NcStatus, NC_E_INVALID_ARG, com_object};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

#[macro_use]
pub(crate) mod widget;
mod button;
pub use crate::ui_node::{Grid, Linear};
pub use crate::ui_node::widgets::{Button, Checkbox, ColorSwatch, Combo, ListBox, ListItem, Number, Scrollbar, Slider, TextBlock, Textbox};
mod combo;
pub(crate) mod events;
mod input;
mod leaves;
mod node;
pub(crate) use node::ComNode;
mod children;
pub(crate) mod containers;
pub(crate) mod disclosure;
pub use crate::ui_node::Disclosure;
mod factory;
pub(crate) mod layout;
mod paint;
mod render;
mod runtime;
pub(crate) mod scroll;
pub use crate::ui_node::ScrollArea;
mod text_area;
pub use crate::ui_node::widgets::TextArea;
mod ui_factory;
pub use ui_factory::UiFactory;
pub use runtime::{Runtime, create_runtime_from_assets};
#[cfg(test)]
use runtime::create_runtime;

/// Validates a whole UTF-8 replacement before touching any live object state.
fn text(input: NcCopyFromBufferOffset<'_, u8>) -> Result<&str, NcStatus> {
    if input.offset() != 0 {
        return Err(NC_E_INVALID_ARG);
    }
    std::str::from_utf8(input.as_slice()).map_err(|_| NC_E_INVALID_ARG)
}

/// Copies canonical ABI geometry into the implementation's concrete math type.
pub(crate) fn local_rect(rect: Rect) -> crate::Recti {
    crate::rect(rect.x, rect.y, rect.width, rect.height)
}

/// Copies legacy geometry without passing a foreign crate's Rust type through COM.
pub(crate) fn copied_rect(rect: crate::Recti) -> Rect {
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
    /// Final writer release can retire a checked-out subtree during observation. Ownership waits here only until that phase ends.
    retired: RefCell<Vec<crate::Children>>,
    /// Construction is reached weakly so widgets cannot keep the owning runtime alive.
    factory: RefCell<std::rc::Weak<factory::NodeFactory>>,
    /// Logical runtime lifetime, independent of reference counts.
    closed: Cell<bool>,
    /// Layout and paint temporarily forbid setters even on otherwise accessible widgets.
    observing: Cell<bool>,
    /// First component failure in the current legacy traversal; never submitted as a valid frame.
    error: Cell<Option<NcStatus>>,
}

impl HostState {
    /// Defers destruction only while setters/close are observationally forbidden; no second live tree is created.
    fn retire(&self, children: crate::Children) {
        if self.observing.get() && !self.closed.get() {
            self.retired.borrow_mut().push(children);
        } else {
            drop(children);
        }
    }
    /// Preserves the first failure so later callbacks cannot accidentally clear it.
    fn fail(&self, error: NcStatus) {
        if self.error.get().is_none() {
            self.error.set(Some(error));
        }
    }
}

#[com_object(interfaces(IWidgetHost))]
/// Lightweight owned authority shared by every widget in one runtime.
struct WidgetHost {
    /// Shared phase/lifetime flags; deliberately no reference to the owning runtime.
    state: Rc<HostState>,
}

impl IWidgetHost for WidgetHost {
    /// Exposes current metadata to independent widget implementations without granting tree ownership.
    fn assets(&self) -> Result<ComPtr<IAssetBundleAbi>, NcStatus> {
        self.check_open()?;
        let factory = self.state.factory.borrow().upgrade().ok_or(UI_E_CLOSED)?;
        let revision = factory.resources.current();
        Ok(revision.source().add_ref())
    }
    /// Issues a private writer and distinct read-only view for a new empty collection.
    fn create_children(&self) -> Result<ComPtr<IChildrenEditorAbi>, NcStatus> {
        self.check_mutation()?;
        let factory = self.state.factory.try_borrow().map_err(|_| UI_E_BUSY)?.upgrade().ok_or(UI_E_CLOSED)?;
        Ok(children::create(factory))
    }
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
        if !self.previous {
            let retired = std::mem::take(&mut *self.host.retired.borrow_mut());
            // Actual widget close calls happen after the outer phase ends and
            // without retaining a RefCell guard or granting mutation during layout.
            drop(retired);
        }
    }
}

#[cfg(test)]
mod tests;
