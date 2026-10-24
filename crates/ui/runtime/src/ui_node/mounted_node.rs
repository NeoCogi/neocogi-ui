//! Concrete COM ownership at the retained node boundary.
use crate::boundary::{copied_rect, local_rect};
use super::Observation;
use neocogi_ui::abi::core::*;
use neocogi_com::{ComPtr, NcCopyFromBufferOffset, NcStatus, NC_E_INVALID_ARG};
use std::{cell::Cell, rc::Rc};
use crate::{AtlasHandle, Dimensioni, WidgetOption, KeyboardBehavior, WidgetUpdateCtx, WidgetPaintCtx, UiInputEvent, Constraints, Skin};
use crate::Change;
use super::{
    child_store::{ChildStore, ChildTransaction},
};

/// Exactly one cached geometry capability per mounted widget, with one authoritative child store for branches.
pub(super) enum NodeLayoutKind {
    /// Leaves have no persistent children and return only their preferred size.
    Leaf(neocogi_com::WeakComBox<ILeafLayoutAbi>),
    /// Branch policy owns semantic edge metadata; this store owns the actual retained children.
    Branch {
        /// Cached external policy, queried and validated once before attachment.
        policy: neocogi_com::WeakComBox<IContainerLayoutAbi>,
        /// Keeps the authenticated read-only identity registered for the branch's full lifetime.
        _view: ComPtr<IChildrenAbi>,
        /// Exact storage behind view, never a copied tree or public runtime-node handle.
        children: Rc<ChildStore>,
    },
}

/// Node-owned COM capabilities; geometry caches remain in the surrounding node.
pub(crate) struct ComNode {
    /// Sole widget owner; application proxies and cached layout views hold weak observations.
    pub(super) widget: neocogi_com::ComBox<IWidgetAbi>,
    /// Cached optional capability owned only as long as this authoritative node.
    pub(super) dispatcher: Option<neocogi_com::WeakComBox<IWidgetDispatcherAbi>>,
    /// Runtime-returned changes persist until ancestor invalidation consumes them.
    pub(super) dirty: Cell<bool>,
    /// Cached leaf or branch policy with runtime-owned geometry access.
    pub(super) layout: NodeLayoutKind,
    /// Validated policy refreshed only at mutable synchronization boundaries.
    pub(super) properties: WidgetProperties,
    /// One concrete service lease; borrows metrics, painter and layout for each synchronous call.
    /// The factory owns no tree (its child registry is weak), so this cannot form an ownership cycle.
    /// Retained foreign scope capabilities still expire through their original service tokens.
    pub(super) services: Rc<super::node_factory::NodeFactory>,
}

impl ComNode {
    /// Rejects recursive destruction before a thread-local topology edit commits.
    /// This inspects the existing borrow word and authoritative child collection;
    /// no second lifetime flag, ownership graph, or provider callback is needed.
    pub(crate) fn ensure_idle(&self) -> Result<(), NcStatus> {
        if !self.widget.is_idle() {
            return Err(UI_E_BUSY);
        }
        match &self.layout {
            NodeLayoutKind::Leaf(_) => Ok(()),
            NodeLayoutKind::Branch { children, .. } => children.ensure_idle(),
        }
    }

    /// Resolves validated COM policy into the private retained router's concrete values.
    pub(crate) fn policy(&self) -> (WidgetOption, KeyboardBehavior, crate::FrameRole) {
        let properties = self.properties;
        let frame = local_frame(properties.frame).expect("node policy is validated");
        (
            WidgetOption::from_bits_retain(u32::from(properties.options.raw())),
            KeyboardBehavior::from_bits_retain(properties.keyboard),
            frame,
        )
    }

    /// Refreshes copied routing policy without borrowing semantic widget storage.
    fn refresh_properties(&mut self) -> Result<(), NcStatus> {
        let refreshed = {
            // Restrict observational authority to the external call. Releasing
            // it before mutation lets the copied slot remain an ordinary field.
            let _phase = Observation::begin(&self.services.state);
            self.widget.properties().and_then(|properties| {
                properties.validate()?;
                Ok(properties)
            })
        };
        match refreshed {
            Ok(properties) => {
                let previous = std::mem::replace(&mut self.properties, properties);
                if previous.options != properties.options || previous.frame != properties.frame {
                    self.mark_measurement_dirty();
                }
            }
            Err(error) => {
                self.mark_measurement_dirty();
                return Err(error);
            }
        }
        Ok(())
    }

