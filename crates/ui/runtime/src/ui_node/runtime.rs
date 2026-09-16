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

//! Authoritative retained-tree phase orchestration around an independent raw-input router.

use super::*;

mod input_router;
mod layout_traversal;
pub(crate) use layout_traversal::LayoutTraversal;
mod paint;
mod update;

use input_router::InputRouter;

use crate::input::InputSnapshot;
use crate::math::RectExt;
// Re-exported into the private paint implementation and runtime tests through `super::*`.
use crate::render::DisplayList;
use crate::{Constraints, Dimensioni, MouseButton, Recti, Skin, UNCLIPPED_RECT, Vec2i};

/// Coordinates retained measurement, layout, update, and paint for one root tree.
///
/// Raw-input state is intentionally delegated to [`InputRouter`]. `UiRuntime` supplies the router
/// with the authoritative tree and committed transform at explicit routing boundaries, then runs
/// the complete update traversal that consumes the router's one staged node-local event.
pub(crate) struct UiRuntime {
    /// Stateless geometry recursion can be owned by a scoped COM layout invocation.
    geometry: LayoutTraversal,
    /// Aggregate root content size in root body coordinates.
    root_content_size: Dimensioni,
    /// Transform from root body coordinates into screen coordinates.
    root_transform: Transform,
    /// Independent state machine for raw-input targeting and transient interaction ownership.
    input_router: InputRouter,
}

impl Default for UiRuntime {
    fn default() -> Self {
        Self {
            geometry: LayoutTraversal {},
            root_content_size: Dimensioni::default(),
            root_transform: Transform::root(UNCLIPPED_RECT),
            input_router: InputRouter::default(),
        }
    }
}

impl UiRuntime {
    /// Projects editable focus through the same transforms and eligibility gates as routing.
    pub(crate) fn text_input_area(&self, root: &Node, style: &Skin) -> Option<Recti> {
        let target = self.input_router.focus_target()?;
        Self::text_input_area_from(root, target, self.root_transform, style)
    }

    /// Finds the focused editor's clipped content box without introducing a geometry cache.
    fn text_input_area_from(node: &Node, target: RuntimeNodeId, parent: Transform, style: &Skin) -> Option<Recti> {
        if !node_accepts_input(node) || !node.intersects_clip(parent) {
            return None;
        }
        if node.id() == target {
            let (options, keyboard) = node_interaction_config(node);
            if options.intersects(WidgetOption::NO_INTERACT) || !keyboard.contains(KeyboardBehavior::TEXT_INPUT) {
                return None;
            }
            let outer = parent.resolve(node.state.layout.allocation);
            return crate::ui_node::frame::frame_geometry(outer, node_frame_role(node), style)
                .content_or_empty()
                .positive_intersection(parent.clip);
        }
        let child_parent = parent.push(node.state.layout);
        node.with_children(|children| children.iter().find_map(|child| Self::text_input_area_from(child, target, child_parent, style)))
    }

    /// Creates an empty runtime.
    pub(crate) fn new() -> Self {
        Self::default()
    }

    /// Clears update-cycle metrics before the initial synchronization layout.
    pub(crate) fn begin_update(&mut self) {
        // InputRouter independently clears event-local delivery while preserving focus/capture;
        // UiRuntime resets only traversal metrics owned by the remaining runtime phases.
        self.input_router.begin_update();
    }

    /// Starts input routing for one normalized raw event in this retained tree.
    pub(crate) fn begin_input_event(&mut self, pointer_input_enabled: bool, event: &UiInputEvent) {
        // WindowManager has already decided whether this root is pointer-eligible. The router owns
        // all remaining per-node policy and stages at most one event for the update traversal.
        self.input_router.begin_input_event(pointer_input_enabled, event);
    }

    /// Clears focus, hover, capture, and queued input while preserving retained node state.
    pub(crate) fn clear_transient_targets(&mut self) {
        self.input_router.clear_transient_targets();
    }

