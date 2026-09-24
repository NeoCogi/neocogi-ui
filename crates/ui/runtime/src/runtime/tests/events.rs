//! Consumer inboxes use the public facade, real routed input and runtime/window capabilities.
use super::*;
use neocogi_ui_support::test_support::events::TestApp;

/// Both textbox outputs share one callback allocation; construction retains it
/// without allocating another receiver or validating an output through a query.
#[test]
fn constructor_observer_allocates_once_and_drops_with_logical_widget_closure() {
    let ui = nc_ui_nodes::bind_builtin_nodes(runtime()).unwrap();
    let (sender, receiver) = std::sync::mpsc::channel();
    let measurement = crate::test_support::AllocationMeasurement::begin();
    let observer = neocogi_ui::textbox::observer(sender, |event| event).unwrap();
    let allocations = measurement.finish();
    assert_eq!(allocations.events, 1, "sender and mapper must live inside the one COM observer allocation");

    let measurement = crate::test_support::AllocationMeasurement::begin();
    let unobserved = ui.widgets().textbox_unobserved("same").unwrap();
    let absent = measurement.finish();
    let measurement = crate::test_support::AllocationMeasurement::begin();
    let observed = ui.widgets().textbox_with_observer("same", Some(observer.as_com_ref())).unwrap();
    let present = measurement.finish();
    assert_eq!(present, absent, "the factory only retains a present observer; absence needs no dummy object");
    drop(observer);
    assert!(matches!(receiver.try_recv(), Err(std::sync::mpsc::TryRecvError::Empty)));
    observed.node_interface().unwrap().close().unwrap();
    assert!(matches!(receiver.try_recv(), Err(std::sync::mpsc::TryRecvError::Disconnected)));
    // The closed editing handle can outlive its observer without retaining its captures.
    assert_eq!(observed.text(), Err(UI_E_CLOSED));
    drop(unobserved);
}

/// Failed factory validation releases a callback that was never published to a widget.
#[test]
fn failed_constructor_releases_observer_captures() {
    let ui = nc_ui_nodes::bind_builtin_nodes(runtime()).unwrap();
    let (sender, receiver) = std::sync::mpsc::channel::<()>();
    let result = ui.widgets().number(NumberOptions { value: f32::NAN, step: 1.0, precision: 0 }, sender, |_| ());
    assert!(matches!(result, Err(NC_E_INVALID_ARG)));
    assert!(matches!(receiver.try_recv(), Err(std::sync::mpsc::TryRecvError::Disconnected)));
}

/// Queues a concrete key press without fabricating semantic widget events.
fn key(ui: &neocogi_ui::Ui, key: KeyCode) {
    ui.input(
        UiInput::Key(KeyStroke {
            key,
            value: 0,
            pressed: true,
            repeat: false,
            modifiers: 0,
        }),
        "",
    )
    .unwrap();
}
/// Emission order governs the consumer FIFO even when ports were registered in the opposite order.
#[test]
fn consumer_preserves_emission_order_snapshot_values_and_bounded_progress() {
    let runtime = runtime();
    let ui = nc_ui_nodes::bind_builtin_nodes(runtime.clone()).unwrap();
    let (session_tx, session) = std::sync::mpsc::channel();
    let mut a = ui
        .widgets()
        .button("a", session_tx.clone(), move |neocogi_ui::button::Event::Activated(event)| event.label)
        .unwrap();
    let mut b = ui
        .widgets()
        .button("b", session_tx.clone(), move |neocogi_ui::button::Event::Activated(event)| event.label)
        .unwrap();
    let mut column = ui.widgets().linear(FlowDirection::TopToBottom, CrossSize::Stretch).unwrap();
    column.insert(0, &a, LinearEdge::new(LayoutTrack::Content)).unwrap().finish().unwrap();
    column.insert(1, &b, LinearEdge::new(LayoutTrack::Content)).unwrap().finish().unwrap();
    let window = ui.window("order", RECT, &column).unwrap();
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
    assert_eq!(&labels, &["a", "a2"]);
    labels.push(session.try_recv().unwrap());
    assert_eq!(&labels, &["a", "a2", "b"]);
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
    assert_eq!(app.as_interface().dispatch_next(), Ok(false));
}

