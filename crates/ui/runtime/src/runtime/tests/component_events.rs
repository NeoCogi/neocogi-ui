//! Production runtime regressions for plugin-style private composite events.

use super::*;
use neocogi_com_events::{EventPort, EventHandler, EventSession as ComEventSession, EventSource};
use neocogi_ui_core_contracts::{ActivationMessage, create_activation_port_sink};

/// Shared test observation deliberately owned outside either COM component.
type Trace = Rc<RefCell<Vec<&'static str>>>;

/// Local fixture connector for a native producer without a declared COM source port.
struct LocalActivation<'source> {
    /// Producer borrowed for sink construction and subscription only.
    source: &'source EventSource<ActivationMessage>,
}

impl EventPort for LocalActivation<'_> {
    type Event = ActivationMessage;
    /// Uses the normal generated sink and production event queue inside registration.
    fn connect<State: 'static, Handler: EventHandler<State, Self::Event> + 'static>(
        self,
        session: &ComEventSession<State>,
        handler: Handler,
    ) -> Result<ComPtr<IEventSubscriptionAbi>, NcStatus> {
        let sink = create_activation_port_sink(session, handler, None)?;
        self.source.subscribe(sink.as_com_ref())
    }
}

/// Exact private handler type, with an optional ordinary window edit after delivery.
struct PrivateRecord {
    /// Component name used to prove child/surface ordering at the boundary.
    name: &'static str,
    /// Optional public output makes upward delivery explicit rather than exposing child events.
    forward: Option<Rc<EventSource<ActivationMessage>>>,
    /// One ordinary tree edit, consumed before entering foreign topology code.
    edit: Edit,
}

impl EventHandler<Trace, ActivationMessage> for PrivateRecord {
    /// Records the captured fact and may remove another mounted component.
    fn handle(&mut self, trace: &mut Trace, _event: &ActivationMessage) -> Result<(), NcStatus> {
        trace.borrow_mut().push(self.name);
        if let Some(source) = &self.forward {
            source.emit(ActivationMessage { label: self.name.to_owned() });
        }
        // A consumed edit cannot repeat during a later dispatch sweep.
        match std::mem::replace(&mut self.edit, Edit::None) {
            Edit::None => {}
            Edit::Close(window) => window.close()?,
            Edit::Insert { mut parent, child } => parent.insert(0, &child, LinearEdge::new(LayoutTrack::Content))?.finish()?,
        }
        Ok(())
    }
}

/// Deliberate topology mutations exercise the real runtime after traversal releases ownership.
enum Edit {
    /// The handler only records and optionally emits its public output.
    None,
    /// Removes a window and its whole subtree before later snapshot tickets run.
    Close(ComPtr<IWindowAbi>),
    /// Mounts a previously unmounted component during delivery.
    Insert {
        /// Existing parent accepts the new child through its ordinary COM editing interface.
        parent: neocogi_ui::LinearWidget,
        /// Child carries its own private session; the application never registers that session.
        child: neocogi_ui::Widget<IWidgetAbi>,
    },
}

/// Application-owned handler running only after all eligible private drivers.
struct AppRecord;

impl EventHandler<Trace, ActivationMessage> for AppRecord {
    /// Marks the public session's position in the deterministic sweep.
    fn handle(&mut self, trace: &mut Trace, _event: &ActivationMessage) -> Result<(), NcStatus> {
        trace.borrow_mut().push("application");
        Ok(())
    }
}

#[com_object(interfaces(IWidget, IContainerLayout, IComponentEventProvider))]
/// Plugin-style composite allocation with its own event source and private session.
/// Its dispatcher is the session's separate COM allocation, never this widget.
struct EventfulComposite {
    /// Real host-bound widget lifetime delegated without storing any runtime root.
    lifetime: ComPtr<IWidgetAbi>,
    /// Producer-owned captured events; the source never stores the private handler.
    source: Rc<EventSource<ActivationMessage>>,
    /// Actual container policy and authoritative child collection permit arbitrary nesting.
    layout: ComPtr<IContainerLayoutAbi>,
    /// Component-owned typed handler/session, entirely inside this binary.
    private_session: ComEventSession<Trace>,
}

impl EventfulComposite {
    /// Builds an ordinary container with one private session and returns its editing capability.
    fn create(
        ui: &neocogi_ui::Ui,
        trace: Trace,
        source: Rc<EventSource<ActivationMessage>>,
        handler: PrivateRecord,
    ) -> (neocogi_ui::Widget<IWidgetAbi>, neocogi_ui::LinearWidget) {
        let shell = ui.widgets().linear(FlowDirection::TopToBottom, CrossSize::Stretch).unwrap();
        let private_session = ComEventSession::new(trace);
        private_session.subscribe(LocalActivation { source: &source }, handler).unwrap();
        let widget = Self::from_value(Self {
            lifetime: shell.node_interface().unwrap(),
            layout: shell.as_interface().add_ref().query().unwrap(),
            source,
            private_session,
        })
        .query()
        .unwrap();
        (neocogi_ui::Widget::from_interface(widget), shell)
    }
}

