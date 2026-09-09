//! Native retained owners used exclusively by algorithm fixtures.
//! Production Node stores one concrete ComNode and has no native ownership alternative.
use std::{cell::RefCell, rc::Rc};
use crate::{AtlasHandle, Constraints, Dimensioni, LeafWidget, Recti, Skin, TypedWidgetHandle, Node};
use crate::ui_node::{Children, Container, WidgetStorage};
use crate::render::{CustomRenderHandle, CustomRenderKey, RendererBackend};
/// Reports a typed-handle borrow that escaped into retained traversal.
///
/// Keep the diagnostic path out of the normal phase-dispatch code: successful traversal is the
/// overwhelmingly common case, while a conflict is an application contract violation that always
/// terminates the current operation.
#[cold]
#[inline(never)]
fn widget_borrow_conflict() -> ! {
    panic!("retained widget invariant violated: a typed access closure must finish before runtime traversal")
}

impl Node {
    /// Creates a leaf node without retaining a typed application handle.
    pub fn widget<W: LeafWidget + 'static>(widget: W) -> Self {
        Self::mount_widget(widget, None).1
    }

    /// Creates a leaf and a weak typed handle to the same retained widget allocation.
    pub fn typed_widget<W: LeafWidget + 'static>(widget: W) -> (TypedWidgetHandle<W>, Self) {
        Self::mount_widget(widget, None)
    }

    /// Creates a framework-internal leaf without returning a typed application handle.
    pub(crate) fn widget_internal<W: LeafWidget + 'static>(widget: W) -> Self {
        Self::widget(widget)
    }

    /// Creates a leaf node with a backend-typed custom-render callback.
    ///
    /// The backend-specific handle is erased only after this checked public boundary; the stored
    /// key remains private and is validated by the renderer registry before use.
    pub fn custom_render<B, W>(widget: W, renderer: CustomRenderHandle<B>) -> Self
    where
        B: RendererBackend,
        W: LeafWidget + 'static,
    {
        Self::mount_widget(widget, Some(renderer.key)).1
    }

    /// Creates a custom-render leaf while preserving weak typed application access.
    pub fn typed_custom_render<B, W>(widget: W, renderer: CustomRenderHandle<B>) -> (TypedWidgetHandle<W>, Self)
    where
        B: RendererBackend,
        W: LeafWidget + 'static,
    {
        Self::mount_widget(widget, Some(renderer.key))
    }

    /// Allocates the native semantic fixture once before constructing its weak test handle.
    fn mount_widget<W: LeafWidget + 'static>(widget: W, custom_render: Option<crate::render::CustomRenderKey>) -> (TypedWidgetHandle<W>, Self) {
        // Allocate once while the concrete type is known, then erase only the strong reference
        // retained by the node. Both references therefore address the same RefCell allocation.
        let widget = Rc::new(RefCell::new(WidgetStorage::new(widget)));
        let handle = TypedWidgetHandle::new(&widget);
        let widget: Rc<RefCell<WidgetStorage<dyn LeafWidget>>> = widget;
        let node = Self::from_data(NodeData::Widget(WidgetNode::new(widget, custom_render)));
        (handle, node)
    }

    /// Creates a branch node from one complete concrete container owner.
    pub fn container(container: Container) -> Self {
        // Container is already the complete child/layout owner; Node adds only common runtime state.
        Self::from_data(NodeData::Container(container))
    }
}
/// Thin retained leaf owner for one erased widget and optional custom-render metadata.
pub(crate) struct WidgetNode {
    /// Sole persistent strong widget owner, erased without changing its allocation.
    pub(crate) widget: Rc<RefCell<WidgetStorage<dyn LeafWidget>>>,
    /// Optional custom backend render callback for custom-render leaves.
    custom_render: Option<CustomRenderKey>,
}

impl WidgetNode {
    /// Erases one concrete runtime at the retained leaf boundary.
    pub(crate) fn new(widget: Rc<RefCell<WidgetStorage<dyn LeafWidget>>>, custom_render: Option<CustomRenderKey>) -> Self {
        Self { widget, custom_render }
    }

    /// Returns the private custom-render callback key, when one was supplied at construction.
    pub(crate) fn custom_render(&self) -> Option<CustomRenderKey> {
        self.custom_render
    }
}

/// Private runtime payload for an owning [`Node`].
pub(crate) enum NodeData {
    /// Node-owned external widget capabilities; no Rust object or weak handle wraps them.
    Com(crate::ui_node::ComNode),
    /// Native leaf used only by algorithm fixtures.
    Widget(WidgetNode),
    /// Direct concrete container owner with one erased geometry implementation.
    Container(Container),
}

impl NodeData {
    /// Reads one concrete policy snapshot for routing and frame geometry.
    pub(crate) fn policy(&self) -> (crate::WidgetOption, crate::KeyboardBehavior, crate::FrameRole) {
        match self {
            Self::Com(node) => node.policy(),
            Self::Widget(node) => {
                let storage = node.widget.try_borrow().unwrap_or_else(|_| widget_borrow_conflict());
                let widget = &storage.widget;
                (widget.effective_widget_opt(), widget.keyboard_behavior(), widget.frame_appearance_role())
            }
            Self::Container(container) => {
                container.with_widget(|widget| (widget.effective_widget_opt(), widget.keyboard_behavior(), widget.frame_appearance_role()))
            }
        }
    }

