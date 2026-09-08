//! Standard picker composes actual COM controls and owns only explicit request and native-path state.
use super::*;
use microui_redux::{EventHandler, EventSession, LinearWidget, ListItemWidget, ScrollAreaWidget, TextBlockWidget, TextboxWidget};
use microui_redux::events::{Activation, FileDialogCompleted, TextChanged, TextSubmitted};
use nc_com::{INcStringAbi, NcString};
use std::{
    path::{Path, PathBuf},
    rc::Weak,
};

/// Shared request lifetime contains no control pointers, handlers or runtime ownership.
struct Request {
    /// True exactly while one request awaits a user outcome.
    active: Cell<bool>,
    /// Permanent component/window closure prevents reopening independently of allocation lifetime.
    closed: Cell<bool>,
    /// Real native semantic queue; only external subscriptions retain application callbacks.
    completed: crate::EventSource<FileDialogCompleted>,
}
impl Request {
    /// Emits one committed outcome after the modal has become idle.
    fn finish(&self, status: FileDialogCompleted) {
        if self.active.replace(false) {
            self.completed.emit(status);
        }
    }
    /// Runtime teardown cannot borrow an active handler model, so retirement uses this independent concrete state.
    fn retire(&self) {
        self.closed.set(true);
        self.finish(FileDialogCompleted::Cancelled);
        // Every payload on this source completes one request, so closure must
        // preserve earlier outcomes as well as an active request's cancellation.
        self.completed.finish_pending();
    }
}

/// Runtime-owned controller retains only weak runtime authority and ordinary concrete UI capabilities.
pub(super) struct Dialog {
    /// Private typed handlers reuse the static session instead of adding a second callback transport.
    session: EventSession<Model>,
    /// Shared independently of an active mutable model borrow during runtime retirement.
    request: Rc<Request>,
}

/// One authoritative picker model; live control values are read through their actual COM interfaces.
struct Model {
    /// Construction and window operations upgrade the engine for one local operation only.
    runtime: Weak<RuntimeStorage>,
    /// Actual stable hidden/reusable modal definition.
    window: ComPtr<IWindowAbi>,
    /// Request source and activity shared with the controller's lifetime operations.
    request: Rc<Request>,
    /// Exact committed native directory; lossy path-field display never replaces its identity.
    directory: PathBuf,
    /// User-edited path display becomes authoritative only after a real changed event.
    path_dirty: bool,
    /// Exact native row selection, cleared only by a real filename edit or successful navigation.
    selected: Option<PathBuf>,
    /// Persistent path field, reused across every directory and request.
    path: TextboxWidget,
    /// Persistent filename field, reused without reconstructing selection from its display label.
    filename: TextboxWidget,
    /// Persistent visible error display; read failures preserve the prior listing and selection.
    error: TextBlockWidget,
    /// Persistent scrolling controllers; refreshing replaces only their application content roots.
    folders: ScrollAreaWidget,
    /// Separate file column keeps its own retained scroll request and clamping behavior.
    files: ScrollAreaWidget,
    /// Navigation is installed after the private session finishes its current registration transaction.
    navigate: Option<PathBuf>,
    /// Complete most recent directory-read diagnostic, also available through the component interface.
    diagnostic: String,
}
impl Model {
    /// Rejects closed components and upgrades runtime ownership for one synchronous operation.
    fn owner(&self) -> Result<Runtime, NcStatus> {
        if self.request.closed.get() {
            return Err(UI_E_CLOSED);
        }
        let storage = self.runtime.upgrade().ok_or(UI_E_CLOSED)?;
        storage.factory.host.check_mutation()?;
        self.window.snapshot()?;
        Ok(Runtime { storage })
    }
    /// Resolves untrimmed user text, preserving the exact native directory when its display was not edited.
    fn resolve_path(&self, input: &str) -> Option<PathBuf> {
        if input.is_empty() {
            return None;
        }
        Some(if self.path_dirty {
            self.directory.join(input)
        } else {
            self.directory.clone()
        })
    }
    /// Hides before emitting the outcome so later callbacks observe an idle reusable component.
    fn finish(&mut self, outcome: FileDialogCompleted) -> Result<(), NcStatus> {
        if self.request.active.get() {
            if self.window.snapshot().is_ok() {
                self.window.set_visibility(Visibility::HIDDEN)?;
            }
            self.request.finish(outcome);
        }
        Ok(())
    }
    /// Accepts exact native row identity, or a nonempty untrimmed filename entered by the user.
    fn accept(&mut self) -> Result<(), NcStatus> {
        let path = if let Some(path) = &self.selected {
            path.clone()
        } else {
            let filename = self.filename.text()?;
            if filename.is_empty() {
                return Ok(());
            }
            self.directory.join(filename)
        };
        self.finish(FileDialogCompleted::Accepted(path))
    }
}

