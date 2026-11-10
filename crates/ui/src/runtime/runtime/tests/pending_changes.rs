//! Pending-change outcomes control real private dispatch, input ordering and update admission.
use super::*;
use super::composite_fixture::Composite;
use neocogi_ui::{
    ButtonWidget,
    events::{ApplyOutcome, MessageProcessor},
};
use neocogi_ui::support::test_support::events::TestApp;
use std::sync::mpsc;

/// Posts one real key press without synthesizing a semantic button activation.
fn key(ui: &neocogi_ui::Ui, key: KeyCode) {
    ui.input(
        UiInput::Key(KeyStroke {
            key,
            action: neocogi_ui::KeyAction::Pressed { repeat: false },
            modifiers: 0,
        }),
        "",
    )
    .unwrap();
}

/// Private child work changes a real button only when the scheduler revisits this inbox.
struct RenameButton {
    /// Weak editing handle; the composite shell owns the button allocation.
    button: ButtonWidget,
    /// Shared fixture trace proves ordering at commit/post boundaries, not final state alone.
    trace: std::rc::Rc<std::cell::RefCell<Vec<&'static str>>>,
}
impl MessageProcessor for RenameButton {
    type Message = ();
    /// Committing the new label publishes ordinary geometry notifications.
    fn handle(&mut self, (): ()) -> Result<(), NcStatus> {
        self.button.set_label("applied")?;
        self.trace.borrow_mut().push("renamed");
        Ok(())
    }
}

