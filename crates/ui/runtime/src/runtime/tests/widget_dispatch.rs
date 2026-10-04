//! Widget-owned behavior, lifetime and attachment regressions through the real runtime.

use super::*;
use neocogi_ui::events::MessageProcessor;
use super::composite_fixture::Composite;

/// Child behavior reports close intent to its parent without deciding ownership.
struct Removal {
    /// The parent owns the matching receiver; the child has no removal capability.
    requests: Option<std::sync::mpsc::Sender<()>>,
    /// External test observation contains no reference back to the widget.
    trace: Rc<RefCell<Vec<&'static str>>>,
    /// Verifies that neither success nor a recoverable error approves removal.
    fail: bool,
}

impl MessageProcessor for Removal {
    type Message = ();
    /// Records entry and return around posting intent; the parent acts afterward.
    fn handle(&mut self, _: ()) -> Result<(), NcStatus> {
        self.trace.borrow_mut().push("handler entered");
        if let Some(requests) = &self.requests {
            requests.send(()).map_err(|_| UI_E_CLOSED)?;
        }
        self.trace.borrow_mut().push("handler returned");
        match self.fail {
            true => Err(NC_E_INVALID_ARG),
            false => Ok(()),
        }
    }
}
impl Drop for Removal {
    /// Terminal behavior must run once, after the active handler releases its gate.
    fn drop(&mut self) {
        self.trace.borrow_mut().push("closed");
    }
}

/// Close intent alone changes no ownership, including when the child handler
/// fails. The parent can decline, then approve a later request; removal drops
/// pending private messages before its own call returns.
#[test]
fn parent_decides_removal_after_child_returns_even_when_the_child_fails() {
    for fail in [false, true] {
        let ui = nc_ui_nodes::bind_builtin_nodes(runtime()).unwrap();
        let trace = Rc::new(RefCell::new(Vec::new()));
        let (requests, decisions) = std::sync::mpsc::channel();
        let (send, messages) = std::sync::mpsc::channel();
        let (local, _) = Composite::build(&ui, Removal { requests: Some(requests.clone()), trace: trace.clone(), fail }, messages);
        let mut events = local.query::<IWidgetDispatcherAbi>().unwrap();
        let window = ui.window("owned behavior", RECT, local.into_interface::<IWidgetAbi>().unwrap()).unwrap();
        let app = neocogi_ui_support::test_support::events::TestApp::new(
            (window.clone(), false), decisions, |(window, approve), ()| {
                match *approve { true => window.close(), false => Ok(()) }
            },
        );
        send.send(()).unwrap();
        assert_eq!(ui.update(SIZE), if fail { Err(NC_E_INVALID_ARG) } else { Ok(()) });
        assert_eq!(&*trace.borrow(), &["handler entered", "handler returned"]);
        assert!(window.snapshot().is_ok(), "a request or handler error cannot approve removal");
        assert_eq!(app.as_interface().dispatch_next(), Ok(ApplicationDispatch::MessageHandled)); // Parent declines.
        assert!(window.snapshot().is_ok());
        assert!(events.is_alive());
        app.state().1 = true;
        requests.send(()).unwrap();
        send.send(()).unwrap(); // Must be discarded, not dispatched, after approval.
        assert_eq!(app.as_interface().dispatch_next(), Ok(ApplicationDispatch::MessageHandled));
        assert_eq!(&*trace.borrow(), &["handler entered", "handler returned", "closed"]);
        assert_eq!(window.snapshot(), Err(UI_E_CLOSED));
        assert!(send.send(()).is_err());
        assert_eq!(events.dispatch_next(), Err(neocogi_com::NC_E_DROPPED));
        assert_eq!(events.dispatch_next(), Err(neocogi_com::NC_E_DROPPED));
        assert_eq!(trace.borrow().len(), 3);
    }
}

/// The actual eventful widget follows the same single-attachment rule as an ordinary leaf.
#[test]
fn composite_widget_has_one_attachment_and_expired_observers_cannot_mount() {
    let ui = nc_ui_nodes::bind_builtin_nodes(runtime()).unwrap();
    let (_send, messages) = std::sync::mpsc::channel();
    let (local, _) = Composite::build(&ui, Removal { requests: None, trace: Rc::default(), fail: false }, messages);
    let mut observed = local.query::<IWidgetAbi>().unwrap();
    let first = ui.window("first", RECT, local.into_interface::<IWidgetAbi>().unwrap()).unwrap();
    assert_eq!(observed.mount(), Err(UI_E_OWNER));
    first.close().unwrap();
    assert_eq!(observed.mount(), Err(neocogi_com::NC_E_DROPPED));
}

