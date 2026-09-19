//! Production runtime regressions for plugin-style private composite events.

use super::*;
use neocogi_com_events::Events;
use neocogi_ui_core_contracts::{
    Activation, IActivationEvents, IActivationEventsAbi, IActivationEventsInterfacesFor, IActivationEventsOutputs, __ActivationReceiverAbi,
};

#[com_object(interfaces(neocogi_ui_core_contracts::__ActivationReceiver, IActivationEvents))]
/// Minimal producer; there is no event-specific shared slot or lifetime state.
struct Emitter {
    /// The one installed receiver owns mapping and weak consumer access.
    destination: Option<ComPtr<__ActivationReceiverAbi>>,
}
impl Emitter {
    /// Allocates a fixture producer with no receiver and no event queue.
    fn create() -> ComPtr<__ActivationReceiverAbi> {
        Self::from_value(Self { destination: None }).query().unwrap()
    }
}
impl neocogi_ui_core_contracts::__ActivationReceiver for Emitter {
    /// The generated concrete receiver also supplies this fixture's posting entry point.
    fn post(&mut self, message: neocogi_com::NcMessage<'_, Activation>) -> Result<(), NcStatus> {
        message.to_owned()?.post_to(&mut self.destination)
    }
}
impl IActivationEvents for Emitter {
    /// Replaces future delivery without affecting values already in any queue.
    fn __map_activated(&mut self, destination: ComRef<'_, __ActivationReceiverAbi>) -> Result<(), NcStatus> {
        self.destination = Some(destination.add_ref());
        Ok(())
    }
}

/// Shared test observation deliberately owned outside either COM component.
type Trace = Rc<RefCell<Vec<&'static str>>>;

/// Exact private handler type, with an optional ordinary window edit after delivery.
struct PrivateRecord {
    /// Component name used to prove child/surface ordering at the boundary.
    name: &'static str,
    /// Optional public output makes upward delivery explicit rather than exposing child events.
    forward: Option<ComPtr<__ActivationReceiverAbi>>,
    /// One ordinary tree edit, consumed before entering foreign topology code.
    edit: Edit,
}

