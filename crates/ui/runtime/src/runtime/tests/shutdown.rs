//! Terminal context teardown is distinct from notifying removal in a running UI.
use super::*;
use super::composite_fixture::Composite;
use neocogi_com::NcValueRef;
use neocogi_ui::events::EventModel;
use std::sync::mpsc::{self, Sender};

#[com_object(interfaces(IWindowEventQueue))]
/// Records every attempted notification, including ones deliberately rejected.
struct WindowQueue {
    /// Application-owned facts must survive destruction of their producer.
    events: Sender<WindowEvent>,
    /// Rejection distinguishes no posting from posting and silently losing its error.
    reject_destroyed: bool,
    /// Reports final receiver release independently of retained window capabilities.
    dropped: Sender<&'static str>,
}
impl IWindowEventQueue for WindowQueue {
    /// Accepts ordinary lifecycle facts and optionally rejects terminal delivery.
    fn post(&mut self, _: ComRef<'_, IWindowAbi>, event: NcValueRef<'_, WindowEvent>) -> Result<(), NcStatus> {
        let event = event.to_owned()?;
        let result = match &event {
            WindowEvent::Destroyed if self.reject_destroyed => Err(NC_E_INVALID_ARG),
            _ => Ok(()),
        };
        self.events.send(event).unwrap();
        result
    }
}
impl Drop for WindowQueue {
    /// Observer storage can disappear first when a test fails; Drop remains infallible.
    fn drop(&mut self) {
        let _ = self.dropped.send("queue");
    }
}

/// Both private queued payloads and their model expose synchronous destruction.
struct Lifetime {
    /// Distinguishes the inbox's pending value from its owning behavior.
    name: &'static str,
    /// Independent observation never retains the UI or an owning widget handle.
    dropped: Sender<&'static str>,
}
impl Drop for Lifetime {
    /// Records destruction once, without making receiver lifetime a cleanup condition.
    fn drop(&mut self) {
        let _ = self.dropped.send(self.name);
    }
}
impl EventModel for Lifetime {
    /// Owned pending values must be dropped without dispatch during shutdown.
    type Message = Lifetime;
    /// A shutdown request never authorizes running remaining private application work.
    fn handle(&mut self, _: Lifetime) -> Result<(), NcStatus> {
        panic!("shutdown must not dispatch private messages")
    }
}

/// Whole-context shutdown releases every surface type without calling lifecycle queues.
/// Facts already delivered to application-owned storage remain readable afterward.
#[test]
fn runtime_shutdown_is_silent_and_preserves_already_queued_facts() {
    for reject_destroyed in [false, true] {
        let ui = nc_ui_nodes::bind_builtin_nodes(runtime()).unwrap();
        let (send, events) = mpsc::channel();
        let (dropped, drops) = mpsc::channel();
        let queue = WindowQueue::from_value(WindowQueue {
            events: send,
            reject_destroyed,
            dropped: dropped.clone(),
        })
        .query::<IWindowEventQueueAbi>()
        .unwrap();
        let (pending, inbox) = mpsc::channel();
        let (root, mut children) = Composite::build(&ui, Lifetime { name: "model", dropped: dropped.clone() }, inbox);
        let root_view = root.query::<IWidgetAbi>().unwrap();
        let child = ui.widgets().button_unobserved("nested widget").unwrap();
        let child_view = child.node_interface().unwrap();
        children.insert(0, child, LinearEdge::new(LayoutTrack::Content)).unwrap();
        let root = ui
            .window_with_queue("root", RECT, root.into_interface::<IWidgetAbi>().unwrap(), Some(queue.as_com_ref()))
            .unwrap();
        let mut views = vec![root_view, child_view];
        let mut windows = vec![];
        for name in ["independent", "child", "dialog"] {
            let owner = ui.widgets().button_unobserved(name).unwrap();
            views.push(owner.node_interface().unwrap());
            let window = match name {
                "independent" => ui.window_with_queue(name, RECT, owner, Some(queue.as_com_ref())),
                "child" => ui.child_window_with_queue(root.as_com_ref(), name, RECT, owner, Some(queue.as_com_ref())),
                "dialog" => ui.dialog_with_queue(root.as_com_ref(), name, RECT, owner, Some(queue.as_com_ref())),
                _ => unreachable!(),
            }
            .unwrap();
            windows.push(window);
        }
        let (popup_send, popup_events) = mpsc::channel();
        let popup_queue = neocogi_ui::test_event_queue!(popup, PopupDismissed, popup_send, |_queue, _origin, event| event).unwrap();
        let owner = ui.widgets().button_unobserved("popup").unwrap();
        views.push(owner.node_interface().unwrap());
        let mut popup = ui.popup_with_queue(root.as_com_ref(), "popup", owner, Some(popup_queue.as_com_ref())).unwrap();
        drop((queue, popup_queue));
        // Commit an application fact, then keep the popup active across terminal teardown.
        root.set_visibility(Visibility::Hidden).unwrap();
        root.set_visibility(Visibility::Visible).unwrap();
        popup.show_at(RECT).unwrap();
        assert_eq!(popup.visibility().unwrap(), Visibility::Visible);
        assert!(popup_events.try_recv().is_err());
        pending.send(Lifetime { name: "pending", dropped }).unwrap();
        ui.input(UiInput::Text, "never dispatched").unwrap();

        ui.close().unwrap();
        assert!(views.iter().all(|view| !view.is_alive()));
        for view in views {
            assert_eq!(view.properties(), Err(neocogi_com::NC_E_DROPPED));
        }
        let mut found: Vec<_> = drops.try_iter().collect();
        found.sort();
        assert_eq!(found, ["model", "pending", "queue"]);
        let facts: Vec<_> = events.try_iter().collect();
        assert!(
            matches!(facts.as_slice(), [WindowEvent::Hidden]),
            "shutdown preserves the earlier fact without posting any new lifecycle event"
        );
        assert!(matches!(popup_events.try_recv(), Err(mpsc::TryRecvError::Disconnected)));
        assert_eq!(ui.update(SIZE), Err(UI_E_CLOSED));
        assert_eq!(root.snapshot(), Err(UI_E_CLOSED));
        assert_eq!(popup.visibility(), Err(UI_E_CLOSED));
        ui.close().unwrap();
        root.close().unwrap();
        for window in windows {
            window.close().unwrap();
        }
        popup.close().unwrap();
        drop(ui);
        assert!(drops.try_recv().is_err(), "repeated close and final Drop do not repeat destruction");
        assert!(events.try_recv().is_err());
    }
}

