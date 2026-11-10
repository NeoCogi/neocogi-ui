//! Consumer inboxes use the public facade, real routed input and runtime/window capabilities.
use super::*;
use neocogi_ui::support::test_support::events::TestApp;

/// Both textbox outputs share one queue allocation; construction retains it
/// without allocating another receiver or validating an output through a query.
#[test]
fn constructor_queue_allocates_once_and_drops_with_logical_widget_closure() {
    let ui = neocogi_ui::nodes::bind_builtin_nodes(runtime()).unwrap();
    let (sender, receiver) = std::sync::mpsc::channel();
    let measurement = crate::runtime::test_support::AllocationMeasurement::begin();
    let queue = neocogi_ui::test_event_queue!(neocogi_ui::TextboxAbi, (), sender, |_queue, _origin, _event| ()).unwrap();
    let allocations = measurement.finish();
    assert_eq!(allocations.events, 1, "sender must live inside the one concrete COM queue allocation");

    let measurement = crate::runtime::test_support::AllocationMeasurement::begin();
    let unobserved = ui.widgets().textbox_unobserved("same").unwrap();
    let absent = measurement.finish();
    let measurement = crate::runtime::test_support::AllocationMeasurement::begin();
    let observed = ui.widgets().textbox_with_queue("same", Some(queue.as_com_ref())).unwrap();
    let present = measurement.finish();
    assert_eq!(present, absent, "the factory only retains a present queue; absence needs no dummy object");
    drop(queue);
    assert!(matches!(receiver.try_recv(), Err(std::sync::mpsc::TryRecvError::Empty)));
    let handle = observed.handle();
    drop(observed);
    let observed = handle;
    assert!(matches!(receiver.try_recv(), Err(std::sync::mpsc::TryRecvError::Disconnected)));
    // The closed editing handle can outlive its queue without retaining its captures.
    assert_eq!(observed.text(), Err(neocogi_com::NC_E_DROPPED));
    drop(unobserved);
}

/// Failed factory validation releases a queue that was never published to a widget.
#[test]
fn failed_constructor_releases_its_queue() {
    let ui = neocogi_ui::nodes::bind_builtin_nodes(runtime()).unwrap();
    let (sender, receiver) = std::sync::mpsc::channel::<()>();
    let result = ui.widgets().number_with_queue(
        NumberOptions { value: f32::NAN, step: 1.0, precision: 0 },
        Some(
            neocogi_ui::test_event_queue!(neocogi_ui::NumberAbi, (), sender, |_queue, _origin, _| ())
                .unwrap()
                .as_com_ref(),
        ),
    );
    assert!(matches!(result, Err(NC_E_INVALID_ARG)));
    assert!(matches!(receiver.try_recv(), Err(std::sync::mpsc::TryRecvError::Disconnected)));
}