/// A hook which only sends child work has no geometry notification to rescue a lost outcome.
#[test]
fn applied_pending_changes_dispatch_children_before_the_first_queued_input() {
    let ui = neocogi_ui::nodes::bind_builtin_nodes(runtime()).unwrap();
    let (send, messages) = mpsc::channel();
    let trace = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
    let queue = neocogi_ui::test_event_queue!(neocogi_ui::ButtonAbi, (), send,
        |queue, _origin, event| {
            event.query_ref::<neocogi_ui::IActivatedEventAbi>()?;
            queue.trace.borrow_mut().push("activated");
        }, { trace: std::rc::Rc<std::cell::RefCell<Vec<&'static str>>> = trace.clone() })
    .unwrap();
    let button = ui.widgets().button_with_queue("before", Some(queue.as_com_ref())).unwrap();
    let observed = button.handle();
    let (child_send, child_messages) = mpsc::channel();
    let (child, mut shell) = Composite::build(
        &ui,
        RenameButton {
            button: button.handle(),
            trace: trace.clone(),
        },
        child_messages,
    );
    shell.insert(0, button, LinearEdge::new(LayoutTrack::Content)).unwrap();
    let window = ui.window("pending child work", RECT, child.into_interface::<IWidgetAbi>().unwrap()).unwrap();
    // Establish focus and drain all construction notifications before scheduling pending work.
    key(&ui, KeyCode::Tab);
    ui.update(SIZE).unwrap();
    let app = TestApp::with_context(
        (Some(child_send), Vec::new()),
        messages,
        |(_, labels), label| {
            labels.push(label);
            Ok(())
        },
        (),
        |(pending, _), ()| match pending.take() {
            Some(sender) => {
                sender.send(()).unwrap();
                Ok(ApplyOutcome::ChangesApplied)
            }
            None => Ok(ApplyOutcome::NothingPending),
        },
    );
    key(&ui, KeyCode::Space);
    app.update(&ui, SIZE).unwrap();
    assert_eq!(app.state().1, [()]);
    assert_eq!(observed.label().unwrap(), "applied");
    assert_eq!(&*trace.borrow(), &["renamed", "activated"]);
    assert_eq!(app.as_interface().dispatch_next(), Ok(ApplicationDispatch::Idle));
    window.close().unwrap();
}

/// Exactly exhausting the shared budget cannot probe another pending request, even an idle one.
#[test]
fn pending_changes_charge_the_budget_and_preserve_unattempted_requests() {
    for requested in [2, 3] {
        let source =
            crate::runtime::test_support::assets::export_bundle(&crate::runtime::SkinBundle::from_atlas(crate::runtime::test_support::test_atlas())).unwrap();
        let compositor = neocogi_ui::gfx::GfxCompositorObject::create(source.as_com_ref()).unwrap();
        let assets = crate::runtime::assets::ImportedAssets::read(source.as_com_ref(), None).unwrap();
        let runtime = Runtime {
            storage: create_storage(assets, compositor),
        };
        let (_send, messages) = mpsc::channel::<()>();
        let app = TestApp::with_context(
            (requested, 0),
            messages,
            |_, ()| unreachable!(),
            (),
            |(pending, completed), ()| match *pending {
                0 => Ok(ApplyOutcome::NothingPending),
                _ => {
                    *pending -= 1;
                    *completed += 1;
                    Ok(ApplyOutcome::ChangesApplied)
                }
            },
        );
        let mut budget = 2;
        let mut widgets = DispatchWidgets::default();
        assert_eq!(
            runtime.dispatch_boundary(Some(app.as_interface()), Dimensioni::new(SIZE.width, SIZE.height), &mut budget, &mut widgets),
            Err(UI_E_EVENT_LIMIT)
        );
        assert_eq!(*app.state(), (requested - 2, 2));
        assert_eq!(budget, 0);
        let mut budget = 2;
        runtime
            .dispatch_boundary(Some(app.as_interface()), Dimensioni::new(SIZE.width, SIZE.height), &mut budget, &mut widgets)
            .unwrap();
        assert_eq!(*app.state(), (0, requested));
    }
}

#[com_object(interfaces(IApplicationDispatcher))]
/// Returns an unknown outcome through valid raw integer storage, without invalid Rust values.
struct UnknownOutcome;
impl IApplicationDispatcher for UnknownOutcome {
    /// Unknown dispatch outcomes remain independent of otherwise valid input scopes.
    fn input_boundary(&mut self, _: InputBoundary) -> Result<(), NcStatus> {
        Ok(())
    }
    /// The Rust raw constructor admits unknown values; generated ABI output checks reject them.
    fn dispatch_next(&mut self) -> Result<ApplicationDispatch, NcStatus> {
        Ok(ApplicationDispatch::from_raw(3))
    }
}

/// An unknown result cannot authorize input or commit a renderable frame.
#[test]
fn unknown_application_dispatch_outcome_stops_before_input_and_allows_retry() {
    let ui = neocogi_ui::nodes::bind_builtin_nodes(runtime()).unwrap();
    let (send, messages) = mpsc::channel();
    let queue = neocogi_ui::test_event_queue!(neocogi_ui::ButtonAbi, (), send, |_queue, _origin, _event| ()).unwrap();
    let button = ui.widgets().button_with_queue("queued", Some(queue.as_com_ref())).unwrap();
    let window = ui.window("unknown outcome", RECT, button).unwrap();
    key(&ui, KeyCode::Tab);
    ui.update(SIZE).unwrap();
    let dispatcher: ComPtr<IApplicationDispatcherAbi> = UnknownOutcome::from_value(UnknownOutcome).query().unwrap();
    key(&ui, KeyCode::Space);
    assert_eq!(ui.update_with_events(SIZE, dispatcher.as_com_ref()), Err(neocogi_com::NC_E_INVALID_TAG));
    let (renderer, _) = renderer();
    assert_eq!(ui.render(SIZE, renderer.as_com_ref()).map_err(|error| error.status), Err(UI_E_UPDATE_REQUIRED));
    let app = TestApp::new(0, messages, |count, ()| {
        *count += 1;
        Ok(())
    });
    app.update(&ui, SIZE).unwrap();
    assert_eq!(*app.state(), 1);
    ui.render(SIZE, renderer.as_com_ref()).unwrap();
    window.close().unwrap();
}

/// A failed pending request preserves its committed edits and leaves raw input for retry.
#[test]
fn failed_pending_changes_leave_input_queued_and_the_frame_uncommitted() {
    let ui = neocogi_ui::nodes::bind_builtin_nodes(runtime()).unwrap();
    let (send, messages) = mpsc::channel();
    let queue = neocogi_ui::test_event_queue!(neocogi_ui::ButtonAbi, (), send, |_queue, _origin, _event| ()).unwrap();
    let button = ui.widgets().button_with_queue("queued", Some(queue.as_com_ref())).unwrap();
    let window = ui.window("pending failure", RECT, button).unwrap();
    key(&ui, KeyCode::Tab);
    ui.update(SIZE).unwrap();
    let app = TestApp::with_context(
        (Some(NC_E_INVALID_ARG), Vec::new()),
        messages,
        |(_, trace), ()| {
            trace.push("input");
            Ok(())
        },
        (),
        |(pending, trace), ()| match pending.take() {
            Some(error) => {
                trace.push("failed");
                Err(error)
            }
            None => Ok(ApplyOutcome::NothingPending),
        },
    );
    key(&ui, KeyCode::Space);
    assert_eq!(app.update(&ui, SIZE), Err(NC_E_INVALID_ARG));
    assert_eq!(app.state().1, ["failed"]);
    let (renderer, _) = renderer();
    assert_eq!(ui.render(SIZE, renderer.as_com_ref()).map_err(|error| error.status), Err(UI_E_UPDATE_REQUIRED));
    app.update(&ui, SIZE).unwrap();
    assert_eq!(app.state().1, ["failed", "input"]);
    ui.render(SIZE, renderer.as_com_ref()).unwrap();
    window.close().unwrap();
}

/// Conservative geometry notifications still protect scheduling when a hook misreports idle.
#[test]
fn idle_application_outcome_still_reconciles_notified_geometry() {
    let ui = neocogi_ui::nodes::bind_builtin_nodes(runtime()).unwrap();
    let button = ui.widgets().button_unobserved("before").unwrap();
    let observed = button.handle();
    let window = ui.window("idle notification", RECT, button).unwrap();
    ui.update(SIZE).unwrap();
    let (_send, messages) = mpsc::channel::<()>();
    let app = TestApp::with_context(
        Some(observed),
        messages,
        |_, ()| unreachable!(),
        (),
        |pending, ()| {
            if let Some(mut button) = pending.take() {
                button.set_label("notified geometry")?;
            }
            Ok(ApplyOutcome::NothingPending)
        },
    );
    app.update(&ui, SIZE).unwrap();
    let (renderer, _) = renderer();
    ui.render(SIZE, renderer.as_com_ref()).unwrap();
    assert_eq!(app.as_interface().dispatch_next(), Ok(ApplicationDispatch::Idle));
    window.close().unwrap();
}
