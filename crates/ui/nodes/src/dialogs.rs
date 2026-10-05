//! Standard picker composed by the independently deployed concrete-nodes provider.
use crate::factory::NodeServices;
use neocogi_ui_core_contracts::*;
use neocogi_ui_nodes_contracts::{ContentIcon, CrossSize, FlowDirection, LayoutTrack, LinearEdge};
use neocogi_ui::{ScrollAreaWidget, TextBlockWidget, TextboxWidget};
use neocogi_com::{ComBox, WeakComBox, ComPtr, ComRef, WeakLocalBox, INcStringComPtrExt, INcStringAbi, NcCopyFromBufferOffset, NcStatus, NcString, NC_E_INVALID_ARG, com_object};
use std::{
    path::{Path, PathBuf},
    rc::Rc,
};
use crate::boundary::text;
use crate::widget_object::WidgetEvents;
use neocogi_ui::OwnedWidget;
use std::sync::mpsc::{Receiver, TryRecvError};

/// Request state is an ordinary field of the exclusively borrowed picker model.
struct Request {
    /// Active versus idle request state; COM ownership alone controls destruction.
    phase: RequestPhase,
    /// Installed concrete completion receiver; consumers own their committed outcomes.
    completed: WidgetEvents,
}

/// Mutually exclusive picker-request lifecycle; no two booleans can disagree.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RequestPhase {
    /// Reusable dialog has no request awaiting a user outcome.
    Idle,
    /// One request is awaiting acceptance or cancellation.
    Active,
}
impl Request {
    /// Observes active request eligibility without lending a mutable model borrow.
    fn active(&self) -> bool {
        matches!(self.phase, RequestPhase::Active)
    }

    /// Publishes a fully installed request only from the reusable idle phase.
    fn begin(&mut self) -> Result<(), NcStatus> {
        match self.phase {
            RequestPhase::Idle => {
                self.phase = RequestPhase::Active;
                Ok(())
            }
            RequestPhase::Active => Err(UI_E_BUSY),
        }
    }

    /// Emits one committed outcome after the modal has become idle.
    fn finish(&mut self, snapshot: impl FnOnce() -> Result<ComBox<neocogi_com::events::IEventAbi>, NcStatus>) -> Result<(), NcStatus> {
        match self.phase {
            RequestPhase::Active => {
                self.phase = RequestPhase::Idle;
                if self.completed.is_bound() { self.completed.post(snapshot()?)?; }
            }
            RequestPhase::Idle => {}
        }
        Ok(())
    }

}

/// Node construction data retained by the picker without retaining the core runtime identity.
struct DialogNodes {
    /// One concrete receiver shared by all private children and directory rows.
    queue: ComPtr<neocogi_com::events::IEventReceiverAbi>,
    /// Non-owning host authority reports logical closure after the owning UI is released.
    host: ComPtr<IWidgetHostAbi>,
    /// Provider-shared imported resources used by dynamically rebuilt directory rows.
    resources: crate::widget_object::SharedAssets,
}

/// One authoritative picker model; live control values are read through their actual COM interfaces.
struct Model {
    /// Concrete-node construction data with no runtime or window-manager projection.
    nodes: Rc<DialogNodes>,
    /// Stable hidden/reusable modal installed after the eventful root is mounted.
    window: Option<ComPtr<IWindowAbi>>,
    /// Plain request state protected by the widget's existing COM borrow gate.
    request: Request,
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
    /// Toolbar identities and commands remain plain exclusively owned model data.
    buttons: Vec<(neocogi_com::WeakComBox<neocogi_ui::ButtonAbi>, Command)>,
    /// Only installed directory rows may resolve commands, even if old rows remain retained.
    rows: Vec<(neocogi_com::WeakComBox<neocogi_ui::ListItemAbi>, Command)>,
}
impl Model {
    /// Returns the modal after the one-time construction transaction has installed it.
    fn window(&self) -> Result<&ComPtr<IWindowAbi>, NcStatus> {
        self.window.as_ref().ok_or(UI_E_BUSY)
    }