/// Explicit private button/row actions carry their data visibly instead of hiding it in callback captures.
enum Action {
    /// Navigate to the committed directory's parent, if any.
    Up,
    /// Navigate to the platform's exact native home setting, if available.
    Home,
    /// Resolve the current path field using its user-edit marker.
    Go,
    /// Accept a nonempty typed filename or exact native row selection.
    Accept,
    /// Cancel the active request while retaining its stable modal.
    Cancel,
    /// Navigate by this row's exact native payload, independent of display-label collisions.
    Folder(PathBuf),
    /// Select this exact native file while showing only its lossy basename.
    File(PathBuf),
}
impl EventHandler<Model, Activation> for Action {
    /// Callbacks read/edit ordinary COM controls only after the runtime releases its traversal context.
    fn handle(&mut self, model: &mut Model, _event: &Activation) -> Result<(), NcStatus> {
        if !model.request.active.get() {
            return Ok(());
        }
        match self {
            Self::Up => model.navigate = model.directory.parent().map(Path::to_owned),
            Self::Home => model.navigate = home_dir(),
            Self::Go => model.navigate = model.resolve_path(&model.path.text()?),
            Self::Accept => model.accept()?,
            Self::Cancel => model.finish(FileDialogCompleted::Cancelled)?,
            Self::Folder(path) => model.navigate = Some(path.clone()),
            Self::File(path) => {
                model.filename.set_text(&path.file_name().unwrap_or(path.as_os_str()).to_string_lossy())?;
                model.selected = Some(path.clone());
            }
        }
        Ok(())
    }
}
/// Concrete dirty-marker roles distinguish real text input from silent programmatic display updates.
enum Edited {
    /// A user path edit makes typed UTF-8 authoritative for future navigation.
    Path,
    /// A user filename edit replaces any previously selected native row identity.
    Filename,
}
impl EventHandler<Model, TextChanged> for Edited {
    /// Updates only the corresponding explicit model dependency; no string equality guesses user intent.
    fn handle(&mut self, model: &mut Model, _event: &TextChanged) -> Result<(), NcStatus> {
        match self {
            Self::Path => model.path_dirty = true,
            Self::Filename => model.selected = None,
        }
        Ok(())
    }
}
/// Enter in the path editor submits its captured text rather than a later live value.
struct Navigate;
impl EventHandler<Model, TextSubmitted> for Navigate {
    /// Defers control reconstruction until the private event registration transaction has ended.
    fn handle(&mut self, model: &mut Model, event: &TextSubmitted) -> Result<(), NcStatus> {
        if model.request.active.get() {
            model.navigate = model.resolve_path(&event.text);
        }
        Ok(())
    }
}
/// Existing window lifecycle settles active requests, including owner-induced hiding and destruction.
struct Lifecycle;
impl EventHandler<Model, WindowEvent> for Lifecycle {
    /// A stale Hidden from cancel/reopen must not cancel a newly visible request.
    fn handle(&mut self, model: &mut Model, event: &WindowEvent) -> Result<(), NcStatus> {
        match event {
            WindowEvent::Hidden | WindowEvent::CloseRequested
                if model.request.active.get() && model.window.snapshot().is_ok_and(|snapshot| snapshot.visibility == Visibility::HIDDEN) =>
            {
                model.finish(FileDialogCompleted::Cancelled)?;
            }
            WindowEvent::Destroyed => {
                model.request.closed.set(true);
                if model.request.active.replace(false) {
                    model.request.completed.close(FileDialogCompleted::Cancelled);
                }
            }
            _ => {}
        }
        Ok(())
    }
}