/// Refusal occurs before the terminal flag, any notification, or any tree destruction.
#[test]
fn busy_shutdown_preserves_tree_and_queue_until_retry() {
    let ui = nc_ui_nodes::bind_builtin_nodes(runtime()).unwrap();
    let (send, events) = mpsc::channel();
    let (dropped, drops) = mpsc::channel();
    let queue = WindowQueue::from_value(WindowQueue {
        events: send,
        reject_destroyed: true,
        dropped: dropped.clone(),
    })
    .query::<IWindowEventQueueAbi>()
    .unwrap();
    let (_pending, inbox) = mpsc::channel();
    let (root, _) = Composite::build(&ui, Lifetime { name: "model", dropped }, inbox);
    let mut observed = root.downgrade();
    let window = ui
        .window_with_queue("busy", RECT, root.into_interface::<IWidgetAbi>().unwrap(), Some(queue.as_com_ref()))
        .unwrap();
    drop(queue);
    observed
        .with_mut(|_| {
            assert_eq!(ui.close(), Err(UI_E_BUSY));
            assert!(window.snapshot().is_ok());
            assert!(events.try_recv().is_err());
            assert!(drops.try_recv().is_err());
        })
        .unwrap();
    ui.update(SIZE).unwrap();
    let _ = events.try_iter().collect::<Vec<_>>();
    ui.close().unwrap();
    assert_eq!(observed.with(|_| ()), Err(neocogi_com::NC_E_DROPPED));
    assert!(events.try_recv().is_err());
    assert_eq!(drops.try_iter().count(), 2);
}

