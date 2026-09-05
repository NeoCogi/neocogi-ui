//
// Copyright 2022-Present (c) Raja Lehtihet & Wael El Oraiby
//
// Redistribution and use in source and binary forms, with or without
// modification, are permitted provided that the following conditions are met:
//
// 1. Redistributions of source code must retain the above copyright notice,
// this list of conditions and the following disclaimer.
//
// 2. Redistributions in binary form must reproduce the above copyright notice,
// this list of conditions and the following disclaimer in the documentation
// and/or other materials provided with the distribution.
//
// 3. Neither the name of the copyright holder nor the names of its contributors
// may be used to endorse or promote products derived from this software without
// specific prior written permission.
//
// THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS "AS IS"
// AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE
// IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE
// ARE DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT HOLDER OR CONTRIBUTORS BE
// LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR
// CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF
// SUBSTITUTE GOODS OR SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS
// INTERRUPTION) HOWEVER CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN
// CONTRACT, STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE)
// ARISING IN ANY WAY OUT OF THE USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE
// POSSIBILITY OF SUCH DAMAGE.
//
//! Application-owned retained file picker with typed completion delivery.
//!
//! [`FileDialog`] is a library component, not a window-manager service. An application constructs
//! and stores one alongside its own state. The component registers a hidden dialog window
//! as a stable child of an application window and binds its controls to the application's normal
//! [`crate::Context`] dispatcher. Opening resets and shows that window; acceptance or cancellation
//! hides it and emits [`FileDialogCompleted`] from the component-owned source.
//!
//! Native paths remain [`PathBuf`] values; lossy labels are presentation only. Directory reads
//! complete before replacing a listing. Accepting a typed name is lexical: the dialog does not
//! require the resulting path to exist or to identify a regular file.

use std::{
    path::{Path, PathBuf},
    rc::Rc,
};

use crate::{
    Button, ButtonParameters, ButtonSubmitted, Context, IconRef, IconRole, Linear, LinearItem, LinearParameters, ListItem, ListItemParameters,
    ListItemSubmitted, Node, Recti, ScrollArea, ScrollAreaOption, ScrollAreaParameters, Textbox, TextboxParameters, TextboxSubmitted, TextBlock,
    TextBlockParameters, TypedWidgetHandle, Ui, WidgetEventPortHandle, WidgetOption, Window, WindowEvent, WindowHandle, WindowOption,
};
#[cfg(test)]
use crate::ui_node::RuntimeNodeId;

const DEFAULT_FILE_DIALOG_TITLE: &str = "Open File";
const DEFAULT_FILE_DIALOG_RECT: Recti = Recti { x: 50, y: 50, width: 720, height: 520 };

/// One-shot configuration used to open a file dialog.
///
/// Native path identity is preserved even when display labels need lossy Unicode conversion.
#[derive(Clone, Debug)]
pub struct FileDialogRequest {
    /// Window title for this activation.
    title: String,
    /// Native directory to read before the request becomes active.
    initial_directory: PathBuf,
    /// Initial screen-space window geometry.
    rect: Recti,
}

impl FileDialogRequest {
    /// Creates a request with the default title, current process directory, and dialog rectangle.
    pub fn new() -> Self {
        Self::default()
    }

    /// Replaces the title displayed by the retained dialog window.
    pub fn with_title(mut self, title: impl Into<String>) -> Self {
        self.title = title.into();
        self
    }

    /// Replaces the directory displayed when the dialog opens.
    ///
    /// A missing or unreadable directory causes `FileDialog::open` to return an error before
    /// showing the window or replacing the previous request data.
    pub fn with_initial_directory(mut self, directory: impl Into<PathBuf>) -> Self {
        self.initial_directory = directory.into();
        self
    }

    /// Replaces the initial outer rectangle in screen coordinates.
    pub const fn with_rect(mut self, rect: Recti) -> Self {
        self.rect = rect;
        self
    }

    /// Returns the configured dialog title.
    pub fn title(&self) -> &str {
        &self.title
    }

    /// Returns the configured initial directory.
    pub fn initial_directory(&self) -> &Path {
        &self.initial_directory
    }

    /// Returns the configured initial outer rectangle.
    pub const fn rect(&self) -> Recti {
        self.rect
    }
}

impl Default for FileDialogRequest {
    fn default() -> Self {
        let initial_directory = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        Self {
            title: DEFAULT_FILE_DIALOG_TITLE.to_owned(),
            initial_directory,
            rect: DEFAULT_FILE_DIALOG_RECT,
        }
    }
}

/// Terminal outcome of one file-dialog activation.
///
/// Completion events always carry one of these outcomes; pending state is represented by
/// [`FileDialog::is_open`] rather than an impossible event variant.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FileDialogStatus {
    /// The user accepted a non-empty native path. Acceptance is lexical; the application must
    /// validate existence/type if required and derive display text with `Path::display`.
    Accepted(PathBuf),
    /// The user cancelled, closed, explicitly cancelled, or otherwise ended the dialog.
    Cancelled,
}

/// One terminal transition emitted by an application-owned [`FileDialog`].
///
/// Each dialog owns a distinct source returned by [`FileDialog::completed`], so the source identity
/// itself identifies the component that completed.
#[derive(Clone, Debug)]
pub struct FileDialogCompleted {
    /// Owned terminal snapshot produced before the dialog returns to hidden idle state.
    status: FileDialogStatus,
}

impl FileDialogCompleted {
    /// Owns the terminal snapshot before deferred dispatch begins.
    fn new(status: FileDialogStatus) -> Self {
        Self { status }
    }

    /// Returns the terminal completion snapshot carried by this event.
    pub fn status(&self) -> &FileDialogStatus {
        // Borrow the event-owned value so handlers can inspect results without another clone.
        &self.status
    }
}

impl crate::WidgetEvent for FileDialogCompleted {}

/// Newly built dynamic rows transferred into one retained dialog column.
struct DialogRows {
    /// Unique row nodes awaiting transfer into the owning `Linear` container.
    nodes: Vec<Node>,
    /// Runtime identities paired with `nodes` for test-only geometry lookup.
    #[cfg(test)]
    ids: Vec<RuntimeNodeId>,
}

/// Test-only identities for static and dynamic dialog controls.
#[cfg(test)]
struct FileDialogTestFields {
    /// Current folder-row identities in display order.
    folder_item_ids: Vec<RuntimeNodeId>,
    /// Current file-row identities in display order.
    file_item_ids: Vec<RuntimeNodeId>,
    /// Persistent toolbar Up button identity.
    up_button_id: RuntimeNodeId,
    /// Persistent acceptance button identity.
    ok_button_id: RuntimeNodeId,
    /// Persistent cancellation button identity.
    cancel_button_id: RuntimeNodeId,
    /// Editable filename identity for input-driven selection invalidation tests.
    filename_id: RuntimeNodeId,
}

/// Failure to activate a file-dialog request; the previous request state remains intact.
#[derive(Debug)]
pub enum FileDialogOpenError {
    /// A request is still awaiting acceptance or cancellation.
    AlreadyOpen,
    /// Reading the requested directory failed; no partial listing or active request was published.
    ReadDirectory {
        /// Native directory whose complete listing could not be read.
        path: PathBuf,
        /// Concrete filesystem error, including entry/metadata iteration failures.
        source: std::io::Error,
    },
    /// The supplied UI cannot show this dialog under its current ownership/visibility policy.
    Surface(crate::SurfaceMutationError),
}

impl std::fmt::Display for FileDialogOpenError {
    /// Preserves ownership, overlap, and filesystem failure context for callers.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::AlreadyOpen => f.write_str("file dialog already has an active request"),
            Self::ReadDirectory { path, source } => write!(f, "cannot read {}: {source}", path.display()),
            Self::Surface(source) => write!(f, "cannot open file dialog: {source}"),
        }
    }
}

impl std::error::Error for FileDialogOpenError {
    /// Preserves the exact manager failure for application diagnostics.
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::AlreadyOpen => None,
            Self::ReadDirectory { source, .. } => Some(source),
            Self::Surface(source) => Some(source),
        }
    }
}

