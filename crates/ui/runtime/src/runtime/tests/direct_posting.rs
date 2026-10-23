//! Direct native update errors cross the generated COM boundary without shadow context state.
use super::*;
use neocogi_ui::events::{EventMessage, EventOrigin, IEventAbi, IEventReceiver, IEventReceiverAbi};
use neocogi_ui_core_contracts::{IClipboardCompletionAbi, ITextClipboard, ITextClipboardAbi};

#[com_object(interfaces(neocogi_ui_core_contracts::ITextClipboard))]
/// A platform read whose completion the test delivers between real widget ABI calls.
struct DeferredRead {
    /// The platform retains only the completion capability, never the editor.
    requests: std::sync::mpsc::Sender<ComPtr<IClipboardCompletionAbi>>,
}
impl ITextClipboard for DeferredRead {
    fn read(&self, completion: ComRef<'_, IClipboardCompletionAbi>) -> Result<(), NcStatus> {
        self.requests.send(completion.try_add_ref()?).map_err(|_| neocogi_com::NC_E_CLOSED)
    }
    fn write(&self, _: NcCopyFromBufferOffset<'_, u8>, _: ComRef<'_, IClipboardCompletionAbi>) -> Result<(), NcStatus> {
        Err(neocogi_com::NC_E_INVALID_ARG)
    }
}

#[com_object(interfaces(neocogi_ui::events::IEventReceiver))]
/// Observes the first failed snapshot without retaining its owner or source.
struct RejectSnapshots {
    /// Records every attempted post, including an erroneous second emission.
    attempts: std::sync::mpsc::Sender<WeakComBox<IEventAbi>>,
}
impl IEventReceiver for RejectSnapshots {
    fn post(&mut self, _: neocogi_com::NcValueRef<'_, EventOrigin>, event: ComBox<IEventAbi>) -> Result<(), (NcStatus, ComBox<IEventAbi>)> {
        self.attempts.send(event.downgrade()).unwrap();
        Err((neocogi_com::NC_E_CLOSED, event))
    }
}

/// Valid focused input geometry for the native single-line editor.
fn focused() -> WidgetUpdate {
    let rect = Rect { x: 0, y: 0, width: 120, height: 24 };
    WidgetUpdate {
        in_hover_root: true,
        rect,
        clip: rect,
        screen: rect,
        hovered: true,
        focused: true,
        clicked: false,
        active: false,
        action: WidgetAction::None,
        mouse_buttons: 0,
        modifiers: 0,
    }
}

