//! Concrete COM ownership at the retained node boundary.
use crate::boundary::{copied_rect, local_rect};
use super::{HostState, Observation};
use neocogi_ui_core_contracts::*;
use neocogi_com::{ComPtr, NcCopyFromBufferOffset, NcStatus, NC_E_INVALID_ARG};
use std::{cell::Cell, rc::Rc};
use super::paint_service::PaintService;
use crate::{AtlasHandle, Dimensioni, WidgetOption, KeyboardBehavior, WidgetUpdateCtx, WidgetPaintCtx, UiInputEvent, Constraints, Skin};
use crate::Change;
use super::{
    child_store::{ChildStore, ChildTransaction},
    layout_service::LayoutService,
};

/// Exactly one cached geometry capability per mounted widget, with one authoritative child store for branches.
pub(super) enum NodeLayoutKind {
    /// Leaves have no persistent children and return only their preferred size.
    Leaf(ComPtr<ILeafLayoutAbi>),
    /// Branch policy owns semantic edge metadata; this store owns the actual retained children.
    Branch {
        /// Cached external policy, queried and validated once before attachment.
        policy: ComPtr<IContainerLayoutAbi>,
        /// Keeps the authenticated read-only identity registered for the branch's full lifetime.
        _view: ComPtr<IChildrenAbi>,
        /// Exact storage behind view, never a copied tree or public runtime-node handle.
        children: Rc<ChildStore>,
    },
}

/// Private mount-lifetime ticket shared with detached callback-boundary snapshots.
/// A component removed by an earlier callback is skipped later in the same
/// sweep, even though a snapshot still owns its separate dispatcher pointer.
pub(crate) struct ComponentEventDriver {
    /// Only this shared lifecycle bit can change while a detached snapshot exists.
    active: Cell<bool>,
    /// Separate COM allocation, validated against the widget identity at mount.
    dispatcher: ComPtr<IEventDispatcherAbi>,
}

impl ComponentEventDriver {
    /// Constructs one active private driver after all provider checks succeed.
    pub(crate) fn new(dispatcher: ComPtr<IEventDispatcherAbi>) -> Self {
        Self { active: Cell::new(true), dispatcher }
    }

    /// Delivers one bounded sweep, or skips a component already removed from its mount.
    pub(crate) fn dispatch(&self, limit: u32) -> Result<u32, NcStatus> {
        if self.active.get() { self.dispatcher.dispatch(limit) } else { Ok(0) }
    }

    /// Invalidates a detached snapshot before any child or widget teardown call.
    fn retire(&self) {
        self.active.set(false);
    }
}

/// Node-owned COM capabilities; geometry caches remain in the surrounding node.
pub(crate) struct ComNode {
    /// Actual common widget capability, also retained by the application proxy.
    pub(super) widget: ComPtr<IWidgetAbi>,
    /// Optional separately allocated private dispatcher queried once at mount.
    /// It is invoked only after traversal releases this node and its descendants.
    pub(super) event_driver: Option<Rc<ComponentEventDriver>>,
    /// Runtime-returned changes persist until ancestor invalidation consumes them.
    pub(super) dirty: Cell<bool>,
    /// Cached leaf or branch policy with runtime-owned geometry access.
    pub(super) layout: NodeLayoutKind,
    /// Validated policy refreshed only at mutable synchronization boundaries.
    pub(super) properties: WidgetProperties,
    /// Owned immutable resource metrics.
    pub(super) metrics: ComPtr<ITextMetricsAbi>,
    /// Owned scope-checked recorder, never a wrapper around WidgetPaintCtx.
    pub(super) painter: PaintService,
    /// Shared owned service supports nested container calls and rejects retained stale tokens.
    pub(super) layout_service: LayoutService,
    /// Error/phase state without a pointer back into the retained tree.
    pub(super) host: Rc<HostState>,
}

impl ComNode {
    /// Resolves validated COM policy into the private retained router's concrete values.
    pub(crate) fn policy(&self) -> (WidgetOption, KeyboardBehavior, crate::FrameRole) {
        let properties = self.properties;
        let frame = FRAME_ROLES
            .iter()
            .find(|(role, _)| *role == properties.frame)
            .expect("node policy is validated")
            .1;
        (
            WidgetOption::from_bits_retain(u32::from(properties.options.raw())),
            KeyboardBehavior::from_bits_retain(properties.keyboard),
            frame,
        )
    }