    /// Revokes every pointer-derived target while preserving persistent keyboard focus.
    pub(crate) fn clear_pointer_targets(&mut self) {
        // Manager-owned overlays and modal exclusion may preempt a gesture without erasing the
        // application's independent keyboard target for this retained scope.
        self.input_router.clear_pointer_targets();
    }

    /// Measures one persistent root node for auto-size without introducing a parallel projection.
    ///
    /// Root chrome owns the conversion from application constraints to the final outer extent.
    pub(crate) fn measure_tree_root(&mut self, root: &mut Node, style: &Skin, atlas: &crate::AtlasHandle, constraints: Constraints) -> Dimensioni {
        root.synchronize_measurement_invalidation();
        self.geometry.measure_node(root, style, atlas, constraints)
    }

    /// Lays out one persistent root node at its authoritative screen-space rectangle.
    pub(crate) fn layout_tree_root(&mut self, root: &mut Node, style: &Skin, atlas: crate::AtlasHandle, outer: Recti, viewport: Recti) {
        root.synchronize_measurement_invalidation();
        // Root layout establishes the transform reused by subsequent routing, update, and paint.
        self.root_transform = Transform::root(viewport);
        self.geometry.layout_node_ref(root, style, &atlas, outer);
        // Cache only derived geometry; the persistent Node remains the authoritative tree.
        self.root_content_size = root.state.layout.content_size;
        // Layout may hide or remove the current transient target, so validate identities now.
        self.input_router.sanitize_transient_targets(std::slice::from_mut(root), self.root_transform);
    }

    /// Updates one persistent root node and its eligible descendants.
    pub(crate) fn update_tree_root(&mut self, root: &mut Node, style: &Skin, atlas: crate::AtlasHandle, input: InputSnapshot) {
        // Update is parent-first and consumes at most one event previously routed to one identity.
        self.update_node_ref(root, self.root_transform, style, atlas, input);
        // Topology may change during update; remove identities whose retained path no longer
        // participates before the next event is routed.
        self.input_router.sanitize_transient_targets(std::slice::from_mut(root), self.root_transform);
    }

    /// Returns whether this runtime accepts pointer hit routing for the current event.
    pub(crate) fn accepts_pointer_input(&self) -> bool {
        self.input_router.accepts_pointer_input()
    }

    /// Reports a live capture or a revoked capture whose remaining gesture must be consumed.
    pub(crate) fn owns_pointer_gesture(&self) -> bool {
        self.input_router.owns_pointer_gesture()
    }

    /// Routes drag/release continuation to the current capture owner, when one remains valid.
    pub(crate) fn route_captured_pointer_input_event(
        &mut self,
        roots: &mut [Node],
        style: &Skin,
        mouse_buttons: MouseButton,
        event: &UiInputEvent,
    ) -> Option<bool> {
        // Supply committed layout state explicitly; InputRouter owns no measurement or transform.
        self.input_router
            .route_captured_pointer_input_event(roots, self.root_transform, style, mouse_buttons, event)
    }

    /// Routes keyboard or text input directly to the current valid focus owner.
    pub(crate) fn route_focus_input_event(&mut self, roots: &mut [Node], style: &Skin, event: &UiInputEvent) -> bool {
        self.input_router.route_focus_input_event(roots, self.root_transform, style, event)
    }

    /// Moves keyboard focus to the next or previous eligible Tab stop.
    pub(crate) fn advance_focus(&mut self, roots: &mut [Node], reverse: bool) -> bool {
        self.input_router.advance_focus(roots, self.root_transform, reverse)
    }

    /// Commits pointer-capture ownership after the selected target has classified an event.
    pub(crate) fn update_pointer_capture(&mut self, owner: RuntimeNodeId, result: input_router::RouteResult, event: &UiInputEvent, mouse_buttons: MouseButton) {
        self.input_router.update_pointer_capture(owner, result, event, mouse_buttons);
    }

