//! One authoritative owned child collection with separate read/edit COM capabilities.
use crate::com::*;
use microui_contracts::*;
use nc_com::{ComPtr, ComRef, IUnknown, NcStatus, com_object};
use std::{cell::{Cell, RefCell}, rc::Rc};
use super::node_factory::NodeFactory;
use crate::Children;
use std::{
    ops::{Deref, DerefMut},
    rc::Weak,
};

/// Shared tree storage; neither the weak registry nor the read-only view duplicates edges.
pub(crate) struct ChildStore {
    /// Construction authority contains no root tree or owning runtime.
    pub(super) factory: Rc<NodeFactory>,
    /// Traversals take ownership, leaving no RefCell guard across component calls.
    nodes: RefCell<Option<Children>>,
    /// Read-only count remains stable while a layout traversal owns the actual nodes.
    count: Cell<u32>,
    /// Sticky topology marker consumed by the owning retained node.
    pub(super) dirty: Cell<bool>,
    /// One-way ownership claim prevents two containers sharing this topology.
    pub(super) claimed: Cell<bool>,
    /// Logical collection closure is independent of retained COM references.
    closed: Cell<bool>,
    /// Canonical view address is a private registry key, never an ABI identifier.
    identity: Cell<usize>,
}

impl ChildStore {
    /// Validates one unclaimed container collection before any widget or topology ownership commits.
    pub(super) fn validate_attachment(self: &Rc<Self>, parent: Option<&Rc<Self>>) -> Result<(), NcStatus> {
        self.open()?;
        if self.claimed.get() {
            return Err(UI_E_OWNER);
        }
        if let Some(parent) = parent {
            self.reject_cycle(parent)?;
        }
        Ok(())
    }

    /// Walks the authoritative prospective subtree; no foreign calls or duplicate edge cache are needed.
    fn reject_cycle(self: &Rc<Self>, parent: &Rc<Self>) -> Result<(), NcStatus> {
        if Rc::ptr_eq(self, parent) {
            return Err(UI_E_OWNER);
        }
        let nodes = self.take()?;
        for node in nodes.iter() {
            if let super::mounted_node::NodeLayoutKind::Branch { children, .. } = &node.data.com().layout {
                children.reject_cycle(parent)?;
            }
        }
        Ok(())
    }
    /// Rejects reads after either collection closure or runtime shutdown.
    pub(super) fn open(&self) -> Result<(), NcStatus> {
        if self.closed.get() || self.factory.state.closed.get() {
            Err(UI_E_CLOSED)
        } else {
            Ok(())
        }
    }
    /// Takes exclusive ownership for a traversal or topology transaction.
    pub(super) fn take(self: &Rc<Self>) -> Result<ChildTransaction, NcStatus> {
        self.open()?;
        self.take_storage()
    }
    /// Teardown also uses this path after the host has logically closed.
    fn take_storage(self: &Rc<Self>) -> Result<ChildTransaction, NcStatus> {
        let nodes = self.nodes.try_borrow_mut().map_err(|_| UI_E_BUSY)?.take().ok_or(UI_E_BUSY)?;
        Ok(ChildTransaction { store: self.clone(), nodes: Some(nodes) })
    }
    /// Permanently closes all descendants; active traversals reject before mutation.
    pub(super) fn close(self: &Rc<Self>) -> Result<ChildEditResult, NcStatus> {
        if self.closed.get() {
            return Ok(ChildEditResult { teardown: nc_com::NC_OK });
        }
        let mut nodes = self.take_storage()?;
        let errors = TeardownErrors::new(&self.factory.state);
        self.closed.set(true);
        self.dirty.set(false);
        nodes.clear();
        Ok(errors.result())
    }

    /// Final writer release closes immediately, deferring owned-node clearing to an active transaction's Drop when necessary.
    fn abandon(&self) {
        self.closed.set(true);
        self.dirty.set(false);
        let nodes = self.nodes.borrow_mut().take();
        if let Some(nodes) = nodes {
            // Logical closure is immediate; actual close waits for any outer
            // observational phase, without a borrowed collection or live tree edge.
            self.factory.state.retire(nodes);
            self.count.set(0);
            *self.nodes.borrow_mut() = Some(Children::new());
        }
    }
}

