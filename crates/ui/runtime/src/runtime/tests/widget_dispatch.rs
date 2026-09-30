//! Widget-owned behavior, lifetime and attachment regressions through the real runtime.

use super::*;
use neocogi_ui::events::EventModel;
use super::composite_fixture::Composite;

/// Behavior can remove its own view while its one mutable model borrow remains active.
struct Removal {
    /// Installed after atomic attachment, before any pending message is dispatched.
    window: Option<ComPtr<IWindowAbi>>,
    /// External test observation contains no reference back to the widget.
    trace: Rc<RefCell<Vec<&'static str>>>,
    /// Exercises cleanup on the handler's recoverable error path as well as success.
    fail: bool,
}

impl EventModel for Removal {
    type Message = ();
    /// Records both sides of removal so cleanup cannot hide recursive model access.
    fn handle(&mut self, _: ()) -> Result<(), NcStatus> {
        self.trace.borrow_mut().push("handler entered");
        self.window.as_ref().unwrap().request_close()?;
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

/// Self-removal closes the inbox and defers finalization through both handler exit paths.
#[test]
fn removed_widget_finalizes_after_handler_return_even_on_error() {
    for fail in [false, true] {
        let ui = nc_ui_nodes::bind_builtin_nodes(runtime()).unwrap();
        let trace = Rc::new(RefCell::new(Vec::new()));
        let (send, messages) = std::sync::mpsc::channel();
        let (local, _) = Composite::build(&ui, Removal { window: None, trace: trace.clone(), fail }, messages);
        let mut events = local.query::<IWidgetAbi>().unwrap();
        let local_owner = local;
        let mut local = local_owner.downgrade();
        let component = neocogi_ui::OwnedWidget::from_interface(local_owner.into_interface::<IWidgetAbi>().unwrap());
        let window = ui.window("owned behavior", RECT, component).unwrap();
        local.try_borrow_mut().unwrap().model.window = Some(window.clone());
        send.send(()).unwrap();
        send.send(()).unwrap();
        assert_eq!(ui.update(SIZE), if fail { Err(NC_E_INVALID_ARG) } else { Ok(()) });
        assert_eq!(&*trace.borrow(), &["handler entered", "handler returned", "closed"]);
        assert_eq!(window.snapshot(), Err(UI_E_CLOSED));
        assert!(send.send(()).is_err(), "closure disconnects even retained widgets");
        assert_eq!(events.dispatch_next(), Err(UI_E_CLOSED));
        assert_eq!(events.dispatch_next(), Err(UI_E_CLOSED));
        assert_eq!(trace.borrow().len(), 3);
    }
}

/// The actual eventful widget follows the same single-attachment rule as an ordinary leaf.
#[test]
fn composite_widget_has_one_attachment_and_expired_observers_cannot_mount() {
    let ui = nc_ui_nodes::bind_builtin_nodes(runtime()).unwrap();
    let (_send, messages) = std::sync::mpsc::channel();
    let (local, _) = Composite::build(&ui, Removal { window: None, trace: Rc::default(), fail: false }, messages);
    let mut observed = local.query::<IWidgetAbi>().unwrap();
    let first = ui.window("first", RECT, local.into_interface::<IWidgetAbi>().unwrap()).unwrap();
    assert_eq!(observed.mount(), Err(UI_E_OWNER));
    first.close().unwrap();
    assert_eq!(observed.mount(), Err(UI_E_CLOSED));
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

/// Removing an owner rejects new calls while an admitted local guard safely finishes.
#[test]
fn owner_removal_rejects_delivery_while_an_admitted_call_finishes() {
    for close_runtime in [false, true] {
        let ui = nc_ui_nodes::bind_builtin_nodes(runtime()).unwrap();
        let trace = Rc::new(RefCell::new(Vec::new()));
        let (send, messages) = std::sync::mpsc::channel();
        let (local, _) = Composite::build(
            &ui,
            Removal {
                window: None,
                trace: trace.clone(),
                fail: false,
            },
            messages,
        );
        let local_owner = local;
        let mut local = local_owner.downgrade();
        let component = neocogi_ui::OwnedWidget::from_interface(local_owner.into_interface::<IWidgetAbi>().unwrap());
        let window = ui.window("guarded local", RECT, component).unwrap();
        let held = local.try_borrow_mut().unwrap();
        match close_runtime {
            true => assert_eq!(ui.close(), Ok(())),
            false => {
                window.close().unwrap();
                assert_eq!(ui.update(SIZE), Ok(()));
            }
        }
        assert!(trace.borrow().is_empty());
        drop(held);
        match close_runtime {
            true => ui.close().unwrap(),
            false => ui.update(SIZE).unwrap(),
        }
        assert_eq!(&*trace.borrow(), &["closed"]);
        assert!(send.send(()).is_err());
    }
}

/// The new common ABI method is idle for ordinary widgets and respects closure.
#[test]
fn ordinary_widgets_have_no_private_queue_and_reject_dispatch_after_removal() {
    let ui = nc_ui_nodes::bind_builtin_nodes(runtime()).unwrap();
    let button = ui.widgets().button_unobserved("ordinary").unwrap();
    let mut widget = button.node_interface().unwrap();
    assert_eq!(widget.dispatch_next(), Ok(false));
    let window = ui.window("ordinary", RECT, button).unwrap();
    assert_eq!(widget.dispatch_next(), Ok(false));
    window.close().unwrap();
    assert_eq!(widget.dispatch_next(), Err(UI_E_CLOSED));
}

/// Private commands distinguish subtree removal from work which must be discarded.
enum OrderedMessage {
    /// Removes the enclosing tree while the receiver's mutable COM call is active.
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
    /// Error injection after committed removal must not bypass pending cleanup.
    fail: bool,
}
impl EventModel for OrderedModel {
    type Message = OrderedMessage;
    /// A removal handler may destroy its parent but protects its own model until return.
    fn handle(&mut self, message: OrderedMessage) -> Result<(), NcStatus> {
        self.trace.borrow_mut().push("handled");
        match message {
            OrderedMessage::Remove(window) => window.request_close()?,
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
        let (parent_send, parent_inbox) = std::sync::mpsc::channel();
        let (parent, mut shell) = Composite::build(
            &ui,
            OrderedModel {
                name: "parent",
                trace: trace.clone(),
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
        assert_eq!(ui.update(SIZE), if fail { Err(NC_E_INVALID_ARG) } else { Ok(()) });
        assert_eq!(&*trace.borrow(), &["handled", "child", "parent"]);
        assert!(child_send.send(OrderedMessage::Record).is_err());
        assert!(parent_send.send(OrderedMessage::Record).is_err());
        // Retained projections neither reactivate nor repeat terminal cleanup.
        assert_eq!(child.query::<IWidgetAbi>().unwrap().dispatch_next(), Err(UI_E_CLOSED));
        assert_eq!(parent.query::<IWidgetAbi>().unwrap().dispatch_next(), Err(UI_E_CLOSED));
        assert_eq!(trace.borrow().len(), 3);
    }
}

/// A local execution guard delays only its own value; ancestor destruction never waits.
#[test]
fn admitted_descendant_finishes_after_its_parent_is_destroyed() {
    let ui = nc_ui_nodes::bind_builtin_nodes(runtime()).unwrap();
    let trace = Rc::new(RefCell::new(Vec::new()));
    let (_parent_send, parent_inbox) = std::sync::mpsc::channel();
    let (parent, mut shell) = Composite::build(
        &ui,
        OrderedModel {
            name: "parent",
            trace: trace.clone(),
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
    let held = child.try_borrow_mut().unwrap();
    assert_eq!(ui.close(), Ok(()));
    assert_eq!(&*trace.borrow(), &["parent"], "removal never waits for an admitted child call");
    drop(held);
    ui.close().unwrap();
    assert_eq!(&*trace.borrow(), &["parent", "child"]);
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
    fn paint(&self, _: WidgetPaint, _: &mut neocogi_ui::Painter<'_>) -> Result<(), NcStatus> {
        Ok(())
    }
    /// Removal invalidates the tree independently of the reported work flag.
    fn dispatch_next(&mut self) -> Result<bool, NcStatus> {
        match self.target.take() {
            Some(window) => {
                window.request_close()?;
                Ok(self.report_message)
            }
            None => Ok(false),
        }
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
            .leaf(
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
impl EventModel for DropModel {
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
        let held = local.try_borrow_mut().unwrap();
        drop(ui);
        drop(held);
        let mut widget = view.node_interface().unwrap();
        assert_eq!(widget.dispatch_next(), Err(UI_E_CLOSED));
        if explicit_close {
            assert_eq!(widget.dispatch_next(), Err(UI_E_CLOSED));
        }
        drop(widget);
        drop(view);
        drop(local);
        drop(window);
        assert!(weak.upgrade().is_none(), "pending cleanup must not form an ownership cycle");
        assert!(send.send(Rc::new(())).is_err());
    }
}
