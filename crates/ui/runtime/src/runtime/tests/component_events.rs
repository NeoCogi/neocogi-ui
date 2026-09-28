//! Production runtime regressions for plugin-style private composite events.

use super::*;
use neocogi_ui_support::test_support::events::TestApp;
use neocogi_ui_core_contracts::{Activation, IMenuItemSinkAbi};

/// A typed fixture sender and its one application inbox, created together.
struct Emitter {
    /// Real COM receiver performs owned decoding before posting into the inbox.
    source: ComPtr<IMenuItemSinkAbi>,
    /// Moved once into the owning controller; subsequent setup is rejected by the fixture.
    messages: Option<std::sync::mpsc::Receiver<Activation>>,
}
impl Emitter {
    /// Constructs complete delivery before any component can publish this source.
    fn create() -> Self {
        let (send, messages) = std::sync::mpsc::channel();
        let source = neocogi_ui::events::activation::sink(send, |event| event).unwrap();
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
    forward: Option<ComPtr<IMenuItemSinkAbi>>,
    /// One ordinary tree edit, consumed before entering foreign topology code.
    edit: Edit,
}

impl PrivateRecord {
    /// Records the captured fact and may remove another mounted component.
    fn handle(&mut self, trace: &Trace) -> Result<(), NcStatus> {
        trace.borrow_mut().push(self.name);
        if let Some(source) = &self.forward {
            source
                .clone()
                .post(neocogi_com::NcValueRef::new(&Activation { label: self.name.into() }).unwrap())?;
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
impl neocogi_ui::events::EventModel for Consumer {
    type Message = Activation;
    /// Handles one message; framework code owns all queue traversal and timing.
    fn handle(&mut self, _: Activation) -> Result<(), NcStatus> {
        self.record.handle(&self.trace)
    }
}

impl super::composite_fixture::Behavior for Consumer {}

/// Builds one real eventful container, supplying defaults for ordinary forwarding.
fn component(
    ui: &neocogi_ui::Ui,
    trace: &Trace,
    name: &'static str,
    source: &mut Emitter,
    forward: Option<ComPtr<IMenuItemSinkAbi>>,
    edit: Edit,
) -> (neocogi_ui::Widget<IWidgetAbi>, neocogi_ui::LinearWidget) {
    let messages = source.messages.take().expect("one consumer per fixture source");
    let (widget, shell) = super::composite_fixture::Composite::build(
        ui, Consumer { trace: trace.clone(), record: PrivateRecord { name, forward, edit } }, messages,
    );
    (neocogi_ui::Widget::from_interface(widget.query().unwrap()), shell)
}

/// Emits a captured fixture fact without entering any application callback.
fn emit(source: &Emitter) {
    source
        .source
        .clone()
        .post(neocogi_com::NcValueRef::new(&Activation { label: "fixture".into() }).unwrap())
        .unwrap();
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
    parent_shell.insert(0, &child, LinearEdge::new(LayoutTrack::Content)).unwrap().finish().unwrap();
    root_shell.insert(0, &parent, LinearEdge::new(LayoutTrack::Content)).unwrap().finish().unwrap();
    let window = ui.window("nested", RECT, &root).unwrap();
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
    let window = ui.window("dynamic", RECT, &parent).unwrap();
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

/// Removing an ancestor inside its descendant handler retires the remaining parent ticket safely.
#[test]
fn descendant_removal_retires_ancestor_and_sibling_dispatch_snapshots() {
    let ui = nc_ui_nodes::bind_builtin_nodes(runtime()).unwrap();
    let trace: Trace = Rc::new(RefCell::new(Vec::new()));
    let mut root_source = Emitter::create();
    let (root, mut shell) = component(&ui, &trace, "root", &mut root_source, None, Edit::None);
    let window = ui.window("removed", RECT, &root).unwrap();
    let mut source = Emitter::create();
    let (child, _) = component(&ui, &trace, "remover", &mut source, None, Edit::Close(window.clone()));
    shell.insert(0, &child, LinearEdge::new(LayoutTrack::Content)).unwrap().finish().unwrap();
    let mut sibling_source = Emitter::create();
    let (sibling, _) = component(&ui, &trace, "sibling", &mut sibling_source, None, Edit::None);
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
    let ui = nc_ui_nodes::bind_builtin_nodes(runtime()).unwrap();
    let trace: Trace = Rc::new(RefCell::new(Vec::new()));
    let mut older_source = Emitter::create();
    let (older, _) = component(&ui, &trace, "older", &mut older_source, None, Edit::None);
    let older_window = ui.window("older", RECT, &older).unwrap();
    let mut newer_source = Emitter::create();
    let (newer, _) = component(&ui, &trace, "newer", &mut newer_source, None, Edit::Close(older_window.clone()));
    let newer_window = ui.window("newer", RECT, &newer).unwrap();
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
    let window = ui.window("interleaving", RECT, &child).unwrap();
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

/// A component that removes its own mount cannot receive another message from the same batch.
#[test]
fn self_removal_stops_private_delivery_before_the_next_message() {
    let ui = nc_ui_nodes::bind_builtin_nodes(runtime()).unwrap();
    let trace: Trace = Rc::new(RefCell::new(Vec::new()));
    let mut parent = ui.widgets().linear(FlowDirection::TopToBottom, CrossSize::Stretch).unwrap();
    let window = ui.window("self removal", RECT, &parent).unwrap();
    let mut source = Emitter::create();
    let (child, _) = component(&ui, &trace, "remover", &mut source, None, Edit::Close(window.clone()));
    parent.insert(0, &child, LinearEdge::new(LayoutTrack::Content)).unwrap().finish().unwrap();
    emit(&source);
    emit(&source);
    ui.update(SIZE).unwrap();
    assert_eq!(&*trace.borrow(), &["remover"]);
}