/// Queues a concrete key press without fabricating semantic widget events.
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
/// Emission order governs the consumer FIFO even when ports were registered in the opposite order.
#[test]
fn consumer_preserves_source_commands_and_bounded_progress_after_label_edits() {
    let runtime = runtime();
    let ui = neocogi_ui::nodes::bind_builtin_nodes(runtime.clone()).unwrap();
    let (session_tx, session) = std::sync::mpsc::channel();
    let a_owner = ui
        .widgets()
        .button_with_queue(
            "a",
            Some(
                neocogi_ui::test_event_queue!(neocogi_ui::ButtonAbi, String, session_tx.clone(), |_queue, _origin, event| {
                    event.query_ref::<neocogi_ui::IActivatedEventAbi>()?;
                    "a".to_owned()
                })
                .unwrap()
                .as_com_ref(),
            ),
        )
        .unwrap();
    let mut a = a_owner.handle();
    let b_owner = ui
        .widgets()
        .button_with_queue(
            "b",
            Some(
                neocogi_ui::test_event_queue!(neocogi_ui::ButtonAbi, String, session_tx.clone(), |_queue, _origin, event| {
                    event.query_ref::<neocogi_ui::IActivatedEventAbi>()?;
                    "b".to_owned()
                })
                .unwrap()
                .as_com_ref(),
            ),
        )
        .unwrap();
    let mut b = b_owner.handle();
    let mut column = ui.widgets().linear(FlowDirection::TopToBottom, CrossSize::Stretch).unwrap();
    column.insert(0, a_owner, LinearEdge::new(LayoutTrack::Content)).unwrap();
    column.insert(1, b_owner, LinearEdge::new(LayoutTrack::Content)).unwrap();
    let window = ui.window("order", RECT, column).unwrap();
    let mut labels = Vec::new();

    key(&ui, KeyCode::Tab);
    key(&ui, KeyCode::Space);
    ui.update(SIZE).unwrap();
    a.set_label("a2").unwrap();
    key(&ui, KeyCode::Space);
    ui.update(SIZE).unwrap();
    key(&ui, KeyCode::Tab);
    key(&ui, KeyCode::Space);
    ui.update(SIZE).unwrap();
    a.set_label("later a").unwrap();
    b.set_label("later b").unwrap();
    labels.extend(session.try_iter().take(2));
    assert_eq!(&labels, &["a", "a"]);
    labels.push(session.try_recv().unwrap());
    assert_eq!(&labels, &["a", "a", "b"]);
    assert!(session.try_recv().is_err());
    // Silent setters create no semantic event and do not make the next sweep deliver anything.
    a.set_label("silent").unwrap();
    let app = TestApp::new(labels, session, |state, message| {
        state.push(message);
        Ok(())
    });
    ui.update_with_events(SIZE, app.as_interface()).unwrap();
    assert_eq!(app.state().len(), 3);
    let (renderer, _) = renderer();
    ui.render(SIZE, renderer.as_com_ref()).unwrap();
    window.close().unwrap();
    assert_eq!(app.as_interface().dispatch_next(), Ok(ApplicationDispatch::Idle));
}

/// One application protocol combines activation and lifecycle facts without per-event handler types.
enum Mutation {
    /// Replaces the old window through ordinary COM editing capabilities.
    Replace,
    /// Records a lifecycle transition emitted during replacement.
    Window(bool),
}

/// An owned application may edit the UI after traversal releases its context.
#[test]
fn eventful_runtime_releases_context_and_routes_later_input_to_the_new_tree() {
    let ui = neocogi_ui::nodes::bind_builtin_nodes(runtime()).unwrap();
    let (renderer, probe) = renderer();
    let (events_tx, events) = std::sync::mpsc::channel();
    let button_owner = ui
        .widgets()
        .button_with_queue(
            "replace window",
            Some(
                neocogi_ui::test_event_queue!(neocogi_ui::ButtonAbi, Mutation, events_tx.clone(), |_queue, _origin, _| { Mutation::Replace })
                    .unwrap()
                    .as_com_ref(),
            ),
        )
        .unwrap();
    let button = button_owner.handle();

    let queue = neocogi_ui::test_event_queue!(neocogi_ui::IWindowAbi, Mutation, events_tx.clone(), |_queue, _origin, event| {
        Mutation::Window(match event.query_weak::<IWindowDestroyedEventAbi>() {
            Ok(_) => true,
            Err(neocogi_com::NC_E_NOINTERFACE) => false,
            Err(status) => return Err(status),
        })
    })
    .unwrap();
    let previous = ui.window_with_queue("first", RECT, button_owner, Some(queue.as_com_ref())).unwrap();
    // Move every callback dependency into the persistent model; retain separate
    // assertion capabilities only where the test also needs direct observations.
    let app = TestApp::new(
        (ui.clone(), renderer.clone(), previous, None, None, Vec::new()),
        events,
        |(ui, renderer, previous, next, editor, observations), message| {
            match message {
                Mutation::Replace => {
                    assert_eq!(ui.update(SIZE), Err(UI_E_BUSY));
                    assert_eq!(ui.render(SIZE, renderer.as_com_ref()).map_err(|error| error.status), Err(UI_E_BUSY));
                    previous.close()?;
                    let replacement_owner = ui.widgets().textbox_unobserved("")?;
                    let replacement = replacement_owner.handle();
                    *next = Some(ui.window("replacement", RECT, replacement_owner).map_err(|(status, _candidate)| status)?);
                    *editor = Some(replacement);
                    observations.push("activated");
                }
                Mutation::Window(true) => observations.push("destroyed"),
                Mutation::Window(_) => {}
            }
            Ok(())
        },
    );
    for input in [KeyCode::Tab, KeyCode::Space, KeyCode::Tab] {
        key(&ui, input);
    }
    ui.input(UiInput::Text, "arrived after replacement").unwrap();
    app.update(&ui, SIZE).unwrap();
    assert_eq!(probe.calls.get(), 0);
    assert_eq!(button.label(), Err(neocogi_com::NC_E_DROPPED));
    assert_eq!(app.state().5, ["activated", "destroyed"]);
    assert_eq!(app.state().4.as_ref().unwrap().text().unwrap(), "arrived after replacement");
    ui.render(SIZE, renderer.as_com_ref()).unwrap();
    app.state().3.as_ref().unwrap().close().unwrap();
}