impl IContainerLayout for EventfulComposite {
    /// Exposes the real host-issued collection, never a copied child list.
    fn children(&self) -> Result<ComPtr<IChildrenAbi>, NcStatus> {
        self.layout.children()
    }
    /// Delegates measurement to the same concrete shell that owns the children.
    fn measure(&mut self, scope: LayoutScope, bounds: LayoutLimits, layout: ComRef<'_, ILayoutServiceAbi>) -> Result<Size, NcStatus> {
        self.layout.measure(scope, bounds, layout)
    }
    /// Preserves ordinary descendant placement and viewport handling.
    fn place(&mut self, scope: LayoutScope, rect: Rect, layout: ComRef<'_, ILayoutServiceAbi>) -> Result<ContainerGeometry, NcStatus> {
        self.layout.place(scope, rect, layout)
    }
    /// Uses the shell's input policy independently of semantic event delivery.
    fn accepts_event(&self, event: WidgetEventAbi, text: NcCopyFromBufferOffset<'_, u8>) -> Result<bool, NcStatus> {
        self.layout.accepts_event(event, text)
    }
}

impl IComponentEventProvider for EventfulComposite {
    /// Yields the separate, typed session dispatcher as an owned COM reference.
    fn private_dispatcher(&self) -> Result<ComPtr<IEventDispatcherAbi>, NcStatus> {
        Ok(self.private_session.as_dispatcher().add_ref())
    }
}

impl IWidget for EventfulComposite {
    /// Delegates host authentication to a factory-created widget lifetime.
    fn host(&self) -> Result<ComPtr<IWidgetHostAbi>, NcStatus> {
        self.lifetime.host()
    }
    /// Claims the underlying one-way mount only after the runtime validates the driver.
    fn mount(&mut self) -> Result<(), NcStatus> {
        self.lifetime.mount()
    }
    /// Cancels private leases before closing its independently retained lifetime.
    fn close(&mut self) -> Result<(), NcStatus> {
        self.source.close_ordinary();
        match self.private_session.cancel() {
            // Self-removal during dispatch retires the queue immediately; the active
            // transaction prunes the now-inactive lease when its callback returns.
            Ok(()) | Err(neocogi_com_events::EVENT_E_BUSY) => self.lifetime.close(),
            Err(error) => Err(error),
        }
    }
    /// Uses the same real common routing policy as its factory-created lifetime.
    fn properties(&self) -> Result<WidgetProperties, NcStatus> {
        self.lifetime.properties()
    }
    /// Delegates ordinary option mutation without shadow state.
    fn set_options(&mut self, options: WidgetOptions) -> Result<(), NcStatus> {
        self.lifetime.set_options(options)
    }
    /// All test events were committed before mount; eventless updates are inert.
    fn update(&mut self, _state: WidgetUpdate, _event: WidgetEventAbi, _text: NcCopyFromBufferOffset<'_, u8>) -> Result<GeometryChange, NcStatus> {
        Ok(GeometryChange::NoChange)
    }
    /// No programmatic geometry edits exist on this fixture.
    fn take_change(&mut self) -> Result<GeometryChange, NcStatus> {
        Ok(GeometryChange::NoChange)
    }
    /// Paints no fixture geometry; runtime scheduling is the test subject.
    fn paint(&mut self, _scope: PaintScope, _state: WidgetPaint, _painter: ComRef<'_, IPainterAbi>) -> Result<(), NcStatus> {
        Ok(())
    }
}

/// Builds one real eventful container, supplying defaults for ordinary forwarding.
fn component(
    ui: &neocogi_ui::Ui,
    trace: &Trace,
    name: &'static str,
    source: &Rc<EventSource<ActivationMessage>>,
    forward: Option<Rc<EventSource<ActivationMessage>>>,
    edit: Edit,
) -> (neocogi_ui::Widget<IWidgetAbi>, neocogi_ui::LinearWidget) {
    EventfulComposite::create(ui, trace.clone(), source.clone(), PrivateRecord { name, forward, edit })
}

/// Emits a captured fixture fact without entering any application callback.
fn emit(source: &EventSource<ActivationMessage>) {
    source.emit(ActivationMessage { label: String::new() });
}