/// Idle application dispatch adds no allocation to an otherwise identical warmed update.
#[test]
fn empty_application_dispatcher_adds_no_per_update_allocations() {
    let ui = nc_ui_nodes::bind_builtin_nodes(runtime()).unwrap();
    let view = ui.widgets().button_unobserved("idle").unwrap();
    let _window = ui.window("allocation comparison", RECT, view).unwrap();
    let (_send, messages) = std::sync::mpsc::channel::<()>();
    let app = neocogi_ui_support::test_support::events::TestApp::new((), messages, |(), ()| Ok(()));
    // Warm both paths before comparing the UI's own retained-layout allocations.
    ui.update(SIZE).unwrap();
    app.update(&ui, SIZE).unwrap();
    let plain = crate::test_support::AllocationMeasurement::begin();
    for _ in 0..64 {
        ui.update(SIZE).unwrap();
    }
    let plain = plain.finish();
    let owned = crate::test_support::AllocationMeasurement::begin();
    for _ in 0..64 {
        app.update(&ui, SIZE).unwrap();
    }
    let owned = owned.finish();
    assert_eq!(owned, plain, "the application dispatcher must not allocate a per-frame adapter");
}

/// Refuse recursive removal before committing state, then synchronously destroy
/// the same owner once the scoped implementation borrow has ended.
#[test]
fn owner_removal_rejects_active_borrows_then_destroys_before_returning() {
    for close_runtime in [false, true] {
        let ui = nc_ui_nodes::bind_builtin_nodes(runtime()).unwrap();
        let trace = Rc::new(RefCell::new(Vec::new()));
        let (send, messages) = std::sync::mpsc::channel();
        let (local, _) = Composite::build(&ui, Removal { requests: None, trace: trace.clone(), fail: false }, messages);
        let mut observed = local.downgrade();
        let window = ui.window("scoped local", RECT, local.into_interface::<IWidgetAbi>().unwrap()).unwrap();
        observed.with_mut(|_| {
            let result = match close_runtime { true => ui.close(), false => window.close() };
            assert_eq!(result, Err(UI_E_BUSY));
            assert!(trace.borrow().is_empty());
        }).unwrap();
        match close_runtime { true => ui.close().unwrap(), false => window.close().unwrap() }
        assert_eq!(&*trace.borrow(), &["closed"]);
        assert!(send.send(()).is_err());
        assert_eq!(observed.with(|_| ()), Err(neocogi_com::NC_E_DROPPED));
    }
}

/// Ordinary controls do not advertise private behavior; removal expires common observations.
#[test]
fn ordinary_widgets_have_no_private_queue_and_reject_observation_after_removal() {
    let ui = nc_ui_nodes::bind_builtin_nodes(runtime()).unwrap();
    let button = ui.widgets().button_unobserved("ordinary").unwrap();
    let widget = button.node_interface().unwrap();
    assert!(matches!(widget.query::<IWidgetDispatcherAbi>(), Err(neocogi_com::NC_E_NOINTERFACE)));
    let window = ui.window("ordinary", RECT, button).unwrap();
    window.close().unwrap();
    assert_eq!(widget.properties(), Err(neocogi_com::NC_E_DROPPED));
}

/// Private commands distinguish subtree removal from work which must be discarded.
enum OrderedMessage {
    /// Asks the external parent to remove the enclosing tree after this call.
    Remove(ComPtr<IWindowAbi>),
    /// Records unexpected successor/ancestor delivery if a stale snapshot is used.
    Record,
}

/// A named composite makes descendant-before-parent closure externally observable.
struct OrderedModel {
    /// Name recorded only when this widget's terminal cleanup runs.
    name: &'static str,
    /// Test-only shared observation, with no capability back to any widget.
    trace: Rc<RefCell<Vec<&'static str>>>,
    /// Reports close intent to an application receiver outside this widget tree.
    requests: Option<std::sync::mpsc::Sender<ComPtr<IWindowAbi>>>,
    /// Failure after posting must not make intent an automatic removal decision.
    fail: bool,
}
impl MessageProcessor for OrderedModel {
    type Message = OrderedMessage;
    /// A child posts intent; only its external parent later destroys the tree.
    fn handle(&mut self, message: OrderedMessage) -> Result<(), NcStatus> {
        self.trace.borrow_mut().push("handled");
        match message {
            OrderedMessage::Remove(window) => self.requests.as_ref().expect("fixture has parent queue")
                .send(window).map_err(|_| UI_E_CLOSED)?,
            OrderedMessage::Record => {}
        }
        match self.fail {
            true => Err(NC_E_INVALID_ARG),
            false => Ok(()),
        }
    }
}
impl Drop for OrderedModel {
    /// Names prove ordering, not merely that both destructors eventually ran.
    fn drop(&mut self) {
        self.trace.borrow_mut().push(self.name);
    }
}