/// A completed paste and Enter produce two facts on success; rejection stops after the first.
/// Both paths must preserve committed text, cursor and geometry invalidation.
#[test]
fn native_textbox_commits_before_post_and_stops_emitting_after_rejection() {
    for reject in [false, true] {
        let source = crate::test_support::assets::export_bundle(&crate::SkinBundle::from_atlas(crate::test_support::test_atlas())).unwrap();
        let gfx = neocogi_ui_support::test_support::gfx_factory(crate::test_support::host_runtime());
        let (mut runtime, driver) = crate::runtime::testing::create_runtime(source.as_com_ref(), gfx.as_com_ref()).unwrap();
        let (send, requests) = std::sync::mpsc::channel();
        let clipboard: ComPtr<ITextClipboardAbi> = DeferredRead::from_value(DeferredRead { requests: send }).query().unwrap();
        runtime.set_clipboard(clipboard.as_com_ref()).unwrap();
        let ui = nc_ui_nodes::bind_builtin_nodes(runtime).unwrap();
        let (accepted, messages) = neocogi_ui::events::channel();
        let (attempts, rejected) = std::sync::mpsc::channel();
        let rejecting: ComPtr<IEventReceiverAbi> = RejectSnapshots::from_value(RejectSnapshots { attempts }).query().unwrap();
        let receiver = if reject { &rejecting } else { &accepted };
        let textbox = ui.widgets().textbox_with_queue("", Some(receiver.as_com_ref())).unwrap();
        let handle = textbox.handle();
        let mut widget = handle.as_interface().query::<IWidgetAbi>().unwrap();
        let origin = EventOrigin::Widget(widget.clone().into_unknown());
        // Drain constructor flags to isolate the edit's sticky invalidation.
        widget.take_change().unwrap();
        let key = |key, value, modifiers| {
            WidgetEvent::Key(KeyStroke {
                key,
                value,
                pressed: true,
                repeat: false,
                modifiers,
            })
        };
        driver
            .with_origin(origin.clone(), || {
                widget.update(focused(), key(KeyCode::Character, 'v' as u32, 2), NcCopyFromBufferOffset::new(0, b""))
            })
            .unwrap()
            .unwrap();
        let completion = requests.try_recv().unwrap();
        completion.complete(0, NcCopyFromBufferOffset::new(0, "pasted α".as_bytes())).unwrap();
        let result = driver
            .with_origin(origin.clone(), || {
                widget.update(focused(), key(KeyCode::Enter, 0, 0), NcCopyFromBufferOffset::new(0, b""))
            })
            .unwrap();
        assert_eq!(result, if reject { Err(neocogi_com::NC_E_CLOSED) } else { Ok(GeometryChange::Changed) });
        assert_eq!(handle.text().unwrap(), "pasted α");
        assert_eq!(handle.cursor().unwrap(), "pasted α".len() as u64);
        assert_eq!(widget.take_change(), Ok(GeometryChange::Changed));
        assert_eq!(widget.take_change(), Ok(GeometryChange::NoChange));
        if reject {
            let failed = rejected.try_recv().unwrap();
            assert!(!failed.is_alive(), "rejected owner is destroyed synchronously");
            assert!(rejected.try_recv().is_err(), "submitted must not be attempted after changed fails");
            assert!(messages.try_recv().is_err());
        } else {
            let changed: EventMessage = messages.try_recv().unwrap();
            let submitted = messages.try_recv().unwrap();
            assert_eq!(
                changed
                    .query::<neocogi_ui::ITextChangedEventAbi>()
                    .unwrap()
                    .unwrap()
                    .text()
                    .unwrap()
                    .to_string()
                    .unwrap(),
                "pasted α"
            );
            assert_eq!(
                submitted
                    .query::<neocogi_ui::ITextSubmittedEventAbi>()
                    .unwrap()
                    .unwrap()
                    .text()
                    .unwrap()
                    .to_string()
                    .unwrap(),
                "pasted α"
            );
            assert!(messages.try_recv().is_err());
        }
        assert_eq!(
            driver
                .with_origin(origin, || widget.update(focused(), WidgetEvent::None, NcCopyFromBufferOffset::new(0, b"")))
                .unwrap(),
            Ok(GeometryChange::NoChange)
        );
        assert_eq!(handle.text().unwrap(), "pasted α", "recovery neither loses nor replays the edit");
        assert!(rejected.try_recv().is_err());
        assert!(messages.try_recv().is_err());
    }
}

