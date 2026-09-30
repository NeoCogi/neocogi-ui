//! Exclusive ownership transfer at a tree attachment boundary.
use super::*;

/// A constructed, unmounted widget whose sole owner can move into a container.
/// Observers deliberately do not implement this trait: they cannot reattach a
/// mounted widget or manufacture another owner through an interface query.
pub trait Mountable {
    /// Consumes ownership and selects the common widget projection once.
    fn mount_widget(self) -> Result<ComBox<IWidgetAbi>, NcStatus>;
}
impl<I: ComInterface> Mountable for OwnedWidget<I> {
    /// Interface selection moves the existing owner; it never retains the value.
    fn mount_widget(self) -> Result<ComBox<IWidgetAbi>, NcStatus> { self.into_interface().query() }
}
impl<I: ComInterface> Mountable for ComBox<I> {
    /// Raw typed factory results can attach without an additional facade object.
    fn mount_widget(self) -> Result<ComBox<IWidgetAbi>, NcStatus> { self.query() }
}