/// Dropping the consumer leaves application state independent and cannot retain the runtime.
#[test]
fn dropping_inbox_and_application_releases_runtime_without_a_source_sink_cycle() {
    let ui = neocogi_ui::nodes::bind_builtin_nodes(runtime()).unwrap();
    let (events_tx, events) = std::sync::mpsc::channel();
    let button_owner = ui
        .widgets()
        .button_with_queue(
            "owned runtime",
            Some(
                neocogi_ui::test_event_queue!(neocogi_ui::ButtonAbi, String, events_tx.clone(), |_queue, _origin, event| {
                    {
                        event.query_ref::<neocogi_ui::IActivatedEventAbi>()?;
                        "owned runtime".to_owned()
                    }
                })
                .unwrap()
                .as_com_ref(),
            ),
        )
        .unwrap();
    let button = button_owner.handle();
    let window = ui.window("owner", RECT, button_owner).unwrap();

    drop(events);
    assert!(button.label().is_ok());
    drop(ui);
    assert_eq!(button.label(), Err(neocogi_com::NC_E_DROPPED));
    assert_eq!(window.title().unwrap_err(), neocogi_com::NC_E_DROPPED);
}

/// Every scalar and labelled-action source publishes only actual user interaction through the shared transport.
#[test]
fn scalar_and_list_event_capabilities_publish_real_user_changes_and_keep_setters_silent() {
    let ui = neocogi_ui::nodes::bind_builtin_nodes(runtime()).unwrap();
    let (session_tx, session) = std::sync::mpsc::channel();
    let labels = Vec::new();

    let checkbox_owner = ui
        .widgets()
        .checkbox_with_queue(
            "check",
            false,
            Some(
                neocogi_ui::test_event_queue!(neocogi_ui::CheckboxAbi, String, session_tx.clone(), |_queue, _origin, event| {
                    format!("checked:{}", event.query_weak::<ICheckboxChangedEventAbi>()?.checked()?)
                })
                .unwrap()
                .as_com_ref(),
            ),
        )
        .unwrap();
    let mut checkbox = checkbox_owner.handle();
    let window = ui.window("checkbox", RECT, checkbox_owner).unwrap();

    checkbox.set_checked(true).unwrap();
    let app = TestApp::new(labels, session, |state, message| {
        state.push(message);
        Ok(())
    });
    ui.update_with_events(SIZE, app.as_interface()).unwrap();
    assert!(app.state().is_empty());
    key(&ui, KeyCode::Tab);
    key(&ui, KeyCode::Space);
    ui.update_with_events(SIZE, app.as_interface()).unwrap();
    assert_eq!(&*app.state(), &["checked:false"]);
    window.close().unwrap();

    let slider_owner = ui
        .widgets()
        .slider_with_queue(
            SliderOptions {
                value: 1.0,
                low: 0.0,
                high: 10.0,
                step: 1.0,
                precision: 0,
            },
            Some(
                neocogi_ui::test_event_queue!(neocogi_ui::SliderAbi, String, session_tx.clone(), |_queue, _origin, event| format!(
                    "numeric:{}",
                    event.query_weak::<INumericChangedEventAbi>()?.value()?
                ))
                .unwrap()
                .as_com_ref(),
            ),
        )
        .unwrap();
    let mut slider = slider_owner.handle();
    let window = ui.window("slider", RECT, slider_owner).unwrap();

    slider.set_value(5.0).unwrap();
    ui.update_with_events(SIZE, app.as_interface()).unwrap();
    assert_eq!(app.state().len(), 1);
    key(&ui, KeyCode::Tab);
    key(&ui, KeyCode::ArrowRight);
    ui.update_with_events(SIZE, app.as_interface()).unwrap();
    assert_eq!(app.state().last().unwrap(), "numeric:6");
    window.close().unwrap();

    let number = ui
        .widgets()
        .number_with_queue(
            NumberOptions { value: 3.0, step: 1.0, precision: 0 },
            Some(
                neocogi_ui::test_event_queue!(neocogi_ui::NumberAbi, String, session_tx.clone(), |_queue, _origin, event| format!(
                    "numeric:{}",
                    event.query_weak::<INumericChangedEventAbi>()?.value()?
                ))
                .unwrap()
                .as_com_ref(),
            ),
        )
        .unwrap();
    let window = ui.window("number", RECT, number).unwrap();

    key(&ui, KeyCode::Tab);
    // Numeric text controls use Up/Down; horizontal sliders use Left/Right.
    key(&ui, KeyCode::ArrowUp);
    ui.update_with_events(SIZE, app.as_interface()).unwrap();
    assert_eq!(app.state().last().unwrap(), "numeric:4");
    window.close().unwrap();

    let item = ui
        .widgets()
        .list_item_with_queue(
            "item",
            Some(
                neocogi_ui::test_event_queue!(neocogi_ui::ListItemAbi, String, session_tx.clone(), |_queue, _origin, event| {
                    event.query_ref::<neocogi_ui::IActivatedEventAbi>()?;
                    "item".to_owned()
                })
                .unwrap()
                .as_com_ref(),
            ),
        )
        .unwrap();

    let list_box = ui
        .widgets()
        .list_box_with_queue(
            "box",
            Some(
                neocogi_ui::test_event_queue!(neocogi_ui::ListBoxAbi, String, session_tx.clone(), |_queue, _origin, event| {
                    event.query_ref::<neocogi_ui::IActivatedEventAbi>()?;
                    "box".to_owned()
                })
                .unwrap()
                .as_com_ref(),
            ),
        )
        .unwrap();
    let mut column = ui.widgets().linear(FlowDirection::TopToBottom, CrossSize::Stretch).unwrap();
    column.insert(0, item, LinearEdge::new(LayoutTrack::Content)).unwrap();
    column.insert(1, list_box, LinearEdge::new(LayoutTrack::Content)).unwrap();
    let window = ui.window("lists", RECT, column).unwrap();

    for input in [KeyCode::Tab, KeyCode::Space, KeyCode::Tab, KeyCode::Space] {
        key(&ui, input);
    }
    ui.update_with_events(SIZE, app.as_interface()).unwrap();
    assert!(app.state().ends_with(&["item".to_owned(), "box".to_owned()]));
    window.close().unwrap();

    let bar_owner = ui
        .widgets()
        .scrollbar_with_queue(
            ScrollRange {
                axis: ScrollAxis::Horizontal,
                viewport: 20,
                content: 100,
                offset: 0,
            },
            Some(
                neocogi_ui::test_event_queue!(neocogi_ui::ScrollbarAbi, String, session_tx.clone(), |_queue, _origin, event| format!(
                    "scroll:{}",
                    event.query_weak::<IScrollbarChangedEventAbi>()?.offset()?
                ))
                .unwrap()
                .as_com_ref(),
            ),
        )
        .unwrap();
    let mut bar = bar_owner.handle();
    let window = ui.window("bar", RECT, bar_owner).unwrap();

    let previous = app.state().len();
    bar.set_offset(0).unwrap();
    ui.update_with_events(SIZE, app.as_interface()).unwrap();
    assert_eq!(app.state().len(), previous);
    let position = Point { x: 300, y: 100 };
    for event in [
        UiInput::Pointer(position),
        UiInput::Down { position, buttons: 1 },
        UiInput::Up { position, buttons: 1 },
    ] {
        ui.input(event, "").unwrap();
    }
    ui.update_with_events(SIZE, app.as_interface()).unwrap();
    assert!(bar.range().unwrap().offset > 0);
    assert_eq!(app.state().last().unwrap(), &format!("scroll:{}", bar.range().unwrap().offset));
    window.close().unwrap();
}

