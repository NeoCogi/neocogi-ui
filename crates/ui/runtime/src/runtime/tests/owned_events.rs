//! Owned component lifetime and attachment regressions through the real runtime.

use super::*;
use neocogi_ui::events::{Controller, EventModel};

/// Behavior can remove its own view while its one mutable model borrow remains active.
struct Removal {
    /// Installed after atomic attachment, before any pending message is dispatched.
    window: Option<ComPtr<IWindowAbi>>,
    /// External test observation contains no reference back to the controller.
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
    /// Terminal behavior must run once, after the active handler releases its gate.
    fn retire(&mut self) -> Result<(), NcStatus> {
        self.trace.borrow_mut().push("retired");
        Ok(())
    }
}

/// Self-removal closes the inbox and defers finalization through both handler exit paths.
#[test]
fn removed_controller_finalizes_after_handler_return_even_on_error() {
    for fail in [false, true] {
        let ui = nc_ui_nodes::bind_builtin_nodes(runtime()).unwrap();
        let trace = Rc::new(RefCell::new(Vec::new()));
        let (send, messages) = std::sync::mpsc::channel();
        let owner = Controller::into_local(Controller::new(Removal { window: None, trace: trace.clone(), fail }, messages));
        let mut events = owner.query::<IEventOwnerAbi>().unwrap();
        let view = ui.widgets().button("owned view").unwrap();
        let component = neocogi_ui::Component::new(view, events.clone());
        let window = ui.window("owned behavior", RECT, &component).unwrap();
        owner.try_borrow_mut().unwrap().model.window = Some(window.clone());
        send.send(()).unwrap();
        send.send(()).unwrap();
        assert_eq!(ui.update(SIZE), if fail { Err(NC_E_INVALID_ARG) } else { Ok(()) });
        assert_eq!(&*trace.borrow(), &["handler entered", "handler returned", "retired"]);
        assert_eq!(window.snapshot(), Err(UI_E_CLOSED));
        assert!(send.send(()).is_err(), "retirement disconnects even retained controllers");
        assert_eq!(events.dispatch_next(), Err(UI_E_CLOSED));
        events.retire().unwrap();
        assert_eq!(trace.borrow().len(), 3);
    }
}

/// Attachment failures preserve the unclaimed side of the view/controller pair.
#[test]
fn component_attachment_claims_each_view_and_owner_only_once() {
    let ui = nc_ui_nodes::bind_builtin_nodes(runtime()).unwrap();
    let trace = Rc::new(RefCell::new(Vec::new()));
    let (_send, messages) = std::sync::mpsc::channel();
    let owner = Controller::into_local(Controller::new(Removal { window: None, trace, fail: false }, messages));
    let events = owner.query::<IEventOwnerAbi>().unwrap();
    let used_view = ui.widgets().button("already mounted").unwrap();
    let first = ui.window("first", RECT, &used_view).unwrap();
    let rejected = neocogi_ui::Component::new(used_view, events.clone());
    assert!(matches!(ui.window("rejected view", RECT, &rejected), Err(UI_E_OWNER)));

    // A rejected view left the controller unclaimed; another fresh view can use it.
    let live = neocogi_ui::Component::new(ui.widgets().button("fresh view").unwrap(), events.clone());
    let second = ui.window("second", RECT, &live).unwrap();
    let untouched = ui.widgets().button("still unclaimed").unwrap();
    let rejected = neocogi_ui::Component::new(untouched.clone(), events);
    assert!(matches!(ui.window("rejected controller", RECT, &rejected), Err(UI_E_OWNER)));
    // The controller rejected the second attachment before claiming its proposed view.
    let third = ui.window("third", RECT, &untouched).unwrap();
    first.close().unwrap();
    second.close().unwrap();
    third.close().unwrap();
    ui.update(SIZE).unwrap();
}
