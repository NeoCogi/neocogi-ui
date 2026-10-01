//! One authoritative owned child collection with separate read/edit COM capabilities.
use neocogi_ui_core_contracts::*;
use neocogi_com::{ComPtr, IUnknown, NcStatus, com_object};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};
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
    /// Checks the actual owned descendants without entering any COM method.
    /// A checked-out traversal is busy: it must return ownership before removal.
    pub(super) fn ensure_idle(&self) -> Result<(), NcStatus> {
        if self.closed.get() { return Ok(()); }
        let storage = self.nodes.try_borrow().map_err(|_| UI_E_BUSY)?;
        let nodes = storage.as_ref().ok_or(UI_E_BUSY)?;
        for node in nodes.iter() { node.data.ensure_idle()?; }
        Ok(())
    }

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
            if let super::mounted_node::NodeLayoutKind::Branch { children, .. } = &node.data.layout {
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
    /// Final writer release destroys the complete idle collection synchronously.
    fn abandon(&self) {
        self.closed.set(true);
        self.dirty.set(false);
        let nodes = self.nodes.borrow_mut().take().expect("removal requires an idle child collection");
        drop(nodes);
        self.count.set(0);
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
        let nodes = self.nodes.take().expect("active transaction owns its nodes until drop");
        debug_assert!(!self.store.closed.get(), "an active collection cannot be destroyed");
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
    fn child(&self, index: u32) -> Result<neocogi_com::WeakComBox<IWidgetAbi>, NcStatus> {
        let nodes = self.store.take()?;
        let node = nodes.get(index as usize).ok_or(neocogi_com::NC_E_OUT_OF_RANGE)?;
        Ok(node.data.widget.downgrade())
    }
}
impl Drop for ChildView {
    /// Unregisters before its canonical COM identity can be reused by another allocation.
    fn drop(&mut self) {
        self.store.factory.children.borrow_mut().remove(&self.store.identity.get());
    }
}

#[com_object(interfaces(IChildrenEditor), ownership = exclusive)]
/// Private writer; retaining the public view does not grant metadata-bypassing edits.
struct ChildEditor {
    /// Actual tree owner shared with the separate read-only capability.
    store: Rc<ChildStore>,
    /// Keeps the registered read-only identity alive while editing remains possible.
    view: ComPtr<IChildrenAbi>,
}

/// Creates both capabilities around one empty child sequence and registers only its view.
pub(super) fn create(factory: Rc<NodeFactory>) -> neocogi_com::ComBox<IChildrenEditorAbi> {
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
    let identity = view.query::<IUnknown>().expect("every COM object has IUnknown").as_raw().object as usize;
    store.identity.set(identity);
    factory.children.borrow_mut().insert(identity, Rc::downgrade(&store));
    ChildEditor::into_box(ChildEditor { store, view })
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
    fn insert(&self, index: u32, widget: neocogi_com::ComBox<IWidgetAbi>) -> Result<(), (NcStatus, neocogi_com::ComBox<IWidgetAbi>)> {
        // Validation borrows the destination only; rejection returns the candidate.
        let prepared = (|| {
            self.store.factory.host.check_mutation()?;
            self.store.factory.check_edit()?;
            let mut nodes = self.store.take()?;
            if index as usize > nodes.len() || nodes.len() == u32::MAX as usize {
                return Err(neocogi_com::NC_E_OUT_OF_RANGE);
            }
            // The metadata provider has prepared its state. Reserve the owning
            // list too, so no Vec growth remains after mount_node consumes the token.
            nodes.deref_mut().nodes.try_reserve(1).map_err(|_| neocogi_com::NC_E_OUT_OF_MEMORY)?;
            Ok(nodes)
        })();
        let mut nodes = match prepared {
            Ok(nodes) => nodes,
            Err(status) => return Err((status, widget)),
        };
        let node = self.store.factory.mount_node(widget, Some(&self.store))?;
        nodes
            .insert(index as usize, node)
            .unwrap_or_else(|_| unreachable!("validated index cannot change during an owned transaction"));
        self.store.dirty.set(true);
        Ok(())
    }
    /// Validates the entire replacement before committing removal of the old subtree.
    fn replace(&self, index: u32, widget: neocogi_com::ComBox<IWidgetAbi>) -> Result<(), (NcStatus, neocogi_com::ComBox<IWidgetAbi>)> {
        // Validation borrows the destination only; rejection returns the candidate.
        let prepared = (|| {
            self.store.factory.host.check_mutation()?;
            self.store.factory.check_edit()?;
            let nodes = self.store.take()?;
            if index as usize >= nodes.len() {
                return Err(neocogi_com::NC_E_OUT_OF_RANGE);
            }
            nodes.get(index as usize).expect("validated index").data.ensure_idle()?;
            Ok(nodes)
        })();
        let mut nodes = match prepared {
            Ok(nodes) => nodes,
            Err(status) => return Err((status, widget)),
        };
        let node = self.store.factory.mount_node(widget, Some(&self.store))?;
        let previous = std::mem::replace(nodes.get_mut(index as usize).expect("validated index"), node);
        self.store.dirty.set(true);
        drop(previous);
        Ok(())
    }
    /// Removes and destroys the indexed owner; no fallible close phase follows.
    fn remove(&self, index: u32) -> Result<(), NcStatus> {
        self.store.factory.host.check_mutation()?;
        self.store.factory.check_edit()?;
        let mut nodes = self.store.take()?;
        if index as usize >= nodes.len() {
            return Err(neocogi_com::NC_E_OUT_OF_RANGE);
        }
        nodes.get(index as usize).expect("validated index").data.ensure_idle()?;
        if !nodes.remove_drop(index as usize) {
            return Err(neocogi_com::NC_E_OUT_OF_RANGE);
        }
        self.store.dirty.set(true);
        Ok(())
    }
    /// Clears ordinary child ownership without closing the collection itself.
    fn clear(&self) -> Result<(), NcStatus> {
        self.store.factory.host.check_mutation()?;
        self.store.factory.check_edit()?;
        let mut nodes = self.store.take()?;
        // Validate every child before dropping any, preserving the collection
        // unchanged if one descendant is executing a recursive removal attempt.
        for node in nodes.iter() { node.data.ensure_idle()?; }
        if !nodes.is_empty() {
            self.store.dirty.set(true);
            nodes.clear();
        }
        Ok(())
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