/// Each editor exposes one typed enum-valued port, and popup dismissal uses its real lifecycle source.
#[test]
fn text_editor_and_popup_event_capabilities_preserve_their_independent_native_sources() {
    let ui = neocogi_ui::nodes::bind_builtin_nodes(runtime()).unwrap();
    let (session_tx, session) = std::sync::mpsc::channel();
    let labels = Vec::new();
    let textbox_owner = ui
        .widgets()
        .textbox_with_queue(
            "",
            Some(
                neocogi_ui::test_event_queue!(neocogi_ui::TextboxAbi, String, session_tx.clone(), |_queue, _origin, event| {
                    match event.query_weak::<ITextChangedEventAbi>() {
                        Ok(value) => format!("changed:{}:{}", value.text()?.to_string()?, value.cursor()?),
                        Err(neocogi_com::NC_E_NOINTERFACE) => {
                            format!("submitted:{}", event.query_weak::<ITextSubmittedEventAbi>()?.text()?.to_string()?)
                        }
                        Err(status) => return Err(status),
                    }
                })
                .unwrap()
                .as_com_ref(),
            ),
        )
        .unwrap();
    let mut textbox = textbox_owner.handle();
    let window = ui.window("textbox", RECT, textbox_owner).unwrap();
    textbox.set_text("a").unwrap();
    let app = TestApp::new(labels, session, |state, message| {
        state.push(message);
        Ok(())
    });
    ui.update_with_events(SIZE, app.as_interface()).unwrap();
    assert!(app.state().is_empty());
    key(&ui, KeyCode::Tab);
    ui.input(UiInput::Text, "é").unwrap();
    key(&ui, KeyCode::Enter);
    ui.update_with_events(SIZE, app.as_interface()).unwrap();
    assert_eq!(&*app.state(), &["changed:aé:3", "submitted:aé"]);
    window.close().unwrap();

    let area = ui
        .widgets()
        .text_area_with_queue(
            "",
            TextWrapping::Word,
            true,
            Some(
                neocogi_ui::test_event_queue!(neocogi_ui::TextAreaAbi, String, session_tx.clone(), |_queue, _origin, event| {
                    match event.query_weak::<ITextChangedEventAbi>() {
                        Ok(value) => format!("changed:{}:{}", value.text()?.to_string()?, value.cursor()?),
                        Err(neocogi_com::NC_E_NOINTERFACE) => {
                            format!("submitted:{}", event.query_weak::<ITextSubmittedEventAbi>()?.text()?.to_string()?)
                        }
                        Err(status) => return Err(status),
                    }
                })
                .unwrap()
                .as_com_ref(),
            ),
        )
        .unwrap();
    let window = ui.window("textarea", RECT, area).unwrap();
    key(&ui, KeyCode::Tab);
    ui.input(UiInput::Text, "☃").unwrap();
    ui.input(
        UiInput::Key(KeyStroke {
            key: KeyCode::Enter,
            action: neocogi_ui::KeyAction::Pressed { repeat: false },
            modifiers: 2,
        }),
        "",
    )
    .unwrap();
    ui.update_with_events(SIZE, app.as_interface()).unwrap();
    assert!(app.state().ends_with(&["changed:☃:3".to_owned(), "submitted:☃".to_owned()]));
    let popup_content = ui.widgets().text_block("popup", TextWrapping::None).unwrap();
    let queue = neocogi_ui::test_event_queue!(neocogi_ui::IPopupAbi, String, session_tx.clone(), |_queue, _origin, _| "dismissed".to_owned()).unwrap();
    let mut popup = ui
        .popup_with_queue(window.as_com_ref(), "popup", popup_content, Some(queue.as_com_ref()))
        .unwrap();
    popup.show_at(Rect { x: 100, y: 100, width: 20, height: 20 }).unwrap();
    ui.update_with_events(SIZE, app.as_interface()).unwrap();
    popup.hide().unwrap();
    ui.update_with_events(SIZE, app.as_interface()).unwrap();
    assert_eq!(app.state().last().unwrap(), "dismissed");
    window.close().unwrap();
    ui.update_with_events(SIZE, app.as_interface()).unwrap();
}