/// Error facts use the same host stack, restore nested scopes and outlive both source widgets.
#[test]
fn error_reports_capture_scoped_sources_and_survive_source_destruction() {
    use neocogi_ui::events::{error, IErrorEventAbi};
    let source = crate::test_support::assets::export_bundle(&crate::SkinBundle::from_atlas(crate::test_support::test_atlas())).unwrap();
    let gfx = neocogi_ui_support::test_support::gfx_factory(crate::test_support::host_runtime());
    let (runtime, driver) = crate::runtime::testing::create_runtime(source.as_com_ref(), gfx.as_com_ref()).unwrap();
    let ui = nc_ui_nodes::bind_builtin_nodes(runtime).unwrap();
    let outer = ui.widgets().button_unobserved("outer").unwrap();
    let inner = ui.widgets().button_unobserved("inner").unwrap();
    let outer_id = outer.as_interface().downgrade();
    let inner_id = inner.as_interface().downgrade();
    let outer_origin = EventOrigin::Widget(outer_id.clone().into_unknown());
    let inner_origin = EventOrigin::Widget(inner_id.clone().into_unknown());
    let host = ui.widgets().host().unwrap();
    let (queue, messages) = neocogi_ui::events::channel();
    let mut state = neocogi_ui::com::widget::WidgetState::new(host.clone(), WidgetOptions::NONE).with_events(Some(queue.clone()));

    // Raw rejection returns the original sole owner when no dispatcher scope exists.
    let event = error(NC_E_INVALID_ARG, "outside scope").unwrap();
    let identity = event.as_raw().object;
    let (status, event) = host.post_event(queue.as_com_ref(), event).unwrap_err();
    assert_eq!(status, NC_E_INVALID_ARG);
    assert_eq!(event.as_raw().object, identity);
    assert_eq!(
        event.query_ref::<IErrorEventAbi>().unwrap().message().unwrap().to_string().unwrap(),
        "outside scope"
    );
    assert!(messages.try_recv().is_err());
    drop(event);

    driver
        .with_origin(outer_origin, || -> Result<(), NcStatus> {
            state.post(error(NC_E_INVALID_ARG, "outer before")?)?;
            driver.with_origin(inner_origin, || state.post(error(neocogi_com::NC_E_NOT_FOUND, "inner α")?))??;
            state.post(error(NcStatus::from_code(-987_654), "outer after")?)
        })
        .unwrap()
        .unwrap();
    // Leaving nested scopes must not manufacture an origin for a later direct call.
    assert_eq!(state.post(error(NC_E_INVALID_ARG, "after scope").unwrap()), Err(NC_E_INVALID_ARG));
    drop((outer, inner));
    assert!(!outer_id.is_alive());
    assert!(!inner_id.is_alive());
    for (expected_source, code, text) in [
        (&outer_id, NC_E_INVALID_ARG.code(), "outer before"),
        (&inner_id, neocogi_com::NC_E_NOT_FOUND.code(), "inner α"),
        (&outer_id, -987_654, "outer after"),
    ] {
        let message = messages.try_recv().unwrap();
        let source = message.origin.widget::<ButtonAbi>().unwrap().unwrap();
        assert!(source.same_object(expected_source));
        assert!(!source.is_alive());
        let report = message.query::<IErrorEventAbi>().unwrap().unwrap();
        assert_eq!(report.status(), Ok(code));
        assert_eq!(report.message().unwrap().to_string().unwrap(), text);
    }
    assert!(messages.try_recv().is_err());
}

/// Reporting failure is an ordinary one-shot transport failure, never a recursive diagnostic.
#[test]
fn rejected_error_report_propagates_status_and_destroys_its_owner_once() {
    let source = crate::test_support::assets::export_bundle(&crate::SkinBundle::from_atlas(crate::test_support::test_atlas())).unwrap();
    let gfx = neocogi_ui_support::test_support::gfx_factory(crate::test_support::host_runtime());
    let (runtime, driver) = crate::runtime::testing::create_runtime(source.as_com_ref(), gfx.as_com_ref()).unwrap();
    let ui = nc_ui_nodes::bind_builtin_nodes(runtime).unwrap();
    let source = ui.widgets().button_unobserved("reporter").unwrap();
    let source_id = source.as_interface().downgrade();
    let (attempts, rejected) = std::sync::mpsc::channel();
    let receiver = RejectSnapshots::from_value(RejectSnapshots { attempts }).query::<IEventReceiverAbi>().unwrap();
    let mut state = neocogi_ui::com::widget::WidgetState::new(ui.widgets().host().unwrap(), WidgetOptions::NONE).with_events(Some(receiver));
    let origin = EventOrigin::Widget(source_id.clone().into_unknown());
    assert_eq!(
        driver
            .with_origin(origin, || state.post(neocogi_ui::events::error(NC_E_INVALID_ARG, "operation failed").unwrap()))
            .unwrap(),
        Err(neocogi_com::NC_E_CLOSED)
    );
    let failed = rejected.try_recv().unwrap();
    assert!(!failed.is_alive());
    assert!(rejected.try_recv().is_err(), "no second report after delivery rejection");
    assert!(source_id.is_alive());
    drop(source);
    assert!(!source_id.is_alive());
}