    /// Refreshes copied routing policy without borrowing semantic widget storage.
    fn refresh_properties(&mut self) {
        let refreshed = {
            // Restrict observational authority to the external call. Releasing
            // it before mutation lets the copied slot remain an ordinary field.
            let _phase = Observation::begin(&self.host);
            self.widget.properties().and_then(|properties| {
                properties.validate()?;
                Ok(properties)
            })
        };
        match refreshed {
            Ok(properties) => {
                let previous = std::mem::replace(&mut self.properties, properties);
                if previous.options != properties.options || previous.frame != properties.frame {
                    self.dirty.set(true);
                }
            }
            Err(error) => {
                self.host.fail(error);
                self.dirty.set(true);
            }
        }
    }

    /// Delivers every update, including eventless calls and input beyond primary activation.
    pub(crate) fn update(&mut self, ctx: &mut WidgetUpdateCtx<'_>, input: Option<&UiInputEvent>) -> Change {
        let state = super::input_contract::snapshot(ctx, input, self.policy().1);
        let (event, text) = super::input_contract::event(input);
        let change = match self.widget.update(state, event, NcCopyFromBufferOffset::new(0, text)) {
            Ok(GeometryChange::Changed) => Change::Changed,
            Ok(GeometryChange::NoChange) => Change::NoChange,
            Ok(_) => {
                self.host.fail(NC_E_INVALID_ARG);
                Change::Changed
            }
            Err(error) => {
                self.host.fail(error);
                Change::Changed
            }
        };
        // Editing modes can change keyboard policy without changing preferred size.
        // Refresh after the call so the next routed input sees the committed mode.
        self.refresh_properties();
        change
    }
    /// Starts an observational scope and gives the widget direct scoped frame recording access.
    pub(crate) fn paint(&mut self, ctx: &mut WidgetPaintCtx<'_>) {
        let _phase = Observation::begin(&self.host);
        if let Err(error) = self.painter.paint(&self.widget, ctx) {
            self.host.fail(error);
        }
    }
}

