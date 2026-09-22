//! Standard picker composed by the independently deployed concrete-nodes provider.
use crate::factory::NodeServices;
use neocogi_ui_core_contracts::*;
use neocogi_ui_nodes_contracts::{ContentIcon, CrossSize, FlowDirection, LayoutTrack, LinearEdge};
use neocogi_ui::{LinearWidget, ListItemWidget, ScrollAreaWidget, TextBlockWidget, TextboxWidget};
use neocogi_com::{ComPtr, ComRef, LocalCom, INcStringAbi, NcCopyFromBufferOffset, NcStatus, NcString, NC_E_INVALID_ARG, com_object};
use std::{
    path::{Path, PathBuf},
    rc::Rc,
};
use crate::boundary::text;
use neocogi_ui::events::{Controller, EventModel};

/// Request state is an ordinary field of the exclusively borrowed picker model.
struct Request {
    /// One authoritative phase covers public calls, child delivery and retirement.
    phase: RequestPhase,
    /// Installed concrete completion receiver; consumers own their committed outcomes.
    completed: Option<ComPtr<IFileDialogObserverAbi>>,
}

/// Mutually exclusive picker-request lifecycle; no two booleans can disagree.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RequestPhase {
    /// Reusable controller has no request awaiting a user outcome.
    Idle,
    /// One request is awaiting acceptance or cancellation.
    Active,
    /// Logical controller/window closure permanently rejects new requests.
    Closed,
}
impl Request {
    /// Observes active request eligibility without lending a mutable model borrow.
    fn active(&self) -> bool {
        matches!(self.phase, RequestPhase::Active)
    }

    /// Observes permanent closure independently of retained COM references.
    fn closed(&self) -> bool {
        matches!(self.phase, RequestPhase::Closed)
    }

    /// Publishes a fully installed request only from the reusable idle phase.
    fn begin(&mut self) -> Result<(), NcStatus> {
        match self.phase {
            RequestPhase::Idle => {
                self.phase = RequestPhase::Active;
                Ok(())
            }
            RequestPhase::Active => Err(UI_E_BUSY),
            RequestPhase::Closed => Err(UI_E_CLOSED),
        }
    }

    /// Emits one committed outcome after the modal has become idle.
    fn finish(&mut self, status: FileDialogCompletion) -> Result<(), NcStatus> {
        match self.phase {
            RequestPhase::Active => {
                self.phase = RequestPhase::Idle;
                IFileDialogEvent::Completed(status).post_to(&mut self.completed)?;
            }
            RequestPhase::Idle | RequestPhase::Closed => {}
        }
        Ok(())
    }
    /// Retirement settles an active request and releases its destination.
    fn retire(&mut self) -> Result<(), NcStatus> {
        self.close()
    }

    /// Closes future completion delivery; already posted outcomes remain in the consumer inbox.
    fn close(&mut self) -> Result<(), NcStatus> {
        match std::mem::replace(&mut self.phase, RequestPhase::Closed) {
            RequestPhase::Active => IFileDialogEvent::Completed(FileDialogCompletion::Cancelled).post_to(&mut self.completed.take()),
            RequestPhase::Idle | RequestPhase::Closed => {
                self.completed = None;
                Ok(())
            }
        }
    }
}

/// Node construction data retained by the picker without retaining the core runtime identity.
struct DialogNodes {
    /// Posting access supplied to each private child during construction.
    events_tx: std::sync::mpsc::Sender<Message>,
    /// Non-owning host authority reports logical closure after the owning UI is released.
    host: ComPtr<IWidgetHostAbi>,
    /// Provider-shared imported resources used by dynamically rebuilt directory rows.
    resources: crate::widget_object::SharedAssets,
}

/// One authoritative picker model; live control values are read through their actual COM interfaces.
struct Model {
    /// Dynamic directory rows clone posting access when their outputs are connected.
    events_tx: std::sync::mpsc::Sender<Message>,
    /// Concrete-node construction data with no runtime or window-manager projection.
    nodes: Rc<DialogNodes>,
    /// Stable hidden/reusable modal installed after the eventful root is mounted.
    window: Option<ComPtr<IWindowAbi>>,
    /// Plain request state protected by the controller's existing COM borrow gate.
    request: Request,
    /// The actual shell detects logical closure before deferred model retirement.
    view: ComPtr<IWidgetAbi>,
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
    /// Stable acceptance control receives the request's Open or Save label.
    accept: neocogi_ui::ButtonWidget,
    /// Persistent visible error display; read failures preserve the prior listing and selection.
    error: TextBlockWidget,
    /// Persistent scrolling controllers; refreshing replaces only their application content roots.
    folders: ScrollAreaWidget,
    /// Separate file column keeps its own retained scroll request and clamping behavior.
    files: ScrollAreaWidget,
    /// Complete most recent directory-read diagnostic, also available through the component interface.
    diagnostic: String,
}
impl Model {
    /// Returns the modal after the one-time construction transaction has installed it.
    fn window(&self) -> Result<&ComPtr<IWindowAbi>, NcStatus> {
        self.window.as_ref().ok_or(UI_E_BUSY)
    }

