//! Widget-owned behavior, lifetime and attachment regressions through the real runtime.

use super::*;
use neocogi_ui::events::EventModel;
use super::composite_fixture::{Behavior, Composite};

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
        self.window.as_ref().unwrap().close()?;
        self.trace.borrow_mut().push("handler returned");
        match self.fail {
            true => Err(NC_E_INVALID_ARG),
            false => Ok(()),
        }
    }
}
impl Behavior for Removal {
    /// Terminal behavior must run once, after the active handler releases its gate.
    fn close(&mut self) -> Result<(), NcStatus> {
        self.trace.borrow_mut().push("closed");
        Ok(())
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
        let component = neocogi_ui::Widget::from_interface(events.clone());
        let window = ui.window("owned behavior", RECT, &component).unwrap();
        local.try_borrow_mut().unwrap().model.window = Some(window.clone());
        send.send(()).unwrap();
        send.send(()).unwrap();
        assert_eq!(ui.update(SIZE), if fail { Err(NC_E_INVALID_ARG) } else { Ok(()) });
        assert_eq!(&*trace.borrow(), &["handler entered", "handler returned", "closed"]);
        assert_eq!(window.snapshot(), Err(UI_E_CLOSED));
        assert!(send.send(()).is_err(), "closure disconnects even retained widgets");
        assert_eq!(events.dispatch_next(), Err(UI_E_CLOSED));
        events.close().unwrap();
        assert_eq!(trace.borrow().len(), 3);
    }
}

/// The actual eventful widget follows the same single-attachment rule as an ordinary leaf.
#[test]
fn composite_widget_has_one_attachment_and_no_separate_local_claim() {
    let ui = nc_ui_nodes::bind_builtin_nodes(runtime()).unwrap();
    let trace = Rc::new(RefCell::new(Vec::new()));
    let (_send, messages) = std::sync::mpsc::channel();
    let (local, _) = Composite::build(&ui, Removal { window: None, trace, fail: false }, messages);
    let view = neocogi_ui::Widget::from_interface(local.query::<IWidgetAbi>().unwrap());
    let first = ui.window("first", RECT, &view).unwrap();
    assert!(matches!(ui.window("second", RECT, &view), Err(UI_E_OWNER)));
    first.close().unwrap();
    assert!(matches!(ui.window("closed", RECT, &view), Err(UI_E_CLOSED)));
}

/// Idle application dispatch adds no allocation to an otherwise identical warmed update.
#[test]
fn persistent_empty_local_adds_no_per_update_allocations() {
    let ui = nc_ui_nodes::bind_builtin_nodes(runtime()).unwrap();
    let view = ui.widgets().button_unobserved("idle").unwrap();
    let _window = ui.window("allocation comparison", RECT, &view).unwrap();
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
    assert_eq!(owned, plain, "the persistent local must not allocate a per-frame adapter");
}

/// A locally borrowed widget must retain pending closure until its guard ends.
#[test]
fn busy_closure_retries_without_resuming_private_delivery() {
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
        let events = local.query::<IWidgetAbi>().unwrap();
        let component = neocogi_ui::Widget::from_interface(events);
        let window = ui.window("guarded local", RECT, &component).unwrap();
        let held = local.try_borrow_mut().unwrap();
        match close_runtime {
            true => assert_eq!(ui.close(), Err(neocogi_com::NC_E_BUSY)),
            false => {
                window.close().unwrap();
                assert_eq!(ui.update(SIZE), Err(neocogi_com::NC_E_BUSY));
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
fn ordinary_widgets_have_no_private_queue_and_reject_dispatch_after_close() {
    let ui = nc_ui_nodes::bind_builtin_nodes(runtime()).unwrap();
    let button = ui.widgets().button_unobserved("ordinary").unwrap();
    let mut widget = button.node_interface().unwrap();
    assert_eq!(widget.dispatch_next(), Ok(false));
    let window = ui.window("ordinary", RECT, &button).unwrap();
    assert_eq!(widget.dispatch_next(), Ok(false));
    window.close().unwrap();
    assert_eq!(widget.dispatch_next(), Err(UI_E_CLOSED));
}