impl ComNode {
    /// Preserves finite/unbounded constraints across the temporary ABI conversion.
    pub(crate) fn measure(&self, _skin: &Skin, _atlas: &AtlasHandle, constraints: Constraints) -> Result<Dimensioni, NcStatus> {
        let _phase = Observation::begin(&self.host);
        let bounds = super::layout_service::limits(constraints);
        let result = match &self.layout {
            NodeLayoutKind::Leaf(layout) => {
                let mut layout = layout.clone();
                layout.measure(bounds, self.metrics.as_com_ref())
            }
            NodeLayoutKind::Branch { policy, children, .. } => self.layout_service.measure(children, policy, bounds),
        };
        match result {
            Ok(size) if size.width >= 0 && size.height >= 0 => Ok(Dimensioni::new(size.width, size.height)),
            result => {
                let error = result.err().unwrap_or(NC_E_INVALID_ARG);
                self.host.fail(error);
                // Keep the ancestor path dirty until a successful retry. A
                // failed measurement must never become a cached zero preference.
                self.dirty.set(true);
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
    ) -> bool {
        let NodeLayoutKind::Branch { policy, children, .. } = &self.layout else {
            return true;
        };
        let _phase = Observation::begin(&self.host);
        match self.layout_service.place(children, policy, traversal, copied_rect(rect)) {
            Ok(output) => {
                current.layout.content_size = Dimensioni::new(output.content.width, output.content.height);
                current.layout.children.clip = local_rect(output.viewport);
                current.layout.children.offset = crate::Vec2i::new(output.offset_x, output.offset_y);
                current.layout.propagate_child_overflow = output.propagate_overflow;
                true
            }
            Err(error) => {
                self.host.fail(error);
                self.dirty.set(true);
                current.invalidate_measurement();
                false
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
                self.host.fail(error);
                self.dirty.set(true);
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
        let _phase = Observation::begin(&self.host);
        let (event, bytes) = super::input_contract::event(Some(input));
        match policy.accepts_event(event, NcCopyFromBufferOffset::new(0, bytes)) {
            Ok(accepts) => accepts,
            Err(error) => {
                self.host.fail(error);
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
    pub(crate) fn take_measurement_dirty(&mut self) -> bool {
        if let NodeLayoutKind::Branch { children, .. } = &self.layout
            && children.dirty.replace(false)
        {
            self.dirty.set(true);
        }
        let change = {
            // The component call is observational; release that authority
            // before mutating this node's ordinary copied policy field.
            let _phase = Observation::begin(&self.host);
            self.widget.take_change()
        };
        match change {
            Ok(GeometryChange::Changed) => {
                // A setter may have changed options or frame policy. Query that
                // copied protocol only after a real component change instead of
                // once per node on every update and render validation pass.
                self.refresh_properties();
                self.dirty.set(true);
            }
            Ok(GeometryChange::NoChange) => {}
            Ok(_) => {
                self.host.fail(NC_E_INVALID_ARG);
                self.dirty.set(true);
            }
            Err(error) => {
                self.host.fail(error);
                self.dirty.set(true);
            }
        }
        self.dirty.replace(false)
    }

    /// Records the geometry result of an update until traversal propagates it.
    pub(crate) fn mark_measurement_dirty(&self) {
        self.dirty.set(true);
    }
}

impl Drop for ComNode {
    /// Tree removal closes the widget while application COM references may remain.
    fn drop(&mut self) {
        if let Some(driver) = &self.event_driver {
            driver.retire();
        }
        if let NodeLayoutKind::Branch { children, .. } = &self.layout {
            // The retained view remains registered until descendants close, even
            // when the external widget's own close implementation later fails.
            if let Err(error) = children.close().and_then(ChildEditResult::finish) {
                self.host.fail(error);
            }
        }
        if let Err(error) = self.widget.close() {
            self.host.fail(error);
        }
    }
}

/// One mapping serves both COM policy decoding and concrete built-in reporting.
const FRAME_ROLES: &[(WidgetFrame, crate::FrameRole)] = &[
    (WidgetFrame::Generic, crate::FrameRole::Surface(crate::SurfaceRole::GenericFrame)),
    (WidgetFrame::Button, crate::FrameRole::Control(crate::ControlRole::Button)),
    (WidgetFrame::Checkbox, crate::FrameRole::Control(crate::ControlRole::Checkbox)),
    (WidgetFrame::TextInput, crate::FrameRole::Control(crate::ControlRole::TextInput)),
    (WidgetFrame::Item, crate::FrameRole::Control(crate::ControlRole::Item)),
    (WidgetFrame::Combo, crate::FrameRole::Control(crate::ControlRole::Combo)),
    (WidgetFrame::SliderTrack, crate::FrameRole::Control(crate::ControlRole::SliderTrack)),
    (WidgetFrame::SliderThumb, crate::FrameRole::Control(crate::ControlRole::SliderThumb)),
    (WidgetFrame::ScrollbarTrack, crate::FrameRole::Control(crate::ControlRole::ScrollbarTrack)),
    (WidgetFrame::ScrollbarThumb, crate::FrameRole::Control(crate::ControlRole::ScrollbarThumb)),
    (WidgetFrame::CloseButton, crate::FrameRole::Control(crate::ControlRole::CloseButton)),
    (WidgetFrame::MinimizeButton, crate::FrameRole::Control(crate::ControlRole::MinimizeButton)),
    (WidgetFrame::MaximizeButton, crate::FrameRole::Control(crate::ControlRole::MaximizeButton)),
    (WidgetFrame::RestoreButton, crate::FrameRole::Control(crate::ControlRole::RestoreButton)),
    (WidgetFrame::ResizeGrip, crate::FrameRole::Control(crate::ControlRole::ResizeGrip)),
    (WidgetFrame::Window, crate::FrameRole::Surface(crate::SurfaceRole::Window)),
    (WidgetFrame::Panel, crate::FrameRole::Surface(crate::SurfaceRole::Panel)),
];

/// Encodes the exact built-in frame family without collapsing input or item roles.
pub(crate) fn frame_contract(role: crate::FrameRole) -> WidgetFrame {
    FRAME_ROLES.iter().find(|(_, value)| *value == role).expect("all frame roles are represented").0
}

/// Decodes a validated semantic frame role for runtime-owned paint resolution.
pub(crate) fn local_frame(role: WidgetFrame) -> Result<crate::FrameRole, NcStatus> {
    FRAME_ROLES
        .iter()
        .find(|(contract, _)| *contract == role)
        .map(|(_, local)| *local)
        .ok_or(NC_E_INVALID_ARG)
}

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