    /// Delivers every update, including eventless calls and input beyond primary activation.
    pub(crate) fn update(&mut self, ctx: &mut WidgetUpdateCtx<'_>, input: Option<&UiInputEvent>) -> Result<Change, NcStatus> {
        let state = super::input_contract::snapshot(ctx, input, self.policy().1);
        let (event, text) = super::input_contract::event(input);
        let origin = neocogi_ui::abi::core::EventOrigin::Widget(self.widget.downgrade().into_unknown());
        let result = self
            .services
            .state
            .event_origins
            .with_origin(origin, || self.widget.update(state, event, NcCopyFromBufferOffset::new(0, text)))?;
        let change = match result {
            Ok(GeometryChange::Changed) => Change::Changed,
            Ok(GeometryChange::NoChange) => Change::NoChange,
            invalid => {
                // A foreign callback may have mutated before failing. Preserve invalidation for
                // recovery without allowing later nodes to observe an apparently successful pass.
                self.mark_measurement_dirty();
                return Err(match invalid {
                    Err(error) => error,
                    Ok(_) => NC_E_INVALID_ARG,
                });
            }
        };
        // Editing modes can change keyboard policy without changing preferred size.
        self.refresh_properties()?;
        Ok(change)
    }
    /// Starts an observational scope and gives the widget direct scoped frame recording access.
    pub(crate) fn paint(&mut self, ctx: &mut WidgetPaintCtx<'_>) -> Result<(), NcStatus> {
        let _phase = Observation::begin(&self.services.state);
        self.services.painter.paint(&self.widget, ctx)
    }
}

impl ComNode {
    /// Preserves finite/unbounded constraints across the temporary ABI conversion.
    pub(crate) fn measure(&self, _skin: &Skin, _atlas: &AtlasHandle, constraints: Constraints) -> Result<Dimensioni, NcStatus> {
        let _phase = Observation::begin(&self.services.state);
        let bounds = super::layout_service::limits(constraints);
        let result = match &self.layout {
            NodeLayoutKind::Leaf(layout) => {
                let mut layout = layout.clone();
                layout.measure(bounds, self.services.metrics.as_com_ref())
            }
            NodeLayoutKind::Branch { policy, children, .. } => self.services.layout.measure(children, policy, bounds),
        };
        match result {
            Ok(size) if size.width >= 0 && size.height >= 0 => Ok(Dimensioni::new(size.width, size.height)),
            result => {
                let error = result.err().unwrap_or(NC_E_INVALID_ARG);
                // Foreign container policy may ignore this returned failure; preserve the
                // boundary guard as well as returning it through ordinary traversal.
                self.services.state.fail(error);
                // Keep the ancestor path dirty until a successful retry. A
                // failed measurement must never become a cached zero preference.
                self.mark_measurement_dirty();
                Err(error)
            }
        }
    }

    /// Applies a successful complete branch result to private retained geometry; errors retain dirty state.
    pub(crate) fn place(
        &self,
        traversal: &mut crate::ui_node::LayoutTraversal,
        _skin: &Skin,
        _atlas: &AtlasHandle,
        rect: crate::Recti,
        current: &mut crate::ui_node::NodeRuntime,
    ) -> Result<(), NcStatus> {
        let NodeLayoutKind::Branch { policy, children, .. } = &self.layout else {
            return Ok(());
        };
        let _phase = Observation::begin(&self.services.state);
        match self.services.layout.place(children, policy, traversal, copied_rect(rect)) {
            Ok(output) => {
                current.layout.content_size = Dimensioni::new(output.content.width, output.content.height);
                current.layout.children.clip = local_rect(output.viewport);
                current.layout.children.offset = crate::Vec2i::new(output.offset_x, output.offset_y);
                current.layout.propagate_child_overflow = output.propagate_overflow;
                Ok(())
            }
            Err(error) => {
                // Retain the guard for external policy that discards a child Result.
                self.services.state.fail(error);
                self.mark_measurement_dirty();
                current.invalidate_measurement();
                Err(error)
            }
        }
    }

