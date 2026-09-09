//! Native allocation and weak handles for algorithm fixtures only.
use std::{
    cell::{Cell, RefCell},
    rc::{Rc, Weak},
};
use crate::Widget;
/// Concrete widget state and the only mutation marker needed by context-free typed handles.
///
/// The marker is written while the handle already owns this cell's exclusive borrow. Tree
/// traversal later consumes it and invalidates node-local caches on the recursive call stack.
pub(crate) struct WidgetStorage<W: ?Sized> {
    /// Whether typed application mutation invalidated this widget's preferred measurement.
    measurement_dirty: Cell<bool>,
    /// Concrete or erased widget behavior stored behind this one common retained record.
    pub(crate) widget: W,
}

impl<W> WidgetStorage<W> {
    /// Wraps one newly mounted concrete widget with clean derived state.
    pub(crate) fn new(widget: W) -> Self {
        // A new node has no cached measurement to invalidate.
        Self {
            measurement_dirty: Cell::new(false),
            widget,
        }
    }
}

impl<W: ?Sized> WidgetStorage<W> {
    pub(crate) fn mark_measurement_dirty(&mut self) {
        self.measurement_dirty.set(true);
    }
}

impl<W: Widget + ?Sized> WidgetStorage<W> {
    /// Collects shared-model changes into this node's ordinary sticky marker before reporting it.
    pub(crate) fn is_measurement_dirty(&self) -> bool {
        if self.widget.take_shared_change() {
            self.measurement_dirty.set(true);
        }
        self.measurement_dirty.get()
    }
    /// Consumes both handle mutations and independently shared semantic requests once.
    pub(crate) fn take_measurement_dirty(&mut self) -> bool {
        self.is_measurement_dirty();
        self.measurement_dirty.replace(false)
    }
}

/// Cloneable, non-owning typed access to one concrete retained widget.
///
/// A [`crate::Node`] owns the only persistent strong reference after construction. Containers and
/// the runtime retain that widget through an erased leaf or container trait object, while
/// application code and coordinating widgets may keep this typed weak view. Removing the node
/// therefore makes every typed handle expire instead of keeping an invisible widget alive.
pub struct TypedWidgetHandle<W: Widget + 'static> {
    /// Observes the native fixture allocation without extending its retained lifetime.
    widget: Weak<RefCell<WidgetStorage<W>>>,
}

impl<W: Widget + 'static> Clone for TypedWidgetHandle<W> {
    fn clone(&self) -> Self {
        Self { widget: self.widget.clone() }
    }
}

impl<W: Widget + 'static> TypedWidgetHandle<W> {
    /// Creates a weak typed view of the allocation that will be erased into a retained node.
    pub(crate) fn new(widget: &Rc<RefCell<WidgetStorage<W>>>) -> Self {
        Self { widget: Rc::downgrade(widget) }
    }

    /// Reports whether the retained tree still owns this widget.
    pub fn is_alive(&self) -> bool {
        self.widget.upgrade().is_some()
    }

    /// Runs a widget-specific read operation when the widget is alive and not mutably borrowed.
    ///
    /// Built-in widgets expose common semantic operations as methods on their specialized handle.
    /// This closure form remains available for custom widget APIs and compound operations.
    pub fn try_read<R>(&self, f: impl FnOnce(&W) -> R) -> Option<R> {
        let widget = self.widget.upgrade()?;
        let widget = widget.try_borrow().ok()?;
        Some(f(&widget.widget))
    }

    /// Runs a widget-specific mutation when the widget is alive and not otherwise borrowed.
    ///
    /// Successful access conservatively dirties retained measurement before `f` runs. A visible
    /// tree therefore requires [`crate::Context::update_ui`] before its next render.
    pub fn try_update<R>(&self, f: impl FnOnce(&mut W) -> R) -> Option<R> {
        let widget = self.widget.upgrade()?;
        let mut widget = widget.try_borrow_mut().ok()?;
        widget.mark_measurement_dirty();
        let result = f(&mut widget.widget);
        Some(result)
    }

    /// Mutates a widget while preserving an owned input when access cannot begin.
    ///
    /// As with [`Self::try_update`], successful access dirties retained measurement before `f`.
    pub fn try_update_with<I, R>(&self, input: I, f: impl FnOnce(&mut W, I) -> R) -> Result<R, I> {
        let widget = match self.widget.upgrade() {
            Some(widget) => widget,
            None => return Err(input),
        };
        let mut widget = match widget.try_borrow_mut() {
            Ok(widget) => widget,
            Err(_) => return Err(input),
        };
        widget.mark_measurement_dirty();
        let result = f(&mut widget.widget, input);
        Ok(result)
    }

    /// Mutates derived or interaction state without invalidating the current UI commit.
    ///
    /// The mutation's derived update state may lag until the next Context update.
    pub(crate) fn try_update_without_measurement<R>(&self, f: impl FnOnce(&mut W) -> R) -> Option<R> {
        let widget = self.widget.upgrade()?;
        let mut widget = widget.try_borrow_mut().ok()?;
        Some(f(&mut widget.widget))
    }
}