/// Single-window removal still reports delivery failures after complete destruction.
#[test]
fn individual_removal_keeps_terminal_notifications_and_error_reporting() {
    let ui = nc_ui_nodes::bind_builtin_nodes(runtime()).unwrap();
    let (send, events) = mpsc::channel();
    let (dropped, drops) = mpsc::channel();
    let queue = WindowQueue::from_value(WindowQueue {
        events: send,
        reject_destroyed: true,
        dropped,
    })
    .query::<IWindowEventQueueAbi>()
    .unwrap();
    let root = ui.widgets().button_unobserved("removed").unwrap();
    let observed = root.node_interface().unwrap();
    let window = ui.window_with_queue("removed", RECT, root, Some(queue.as_com_ref())).unwrap();
    let child = ui
        .dialog_with_queue(
            window.as_com_ref(),
            "descendant",
            RECT,
            ui.widgets().button_unobserved("child").unwrap(),
            Some(queue.as_com_ref()),
        )
        .unwrap();
    let survivor = ui.window("surviving UI", RECT, ui.widgets().button_unobserved("survivor").unwrap()).unwrap();
    drop(queue);

    assert_eq!(window.close(), Err(NC_E_INVALID_ARG));
    assert!(!observed.is_alive());
    assert_eq!(child.snapshot(), Err(UI_E_CLOSED));
    assert_eq!(events.try_iter().filter(|event| matches!(event, WindowEvent::Destroyed)).count(), 2);
    assert_eq!(drops.try_recv(), Ok("queue"));
    assert!(survivor.snapshot().is_ok());
    ui.update(SIZE).unwrap();
    window.close().unwrap();
    assert!(events.try_recv().is_err());
}

/// Explicit close releases context resources even while the public runtime stays retained.
#[test]
fn shutdown_releases_private_context_and_compositor_before_final_runtime_drop() {
    let source = crate::test_support::assets::export_bundle(&crate::SkinBundle::from_atlas(crate::test_support::test_atlas())).unwrap();
    let gfx = crate::test_support::gfx_factory();
    let compositor = gfx.create_compositor(source.as_com_ref()).unwrap();
    let observed = compositor.downgrade().unwrap();
    let assets = crate::assets::ImportedAssets::read(source.as_com_ref(), None).unwrap();
    let storage = create_storage(assets, compositor);
    let runtime: ComPtr<UiRuntimeAbi> = Runtime::from_value(Runtime { storage: storage.clone() }).query().unwrap();
    runtime.close().unwrap();
    assert!(storage.context.borrow().is_none());
    assert!(observed.upgrade().unwrap().is_none());
    assert_eq!(runtime.update(SIZE), Err(UI_E_CLOSED));
    runtime.close().unwrap();
}

/// Final owner release follows the same silent context destruction as explicit shutdown.
#[test]
fn final_runtime_drop_does_not_publish_window_destruction() {
    let ui = nc_ui_nodes::bind_builtin_nodes(runtime()).unwrap();
    let (send, events) = mpsc::channel();
    let (dropped, drops) = mpsc::channel();
    let queue = WindowQueue::from_value(WindowQueue {
        events: send,
        reject_destroyed: true,
        dropped,
    })
    .query::<IWindowEventQueueAbi>()
    .unwrap();
    let owner = ui.widgets().button_unobserved("final drop").unwrap();
    let observed = owner.node_interface().unwrap();
    let window = ui.window_with_queue("final drop", RECT, owner, Some(queue.as_com_ref())).unwrap();
    drop(queue);
    drop(ui);
    assert!(!observed.is_alive());
    assert!(matches!(events.try_recv(), Err(mpsc::TryRecvError::Disconnected)));
    assert_eq!(drops.try_recv(), Ok("queue"));
    window.close().unwrap();
}

/// Root application messages can terminate the context after all widget calls return.
#[test]
fn application_shutdown_stops_the_update_and_leaves_later_application_work_queued() {
    /// Application-owned work has its own lifetime after the UI context has ended.
    enum Message {
        /// The application already approved complete context teardown.
        Shutdown,
        /// Must not run as part of the update that shuts down the context.
        Later,
    }
    let ui = nc_ui_nodes::bind_builtin_nodes(runtime()).unwrap();
    let owner = ui.widgets().button_unobserved("application shutdown").unwrap();
    let widget = owner.node_interface().unwrap();
    let window = ui.window("application shutdown", RECT, owner).unwrap();
    let (send, messages) = mpsc::channel();
    let app = neocogi_ui_support::test_support::events::TestApp::new((ui.clone(), 0), messages, |(ui, later), message| match message {
        Message::Shutdown => ui.close(),
        Message::Later => {
            *later += 1;
            Ok(())
        }
    });
    send.send(Message::Shutdown).unwrap();
    send.send(Message::Later).unwrap();
    assert_eq!(app.update(&ui, SIZE), Err(UI_E_CLOSED));
    assert!(!widget.is_alive());
    assert_eq!(window.snapshot(), Err(UI_E_CLOSED));
    assert_eq!(app.state().1, 0);
    // The application may deliberately process its own inbox without the closed UI.
    assert_eq!(app.as_interface().dispatch_next(), Ok(ApplicationDispatch::MessageHandled));
    assert_eq!(app.state().1, 1);
    ui.close().unwrap();
}