/// One completely built column and its concrete row handlers, still unpublished to the modal.
struct Column {
    /// Actual COM content root to install into the stable scroll controller.
    root: LinearWidget,
    /// Concrete exact-path actions registered only after the complete directory candidate succeeds.
    rows: Vec<(ListItemWidget, Action)>,
}
/// Complete directory candidate; neither column becomes live if any native read fails.
struct Listing {
    /// Independently scrollable folders including the exact parent navigation row.
    folders: Column,
    /// Independently scrollable files, keeping native payloads separate from display names.
    files: Column,
}

/// Constructs an ordinary concrete Linear from heterogeneous existing COM widget capabilities.
fn line(runtime: &Runtime, direction: FlowDirection, cross: CrossSize, children: Vec<(ComPtr<IWidgetAbi>, LayoutTrack)>) -> Result<LinearWidget, NcStatus> {
    let line = LinearWidget::from_interface(runtime.widget_factory().create_linear(direction, cross.into())?);
    for (index, (child, track)) in children.into_iter().enumerate() {
        line.as_interface().insert(index as u32, child.as_com_ref(), LinearEdge::new(track))?.finish()?;
    }
    Ok(line)
}
/// Constructs a text display through the ordinary widget factory.
fn label(runtime: &Runtime, value: &str) -> Result<TextBlockWidget, NcStatus> {
    Ok(TextBlockWidget::from_interface(
        runtime
            .widget_factory()
            .create_text_block(NcCopyFromBufferOffset::new(0, value.as_bytes()), TextWrapping::NONE)?,
    ))
}
/// Constructs a complete command Button through the dynamically implemented factory shorthand.
fn button(runtime: &Runtime, value: &str) -> Result<microui_redux::ButtonWidget, NcStatus> {
    Ok(microui_redux::ButtonWidget::from_interface(
        runtime
            .widget_factory()
            .create_button(NcCopyFromBufferOffset::new(0, value.as_bytes()), WidgetOptions::FRAME)?,
    ))
}
/// Builds a complete column with exact path actions and fixed ordinary list-row measurements.
fn column(runtime: &Runtime, directory: &Path, paths: Vec<PathBuf>, folder: bool) -> Result<Column, NcStatus> {
    let mut children = vec![(
        label(runtime, if folder { "Folders" } else { "Files" })?.node_interface()?,
        LayoutTrack::Content,
    )];
    let mut rows = Vec::with_capacity(paths.len());
    for path in paths {
        let display = if folder && directory.parent() == Some(path.as_path()) {
            "..".into()
        } else {
            path.file_name().unwrap_or(path.as_os_str()).to_string_lossy()
        };
        let mut item = ListItemWidget::from_interface(runtime.widget_factory().create_list_item(NcCopyFromBufferOffset::new(0, display.as_bytes()))?);
        item.set_icon_role(if folder { ContentIcon::CLOSED_FOLDER } else { ContentIcon::FILE })?;
        children.push((item.node_interface()?, LayoutTrack::Content));
        rows.push((item, if folder { Action::Folder(path) } else { Action::File(path) }));
    }
    if rows.is_empty() {
        children.push((label(runtime, "(empty)")?.node_interface()?, LayoutTrack::Content));
    }
    Ok(Column {
        root: line(runtime, FlowDirection::TOP_TO_BOTTOM, CrossSize::Stretch, children)?,
        rows,
    })
}
impl Listing {
    /// Builds actual COM rows only after a complete native directory read succeeded.
    fn new(runtime: &Runtime, directory: &Path, entries: (Vec<PathBuf>, Vec<PathBuf>)) -> Result<Self, NcStatus> {
        Ok(Self {
            folders: column(runtime, directory, entries.0, true)?,
            files: column(runtime, directory, entries.1, false)?,
        })
    }
}

