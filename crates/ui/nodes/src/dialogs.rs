//! Standard picker composed by the independently deployed concrete-nodes provider.
use crate::factory::NodeServices;
use neocogi_ui_core_contracts::*;
use neocogi_ui_nodes_contracts::{ContentIcon, CrossSize, FlowDirection, LayoutTrack, LinearEdge};
use neocogi_ui::{Events, LinearWidget, ListItemWidget, ScrollAreaWidget, TextBlockWidget, TextboxWidget};
use neocogi_com::{ComPtr, ComRef, INcStringAbi, NcCopyFromBufferOffset, NcStatus, NcString, NC_E_INVALID_ARG, com_object};
use std::{
    cell::RefCell,
    path::{Path, PathBuf},
    rc::Rc,
};
use crate::boundary::text;

/// Shared request lifetime contains no control pointers, handlers or runtime ownership.
struct Request {
    /// One request phase is shared by the controller, private model and teardown.
    /// The enclosing RefCell provides their single checked mutation boundary.
    phase: RequestPhase,
    /// Installed concrete completion receiver; consumers own their committed outcomes.
    completed: Option<ComPtr<__FileDialogCompletionReceiverAbi>>,
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
                status.post_to(&mut self.completed)?;
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
            RequestPhase::Active => FileDialogCompletion::Cancelled.post_to(&mut self.completed.take()),
            RequestPhase::Idle | RequestPhase::Closed => {
                self.completed = None;
                Ok(())
            }
        }
    }
}

#[com_object(interfaces(IWidget, IContainerLayout, IComponentEventProvider))]
/// Transparent dialog root that adds a private dispatcher to one concrete Linear allocation.
///
/// Layout, routing, paint and lifetime remain owned by the wrapped concrete container. The only
/// added capability is the separate dispatcher identity discovered by the production runtime when
/// this root is mounted, so the nodes plugin requires no private runtime controller registry.
struct DialogRoot {
    /// Common widget projection of the exact concrete shell allocation.
    widget: ComPtr<IWidgetAbi>,
    /// Container projection of that same shell allocation.
    layout: ComPtr<IContainerLayoutAbi>,
    /// Separate component-owned dispatcher allocation, never the root's IUnknown identity.
    dispatcher: ComPtr<IEventDispatcherAbi>,
    /// Terminal component closure settles an active request even after this driver leaves the forest.
    request: Rc<RefCell<Request>>,
}

impl DialogRoot {
    /// Captures every required shell capability before publishing the wrapper object.
    fn create(shell: &LinearWidget, dispatcher: ComPtr<IEventDispatcherAbi>, request: Rc<RefCell<Request>>) -> Result<ComPtr<IWidgetAbi>, NcStatus> {
        let shell = shell.as_interface().add_ref();
        let value = Self {
            widget: shell.query()?,
            layout: shell.query()?,
            dispatcher,
            request,
        };
        Ok(Self::from_value(value).query().expect("DialogRoot declares IWidget"))
    }
}

impl IComponentEventProvider for DialogRoot {
    /// Returns the component's distinct framework dispatcher for runtime boundary dispatch.
    fn private_dispatcher(&self) -> Result<ComPtr<IEventDispatcherAbi>, NcStatus> {
        Ok(self.dispatcher.clone())
    }
}

impl IWidget for DialogRoot {
    /// Delegates runtime ownership to the concrete shell.
    fn host(&self) -> Result<ComPtr<IWidgetHostAbi>, NcStatus> {
        self.widget.host()
    }

    /// Claims the concrete shell's one-way attachment exactly once.
    fn mount(&mut self) -> Result<(), NcStatus> {
        self.widget.mount()
    }

    /// Closes the concrete shell and its complete child topology.
    fn close(&mut self) -> Result<(), NcStatus> {
        let result = self.widget.close();
        let completion = self.request.try_borrow_mut().map_err(|_| UI_E_BUSY)?.close();
        result.and(completion)
    }

    /// Returns the shell's routing and frame policy without a shadow copy.
    fn properties(&self) -> Result<WidgetProperties, NcStatus> {
        self.widget.properties()
    }

