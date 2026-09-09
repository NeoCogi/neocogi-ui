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

#[cfg(not(test))]
use crate::ui_node::ComNode as NodeData;
#[cfg(test)]
use crate::test_support::node::NodeData;

use crate::math::RectExt;
use crate::{AtlasHandle, Constraints, Dimensioni, Recti, SliceInsets, Skin};

use super::{ChildParticipation, Children, NodeLayout, RuntimeNodeId};

/// One preferred-size result associated with an exact installed skin generation.
struct MeasurementEntry {
    /// Parent-supplied bounds that produced this retained preferred size.
    constraints: Constraints,
    /// Complete skin generation observed by the public widget measurement contract.
    revision: crate::theme::SkinRevision,
    /// Normalized non-negative outer size returned for this key.
    preferred: Dimensioni,
}

/// Bounded preferred-size cache owned directly by one retained node.
///
/// Four retained entries cover the small set of constraints produced by responsive parent flows.
/// Clearing preserves the vector's capacity, so warmed invalidation reuses its storage.
struct MeasurementCache {
    /// Most recent successful preferences for this node, bounded to four constraint keys.
    entries: Vec<MeasurementEntry>,
    /// Set until placement succeeds even when preferred-size measurement can be reused.
    layout_dirty: bool,
}

impl MeasurementCache {
    /// Starts without cached geometry; the first traversal must measure and place.
    fn new() -> Self {
        Self { entries: Vec::new(), layout_dirty: true }
    }

    /// Retains allocation while discarding preferences and the placement commit.
    fn invalidate(&mut self) {
        self.entries.clear();
        self.layout_dirty = true;
    }

    /// Invalidates placement alone for changed child transforms or dependent allocations.
    fn invalidate_layout(&mut self) {
        self.layout_dirty = true;
    }

    /// Reports whether placement has yet to commit for the current inputs.
    fn layout_is_dirty(&self) -> bool {
        self.layout_dirty
    }

    /// Records successful placement after the complete container result is installed.
    fn validate_layout(&mut self) {
        self.layout_dirty = false;
    }

    /// Returns a retained result only for the exact constraints and complete skin generation.
    fn lookup(&self, constraints: Constraints, revision: crate::theme::SkinRevision) -> Option<Dimensioni> {
        // SkinRevision represents all Skin fields and the paired atlas at the bundle boundary, so
        // no incomplete list of measurement-relevant fields can drift from LeafWidget::measure.
        self.entries
            .iter()
            .find(|cached| cached.constraints == constraints && cached.revision == revision)
            .map(|cached| cached.preferred)
    }

    /// Evicts the oldest constraint key when the fixed cache budget is reached.
    fn insert(&mut self, entry: MeasurementEntry) {
        if self.entries.len() == 4 {
            self.entries.remove(0);
        }
        self.entries.push(entry);
    }
}

#[cfg(test)]
use super::node_layout::advance_runtime_node_id;
use super::Transform;
#[cfg(test)]
use crate::Vec2i;

/// Common derived and transient runtime state shared by widgets and containers.
///
/// This state deliberately has no generic visibility bit. Root visibility and container-owned
/// descendant gating are separate mechanisms with different owners.
pub(crate) struct NodeRuntime {
    /// Private process-unique runtime identity.
    id: RuntimeNodeId,
    /// Persistent layout result for traversal.
    pub(crate) layout: NodeLayout,
    /// Cursor is hovering this node.
    pub(crate) hovered: bool,
    /// This node currently owns focus.
    pub(crate) focused: bool,
    /// Mouse was pressed on this node during the current input transaction.
    pub(crate) clicked: bool,
    /// Mouse is held down while this node owns focus.
    pub(crate) active: bool,
    /// Parent-layout result consumed uniformly by traversal and dispatch.
    pub(crate) participation: ChildParticipation,
    /// Persistent preferred sizes owned directly by this retained node.
    measurement: MeasurementCache,
    /// Whether this node has received its first authoritative allocation.
    placed: bool,
}

impl NodeRuntime {
    /// Returns this node's private process-unique identity.
    pub(crate) fn id(&self) -> RuntimeNodeId {
        self.id
    }