/// Owned collection transaction restored on every return path, including errors.
pub(crate) struct ChildTransaction {
    /// Vacant slot and copied count updated when this transaction ends.
    store: Rc<ChildStore>,
    /// Sole owned child sequence for the duration of this operation.
    nodes: Option<Children>,
}
impl Deref for ChildTransaction {
    type Target = Children;
    /// Borrows only the locally owned collection, never a RefCell guard.
    fn deref(&self) -> &Children {
        self.nodes.as_ref().expect("active transaction owns its nodes")
    }
}
impl DerefMut for ChildTransaction {
    /// Mutates the locally owned collection without exporting it through COM.
    fn deref_mut(&mut self) -> &mut Children {
        self.nodes.as_mut().expect("active transaction owns its nodes")
    }
}
impl Drop for ChildTransaction {
    /// Returns ownership and the coherent copied count before a later call can enter.
    fn drop(&mut self) {
        let mut nodes = self.nodes.take().expect("active transaction owns its nodes until drop");
        if self.store.closed.get() {
            self.store.factory.state.retire(nodes);
            nodes = Children::new();
        }
        self.store
            .count
            .set(u32::try_from(nodes.len()).expect("insertion enforces the ABI child-count limit"));
        *self.store.nodes.borrow_mut() = Some(nodes);
    }
}

#[com_object(interfaces(IChildren))]
/// Read-only tree capability; its identity intentionally exposes no editor interface.
struct ChildView {
    /// One shared authoritative node sequence, kept safe by logical closure checks.
    store: Rc<ChildStore>,
}
impl IChildren for ChildView {
    /// Returns stable copied cardinality even when a scoped layout owns the nodes.
    fn child_count(&self) -> Result<u32, NcStatus> {
        self.store.open()?;
        Ok(self.store.count.get())
    }
    /// Returns one actual child projection; checked-out collections report BUSY.
    fn child(&self, index: u32) -> Result<ComPtr<IWidgetAbi>, NcStatus> {
        let nodes = self.store.take()?;
        let node = nodes.get(index as usize).ok_or(nc_com::NC_E_OUT_OF_RANGE)?;
        Ok(node.data.com().widget.clone())
    }
}
impl Drop for ChildView {
    /// Unregisters before its canonical COM identity can be reused by another allocation.
    fn drop(&mut self) {
        self.store.factory.children.borrow_mut().remove(&self.store.identity.get());
    }
}

#[com_object(interfaces(IChildrenEditor))]
/// Private writer; retaining the public view does not grant metadata-bypassing edits.
struct ChildEditor {
    /// Actual tree owner shared with the separate read-only capability.
    store: Rc<ChildStore>,
    /// Keeps the registered read-only identity alive while editing remains possible.
    view: ComPtr<IChildrenAbi>,
}

/// Creates both capabilities around one empty child sequence and registers only its view.
pub(super) fn create(factory: Rc<NodeFactory>) -> ComPtr<IChildrenEditorAbi> {
    let store = Rc::new(ChildStore {
        factory: factory.clone(),
        nodes: RefCell::new(Some(Children::new())),
        count: Cell::new(0),
        dirty: Cell::new(false),
        closed: Cell::new(false),
        claimed: Cell::new(false),
        identity: Cell::new(0),
    });
    let view: ComPtr<IChildrenAbi> = ChildView::from_value(ChildView { store: store.clone() })
        .query()
        .expect("view declares IChildren");
    let identity = view.query::<IUnknown>().expect("every COM object has IUnknown").as_ptr() as usize;
    store.identity.set(identity);
    factory.children.borrow_mut().insert(identity, Rc::downgrade(&store));
    ChildEditor::from_value(ChildEditor { store, view })
        .query()
        .expect("editor declares IChildrenEditor")
}