/// A failed consumer invalidates the frame without replaying the attempt or losing later raw input.
#[test]
fn failed_consumer_recovers_on_next_update_and_preserves_remaining_native_input() {
    let ui = neocogi_ui::nodes::bind_builtin_nodes(runtime()).unwrap();
    let (events_tx, events) = std::sync::mpsc::channel();
    let button = ui
        .widgets()
        .button_with_queue(
            "fail once",
            Some(
                neocogi_ui::test_event_queue!(neocogi_ui::ButtonAbi, (), events_tx.clone(), |_queue, _origin, _| ())
                    .unwrap()
                    .as_com_ref(),
            ),
        )
        .unwrap();
    let editor = ui.widgets().textbox_unobserved("").unwrap();
    let mut column = ui.widgets().linear(FlowDirection::TopToBottom, CrossSize::Stretch).unwrap();
    column.insert(0, button, LinearEdge::new(LayoutTrack::Content)).unwrap();
    let editor_owner = editor;
    let editor = editor_owner.handle();
    column.insert(1, editor_owner, LinearEdge::new(LayoutTrack::Content)).unwrap();
    let window = ui.window("recovery", RECT, column).unwrap();

    let app = TestApp::new(0, events, |attempts, ()| {
        *attempts += 1;
        match *attempts {
            1 => Err(NC_E_INVALID_ARG),
            _ => Ok(()),
        }
    });
    for input in [KeyCode::Tab, KeyCode::Space, KeyCode::Tab] {
        key(&ui, input);
    }
    ui.input(UiInput::Text, "still queued").unwrap();
    assert_eq!(app.update(&ui, SIZE), Err(NC_E_INVALID_ARG));
    assert_eq!(*app.state(), 1);
    assert_eq!(editor.text().unwrap(), "");
    let (renderer, _) = renderer();
    assert_eq!(ui.render(SIZE, renderer.as_com_ref()).map_err(|error| error.status), Err(UI_E_UPDATE_REQUIRED));
    app.update(&ui, SIZE).unwrap();
    assert_eq!(*app.state(), 1);
    assert_eq!(editor.text().unwrap(), "still queued");
    key(&ui, KeyCode::Tab);
    key(&ui, KeyCode::Space);
    app.update(&ui, SIZE).unwrap();
    assert_eq!(*app.state(), 2);
    ui.render(SIZE, renderer.as_com_ref()).unwrap();
    window.close().unwrap();
}