    /// Writes layout as the source of truth.
    pub(crate) fn set_layout(&mut self, layout: NodeLayout) {
        self.layout = layout;
    }

    pub(crate) fn invalidate_measurement(&mut self) {
        self.measurement.invalidate();
    }

    pub(crate) fn invalidate_layout(&mut self) {
        self.measurement.invalidate_layout();
    }

    pub(crate) fn layout_is_dirty(&self) -> bool {
        self.measurement.layout_is_dirty()
    }

    pub(crate) fn validate_layout(&mut self) {
        self.measurement.validate_layout();
        self.placed = true;
    }
}

/// Unique owning retained-tree node.
///
/// A `Node` owns one mounted COM widget and its cached layout capability. Successful insertion
/// transfers this non-cloneable owner into one [`Children`] collection. Its identity and geometry
/// remain private to traversal; applications retain checked COM interfaces. Attached nodes cannot be
/// detached or reparented: topology APIs either keep ownership in place or drop the removed
/// runtime. Build a replacement node when content must move to another parent.
pub struct Node {
    /// Common state for layout, identity, and interaction.
    pub(crate) state: NodeRuntime,
    /// Node-specific payload.
    pub(crate) data: NodeData,
}

impl Node {
    /// Mounts an already authenticated COM leaf without another widget allocation.
    pub(crate) fn com_widget(widget: crate::ui_node::ComNode) -> Self {
        // Only the algorithm harness accepts native owners; deployed nodes store this value directly.
        #[cfg(test)]
        let widget = NodeData::Com(widget);
        Self::from_data(widget)
    }

    /// Allocates the unique runtime identity and empty geometry at final ownership transfer.
    pub(crate) fn from_data(kind: NodeData) -> Self {
        // Identity is allocated once at the final owning boundary and survives every subsequent move
        // of the non-Clone Node through unmounted construction and retained insertion.
        Self {
            state: NodeRuntime {
                id: RuntimeNodeId::allocate(),
                layout: NodeLayout::default(),
                hovered: false,
                focused: false,
                clicked: false,
                active: false,
                participation: ChildParticipation::Active,
                measurement: MeasurementCache::new(),
                placed: false,
            },
            data: kind,
        }
    }

    /// Returns this node's private identity for runtime-only traversal.
    pub(crate) fn id(&self) -> RuntimeNodeId {
        self.state.id()
    }

    /// Tests retained bounds against an inherited screen-space viewport.
    pub(crate) fn intersects_clip(&self, parent_transform: Transform) -> bool {
        let allocation = self.state.layout.allocation;
        if !self.state.placed || self.data.is_measurement_dirty() {
            // New nodes and directly mutated later subtrees participate before layout commits
            // their resulting geometry.
            return true;
        }
        let content = self.state.layout.content_size;
        let bounds = Recti::new(
            allocation.x,
            allocation.y,
            allocation.width.max(content.width),
            allocation.height.max(content.height),
        );
        parent_transform.resolve(bounds).overlaps(parent_transform.clip)
    }

    /// Measures this node's desired outer size. Its parent assigns the later exact allocation.
    pub(crate) fn measure(&mut self, style: &Skin, atlas: &AtlasHandle, constraints: Constraints) -> Dimensioni {
        self.measure_with_cache_status(style, atlas, constraints).0
    }

    /// Measures and reports whether the retained result satisfied this exact query.
    pub(crate) fn measure_with_cache_status(&mut self, style: &Skin, atlas: &AtlasHandle, constraints: Constraints) -> (Dimensioni, bool) {
        let revision = style.revision();
        if let Some(cached) = self.state.measurement.lookup(constraints, revision) {
            return (cached, true);
        }

        // One policy snapshot defines framing for both leaf and container capabilities.
        // COM failures remain dirty and never enter the preferred-size cache.
        let (options, _, role) = self.data.policy();
        let frame_insets = if options.intersects(crate::WidgetOption::FRAME) {
            crate::ui_node::frame::normal_frame_visual(style, role).patch.insets.normalized()
        } else {
            SliceInsets::ZERO
        };
        let measured_content = match self
            .data
            .measure(style, atlas, crate::ui_node::frame::content_constraints(constraints, frame_insets))
        {
            Ok(size) => size,
            Err(_) => {
                self.state.invalidate_measurement();
                return (Dimensioni::default(), false);
            }
        };
        // Widgets cannot return negative geometry. Node placement policy is intentionally absent:
        // the parent consumes this desired size while resolving its own child relationship.
        let preferred_content = Dimensioni::new(measured_content.width.max(0), measured_content.height.max(0));
        let preferred = crate::ui_node::frame::outer_preferred(preferred_content, frame_insets);
        self.state.measurement.insert(MeasurementEntry { constraints, revision, preferred });
        (preferred, false)
    }