impl Dialog {
    /// Constructs one hidden modal from actual COM controls; no native widget handle or generic application state enters its ABI.
    fn new(runtime: &Runtime, parent: ComRef<'_, IWindowAbi>) -> Result<Rc<Self>, NcStatus> {
        runtime.storage.factory.host.check_mutation()?;
        runtime.storage.parent(parent)?;
        let path = TextboxWidget::from_interface(runtime.widget_factory().create_textbox(NcCopyFromBufferOffset::new(0, b""))?);
        let filename = TextboxWidget::from_interface(runtime.widget_factory().create_textbox(NcCopyFromBufferOffset::new(0, b""))?);
        let up = button(runtime, "Up")?;
        let home = button(runtime, "Home")?;
        let go = button(runtime, "Go")?;
        let accept = button(runtime, "Open")?;
        let cancel = button(runtime, "Cancel")?;
        let folder_content = column(runtime, Path::new(""), Vec::new(), true)?;
        let file_content = column(runtime, Path::new(""), Vec::new(), false)?;
        let folders = ScrollAreaWidget::from_interface(
            runtime
                .widget_factory()
                .create_scroll_area(folder_content.root.node_interface()?.as_com_ref(), true)?,
        );
        let files = ScrollAreaWidget::from_interface(
            runtime
                .widget_factory()
                .create_scroll_area(file_content.root.node_interface()?.as_com_ref(), true)?,
        );
        let error = label(runtime, "")?;
        let toolbar = line(
            runtime,
            FlowDirection::LEFT_TO_RIGHT,
            CrossSize::Content,
            vec![
                (up.node_interface()?, LayoutTrack::Fixed { extent: 56 }),
                (home.node_interface()?, LayoutTrack::Fixed { extent: 56 }),
                (path.node_interface()?, LayoutTrack::Flex { weight: 1.0 }),
                (go.node_interface()?, LayoutTrack::Fixed { extent: 56 }),
            ],
        )?;
        let browser = line(
            runtime,
            FlowDirection::LEFT_TO_RIGHT,
            CrossSize::Stretch,
            vec![
                (folders.node_interface()?, LayoutTrack::Flex { weight: 1.0 }),
                (files.node_interface()?, LayoutTrack::Flex { weight: 2.0 }),
            ],
        )?;
        let filename_line = line(
            runtime,
            FlowDirection::LEFT_TO_RIGHT,
            CrossSize::Content,
            vec![
                (label(runtime, "File name:")?.node_interface()?, LayoutTrack::Fixed { extent: 86 }),
                (filename.node_interface()?, LayoutTrack::Flex { weight: 1.0 }),
            ],
        )?;
        let actions = line(
            runtime,
            FlowDirection::LEFT_TO_RIGHT,
            CrossSize::Content,
            vec![
                (label(runtime, "")?.node_interface()?, LayoutTrack::Flex { weight: 1.0 }),
                (cancel.node_interface()?, LayoutTrack::Fixed { extent: 96 }),
                (accept.node_interface()?, LayoutTrack::Fixed { extent: 96 }),
            ],
        )?;
        let shell = line(
            runtime,
            FlowDirection::TOP_TO_BOTTOM,
            CrossSize::Stretch,
            vec![
                (toolbar.node_interface()?, LayoutTrack::Content),
                (browser.node_interface()?, LayoutTrack::Flex { weight: 1.0 }),
                (filename_line.node_interface()?, LayoutTrack::Content),
                (error.node_interface()?, LayoutTrack::Content),
                (actions.node_interface()?, LayoutTrack::Content),
            ],
        )?;
        let window = runtime.create_dialog(
            parent,
            NcCopyFromBufferOffset::new(0, b"Open File"),
            FILE_DIALOG_RECT,
            shell.node_interface()?.as_com_ref(),
        )?;
        window.set_options(WindowOptions::FRAME)?;
        let request = Rc::new(Request {
            active: Cell::new(false),
            closed: Cell::new(false),
            completed: crate::EventSource::new(),
        });
        let dialog = Rc::new(Self {
            session: EventSession::new(Model {
                runtime: Rc::downgrade(&runtime.storage),
                window: window.clone(),
                request: request.clone(),
                directory: PathBuf::new(),
                path_dirty: false,
                selected: None,
                path: path.clone(),
                filename: filename.clone(),
                error,
                folders,
                files,
                navigate: None,
                diagnostic: String::new(),
            }),
            request,
        });
        let connected = dialog.connect(
            &path,
            &filename,
            &window,
            [
                (up, Action::Up),
                (home, Action::Home),
                (go, Action::Go),
                (accept, Action::Accept),
                (cancel, Action::Cancel),
            ],
        );
        if let Err(error) = connected {
            // A failed construction must not leave an unreachable modal definition in the forest.
            let _ = window.close();
            return Err(error);
        }
        Ok(dialog)
    }

