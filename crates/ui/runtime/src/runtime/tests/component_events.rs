//! Production runtime regressions for plugin-style private composite events.

use super::*;
use neocogi_ui_support::test_support::events::TestApp;
use neocogi_ui_core_contracts::Activation;

/// A typed fixture sender and its one application inbox, created together.
struct Emitter {
    /// Private Rust queue injects owned fixture messages into real composite dispatch.
    source: std::sync::mpsc::Sender<Activation>,
    /// Moved once into the owning controller; subsequent setup is rejected by the fixture.
    messages: Option<std::sync::mpsc::Receiver<Activation>>,
}
impl Emitter {
    /// Constructs complete delivery before any component can publish this source.
    fn create() -> Self {
        let (send, messages) = std::sync::mpsc::channel();
        let source = send;
        Self { source, messages: Some(messages) }
    }
}

/// Shared test observation deliberately owned outside either COM component.
type Trace = Rc<RefCell<Vec<&'static str>>>;

/// Exact private handler type, with an optional ordinary window edit after delivery.
struct PrivateRecord {
    /// Component name used to prove child/surface ordering at the boundary.
    name: &'static str,
    /// Optional public output makes upward delivery explicit rather than exposing child events.
    forward: Option<std::sync::mpsc::Sender<Activation>>,
    /// One ordinary tree edit, consumed before entering foreign topology code.
    edit: Edit,
}

