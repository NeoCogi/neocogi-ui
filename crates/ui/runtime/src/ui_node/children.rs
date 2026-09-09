//
// Copyright 2023-Present (c) Raja Lehtihet & Wael El Oraiby
//
// Redistribution and use in source and binary forms, with or without
// modification, are permitted provided that the following conditions are met:
//
// 1. Redistributions of source code must retain the above copyright notice,
// this list of conditions and the following disclaimer.
//
// 2. Redistributions in binary form must reproduce the above copyright notice,
// this list of conditions and the following disclaimer in the documentation
// and/or other materials provided with the distribution.
//
// 3. Neither the name of the copyright holder nor the names of its contributors
// may be used to endorse or promote products derived from this software without
// specific prior written permission.
//
// THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS "AS IS"
// AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE
// IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE
// ARE DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT HOLDER OR CONTRIBUTORS BE
// LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR
// CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF
// SUBSTITUTE GOODS OR SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS
// INTERRUPTION) HOWEVER CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN
// CONTRACT, STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE)
// ARISING IN ANY WAY OUT OF THE USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE
// POSSIBILITY OF SUCH DAMAGE.
//

//! Opaque ownership of retained child nodes.

use super::Node;

/// Opaque ordered owner of unique retained child nodes.
///
/// Public code can transfer new nodes in or drop existing owners but cannot borrow attached nodes,
/// recover a removed owner, inspect runtime identity, or reparent a child. Framework-created
/// [`crate::ContainerLayoutCtx`] provides scoped geometry operations without weakening those
/// ownership rules.
pub struct Children {
    /// Unique mounted owners, available only to framework traversal.
    pub(crate) nodes: Vec<Node>,
    /// Topology edits invalidate cached geometry even when widget update returns `NoChange`.
    #[cfg(test)]
    pub(crate) measurement_dirty: bool,
}

impl Children {
    /// Creates an empty child collection.
    pub const fn new() -> Self {
        // No backing allocation is created until the first node is inserted.
        Self {
            nodes: Vec::new(),
            #[cfg(test)]
            measurement_dirty: false,
        }
    }

    /// Returns the number of owned child nodes.
    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    /// Returns whether this collection owns no children.
    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    /// Appends one still-unmounted node and commits this collection as its owner.
    #[cfg(test)]
    pub(crate) fn push(&mut self, node: Node) {
        // Moving the unique Node into the vector establishes this collection as its owner.
        self.nodes.push(node);
    }

    /// Inserts a node at `index`, returning it unchanged when the index exceeds `len`.
    #[allow(clippy::result_large_err)] // The exact unboxed owner is the failure value by contract.
    pub(crate) fn insert(&mut self, index: usize, node: Node) -> Result<(), Node> {
        // Validate before consuming `node`, preserving the exact owner when the index is invalid.
        if index > self.nodes.len() {
            return Err(node);
        }
        self.nodes.insert(index, node);
        Ok(())
    }

    /// Drops the indexed child owner and reports whether one existed.
    pub(crate) fn remove_drop(&mut self, index: usize) -> bool {
        // Do not manufacture a detached-node path: a successful removal drops the owner in place.
        if index >= self.nodes.len() {
            return false;
        }
        self.nodes.remove(index);
        true
    }

    /// Drops every currently owned child.
    pub(crate) fn clear(&mut self) {
        if self.nodes.is_empty() {
            return;
        }
        self.nodes.clear();
    }

    /// Replaces all children in iterator order, dropping the previous owners.
    #[cfg(test)]
    pub(crate) fn replace(&mut self, nodes: impl IntoIterator<Item = Node>) {
        // Collect the replacement sequence once, then drop the previous vector and its subtrees.
        let nodes: Vec<_> = nodes.into_iter().collect();
        self.nodes = nodes;
    }

    /// Iterates children for framework traversal without making attached nodes public.
    pub(crate) fn iter(&self) -> impl DoubleEndedIterator<Item = &Node> {
        self.nodes.iter()
    }

    /// Iterates children mutably for framework traversal.
    pub(crate) fn iter_mut(&mut self) -> impl DoubleEndedIterator<Item = &mut Node> {
        self.nodes.iter_mut()
    }

    /// Returns one child for framework-only derived-layout inspection.
    pub(crate) fn get(&self, index: usize) -> Option<&Node> {
        self.nodes.get(index)
    }

    /// Returns one child for framework-only layout/traversal.
    pub(crate) fn get_mut(&mut self, index: usize) -> Option<&mut Node> {
        self.nodes.get_mut(index)
    }
}

impl Default for Children {
    fn default() -> Self {
        Self::new()
    }
}

impl FromIterator<Node> for Children {
    fn from_iter<T: IntoIterator<Item = Node>>(iter: T) -> Self {
        Self {
            nodes: iter.into_iter().collect(),
            #[cfg(test)]
            measurement_dirty: false,
        }
    }
}

#[cfg(test)]
mod handle_tests {
    use super::*;
    use std::{cell::RefCell, rc::Rc};
    use crate::ChildrenHandle;

    /// Builds a small node without depending on container construction during owner tests.
    fn text_node(label: &str) -> Node {
        // TextBlock construction gives the test a real typed leaf widget runtime, ensuring
        // these checks exercise the same drop path used by application nodes.
        crate::TextBlock::create(crate::TextBlockParameters::new(label)).1
    }

    #[test]
    fn weak_handle_never_keeps_the_collection_alive() {
        let children = Rc::new(RefCell::new([text_node("child")].into_iter().collect()));
        let handle = ChildrenHandle::new(&children);

        assert_eq!(handle.len(), Some(1));
        drop(children);
        assert_eq!(handle.len(), None);
    }

    #[test]
    fn failed_mutation_preserves_the_exact_node_owner() {
        let children = Rc::new(RefCell::new(Children::new()));
        let handle = ChildrenHandle::new(&children);
        let candidate = text_node("candidate");
        let candidate_id = candidate.id();

        let rejected = {
            let _borrow = children.borrow_mut();
            handle.try_push(candidate).expect_err("an active traversal borrow must reject mounted mutation")
        };

        assert_eq!(rejected.id(), candidate_id);
        assert_eq!(handle.len(), Some(0));
    }
}