impl IChildrenEditor for ChildEditor {
    /// Shares only the read capability; QI on it cannot recover this private editor.
    fn view(&self) -> Result<ComPtr<IChildrenAbi>, NcStatus> {
        self.store.open()?;
        Ok(self.view.clone())
    }
    /// Validates the slot and candidate before consuming the child's mount-once claim.
    fn insert(&self, index: u32, widget: ComRef<'_, IWidgetAbi>) -> Result<ChildEditResult, NcStatus> {
        self.store.factory.host.check_mutation()?;
        self.store.factory.check_edit()?;
        let mut nodes = self.store.take()?;
        if index as usize > nodes.len() || nodes.len() == u32::MAX as usize {
            return Err(nc_com::NC_E_OUT_OF_RANGE);
        }
        let node = self.store.factory.mount_node(widget, Some(&self.store))?;
        nodes
            .insert(index as usize, node)
            .unwrap_or_else(|_| unreachable!("validated index cannot change during an owned transaction"));
        self.store.dirty.set(true);
        Ok(ChildEditResult { teardown: nc_com::NC_OK })
    }
    /// Validates the entire replacement before committing removal of the old subtree.
    fn replace(&self, index: u32, widget: ComRef<'_, IWidgetAbi>) -> Result<ChildEditResult, NcStatus> {
        self.store.factory.host.check_mutation()?;
        self.store.factory.check_edit()?;
        let mut nodes = self.store.take()?;
        if index as usize >= nodes.len() {
            return Err(nc_com::NC_E_OUT_OF_RANGE);
        }
        let node = self.store.factory.mount_node(widget, Some(&self.store))?;
        let errors = TeardownErrors::new(&self.store.factory.state);
        let previous = std::mem::replace(nodes.get_mut(index as usize).expect("validated index"), node);
        self.store.dirty.set(true);
        drop(previous);
        Ok(errors.result())
    }
    /// Commits removal before reporting a component's fallible close result.
    fn remove(&self, index: u32) -> Result<ChildEditResult, NcStatus> {
        self.store.factory.host.check_mutation()?;
        self.store.factory.check_edit()?;
        let mut nodes = self.store.take()?;
        if index as usize >= nodes.len() {
            return Err(nc_com::NC_E_OUT_OF_RANGE);
        }
        let errors = TeardownErrors::new(&self.store.factory.state);
        if !nodes.remove_drop(index as usize) {
            return Err(nc_com::NC_E_OUT_OF_RANGE);
        }
        self.store.dirty.set(true);
        Ok(errors.result())
    }
    /// Clears ordinary child ownership without closing the collection itself.
    fn clear(&self) -> Result<ChildEditResult, NcStatus> {
        self.store.factory.host.check_mutation()?;
        self.store.factory.check_edit()?;
        let mut nodes = self.store.take()?;
        let errors = TeardownErrors::new(&self.store.factory.state);
        if !nodes.is_empty() {
            self.store.dirty.set(true);
            nodes.clear();
        }
        Ok(errors.result())
    }
    /// Explicit closure respects observation phases but permits runtime shutdown cleanup.
    fn close(&self) -> Result<ChildEditResult, NcStatus> {
        if self.store.closed.get() {
            return self.store.close();
        }
        self.store.factory.check_edit()?;
        match self.store.factory.host.check_mutation() {
            Ok(()) | Err(UI_E_CLOSED) => {}
            Err(error) => return Err(error),
        }
        self.store.close()
    }
}
impl Drop for ChildEditor {
    /// Final writer release logically closes children without invoking application callbacks.
    fn drop(&mut self) {
        self.store.abandon();
    }
}

/// Weak registry entries authenticate host-created views without retaining another tree.
pub(crate) type ChildRegistry = std::collections::BTreeMap<usize, Weak<ChildStore>>;

/// Isolates one committed edit's teardown errors while retaining an enclosing failure.
struct TeardownErrors<'a> {
    /// Shared first-error latch, never a borrowed tree or application state.
    state: &'a HostState,
    /// Earlier enclosing traversal failure restored when this local report is complete.
    previous: Option<NcStatus>,
}
impl<'a> TeardownErrors<'a> {
    /// Starts an independent report without losing an earlier enclosing error.
    fn new(state: &'a HostState) -> Self {
        Self {
            state,
            previous: state.error.replace(None),
        }
    }
    /// Reports this edit's close result after topology has committed.
    fn result(&self) -> ChildEditResult {
        ChildEditResult {
            teardown: self.state.error.get().unwrap_or(nc_com::NC_OK),
        }
    }
}
impl Drop for TeardownErrors<'_> {
    /// Earlier failures retain first-error precedence in the outer transaction.
    fn drop(&mut self) {
        if let Some(error) = self.previous {
            self.state.error.set(Some(error));
        }
    }
}
