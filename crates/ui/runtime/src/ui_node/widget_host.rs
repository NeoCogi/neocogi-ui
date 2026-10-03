//! Runtime authority shared by mounted widget objects and retained node services.

use neocogi_ui_core_contracts::*;
use neocogi_com::{ComPtr, NcStatus, com_object};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

/// Phase state contains no tree or application references, so widget retention cannot cycle.
pub(crate) struct HostState {
    /// Replaceable host service shared by independent COM widgets; no borrow spans a request.
    pub(crate) clipboard: RefCell<Option<ComPtr<neocogi_ui_core_contracts::ITextClipboardAbi>>>,
    /// Topology edits invalidate the runtime's reusable descendant-first widget snapshot.
    pub(crate) dispatch_order_changed: Cell<bool>,
    /// Monotonically increasing epoch counter incremented whenever any widget or topology changes geometry.
    pub(crate) dirty_generation: Cell<u64>,
    /// Construction is reached weakly so widgets cannot keep the owning runtime alive.
    pub(crate) factory: RefCell<std::rc::Weak<super::node_factory::NodeFactory>>,
    /// Logical runtime lifetime, independent of reference counts.
    pub(crate) closed: Cell<bool>,
    /// Layout and paint temporarily forbid setters even on otherwise accessible widgets.
    pub(crate) observing: Cell<bool>,
    /// First component failure in the current legacy traversal; never submitted as a valid frame.
    pub(crate) error: Cell<Option<NcStatus>>,
}

impl Default for HostState {
    fn default() -> Self {
        Self {
            clipboard: RefCell::default(),
            dispatch_order_changed: Cell::new(true),
            dirty_generation: Cell::new(1),
            factory: RefCell::default(),
            closed: Cell::new(false),
            observing: Cell::new(false),
            error: Cell::new(None),
        }
    }
}

impl HostState {
    /// Bumps the dirty generation counter when a component mutates or topology changes.
    pub(crate) fn mark_dirty(&self) {
        self.dirty_generation.set(self.dirty_generation.get().wrapping_add(1));
    }

    /// Preserves the first failure so later callbacks cannot accidentally clear it.
    pub(crate) fn fail(&self, error: NcStatus) {
        if self.error.get().is_none() {
            self.error.set(Some(error));
        }
        self.mark_dirty();
    }
}

#[com_object(interfaces(IWidgetHost))]
/// Lightweight owned authority shared by every widget in one runtime.
pub(crate) struct WidgetHost {
    /// Shared phase and lifetime flags; deliberately no reference to the owning runtime.
    pub(crate) state: Rc<HostState>,
}

impl IWidgetHost for WidgetHost {
    /// Clones only the installed platform capability, without exposing retained runtime state.
    fn clipboard(&self) -> Result<ComPtr<neocogi_ui_core_contracts::ITextClipboardAbi>, NcStatus> {
        self.check_mutation()?;
        self.state.clipboard.borrow().clone().ok_or(neocogi_com::NC_E_NOT_FOUND)
    }
    /// Exposes current metadata to independent widget implementations without granting tree ownership.
    fn assets(&self) -> Result<ComPtr<IAssetBundleAbi>, NcStatus> {
        self.check_open()?;
        let factory = self.state.factory.borrow().upgrade().ok_or(neocogi_com::NC_E_DROPPED)?;
        let revision = factory.resources.borrow();
        Ok(revision.source().add_ref())
    }

    /// Issues a private writer and distinct read-only view for a new empty collection.
    fn create_children(&self) -> Result<neocogi_com::ComBox<IChildrenEditorAbi>, NcStatus> {
        self.check_mutation()?;
        let factory = self.state.factory.try_borrow().map_err(|_| UI_E_BUSY)?.upgrade().ok_or(neocogi_com::NC_E_DROPPED)?;
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
    /// Signals to the host that a mounted component has committed a geometry or layout change.
    fn mark_dirty(&self) -> Result<(), NcStatus> {
        self.check_open()?;
        self.state.mark_dirty();
        Ok(())
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
    /// Restores observation authority on every exit path.
    fn drop(&mut self) {
        self.host.observing.set(self.previous);

    }
}