impl PrivateRecord {
    /// Records the captured fact and may remove another mounted component.
    fn handle(&mut self, trace: &Trace) -> Result<(), NcStatus> {
        trace.borrow_mut().push(self.name);
        if let Some(source) = &self.forward {
            let _ = source.send(Activation { label: self.name.into() });
        }
        // A consumed edit cannot repeat during a later dispatch sweep.
        match std::mem::replace(&mut self.edit, Edit::None) {
            Edit::None => {}
            Edit::Close(window) => window.close()?,
            Edit::RequestClose { window, requests } => requests.send(window).map_err(|_| UI_E_CLOSED)?,
            Edit::Insert { mut parent, child } => parent.insert(0, child, LinearEdge::new(LayoutTrack::Content)).map_err(|(status, _candidate)| status)?,
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
    /// Reports intent to an external parent without mutating the tree.
    RequestClose {
        /// Window whose ownership decision belongs to the parent.
        window: ComPtr<IWindowAbi>,
        /// Parent's ordinary application message channel.
        requests: std::sync::mpsc::Sender<ComPtr<IWindowAbi>>,
    },
    /// Mounts a previously unmounted component during delivery.
    Insert {
        /// Existing parent accepts the new child through its ordinary COM editing interface.
        parent: neocogi_ui::LinearWidget,
        /// Child carries its own private inbox; the application never registers that inbox.
        child: neocogi_ui::OwnedWidget<IWidgetAbi>,
    },
}

/// Fixture consumer owns ordinary command state, independently of the runtime's tree.
struct Consumer {
    /// Shared assertion history is outside every component's COM allocation.
    trace: Trace,
    /// Shared component access requires one checked mutable boundary.
    record: PrivateRecord,
}
impl neocogi_ui::events::EventModel for Consumer {
    type Message = Activation;
    /// Handles one message; framework code owns all queue traversal and timing.
    fn handle(&mut self, _: Activation) -> Result<(), NcStatus> {
        self.record.handle(&self.trace)
    }
}

/// Builds one real eventful container, supplying defaults for ordinary forwarding.
fn component(
    ui: &neocogi_ui::Ui,
    trace: &Trace,
    name: &'static str,
    source: &mut Emitter,
    forward: Option<std::sync::mpsc::Sender<Activation>>,
    edit: Edit,
) -> (neocogi_ui::OwnedWidget<IWidgetAbi>, neocogi_ui::LinearWidget) {
    let messages = source.messages.take().expect("one consumer per fixture source");
    let (widget, shell) = super::composite_fixture::Composite::build(
        ui,
        Consumer {
            trace: trace.clone(),
            record: PrivateRecord { name, forward, edit },
        },
        messages,
    );
    (neocogi_ui::OwnedWidget::from_interface(widget.into_interface().unwrap()), shell)
}

/// Emits a captured fixture fact without entering any application callback.
fn emit(source: &Emitter) {
    let _ = source.source.send(Activation { label: "fixture".into() });
}

/// Three actual component levels forward upward before the application, with no manual child pumping.
#[test]
fn nested_components_dispatch_child_first_and_promote_only_explicit_outputs() {
    let ui = nc_ui_nodes::bind_builtin_nodes(runtime()).unwrap();
    let trace: Trace = Rc::new(RefCell::new(Vec::new()));
    let mut sources = (0..4).map(|_| Emitter::create()).collect::<Vec<_>>();
    let forward0 = sources[1].source.clone();
    let (child, _) = component(&ui, &trace, "child", &mut sources[0], Some(forward0), Edit::None);
    let forward1 = sources[2].source.clone();
    let (parent, mut parent_shell) = component(&ui, &trace, "parent", &mut sources[1], Some(forward1), Edit::None);
    let forward2 = sources[3].source.clone();
    let (root, mut root_shell) = component(&ui, &trace, "root", &mut sources[2], Some(forward2), Edit::None);
    parent_shell.insert(0, child, LinearEdge::new(LayoutTrack::Content)).unwrap();
    root_shell.insert(0, parent, LinearEdge::new(LayoutTrack::Content)).unwrap();
    let window = ui.window("nested", RECT, root).unwrap();
    let app = sources[3].messages.take().unwrap();

    emit(&sources[0]);
    ui.update(SIZE).unwrap(); // No application dispatcher is needed to drive every private level.
    assert_eq!(&*trace.borrow(), &["child", "parent", "root"]);
    let app = TestApp::new(trace.clone(), app, |trace, _| {
        trace.borrow_mut().push("application");
        Ok(())
    });
    app.update(&ui, SIZE).unwrap();
    assert_eq!(&*trace.borrow(), &["child", "parent", "root", "application"]);
    trace.borrow_mut().clear();
    emit(&sources[0]);
    app.update(&ui, SIZE).unwrap();
    assert_eq!(&*trace.borrow(), &["child", "parent", "root", "application"]);
    window.close().unwrap();
    emit(&sources[0]);
    app.update(&ui, SIZE).unwrap();
    assert_eq!(trace.borrow().len(), 4, "removed descendants never run again");
}

/// A component mounted by a handler joins the next sweep of the same safe boundary.
#[test]
fn callback_mounted_descendant_is_discovered_without_application_registration() {
    let ui = nc_ui_nodes::bind_builtin_nodes(runtime()).unwrap();
    let trace: Trace = Rc::new(RefCell::new(Vec::new()));
    let mut child_source = Emitter::create();
    let (child, _) = component(&ui, &trace, "inserted", &mut child_source, None, Edit::None);
    emit(&child_source);
    let mut parent_source = Emitter::create();
    let (parent, shell) = component(&ui, &trace, "parent", &mut parent_source, None, Edit::None);
    let window = ui.window("dynamic", RECT, parent).unwrap();
    let mut trigger = Emitter::create();
    let app = trigger.messages.take().unwrap();
    let action = PrivateRecord {
        name: "application",
        forward: None,
        edit: Edit::Insert { parent: shell, child },
    };
    emit(&trigger);
    let app = TestApp::new((action, trace.clone()), app, |(action, trace), _| action.handle(trace));
    app.update(&ui, SIZE).unwrap();
    assert_eq!(&*trace.borrow(), &["application", "inserted"]);
    window.close().unwrap();
}

/// A descendant can ask for removal; the parent decides after private delivery settles.
#[test]
fn descendant_close_intent_leaves_parent_and_siblings_live_until_parent_approves() {
    let ui = nc_ui_nodes::bind_builtin_nodes(runtime()).unwrap();
    let trace: Trace = Rc::new(RefCell::new(Vec::new()));
    let mut root_source = Emitter::create();
    let (root, mut shell) = component(&ui, &trace, "root", &mut root_source, None, Edit::None);
    let window = ui.window("removed", RECT, root).unwrap();
    let (requests, decisions) = std::sync::mpsc::channel();
    let app = TestApp::new((), decisions, |(), window: ComPtr<IWindowAbi>| window.close());
    let mut source = Emitter::create();
    let (child, _) = component(&ui, &trace, "remover", &mut source, None, Edit::RequestClose { window: window.clone(), requests });
    shell.insert(0, child, LinearEdge::new(LayoutTrack::Content)).unwrap();
    let mut sibling_source = Emitter::create();
    let (sibling, _) = component(&ui, &trace, "sibling", &mut sibling_source, None, Edit::None);
    shell.insert(1, sibling, LinearEdge::new(LayoutTrack::Content)).unwrap();
    for source in [&source, &sibling_source, &root_source] {
        emit(source);
    }
    ui.update(SIZE).unwrap();
    assert_eq!(&*trace.borrow(), &["remover", "sibling", "root"]);
    assert!(window.snapshot().is_ok(), "no parent has approved the request yet");
    app.update(&ui, SIZE).unwrap();
    assert_eq!(window.snapshot(), Err(UI_E_CLOSED));
}

/// Child-surface ordering still precedes application delivery and skips a removed later surface.
#[test]
fn mounted_component_events_dispatch_before_application_and_skip_removed_snapshots() {
    let ui = nc_ui_nodes::bind_builtin_nodes(runtime()).unwrap();
    let trace: Trace = Rc::new(RefCell::new(Vec::new()));
    let mut older_source = Emitter::create();
    let (older, _) = component(&ui, &trace, "older", &mut older_source, None, Edit::None);
    let older_window = ui.window("older", RECT, older).unwrap();
    let mut newer_source = Emitter::create();
    let (newer, _) = component(&ui, &trace, "newer", &mut newer_source, None, Edit::Close(older_window.clone()));
    let newer_window = ui.window("newer", RECT, newer).unwrap();
    let mut app_source = Emitter::create();
    let app = app_source.messages.take().unwrap();
    for source in [&older_source, &newer_source, &app_source] {
        emit(source);
    }
    let app = TestApp::new(trace.clone(), app, |trace, _| {
        trace.borrow_mut().push("application");
        Ok(())
    });
    app.update(&ui, SIZE).unwrap();
    assert_eq!(&*trace.borrow(), &["newer", "application"]);
    assert_eq!(older_window.snapshot(), Err(UI_E_CLOSED));
    newer_window.close().unwrap();
}

/// Private work posted by one application message settles before its already queued successor.
#[test]
fn application_messages_interleave_with_automatic_private_settlement() {
    let ui = nc_ui_nodes::bind_builtin_nodes(runtime()).unwrap();
    let trace: Trace = Rc::new(RefCell::new(Vec::new()));
    let mut source = Emitter::create();
    let (child, _) = component(&ui, &trace, "child", &mut source, None, Edit::None);
    let window = ui.window("interleaving", RECT, child).unwrap();
    let (events_tx, events) = std::sync::mpsc::channel();
    events_tx.send("first").unwrap();
    events_tx.send("second").unwrap();
    let app = TestApp::new((trace.clone(), source), events, |(trace, source), message| {
        trace.borrow_mut().push(message);
        if message == "first" {
            emit(source);
        }
        Ok(())
    });
    app.update(&ui, SIZE).unwrap();
    assert_eq!(&*trace.borrow(), &["first", "child", "second"]);
    window.close().unwrap();
}

/// Repeated private work stays eligible until a parent actually approves removal.
#[test]
fn close_intent_does_not_silently_discard_pending_private_messages() {
    let ui = nc_ui_nodes::bind_builtin_nodes(runtime()).unwrap();
    let trace: Trace = Rc::new(RefCell::new(Vec::new()));
    let parent_owner = ui.widgets().linear(FlowDirection::TopToBottom, CrossSize::Stretch).unwrap();
    let mut parent = parent_owner.handle();
    let window = ui.window("self removal", RECT, parent_owner).unwrap();
    let (requests, decisions) = std::sync::mpsc::channel();
    let app = TestApp::new((), decisions, |(), window: ComPtr<IWindowAbi>| window.close());
    let mut source = Emitter::create();
    let (child, _) = component(&ui, &trace, "remover", &mut source, None, Edit::RequestClose { window: window.clone(), requests });
    parent.insert(0, child, LinearEdge::new(LayoutTrack::Content)).unwrap();
    emit(&source);
    emit(&source);
    ui.update(SIZE).unwrap();
    assert_eq!(&*trace.borrow(), &["remover", "remover"]);
    assert!(window.snapshot().is_ok());
    app.update(&ui, SIZE).unwrap();
    assert_eq!(window.snapshot(), Err(UI_E_CLOSED));
    assert!(source.source.send(Activation { label: "late".into() }).is_err());
}