    /// Connects fixed controls exactly once; dynamic rows use the same typed activation helper when refreshed.
    fn connect(
        &self,
        path: &TextboxWidget,
        filename: &TextboxWidget,
        window: &ComPtr<IWindowAbi>,
        buttons: [(microui_redux::ButtonWidget, Action); 5],
    ) -> Result<(), NcStatus> {
        self.session.subscribe_text_changed(path, Edited::Path)?;
        self.session.subscribe_text_submitted(path, Navigate)?;
        self.session.subscribe_text_changed(filename, Edited::Filename)?;
        for (button, action) in buttons {
            self.session.subscribe_activation(&button, action)?;
        }
        self.session.subscribe_window(window.as_com_ref(), Lifecycle)?;
        Ok(())
    }

    /// Prepares both complete columns without publishing a partial listing when any directory entry fails.
    fn candidate(&self, model: &mut Model, directory: &Path) -> Result<Listing, NcStatus> {
        let runtime = model.owner()?;
        let entries = match read_directory(directory) {
            Ok(entries) => entries,
            Err(error) => {
                model.diagnostic = format!("Cannot read {}: {error}", directory.display());
                return Err(UI_E_IO);
            }
        };
        Listing::new(&runtime, directory, entries)
    }

    /// Registers complete candidates while the private session is idle, then replaces only dynamic content roles.
    fn install(&self, model: &mut Model, directory: PathBuf, listing: Listing, reset_scroll: bool) -> Result<(), NcStatus> {
        for (row, action) in listing.folders.rows.into_iter().chain(listing.files.rows) {
            self.session.subscribe_activation(&row, action)?;
        }
        // These are private built-in roles: their controller validates the live
        // modal before replacement and no caller can substitute a foreign child.
        model.folders.replace_content(&listing.folders.root)?.finish()?;
        model.files.replace_content(&listing.files.root)?.finish()?;
        model.directory = directory;
        model.selected = None;
        model.path_dirty = false;
        model.navigate = None;
        model.diagnostic.clear();
        model.path.set_text(&model.directory.to_string_lossy())?;
        model.filename.clear()?;
        model.error.clear()?;
        if reset_scroll {
            model.folders.set_offset(Point::default())?;
            model.files.set_offset(Point::default())?;
        }
        Ok(())
    }

    /// Opens only after the complete native request and directory listing validate successfully.
    fn open(&self, title: &str, rect: Rect, directory: PathBuf) -> Result<(), NcStatus> {
        let mut model = self.session.state_mut()?;
        model.owner()?;
        if self.request.active.get() {
            return Err(UI_E_BUSY);
        }
        if rect.width < 0 || rect.height < 0 {
            return Err(NC_E_INVALID_ARG);
        }
        let directory = std::path::absolute(directory).map_err(|error| {
            model.diagnostic = error.to_string();
            UI_E_IO
        })?;
        let listing = self.candidate(&mut model, &directory)?;
        // Modal eligibility is checked before replacing existing request data.
        model.window.set_visibility(Visibility::VISIBLE)?;
        self.install(&mut model, directory, listing, true)?;
        model.window.set_title(NcCopyFromBufferOffset::new(0, title.as_bytes()))?;
        model.window.set_rect(rect)?;
        self.request.active.set(true);
        Ok(())
    }

    /// Runs private control callbacks, then installs deferred navigation outside their registration transaction.
    pub(super) fn dispatch(&self, limit: u32) -> Result<u32, NcStatus> {
        let count = self.session.dispatch_bounded(limit)?;
        let mut model = self.session.state_mut()?;
        if let Some(directory) = model.navigate.take().filter(|_| self.request.active.get()) {
            match self.candidate(&mut model, &directory) {
                Ok(listing) => self.install(&mut model, directory, listing, false)?,
                Err(UI_E_IO) => {
                    // A failed read changes only diagnostics, preserving prior native
                    // directory, rows, selection and their actual COM capabilities.
                    let diagnostic = model.diagnostic.clone();
                    model.error.set_text(&diagnostic)?;
                    model.path_dirty = false;
                    let directory = model.directory.to_string_lossy().into_owned();
                    model.path.set_text(&directory)?;
                }
                Err(error) => return Err(error),
            }
        }
        Ok(count)
    }

