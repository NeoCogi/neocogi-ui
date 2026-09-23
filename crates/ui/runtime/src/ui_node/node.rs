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

use crate::ui_node::ComNode;
use crate::ui_node::mounted_node::NodeLayoutKind;
use neocogi_com::NcStatus;
use std::rc::Rc;
use crate::ui_node::mounted_node::ComponentEventDriver;

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

use super::Transform;

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
    pub(crate) data: ComNode,
}

impl Node {
    /// Copies mounted component drivers in child-first order without invoking
    /// callbacks or lending any retained-tree or child-store borrow to them.
    /// A checkout conflict is an error, never a silently skipped component.
    pub(crate) fn collect_private_event_drivers(&self, drivers: &mut Vec<Rc<ComponentEventDriver>>) -> Result<(), NcStatus> {
        match &self.data.layout {
            NodeLayoutKind::Leaf(_) => {}
            NodeLayoutKind::Branch { children, .. } => {
                let nodes = children.take()?;
                for child in nodes.iter() {
                    child.collect_private_event_drivers(drivers)?;
                }
            }
        }
        if let Some(driver) = &self.data.event_driver {
            drivers.push(driver.clone());
        }
        Ok(())
    }

    /// Mounts an already authenticated COM leaf without another widget allocation.
    pub(crate) fn com_widget(widget: crate::ui_node::ComNode) -> Self {
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
            data: widget,
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
    pub(crate) fn measure(&mut self, style: &Skin, atlas: &AtlasHandle, constraints: Constraints) -> Result<Dimensioni, NcStatus> {
        self.measure_with_cache_status(style, atlas, constraints).map(|(size, _)| size)
    }

    /// Measures and reports whether the retained result satisfied this exact query.
    pub(crate) fn measure_with_cache_status(&mut self, style: &Skin, atlas: &AtlasHandle, constraints: Constraints) -> Result<(Dimensioni, bool), NcStatus> {
        let revision = style.revision();
        if let Some(cached) = self.state.measurement.lookup(constraints, revision) {
            return Ok((cached, true));
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
            Err(error) => {
                self.state.invalidate_measurement();
                return Err(error);
            }
        };
        // Widgets cannot return negative geometry. Node placement policy is intentionally absent:
        // the parent consumes this desired size while resolving its own child relationship.
        let preferred_content = Dimensioni::new(measured_content.width.max(0), measured_content.height.max(0));
        let preferred = crate::ui_node::frame::outer_preferred(preferred_content, frame_insets);
        self.state.measurement.insert(MeasurementEntry { constraints, revision, preferred });
        Ok((preferred, false))
    }

    /// Consumes widget mutation markers and invalidates each changed node's ancestor path.
    pub(crate) fn synchronize_measurement_invalidation(&mut self) -> Result<bool, neocogi_com::NcStatus> {
        let collected = (|| {
            let mut measurement_dirty = self.data.take_measurement_dirty()?;
            self.with_children_mut(|children| {
                for child in children.iter_mut() {
                    measurement_dirty |= child.synchronize_measurement_invalidation()?;
                }
                Ok::<(), neocogi_com::NcStatus>(())
            }).transpose()?;
            Ok(measurement_dirty)
        })();
        match collected {
            Ok(dirty) => {
                if dirty {
                    self.state.invalidate_measurement();
                }
                Ok(dirty)
            }
            Err(error) => {
                // Earlier children may already have consumed their dirty marker. Invalidate this
                // ancestor on every failed sweep so retry cannot reuse a stale preferred size.
                self.state.invalidate_measurement();
                self.data.mark_measurement_dirty();
                Err(error)
            }
        }
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui_node::node_layout::advance_runtime_node_id;
    use crate::Vec2i;

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
    fn runtime_identity_exhaustion_is_detected_without_mutating_the_global_allocator() {
        assert_eq!(advance_runtime_node_id(1), Some(2));
        assert_eq!(advance_runtime_node_id(u64::MAX), None);
    }
}
