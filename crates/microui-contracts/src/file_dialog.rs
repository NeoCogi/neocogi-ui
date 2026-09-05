//! Standard native-path file picker with a reusable modal window and one completion source.
use nc_com::{ComPtr, ComRef, INcStringAbi, NcCopyFromBufferOffset, NcStatus, com_interface};

/// Initial native-picker geometry shared by static request defaults and hidden plugin construction.
pub const FILE_DIALOG_RECT: crate::Rect = crate::Rect { x: 40, y: 40, width: 600, height: 400 };

#[com_interface(auto_guid = "microui.FileDialogSink:v1")]
/// Concrete terminal outcome of one request; there is no impossible Pending event variant.
pub trait IFileDialogSink {
    /// Accepts an exact native path independently of its potentially lossy filename label.
    fn accepted(&self, path: ComRef<'_, crate::INativePathAbi>) -> Result<(), NcStatus>;
    /// Reports explicit cancellation, hiding, close affordance or destruction of an active request.
    fn cancelled(&self) -> Result<(), NcStatus>;
}
#[com_interface(auto_guid = "microui.FileDialogEvents:v1")]
/// Independent completion source for this reusable picker, using the shared application-owned lease transport.
pub trait IFileDialogEvents {
    /// Connects one concrete completion consumer without retaining the application on the producer.
    fn subscribe(&self, sink: ComRef<'_, IFileDialogSinkAbi>) -> Result<ComPtr<crate::IEventSubscriptionAbi>, NcStatus>;
}
#[com_interface(auto_guid = "microui.FileDialog:v1")]
/// UI-owned file picker controller and real COM controls; retaining its interface does not keep its runtime open.
pub trait IFileDialog {
    /// Returns the actual stable modal window capability for ordinary lifecycle and geometry operations.
    fn window(&self) -> Result<ComPtr<crate::IWindowAbi>, NcStatus>;
    /// Reads and validates a complete native directory before changing request state or showing the modal.
    fn open(&self, title: NcCopyFromBufferOffset<'_, u8>, rect: crate::Rect, directory: ComRef<'_, crate::INativePathAbi>) -> Result<(), NcStatus>;
    /// Reports whether a request is active; hidden/idle is distinct from permanently closed.
    fn active(&self) -> Result<bool, NcStatus>;
    /// Cancels one active request and hides its reusable modal; idle cancellation returns false.
    fn cancel(&self) -> Result<bool, NcStatus>;
    /// Permanently destroys the modal and ends its active request, while retained handles remain safe.
    fn close(&self) -> Result<(), NcStatus>;
    /// Copies the latest failed directory-read diagnostic without replacing the previous valid listing.
    fn diagnostic(&self) -> Result<ComPtr<INcStringAbi>, NcStatus>;
}
#[com_interface(auto_guid = "microui.FileDialogs:v1")]
/// Standard picker construction capability on the same runtime identity as ordinary window management.
pub trait IFileDialogs {
    /// Creates one hidden reusable modal owned by the authenticated ordinary window.
    fn create(&self, parent: ComRef<'_, crate::IWindowAbi>) -> Result<ComPtr<IFileDialogAbi>, NcStatus>;
}