    /// Applies common presentation options to the concrete shell.
    fn set_options(&mut self, options: WidgetOptions) -> Result<(), NcStatus> {
        self.widget.set_options(options)
    }

    /// Forwards one routed transaction to the concrete shell allocation.
    fn update(&mut self, state: WidgetUpdate, event: WidgetEventAbi, text: NcCopyFromBufferOffset<'_, u8>) -> Result<GeometryChange, NcStatus> {
        self.widget.update(state, event, text)
    }

    /// Drains the shell's single sticky geometry-change source.
    fn take_change(&mut self) -> Result<GeometryChange, NcStatus> {
        self.widget.take_change()
    }

    /// Forwards scoped paint directly to the concrete shell.
    fn paint(&mut self, scope: PaintScope, state: WidgetPaint, painter: ComRef<'_, IPainterAbi>) -> Result<(), NcStatus> {
        self.widget.paint(scope, state, painter)
    }
}

impl IContainerLayout for DialogRoot {
    /// Returns the concrete shell's stable child collection identity.
    fn children(&self) -> Result<ComPtr<IChildrenAbi>, NcStatus> {
        self.layout.children()
    }

    /// Delegates intrinsic measurement without translating layout scopes.
    fn measure(&mut self, scope: LayoutScope, bounds: LayoutLimits, layout: ComRef<'_, ILayoutServiceAbi>) -> Result<Size, NcStatus> {
        self.layout.measure(scope, bounds, layout)
    }

    /// Delegates child placement and returns the shell's exact copied geometry.
    fn place(&mut self, scope: LayoutScope, rect: Rect, layout: ComRef<'_, ILayoutServiceAbi>) -> Result<ContainerGeometry, NcStatus> {
        self.layout.place(scope, rect, layout)
    }

    /// Preserves the concrete shell's ancestor-bubbling filter.
    fn accepts_event(&self, event: WidgetEventAbi, text: NcCopyFromBufferOffset<'_, u8>) -> Result<bool, NcStatus> {
        self.layout.accepts_event(event, text)
    }
}

/// Node-plugin controller owns one private inbox and ordinary public COM capabilities.
struct Dialog {
    /// Ordinary mutable model shared only by the controller and its separate mounted driver.
    model: Rc<RefCell<Model>>,
    /// Shared independently of an active mutable model borrow during runtime retirement.
    request: Rc<RefCell<Request>>,
}

/// Node construction data retained by the picker without retaining the core runtime identity.
struct DialogNodes {
    /// Non-owning host authority reports logical closure after the owning UI is released.
    host: ComPtr<IWidgetHostAbi>,
    /// Provider-shared imported resources used by dynamically rebuilt directory rows.
    resources: crate::widget_object::SharedAssets,
}