    /// Rejects closed components and validates public runtime mutation authority.
    fn owner(&self) -> Result<(), NcStatus> {
        if self.request.closed() {
            return Err(UI_E_CLOSED);
        }
        self.nodes.host.check_mutation()?;
        self.view.properties()?;
        Ok(())
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
    /// Hides before emitting the outcome so later messages observe an idle reusable component.
    fn finish(&mut self, outcome: FileDialogCompletion) -> Result<(), NcStatus> {
        if self.request.active() {
            if self.window().is_ok_and(|window| window.snapshot().is_ok()) {
                self.window()?.set_visibility(Visibility::Hidden)?;
            }
            self.request.finish(outcome)?;
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
        // Capture the exact native identity once at commit, not through a lossy
        // display label or a dispatcher-time reconstruction of this model.
        self.finish(FileDialogCompletion::Accepted {
            path: neocogi_ui::NativePath::new(&path).as_interface().add_ref(),
        })
    }
}

/// Private component messages own every captured value, including exact native paths.
#[derive(Clone)]
enum Message {
    /// Navigate to the committed directory's parent.
    Up,
    /// Navigate to the platform's native home setting.
    Home,
    /// Resolve the path field when the Go control activates.
    Go,
    /// Accept the selected native file or nonempty typed filename.
    Accept,
    /// Complete the current request with cancellation.
    Cancel,
    /// Navigate using an exact row identity, independent of its display label.
    Folder(PathBuf),
    /// Select the exact native file represented by a row.
    File(PathBuf),
    /// Mark the path field as user-authored rather than programmatically displayed.
    PathEdited,
    /// Clear a native selection after a user edits the filename.
    FilenameEdited,
    /// Submit the captured path text without reading a later editor value.
    Navigate(String),
    /// Observe the existing modal's committed lifecycle fact.
    Lifecycle(WindowEvent),
}

/// One completely built column and its owned row commands, still unpublished to the modal.
struct Column {
    /// Actual COM content root to install into the stable scroll controller.
    root: LinearWidget,
}
/// Complete directory candidate; neither column becomes live if any native read fails.
struct Listing {
    /// Independently scrollable folders including the exact parent navigation row.
    folders: Column,
    /// Independently scrollable files, keeping native payloads separate from display names.
    files: Column,
}

/// Constructs an ordinary concrete Linear from heterogeneous existing COM widget capabilities.
fn line(
    nodes: &Rc<DialogNodes>,
    direction: FlowDirection,
    cross: CrossSize,
    children: Vec<(ComPtr<IWidgetAbi>, LayoutTrack)>,
) -> Result<LinearWidget, NcStatus> {
    let line = LinearWidget::from_interface(crate::Linear::create_com(nodes.host.clone(), nodes.resources.clone(), direction, cross.into())?);
    for (index, (child, track)) in children.into_iter().enumerate() {
        line.as_interface()
            .insert(
                index as u32,
                neocogi_ui_core_contracts::Mount::widget(child.as_com_ref()).as_message(),
                LinearEdge::new(track),
            )?
            .finish()?;
    }
    Ok(line)
}
/// Constructs a text display through the ordinary widget factory.
fn label(nodes: &Rc<DialogNodes>, value: &str) -> Result<TextBlockWidget, NcStatus> {
    Ok(TextBlockWidget::from_interface(crate::TextBlock::create_com(
        nodes.host.clone(),
        nodes.resources.clone(),
        NcCopyFromBufferOffset::new(0, value.as_bytes()),
        TextWrapping::None,
    )?))
}
/// Constructs a complete command Button through the dynamically implemented factory shorthand.
fn button(nodes: &Rc<DialogNodes>, value: &str, action: Message) -> Result<neocogi_ui::ButtonWidget, NcStatus> {
    let observer = neocogi_ui::button::observer(nodes.events_tx.clone(), move |_| action.clone())?;
    Ok(neocogi_ui::ButtonWidget::from_interface(crate::Button::create_com(
        nodes.host.clone(),
        nodes.resources.clone(),
        NcCopyFromBufferOffset::new(0, value.as_bytes()),
        WidgetOptions::FRAME,
        Some(observer.as_com_ref()),
    )?))
}
/// Builds a complete column with exact path actions and fixed ordinary list-row measurements.
fn column(nodes: &Rc<DialogNodes>, directory: &Path, paths: Vec<PathBuf>, folder: bool) -> Result<Column, NcStatus> {
    let mut children = vec![(label(nodes, if folder { "Folders" } else { "Files" })?.node_interface()?, LayoutTrack::Content)];
    let empty = paths.is_empty();
    for path in paths {
        let display = if folder && directory.parent() == Some(path.as_path()) {
            "..".into()
        } else {
            path.file_name().unwrap_or(path.as_os_str()).to_string_lossy()
        };
        let action = if folder { Message::Folder(path.clone()) } else { Message::File(path.clone()) };
        let observer = neocogi_ui::list_item::observer(nodes.events_tx.clone(), move |_| action.clone())?;
        let mut item = ListItemWidget::from_interface(crate::ListItem::create_com(
            nodes.host.clone(),
            nodes.resources.clone(),
            NcCopyFromBufferOffset::new(0, display.as_bytes()),
            Some(observer.as_com_ref()),
        )?);
        item.set_icon_role(if folder { ContentIcon::ClosedFolder } else { ContentIcon::File })?;
        children.push((item.node_interface()?, LayoutTrack::Content));
    }
    if empty {
        children.push((label(nodes, "(empty)")?.node_interface()?, LayoutTrack::Content));
    }
    Ok(Column {
        root: line(nodes, FlowDirection::TopToBottom, CrossSize::Stretch, children)?,
    })
}
impl Listing {
    /// Builds actual COM rows only after a complete native directory read succeeded.
    fn new(nodes: &Rc<DialogNodes>, directory: &Path, entries: (Vec<PathBuf>, Vec<PathBuf>)) -> Result<Self, NcStatus> {
        Ok(Self {
            folders: column(nodes, directory, entries.0, true)?,
            files: column(nodes, directory, entries.1, false)?,
        })
    }
}

impl FileDialog {
    /// Constructs one hidden modal from actual COM controls; no native widget handle or generic application state enters its ABI.
    fn new(
        services: Rc<NodeServices>,
        parent: ComRef<'_, IWindowAbi>,
        observer: Option<ComRef<'_, IFileDialogObserverAbi>>,
    ) -> Result<LocalCom<Self>, NcStatus> {
        services.host.check_mutation()?;
        let (events_tx, events) = std::sync::mpsc::channel();
        let nodes = Rc::new(DialogNodes {
            events_tx: events_tx.clone(),
            host: services.host.clone(),
            resources: services.resources.clone(),
        });
        let path_observer = neocogi_ui::textbox::observer(events_tx.clone(), |event| match event {
            neocogi_ui::textbox::Event::Changed(_) => Message::PathEdited,
            neocogi_ui::textbox::Event::Submitted(value) => Message::Navigate(value.text),
        })?;
        let filename_observer = neocogi_ui::textbox::observer(events_tx.clone(), |event| match event {
            neocogi_ui::textbox::Event::Changed(_) => Message::FilenameEdited,
            neocogi_ui::textbox::Event::Submitted(_) => Message::Accept,
        })?;
        let path = TextboxWidget::from_interface(crate::Textbox::create_com(
            nodes.host.clone(),
            nodes.resources.clone(),
            NcCopyFromBufferOffset::new(0, b""),
            Some(path_observer.as_com_ref()),
        )?);
        let filename = TextboxWidget::from_interface(crate::Textbox::create_com(
            nodes.host.clone(),
            nodes.resources.clone(),
            NcCopyFromBufferOffset::new(0, b""),
            Some(filename_observer.as_com_ref()),
        )?);
        let up = button(&nodes, "Up", Message::Up)?;
        let home = button(&nodes, "Home", Message::Home)?;
        let go = button(&nodes, "Go", Message::Go)?;
        let accept = button(&nodes, "Open", Message::Accept)?;
        let cancel = button(&nodes, "Cancel", Message::Cancel)?;
        let folder_content = column(&nodes, Path::new(""), Vec::new(), true)?;
        let file_content = column(&nodes, Path::new(""), Vec::new(), false)?;
        let folders = ScrollAreaWidget::from_interface(crate::ScrollArea::create_com(
            nodes.host.clone(),
            nodes.resources.clone(),
            neocogi_ui_core_contracts::Mount::widget(folder_content.root.node_interface()?.as_com_ref()).as_message(),
            true,
        )?);
        let files = ScrollAreaWidget::from_interface(crate::ScrollArea::create_com(
            nodes.host.clone(),
            nodes.resources.clone(),
            neocogi_ui_core_contracts::Mount::widget(file_content.root.node_interface()?.as_com_ref()).as_message(),
            true,
        )?);
        let error = label(&nodes, "")?;
        let toolbar = line(
            &nodes,
            FlowDirection::LeftToRight,
            CrossSize::Content,
            vec![
                (up.node_interface()?, LayoutTrack::Fixed { extent: 56 }),
                (home.node_interface()?, LayoutTrack::Fixed { extent: 56 }),
                (path.node_interface()?, LayoutTrack::Flex { weight: 1.0 }),
                (go.node_interface()?, LayoutTrack::Fixed { extent: 56 }),
            ],
        )?;
        let browser = line(
            &nodes,
            FlowDirection::LeftToRight,
            CrossSize::Stretch,
            vec![
                (folders.node_interface()?, LayoutTrack::Flex { weight: 1.0 }),
                (files.node_interface()?, LayoutTrack::Flex { weight: 2.0 }),
            ],
        )?;
        let filename_line = line(
            &nodes,
            FlowDirection::LeftToRight,
            CrossSize::Content,
            vec![
                (label(&nodes, "File name:")?.node_interface()?, LayoutTrack::Fixed { extent: 86 }),
                (filename.node_interface()?, LayoutTrack::Flex { weight: 1.0 }),
            ],
        )?;
        let actions = line(
            &nodes,
            FlowDirection::LeftToRight,
            CrossSize::Content,
            vec![
                (label(&nodes, "")?.node_interface()?, LayoutTrack::Flex { weight: 1.0 }),
                (cancel.node_interface()?, LayoutTrack::Fixed { extent: 96 }),
                (accept.node_interface()?, LayoutTrack::Fixed { extent: 96 }),
            ],
        )?;
        let shell = line(
            &nodes,
            FlowDirection::TopToBottom,
            CrossSize::Stretch,
            vec![
                (toolbar.node_interface()?, LayoutTrack::Content),
                (browser.node_interface()?, LayoutTrack::Flex { weight: 1.0 }),
                (filename_line.node_interface()?, LayoutTrack::Content),
                (error.node_interface()?, LayoutTrack::Content),
                (actions.node_interface()?, LayoutTrack::Content),
            ],
        )?;
        let request = Request {
            phase: RequestPhase::Idle,
            completed: observer.map(|destination| destination.add_ref()),
        };
        let model = Model {
            events_tx,
            nodes,
            window: None,
            request,
            view: shell.node_interface()?,
            directory: PathBuf::new(),
            path_dirty: false,
            selected: None,
            path: path.clone(),
            filename: filename.clone(),
            accept: accept.clone(),
            error,
            folders,
            files,
            diagnostic: String::new(),
        };
        let dialog = Self::into_local(Self {
            controller: Controller::new(model, events),
        });
        // The root owns this controller once attachment commits. The controller
        // owns ordinary view capabilities, whose host link does not retain the runtime.
        let mount = Mount::Component {
            view: shell.node_interface()?,
            owner: dialog.query()?,
        };
        let windows = services.windows.clone();
        let window = windows.create_dialog(parent, NcCopyFromBufferOffset::new(0, b"Open File"), FILE_DIALOG_RECT, mount.as_message())?;
        let connected = (|| {
            window.set_options(WindowOptions::FRAME)?;
            let mut local = dialog.try_borrow_mut()?;
            local.controller.model.window = Some(window.clone());
            neocogi_ui::events::bind(
                neocogi_ui::events::window_events(window.as_com_ref())?,
                local.controller.model.events_tx.clone(),
                Message::Lifecycle,
            )
        })();
        if let Err(error) = connected {
            // A failed construction must not leave an unreachable modal definition in the forest.
            let _ = window.close();
            let _ = dialog.try_borrow_mut()?.controller.retire();
            return Err(error);
        }
        Ok(dialog)
    }
}

impl Model {
    /// Prepares both complete columns without publishing a partial listing when any directory entry fails.
    fn candidate(&mut self, directory: &Path) -> Result<Listing, NcStatus> {
        self.owner()?;
        let entries = match read_directory(directory) {
            Ok(entries) => entries,
            Err(error) => {
                self.diagnostic = format!("Cannot read {}: {error}", directory.display());
                return Err(UI_E_IO);
            }
        };
        Listing::new(&self.nodes, directory, entries)
    }

    /// Registers complete candidates while the private inbox is idle, then replaces only dynamic content roles.
    fn install(&mut self, directory: PathBuf, listing: Listing, reset_scroll: bool) -> Result<(), NcStatus> {
        // These are private built-in roles: their controller validates the live
        // modal before replacement and no caller can substitute a foreign child.
        self.folders.replace_content(&listing.folders.root)?.finish()?;
        self.files.replace_content(&listing.files.root)?.finish()?;
        self.directory = directory;
        self.selected = None;
        self.path_dirty = false;
        self.diagnostic.clear();
        self.path.set_text(&self.directory.to_string_lossy())?;
        self.filename.clear()?;
        self.error.clear()?;
        if reset_scroll {
            self.folders.set_offset(Point::default())?;
            self.files.set_offset(Point::default())?;
        }
        Ok(())
    }

    /// Opens only after the complete native request and directory listing validate successfully.
    fn open(&mut self, title: &str, rect: Rect, directory: PathBuf, mode: FileDialogMode, filename: PathBuf) -> Result<(), NcStatus> {
        self.owner()?;
        if self.request.active() {
            return Err(UI_E_BUSY);
        }
        if rect.width < 0 || rect.height < 0 {
            return Err(NC_E_INVALID_ARG);
        }
        let action = match mode {
            FileDialogMode::Open => "Open",
            FileDialogMode::Save => "Save",
            _ => return Err(NC_E_INVALID_ARG),
        };
        // A suggestion is one native filename, never another route out of the
        // requested directory. Validation precedes every visible request change.
        if !filename.as_os_str().is_empty() && !matches!(filename.components().collect::<Vec<_>>().as_slice(), [std::path::Component::Normal(_)]) {
            return Err(NC_E_INVALID_ARG);
        }
        let directory = std::path::absolute(directory).map_err(|error| {
            self.diagnostic = error.to_string();
            UI_E_IO
        })?;
        let listing = self.candidate(&directory)?;
        // Modal eligibility is checked before replacing existing request data.
        self.window()?.set_visibility(Visibility::Visible)?;
        self.install(directory, listing, true)?;
        self.accept.set_label(action)?;
        if !filename.as_os_str().is_empty() {
            self.filename.set_text(&filename.to_string_lossy())?;
            // Preserve native path identity even when its display is lossy.
            self.selected = Some(self.directory.join(filename));
        }
        self.window()?.set_title(NcCopyFromBufferOffset::new(0, title.as_bytes()))?;
        self.window()?.set_rect(rect)?;
        self.request.begin()?;
        Ok(())
    }

    /// Handles one child message and completes any navigation before the next message.
    fn update(&mut self, message: Message) -> Result<(), NcStatus> {
        let directory = match message {
            Message::Lifecycle(event) => {
                match event {
                    WindowEvent::Hidden | WindowEvent::CloseRequested
                        if self.request.active()
                            && self
                                .window()
                                .is_ok_and(|window| window.snapshot().is_ok_and(|snapshot| snapshot.visibility == Visibility::Hidden)) =>
                    {
                        // An older hide fact must not cancel a newly visible request.
                        self.finish(FileDialogCompletion::Cancelled)?;
                    }
                    WindowEvent::Destroyed => self.request.close()?,
                    _ => {}
                }
                None
            }
            _ if !self.request.active() => None,
            Message::PathEdited => {
                self.path_dirty = true;
                None
            }
            Message::FilenameEdited => {
                self.selected = None;
                None
            }
            Message::Up => self.directory.parent().map(Path::to_owned),
            Message::Home => home_dir(),
            Message::Go => self.resolve_path(&self.path.text()?),
            Message::Accept => {
                self.accept()?;
                None
            }
            Message::Cancel => {
                self.finish(FileDialogCompletion::Cancelled)?;
                None
            }
            Message::Folder(path) => Some(path),
            Message::File(path) => {
                self.filename.set_text(&path.file_name().unwrap_or(path.as_os_str()).to_string_lossy())?;
                self.selected = Some(path);
                None
            }
            Message::Navigate(text) => self.resolve_path(&text),
        };
        if let Some(directory) = directory {
            match self.candidate(&directory) {
                Ok(listing) => self.install(directory, listing, false)?,
                Err(UI_E_IO) => {
                    // A failed candidate leaves the directory, rows and selection intact.
                    self.error.set_text(&self.diagnostic)?;
                    self.path_dirty = false;
                    self.path.set_text(&self.directory.to_string_lossy())?;
                }
                Err(error) => return Err(error),
            }
        }
        Ok(())
    }
}
impl EventModel for Model {
    type Message = Message;

    /// Child events enter the same mutable model used by the public picker capability.
    fn handle(&mut self, message: Message) -> Result<(), NcStatus> {
        self.update(message)
    }

    /// View removal cancels one outstanding request after any active handler returns.
    fn retire(&mut self) -> Result<(), NcStatus> {
        self.request.retire()
    }
}

#[com_object(interfaces(IFileDialog, IEventOwner))]
/// One allocation owns the picker model, its inbox and its public capabilities.
struct FileDialog {
    /// Ordinary owned state, exclusively accessed through this allocation's COM gate.
    controller: Controller<Model>,
}
impl IEventOwner for FileDialog {
    /// Commits the controller and its distinct concrete shell to one owning mount.
    fn mount(&mut self, view: ComRef<'_, IWidgetAbi>) -> Result<(), NcStatus> {
        self.controller.mount(view)
    }
    /// Handles one private child message without another COM allocation or shared model.
    fn dispatch_next(&mut self) -> Result<bool, NcStatus> {
        self.controller.dispatch_next()
    }
    /// Ends private delivery and settles the request exactly once after view removal.
    fn retire(&mut self) -> Result<(), NcStatus> {
        self.controller.retire()
    }
}
impl IFileDialog for FileDialog {
    /// Returns the actual modal capability without exposing private structural roles.
    fn window(&self) -> Result<ComPtr<IWindowAbi>, NcStatus> {
        Ok(self.controller.model.window()?.clone())
    }
    /// Decodes native values before mutating the request or replacing directory rows.
    fn open(
        &mut self,
        title: NcCopyFromBufferOffset<'_, u8>,
        rect: Rect,
        directory: ComRef<'_, INativePathAbi>,
        mode: FileDialogMode,
        filename: ComRef<'_, INativePathAbi>,
    ) -> Result<(), NcStatus> {
        self.controller.model.open(
            text(title)?,
            rect,
            neocogi_ui::path::decode(directory)?,
            mode,
            neocogi_ui::path::decode(filename)?,
        )
    }
    /// Rejects a closed view immediately, even before its deferred retirement runs.
    fn active(&self) -> Result<bool, NcStatus> {
        self.controller.model.view.properties()?;
        match self.controller.model.request.phase {
            RequestPhase::Closed => Err(UI_E_CLOSED),
            RequestPhase::Active => Ok(true),
            RequestPhase::Idle => Ok(false),
        }
    }
    /// Explicit cancellation is idempotent and leaves the modal available for reuse.
    fn cancel(&mut self) -> Result<bool, NcStatus> {
        let model = &mut self.controller.model;
        model.owner()?;
        let active = model.request.active();
        model.finish(FileDialogCompletion::Cancelled)?;
        Ok(active)
    }
    /// Removes the view and settles the request with the current exclusive model access.
    fn close(&mut self) -> Result<(), NcStatus> {
        if self.controller.model.request.closed() {
            return Ok(());
        }
        self.controller.model.window()?.close()?;
        // Runtime retirement may also be queued; Controller makes that later
        // call idempotent without recursively entering this public call.
        self.controller.retire()
    }
    /// Diagnostics remain readable after the component's terminal closure.
    fn diagnostic(&self) -> Result<ComPtr<INcStringAbi>, NcStatus> {
        Ok(NcString::from_utf8_str(&self.controller.model.diagnostic))
    }
}
/// Creates the one nodes-owned controller and returns its public editing capability.
pub(crate) fn create(
    services: Rc<NodeServices>,
    parent: ComRef<'_, IWindowAbi>,
    observer: Option<ComRef<'_, IFileDialogObserverAbi>>,
) -> Result<ComPtr<IFileDialogAbi>, NcStatus> {
    FileDialog::new(services, parent, observer)?.query()
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
