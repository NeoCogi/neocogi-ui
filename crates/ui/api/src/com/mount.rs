//! Typed widget projection at a tree attachment boundary.

use super::*;

/// Resolves the actual widget whose one-way attachment the runtime will claim.
/// No separate controller or metadata lifetime accompanies this capability.
pub trait Mountable {
    /// Retains the concrete widget; inspection alone never consumes its attachment.
    fn mount_widget(&self) -> Result<ComPtr<IWidgetAbi>, NcStatus>;
}

impl<I: ComInterface> Mountable for Widget<I> {
    /// Queries the common interface on the same object used for typed editing.
    fn mount_widget(&self) -> Result<ComPtr<IWidgetAbi>, NcStatus> {
        self.node_interface()
    }
}
