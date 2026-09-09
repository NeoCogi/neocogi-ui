//! Weak topology editing used only by native algorithm fixtures.
use std::{
    cell::RefCell,
    rc::{Rc, Weak},
};
use crate::{Node, Children};
/// Weak, opaque capability for editing a container's ordered children.
///
/// [`crate::ContainerWidget::bind_children`] supplies this handle during construction. Keeping or
/// cloning it never keeps a removed subtree alive. Edits fail during traversal or after destruction;
/// ownership-taking failures return the original input, and successful edits invalidate geometry.
///
/// Keep the handle private when the container has per-child layout metadata. Update that metadata
/// only after a successful edit, before returning control to the runtime. A default handle is
/// unbound and rejects edits. Attached nodes can never be borrowed, detached, or reparented.
#[derive(Clone, Default)]
pub struct ChildrenHandle {
    /// The generic container remains the sole persistent owner of this collection.
    cell: Weak<RefCell<Children>>,
}

impl ChildrenHandle {
    /// Creates a non-owning topology capability for `children`.
    ///
    /// The downgraded reference is intentional: application-facing container state may outlive a
    /// particular access closure, but it must never keep a removed container or its descendants
    /// alive. Every operation therefore upgrades and borrows the collection afresh.
    pub(crate) fn new(children: &Rc<RefCell<Children>>) -> Self {
        // Store only a Weak reference so the Container remains the collection's lifetime owner.
        Self { cell: Rc::downgrade(children) }
    }

    /// Runs a crate-internal atomic metadata/topology update while preserving `input` on failure.
    pub(crate) fn try_update_with<I, R>(&self, input: I, f: impl FnOnce(&mut Children, I) -> R) -> Result<R, I> {
        // Built-in containers use this scoped operation to update child ownership and their
        // index-matched edge metadata under one state closure. The collection borrow never escapes
        // into public application code.
        let Some(owner) = self.cell.upgrade() else {
            return Err(input);
        };
        let Ok(mut children) = owner.try_borrow_mut() else {
            return Err(input);
        };
        let result = f(&mut children, input);
        // The closure may replace the complete Children value; mark the final installed collection.
        // A rejected index can conservatively invalidate too, without risking stale geometry.
        children.measurement_dirty = true;
        Ok(result)
    }

    /// Reports the current child count, or `None` when storage is unavailable.
    pub fn len(&self) -> Option<usize> {
        // Read access is checked because update and paint traversal may hold a mutable collection
        // borrow while visiting descendants.
        let owner = self.cell.upgrade()?;
        let children = owner.try_borrow().ok()?;
        Some(children.len())
    }

    /// Reports whether the collection is empty, or `None` when storage is unavailable.
    pub fn is_empty(&self) -> Option<bool> {
        // Reuse `len` so liveness and borrow-conflict behavior has one implementation.
        self.len().map(|len| len == 0)
    }

    /// Appends one unmounted node, returning the exact owner when access is unavailable.
    #[allow(clippy::result_large_err)] // Ownership-transfer failures must preserve the unboxed node.
    pub fn try_push(&self, node: Node) -> Result<(), Node> {
        self.try_update_with(node, Children::push)
    }

    /// Inserts at `index`; unavailable access or an index greater than `len` returns the node.
    #[allow(clippy::result_large_err)] // Same unique-owner contract as append.
    pub fn try_insert(&self, index: usize, node: Node) -> Result<(), Node> {
        self.try_update_with(node, |children, node| children.insert(index, node))?
    }

    /// Drops one indexed subtree; `None` means unavailable, `Some(false)` means no such index.
    pub fn try_remove_drop(&self, index: usize) -> Option<bool> {
        self.try_update_with(index, Children::remove_drop).ok()
    }

    /// Drops all child owners, or returns `None` when access is unavailable.
    pub fn try_clear(&self) -> Option<()> {
        self.try_update_with((), |children, ()| children.clear()).ok()
    }

    /// Replaces all children, returning the unconsumed iterator when access is unavailable.
    ///
    /// Collection happens only after acquiring exclusive access. The replacement is collected
    /// before old owners are dropped, so iterator panics leave the installed children intact.
    pub fn try_replace<I: IntoIterator<Item = Node>>(&self, nodes: I) -> Result<(), I> {
        self.try_update_with(nodes, Children::replace)
    }
}
