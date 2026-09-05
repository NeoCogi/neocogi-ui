//! Native branch ownership for retained algorithm fixtures only.
use std::{cell::RefCell, rc::Rc};
use crate::{Constraints, Dimensioni, Recti, Skin, TypedWidgetHandle, UiInputEvent, Widget, ContainerWidget, ContainerLayoutCtx, MeasureCtx};
use crate::ui_node::{Children, ChildrenHandle, NodeRuntime, LayoutTraversal, WidgetStorage};
/// Retained owner for one erased typed branch widget and an opaque ordered child collection.
///
/// This is the only strong owner of its direct [`Node`](crate::Node) values. The concrete widget is
/// independently allocated so runtime phases can scope widget and child access to one operation
/// without exposing either retained allocation through the public container API.
pub struct Container {
    /// One authoritative shared cell for mounted descendants.
    children: Rc<RefCell<Children>>,
    /// Sole persistent strong owner of the concrete typed container widget.
    widget: Rc<RefCell<WidgetStorage<dyn ContainerWidget>>>,
}

impl Container {
    /// Creates an unmounted typed branch widget and its weak concrete handle.
    pub fn new<W>(widget: W, children: impl IntoIterator<Item = crate::Node>) -> (TypedWidgetHandle<W>, Self)
    where
        W: ContainerWidget + 'static,
    {
        Self::from_shared(Rc::new(RefCell::new(children.into_iter().collect())), widget)
    }

    /// Creates a typed branch around child storage prepared by a built-in constructor.
    pub(crate) fn from_shared<W>(children: Rc<RefCell<Children>>, widget: W) -> (TypedWidgetHandle<W>, Self)
    where
        W: ContainerWidget + 'static,
    {
        let widget = Rc::new(RefCell::new(WidgetStorage::new(widget)));
        Self::from_shared_owner(children, widget)
    }

    /// Adopts a concrete widget allocation prepared for internal weak composition links.
    pub(crate) fn from_shared_owner<W>(children: Rc<RefCell<Children>>, widget: Rc<RefCell<WidgetStorage<W>>>) -> (TypedWidgetHandle<W>, Self)
    where
        W: ContainerWidget + 'static,
    {
        // Built-in and downstream widgets receive exactly the same weak ownership capability.
        widget.borrow_mut().widget.bind_children(ChildrenHandle::new(&children));
        let handle = TypedWidgetHandle::new(&widget);
        let widget: Rc<RefCell<WidgetStorage<dyn ContainerWidget>>> = widget;
        (handle, Self { children, widget })
    }

    /// Returns whether the installed surface supports one dispatcher-selected event.
    pub(crate) fn accepts_event(&self, event: &UiInputEvent) -> bool {
        // A geometry-only container has no event-receiving surface. An installed surface owns only
        // its additional state-dependent filter; generic option policy remains in the dispatcher.
        self.widget
            .try_borrow()
            .unwrap_or_else(|_| typed_container_borrow_conflict())
            .widget
            .accepts_event(event)
    }

    /// Runs one immutable framework traversal without exposing child storage publicly.
    pub(crate) fn with_children<R>(&self, f: impl FnOnce(&Children) -> R) -> R {
        // A failed borrow is a phase-ordering bug, not a recoverable absence of children. Panicking
        // here reports the invariant violation at the ownership boundary instead of deeper in
        // recursive traversal.
        let children = self
            .children
            .try_borrow()
            .unwrap_or_else(|_| panic!("retained child invariant violated: collection is mutably borrowed during immutable traversal"));
        f(&children)
    }

    /// Runs one mutable framework traversal while preventing concurrent topology mutation.
    pub(crate) fn with_children_mut<R>(&self, f: impl FnOnce(&mut Children) -> R) -> R {
        // Hold the borrow for the complete callback so topology cannot change while a recursive
        // runtime pass is using node references from this collection.
        let mut children = self
            .children
            .try_borrow_mut()
            .unwrap_or_else(|_| panic!("retained child invariant violated: collection is already borrowed during mutable traversal"));
        f(&mut children)
    }