/// One application protocol combines activation and lifecycle facts without per-event handler types.
enum Mutation {
    /// Replaces the old window through ordinary COM editing capabilities.
    Replace,
    /// Records a lifecycle transition emitted during replacement.
    Window(WindowEvent),
}

/// An owned application may edit the UI after traversal releases its context.
#[test]
fn eventful_runtime_releases_context_and_routes_later_input_to_the_new_tree() {
    let ui = nc_ui_nodes::bind_builtin_nodes(runtime()).unwrap();
    let (renderer, probe) = renderer();
    let (events_tx, events) = std::sync::mpsc::channel();
    let button = ui
        .widgets()
        .button("replace window", events_tx.clone(), move |neocogi_ui::button::Event::Activated(_)| {
            Mutation::Replace
        })
        .unwrap();
    let previous = ui.window("first", RECT, &button).unwrap();

    neocogi_ui::events::bind(neocogi_ui::events::window_events(previous.as_com_ref()).unwrap(), events_tx.clone(), |event| {
        Mutation::Window(event)
    })
    .unwrap();
    // Move every callback dependency into the persistent model; retain separate
    // assertion capabilities only where the test also needs direct observations.
    let app = TestApp::new(
        (ui.clone(), renderer.clone(), previous, None, None, Vec::new()),
        events,
        |(ui, renderer, previous, next, editor, observations), message| {
            match message {
                Mutation::Replace => {
                    assert_eq!(ui.update(SIZE), Err(UI_E_BUSY));
                    assert_eq!(ui.render(SIZE, renderer.as_com_ref()), Err(UI_E_BUSY));
                    previous.close()?;
                    let replacement = ui.widgets().textbox_unobserved("")?;
                    *next = Some(ui.window("replacement", RECT, &replacement)?);
                    *editor = Some(replacement);
                    observations.push("activated");
                }
                Mutation::Window(WindowEvent::Destroyed) => observations.push("destroyed"),
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
    assert_eq!(button.label(), Err(UI_E_CLOSED));
    assert_eq!(app.state().5, ["activated", "destroyed"]);
    assert_eq!(app.state().4.as_ref().unwrap().text().unwrap(), "arrived after replacement");
    ui.render(SIZE, renderer.as_com_ref()).unwrap();
    app.state().3.as_ref().unwrap().close().unwrap();
}

/// Dropping the consumer leaves application state independent and cannot retain the runtime.
#[test]
fn dropping_inbox_and_application_releases_runtime_without_a_source_sink_cycle() {
    let ui = nc_ui_nodes::bind_builtin_nodes(runtime()).unwrap();
    let (events_tx, events) = std::sync::mpsc::channel();
    let button = ui
        .widgets()
        .button("owned runtime", events_tx.clone(), move |neocogi_ui::button::Event::Activated(event)| {
            event.label
        })
        .unwrap();
    let window = ui.window("owner", RECT, &button).unwrap();

    drop(events);
    assert!(button.label().is_ok());
    drop(ui);
    assert_eq!(button.label(), Err(UI_E_CLOSED));
    assert_eq!(window.title().unwrap_err(), UI_E_CLOSED);
}

/// Every scalar and labelled-action source publishes only actual user interaction through the shared transport.
#[test]
fn scalar_and_list_event_capabilities_publish_real_user_changes_and_keep_setters_silent() {
    let ui = nc_ui_nodes::bind_builtin_nodes(runtime()).unwrap();
    let (session_tx, session) = std::sync::mpsc::channel();
    let labels = Vec::new();

    let mut checkbox = ui
        .widgets()
        .checkbox("check", false, session_tx.clone(), move |neocogi_ui::checkbox::Event::Changed(event)| {
            format!("checked:{}", event.checked)
        })
        .unwrap();
    let window = ui.window("checkbox", RECT, &checkbox).unwrap();

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

    let mut slider = ui
        .widgets()
        .slider(
            SliderOptions {
                value: 1.0,
                low: 0.0,
                high: 10.0,
                step: 1.0,
                precision: 0,
            },
            session_tx.clone(),
            move |neocogi_ui::slider::Event::Changed(event)| format!("numeric:{}", event.value),
        )
        .unwrap();
    let window = ui.window("slider", RECT, &slider).unwrap();

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
        .number(
            NumberOptions { value: 3.0, step: 1.0, precision: 0 },
            session_tx.clone(),
            move |neocogi_ui::number::Event::Changed(event)| format!("numeric:{}", event.value),
        )
        .unwrap();
    let window = ui.window("number", RECT, &number).unwrap();

    key(&ui, KeyCode::Tab);
    // Numeric text controls use Up/Down; horizontal sliders use Left/Right.
    key(&ui, KeyCode::ArrowUp);
    ui.update_with_events(SIZE, app.as_interface()).unwrap();
    assert_eq!(app.state().last().unwrap(), "numeric:4");
    window.close().unwrap();

    let item = ui
        .widgets()
        .list_item("item", session_tx.clone(), move |neocogi_ui::list_item::Event::Activated(event)| event.label)
        .unwrap();

    let list_box = ui
        .widgets()
        .list_box("box", session_tx.clone(), move |neocogi_ui::list_box::Event::Activated(event)| event.label)
        .unwrap();
    let mut column = ui.widgets().linear(FlowDirection::TopToBottom, CrossSize::Stretch).unwrap();
    column.insert(0, &item, LinearEdge::new(LayoutTrack::Content)).unwrap().finish().unwrap();
    column.insert(1, &list_box, LinearEdge::new(LayoutTrack::Content)).unwrap().finish().unwrap();
    let window = ui.window("lists", RECT, &column).unwrap();

    for input in [KeyCode::Tab, KeyCode::Space, KeyCode::Tab, KeyCode::Space] {
        key(&ui, input);
    }
    ui.update_with_events(SIZE, app.as_interface()).unwrap();
    assert!(app.state().ends_with(&["item".to_owned(), "box".to_owned()]));
    window.close().unwrap();

    let mut bar = ui
        .widgets()
        .scrollbar(
            ScrollRange {
                axis: ScrollAxis::Horizontal,
                viewport: 20,
                content: 100,
                offset: 0,
            },
            session_tx.clone(),
            move |neocogi_ui::scrollbar::Event::Changed(event)| format!("scroll:{}", event.offset),
        )
        .unwrap();
    let window = ui.window("bar", RECT, &bar).unwrap();

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
    let ui = nc_ui_nodes::bind_builtin_nodes(runtime()).unwrap();
    let (session_tx, session) = std::sync::mpsc::channel();
    let labels = Vec::new();
    let mut textbox = ui
        .widgets()
        .textbox("", session_tx.clone(), |event| match event {
            neocogi_ui::textbox::Event::Changed(value) => format!("changed:{}:{}", value.text, value.cursor),
            neocogi_ui::textbox::Event::Submitted(value) => format!("submitted:{}", value.text),
        })
        .unwrap();
    let window = ui.window("textbox", RECT, &textbox).unwrap();
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
        .text_area("", TextWrapping::Word, true, session_tx.clone(), |event| match event {
            neocogi_ui::text_area::Event::Changed(value) => format!("changed:{}:{}", value.text, value.cursor),
            neocogi_ui::text_area::Event::Submitted(value) => format!("submitted:{}", value.text),
        })
        .unwrap();
    let window = ui.window("textarea", RECT, &area).unwrap();
    key(&ui, KeyCode::Tab);
    ui.input(UiInput::Text, "☃").unwrap();
    ui.input(
        UiInput::Key(KeyStroke {
            key: KeyCode::Enter,
            value: 0,
            pressed: true,
            repeat: false,
            modifiers: 2,
        }),
        "",
    )
    .unwrap();
    ui.update_with_events(SIZE, app.as_interface()).unwrap();
    assert!(app.state().ends_with(&["changed:☃:3".to_owned(), "submitted:☃".to_owned()]));
    let popup_content = ui.widgets().text_block("popup", TextWrapping::None).unwrap();
    let mut popup = ui.popup(window.as_com_ref(), "popup", &popup_content).unwrap();
    neocogi_ui::events::bind(neocogi_ui::events::popup_events(popup.as_com_ref()).unwrap(), session_tx.clone(), |_| {
        "dismissed".to_owned()
    })
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
    let ui = nc_ui_nodes::bind_builtin_nodes(runtime()).unwrap();
    let (events_tx, events) = std::sync::mpsc::channel();
    let button = ui
        .widgets()
        .button("fail once", events_tx.clone(), move |neocogi_ui::button::Event::Activated(_)| ())
        .unwrap();
    let editor = ui.widgets().textbox_unobserved("").unwrap();
    let mut column = ui.widgets().linear(FlowDirection::TopToBottom, CrossSize::Stretch).unwrap();
    column.insert(0, &button, LinearEdge::new(LayoutTrack::Content)).unwrap().finish().unwrap();
    column.insert(1, &editor, LinearEdge::new(LayoutTrack::Content)).unwrap().finish().unwrap();
    let window = ui.window("recovery", RECT, &column).unwrap();

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
    assert_eq!(ui.render(SIZE, renderer.as_com_ref()), Err(UI_E_UPDATE_REQUIRED));
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
    let ui = nc_ui_nodes::bind_builtin_nodes(runtime()).unwrap();
    let button = ui.widgets().button_unobserved("cascade").unwrap();
    let window = ui.window("cascade", RECT, &button).unwrap();
    let (events_tx, events) = std::sync::mpsc::channel();
    neocogi_ui::events::bind(neocogi_ui::events::window_events(window.as_com_ref()).unwrap(), events_tx.clone(), |event| {
        event
    })
    .unwrap();
    window.set_visibility(Visibility::Hidden).unwrap();
    let app = TestApp::new((0, window.clone()), events, |(observed, window), event| {
        if event == WindowEvent::Hidden {
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
    assert_eq!(app.as_interface().dispatch_next(), Ok(false));
    window.close().unwrap();
}

#[com_object(interfaces(IEventOwner))]
/// A foreign consumer fails while the runtime is settling the frame.
struct FailingOwner;
impl IEventOwner for FailingOwner {
    /// This fixture is an application root and cannot attach to a widget.
    fn mount(&mut self, _: ComRef<'_, IWidgetAbi>) -> Result<(), NcStatus> {
        Err(UI_E_OWNER)
    }
    /// The stateless fixture has no terminal outcomes to settle.
    fn retire(&mut self) -> Result<(), NcStatus> {
        Ok(())
    }
    /// Propagates the consumer failure through the actual COM dispatcher boundary.
    fn dispatch_next(&mut self) -> Result<bool, NcStatus> {
        Err(NC_E_INVALID_ARG)
    }
}

/// Failed foreign consumers leave the frame uncommitted and allow an ordinary successful retry.
#[test]
fn runtime_preserves_uncommitted_frame_on_foreign_consumer_failure() {
    let runtime = runtime();
    let ui = nc_ui_nodes::bind_builtin_nodes(runtime.clone()).unwrap();
    let button = ui.widgets().button_unobserved("count").unwrap();
    let window = ui.window("count", RECT, &button).unwrap();
    let dispatcher: ComPtr<IEventOwnerAbi> = FailingOwner::from_value(FailingOwner).query().unwrap();
    assert_eq!(runtime.update_with_events(SIZE, dispatcher.as_com_ref()), Err(NC_E_INVALID_ARG));
    let (renderer, _) = renderer();
    assert_eq!(ui.render(SIZE, renderer.as_com_ref()), Err(UI_E_UPDATE_REQUIRED));
    ui.update(SIZE).unwrap();
    ui.render(SIZE, renderer.as_com_ref()).unwrap();
    window.close().unwrap();
}

/// One shared budget bounds successive application posts without discarding the next fact.
#[test]
fn boundary_budget_stops_a_cascade_and_preserves_the_unattempted_message() {
    let source = crate::test_support::assets::export_bundle(&crate::SkinBundle::from_atlas(crate::test_support::test_atlas())).unwrap();
    let compositor = neocogi_gfx_impl::GfxCompositorObject::create(source.as_com_ref()).unwrap();
    let assets = crate::assets::ImportedAssets::read(source.as_com_ref(), None).unwrap();
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
        let result = runtime.dispatch_boundary(Some(app.as_interface()), Dimensioni::new(SIZE.width, SIZE.height), &mut remaining);
        assert_eq!(result, Err(UI_E_EVENT_LIMIT));
        assert_eq!(remaining, 0);
        assert_eq!(app.state().0, 2);
        app.state().2 = false;
        let mut pending = 0;
        while app.as_interface().dispatch_next().unwrap() {
            pending += 1;
        }
        assert_eq!(pending, if repeat { 2 } else { 0 });
    }
}

/// A generated receiver's domain failure must propagate out of native widget routing.
#[test]
fn failed_post_preserves_committed_widget_state_and_requires_a_new_frame() {
    let ui = nc_ui_nodes::bind_builtin_nodes(runtime()).unwrap();
    let (events_tx, events) = std::sync::mpsc::channel();
    let (renderer, _) = renderer();
    let mut reject_next = true;
    let observer = neocogi_ui::number::Event::observer(move |neocogi_ui::number::Event::Changed(event)| {
        // A receiver may reject one post and recover without changing the
        // widget's construction-time destination or undoing its committed value.
        match std::mem::replace(&mut reject_next, false) {
            true => Err(NC_E_INVALID_ARG),
            false => {
                events_tx.send(event.value).unwrap();
                Ok(())
            }
        }
    })
    .unwrap();
    let number = ui
        .widgets()
        .number_with_observer(NumberOptions { value: 9.0, step: 1.0, precision: 0 }, Some(observer.as_com_ref()))
        .unwrap();
    let window = ui.window("post failure", RECT, &number).unwrap();
    ui.update(SIZE).unwrap();
    key(&ui, KeyCode::Tab);
    key(&ui, KeyCode::ArrowUp);
    assert_eq!(ui.update(SIZE), Err(NC_E_INVALID_ARG));
    assert_eq!(number.value(), Ok(10.0));
    assert!(events.try_recv().is_err());
    assert_eq!(ui.render(SIZE, renderer.as_com_ref()), Err(UI_E_UPDATE_REQUIRED));

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
    /// Foreign consumer rejects this committed lifecycle fact without enqueueing it.
    fn reject(_: WindowEvent) -> Result<(), NcStatus> {
        Err(NC_E_INVALID_ARG)
    }
    let ui = nc_ui_nodes::bind_builtin_nodes(runtime()).unwrap();
    let button = ui.widgets().button_unobserved("lifecycle").unwrap();
    let window = ui.window("post failure", RECT, &button).unwrap();
    let source = window.query::<IWindowEventsAbi>().unwrap();
    let (events_tx, events) = std::sync::mpsc::channel();
    source.changed()(Box::new(reject)).unwrap();
    assert_eq!(window.set_visibility(Visibility::Hidden), Err(NC_E_INVALID_ARG));
    assert_eq!(window.snapshot().unwrap().visibility, Visibility::Hidden);
    assert!(events.try_recv().is_err());
    neocogi_ui::events::bind(source.changed(), events_tx.clone(), |event| event).unwrap();
    window.close().unwrap();
}