    /// Takes branch storage without holding any interior borrow across the runtime's recursive traversal.
    pub(crate) fn children(&self) -> Option<ChildTransaction> {
        let NodeLayoutKind::Branch { children, .. } = &self.layout else {
            return None;
        };
        match children.take() {
            Ok(children) => Some(children),
            Err(error) => {
                self.services.state.fail(error);
                self.mark_measurement_dirty();
                None
            }
        }
    }

    /// Identifies a branch without another COM query or child-count call.
    pub(crate) fn is_container(&self) -> bool {
        matches!(self.layout, NodeLayoutKind::Branch { .. })
    }

    /// Applies optional branch event policy under observational authority before router commitment.
    pub(crate) fn accepts_event(&self, input: &UiInputEvent) -> bool {
        let NodeLayoutKind::Branch { policy, .. } = &self.layout else {
            return true;
        };
        let _phase = Observation::begin(&self.services.state);
        let (event, bytes) = super::input_contract::event(Some(input));
        match policy.accepts_event(event, NcCopyFromBufferOffset::new(0, bytes)) {
            Ok(accepts) => accepts,
            Err(error) => {
                self.services.state.fail(error);
                false
            }
        }
    }
}

impl ComNode {
    /// Reports already collected invalidation without crossing a component boundary.
    pub(crate) fn is_measurement_dirty(&self) -> bool {
        self.dirty.get() || matches!(&self.layout, NodeLayoutKind::Branch { children, .. } if children.dirty.get())
    }

    /// Drains component and child changes once at the mutable traversal boundary.
    pub(crate) fn take_measurement_dirty(&mut self) -> Result<bool, NcStatus> {
        if let NodeLayoutKind::Branch { children, .. } = &self.layout
            && children.dirty.replace(false)
        {
            self.dirty.set(true);
        }
        let change = {
            let _phase = Observation::begin(&self.services.state);
            self.widget.take_change()
        };
        match change {
            Ok(GeometryChange::Changed) => self.dirty.set(true),
            Ok(GeometryChange::NoChange) => {}
            invalid => {
                self.dirty.set(true);
                return Err(match invalid {
                    Err(error) => error,
                    Ok(_) => NC_E_INVALID_ARG,
                });
            }
        }
        if self.dirty.get() {
            // Retrying a failed property query must not depend on a consumed take_change marker.
            // Keep dirty set until both copied policy and ancestor invalidation can be committed.
            self.refresh_properties()?;
        }
        Ok(self.dirty.replace(false))
    }

    /// Records the geometry result of an update until traversal propagates it.
    pub(crate) fn mark_measurement_dirty(&self) {
        self.dirty.set(true);
        self.services.state.changes_pending.set(true);
    }
}

impl Drop for ComNode {
    /// Removing the node releases its unique widget owner through normal field
    /// destruction. Weak dispatch snapshots cannot prolong its resource lifetime.
    fn drop(&mut self) {
        self.services.state.dispatch_order_changed.set(true);
        self.services.state.changes_pending.set(true);
        if self.dispatcher.is_some() {
            self.services
                .state
                .dispatcher_count
                .set(self.services.state.dispatcher_count.get().checked_sub(1).expect("counted dispatcher node"));
        }
    }
}

pub(crate) use neocogi_ui_support::frame::local_frame;

impl ComNode {
    /// Visits branch storage under its transaction; leaves have an empty sequence.
    pub(crate) fn with_children<R>(&self, f: impl FnOnce(&crate::Children) -> R) -> R {
        match self.children() {
            Some(children) => f(&children),
            None => f(&crate::Children::new()),
        }
    }

    /// Keeps the owned child transaction alive for exactly this recursive traversal.
    pub(crate) fn with_children_mut<R>(&self, f: impl FnOnce(&mut crate::Children) -> R) -> Option<R> {
        self.children().map(|mut children| f(&mut children))
    }
}
