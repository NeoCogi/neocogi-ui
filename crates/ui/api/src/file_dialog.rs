//! Reusable native file requests publish completions into the application's typed inbox.
//!
//! ```no_run
//! use neocogi_ui::{Ui, FileDialog, FileDialogRequest, IWindowAbi, FileDialogCompletion};
//! use neocogi_com::{ComRef, NcStatus};
//! use std::path::Path;
//!
//! fn create(ui: &Ui, owner: ComRef<'_, IWindowAbi>, directory: &Path)
//!     -> Result<(FileDialog, std::sync::mpsc::Receiver<FileDialogCompletion>), NcStatus>
//! {
//!     let mut picker = ui.file_dialog(owner)?;
//!     let (events_tx, events) = std::sync::mpsc::channel();
//!     neocogi_ui::events::bind(picker.completed(), events_tx.clone(), |message| message)?;
//!     picker.open(&FileDialogRequest::new(directory))?;
//!     Ok((picker, events))
//! }
//! ```
//!
//! Pass the inbox and an ordinary mutable consumer to `ui.update_with_events`.
//! Decode native paths with `events::completed_file_path` when handling a completion.
//!
//! Native reads validate the complete candidate before installing rows. Selection
//! is lexical: applications decide whether an accepted path must exist or have a
//! particular file type. Unicode display labels are never path lookup keys.
//!
//! Set `request.mode = FileDialogMode::Save` and `request.filename` to suggest a
//! destination. The action is labeled Save, and Enter in the filename field
//! accepts it. Suggestions must be single filenames. The caller confirms any
//! overwrite before writing; the picker itself never creates or replaces files.
use crate::*;
use neocogi_com::{ComPtr, ComRef, INcStringComPtrExt, NcCopyFromBufferOffset, NcStatus};
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
    interface: ComPtr<IFileDialogAbi>,
}
impl FileDialog {
    /// Wraps an already checked picker capability without copying its request state.
    pub fn from_interface(interface: ComPtr<IFileDialogAbi>) -> Self {
        Self { interface }
    }
    /// Borrows the concrete component interface for typed event registration or custom static helpers.
    pub fn as_interface(&self) -> ComRef<'_, IFileDialogAbi> {
        self.interface.as_com_ref()
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
    /// Permanently destroys the component's modal and closes its actual widget subtree.
    pub fn close(&mut self) -> Result<(), NcStatus> {
        self.interface.close()
    }
    /// Copies the latest directory-read diagnostic, including native-path display and the concrete OS error.
    pub fn diagnostic(&self) -> Result<String, NcStatus> {
        self.interface.diagnostic()?.to_string()
    }
}