/// A child requests removal, returns, and then ordinary field destruction drops the whole tree.
#[test]
fn descendant_removal_request_drops_the_complete_tree_after_handler_return() {
    for fail in [false, true] {
        let ui = nc_ui_nodes::bind_builtin_nodes(runtime()).unwrap();
        let trace = Rc::new(RefCell::new(Vec::new()));
        let (requests, decisions) = std::sync::mpsc::channel();
        let app = neocogi_ui_support::test_support::events::TestApp::new((), decisions, |(), window: ComPtr<IWindowAbi>| window.close());
        let (parent_send, parent_inbox) = std::sync::mpsc::channel();
        let (parent, mut shell) = Composite::build(
            &ui,
            OrderedModel {
                name: "parent",
                trace: trace.clone(),
                requests: None,
                fail: false,
            },
            parent_inbox,
        );
        let parent_owner = parent;
        let parent = parent_owner.downgrade();
        let parent_view = neocogi_ui::OwnedWidget::from_interface(parent_owner.into_interface::<IWidgetAbi>().unwrap());
        let (child_send, child_inbox) = std::sync::mpsc::channel();
        let (child, _) = Composite::build(
            &ui,
            OrderedModel {
                name: "child",
                trace: trace.clone(),
                requests: Some(requests),
                fail,
            },
            child_inbox,
        );
        let child_owner = child;
        let child = child_owner.downgrade();
        let child_view = neocogi_ui::OwnedWidget::from_interface(child_owner.into_interface::<IWidgetAbi>().unwrap());
        shell.insert(0, child_view, LinearEdge::new(LayoutTrack::Content)).unwrap();
        let window = ui.window("ordered closure", RECT, parent_view).unwrap();
        child_send.send(OrderedMessage::Remove(window)).unwrap();
        child_send.send(OrderedMessage::Record).unwrap();
        parent_send.send(OrderedMessage::Record).unwrap();
        assert_eq!(child.query::<IWidgetDispatcherAbi>().unwrap().dispatch_next(), if fail { Err(NC_E_INVALID_ARG) } else { Ok(true) });
        assert_eq!(&*trace.borrow(), &["handled"], "requesting child has returned without destruction");
        assert_eq!(app.as_interface().dispatch_next(), Ok(ApplicationDispatch::MessageHandled));
        assert_eq!(&*trace.borrow(), &["handled", "child", "parent"]);
        assert!(child_send.send(OrderedMessage::Record).is_err());
        assert!(parent_send.send(OrderedMessage::Record).is_err());
        // Retained projections neither reactivate nor repeat terminal cleanup.
        assert_eq!(child.query::<IWidgetDispatcherAbi>().unwrap().dispatch_next(), Err(neocogi_com::NC_E_DROPPED));
        assert_eq!(parent.query::<IWidgetDispatcherAbi>().unwrap().dispatch_next(), Err(neocogi_com::NC_E_DROPPED));
        assert_eq!(trace.borrow().len(), 3);
    }
}

/// An active descendant prevents partial ancestor removal; idle removal destroys both.
#[test]
fn active_descendant_rejects_partial_removal_then_the_whole_tree_drops() {
    let ui = nc_ui_nodes::bind_builtin_nodes(runtime()).unwrap();
    let trace = Rc::new(RefCell::new(Vec::new()));
    let (_parent_send, parent_inbox) = std::sync::mpsc::channel();
    let (parent, mut shell) = Composite::build(
        &ui,
        OrderedModel {
            name: "parent",
            trace: trace.clone(),
            requests: None,
            fail: false,
        },
        parent_inbox,
    );
    let (_child_send, child_inbox) = std::sync::mpsc::channel();
    let (child, _) = Composite::build(
        &ui,
        OrderedModel {
            name: "child",
            trace: trace.clone(),
            requests: None,
            fail: false,
        },
        child_inbox,
    );
    let child_owner = child;
    let mut child = child_owner.downgrade();
    let child_view = neocogi_ui::OwnedWidget::from_interface(child_owner.into_interface::<IWidgetAbi>().unwrap());
    shell.insert(0, child_view, LinearEdge::new(LayoutTrack::Content)).unwrap();
    let parent_owner = parent;
    let parent_view = neocogi_ui::OwnedWidget::from_interface(parent_owner.into_interface::<IWidgetAbi>().unwrap());
    let _window = ui.window("blocked descendant", RECT, parent_view).unwrap();
    child.with_mut(|_| {
        assert_eq!(ui.close(), Err(UI_E_BUSY));
        assert!(trace.borrow().is_empty(), "a refusal must preserve the parent too");
    }).unwrap();
    ui.close().unwrap();
    assert_eq!(&*trace.borrow(), &["child", "parent"]);
}