/// Newly posted lifecycle messages remain FIFO and settle in the same runtime update.
#[test]
fn real_lifecycle_cascade_is_deferred_until_each_consumer_returns() {
    let ui = neocogi_ui::nodes::bind_builtin_nodes(runtime()).unwrap();
    let button = ui.widgets().button_unobserved("cascade").unwrap();
    let (queue, events) = neocogi_ui::events::channel();
    let window = ui.window_with_queue("cascade", RECT, button, Some(queue.as_com_ref())).unwrap();
    window.set_visibility(Visibility::Hidden).unwrap();
    let app = TestApp::new((0, window.clone()), events, |(observed, window), event| {
        if event.query::<IWindowHiddenEventAbi>()?.is_some() {
            *observed += 1;
            if *observed < 3 {
                window.set_visibility(Visibility::Visible)?;
                window.set_visibility(Visibility::Hidden)?;
            }
        }
        Ok(())
    });
    app.update(&ui, SIZE).unwrap();
    assert_eq!(app.state().0, 3);
    assert_eq!(app.as_interface().dispatch_next(), Ok(ApplicationDispatch::Idle));
    window.close().unwrap();
}

#[com_object(interfaces(IApplicationDispatcher))]
/// A foreign consumer fails while the runtime is settling the frame.
struct FailingOwner;
impl IApplicationDispatcher for FailingOwner {
    /// This fixture fails dispatch rather than input bookkeeping.
    fn input_boundary(&mut self, _: InputBoundary) -> Result<(), NcStatus> {
        Ok(())
    }
    /// Propagates the consumer failure through the actual COM dispatcher boundary.
    fn dispatch_next(&mut self) -> Result<ApplicationDispatch, NcStatus> {
        Err(NC_E_INVALID_ARG)
    }
}