    /// Delivers an update directly to the owning representation.
    pub(crate) fn update(&mut self, ctx: &mut crate::WidgetUpdateCtx<'_>, input: Option<&crate::UiInputEvent>) -> crate::Change {
        match self {
            Self::Com(node) => node.update(ctx, input),
            Self::Widget(node) => node
                .widget
                .try_borrow_mut()
                .unwrap_or_else(|_| widget_borrow_conflict())
                .widget
                .update(ctx, input),
            Self::Container(container) => container.with_widget_mut(|widget| widget.update(ctx, input)),
        }
    }

    /// Paints without exposing a Rust widget trait object for COM-owned nodes.
    pub(crate) fn paint(&mut self, ctx: &mut crate::WidgetPaintCtx<'_>) {
        match self {
            Self::Com(node) => node.paint(ctx),
            Self::Widget(node) => node.widget.try_borrow_mut().unwrap_or_else(|_| widget_borrow_conflict()).widget.paint(ctx),
            Self::Container(container) => container.with_widget_mut(|widget| widget.paint(ctx)),
        }
    }

    pub(crate) fn is_measurement_dirty(&self) -> bool {
        match self {
            Self::Widget(node) => node.widget.try_borrow().unwrap_or_else(|_| widget_borrow_conflict()).is_measurement_dirty(),
            Self::Com(widget) => widget.is_measurement_dirty(),
            Self::Container(container) => container.is_measurement_dirty(),
        }
    }

    pub(crate) fn take_measurement_dirty(&mut self) -> bool {
        match self {
            Self::Widget(node) => node
                .widget
                .try_borrow_mut()
                .unwrap_or_else(|_| widget_borrow_conflict())
                .take_measurement_dirty(),
            Self::Com(widget) => widget.take_measurement_dirty(),
            Self::Container(container) => container.take_measurement_dirty(),
        }
    }

    pub(crate) fn mark_measurement_dirty(&mut self) {
        match self {
            Self::Widget(node) => node
                .widget
                .try_borrow_mut()
                .unwrap_or_else(|_| widget_borrow_conflict())
                .mark_measurement_dirty(),
            Self::Com(widget) => widget.mark_measurement_dirty(),
            Self::Container(container) => container.mark_measurement_dirty(),
        }
    }
}

impl NodeData {
    /// COM child stores contain only COM nodes, including in mixed algorithm tests.
    pub(crate) fn com(&self) -> &crate::ui_node::ComNode {
        match self {
            Self::Com(node) => node,
            _ => unreachable!("COM child stores accept only COM nodes"),
        }
    }
    /// Fixture branches use their native child owner; COM fixtures use the actual service.
    pub(crate) fn with_children<R>(&self, f: impl FnOnce(&Children) -> R) -> R {
        match self {
            Self::Com(node) => node.with_children(f),
            Self::Container(node) => node.with_children(f),
            Self::Widget(_) => f(&Children::new()),
        }
    }
    /// Mutable fixture traversal keeps native child borrows scoped to the recursive operation.
    pub(crate) fn with_children_mut<R>(&mut self, f: impl FnOnce(&mut Children) -> R) -> Option<R> {
        match self {
            Self::Com(node) => node.with_children_mut(f),
            Self::Container(node) => Some(node.with_children_mut(f)),
            Self::Widget(_) => None,
        }
    }
    /// Geometry capability is fixed at construction in both fixture representations.
    pub(crate) fn is_container(&self) -> bool {
        match self {
            Self::Com(node) => node.is_container(),
            Self::Container(_) => true,
            Self::Widget(_) => false,
        }
    }
    /// Native leaves accept router-selected events; branches apply their semantic filter.
    pub(crate) fn accepts_event(&self, event: &crate::UiInputEvent) -> bool {
        match self {
            Self::Com(node) => node.accepts_event(event),
            Self::Container(node) => node.accepts_event(event),
            Self::Widget(_) => true,
        }
    }
    /// Reuses the same framing and caching path around each fixture's content algorithm.
    pub(crate) fn measure(&mut self, skin: &Skin, atlas: &AtlasHandle, constraints: Constraints) -> Result<Dimensioni, neocogi_com::NcStatus> {
        match self {
            Self::Com(node) => node.measure(skin, atlas, constraints),
            Self::Widget(node) => Ok(node.widget.borrow().widget.measure(skin, atlas, constraints)),
            Self::Container(node) => Ok(node.measure_content(skin, atlas, constraints)),
        }
    }
    /// Returns success only when a branch completed placement; leaves need no child pass.
    pub(crate) fn place(
        &mut self,
        traversal: &mut crate::ui_node::LayoutTraversal,
        skin: &Skin,
        atlas: &AtlasHandle,
        rect: Recti,
        current: &mut crate::ui_node::NodeRuntime,
    ) -> bool {
        match self {
            Self::Com(node) => node.place(traversal, skin, atlas, rect, current),
            Self::Container(node) => {
                node.place(traversal, skin, atlas, rect, current);
                true
            }
            Self::Widget(_) => true,
        }
    }
    /// Old GPU callbacks exist only to preserve native renderer ordering fixtures.
    pub(crate) fn custom_render(&self) -> Option<CustomRenderKey> {
        match self {
            Self::Widget(node) => node.custom_render(),
            _ => None,
        }
    }
}