impl PrivateRecord {
    /// Records the captured fact and may remove another mounted component.
    fn handle(&mut self, trace: &Trace) -> Result<(), NcStatus> {
        trace.borrow_mut().push(self.name);
        if let Some(source) = &self.forward {
            source.clone().post(neocogi_com::NcMessage::new(&Activation { label: self.name.into() }))?;
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
        /// Child carries its own private inbox; the application never registers that inbox.
        child: neocogi_ui::Widget<IWidgetAbi>,
    },
}

/// Fixture consumer owns ordinary command state, independently of the runtime's tree.
struct Consumer {
    /// Shared assertion history is outside every component's COM allocation.
    trace: Trace,
    /// Shared component access requires one checked mutable boundary.
    record: PrivateRecord,
}
impl Consumer {
    /// Handles one message; framework code owns all queue traversal and timing.
    fn update(&mut self, _: Activation) -> Result<(), NcStatus> {
        self.record.handle(&self.trace)
    }
}

#[com_object(interfaces(IWidget, IContainerLayout, IComponentEventProvider))]
/// Plugin-style composite allocation with its own event source and private inbox.
/// Its dispatcher is the inbox's separate COM allocation, never this widget.
struct EventfulComposite {
    /// Real host-bound widget lifetime delegated without storing any runtime root.
    lifetime: ComPtr<IWidgetAbi>,
    /// Actual container policy and authoritative child collection permit arbitrary nesting.
    layout: ComPtr<IContainerLayoutAbi>,
    /// Component-owned typed handler/inbox, entirely inside this binary.
    private_events: Events<Activation>,
    /// Component ownership outlives its weak framework driver.
    _consumer: Rc<RefCell<Consumer>>,
    /// Separate allocation supplied by the framework, never a handwritten dispatcher.
    dispatcher: ComPtr<IEventDispatcherAbi>,
}

impl EventfulComposite {
    /// Builds an ordinary container with one private inbox and returns its editing capability.
    fn create(
        ui: &neocogi_ui::Ui,
        trace: Trace,
        source: ComPtr<__ActivationReceiverAbi>,
        handler: PrivateRecord,
    ) -> (neocogi_ui::Widget<IWidgetAbi>, neocogi_ui::LinearWidget) {
        let shell = ui.widgets().linear(FlowDirection::TopToBottom, CrossSize::Stretch).unwrap();
        let mut private_events = Events::new();
        private_events
            .map(source.query::<IActivationEventsAbi>().unwrap().activated(), |event| event)
            .unwrap();
        let consumer = Rc::new(RefCell::new(Consumer { trace, record: handler }));
        let dispatcher = private_events.component_dispatcher(&consumer, Consumer::update).unwrap();
        let widget = Self::from_value(Self {
            lifetime: shell.node_interface().unwrap(),
            layout: shell.as_interface().add_ref().query().unwrap(),
            private_events,
            _consumer: consumer,
            dispatcher,
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
    /// Yields the separate, typed inbox dispatcher as an owned COM reference.
    fn private_dispatcher(&self) -> Result<ComPtr<IEventDispatcherAbi>, NcStatus> {
        Ok(self.dispatcher.clone())
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
    /// Ordinary widget closure retires its private queue and destination.
    fn close(&mut self) -> Result<(), NcStatus> {
        self.private_events = Events::new();
        self.lifetime.close()
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
    source: &ComPtr<__ActivationReceiverAbi>,
    forward: Option<ComPtr<__ActivationReceiverAbi>>,
    edit: Edit,
) -> (neocogi_ui::Widget<IWidgetAbi>, neocogi_ui::LinearWidget) {
    EventfulComposite::create(ui, trace.clone(), source.clone(), PrivateRecord { name, forward, edit })
}

/// Emits a captured fixture fact without entering any application callback.
fn emit(source: &ComPtr<__ActivationReceiverAbi>) {
    source
        .clone()
        .post(neocogi_com::NcMessage::new(&Activation { label: "fixture".into() }))
        .unwrap();
}

/// Three actual component levels forward upward before the application, with no manual child pumping.
#[test]
fn nested_components_dispatch_child_first_and_promote_only_explicit_outputs() {
    let ui = neocogi_ui_nodes::bind_builtin_nodes(runtime()).unwrap();
    let trace: Trace = Rc::new(RefCell::new(Vec::new()));
    let sources = (0..4).map(|_| Emitter::create()).collect::<Vec<_>>();
    let (child, _) = component(&ui, &trace, "child", &sources[0], Some(sources[1].clone()), Edit::None);
    let (parent, mut parent_shell) = component(&ui, &trace, "parent", &sources[1], Some(sources[2].clone()), Edit::None);
    let (root, mut root_shell) = component(&ui, &trace, "root", &sources[2], Some(sources[3].clone()), Edit::None);
    parent_shell.insert(0, &child, LinearEdge::new(LayoutTrack::Content)).unwrap().finish().unwrap();
    root_shell.insert(0, &parent, LinearEdge::new(LayoutTrack::Content)).unwrap().finish().unwrap();
    let window = ui.window("nested", RECT, &root).unwrap();
    let mut app = Events::new();
    app.map(sources[3].query::<IActivationEventsAbi>().unwrap().activated(), |_| ()).unwrap();

    emit(&sources[0]);
    ui.update(SIZE).unwrap(); // No application dispatcher is needed to drive every private level.
    assert_eq!(&*trace.borrow(), &["child", "parent", "root"]);
    ui.update_with_events(SIZE, &mut app, |_| {
        trace.borrow_mut().push("application");
        Ok(())
    })
    .unwrap();
    assert_eq!(&*trace.borrow(), &["child", "parent", "root", "application"]);
    trace.borrow_mut().clear();
    emit(&sources[0]);
    ui.update_with_events(SIZE, &mut app, |_| {
        trace.borrow_mut().push("application");
        Ok(())
    })
    .unwrap();
    assert_eq!(&*trace.borrow(), &["child", "parent", "root", "application"]);
    window.close().unwrap();
    emit(&sources[0]);
    ui.update_with_events(SIZE, &mut app, |_| {
        trace.borrow_mut().push("application");
        Ok(())
    })
    .unwrap();
    assert_eq!(trace.borrow().len(), 4, "removed descendants never run again");
}

/// A component mounted by a handler joins the next sweep of the same safe boundary.
#[test]
fn callback_mounted_descendant_is_discovered_without_application_registration() {
    let ui = neocogi_ui_nodes::bind_builtin_nodes(runtime()).unwrap();
    let trace: Trace = Rc::new(RefCell::new(Vec::new()));
    let child_source = Emitter::create();
    let (child, _) = component(&ui, &trace, "inserted", &child_source, None, Edit::None);
    emit(&child_source);
    let parent_source = Emitter::create();
    let (parent, shell) = component(&ui, &trace, "parent", &parent_source, None, Edit::None);
    let window = ui.window("dynamic", RECT, &parent).unwrap();
    let trigger = Emitter::create();
    let mut app = Events::new();
    app.map(trigger.query::<IActivationEventsAbi>().unwrap().activated(), |_| ()).unwrap();
    let mut action = PrivateRecord {
        name: "application",
        forward: None,
        edit: Edit::Insert { parent: shell, child },
    };
    emit(&trigger);
    ui.update_with_events(SIZE, &mut app, |_| action.handle(&trace)).unwrap();
    assert_eq!(&*trace.borrow(), &["application", "inserted"]);
    window.close().unwrap();
}

/// Removing an ancestor inside its descendant handler retires the remaining parent ticket safely.
#[test]
fn descendant_removal_retires_ancestor_and_sibling_dispatch_snapshots() {
    let ui = neocogi_ui_nodes::bind_builtin_nodes(runtime()).unwrap();
    let trace: Trace = Rc::new(RefCell::new(Vec::new()));
    let root_source = Emitter::create();
    let (root, mut shell) = component(&ui, &trace, "root", &root_source, None, Edit::None);
    let window = ui.window("removed", RECT, &root).unwrap();
    let source = Emitter::create();
    let (child, _) = component(&ui, &trace, "remover", &source, None, Edit::Close(window.clone()));
    shell.insert(0, &child, LinearEdge::new(LayoutTrack::Content)).unwrap().finish().unwrap();
    let sibling_source = Emitter::create();
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
    let older_source = Emitter::create();
    let (older, _) = component(&ui, &trace, "older", &older_source, None, Edit::None);
    let older_window = ui.window("older", RECT, &older).unwrap();
    let newer_source = Emitter::create();
    let (newer, _) = component(&ui, &trace, "newer", &newer_source, None, Edit::Close(older_window.clone()));
    let newer_window = ui.window("newer", RECT, &newer).unwrap();
    let app_source = Emitter::create();
    let mut app = Events::new();
    app.map(app_source.query::<IActivationEventsAbi>().unwrap().activated(), |_| ()).unwrap();
    for source in [&older_source, &newer_source, &app_source] {
        emit(source);
    }
    ui.update_with_events(SIZE, &mut app, |_| {
        trace.borrow_mut().push("application");
        Ok(())
    })
    .unwrap();
    assert_eq!(&*trace.borrow(), &["newer", "application"]);
    assert_eq!(older_window.snapshot(), Err(UI_E_CLOSED));
    newer_window.close().unwrap();
}

/// Private work posted by one application message settles before its already queued successor.
#[test]
fn application_messages_interleave_with_automatic_private_settlement() {
    let ui = neocogi_ui_nodes::bind_builtin_nodes(runtime()).unwrap();
    let trace: Trace = Rc::new(RefCell::new(Vec::new()));
    let source = Emitter::create();
    let (child, _) = component(&ui, &trace, "child", &source, None, Edit::None);
    let window = ui.window("interleaving", RECT, &child).unwrap();
    let mut events = Events::new();
    events.sender().post("first").unwrap();
    events.sender().post("second").unwrap();
    ui.update_with_events(SIZE, &mut events, |message| {
        trace.borrow_mut().push(message);
        if message == "first" {
            emit(&source);
        }
        Ok(())
    })
    .unwrap();
    assert_eq!(&*trace.borrow(), &["first", "child", "second"]);
    window.close().unwrap();
}

/// A component that removes its own mount cannot receive another message from the same batch.
#[test]
fn self_removal_stops_private_delivery_before_the_next_message() {
    let ui = neocogi_ui_nodes::bind_builtin_nodes(runtime()).unwrap();
    let trace: Trace = Rc::new(RefCell::new(Vec::new()));
    let parent = ui.widgets().linear(FlowDirection::TopToBottom, CrossSize::Stretch).unwrap();
    let window = ui.window("self removal", RECT, &parent).unwrap();
    let source = Emitter::create();
    let (child, _) = component(&ui, &trace, "remover", &source, None, Edit::Close(window.clone()));
    parent
        .as_interface()
        .insert(0, child.node_interface().unwrap().as_com_ref(), LinearEdge::new(LayoutTrack::Content))
        .unwrap()
        .finish()
        .unwrap();
    emit(&source);
    emit(&source);
    ui.update(SIZE).unwrap();
    assert_eq!(&*trace.borrow(), &["remover"]);
}