    /// Validates runtime mutation authority; the ABI entry already protects lifetime.
    fn owner(&self) -> Result<(), NcStatus> {
        self.nodes.host.check_mutation()?;
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
    fn finish(&mut self, snapshot: impl FnOnce() -> Result<ComBox<neocogi_com::events::IEventAbi>, NcStatus>) -> Result<(), NcStatus> {
        if self.request.active() {
            if self.window().is_ok_and(|window| window.snapshot().is_ok()) {
                self.window()?.set_visibility(Visibility::Hidden)?;
            }
            self.request.finish(snapshot)?;
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
        self.finish(|| neocogi_ui_core_contracts::events::file_accepted(neocogi_ui::NativePath::new(&path).as_interface().add_ref()))
    }
}

/// Application commands resolved from private child lifetime identities.
#[derive(Clone)]
enum Command {
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
}

/// One completely built column and its owned row commands, still unpublished to the modal.
struct Column {
    /// Actual COM content root to install into the stable scroll controller.
    root: OwnedWidget<neocogi_ui::LinearAbi>,
    /// Exact row identities and actions prepared with this content candidate.
    actions: Vec<(neocogi_com::WeakComBox<neocogi_ui::ListItemAbi>, Command)>,
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
    children: Vec<(ComBox<IWidgetAbi>, LayoutTrack)>,
) -> Result<OwnedWidget<neocogi_ui::LinearAbi>, NcStatus> {
    let mut line = OwnedWidget::from_interface(crate::Linear::create_com(nodes.host.clone(), nodes.resources.clone(), direction, cross.into())?);
    for (index, (child, track)) in children.into_iter().enumerate() {
        // The heterogeneous list already contains IWidget owners. Use the
        // generated client directly; a discarded rejection needs no projection.
        line.as_interface_mut().insert(index as u32, child, LinearEdge::new(track))
            .map_err(|(status, _unpublished)| status)?;
    }
    Ok(line)
}
/// Constructs a text display through the ordinary widget factory.
fn label(nodes: &Rc<DialogNodes>, value: &str) -> Result<OwnedWidget<neocogi_ui::TextBlockAbi>, NcStatus> {
    Ok(OwnedWidget::from_interface(crate::TextBlock::create_com(
        nodes.host.clone(),
        nodes.resources.clone(),
        NcCopyFromBufferOffset::new(0, value.as_bytes()),
        TextWrapping::None,
    )?))
}
/// Constructs a complete command Button through the dynamically implemented factory shorthand.
fn button(nodes: &Rc<DialogNodes>, value: &str) -> Result<OwnedWidget<neocogi_ui::ButtonAbi>, NcStatus> {
    let queue = nodes.queue.clone();
    let button = OwnedWidget::from_interface(crate::Button::create_com(
        nodes.host.clone(),
        nodes.resources.clone(),
        NcCopyFromBufferOffset::new(0, value.as_bytes()),
        Some(queue),
    )?);
    Ok(button)
}
/// Builds a complete column with exact path actions and fixed ordinary list-row measurements.
fn column(nodes: &Rc<DialogNodes>, directory: &Path, paths: Vec<PathBuf>, folder: bool) -> Result<Column, NcStatus> {
    let mut children = vec![(label(nodes, if folder { "Folders" } else { "Files" })?.into_interface().query::<IWidgetAbi>().map_err(|(status, _unpublished)| status)?, LayoutTrack::Content)];
    let empty = paths.is_empty();
    let mut actions = Vec::with_capacity(paths.len());
    for path in paths {
        let display = if folder && directory.parent() == Some(path.as_path()) {
            "..".into()
        } else {
            path.file_name().unwrap_or(path.as_os_str()).to_string_lossy()
        };
        let action = if folder { Command::Folder(path.clone()) } else { Command::File(path.clone()) };
        let queue = nodes.queue.clone();
        let mut item = OwnedWidget::from_interface(crate::ListItem::create_com(
            nodes.host.clone(),
            nodes.resources.clone(),
            NcCopyFromBufferOffset::new(0, display.as_bytes()),
            Some(queue),
        )?);
        actions.push((item.as_interface().downgrade(), action));
        item.set_icon_role(if folder { ContentIcon::ClosedFolder } else { ContentIcon::File })?;
        children.push((item.into_interface().query::<IWidgetAbi>().map_err(|(status, _unpublished)| status)?, LayoutTrack::Content));
    }
    if empty {
        children.push((label(nodes, "(empty)")?.into_interface().query::<IWidgetAbi>().map_err(|(status, _unpublished)| status)?, LayoutTrack::Content));
    }
    Ok(Column {
        root: line(nodes, FlowDirection::TopToBottom, CrossSize::Stretch, children)?,
        actions,
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
    fn new(services: Rc<NodeServices>, parent: ComRef<'_, IWindowAbi>, queue: Option<ComPtr<neocogi_com::events::IEventReceiverAbi>>) -> Result<WeakLocalBox<Self>, NcStatus> {
        services.host.check_mutation()?;
        let (children, events) = neocogi_com::events::channel();
        let nodes = Rc::new(DialogNodes {
            queue: children,
            host: services.host.clone(),
            resources: services.resources.clone(),
        });
        let path_sink = nodes.queue.clone();
        let filename_sink = path_sink.clone();
        let path = OwnedWidget::from_interface(crate::Textbox::create_com(
            nodes.host.clone(),
            nodes.resources.clone(),
            NcCopyFromBufferOffset::new(0, b""),
            Some(path_sink),
        )?);
        let filename = OwnedWidget::from_interface(crate::Textbox::create_com(
            nodes.host.clone(),
            nodes.resources.clone(),
            NcCopyFromBufferOffset::new(0, b""),
            Some(filename_sink),
        )?);
        let up = button(&nodes, "Up")?;
        let home = button(&nodes, "Home")?;
        let go = button(&nodes, "Go")?;
        let accept = button(&nodes, "Open")?;
        let cancel = button(&nodes, "Cancel")?;
        let folder_content = column(&nodes, Path::new(""), Vec::new(), true)?;
        let file_content = column(&nodes, Path::new(""), Vec::new(), false)?;
        let folders = OwnedWidget::from_interface(crate::ScrollArea::create_com(
            nodes.host.clone(),
            nodes.resources.clone(),
            folder_content.root.into_interface().query::<IWidgetAbi>().map_err(|(status, _unpublished)| status)?.into(),
            true,
        ).map_err(|(status, _unpublished)| status)?);
        let files = OwnedWidget::from_interface(crate::ScrollArea::create_com(
            nodes.host.clone(),
            nodes.resources.clone(),
            file_content.root.into_interface().query::<IWidgetAbi>().map_err(|(status, _unpublished)| status)?.into(),
            true,
        ).map_err(|(status, _unpublished)| status)?);
        let error = label(&nodes, "")?;
        // Capture observers before each unique owner moves into the layout tree.
        let path_handle = path.handle();
        let filename_handle = filename.handle();
        let accept_handle = accept.handle();
        let error_handle = error.handle();
        let folders_handle = folders.handle();
        let files_handle = files.handle();
        let buttons = vec![
            (up.as_interface().downgrade(), Command::Up),
            (home.as_interface().downgrade(), Command::Home),
            (go.as_interface().downgrade(), Command::Go),
            (accept.as_interface().downgrade(), Command::Accept),
            (cancel.as_interface().downgrade(), Command::Cancel),
        ];
        let toolbar = line(
            &nodes,
            FlowDirection::LeftToRight,
            CrossSize::Content,
            vec![
                (up.into_interface().query::<IWidgetAbi>().map_err(|(status, _unpublished)| status)?, LayoutTrack::Fixed { extent: 56 }),
                (home.into_interface().query::<IWidgetAbi>().map_err(|(status, _unpublished)| status)?, LayoutTrack::Fixed { extent: 56 }),
                (path.into_interface().query::<IWidgetAbi>().map_err(|(status, _unpublished)| status)?, LayoutTrack::Flex { weight: 1.0 }),
                (go.into_interface().query::<IWidgetAbi>().map_err(|(status, _unpublished)| status)?, LayoutTrack::Fixed { extent: 56 }),
            ],
        )?;
        let browser = line(
            &nodes,
            FlowDirection::LeftToRight,
            CrossSize::Stretch,
            vec![
                (folders.into_interface().query::<IWidgetAbi>().map_err(|(status, _unpublished)| status)?, LayoutTrack::Flex { weight: 1.0 }),
                (files.into_interface().query::<IWidgetAbi>().map_err(|(status, _unpublished)| status)?, LayoutTrack::Flex { weight: 2.0 }),
            ],
        )?;
        let filename_line = line(
            &nodes,
            FlowDirection::LeftToRight,
            CrossSize::Content,
            vec![
                (label(&nodes, "File name:")?.into_interface().query::<IWidgetAbi>().map_err(|(status, _unpublished)| status)?, LayoutTrack::Fixed { extent: 86 }),
                (filename.into_interface().query::<IWidgetAbi>().map_err(|(status, _unpublished)| status)?, LayoutTrack::Flex { weight: 1.0 }),
            ],
        )?;
        let actions = line(
            &nodes,
            FlowDirection::LeftToRight,
            CrossSize::Content,
            vec![
                (label(&nodes, "")?.into_interface().query::<IWidgetAbi>().map_err(|(status, _unpublished)| status)?, LayoutTrack::Flex { weight: 1.0 }),
                (cancel.into_interface().query::<IWidgetAbi>().map_err(|(status, _unpublished)| status)?, LayoutTrack::Fixed { extent: 96 }),
                (accept.into_interface().query::<IWidgetAbi>().map_err(|(status, _unpublished)| status)?, LayoutTrack::Fixed { extent: 96 }),
            ],
        )?;
        // Embed the actual root layout value in FileDialog. It has no separate COM
        // allocation or controller/view lifetime; children remain ordinary COM widgets.
        let mut shell = crate::Linear::build(
            nodes.host.clone(),
            FlowDirection::TopToBottom,
            CrossSize::Stretch,
            Some(&nodes.resources),
        )?;
        for (index, (child, track)) in [
            (toolbar.into_interface().query::<IWidgetAbi>().map_err(|(status, _unpublished)| status)?, LayoutTrack::Content),
            (browser.into_interface().query::<IWidgetAbi>().map_err(|(status, _unpublished)| status)?, LayoutTrack::Flex { weight: 1.0 }),
            (filename_line.into_interface().query::<IWidgetAbi>().map_err(|(status, _unpublished)| status)?, LayoutTrack::Content),
            (error.into_interface().query::<IWidgetAbi>().map_err(|(status, _unpublished)| status)?, LayoutTrack::Content),
            (actions.into_interface().query::<IWidgetAbi>().map_err(|(status, _unpublished)| status)?, LayoutTrack::Content),
        ]
        .into_iter()
        .enumerate()
        {
            neocogi_ui_nodes_contracts::Linear::insert(&mut shell, index as u32, child.into(), LinearEdge::new(track)).map_err(|(status, _unpublished)| status)?;
        }
        let request = Request {
            phase: RequestPhase::Idle,
            completed: Default::default(),
        };
        let mut model = Model {
            nodes,
            window: None,
            request,
            directory: PathBuf::new(),
            path_dirty: false,
            selected: None,
            path: path_handle,
            filename: filename_handle,
            accept: accept_handle,
            error: error_handle,
            folders: folders_handle,
            files: files_handle,
            diagnostic: String::new(),
            rows: Vec::new(),
            buttons,
        };
        let dialog = Self::into_local_box_cyclic::<IFileDialogAbi>(move |origin| {
            model.request.completed = WidgetEvents::with_queue(queue, origin);
            Self { shell, model, messages: events }
        })?;
        let windows = services.windows.clone();
        let window_sink = dialog.try_borrow()?.model.nodes.queue.clone();
        let window_binding = neocogi_com::events::EventBinding::from_queue(Some(window_sink.as_com_ref()))?;
        let mut observer = dialog.downgrade();
        let root = dialog.into_interface::<IWidgetAbi>().map_err(|(status, _unpublished)| status)?;
        let window = windows.create_dialog(
            parent,
            NcCopyFromBufferOffset::new(0, b"Open File"),
            FILE_DIALOG_RECT,
            root,
            neocogi_com::NcValueRef::new(&window_binding)?,
        ).map_err(|(status, _unpublished)| status)?;
        let connected = (|| {
            window.set_options(WindowOptions::FRAME)?;
            observer.with_mut(|local| local.model.window = Some(window.clone()))?;
            Ok::<(), NcStatus>(())
        })();
        if let Err(error) = connected {
            // A failed construction must not leave an unreachable modal definition in the forest.
            let _ = window.close();
            return Err(error);
        }
        Ok(observer)
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
        // These are private built-in roles: their composite validates the live
        // modal before replacement and no caller can substitute a foreign child.
        // Fresh candidates are abandoned on failure after the receiving call
        // returns. Preserve IWidget through the operation instead of restoring a
        // Linear interface immediately before destroying a rejected candidate.
        let folders = listing.folders.root.into_interface().query::<IWidgetAbi>().map_err(|(status, _unpublished)| status)?;
        self.folders.as_interface_mut().replace_content(folders).map_err(|(status, _unpublished)| status)?;
        let files = listing.files.root.into_interface().query::<IWidgetAbi>().map_err(|(status, _unpublished)| status)?;
        self.files.as_interface_mut().replace_content(files).map_err(|(status, _unpublished)| status)?;
        self.rows = listing.folders.actions.into_iter().chain(listing.files.actions).collect();
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
    fn update(&mut self, event: neocogi_com::events::EventMessage) -> Result<(), NcStatus> {
        // Lifecycle observations remain useful after destruction. Authenticate the
        // modal identity before inspecting the fact; an old hide cannot cancel a
        // newer visible request.
        if let Some(origin) = event.origin.service::<IWindowAbi>()? {
            if origin.same_object(&self.window()?.downgrade()?) && self.request.active() {
                let close = event.downcast::<IWindowCloseRequestedEventAbi>()?.is_some();
                let hidden = event.downcast::<IWindowHiddenEventAbi>()?.is_some()
                    && self.window()?.snapshot().is_ok_and(|s| s.visibility == Visibility::Hidden);
                if close || hidden { self.finish(neocogi_ui_core_contracts::events::file_cancelled)?; }
            }
            return Ok(());
        }
        let command = if let Some(origin) = event.origin.widget::<neocogi_ui::ButtonAbi>()? {
            if !origin.is_alive() || event.downcast::<IActivatedEventAbi>()?.is_none() { return Ok(()); }
            self.buttons.iter().find(|(control, _)| control.same_object(&origin)).map(|(_, action)| action.clone())
        } else if let Some(origin) = event.origin.widget::<neocogi_ui::ListItemAbi>()? {
            if !origin.is_alive() || event.downcast::<IActivatedEventAbi>()?.is_none() { return Ok(()); }
            self.rows.iter().find(|(control, _)| control.same_object(&origin)).map(|(_, action)| action.clone())
        } else if let Some(origin) = event.origin.widget::<neocogi_ui::TextboxAbi>()? {
            if !origin.is_alive() { return Ok(()); }
            let is_path = origin.same_object(self.path.as_interface());
            let is_filename = origin.same_object(self.filename.as_interface());
            if !is_path && !is_filename { return Ok(()); }
            if event.downcast::<ITextChangedEventAbi>()?.is_some() {
                Some(if is_path { Command::PathEdited } else { Command::FilenameEdited })
            } else if let Some(submitted) = event.downcast::<ITextSubmittedEventAbi>()? {
                Some(if is_path { Command::Navigate(submitted.text()?.to_string()?) } else { Command::Accept })
            } else { None }
        } else { None };
        let Some(message) = command else { return Ok(()); };
        let directory = match message {
            _ if !self.request.active() => None,
            Command::PathEdited => {
                self.path_dirty = true;
                None
            }
            Command::FilenameEdited => {
                self.selected = None;
                None
            }
            Command::Up => self.directory.parent().map(Path::to_owned),
            Command::Home => home_dir(),
            Command::Go => self.resolve_path(&self.path.text()?),
            Command::Accept => {
                self.accept()?;
                None
            }
            Command::Cancel => {
                self.finish(neocogi_ui_core_contracts::events::file_cancelled)?;
                None
            }
            Command::Folder(path) => Some(path),
            Command::File(path) => {
                self.filename.set_text(&path.file_name().unwrap_or(path.as_os_str()).to_string_lossy())?;
                self.selected = Some(path);
                None
            }
            Command::Navigate(text) => self.resolve_path(&text),
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
#[com_object(interfaces(IFileDialog, IWidget, IContainerLayout, IWidgetDispatcher), ownership = exclusive)]
/// One concrete composite owns its layout, model and inbox under one COM borrow gate.
struct FileDialog {
    /// Inline root layout: direct Rust delegation does not reenter a COM projection.
    shell: crate::Linear,
    /// Authoritative picker state, including typed child handles and completion mapping.
    model: Model,
    /// Sole receiving endpoint, destroyed with this exclusively owned value.
    messages: Receiver<neocogi_com::events::EventMessage>,
}

/// Optional dispatch delegates directly to the same object, with no COM reentry.
impl IWidgetDispatcher for FileDialog {
    /// Receives one child fact with ordinary exclusive access to the model.
    fn dispatch_next(&mut self) -> Result<bool, NcStatus> {
        IWidget::properties(&self.shell)?;
        self.model.nodes.host.check_mutation()?;
        match self.messages.try_recv() {
            Ok(message) => {
                self.model.update(message)?;
                Ok(true)
            }
            Err(TryRecvError::Empty | TryRecvError::Disconnected) => Ok(false),
        }
    }
}

impl IWidget for FileDialog {
    /// Returns the allocating runtime without retaining its tree.
    fn host(&self) -> Result<ComPtr<IWidgetHostAbi>, NcStatus> {
        IWidget::host(&self.shell)
    }
    /// The inline layout carries the composite's one attachment state.
    fn mount(&mut self) -> Result<(), NcStatus> {
        IWidget::mount(&mut self.shell)
    }
    /// Uses the root layout's authoritative options and interaction policy.
    fn properties(&self) -> Result<WidgetProperties, NcStatus> {
        IWidget::properties(&self.shell)
    }
    /// Reads the requested configuration without including transient runtime policy.
    fn requested_flags(&self) -> Result<WidgetOptions, NcStatus> {
        IWidget::requested_flags(&self.shell)
    }
    /// Replaces only requested flags on the actual inline root state.
    fn replace_flags(&mut self, flags: WidgetOptions) -> Result<(), NcStatus> {
        IWidget::replace_flags(&mut self.shell, flags)
    }
    /// Sets selected flags without restoring a captured option snapshot.
    fn set_flags(&mut self, flags: WidgetOptions) -> Result<(), NcStatus> {
        IWidget::set_flags(&mut self.shell, flags)
    }
    /// Clears selected flags on that same state.
    fn clear_flags(&mut self, flags: WidgetOptions) -> Result<(), NcStatus> {
        IWidget::clear_flags(&mut self.shell, flags)
    }
    /// Directly runs the root layout's validated input phase, without a second COM call.
    fn update(&mut self, state: WidgetUpdate, event: WidgetEvent, text: NcCopyFromBufferOffset<'_, u8>) -> Result<GeometryChange, NcStatus> {
        IWidget::update(&mut self.shell, state, event, text)
    }
    /// Drains the one authoritative geometry marker.
    fn take_change(&mut self) -> Result<GeometryChange, NcStatus> {
        IWidget::take_change(&mut self.shell)
    }
    /// Records the root appearance using the caller's scoped painting authority.
    fn paint(&mut self, scope: PaintScope, state: WidgetPaint, painter: ComRef<'_, IPainterAbi>) -> Result<(), NcStatus> {
        IWidget::paint(&mut self.shell, scope, state, painter)
    }
}
impl IContainerLayout for FileDialog {
    /// Publishes the exact child collection whose writer is embedded in the root layout.
    fn children(&self) -> Result<ComPtr<IChildrenAbi>, NcStatus> {
        IContainerLayout::children(&self.shell)
    }
    /// Reuses Linear's algorithm directly in the composite's allocation.
    fn measure(&mut self, scope: LayoutScope, bounds: LayoutLimits, service: ComRef<'_, ILayoutServiceAbi>) -> Result<Size, NcStatus> {
        IContainerLayout::measure(&mut self.shell, scope, bounds, service)
    }
    /// Places the same children under the caller's scoped layout transaction.
    fn place(&mut self, scope: LayoutScope, rect: Rect, service: ComRef<'_, ILayoutServiceAbi>) -> Result<ContainerGeometry, NcStatus> {
        IContainerLayout::place(&mut self.shell, scope, rect, service)
    }
    /// Preserves ordinary root event filtering without borrowing another object.
    fn accepts_event(&self, event: WidgetEvent, text: NcCopyFromBufferOffset<'_, u8>) -> Result<bool, NcStatus> {
        IContainerLayout::accepts_event(&self.shell, event, text)
    }
}
impl IFileDialog for FileDialog {
    /// Returns the actual modal capability without exposing private structural roles.
    fn window(&self) -> Result<ComPtr<IWindowAbi>, NcStatus> {
        Ok(self.model.window()?.clone())
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
        self.model.open(
            text(title)?,
            rect,
            neocogi_ui::path::decode(directory)?,
            mode,
            neocogi_ui::path::decode(filename)?,
        )?;
        // A successful new request supersedes pending interaction from the old
        // one, including a queued caption intent. Clear only this private inbox;
        // completed results already posted to the application remain untouched.
        // Failed opens return above without discarding queued messages.
        while self.messages.try_recv().is_ok() {}
        Ok(())
    }
    /// Reports request activity; expired owners are rejected by the ABI entry.
    fn active(&self) -> Result<bool, NcStatus> {
        IWidget::properties(&self.shell)?;
        match self.model.request.phase {
            RequestPhase::Active => Ok(true),
            RequestPhase::Idle => Ok(false),
        }
    }
    /// Explicit cancellation is idempotent and leaves the modal available for reuse.
    fn cancel(&mut self) -> Result<bool, NcStatus> {
        let model = &mut self.model;
        model.owner()?;
        let active = model.request.active();
        model.finish(neocogi_ui_core_contracts::events::file_cancelled)?;
        Ok(active)
    }
    /// Copies diagnostics while the dialog owner exists; expired handles fail at entry.
    fn diagnostic(&self) -> Result<ComPtr<INcStringAbi>, NcStatus> {
        Ok(NcString::from_utf8_str(&self.model.diagnostic))
    }
}
/// Creates the one concrete composite widget and returns its public editing capability.
pub(crate) fn create(
    services: Rc<NodeServices>,
    parent: ComRef<'_, IWindowAbi>,
    queue: Option<ComPtr<neocogi_com::events::IEventReceiverAbi>>,
) -> Result<WeakComBox<IFileDialogAbi>, NcStatus> {
    FileDialog::new(services, parent, queue)?.query()
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