/// Passive leaf behavior deliberately edits topology without claiming to consume a message.
struct TopologyEdit {
    /// Consumed once so the next visit is truly idle.
    target: Option<ComPtr<IWindowAbi>>,
    /// Both true and false must invalidate a stale dispatch snapshot after removal.
    report_message: bool,
}
impl neocogi_ui::LeafContent for TopologyEdit {
    /// Geometry is irrelevant to the scheduling probe but still uses the real layout ABI.
    fn measure(&self, _: LayoutLimits, _: ComRef<'_, ITextMetricsAbi>) -> Result<Size, NcStatus> {
        Ok(Size { width: 10, height: 10 })
    }
    /// No drawing is needed to execute a real mounted leaf.
    fn paint(&mut self, _: WidgetPaint, _: &mut neocogi_ui::Painter<'_>) -> Result<(), NcStatus> {
        Ok(())
    }

}

/// A removed later entry must never be called from the pre-removal snapshot.
#[test]
fn topology_changes_refresh_dispatch_even_when_the_widget_reports_no_message() {
    for report_message in [false, true] {
        let ui = nc_ui_nodes::bind_builtin_nodes(runtime()).unwrap();
        let trace = Rc::new(RefCell::new(Vec::new()));
        let (send, inbox) = std::sync::mpsc::channel();
        let (older, _) = Composite::build(
            &ui,
            OrderedModel {
                name: "older closed",
                trace: trace.clone(),
                requests: None,
                fail: false,
            },
            inbox,
        );
        let older_owner = older;
        let older_view = neocogi_ui::OwnedWidget::from_interface(older_owner.into_interface::<IWidgetAbi>().unwrap());
        let older_window = ui.window("removed later snapshot entry", RECT, older_view).unwrap();
        send.send(OrderedMessage::Record).unwrap();
        let editor = ui
            .widgets()
            .dispatching_leaf(
                TopologyEdit {
                    target: Some(older_window),
                    report_message,
                },
                WidgetOptions::NO_INTERACT,
            )
            .unwrap();
        let _newer_window = ui.window("runs first", RECT, editor).unwrap();
        ui.update(SIZE).unwrap();
        assert_eq!(&*trace.borrow(), &["older closed"]);
        assert!(send.send(OrderedMessage::Record).is_err());
    }
}

/// A queued non-Send value lets the test observe receiver destruction without raw pointers.
struct DropModel;
impl MessageProcessor for DropModel {
    type Message = Rc<()>;
    /// This fixture's pending value should be discarded rather than dispatched.
    fn handle(&mut self, _: Rc<()>) -> Result<(), NcStatus> {
        panic!("closed runtime must never dispatch queued work")
    }
}

/// Final runtime destruction cannot retain a blocked widget in a host/widget cycle.
#[test]
fn final_runtime_release_does_not_leak_a_borrowed_widget_or_its_private_inbox() {
    for explicit_close in [false, true] {
        let ui = nc_ui_nodes::bind_builtin_nodes(runtime()).unwrap();
        let (send, inbox) = std::sync::mpsc::channel();
        let (local, _) = Composite::build(&ui, DropModel, inbox);
        let local_owner = local;
        let mut local = local_owner.downgrade();
        let view_owner = neocogi_ui::OwnedWidget::from_interface(local_owner.into_interface::<IWidgetAbi>().unwrap());
        let view = view_owner.handle();
        let window = ui.window("final release", RECT, view_owner).unwrap();
        let value = Rc::new(());
        let weak = Rc::downgrade(&value);
        send.send(value).unwrap();
        local.with_mut(|_| ()).unwrap();
        drop(ui);
        assert!(weak.upgrade().is_none(), "runtime Drop must destroy queued values before returning");
        let widget = view.node_interface().unwrap();
        assert_eq!(widget.properties(), Err(neocogi_com::NC_E_DROPPED));
        if explicit_close {
            assert_eq!(widget.properties(), Err(neocogi_com::NC_E_DROPPED));
        }
        drop(widget);
        drop(view);
        drop(local);
        drop(window);
        assert!(weak.upgrade().is_none(), "pending cleanup must not form an ownership cycle");
        assert!(send.send(Rc::new(())).is_err());
    }
}

impl neocogi_ui::DispatchingLeafContent for TopologyEdit {
    fn dispatch_next(&mut self, _: &mut neocogi_ui::LeafDispatchContext<'_>) -> Result<bool, NcStatus> {
        match self.target.take() {
            Some(window) => {
                window.close()?;
                Ok(self.report_message)
            }
            None => Ok(false),
        }
    }
}
