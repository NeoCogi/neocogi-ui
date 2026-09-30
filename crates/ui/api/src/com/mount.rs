//! Exclusive ownership transfer at a tree attachment boundary.
use super::*;

/// An unmounted owner that can be offered to a container or window.
/// Failure returns the original typed owner; weak observers cannot implement
/// attachment by manufacturing another owner. No detached mounted widget is returned.
pub trait Mountable: Sized {
    /// Selects IWidget, performs an owning operation, and restores the original
    /// Rust wrapper on rejection. Implementations must preserve object identity.
    /// This helper performs no attachment or lifetime bookkeeping itself.
    fn with_mount<R>(self, operation: impl FnOnce(ComBox<IWidgetAbi>) -> Result<R, (NcStatus, ComBox<IWidgetAbi>)>)
        -> Result<R, (NcStatus, Self)>;
}
impl<I: ComInterface> Mountable for OwnedWidget<I> {
    /// The facade has no independent ownership; forward and restore its wrapper.
    fn with_mount<R>(self, operation: impl FnOnce(ComBox<IWidgetAbi>) -> Result<R, (NcStatus, ComBox<IWidgetAbi>)>)
        -> Result<R, (NcStatus, Self)> {
        self.into_interface().with_mount(operation).map_err(|(status, owner)| (status, Self::from_interface(owner)))
    }
}
impl<I: ComInterface> Mountable for ComBox<I> {
    /// Return the same allocation and original interface on every rejection.
    fn with_mount<R>(self, operation: impl FnOnce(ComBox<IWidgetAbi>) -> Result<R, (NcStatus, ComBox<IWidgetAbi>)>)
        -> Result<R, (NcStatus, Self)> {
        let widget = self.query::<IWidgetAbi>()?;
        operation(widget).map_err(|(status, owner)| {
            // A conforming operation returns exactly its input. Immutable class
            // metadata therefore still contains this owner's original interface.
            (status, owner.query::<I>().expect("rejected owner retains its original interface"))
        })
    }
}