    /// Finished request queues survive independently of their controller, so closed components retire immediately.
    pub(super) fn finished(&self) -> bool {
        self.request.closed.get()
    }
    /// Runtime teardown closes outstanding requests without borrowing a potentially active private handler model.
    pub(super) fn retire(&self) {
        self.request.retire();
    }
}
impl Drop for Dialog {
    /// Final controller destruction preserves one active-request cancellation for an already connected consumer.
    fn drop(&mut self) {
        self.request.retire();
    }
}

#[com_object(interfaces(IFileDialog, IFileDialogEvents))]
/// One public controller identity owns no application callback, generic state pointer or native filesystem borrow.
struct FileDialog {
    /// Runtime ownership keeps an active picker functional even if its caller releases the editing pointer.
    dialog: Rc<Dialog>,
}
impl IFileDialog for FileDialog {
    /// Returns the actual modal capability; application code cannot replace its private structural roles.
    fn window(&self) -> Result<ComPtr<IWindowAbi>, NcStatus> {
        Ok(self.dialog.session.state()?.window.clone())
    }
    /// Decodes native units before request mutation and preserves the prior request/listing on a read failure.
    fn open(&self, title: NcCopyFromBufferOffset<'_, u8>, rect: Rect, directory: ComRef<'_, INativePathAbi>) -> Result<(), NcStatus> {
        self.dialog.open(text(title)?, rect, microui_redux::path::decode(directory)?)
    }
    /// Reports request state independently of hidden versus visible window allocation.
    fn active(&self) -> Result<bool, NcStatus> {
        if self.dialog.request.closed.get() {
            return Err(UI_E_CLOSED);
        }
        Ok(self.dialog.request.active.get())
    }
    /// Explicit cancellation is idempotent and keeps the modal available for a later open.
    fn cancel(&self) -> Result<bool, NcStatus> {
        let mut model = self.dialog.session.state_mut()?;
        model.owner()?;
        let active = self.dialog.request.active.get();
        model.finish(FileDialogCompleted::Cancelled)?;
        Ok(active)
    }
    /// Actual window destruction produces the same lifecycle cancellation as destroying its owner.
    fn close(&self) -> Result<(), NcStatus> {
        if self.dialog.request.closed.get() {
            return Ok(());
        }
        self.dialog.session.state()?.window.close()?;
        self.dialog.request.closed.set(true);
        if self.dialog.request.active.replace(false) {
            self.dialog.request.completed.close(FileDialogCompleted::Cancelled);
        }
        Ok(())
    }
    /// Diagnostics remain available after failure or closure to explain a rejected directory candidate.
    fn diagnostic(&self) -> Result<ComPtr<INcStringAbi>, NcStatus> {
        Ok(NcString::from_utf8_str(&self.dialog.session.state()?.diagnostic))
    }
}
impl IFileDialogEvents for FileDialog {
    /// Returns the same completion source for every request without letting producers retain application state.
    fn subscribe(&self, sink: ComRef<'_, IFileDialogSinkAbi>) -> Result<ComPtr<IEventSubscriptionAbi>, NcStatus> {
        self.dialog.session.state()?.owner()?;
        super::super::events::subscribe(self.dialog.request.completed.event(), sink)
    }
}
impl IFileDialogs for Runtime {
    /// Creates and retains a concrete internal controller whose runtime reference remains weak.
    fn create(&self, parent: ComRef<'_, IWindowAbi>) -> Result<ComPtr<IFileDialogAbi>, NcStatus> {
        let dialog = Dialog::new(self, parent)?;
        self.storage.dialogs.borrow_mut().push(dialog.clone());
        Ok(FileDialog::from_value(FileDialog { dialog }).query().expect("file picker declares IFileDialog"))
    }
}

/// Reads the platform home setting without converting its native encoding to Unicode.
fn home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .filter(|home| !home.is_empty())
        .or_else(|| std::env::var_os("USERPROFILE").filter(|home| !home.is_empty()))
        .map(PathBuf::from)
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

#[cfg(test)]
mod tests;