/// Reusable retained file-picker component owned by application state.
///
/// Construct it once with [`FileDialog::new`], store the returned value at the location selected by
/// the supplied accessor, and subscribe to [`FileDialog::completed`] with the application's normal
/// `Context` dispatcher. The window manager sees one hidden modal window owned by the caller-supplied
/// application window; all file-picker behavior remains in this component.
pub struct FileDialog {
    /// Application-side semantic activation, synchronized with the hidden dialog window.
    active: bool,
    /// Component-owned terminal event source retained independently of any one activation.
    completed: crate::EventSource<FileDialogCompleted>,
    /// Non-owning stable capability and event projection for the Context-owned dialog window.
    window: WindowHandle,
    /// Native directory currently represented by the two retained list columns.
    current_working_directory: PathBuf,
    /// Native folder paths retained only for geometry assertions; production rows own their payload.
    #[cfg(test)]
    folders: Vec<PathBuf>,
    /// Native file paths retained only for geometry assertions; production rows own their payload.
    #[cfg(test)]
    files: Vec<PathBuf>,
    /// Weak retained topology capability for replacing folder rows in place.
    folder_column: TypedWidgetHandle<Linear>,
    /// Weak retained topology capability for replacing file rows in place.
    file_column: TypedWidgetHandle<Linear>,
    /// Retained folder viewport state used by refresh and tests.
    folder_scroll: TypedWidgetHandle<ScrollArea>,
    /// Retained file viewport state used by refresh and tests.
    file_scroll: TypedWidgetHandle<ScrollArea>,
    /// Shared typed source used by every dynamically rebuilt folder row.
    folder_item_port: Rc<crate::EventSource<ListItemSubmitted<PathBuf>>>,
    /// Shared typed source used by every dynamically rebuilt file row.
    file_item_port: Rc<crate::EventSource<ListItemSubmitted<PathBuf>>>,
    /// Weak handle for the editable directory path.
    path_box: TypedWidgetHandle<Textbox>,
    /// Weak handle for the accepted file name.
    file_name_box: TypedWidgetHandle<Textbox>,
    /// Whether the user has edited the path input since it displayed the native current directory.
    path_input_dirty: bool,
    /// Exact row selection, retained independently of its lossy filename presentation.
    selected_path: Option<PathBuf>,
    /// Visible filesystem diagnostic; successful navigation clears it.
    error_text: TypedWidgetHandle<TextBlock>,
    /// Node identities used only for retained geometry and interaction tests.
    #[cfg(test)]
    test: FileDialogTestFields,
}