    /// Routes one ordinary pointer hit to the deepest topmost application node.
    pub(crate) fn route_input_event_to_node_ref(
        &mut self,
        node: &mut Node,
        style: &Skin,
        event: &UiInputEvent,
    ) -> Option<(RuntimeNodeId, input_router::RouteResult)> {
        self.input_router.route_input_event_to_node_ref(node, self.root_transform, style, event)
    }
}

/// Returns whether a node participates in traversal through every ancestor visibility gate.
fn contains_active_node_in(roots: &[Node], id: RuntimeNodeId, root_transform: Transform) -> bool {
    roots.iter().any(|root| contains_active_node(root, id, root_transform))
}

/// Returns whether one identity remains an eligible persistent keyboard-focus owner.
fn contains_focusable_node_in(roots: &[Node], id: RuntimeNodeId, root_transform: Transform) -> bool {
    roots.iter().any(|root| contains_focusable_node(root, id, root_transform))
}

/// Searches one active branch for a focusable identity without retaining widget borrows.
fn contains_focusable_node(node: &Node, id: RuntimeNodeId, parent_transform: Transform) -> bool {
    if !node_accepts_input(node) || !node.intersects_clip(parent_transform) {
        return false;
    }
    if node.id() == id {
        let (opt, keyboard) = node_interaction_config(node);
        return !opt.intersects(WidgetOption::NO_INTERACT) && keyboard.is_focusable();
    }
    if !node_children_visible(node) {
        return false;
    }
    let child_transform = parent_transform.push(node.state.layout);
    node.with_children(|children| children.iter().any(|child| contains_focusable_node(child, id, child_transform)))
}

/// Searches one retained branch while enforcing every layout participation and clip gate.
fn contains_active_node(node: &Node, id: RuntimeNodeId, parent_transform: Transform) -> bool {
    // A disabled or hidden child filters its complete subtree from router-owned identities.
    // Roots use the default active value, so the same predicate is valid at every depth.
    if !matches!(node.state.participation, crate::ChildParticipation::Active) || !node.intersects_clip(parent_transform) {
        return false;
    }
    if node.id() == id {
        return true;
    }
    if !node_children_visible(node) {
        return false;
    }
    let child_transform = parent_transform.push(node.state.layout);
    node.with_children(|children| {
        children
            .iter()
            .filter(|child| child.intersects_clip(child_transform))
            .any(|child| contains_active_node(child, id, child_transform))
    })
}

/// Returns whether this node kind can expose retained descendants to any runtime phase.
fn node_children_visible(node: &Node) -> bool {
    // Only Container can own children; participation is checked separately by the caller.
    node.is_container()
}

/// Returns whether parent layout allows update and paint traversal through this node.
fn node_is_visible(node: &Node) -> bool {
    matches!(
        node.state.participation,
        crate::ChildParticipation::Active | crate::ChildParticipation::Disabled
    )
}

/// Returns whether parent layout allows the input router to enter this node's subtree.
fn node_accepts_input(node: &Node) -> bool {
    matches!(node.state.participation, crate::ChildParticipation::Active)
}

/// Resolves the semantic role used by a node's optional shared frame geometry.
fn node_frame_role(node: &Node) -> Option<crate::FrameRole> {
    let (options, _, role) = node.data.policy();
    options.intersects(WidgetOption::FRAME).then_some(role)
}

/// Resolves effective pointer options and keyboard behavior for the current update pass.
fn node_interaction_config(node: &Node) -> (WidgetOption, KeyboardBehavior) {
    let (mut opt, keyboard, _) = node.data.policy();
    if !node_accepts_input(node) {
        // Keep layout eligibility authoritative without mutating the concrete widget's options.
        opt |= WidgetOption::NO_INTERACT;
    }
    (opt, keyboard)
}
