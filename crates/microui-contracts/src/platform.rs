//! Platform input uses the same copied vocabulary as UI injection. OS objects stay in the host.
use crate::*;
use nc_com::{com_enum, com_interface, com_int_enum};

#[com_interface(auto_guid = "microui.InputSink:v1")]
/// Synchronous input destination, implemented by the UI runtime or a concrete application adapter.
pub trait IInputSink {
    /// Copies one ordered event. Text requires complete UTF-8 at offset zero; every other event
    /// requires an empty text argument. The platform borrows this interface only during polling.
    fn input(&self, event: UiInputAbi, text: NcCopyFromBufferOffset<'_, u8>) -> Result<(), NcStatus>;
}

com_int_enum! {
    /// Outcome of removing one relevant OS transition from this window's queue.
    pub enum PollStatus: u8 {
        /// No relevant transition remains; update and render the accumulated input.
        IDLE = 0,
        /// Input was delivered or drawable/window state changed; poll again before rendering.
        EVENT = 1,
        /// Application quit or this native window's close was requested.
        CLOSE = 2,
    }
}

#[com_enum(tag = u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
/// Committed native text-input placement, expressed in physical drawable pixels.
pub enum TextInputArea {
    /// No editable focus currently requests IME or text composition.
    None,
    /// Nonnegative editor rectangle; hosts convert this to logical OS/CSS coordinates.
    Area(Rect),
}

#[com_interface(auto_guid = "microui.PlatformWindow:v1")]
/// Native window, event pump and renderer share one COM identity and lifetime.
pub trait IPlatformWindow {
    /// Returns current drawable dimensions, including device scaling. Resize is synchronized
    /// by passing this value to the next UI update before rendering.
    fn size(&self) -> Result<Size, NcStatus>;
    /// Removes one relevant transition and borrows the destination for synchronous delivery.
    /// A failed delivery consumes the attempted event; later events remain queued. Recursive
    /// polling of this window returns BUSY. The destination is never retained by the platform.
    fn poll(&self, destination: ComRef<'_, IInputSinkAbi>) -> Result<PollStatus, NcStatus>;
    /// Enables/disables native text input and positions its composition UI after a UI update.
    /// Calling repeatedly with the same editable focus must preserve an active composition.
    fn text_input(&self, area: TextInputAreaAbi) -> Result<(), NcStatus>;
}

#[com_interface(auto_guid = "microui.Platform:v1")]
/// Platform plugin's concrete window factory.
pub trait IPlatform {
    /// Creates an independent window and renderer; title is UTF-8 at offset zero.
    fn create_window(
        &self,
        title: NcCopyFromBufferOffset<'_, u8>,
        size: Size,
        assets: ComRef<'_, IAssetBundleAbi>,
    ) -> Result<ComPtr<IPlatformWindowAbi>, NcStatus>;
}