impl FileDialog {
    /// Builds one hidden retained file picker and binds its controls to application state.
    ///
    /// `parent` is the stable application window that owns the dialog. Destroying that window also
    /// destroys the dialog, while hiding it temporarily hides the dialog without discarding its
    /// retained controls. Ownership does not make dialog geometry parent-relative.
    ///
    /// The accessor is stored with the component's subscriptions and is not invoked during
    /// construction. Once the returned value is placed in application state, the accessor must
    /// resolve that same `FileDialog` for the remainder of the component's lifetime.
    ///
    /// # Panics
    ///
    /// Panics if `parent` is stale, belongs to a different Context, or does not identify an
    /// ordinary window that can own a dialog in `ctx`.
    pub fn new<B: crate::render::RendererBackend, State: 'static>(
        ctx: &mut Context<B, State>,
        parent: &WindowHandle,
        accessor: for<'a> fn(&'a mut State) -> &'a mut FileDialog,
    ) -> Self {
        // Start with an empty dormant model. The first activation supplies directory data, title, and
        // geometry immediately before this already-retained modal window becomes visible.
        let current_working_directory = PathBuf::new();
        let folders = Vec::new();
        let files = Vec::new();
        let folder_item_port = Rc::new(crate::EventSource::new());
        let file_item_port = Rc::new(crate::EventSource::new());
        let folder_rows = Self::make_rows(&current_working_directory, &folders, IconRole::ClosedFolder, &folder_item_port);
        let file_rows = Self::make_rows(&current_working_directory, &files, IconRole::File, &file_item_port);

        let (up_handle, up_node) = Button::create(ButtonParameters::new("Up"));
        let (home_handle, home_node) = Button::create(ButtonParameters::new("Home"));
        let (path_box, path_node) = Textbox::create(TextboxParameters::new(current_working_directory.to_string_lossy()));
        let (go_handle, go_node) = Button::create(ButtonParameters::new("Go"));

        let (folder_column, folder_content) = Linear::create(LinearParameters::vertical(
            std::iter::once(Self::static_item("Folders")).chain(folder_rows.nodes),
        ));
        let (file_column, file_content) = Linear::create(LinearParameters::vertical(std::iter::once(Self::static_item("Files")).chain(file_rows.nodes)));
        let (folder_scroll, folder_scroll_node) = ScrollArea::create(ScrollAreaParameters::new(
            ScrollAreaOption::FRAME | ScrollAreaOption::ENABLE_SCROLL,
            folder_content,
        ));
        let (file_scroll, file_scroll_node) = ScrollArea::create(ScrollAreaParameters::new(
            ScrollAreaOption::FRAME | ScrollAreaOption::ENABLE_SCROLL,
            file_content,
        ));

        let (file_name_box, file_name_node) = Textbox::create(TextboxParameters::new(""));
        let (cancel_handle, cancel_node) = Button::create(ButtonParameters::new("Cancel"));
        let (ok_handle, ok_node) = Button::create(ButtonParameters::new("Open"));

        #[cfg(test)]
        let test = FileDialogTestFields {
            folder_item_ids: folder_rows.ids,
            file_item_ids: file_rows.ids,
            up_button_id: up_node.id(),
            ok_button_id: ok_node.id(),
            cancel_button_id: cancel_node.id(),
            filename_id: file_name_node.id(),
        };

        let (_, toolbar) = Linear::create(LinearParameters::horizontal([
            LinearItem::fixed(up_node, 56),
            LinearItem::fixed(home_node, 56),
            LinearItem::flex(path_node, 1.0),
            LinearItem::fixed(go_node, 56),
        ]));
        let (_, browser) =
            Linear::create(LinearParameters::horizontal([LinearItem::flex(folder_scroll_node, 1.0), LinearItem::flex(file_scroll_node, 2.0)]).stretch_cross());
        let (_, filename) = Linear::create(LinearParameters::horizontal([
            LinearItem::fixed(Self::static_item("File name:"), 86),
            LinearItem::flex(file_name_node, 1.0),
        ]));
        let (_, actions) = Linear::create(LinearParameters::horizontal([
            LinearItem::flex(Self::static_item(""), 1.0),
            LinearItem::fixed(cancel_node, 96),
            LinearItem::fixed(ok_node, 96),
        ]));
        let (error_text, error_node) = TextBlock::create(TextBlockParameters::new(""));
        let (_, shell) = Linear::create(LinearParameters::vertical([
            LinearItem::content(toolbar),
            LinearItem::flex(browser, 1.0),
            LinearItem::content(filename),
            LinearItem::content(error_node),
            LinearItem::content(actions),
        ]));

        // Register the modal surface as a stable child of the application window supplied by the
        // caller. Construction fails only when that typed parent capability is stale or ineligible.
        let window = ctx
            .ui()
            .create_dialog(parent, Window::new(DEFAULT_FILE_DIALOG_TITLE, DEFAULT_FILE_DIALOG_RECT, shell))
            .expect("new file-dialog parent must remain registered");
        ctx.ui()
            .set_window_options(&window, WindowOption::FRAME)
            .expect("new file-dialog window must accept options");

        // These sources are new and private to this component, so connection failure would indicate
        // an internal construction error rather than an application-level subscription conflict.
        ctx.subscribe(up_handle.submitted(), move |state, _, event| accessor(state).up_submitted(event))
            .expect("new file-dialog Up event must be unsubscribed");
        ctx.subscribe(home_handle.submitted(), move |state, _, event| accessor(state).home_submitted(event))
            .expect("new file-dialog Home event must be unsubscribed");
        // Real user edits make the text fields authoritative over a retained native path.
        ctx.subscribe(path_box.changed(), move |state, _, _| accessor(state).path_input_dirty = true)
            .expect("new file-dialog path-change event must be unsubscribed");
        ctx.subscribe(path_box.submitted(), move |state, _, event| accessor(state).path_submitted(event))
            .expect("new file-dialog path event must be unsubscribed");
        ctx.subscribe(go_handle.submitted(), move |state, _, event| accessor(state).go_submitted(event))
            .expect("new file-dialog Go event must be unsubscribed");
        ctx.subscribe(folder_item_port.event(), move |state, _, event| accessor(state).folder_submitted(event))
            .expect("new file-dialog folder event must be unsubscribed");
        ctx.subscribe(file_name_box.changed(), move |state, _, _| accessor(state).selected_path = None)
            .expect("new file-dialog filename event must be unsubscribed");
        ctx.subscribe(file_item_port.event(), move |state, _, event| accessor(state).file_submitted(event))
            .expect("new file-dialog file event must be unsubscribed");
        ctx.subscribe(ok_handle.submitted(), move |state, ui, event| accessor(state).accept_submitted(ui, event))
            .expect("new file-dialog Open event must be unsubscribed");
        ctx.subscribe(cancel_handle.submitted(), move |state, ui, event| accessor(state).cancel_submitted(ui, event))
            .expect("new file-dialog Cancel event must be unsubscribed");
        ctx.subscribe(window.events(), move |state, ui, event| accessor(state).window_event(ui, event))
            .expect("new file-dialog window event must be unsubscribed");

        Self {
            active: false,
            completed: crate::EventSource::new(),
            window,
            current_working_directory,
            #[cfg(test)]
            folders,
            #[cfg(test)]
            files,
            folder_column,
            file_column,
            folder_scroll,
            file_scroll,
            folder_item_port,
            file_item_port,
            path_box,
            file_name_box,
            path_input_dirty: false,
            selected_path: None,
            error_text,
            #[cfg(test)]
            test,
        }
    }

    /// Creates a noninteractive heading/empty-list row through the ordinary widget path.
    fn static_item(label: &str) -> Node {
        let (_, node) = ListItem::create(ListItemParameters::with_opt(label, WidgetOption::NO_INTERACT));
        node
    }

    /// Reads and sorts a complete native listing; no entry or metadata error is silently dropped.
    fn read_directory(path: &Path) -> std::io::Result<(Vec<PathBuf>, Vec<PathBuf>)> {
        let mut folders = Vec::new();
        let mut files = Vec::new();
        for entry in std::fs::read_dir(path)? {
            let path = entry?.path();
            // Follow directory symlinks consistently with navigation. An unreadable or dangling
            // target is an explicit listing failure instead of silently changing its row kind.
            if path.metadata()?.is_dir() {
                folders.push(path);
            } else {
                files.push(path);
            }
        }
        folders.sort();
        files.sort();
        if let Some(parent) = path.parent().filter(|parent| !parent.as_os_str().is_empty()) {
            folders.insert(0, parent.to_path_buf());
        }
        Ok((folders, files))
    }

    /// Builds either column through one path-preserving row implementation.
    fn make_rows(cwd: &Path, paths: &[PathBuf], icon: IconRole, submitted_event: &Rc<crate::EventSource<ListItemSubmitted<PathBuf>>>) -> DialogRows {
        let mut nodes = Vec::with_capacity(paths.len().max(1));
        #[cfg(test)]
        let mut ids = Vec::with_capacity(paths.len());
        for path in paths {
            let label = if cwd.parent() == Some(path.as_path()) {
                std::borrow::Cow::Borrowed("..")
            } else {
                path.file_name().unwrap_or(path.as_os_str()).to_string_lossy()
            };
            // Display text is never a key: even identical replacement-character labels carry
            // distinct native paths in their statically typed submission payloads.
            let (_, node) = ListItem::create_with_event_port(
                ListItemParameters::with_icon(label, IconRef::role(icon)).with_value(path.clone()),
                Rc::clone(submitted_event),
            );
            #[cfg(test)]
            ids.push(node.id());
            nodes.push(node);
        }
        if nodes.is_empty() {
            nodes.push(Self::static_item("No entries"));
        }
        DialogRows {
            nodes,
            #[cfg(test)]
            ids,
        }
    }

    /// Commits an already complete listing to the two existing retained columns.
    fn install_entries(&mut self, folders: Vec<PathBuf>, files: Vec<PathBuf>) {
        let folder_rows = Self::make_rows(&self.current_working_directory, &folders, IconRole::ClosedFolder, &self.folder_item_port);
        let file_rows = Self::make_rows(&self.current_working_directory, &files, IconRole::File, &self.file_item_port);
        #[cfg(test)]
        let folder_item_ids = folder_rows.ids;
        #[cfg(test)]
        let file_item_ids = file_rows.ids;
        if let Err(rejected) = replace_column_rows(
            &self.folder_column,
            std::iter::once(Self::static_item("Folders")).chain(folder_rows.nodes).collect(),
        ) {
            panic!("file-dialog folder column unavailable with {} replacement nodes", rejected.len());
        }
        if let Err(rejected) = replace_column_rows(&self.file_column, std::iter::once(Self::static_item("Files")).chain(file_rows.nodes).collect()) {
            panic!("file-dialog file column unavailable with {} replacement nodes", rejected.len());
        }

        #[cfg(test)]
        {
            self.folders = folders;
            self.files = files;
            self.test.folder_item_ids = folder_item_ids;
            self.test.file_item_ids = file_item_ids;
        }
    }

    /// Returns the completion source owned by this component.
    pub fn completed(&self) -> WidgetEventPortHandle<FileDialogCompleted> {
        self.completed.event()
    }

    /// Returns the retained dialog window used by this component.
    ///
    /// Hiding or destroying this window (or its owner) cancels an active request at the next
    /// event dispatch boundary. Use `Ui::window_visible` when current surface policy is needed.
    pub fn window(&self) -> &WindowHandle {
        // Lend the aggregate capability without transferring ownership or exposing its private ID.
        &self.window
    }

    /// Returns whether a request awaits completion, regardless of which dialog is frontmost.
    ///
    /// External hiding/destruction is reconciled at the next Context update's event dispatch.
    /// This value describes pending component work; `Ui::window_visible` reports current visibility.
    pub const fn is_open(&self) -> bool {
        self.active
    }

    /// Resets and shows this file picker through the current retained UI transaction.
    ///
    /// Completion is emitted through [`Self::completed`] during a later retained event dispatch.
    ///
    /// # Errors
    ///
    /// An already active request returns `AlreadyOpen`. A stale/foreign window or hidden owner
    /// returns `Surface`; a failed complete directory read returns `ReadDirectory`. Neither
    /// failure publishes request data, shows the window, or changes activity.
    pub fn open(&mut self, ui: &mut Ui<'_>, mut request: FileDialogRequest) -> Result<(), FileDialogOpenError> {
        if self.active {
            return Err(FileDialogOpenError::AlreadyOpen);
        }
        // Validate ownership and read the full candidate before any visibility or model mutation.
        ui.window_visible(&self.window).map_err(FileDialogOpenError::Surface)?;
        request.initial_directory = std::path::absolute(&request.initial_directory).map_err(|source| FileDialogOpenError::ReadDirectory {
            path: request.initial_directory.clone(),
            source,
        })?;
        let entries = Self::read_directory(&request.initial_directory).map_err(|source| FileDialogOpenError::ReadDirectory {
            path: request.initial_directory.clone(),
            source,
        })?;
        // Showing then validates modal eligibility. All remaining model mutations are infallible.
        ui.set_window_visible(&self.window, true).map_err(FileDialogOpenError::Surface)?;
        let (title, rect) = self.prepare_open(request, entries);
        ui.set_window_name(&self.window, title).expect("validated dialog remains registered");
        ui.set_window_rect(&self.window, rect).expect("validated dialog remains registered");
        Ok(())
    }

    /// Cancels an open picker through the current retained UI transaction.
    ///
    /// Returns `false` when it is already idle. A successful cancellation queues exactly one
    /// [`FileDialogCompleted`] event for the next application dispatch boundary.
    ///
    /// # Panics
    ///
    /// Panics if a live picker is used with a `Ui` from a different Context. An already destroyed
    /// window needs no visibility mutation and can still settle its pending request.
    pub fn cancel(&mut self, ui: &mut Ui<'_>) -> bool {
        if !self.active {
            return false;
        }
        // Reuse the same completion path as buttons and title-bar dismissal so visibility and the
        // component event cannot diverge.
        self.finish(ui, FileDialogStatus::Cancelled);
        true
    }

    /// Resets activation-specific model and widget state before the dialog window is shown.
    fn prepare_open(&mut self, request: FileDialogRequest, entries: (Vec<PathBuf>, Vec<PathBuf>)) -> (String, Recti) {
        let FileDialogRequest { title, initial_directory, rect } = request;
        self.current_working_directory = initial_directory;
        self.path_box
            .try_update_with(self.current_working_directory.to_string_lossy().into_owned(), |state, path| {
                state.set_text(path)
            })
            .expect("file-dialog path box must remain mounted");
        self.file_name_box
            .try_update(Textbox::clear)
            .expect("file-dialog filename box must remain mounted");
        self.folder_scroll
            .try_update(|state| state.set_offset(crate::Vec2i::default()))
            .expect("file-dialog folder scroll area must remain mounted");
        self.file_scroll
            .try_update(|state| state.set_offset(crate::Vec2i::default()))
            .expect("file-dialog file scroll area must remain mounted");
        self.path_input_dirty = false;
        self.selected_path = None;
        self.error_text.clear().expect("file-dialog error text must remain mounted");
        self.install_entries(entries.0, entries.1);
        self.active = true;
        (title, rect)
    }

    /// Resolves a typed directory lexically; whitespace is part of the native filename.
    fn resolve_directory_path(&self, input: &str) -> Option<PathBuf> {
        if input.is_empty() {
            return None;
        }
        // An unchanged lossy path display refers to the committed native directory, not to a
        // different Unicode path containing replacement characters.
        if !self.path_input_dirty && self.path_box.text().as_deref() == Some(input) {
            return Some(self.current_working_directory.clone());
        }
        Some(self.current_working_directory.join(input))
    }

    /// Reads the platform home setting without converting its native encoding to Unicode.
    fn home_dir() -> Option<PathBuf> {
        std::env::var_os("HOME")
            .filter(|home| !home.is_empty())
            .or_else(|| std::env::var_os("USERPROFILE").filter(|home| !home.is_empty()))
            .map(PathBuf::from)
    }

    /// Accepts an exact selected path or resolves the user's untrimmed UTF-8 filename.
    fn accepted_status(&self) -> Option<FileDialogStatus> {
        if let Some(path) = &self.selected_path {
            return Some(FileDialogStatus::Accepted(path.clone()));
        }
        let typed_name = self.file_name_box.text()?;
        if typed_name.is_empty() {
            return None;
        }
        let path = self.current_working_directory.join(&typed_name);
        Some(FileDialogStatus::Accepted(path))
    }

    /// Reads a complete candidate before replacing the current directory, rows, or selection.
    fn navigate_and_refresh(&mut self, directory: PathBuf) {
        match Self::read_directory(&directory) {
            Ok((folders, files)) => {
                self.current_working_directory = directory;
                self.install_entries(folders, files);
                self.selected_path = None;
                self.file_name_box.clear().expect("file-dialog filename must remain mounted");
                self.error_text.clear().expect("file-dialog error text must remain mounted");
            }
            Err(error) => {
                // Preserve the old snapshot and selection; show the attempted path with its error.
                self.error_text
                    .set_text(format!("Cannot read {}: {error}", directory.display()))
                    .expect("file-dialog error text must remain mounted");
            }
        }
        self.path_input_dirty = false;
        self.path_box
            .set_text(self.current_working_directory.to_string_lossy())
            .expect("file-dialog path must remain mounted");
    }

    /// Navigates to the current directory's parent after an Up-button submission.
    fn up_submitted(&mut self, _event: &ButtonSubmitted) {
        // Resolve the parent from committed controller state; filesystem refresh remains atomic with
        // the navigation update performed by `navigate_and_refresh`.
        let parent = self.current_working_directory.parent().map(Path::to_path_buf);
        if let Some(parent) = parent {
            self.navigate_and_refresh(parent);
        }
    }

    /// Navigates to the platform home directory after a Home-button submission.
    fn home_submitted(&mut self, _event: &ButtonSubmitted) {
        // An unavailable home setting has no destination; filesystem failures remain visible.
        if let Some(home) = Self::home_dir() {
            self.navigate_and_refresh(home);
        }
    }

    /// Resolves and navigates to a directory submitted from the path textbox.
    fn path_submitted(&mut self, event: &TextboxSubmitted) {
        // The payload is an owned semantic snapshot, so no textbox borrow is held during navigation.
        if let Some(path) = self.resolve_directory_path(&event.text) {
            self.navigate_and_refresh(path);
        }
    }

    /// Resolves the current path textbox contents after a Go-button submission.
    fn go_submitted(&mut self, _event: &ButtonSubmitted) {
        // Go has no text payload; read the retained path box only after widget traversal has ended.
        let input = self.path_box.try_read(|state| state.text().to_owned()).unwrap_or_default();
        if let Some(path) = self.resolve_directory_path(&input) {
            self.navigate_and_refresh(path);
        }
    }

    /// Navigates by the row's native payload; display-label collisions are irrelevant.
    fn folder_submitted(&mut self, event: &ListItemSubmitted<PathBuf>) {
        self.navigate_and_refresh(event.value.clone());
    }

    /// Retains the exact native row selection while displaying only its lossy basename.
    fn file_submitted(&mut self, event: &ListItemSubmitted<PathBuf>) {
        let path = &event.value;
        self.file_name_box
            .set_text(path.file_name().unwrap_or(path.as_os_str()).to_string_lossy())
            .expect("file-dialog filename must remain mounted");
        self.selected_path = Some(path.clone());
    }

    /// Ends the active request and queues its single terminal component event.
    fn emit_completion(&mut self, completion: FileDialogStatus) {
        // Every caller checks activity before entering, keeping duplicate terminal events a debug
        // invariant violation rather than silently changing the component state twice.
        debug_assert!(self.active);
        self.active = false;
        self.completed.emit(FileDialogCompleted::new(completion));
    }

    /// Hides the modal surface before publishing one accepted or cancelled result.
    fn finish(&mut self, ui: &mut Ui<'_>, completion: FileDialogStatus) {
        // Hide the retained modal before publishing completion so later subscribers observe the
        // component as idle at the same dispatch boundary.
        if self.window.events().is_alive() {
            ui.set_window_visible(&self.window, false)
                .expect("a live dialog must be used with its owning Ui");
        }
        self.emit_completion(completion);
    }

    /// Accepts the current selection when the Open button is submitted.
    fn accept_submitted(&mut self, ui: &mut Ui<'_>, _event: &ButtonSubmitted) {
        // An event queued before another completion may arrive after the component became idle.
        if !self.active {
            return;
        }
        if let Some(completion) = self.accepted_status() {
            self.finish(ui, completion);
        }
    }

    /// Cancels the active request when the dialog's Cancel button is submitted.
    fn cancel_submitted(&mut self, ui: &mut Ui<'_>, _event: &ButtonSubmitted) {
        // Ignore a stale queued button submission after another path completed the request.
        if self.active {
            self.finish(ui, FileDialogStatus::Cancelled);
        }
    }

    /// Handles one unified event from the retained dialog window.
    fn window_event(&mut self, ui: &mut Ui<'_>, event: &WindowEvent) {
        // Non-close lifecycle observations remain on the shared port but do not alter file-picker
        // state. A close request converges with explicit and button cancellation.
        match event {
            WindowEvent::CloseRequested | WindowEvent::Hidden if self.active && ui.window_visible(&self.window) != Ok(true) => {
                // A queued hide from cancel/reopen must not cancel the newly visible request.
                self.finish(ui, FileDialogStatus::Cancelled);
            }
            WindowEvent::Destroyed if self.active => self.finish(ui, FileDialogStatus::Cancelled),
            WindowEvent::CloseRequested | WindowEvent::Hidden | WindowEvent::Destroyed | WindowEvent::GeometryChanged { .. } => {}
            WindowEvent::Minimized { .. } | WindowEvent::Maximized { .. } | WindowEvent::Restored { .. } => {
                // File-dialog windows do not expose minimize/maximize buttons, so these variants
                // are exhaustive defensive handling for application-customized future options.
            }
        }
    }
}