/// Failed foreign consumers leave the frame uncommitted and allow an ordinary successful retry.
#[test]
fn runtime_preserves_uncommitted_frame_on_foreign_consumer_failure() {
    let runtime = runtime();
    let ui = neocogi_ui::nodes::bind_builtin_nodes(runtime.clone()).unwrap();
    let button = ui.widgets().button_unobserved("count").unwrap();
    let window = ui.window("count", RECT, button).unwrap();
    let dispatcher: ComPtr<IApplicationDispatcherAbi> = FailingOwner::from_value(FailingOwner).query().unwrap();
    assert_eq!(runtime.update_with_events(SIZE, dispatcher.as_com_ref()), Err(NC_E_INVALID_ARG));
    let (renderer, _) = renderer();
    assert_eq!(ui.render(SIZE, renderer.as_com_ref()).map_err(|error| error.status), Err(UI_E_UPDATE_REQUIRED));
    ui.update(SIZE).unwrap();
    ui.render(SIZE, renderer.as_com_ref()).unwrap();
    window.close().unwrap();
}

/// One shared budget bounds successive application posts without discarding the next fact.
#[test]
fn boundary_budget_stops_a_cascade_and_preserves_the_unattempted_message() {
    let source =
        crate::runtime::test_support::assets::export_bundle(&crate::runtime::SkinBundle::from_atlas(crate::runtime::test_support::test_atlas())).unwrap();
    let compositor = neocogi_ui::gfx::GfxCompositorObject::create(source.as_com_ref()).unwrap();
    let assets = crate::runtime::assets::ImportedAssets::read(source.as_com_ref(), None).unwrap();
    let runtime = Runtime {
        storage: create_storage(assets, compositor),
    };
    // Both a continuing cascade and an exactly empty channel stop at the ceiling.
    // No extra dequeue may be used to distinguish these cases.
    for repeat in [false, true] {
        let (events_tx, events) = std::sync::mpsc::channel();
        let sender = events_tx.clone();
        sender.send(()).unwrap();
        sender.send(()).unwrap();
        let app = TestApp::new((0, sender, repeat), events, |(delivered, sender, repeat), ()| {
            *delivered += 1;
            if *repeat {
                sender.send(()).unwrap();
            }
            Ok(())
        });
        let mut remaining = 2;
        let mut widgets = DispatchWidgets::default();
        let result = runtime.dispatch_boundary(Some(app.as_interface()), Dimensioni::new(SIZE.width, SIZE.height), &mut remaining, &mut widgets);
        assert_eq!(result, Err(UI_E_EVENT_LIMIT));
        assert_eq!(remaining, 0);
        assert_eq!(app.state().0, 2);
        app.state().2 = false;
        let mut pending = 0;
        while app.as_interface().dispatch_next().unwrap() == ApplicationDispatch::MessageHandled {
            pending += 1;
        }
        assert_eq!(pending, if repeat { 2 } else { 0 });
    }
}

