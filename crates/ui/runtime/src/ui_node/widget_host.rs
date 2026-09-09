//! Runtime authority shared by mounted widget objects and retained node services.

use neocogi_ui_contracts::*;
use neocogi_com::{ComPtr, NcStatus, com_object};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

/// Phase state contains no tree or application references, so widget retention cannot cycle.
#[derive(Default)]
pub(crate) struct HostState {
    /// Final writer release can retire a checked-out subtree during observation. Ownership waits here only until that phase ends.
    pub(crate) retired: RefCell<Vec<crate::Children>>,
    /// Construction is reached weakly so widgets cannot keep the owning runtime alive.
    pub(crate) factory: RefCell<std::rc::Weak<super::node_factory::NodeFactory>>,
    /// Logical runtime lifetime, independent of reference counts.
    pub(crate) closed: Cell<bool>,
    /// Layout and paint temporarily forbid setters even on otherwise accessible widgets.
    pub(crate) observing: Cell<bool>,
    /// First component failure in the current legacy traversal; never submitted as a valid frame.
    pub(crate) error: Cell<Option<NcStatus>>,
}

impl HostState {
    /// Defers destruction only while setters or close are observationally forbidden.
    pub(crate) fn retire(&self, children: crate::Children) {
        if self.observing.get() && !self.closed.get() {
            self.retired.borrow_mut().push(children);
        } else {
            drop(children);
        }
    }

    /// Preserves the first failure so later callbacks cannot accidentally clear it.
    pub(crate) fn fail(&self, error: NcStatus) {
        if self.error.get().is_none() {
            self.error.set(Some(error));
        }
    }
}

#[com_object(interfaces(IWidgetHost))]
/// Lightweight owned authority shared by every widget in one runtime.
pub(crate) struct WidgetHost {
    /// Shared phase and lifetime flags; deliberately no reference to the owning runtime.
    pub(crate) state: Rc<HostState>,
}

impl IWidgetHost for WidgetHost {
    /// Exposes current metadata to independent widget implementations without granting tree ownership.
    fn assets(&self) -> Result<ComPtr<IAssetBundleAbi>, NcStatus> {
        self.check_open()?;
        let factory = self.state.factory.borrow().upgrade().ok_or(UI_E_CLOSED)?;
        let revision = factory.resources.borrow();
        Ok(revision.source().add_ref())
    }

    /// Issues a private writer and distinct read-only view for a new empty collection.
    fn create_children(&self) -> Result<ComPtr<IChildrenEditorAbi>, NcStatus> {
        self.check_mutation()?;
        let factory = self
            .state
            .factory
            .try_borrow()
            .map_err(|_| UI_E_BUSY)?
            .upgrade()
            .ok_or(UI_E_CLOSED)?;
        Ok(super::child_store::create(factory))
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
pub(crate) struct Observation<'a> {
    /// Host authority remains alive for the complete synchronous call.
    host: &'a HostState,
    /// Recursive calls restore their parent's state, not an assumed idle state.
    previous: bool,
}

impl<'a> Observation<'a> {
    /// Begins an observational measurement or paint call.
    pub(crate) fn begin(host: &'a HostState) -> Self {
        Self {
            host,
            previous: host.observing.replace(true),
        }
    }
}

impl Drop for Observation<'_> {
    /// Ends the scope on every exit path and releases any safely retired subtree.
    fn drop(&mut self) {
        self.host.observing.set(self.previous);
        if !self.previous {
            let retired = std::mem::take(&mut *self.host.retired.borrow_mut());
            // Actual widget close calls happen after the outer phase ends and without retaining a
            // RefCell guard or granting mutation during layout.
            drop(retired);
        }
    }
}
