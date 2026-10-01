//! Standard native-path file-picker requests with one construction-time queue.
//!
//! ```no_run
//! use neocogi_ui::{Ui, IWindowAbi, IFileDialogEventQueueAbi, FileDialogRequest};
//! use neocogi_com::{ComRef, NcStatus};
//! fn example(ui: &Ui, owner: ComRef<'_, IWindowAbi>, queue: ComRef<'_, IFileDialogEventQueueAbi>) -> Result<(), NcStatus> {
//!     let mut picker = ui.file_dialog_with_queue(owner, Some(queue))?;
//!     picker.open(&FileDialogRequest::new(std::path::Path::new(".")))?;
//!     // Keep the picker and consume messages through the application's normal inbox.
//!     Ok(())
//! }
//! ```
//!
//! Directory reads validate the complete candidate before publishing new rows; failure
//! preserves the prior request/listing. Paths keep native bytes or UTF-16 units,
//! independently of display labels. Save filenames must be a single component.
//! The picker returns a path; the application validates it, confirms overwrites,
//! and performs any file writes.
//!
//! Decode exact native paths with `events::completed_file_path` when handling a completion.
//! Whole-context `Ui::close()` drops the picker without synthesizing cancellation or
//! dispatching pending completion work. If application shutdown requires a cancellation
//! result, call `cancel()` and handle that result before terminating the UI context.
use crate::*;
use neocogi_com::{ComPtr, INcStringComPtrExt, NcCopyFromBufferOffset, NcStatus};
use std::path::{Path, PathBuf};

/// Complete application-owned request; path identity never passes through the title or display label.
#[derive(Clone, Debug)]
pub struct FileDialogRequest {
    /// Human-readable modal title.
    pub title: String,
    /// Exact native directory read before the request becomes active.
    pub directory: PathBuf,
    /// Open or Save presentation; callers retain responsibility for file validation and overwrite confirmation.
    pub mode: FileDialogMode,
    /// Suggested single filename, kept in exact native units until the user edits it.
    /// Empty starts without a selection; directory components are rejected.
    pub filename: PathBuf,
    /// Initial outer rectangle in screen coordinates.
    pub rect: Rect,
}
impl FileDialogRequest {
    /// Creates an Open request rooted at the explicitly supplied native directory.
    pub fn new(directory: &Path) -> Self {
        Self {
            title: "Open File".to_owned(),
            directory: directory.to_owned(),
            mode: FileDialogMode::Open,
            filename: PathBuf::new(),
            rect: FILE_DIALOG_RECT,
        }
    }
}

/// Thin picker client; its stable actual modal, rows and request state all remain in the UI plugin.
#[derive(Clone)]
pub struct FileDialog {
    /// Producer-owned picker capability, independent of its window's logical lifetime.
    interface: neocogi_com::WeakComBox<IFileDialogAbi>,
}
impl FileDialog {
    /// Wraps an already checked picker capability without copying its request state.
    pub fn from_interface(interface: neocogi_com::WeakComBox<IFileDialogAbi>) -> Self {
        Self { interface }
    }
    /// Borrows the concrete component interface for custom static helpers; its single queue was attached by the constructor helper.
    pub fn as_interface(&self) -> &neocogi_com::WeakComBox<IFileDialogAbi> {
        &self.interface
    }
    /// Returns the actual modal for ordinary window operations; it cannot be replaced by arbitrary popup content.
    pub fn window(&self) -> Result<ComPtr<IWindowAbi>, NcStatus> {
        self.interface.window()
    }
    /// Copies exact native path units for the request and preserves the previous state on directory-read failure.
    pub fn open(&mut self, request: &FileDialogRequest) -> Result<(), NcStatus> {
        let path = NativePath::new(&request.directory);
        let filename = NativePath::new(&request.filename);
        self.interface.open(
            NcCopyFromBufferOffset::new(0, request.title.as_bytes()),
            request.rect,
            path.as_interface(),
            request.mode,
            filename.as_interface(),
        )
    }
    /// Reports whether a request awaits acceptance or cancellation.
    pub fn active(&self) -> Result<bool, NcStatus> {
        self.interface.active()
    }
    /// Cancels once while retaining the modal definition for another request.
    pub fn cancel(&mut self) -> Result<bool, NcStatus> {
        self.interface.cancel()
    }
    /// Ends an active request and attempts removal even if completion delivery fails.
    /// Removal errors take precedence and may be retried without another completion.
    /// Successful removal expires this weak handle, including when delivery failed.
    pub fn close(&mut self) -> Result<(), NcStatus> {
        // Obtain the external window first, then finish the provider call before
        // removing its owner. No executing FileDialog method can destroy itself.
        let window = self.interface.window()?;
        let completed = self.interface.cancel().map(|_| ());
        let removed = window.close();
        removed.and(completed)
    }
    /// Copies the latest directory-read diagnostic, including native-path display and the concrete OS error.
    pub fn diagnostic(&self) -> Result<String, NcStatus> {
        self.interface.diagnostic()?.to_string()
    }
}

/// Constructor-bound completion queue and its generated local event type.
pub use crate::events::file_dialog::Event;