/// Three actual component levels forward upward before the application, with no manual child pumping.
#[test]
fn nested_components_dispatch_child_first_and_promote_only_explicit_outputs() {
    let ui = neocogi_ui_nodes::bind_builtin_nodes(runtime()).unwrap();
    let trace: Trace = Rc::new(RefCell::new(Vec::new()));
    let sources = (0..4).map(|_| Rc::new(EventSource::new())).collect::<Vec<_>>();
    let (child, _) = component(&ui, &trace, "child", &sources[0], Some(sources[1].clone()), Edit::None);
    let (parent, mut parent_shell) = component(&ui, &trace, "parent", &sources[1], Some(sources[2].clone()), Edit::None);
    let (root, mut root_shell) = component(&ui, &trace, "root", &sources[2], Some(sources[3].clone()), Edit::None);
    parent_shell.insert(0, &child, LinearEdge::new(LayoutTrack::Content)).unwrap().finish().unwrap();
    root_shell.insert(0, &parent, LinearEdge::new(LayoutTrack::Content)).unwrap().finish().unwrap();
    let window = ui.window("nested", RECT, &root).unwrap();
    let app = ComEventSession::new(trace.clone());
    app.subscribe(LocalActivation { source: &sources[3] }, AppRecord).unwrap();

    emit(&sources[0]);
    ui.update(SIZE).unwrap(); // No application dispatcher is needed to drive every private level.
    assert_eq!(&*trace.borrow(), &["child", "parent", "root"]);
    ui.update_with_events(SIZE, &app).unwrap();
    assert_eq!(&*trace.borrow(), &["child", "parent", "root", "application"]);
    trace.borrow_mut().clear();
    emit(&sources[0]);
    ui.update_with_events(SIZE, &app).unwrap();
    assert_eq!(&*trace.borrow(), &["child", "parent", "root", "application"]);
    window.close().unwrap();
    emit(&sources[0]);
    ui.update_with_events(SIZE, &app).unwrap();
    assert_eq!(trace.borrow().len(), 4, "removed descendants never run again");
}

/// A component mounted by a handler joins the next sweep of the same safe boundary.
#[test]
fn callback_mounted_descendant_is_discovered_without_application_registration() {
    let ui = neocogi_ui_nodes::bind_builtin_nodes(runtime()).unwrap();
    let trace: Trace = Rc::new(RefCell::new(Vec::new()));
    let child_source = Rc::new(EventSource::new());
    let (child, _) = component(&ui, &trace, "inserted", &child_source, None, Edit::None);
    emit(&child_source);
    let parent_source = Rc::new(EventSource::new());
    let (parent, shell) = component(&ui, &trace, "parent", &parent_source, None, Edit::None);
    let window = ui.window("dynamic", RECT, &parent).unwrap();
    let trigger = EventSource::new();
    let app = ComEventSession::new(trace.clone());
    app.subscribe(
        LocalActivation { source: &trigger },
        PrivateRecord {
            name: "application",
            forward: None,
            edit: Edit::Insert { parent: shell, child },
        },
    )
    .unwrap();
    emit(&trigger);
    ui.update_with_events(SIZE, &app).unwrap();
    assert_eq!(&*trace.borrow(), &["application", "inserted"]);
    window.close().unwrap();
}

/// Removing an ancestor inside its descendant handler retires the remaining parent ticket safely.
#[test]
fn descendant_removal_retires_ancestor_and_sibling_dispatch_snapshots() {
    let ui = neocogi_ui_nodes::bind_builtin_nodes(runtime()).unwrap();
    let trace: Trace = Rc::new(RefCell::new(Vec::new()));
    let root_source = Rc::new(EventSource::new());
    let (root, mut shell) = component(&ui, &trace, "root", &root_source, None, Edit::None);
    let window = ui.window("removed", RECT, &root).unwrap();
    let source = Rc::new(EventSource::new());
    let (child, _) = component(&ui, &trace, "remover", &source, None, Edit::Close(window.clone()));
    shell.insert(0, &child, LinearEdge::new(LayoutTrack::Content)).unwrap().finish().unwrap();
    let sibling_source = Rc::new(EventSource::new());
    let (sibling, _) = component(&ui, &trace, "sibling", &sibling_source, None, Edit::None);
    shell.insert(1, &sibling, LinearEdge::new(LayoutTrack::Content)).unwrap().finish().unwrap();
    for source in [&source, &sibling_source, &root_source] {
        emit(source);
    }
    ui.update(SIZE).unwrap();
    assert_eq!(&*trace.borrow(), &["remover"]);
    assert_eq!(window.snapshot(), Err(UI_E_CLOSED));
}

/// Child-surface ordering still precedes application delivery and skips a removed later surface.
#[test]
fn mounted_component_events_dispatch_before_application_and_skip_removed_snapshots() {
    let ui = neocogi_ui_nodes::bind_builtin_nodes(runtime()).unwrap();
    let trace: Trace = Rc::new(RefCell::new(Vec::new()));
    let older_source = Rc::new(EventSource::new());
    let (older, _) = component(&ui, &trace, "older", &older_source, None, Edit::None);
    let older_window = ui.window("older", RECT, &older).unwrap();
    let newer_source = Rc::new(EventSource::new());
    let (newer, _) = component(&ui, &trace, "newer", &newer_source, None, Edit::Close(older_window.clone()));
    let newer_window = ui.window("newer", RECT, &newer).unwrap();
    let app_source = EventSource::new();
    let app = ComEventSession::new(trace.clone());
    app.subscribe(LocalActivation { source: &app_source }, AppRecord).unwrap();
    for source in [older_source.as_ref(), newer_source.as_ref(), &app_source] {
        emit(source);
    }
    ui.update_with_events(SIZE, &app).unwrap();
    assert_eq!(&*trace.borrow(), &["newer", "application"]);
    assert_eq!(older_window.snapshot(), Err(UI_E_CLOSED));
    newer_window.close().unwrap();
}