    /// Consumes widget mutation markers and invalidates each changed node's ancestor path.
    pub(crate) fn synchronize_measurement_invalidation(&mut self) -> bool {
        let mut measurement_dirty = self.data.take_measurement_dirty();
        self.with_children_mut(|children| {
            for child in children.iter_mut() {
                measurement_dirty |= child.synchronize_measurement_invalidation();
            }
        });
        if measurement_dirty {
            self.state.invalidate_measurement();
        }
        measurement_dirty
    }

    /// Reports whether this node or any descendant has uncommitted widget measurement state.
    pub(crate) fn has_measurement_dirty(&self) -> bool {
        self.data.is_measurement_dirty() || self.with_children(|children| children.iter().any(Self::has_measurement_dirty))
    }

    /// Marks this node as the local source of a preferred-size invalidation.
    pub(crate) fn mark_measurement_dirty(&mut self) {
        self.data.mark_measurement_dirty();
    }

    /// Writes layout as the source of truth.
    pub(crate) fn set_layout(&mut self, layout: NodeLayout) {
        self.state.set_layout(layout);
    }

    /// Runs `f` with this node's opaque authoritative child collection.
    pub(crate) fn with_children<R>(&self, f: impl FnOnce(&Children) -> R) -> R {
        self.data.with_children(f)
    }

    /// Visits authoritative children only while their transaction is alive.
    pub(crate) fn with_children_mut<R>(&mut self, f: impl FnOnce(&mut Children) -> R) -> Option<R> {
        self.data.with_children_mut(f)
    }

    /// Uses the cached layout capability without querying an interface again.
    pub(crate) fn is_container(&self) -> bool {
        self.data.is_container()
    }

    /// Counts this node and every retained descendant.
    #[cfg(test)]
    pub(crate) fn debug_node_count(&self) -> usize {
        self.with_children(|children| 1 + children.iter().map(Self::debug_node_count).sum::<usize>())
    }

    /// Runs a test-only immutable visitor against one matching retained node.
    #[cfg(test)]
    pub(crate) fn with_node<R>(&self, id: RuntimeNodeId, f: impl FnOnce(&Node) -> R) -> Option<R> {
        // Keep the one-shot visitor in an Option so recursive descent can move it exactly once when
        // the requested identity is reached without returning a borrow from opaque child storage.
        let mut f = Some(f);
        self.with_node_inner(id, &mut f)
    }