/// One authoritative picker model; live control values are read through their actual COM interfaces.
struct Model {
    /// Private inbox combines child protocols in actual emission order; state is borrowed only by its consumer.
    events: Events<Message>,
    /// Concrete-node construction data with no runtime or window-manager projection.
    nodes: Rc<DialogNodes>,
    /// Stable hidden/reusable modal installed after the eventful root is mounted.
    window: Option<ComPtr<IWindowAbi>>,
    /// Request source and activity shared with the controller's lifetime operations.
    request: Rc<RefCell<Request>>,
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
        if self.request.try_borrow().map_err(|_| UI_E_BUSY)?.closed() {
            return Err(UI_E_CLOSED);
        }
        self.nodes.host.check_mutation()?;
        self.window()?.snapshot()?;
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
        if self.request.try_borrow().map_err(|_| UI_E_BUSY)?.active() {
            if self.window().is_ok_and(|window| window.snapshot().is_ok()) {
                self.window()?.set_visibility(Visibility::Hidden)?;
            }
            self.request.try_borrow_mut().map_err(|_| UI_E_BUSY)?.finish(outcome)?;
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

/// Private component messages. Only synchronous delivery borrows captured text
/// or lifecycle data; configured row messages own their exact native path.
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
    /// Concrete exact-path actions registered only after the complete directory candidate succeeds.
    rows: Vec<(ListItemWidget, Message)>,
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
        line.as_interface().insert(index as u32, child.as_com_ref(), LinearEdge::new(track))?.finish()?;
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
fn button(nodes: &Rc<DialogNodes>, value: &str) -> Result<neocogi_ui::ButtonWidget, NcStatus> {
    Ok(neocogi_ui::ButtonWidget::from_interface(crate::Button::create_com(
        nodes.host.clone(),
        nodes.resources.clone(),
        NcCopyFromBufferOffset::new(0, value.as_bytes()),
        WidgetOptions::FRAME,
    )?))
}
/// Builds a complete column with exact path actions and fixed ordinary list-row measurements.
fn column(nodes: &Rc<DialogNodes>, directory: &Path, paths: Vec<PathBuf>, folder: bool) -> Result<Column, NcStatus> {
    let mut children = vec![(label(nodes, if folder { "Folders" } else { "Files" })?.node_interface()?, LayoutTrack::Content)];
    let mut rows = Vec::with_capacity(paths.len());
    for path in paths {
        let display = if folder && directory.parent() == Some(path.as_path()) {
            "..".into()
        } else {
            path.file_name().unwrap_or(path.as_os_str()).to_string_lossy()
        };
        let mut item = ListItemWidget::from_interface(crate::ListItem::create_com(
            nodes.host.clone(),
            nodes.resources.clone(),
            NcCopyFromBufferOffset::new(0, display.as_bytes()),
        )?);
        item.set_icon_role(if folder { ContentIcon::ClosedFolder } else { ContentIcon::File })?;
        children.push((item.node_interface()?, LayoutTrack::Content));
        rows.push((item, if folder { Message::Folder(path) } else { Message::File(path) }));
    }
    if rows.is_empty() {
        children.push((label(nodes, "(empty)")?.node_interface()?, LayoutTrack::Content));
    }
    Ok(Column {
        root: line(nodes, FlowDirection::TopToBottom, CrossSize::Stretch, children)?,
        rows,
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

impl Dialog {
    /// Constructs one hidden modal from actual COM controls; no native widget handle or generic application state enters its ABI.
    fn new(services: Rc<NodeServices>, parent: ComRef<'_, IWindowAbi>) -> Result<Rc<Self>, NcStatus> {
        services.host.check_mutation()?;
        let nodes = Rc::new(DialogNodes {
            host: services.host.clone(),
            resources: services.resources.clone(),
        });
        let path = TextboxWidget::from_interface(crate::Textbox::create_com(
            nodes.host.clone(),
            nodes.resources.clone(),
            NcCopyFromBufferOffset::new(0, b""),
        )?);
        let filename = TextboxWidget::from_interface(crate::Textbox::create_com(
            nodes.host.clone(),
            nodes.resources.clone(),
            NcCopyFromBufferOffset::new(0, b""),
        )?);
        let up = button(&nodes, "Up")?;
        let home = button(&nodes, "Home")?;
        let go = button(&nodes, "Go")?;
        let accept = button(&nodes, "Open")?;
        let cancel = button(&nodes, "Cancel")?;
        let folder_content = column(&nodes, Path::new(""), Vec::new(), true)?;
        let file_content = column(&nodes, Path::new(""), Vec::new(), false)?;
        let folders = ScrollAreaWidget::from_interface(crate::ScrollArea::create_com(
            nodes.host.clone(),
            nodes.resources.clone(),
            folder_content.root.node_interface()?.as_com_ref(),
            true,
        )?);
        let files = ScrollAreaWidget::from_interface(crate::ScrollArea::create_com(
            nodes.host.clone(),
            nodes.resources.clone(),
            file_content.root.node_interface()?.as_com_ref(),
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
        let request = Rc::new(RefCell::new(Request {
            phase: RequestPhase::Idle,
            completed: None,
        }));
        let model = Rc::new(RefCell::new(Model {
            events: Events::new(),
            nodes,
            window: None,
            request: request.clone(),
            directory: PathBuf::new(),
            path_dirty: false,
            selected: None,
            path: path.clone(),
            filename: filename.clone(),
            error,
            folders,
            files,
            diagnostic: String::new(),
        }));
        let dialog = Rc::new(Self { model, request });
        let driver = dialog.model.borrow().events.component_dispatcher(&dialog.model, Model::update)?;
        let root = DialogRoot::create(&shell, driver, dialog.request.clone())?;
        let windows = services.windows.clone();
        let window = windows.create_dialog(parent, NcCopyFromBufferOffset::new(0, b"Open File"), FILE_DIALOG_RECT, root.as_com_ref())?;
        window.set_options(WindowOptions::FRAME)?;
        dialog.model.try_borrow_mut().map_err(|_| UI_E_BUSY)?.window = Some(window.clone());
        let connected = dialog.model.try_borrow_mut().map_err(|_| UI_E_BUSY)?.connect(
            &path,
            &filename,
            &window,
            [
                (up, Message::Up),
                (home, Message::Home),
                (go, Message::Go),
                (accept, Message::Accept),
                (cancel, Message::Cancel),
            ],
        );
        if let Err(error) = connected {
            // A failed construction must not leave an unreachable modal definition in the forest.
            let _ = window.close();
            return Err(error);
        }
        Ok(dialog)
    }
}

impl Model {
    /// Connects fixed controls exactly once; each callback translates its typed event into the private update protocol.
    fn connect(
        &mut self,
        path: &TextboxWidget,
        filename: &TextboxWidget,
        window: &ComPtr<IWindowAbi>,
        buttons: [(neocogi_ui::ButtonWidget, Message); 5],
    ) -> Result<(), NcStatus> {
        let events = &mut self.events;
        events.map(path.changed(), |_| Message::PathEdited)?;
        events.map(path.submitted(), |event| Message::Navigate(event.text))?;
        events.map(filename.changed(), |_| Message::FilenameEdited)?;
        for (button, action) in buttons {
            events.map(button.activated(), move |_| action.clone())?;
        }
        events.map(neocogi_ui::events::window_events(window.as_com_ref())?, |event| Message::Lifecycle(event))?;
        Ok(())
    }

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
        for (row, action) in listing.folders.rows.into_iter().chain(listing.files.rows) {
            self.events.map(row.activated(), move |_| action.clone())?;
        }
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
    fn open(&mut self, title: &str, rect: Rect, directory: PathBuf) -> Result<(), NcStatus> {
        self.owner()?;
        if self.request.try_borrow().map_err(|_| UI_E_BUSY)?.active() {
            return Err(UI_E_BUSY);
        }
        if rect.width < 0 || rect.height < 0 {
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
        self.window()?.set_title(NcCopyFromBufferOffset::new(0, title.as_bytes()))?;
        self.window()?.set_rect(rect)?;
        self.request.try_borrow_mut().map_err(|_| UI_E_BUSY)?.begin()?;
        Ok(())
    }

    /// Handles one child message and completes any navigation before the next message.
    fn update(&mut self, message: Message) -> Result<(), NcStatus> {
        let directory = match message {
            Message::Lifecycle(event) => {
                match event {
                    WindowEvent::Hidden | WindowEvent::CloseRequested
                        if self.request.try_borrow().map_err(|_| UI_E_BUSY)?.active()
                            && self
                                .window()
                                .is_ok_and(|window| window.snapshot().is_ok_and(|snapshot| snapshot.visibility == Visibility::Hidden)) =>
                    {
                        // An older hide fact must not cancel a newly visible request.
                        self.finish(FileDialogCompletion::Cancelled)?;
                    }
                    WindowEvent::Destroyed => self.request.try_borrow_mut().map_err(|_| UI_E_BUSY)?.close()?,
                    _ => {}
                }
                None
            }
            _ if !self.request.try_borrow().map_err(|_| UI_E_BUSY)?.active() => None,
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
impl Drop for Dialog {
    /// Final controller destruction closes its mounted modal and preserves one terminal cancellation.
    fn drop(&mut self) {
        if let Ok(model) = self.model.try_borrow()
            && let Some(window) = &model.window
        {
            let _ = window.close();
        }
        if let Ok(mut request) = self.request.try_borrow_mut() {
            let _ = request.retire();
        }
    }
}

#[com_object(interfaces(IFileDialog))]
/// One public controller identity owns no application callback, generic state pointer or native filesystem borrow.
struct FileDialog {
    /// Runtime ownership keeps an active picker functional even if its caller releases the editing pointer.
    dialog: Rc<Dialog>,
}
impl IFileDialog for FileDialog {
    /// Installs one concrete destination in the shared request's actual state.
    fn __map_completed(&mut self, destination: ComRef<'_, __FileDialogCompletionReceiverAbi>) -> Result<(), NcStatus> {
        self.dialog.model.try_borrow().map_err(|_| UI_E_BUSY)?.owner()?;
        self.dialog.request.try_borrow_mut().map_err(|_| UI_E_BUSY)?.completed = Some(destination.add_ref());
        Ok(())
    }

    /// Returns the actual modal capability; application code cannot replace its private structural roles.
    fn window(&self) -> Result<ComPtr<IWindowAbi>, NcStatus> {
        Ok(self.dialog.model.try_borrow().map_err(|_| UI_E_BUSY)?.window()?.clone())
    }
    /// Decodes native units before request mutation and preserves the prior request/listing on a read failure.
    fn open(&self, title: NcCopyFromBufferOffset<'_, u8>, rect: Rect, directory: ComRef<'_, INativePathAbi>) -> Result<(), NcStatus> {
        self.dialog
            .model
            .try_borrow_mut()
            .map_err(|_| UI_E_BUSY)?
            .open(text(title)?, rect, neocogi_ui::path::decode(directory)?)
    }
    /// Reports request state independently of hidden versus visible window allocation.
    fn active(&self) -> Result<bool, NcStatus> {
        let request = self.dialog.request.try_borrow().map_err(|_| UI_E_BUSY)?;
        match request.phase {
            RequestPhase::Closed => Err(UI_E_CLOSED),
            RequestPhase::Active => Ok(true),
            RequestPhase::Idle => Ok(false),
        }
    }
    /// Explicit cancellation is idempotent and keeps the modal available for a later open.
    fn cancel(&self) -> Result<bool, NcStatus> {
        let mut model = self.dialog.model.try_borrow_mut().map_err(|_| UI_E_BUSY)?;
        model.owner()?;
        let active = self.dialog.request.try_borrow().map_err(|_| UI_E_BUSY)?.active();
        model.finish(FileDialogCompletion::Cancelled)?;
        Ok(active)
    }
    /// Actual window destruction produces the same lifecycle cancellation as destroying its owner.
    fn close(&self) -> Result<(), NcStatus> {
        if self.dialog.request.try_borrow().map_err(|_| UI_E_BUSY)?.closed() {
            return Ok(());
        }
        self.dialog.model.try_borrow().map_err(|_| UI_E_BUSY)?.window()?.close()?;
        self.dialog.request.try_borrow_mut().map_err(|_| UI_E_BUSY)?.close()?;
        Ok(())
    }
    /// Diagnostics remain available after failure or closure to explain a rejected directory candidate.
    fn diagnostic(&self) -> Result<ComPtr<INcStringAbi>, NcStatus> {
        Ok(NcString::from_utf8_str(&self.dialog.model.try_borrow().map_err(|_| UI_E_BUSY)?.diagnostic))
    }
}
/// Creates one nodes-owned picker whose mounted root drives all private component events.
pub(crate) fn create(services: Rc<NodeServices>, parent: ComRef<'_, IWindowAbi>) -> Result<ComPtr<IFileDialogAbi>, NcStatus> {
    let dialog = Dialog::new(services, parent)?;
    Ok(FileDialog::from_value(FileDialog { dialog }).query().expect("file picker declares IFileDialog"))
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