#[allow(clippy::result_large_err)]
fn replace_column_rows(handle: &TypedWidgetHandle<Linear>, nodes: Vec<Node>) -> Result<(), Vec<Node>> {
    handle.try_update_with(nodes, |state, nodes| state.replace(nodes))?
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{NoopRenderer, test_atlas};
    use crate::{Button, ButtonParameters, ButtonSubmitted, Context, Dimensioni, MouseButton, SurfaceMutationError, WindowOption, rect};
    use std::{
        fs,
        time::{SystemTime, UNIX_EPOCH},
    };

    fn dimensions() -> Dimensioni {
        Dimensioni::new(900, 700)
    }

    struct Model {
        dialog: FileDialog,
        completions: Vec<FileDialogStatus>,
        behind_submissions: usize,
    }

    impl Model {
        fn dialog_mut(state: &mut Self) -> &mut FileDialog {
            &mut state.dialog
        }

        fn completed(&mut self, event: &FileDialogCompleted) {
            self.completions.push(event.status().clone());
        }

        fn open_from_button(&mut self, ui: &mut Ui<'_>, _event: &ButtonSubmitted) {
            self.dialog
                .open(ui, FileDialogRequest::default())
                .expect("dialog and owner must be available for this activation");
        }

        fn behind_submitted(&mut self, _event: &ButtonSubmitted) {
            self.behind_submissions += 1;
        }
    }

    fn context_and_model() -> (Context<NoopRenderer, Model>, Model) {
        let (context, model, _) = context_model_and_owner();
        (context, model)
    }

    /// Retains the public owner capability for owner-driven lifecycle regression cases.
    fn context_model_and_owner() -> (Context<NoopRenderer, Model>, Model, WindowHandle) {
        let mut context = Context::new_test_state(NoopRenderer { atlas: test_atlas() }, dimensions());
        // The fixture window gives the component the same stable lifetime owner required from a
        // real application. Its geometry is unrelated to the dialog's screen-space rectangle.
        let owner = context.ui().create_window(Window::new(
            "file-dialog owner",
            rect(0, 0, 1, 1),
            Button::create(ButtonParameters::new("owner")).1,
        ));
        let dialog = FileDialog::new(&mut context, &owner, Model::dialog_mut);
        context.subscribe(dialog.completed(), |state, _, event| Model::completed(state, event)).unwrap();
        (
            context,
            Model {
                dialog,
                completions: Vec::new(),
                behind_submissions: 0,
            },
            owner,
        )
    }

    /// Every manager dismissal path settles a pending request once, without retaining its window.
    #[test]
    fn hiding_or_destroying_the_dialog_or_owner_completes_once() {
        for owner_target in [false, true] {
            for destroy in [false, true] {
                let (mut context, mut model, owner) = context_model_and_owner();
                model.dialog.open(&mut context.ui(), FileDialogRequest::default()).unwrap();
                let target = if owner_target { &owner } else { model.dialog.window() };
                if destroy {
                    context.ui().destroy_window(target).unwrap();
                    assert!(!model.dialog.window().events().is_alive());
                } else {
                    context.ui().set_window_visible(target, false).unwrap();
                    assert_eq!(context.ui().window_visible(model.dialog.window()), Ok(false));
                }
                // Surface policy changes synchronously; component completion waits for dispatch.
                assert!(model.dialog.is_open());
                context.update_ui_state(dimensions(), &mut model);
                assert!(!model.dialog.is_open());
                assert_eq!(model.completions, [FileDialogStatus::Cancelled]);
                context.update_ui_state(dimensions(), &mut model);
                assert_eq!(model.completions, [FileDialogStatus::Cancelled]);
            }
        }
    }

    /// A queued hide reports past policy and must not cancel an explicitly reopened request.
    #[test]
    fn cancel_then_reopen_before_dispatch_ignores_the_old_hide_notification() {
        let (mut context, mut model) = context_and_model();
        model.dialog.open(&mut context.ui(), FileDialogRequest::default()).unwrap();
        assert!(model.dialog.cancel(&mut context.ui()));
        model.dialog.open(&mut context.ui(), FileDialogRequest::default()).unwrap();
        context.update_ui_state(dimensions(), &mut model);
        assert_eq!(model.completions, [FileDialogStatus::Cancelled]);
        assert!(model.dialog.is_open());
        assert_eq!(context.ui().window_visible(model.dialog.window()), Ok(true));
    }

    /// Failed modal eligibility cannot publish an activation that has no visible window.
    #[test]
    fn failed_open_preserves_idle_request_state() {
        let (mut context, mut model, owner) = context_model_and_owner();
        let original_directory = model.dialog.current_working_directory.clone();
        context.ui().set_window_visible(&owner, false).unwrap();
        let request = FileDialogRequest::default();
        assert!(matches!(
            model.dialog.open(&mut context.ui(), request),
            Err(FileDialogOpenError::Surface(SurfaceMutationError::InvalidDialogOwner))
        ));
        assert!(!model.dialog.is_open());
        assert_eq!(model.dialog.current_working_directory, original_directory);
        let (mut foreign, _) = context_and_model();
        assert!(matches!(
            model.dialog.open(&mut foreign.ui(), FileDialogRequest::default()),
            Err(FileDialogOpenError::Surface(SurfaceMutationError::UnknownWindow))
        ));
        context.ui().destroy_window(&owner).unwrap();
        assert!(matches!(
            model.dialog.open(&mut context.ui(), FileDialogRequest::default()),
            Err(FileDialogOpenError::Surface(SurfaceMutationError::UnknownWindow))
        ));
        context.update_ui_state(dimensions(), &mut model);
        assert!(model.completions.is_empty());
    }

    fn unique_temp_dir(name: &str) -> std::path::PathBuf {
        let nanos = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        std::env::temp_dir().join(format!("microui-redux-{name}-{}-{nanos}", std::process::id()))
    }

    /// Presses one retained node through the typed window that owns its concrete widget tree.
    fn click_node(context: &mut Context<NoopRenderer, Model>, model: &mut Model, window: &WindowHandle, node: RuntimeNodeId, batched: bool) {
        // Internal geometry diagnostics resolve the private stable identity only after this test
        // helper receives the same non-owning capability used by public mutations.
        let rect = context.debug_root_node_rect(window.id(), node).expect("node rect should be laid out");
        let x = rect.x + rect.width / 2;
        let y = rect.y + rect.height / 2;
        context.mousemove(x, y);
        if !batched {
            context.update_ui_state(dimensions(), model);
        }
        context.mousedown(x, y, MouseButton::LEFT);
        context.update_ui_state(dimensions(), model);
    }

    fn release_pointer(context: &mut Context<NoopRenderer, Model>, model: &mut Model) {
        context.mouseup(0, 0, MouseButton::LEFT);
        context.update_ui_state(dimensions(), model);
    }

    #[test]
    fn request_builders_and_application_owned_open_are_public_contract() {
        let dir = unique_temp_dir("request");
        fs::create_dir_all(&dir).unwrap();
        let request = FileDialogRequest::new()
            .with_title("Choose")
            .with_initial_directory(&dir)
            .with_rect(Recti::new(10, 20, 400, 300));
        assert_eq!(request.title(), "Choose");
        assert_eq!(request.initial_directory(), dir);
        let request_rect = request.rect();
        assert_eq!((request_rect.x, request_rect.y, request_rect.width, request_rect.height), (10, 20, 400, 300));

        let (mut context, mut model) = context_and_model();
        let dialog_window = model.dialog.window().clone();
        model
            .dialog
            .open(&mut context.ui(), request)
            .expect("dialog and owner must be available for this activation");
        assert!(model.dialog.is_open());
        assert_eq!(context.debug_root_name(dialog_window.id()).as_deref(), Some("Choose"));
        let window_rect = context.debug_root_rect(dialog_window.id()).unwrap();
        assert_eq!((window_rect.x, window_rect.y, window_rect.width, window_rect.height), (10, 20, 400, 300));
        assert_eq!(context.debug_modal_root(), Some(dialog_window.id()));
        assert!(model.dialog.cancel(&mut context.ui()));
        context.update_ui_state(dimensions(), &mut model);
        assert_eq!(model.completions, [FileDialogStatus::Cancelled]);
        fs::remove_dir_all(dir).unwrap();
    }

    /// Both opening and navigation retain the previous complete snapshot on filesystem failure.
    #[test]
    fn unreadable_directory_does_not_publish_an_empty_or_partial_listing() {
        let directory = tempfile::tempdir().unwrap();
        let file = directory.path().join("existing.txt");
        fs::write(&file, b"existing").unwrap();
        let missing = directory.path().join("missing");
        let (mut context, mut model) = context_and_model();
        assert!(
            matches!(model.dialog.open(&mut context.ui(), FileDialogRequest::new().with_initial_directory(&missing)),
            Err(FileDialogOpenError::ReadDirectory { path, source }) if path == missing && source.kind() == std::io::ErrorKind::NotFound)
        );
        assert!(!model.dialog.is_open());
        assert_eq!(context.ui().window_visible(model.dialog.window()), Ok(false));
        model
            .dialog
            .open(&mut context.ui(), FileDialogRequest::new().with_initial_directory(directory.path()))
            .unwrap();
        let rows = model.dialog.test.file_item_ids.clone();
        model.dialog.file_submitted(&ListItemSubmitted {
            label: "existing.txt".to_owned(),
            value: file.clone(),
        });
        model.dialog.navigate_and_refresh(missing);
        assert_eq!(model.dialog.current_working_directory, directory.path());
        assert_eq!(model.dialog.test.file_item_ids, rows);
        assert_eq!(model.dialog.accepted_status(), Some(FileDialogStatus::Accepted(file)));
        assert!(model.dialog.error_text.text().unwrap().starts_with("Cannot read "));
        model.dialog.navigate_and_refresh(directory.path().to_path_buf());
        assert_eq!(model.dialog.error_text.text().as_deref(), Some(""));
    }

    /// Directory iteration must propagate entry/metadata failures rather than filtering them out.
    #[cfg(unix)]
    #[test]
    fn broken_entry_rejects_the_complete_directory_candidate() {
        let directory = tempfile::tempdir().unwrap();
        fs::write(directory.path().join("readable"), b"ok").unwrap();
        std::os::unix::fs::symlink("missing-target", directory.path().join("broken-link")).unwrap();
        assert_eq!(FileDialog::read_directory(directory.path()).unwrap_err().kind(), std::io::ErrorKind::NotFound);
    }

    /// Typed names retain spaces, while native row selections survive lossy/normalized display.
    #[test]
    fn typed_filename_spaces_are_not_trimmed() {
        let directory = tempfile::tempdir().unwrap();
        fs::create_dir(directory.path().join(" child ")).unwrap();
        let (mut context, mut model) = context_and_model();
        model
            .dialog
            .open(&mut context.ui(), FileDialogRequest::new().with_initial_directory(directory.path()))
            .unwrap();
        model.dialog.file_name_box.set_text(" picked.txt ").unwrap();
        assert_eq!(
            model.dialog.accepted_status(),
            Some(FileDialogStatus::Accepted(directory.path().join(" picked.txt ")))
        );
        assert_eq!(model.dialog.resolve_directory_path(" child "), Some(directory.path().join(" child ")));
    }

    /// Two equal lossy labels must still select and navigate to their distinct native paths.
    #[cfg(unix)]
    #[test]
    fn native_row_payloads_preserve_non_utf8_names_and_label_collisions() {
        use std::os::unix::ffi::OsStringExt;
        let directory = tempfile::tempdir().unwrap();
        let files =
            [b"file-\x80".as_slice(), b"file-\x81".as_slice(), b"\n".as_slice()].map(|name| directory.path().join(std::ffi::OsString::from_vec(name.to_vec())));
        let folders = [b"dir-\x80".as_slice(), b"dir-\x81".as_slice()].map(|name| directory.path().join(std::ffi::OsString::from_vec(name.to_vec())));
        assert_eq!(files[0].to_string_lossy(), files[1].to_string_lossy());
        for path in &files {
            fs::write(path, b"native").unwrap();
        }
        for path in &folders {
            fs::create_dir(path).unwrap();
        }
        let (mut context, mut model) = context_and_model();
        for expected in &files {
            model
                .dialog
                .open(&mut context.ui(), FileDialogRequest::new().with_initial_directory(directory.path()))
                .unwrap();
            context.update_ui_state(dimensions(), &mut model);
            let index = model.dialog.files.iter().position(|path| path == expected).unwrap();
            let node = model.dialog.test.file_item_ids[index];
            let window = model.dialog.window.clone();
            click_node(&mut context, &mut model, &window, node, true);
            release_pointer(&mut context, &mut model);
            let accept = model.dialog.test.ok_button_id;
            click_node(&mut context, &mut model, &window, accept, true);
            assert_eq!(model.completions.pop(), Some(FileDialogStatus::Accepted(expected.clone())));
            release_pointer(&mut context, &mut model);
        }
        // Editing and then restoring the same lossy label must discard the original native row
        // identity. This exercises real TextboxChanged dispatch, not a comparison of final strings.
        model
            .dialog
            .open(&mut context.ui(), FileDialogRequest::new().with_initial_directory(directory.path()))
            .unwrap();
        context.update_ui_state(dimensions(), &mut model);
        let index = model.dialog.files.iter().position(|path| path == &files[0]).unwrap();
        let row = model.dialog.test.file_item_ids[index];
        let window = model.dialog.window.clone();
        click_node(&mut context, &mut model, &window, row, true);
        release_pointer(&mut context, &mut model);
        let filename = model.dialog.test.filename_id;
        click_node(&mut context, &mut model, &window, filename, true);
        context.text("x");
        context.key(crate::KeyEvent::pressed(crate::Key::Backspace, crate::Modifiers::NONE));
        context.update_ui_state(dimensions(), &mut model);
        let unicode_path = directory.path().join(files[0].file_name().unwrap().to_string_lossy().as_ref());
        assert_ne!(unicode_path, files[0]);
        assert_eq!(model.dialog.accepted_status(), Some(FileDialogStatus::Accepted(unicode_path)));
        model.dialog.cancel(&mut context.ui());
        release_pointer(&mut context, &mut model);
        for expected in &folders {
            model
                .dialog
                .open(&mut context.ui(), FileDialogRequest::new().with_initial_directory(directory.path()))
                .unwrap();
            context.update_ui_state(dimensions(), &mut model);
            let index = model.dialog.folders.iter().position(|path| path == expected).unwrap();
            let node = model.dialog.test.folder_item_ids[index];
            let window = model.dialog.window.clone();
            click_node(&mut context, &mut model, &window, node, true);
            assert_eq!(&model.dialog.current_working_directory, expected);
            model.dialog.go_submitted(&ButtonSubmitted);
            assert_eq!(
                &model.dialog.current_working_directory, expected,
                "unchanged lossy path input refreshes the native directory"
            );
            model.dialog.cancel(&mut context.ui());
            release_pointer(&mut context, &mut model);
        }
    }

    /// Proves the component exposes the same authenticated, fallible capability as other windows.
    #[test]
    fn destroyed_dialog_window_reports_the_concrete_surface_error() {
        let (mut context, model) = context_and_model();
        let dialog_window = model.dialog.window().clone();

        // Destruction makes both stable handles stale and expires their event endpoints; subsequent
        // public mutation reports the concrete window failure instead of consulting a forgeable ID.
        context.ui().destroy_window(&dialog_window).unwrap();
        assert!(!model.dialog.window().events().is_alive());
        assert_eq!(context.ui().set_window_visible(&dialog_window, true), Err(SurfaceMutationError::UnknownWindow));
    }

    #[test]
    fn context_aware_application_handler_opens_the_component() {
        let (mut context, mut model) = context_and_model();
        let (button, node) = Button::create(ButtonParameters::new("open"));
        let button_id = node.id();
        let window = context.ui().create_window(Window::new("window", rect(0, 0, 100, 80), node));
        context.subscribe(button.submitted(), Model::open_from_button).unwrap();
        context.update_ui_state(dimensions(), &mut model);

        click_node(&mut context, &mut model, &window, button_id, true);
        assert!(model.dialog.is_open());
        assert_eq!(context.debug_modal_root(), Some(model.dialog.window().id()));
    }

    #[test]
    fn accepted_dialog_dispatches_completion_in_the_input_transaction() {
        let directory = tempfile::tempdir().unwrap();
        let (mut context, mut model) = context_and_model();
        model
            .dialog
            .open(&mut context.ui(), FileDialogRequest::new().with_initial_directory(directory.path()))
            .expect("dialog and owner must be available for this activation");
        context.update_ui_state(dimensions(), &mut model);
        model
            .dialog
            .file_name_box
            .try_update_with("picked.txt", |state, name| state.set_text(name))
            .unwrap();
        let dialog_window = model.dialog.window.clone();
        let open = model.dialog.test.ok_button_id;

        click_node(&mut context, &mut model, &dialog_window, open, true);
        assert_eq!(model.completions, [FileDialogStatus::Accepted(directory.path().join("picked.txt"))]);
        assert!(!model.dialog.is_open());
        assert_eq!(context.debug_root_visible(dialog_window.id()), Some(false));
    }

    #[test]
    fn explicit_cancel_queues_one_completion_for_the_next_update() {
        let (mut context, mut model) = context_and_model();
        model
            .dialog
            .open(&mut context.ui(), FileDialogRequest::default())
            .expect("dialog and owner must be available for this activation");
        assert!(model.dialog.cancel(&mut context.ui()));
        assert!(!model.dialog.cancel(&mut context.ui()));
        assert!(model.completions.is_empty());

        context.update_ui_state(dimensions(), &mut model);
        assert_eq!(model.completions, [FileDialogStatus::Cancelled]);
    }

    #[test]
    fn sequential_opens_reuse_controls_and_reset_request_state() {
        let first_dir = unique_temp_dir("first");
        let second_dir = unique_temp_dir("second");
        fs::create_dir_all(&first_dir).unwrap();
        fs::create_dir_all(&second_dir).unwrap();
        let (mut context, mut model) = context_and_model();
        let dialog_window = model.dialog.window.clone();
        let open = model.dialog.test.ok_button_id;
        let path = model.dialog.path_box.clone();
        let filename = model.dialog.file_name_box.clone();
        let scroll = model.dialog.file_scroll.clone();

        model
            .dialog
            .open(
                &mut context.ui(),
                FileDialogRequest::new().with_title("First").with_initial_directory(&first_dir),
            )
            .expect("dialog and owner must be available for this activation");
        filename.try_update_with("stale.txt", |state, text| state.set_text(text)).unwrap();
        scroll.try_update(|state| state.set_offset(crate::vec2(0, 20))).unwrap();
        assert!(model.dialog.cancel(&mut context.ui()));
        context.update_ui_state(dimensions(), &mut model);

        model
            .dialog
            .open(
                &mut context.ui(),
                FileDialogRequest::new().with_title("Second").with_initial_directory(&second_dir),
            )
            .expect("dialog and owner must be available for this activation");
        assert_eq!(model.dialog.window.id(), dialog_window.id());
        assert_eq!(model.dialog.test.ok_button_id, open);
        assert_eq!(
            path.try_read(|state| state.text().to_owned()).as_deref(),
            Some(second_dir.to_string_lossy().as_ref())
        );
        assert_eq!(filename.try_read(|state| state.text().to_owned()).as_deref(), Some(""));
        let offset = scroll.try_read(|state| state.offset()).unwrap();
        assert_eq!((offset.x, offset.y), (0, 0));
        assert_eq!(context.debug_root_name(dialog_window.id()).as_deref(), Some("Second"));

        fs::remove_dir_all(first_dir).unwrap();
        fs::remove_dir_all(second_dir).unwrap();
    }

    #[test]
    fn overlapping_open_returns_an_error_without_closing_the_active_component() {
        let (mut context, mut model) = context_and_model();
        model.dialog.open(&mut context.ui(), FileDialogRequest::default()).unwrap();
        assert!(matches!(
            model.dialog.open(&mut context.ui(), FileDialogRequest::default()),
            Err(FileDialogOpenError::AlreadyOpen)
        ));
        assert!(model.dialog.is_open());
        assert_eq!(context.debug_modal_root(), Some(model.dialog.window.id()));
    }

    #[test]
    fn two_application_owned_dialogs_are_independent() {
        struct DualModel {
            first: FileDialog,
            second: FileDialog,
            first_completions: usize,
            second_completions: usize,
        }

        impl DualModel {
            fn first_mut(state: &mut Self) -> &mut FileDialog {
                &mut state.first
            }

            fn second_mut(state: &mut Self) -> &mut FileDialog {
                &mut state.second
            }

            fn first_completed(&mut self, _event: &FileDialogCompleted) {
                self.first_completions += 1;
            }

            fn second_completed(&mut self, _event: &FileDialogCompleted) {
                self.second_completions += 1;
            }
        }

        let mut context = Context::new_test_state(NoopRenderer { atlas: test_atlas() }, dimensions());
        let owner = context.ui().create_window(Window::new(
            "file-dialog owner",
            rect(0, 0, 1, 1),
            Button::create(ButtonParameters::new("owner")).1,
        ));
        let first = FileDialog::new(&mut context, &owner, DualModel::first_mut);
        let second = FileDialog::new(&mut context, &owner, DualModel::second_mut);
        context
            .subscribe(first.completed(), |state, _, event| DualModel::first_completed(state, event))
            .unwrap();
        context
            .subscribe(second.completed(), |state, _, event| DualModel::second_completed(state, event))
            .unwrap();
        let mut model = DualModel {
            first,
            second,
            first_completions: 0,
            second_completions: 0,
        };

        model
            .first
            .open(&mut context.ui(), FileDialogRequest::new().with_title("First"))
            .expect("dialog and owner must be available for this activation");
        model
            .second
            .open(&mut context.ui(), FileDialogRequest::new().with_title("Second"))
            .expect("dialog and owner must be available for this activation");
        assert_ne!(model.first.window.id(), model.second.window.id());
        assert_eq!(context.debug_modal_root(), Some(model.second.window.id()));
        assert!(model.second.cancel(&mut context.ui()));
        context.update_ui_state(dimensions(), &mut model);
        assert_eq!((model.first_completions, model.second_completions), (0, 1));
        assert!(model.first.is_open());
        assert_eq!(context.debug_modal_root(), Some(model.first.window.id()));
    }

    #[test]
    fn open_dialog_blocks_pointer_input_to_underlying_windows() {
        let (mut context, mut model) = context_and_model();
        let (button, button_node) = Button::create(ButtonParameters::new("behind"));
        context
            .subscribe(button.submitted(), |state, _, event| Model::behind_submitted(state, event))
            .unwrap();
        let window = context.ui().create_window(Window::new("window", rect(0, 0, 100, 80), button_node));
        context
            .ui()
            .set_window_options(&window, WindowOption::FRAME | WindowOption::NO_TITLE | WindowOption::NO_RESIZE)
            .unwrap();
        model
            .dialog
            .open(&mut context.ui(), FileDialogRequest::default())
            .expect("dialog and owner must be available for this activation");
        context.update_ui_state(dimensions(), &mut model);

        context.mousedown(10, 10, MouseButton::LEFT);
        context.mouseup(10, 10, MouseButton::LEFT);
        context.update_ui_state(dimensions(), &mut model);
        assert_eq!(model.behind_submissions, 0);
        assert!(model.dialog.is_open());
        assert_eq!(context.debug_rendered_root_names(), ["file-dialog owner", "window", "Open File"]);
    }

    #[test]
    fn action_buttons_keep_standard_height_and_browser_absorbs_resize() {
        let (mut context, mut model) = context_and_model();
        model
            .dialog
            .open(&mut context.ui(), FileDialogRequest::default())
            .expect("dialog and owner must be available for this activation");
        context.update_ui_state(dimensions(), &mut model);
        let dialog_window = model.dialog.window.clone();
        let toolbar_before = context.debug_root_node_rect(dialog_window.id(), model.dialog.test.up_button_id).unwrap();
        let cancel_before = context.debug_root_node_rect(dialog_window.id(), model.dialog.test.cancel_button_id).unwrap();
        let open_before = context.debug_root_node_rect(dialog_window.id(), model.dialog.test.ok_button_id).unwrap();
        assert_eq!(cancel_before.height, toolbar_before.height);
        assert_eq!(open_before.height, toolbar_before.height);

        let mut resized = context.debug_root_rect(dialog_window.id()).unwrap();
        resized.height += 80;
        context.ui().set_window_rect(&dialog_window, resized).unwrap();
        context.update_ui_state(dimensions(), &mut model);
        let toolbar_after = context.debug_root_node_rect(dialog_window.id(), model.dialog.test.up_button_id).unwrap();
        let open_after = context.debug_root_node_rect(dialog_window.id(), model.dialog.test.ok_button_id).unwrap();
        assert_eq!(
            (toolbar_after.x, toolbar_after.y, toolbar_after.width, toolbar_after.height),
            (toolbar_before.x, toolbar_before.y, toolbar_before.width, toolbar_before.height)
        );
        assert_eq!(open_after.height, open_before.height);
        assert_eq!(open_after.y - open_before.y, 80);
    }

    fn selection_flow(batched: bool) {
        let dir = unique_temp_dir(if batched { "batched" } else { "select" });
        fs::create_dir_all(&dir).unwrap();
        let file_path = dir.join("picked.txt");
        fs::write(&file_path, b"picked").unwrap();
        let (mut context, mut model) = context_and_model();
        model
            .dialog
            .open(&mut context.ui(), FileDialogRequest::new().with_initial_directory(&dir))
            .expect("dialog and owner must be available for this activation");
        context.update_ui_state(dimensions(), &mut model);
        let dialog_window = model.dialog.window.clone();
        let file_node = model.dialog.test.file_item_ids[0];
        click_node(&mut context, &mut model, &dialog_window, file_node, batched);
        assert_eq!(
            model.dialog.file_name_box.try_read(|state| state.text().to_owned()).as_deref(),
            Some("picked.txt")
        );
        release_pointer(&mut context, &mut model);
        let open = model.dialog.test.ok_button_id;
        click_node(&mut context, &mut model, &dialog_window, open, batched);
        assert_eq!(model.completions, [FileDialogStatus::Accepted(file_path)]);
        assert!(!model.dialog.is_open());
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn selecting_file_then_open_accepts_and_hides_window() {
        selection_flow(false);
    }

    #[test]
    fn clicks_work_when_move_and_press_are_queued_together() {
        selection_flow(true);
    }

    #[test]
    fn empty_accept_stays_open_and_cancel_button_completes() {
        let (mut context, mut model) = context_and_model();
        model
            .dialog
            .open(&mut context.ui(), FileDialogRequest::default())
            .expect("dialog and owner must be available for this activation");
        context.update_ui_state(dimensions(), &mut model);
        let dialog_window = model.dialog.window.clone();
        let open = model.dialog.test.ok_button_id;
        let cancel = model.dialog.test.cancel_button_id;
        click_node(&mut context, &mut model, &dialog_window, open, false);
        assert!(model.dialog.is_open());
        assert!(model.completions.is_empty());

        release_pointer(&mut context, &mut model);
        click_node(&mut context, &mut model, &dialog_window, cancel, false);
        assert_eq!(model.completions, [FileDialogStatus::Cancelled]);
        assert!(!model.dialog.is_open());
    }

    #[test]
    fn close_request_cancels_and_hides_the_dialog_window() {
        let (mut context, mut model) = context_and_model();
        model
            .dialog
            .open(&mut context.ui(), FileDialogRequest::default())
            .expect("dialog and owner must be available for this activation");
        context.update_ui_state(dimensions(), &mut model);
        let dialog_window = model.dialog.window.clone();
        let close = context
            .debug_root_chrome(dialog_window.id())
            .unwrap()
            .1
            .expect("dialog should have a close button");
        context.mousemove(close.x + close.width / 2, close.y + close.height / 2);
        context.mousedown(close.x + close.width / 2, close.y + close.height / 2, MouseButton::LEFT);
        context.mouseup(close.x + close.width / 2, close.y + close.height / 2, MouseButton::LEFT);
        context.update_ui_state(dimensions(), &mut model);
        assert_eq!(model.completions, [FileDialogStatus::Cancelled]);
        assert!(!model.dialog.is_open());
        assert_eq!(context.debug_root_visible(dialog_window.id()), Some(false));
    }

    #[test]
    fn folder_submission_navigates_and_replaces_only_dynamic_rows() {
        let dir = unique_temp_dir("navigate");
        let child = dir.join("child");
        fs::create_dir_all(&child).unwrap();
        fs::write(child.join("inside.txt"), b"inside").unwrap();
        let (mut context, mut model) = context_and_model();
        model
            .dialog
            .open(&mut context.ui(), FileDialogRequest::new().with_initial_directory(&dir))
            .expect("dialog and owner must be available for this activation");
        context.update_ui_state(dimensions(), &mut model);
        let index = model
            .dialog
            .folders
            .iter()
            .position(|folder| Path::new(folder) == child)
            .expect("child directory should be listed");
        let dialog_window = model.dialog.window.clone();
        let child_node = model.dialog.test.folder_item_ids[index];
        let folder_scroll = model.dialog.folder_scroll.clone();
        let path_box = model.dialog.path_box.clone();
        click_node(&mut context, &mut model, &dialog_window, child_node, false);
        assert_eq!(Path::new(&model.dialog.current_working_directory), child);
        assert_eq!(model.dialog.files, [child.join("inside.txt")]);
        assert!(folder_scroll.is_alive());
        assert!(path_box.is_alive());
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn refresh_preserves_handles_and_clamps_scroll() {
        let dir = unique_temp_dir("refresh");
        fs::create_dir_all(&dir).unwrap();
        for index in 0..60 {
            fs::write(dir.join(format!("file-{index:02}.txt")), b"row").unwrap();
        }
        let (mut context, mut model) = context_and_model();
        model
            .dialog
            .open(&mut context.ui(), FileDialogRequest::new().with_initial_directory(&dir))
            .expect("dialog and owner must be available for this activation");
        context.update_ui_state(dimensions(), &mut model);
        let path = model.dialog.path_box.clone();
        let folder_scroll = model.dialog.folder_scroll.clone();
        let file_scroll = model.dialog.file_scroll.clone();
        file_scroll.try_update(|state| state.set_offset(crate::vec2(0, 40))).unwrap();
        fs::write(dir.join("new-file.txt"), b"row").unwrap();
        model.dialog.go_submitted(&ButtonSubmitted);
        context.update_ui_state(dimensions(), &mut model);
        assert!(path.is_alive() && folder_scroll.is_alive() && file_scroll.is_alive());
        assert_eq!(file_scroll.try_read(|state| state.offset().y), Some(40));

        for entry in fs::read_dir(&dir).unwrap() {
            fs::remove_file(entry.unwrap().path()).unwrap();
        }
        model.dialog.go_submitted(&ButtonSubmitted);
        context.update_ui_state(dimensions(), &mut model);
        assert_eq!(file_scroll.try_read(|state| state.offset().y), Some(0));
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn unavailable_column_replacement_returns_every_node() {
        let (column, owner) = Linear::create(LinearParameters::vertical(std::iter::empty::<Node>()));
        let rejected = column
            .try_read(|_| {
                let replacements = vec![FileDialog::static_item("one"), FileDialog::static_item("two")];
                replace_column_rows(&column, replacements).expect_err("active read must reject mutation")
            })
            .unwrap();
        assert_eq!(rejected.len(), 2);
        drop(owner);
    }
}