    /// Invokes the geometry implementation for one placement pass.
    pub(crate) fn place(&mut self, runtime: &mut LayoutTraversal, skin: &Skin, atlas: &crate::AtlasHandle, rect: Recti, current: &mut NodeRuntime) {
        // The widget receives the authoritative collection only for this call. A typed mutation
        // attempted against the same container while this borrow is active is rejected cleanly by
        // ChildrenHandle rather than invalidating indices during placement.
        let mut children = self
            .children
            .try_borrow_mut()
            .unwrap_or_else(|_| panic!("retained child invariant violated: collection is already borrowed during layout"));
        let mut ctx = ContainerLayoutCtx::new(runtime, skin, atlas, rect, &mut children);
        self.widget
            .try_borrow_mut()
            .unwrap_or_else(|_| typed_container_borrow_conflict())
            .widget
            .place(&mut ctx, rect);
        let geometry = ctx.result().expect("native fixture geometry cannot fail");
        current.layout.content_size = Dimensioni::new(geometry.content.width, geometry.content.height);
        current.layout.children.clip = crate::com::local_rect(geometry.viewport);
        current.layout.children.offset = crate::Vec2i::new(geometry.offset_x, geometry.offset_y);
        current.layout.propagate_child_overflow = geometry.propagate_overflow;
    }

    /// Resolves frame insets and measures content under one typed-runtime borrow.
    pub(crate) fn measure_content(&mut self, style: &Skin, atlas: &crate::AtlasHandle, constraints: Constraints) -> Dimensioni {
        let mut children = self
            .children
            .try_borrow_mut()
            .unwrap_or_else(|_| panic!("retained child invariant violated: collection is already borrowed during measurement"));
        let widget = self.widget.try_borrow().unwrap_or_else(|_| typed_container_borrow_conflict());
        let mut ctx = MeasureCtx::new(style, atlas, &mut children);
        ContainerWidget::measure(&widget.widget, &mut ctx, constraints)
    }

    /// Runs one common widget query against the concrete container object.
    pub(crate) fn with_widget<R>(&self, f: impl FnOnce(&dyn Widget) -> R) -> R {
        let widget = self.widget.try_borrow().unwrap_or_else(|_| typed_container_borrow_conflict());
        f(&widget.widget)
    }

    /// Runs one common mutable widget phase against the concrete container object.
    pub(crate) fn with_widget_mut<R>(&mut self, f: impl FnOnce(&mut dyn Widget) -> R) -> R {
        let mut widget = self.widget.try_borrow_mut().unwrap_or_else(|_| typed_container_borrow_conflict());
        f(&mut widget.widget)
    }

    /// Combines semantic widget edits and independently issued topology edits.
    pub(crate) fn is_measurement_dirty(&self) -> bool {
        self.children.borrow().measurement_dirty
            || self
                .widget
                .try_borrow()
                .unwrap_or_else(|_| typed_container_borrow_conflict())
                .is_measurement_dirty()
    }

    /// Consumes both sources, even when the first already requires measurement.
    pub(crate) fn take_measurement_dirty(&mut self) -> bool {
        let topology_dirty = std::mem::take(&mut self.children.borrow_mut().measurement_dirty);
        topology_dirty
            | self
                .widget
                .try_borrow_mut()
                .unwrap_or_else(|_| typed_container_borrow_conflict())
                .take_measurement_dirty()
    }

    /// Invalidates geometry following a widget-reported change.
    pub(crate) fn mark_measurement_dirty(&mut self) {
        self.widget
            .try_borrow_mut()
            .unwrap_or_else(|_| typed_container_borrow_conflict())
            .mark_measurement_dirty();
    }
}

/// Reports application access that overlaps a typed container runtime phase.
#[cold]
#[inline(never)]
fn typed_container_borrow_conflict() -> ! {
    panic!("retained widget invariant violated: a typed access closure must finish before runtime traversal")
}
