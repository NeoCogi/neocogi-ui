//! Runtime authority shared by mounted widget objects and retained node services.

use neocogi_ui::abi::core::*;
use neocogi_com::{ComPtr, NcStatus, com_object};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

/// Phase state contains no tree or application references, so widget retention cannot cycle.
#[derive(Default)]
pub(crate) struct HostState {
    /// Dispatch-owner capability; widget-facing host projections can only post.
    pub(crate) event_origins: neocogi_ui_support::EventOriginOwner,
    /// Replaceable service shared by distinct COM projections. Clone only the local Rc under
    /// this slot's borrow; foreign retention/release must run after that guard has ended.
    pub(crate) clipboard: RefCell<Option<Rc<ComPtr<neocogi_ui::abi::core::ITextClipboardAbi>>>>,
    /// Topology edits invalidate the runtime's reusable descendant-first widget snapshot.
    pub(crate) dispatch_order_changed: Cell<bool>,
    /// Conservative count of live dispatcher-bearing nodes, including unmounted roots.
    pub(crate) dispatcher_count: Cell<usize>,
    /// Conservative uncollected geometry notification; mutation and observation may signal it.
    pub(crate) changes_pending: Cell<bool>,
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
    /// Preserves the first failure so later callbacks cannot accidentally clear it.
    pub(crate) fn fail(&self, error: NcStatus) {
        self.changes_pending.set(true);
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
    /// Reads the active scoped origin without exposing stack control over COM.
    fn post_event(
        &self,
        receiver: neocogi_com::ComRef<'_, neocogi_ui::abi::core::IEventReceiverAbi>,
        event: neocogi_com::ComBox<neocogi_ui::abi::core::IEventAbi>,
    ) -> Result<(), (NcStatus, neocogi_com::ComBox<neocogi_ui::abi::core::IEventAbi>)> {
        if let Err(status) = self.check_mutation() {
            return Err((status, event));
        }
        self.state.event_origins.post(receiver, event)
    }
    /// Notification is not a widget mutation and remains legal during recovery/collection.
    fn mark_geometry_changed(&self) -> Result<(), NcStatus> {
        self.check_open()?;
        self.state.changes_pending.set(true);
        Ok(())
    }
    /// Clones only the installed platform capability, without exposing retained runtime state.
    fn clipboard(&self) -> Result<ComPtr<neocogi_ui::abi::core::ITextClipboardAbi>, NcStatus> {
        self.check_mutation()?;
        let installed = self.state.clipboard.borrow().clone().ok_or(neocogi_com::NC_E_NOT_FOUND)?;
        // The selected owner remains pinned even if AddRef replaces the service or closes the
        // runtime. This already-admitted lookup returns that snapshot; later calls see closure.
        Ok(ComPtr::clone(installed.as_ref()))
    }
    /// Exposes current metadata to independent widget implementations without granting tree ownership.
    fn assets(&self) -> Result<ComPtr<IAssetBundleAbi>, NcStatus> {
        self.check_open()?;
        let factory = self.state.factory.borrow().upgrade().ok_or(neocogi_com::NC_E_DROPPED)?;
        // Retain the immutable local value first; no cache borrow spans foreign AddRef.
        let revision = factory.resources.borrow().clone();
        revision.source().try_add_ref()
    }

    /// Issues a private writer and distinct read-only view for a new empty collection.
    fn create_children(&self) -> Result<neocogi_com::ComBox<IChildrenEditorAbi>, NcStatus> {
        self.check_mutation()?;
        let factory = self
            .state
            .factory
            .try_borrow()
            .map_err(|_| UI_E_BUSY)?
            .upgrade()
            .ok_or(neocogi_com::NC_E_DROPPED)?;
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
    /// Restores observation authority on every exit path.
    fn drop(&mut self) {
        self.host.observing.set(self.previous);
    }
}