    #[cfg(test)]
    fn with_node_inner<R, F>(&self, id: RuntimeNodeId, f: &mut Option<F>) -> Option<R>
    where
        F: FnOnce(&Node) -> R,
    {
        if self.id() == id {
            // Runtime IDs are unique, so taking the callback here proves it cannot run twice.
            return Some(f.take().expect("node visitor invoked twice")(self));
        }
        // The child borrow remains scoped to this search and cannot escape through the callback.
        self.with_children(|children| children.iter().find_map(|child| child.with_node_inner(id, f)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::test_atlas;

    fn text_node(label: &str) -> (crate::TypedWidgetHandle<crate::TextBlock>, Node) {
        crate::TextBlock::create(crate::TextBlockParameters::new(label))
    }

    #[test]
    fn pushing_a_node_uses_node_local_child_geometry() {
        let mut layout = NodeLayout::from_parts(Recti::new(10, 20, 30, 40), Recti::new(2, 3, 20, 10), Dimensioni::new(30, 40));
        layout.children.offset = Vec2i::new(4, -5);

        let parent = Transform {
            offset: Vec2i::new(100, 200),
            clip: Recti::new(0, 0, 1000, 1000),
        };
        let child = parent.push(layout);

        assert_eq!((child.offset.x, child.offset.y), (114, 215));
        assert_eq!((child.clip.x, child.clip.y, child.clip.width, child.clip.height), (112, 223, 20, 10));
    }

    #[test]
    fn owning_nodes_allocate_once_and_preserve_identity_while_moved_or_rejected() {
        let (_, first) = text_node("first");
        let first_id = first.state.id.0.get();
        let (_, second) = text_node("second");
        let second_id = second.state.id.0.get();
        assert!(second_id > first_id, "the process-wide allocator must increase monotonically");

        let mut children = Children::new();
        let rejected = children.insert(1, first).expect_err("out-of-range insertion must reject the exact owner");
        assert_eq!(rejected.state.id.0.get(), first_id);
        assert!(children.is_empty());

        children.push(rejected);
        assert_eq!(children.nodes[0].state.id.0.get(), first_id);
    }

    #[test]
    fn measurement_reports_content_independently_of_exact_parent_allocation() {
        let (_, mut node) = text_node("same content");
        let atlas = test_atlas();
        // Resolve the measurement style from the exact handle passed to both measurement and
        // layout so same-slot resources from another atlas can never satisfy this regression.
        let style = crate::test_support::test_skin(&atlas);

        let preferred = node.measure(&style, &atlas, Constraints::unbounded());
        let mut runtime = crate::ui_node::UiRuntime::new();
        runtime.begin_update();
        runtime.layout_tree_root(&mut node, &style, atlas.clone(), Recti::new(0, 0, 300, 200), crate::UNCLIPPED_RECT);
        let measured_again = node.measure(&style, &atlas, Constraints::unbounded());

        assert_eq!((measured_again.width, measured_again.height), (preferred.width, preferred.height));
        let allocation = node.state.layout.allocation;
        assert_eq!((allocation.x, allocation.y, allocation.width, allocation.height), (0, 0, 300, 200));
    }

    #[test]
    fn runtime_identity_exhaustion_is_detected_without_mutating_the_global_allocator() {
        assert_eq!(advance_runtime_node_id(1), Some(2));
        assert_eq!(advance_runtime_node_id(u64::MAX), None);
    }

    #[test]
    fn children_topology_operations_drop_only_the_replaced_owners() {
        let (first_state, first) = text_node("first");
        let (second_state, second) = text_node("second");
        let (third_state, third) = text_node("third");
        let mut children: Children = [first, second].into_iter().collect();

        assert_eq!(children.len(), 2);
        assert!(children.remove_drop(0));
        assert!(!first_state.is_alive());
        assert!(second_state.is_alive());

        children.replace([third]);
        assert!(!second_state.is_alive());
        assert!(third_state.is_alive());

        children.clear();
        assert!(!third_state.is_alive());
        assert!(!children.remove_drop(0));
    }

    #[test]
    #[allow(clippy::result_large_err)] // The test verifies that failed mutation returns the exact owner.
    fn ownership_moving_widget_access_returns_the_same_unmounted_node_on_failure() {
        let (column_state, column_node) = crate::Linear::create(crate::LinearParameters::vertical(std::iter::empty::<Node>()));
        let (_, candidate) = text_node("candidate");
        let candidate_id = candidate.state.id.0.get();

        let rejected = column_state
            .try_read(|_| match column_state.try_update_with(candidate, |column, node| column.push(node)) {
                Err(candidate) => candidate,
                Ok(_) => panic!("the active read must prevent mutation"),
            })
            .expect("column read must be available");
        assert_eq!(rejected.state.id.0.get(), candidate_id);

        drop(column_node);
        let rejected = match column_state.try_update_with(rejected, |column, node| column.push(node)) {
            Err(rejected) => rejected,
            Ok(_) => panic!("expired state must return the input owner"),
        };
        assert_eq!(rejected.state.id.0.get(), candidate_id);
    }
}