/// A generated receiver's domain failure must propagate out of native widget routing.
#[test]
fn failed_post_preserves_committed_widget_state_and_requires_a_new_frame() {
    let ui = neocogi_ui::nodes::bind_builtin_nodes(runtime()).unwrap();
    let (events_tx, events) = std::sync::mpsc::channel();
    let (renderer, _) = renderer();
    let queue = neocogi_ui::test_event_queue!(neocogi_ui::NumberAbi, f32, events_tx,
        |queue, _origin, event| {
            // A queue may reject one post without rolling back the producer's edit.
            match std::mem::replace(&mut queue.reject_next, false) {
                true => return Err(NC_E_INVALID_ARG),
                false => event.query_weak::<INumericChangedEventAbi>()?.value()?,
            }
        }, { reject_next: bool = true })
    .unwrap();
    let number = ui
        .widgets()
        .number_with_queue(NumberOptions { value: 9.0, step: 1.0, precision: 0 }, Some(queue.as_com_ref()))
        .unwrap();
    let number_owner = number;
    let number = number_owner.handle();
    let window = ui.window("post failure", RECT, number_owner).unwrap();
    ui.update(SIZE).unwrap();
    key(&ui, KeyCode::Tab);
    key(&ui, KeyCode::ArrowUp);
    assert_eq!(ui.update(SIZE), Err(NC_E_INVALID_ARG));
    assert_eq!(number.value(), Ok(10.0));
    assert!(events.try_recv().is_err());
    assert_eq!(ui.render(SIZE, renderer.as_com_ref()).map_err(|error| error.status), Err(UI_E_UPDATE_REQUIRED));

    ui.update(SIZE).unwrap();
    ui.render(SIZE, renderer.as_com_ref()).unwrap();
    key(&ui, KeyCode::ArrowUp);
    ui.update(SIZE).unwrap();
    assert_eq!(events.try_recv().unwrap(), 11.0);
    window.close().unwrap();
}

/// Window mutations report posting errors after committing their complete lifecycle transition.
#[test]
fn failed_lifecycle_post_preserves_visibility_and_returns_its_receiver_error() {
    #[neocogi_com::com_object(interfaces(neocogi_ui::events::IEventReceiver))]
    /// Concrete foreign queue rejects each fully decoded lifecycle event.
    struct Reject;
    impl neocogi_ui::events::IEventReceiver for Reject {
        /// Valid decoding does not oblige an application receiver to accept a post.
        fn post(
            &mut self,
            _: neocogi_com::NcValueRef<'_, neocogi_ui::events::EventOrigin>,
            event_owner: neocogi_com::ComBox<neocogi_ui::events::IEventAbi>,
        ) -> Result<(), (NcStatus, neocogi_com::ComBox<neocogi_ui::events::IEventAbi>)> {
            let result = (|| -> Result<(), NcStatus> {
                event_owner.query_weak::<IWindowEventAbi>()?;
                Err(NC_E_INVALID_ARG)
            })();
            result.map_err(|status| (status, event_owner))
        }
    }
    let ui = neocogi_ui::nodes::bind_builtin_nodes(runtime()).unwrap();
    let button = ui.widgets().button_unobserved("lifecycle").unwrap();
    let queue = Reject::from_value(Reject).query::<neocogi_ui::events::IEventReceiverAbi>().unwrap();
    let window = ui.window_with_queue("post failure", RECT, button, Some(queue.as_com_ref())).unwrap();
    assert_eq!(window.set_visibility(Visibility::Hidden), Err(NC_E_INVALID_ARG));
    assert_eq!(window.snapshot().unwrap().visibility, Visibility::Hidden);

    assert_eq!(window.close(), Err(NC_E_INVALID_ARG));
}
