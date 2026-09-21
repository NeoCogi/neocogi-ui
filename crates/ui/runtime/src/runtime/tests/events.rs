//! Consumer inboxes use the public facade, real routed input and runtime/window capabilities.
use super::*;

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
    let mut a = ui.widgets().button("a").unwrap();
    let mut b = ui.widgets().button("b").unwrap();
    let mut column = ui.widgets().linear(FlowDirection::TopToBottom, CrossSize::Stretch).unwrap();
    column.insert(0, &a, LinearEdge::new(LayoutTrack::Content)).unwrap().finish().unwrap();
    column.insert(1, &b, LinearEdge::new(LayoutTrack::Content)).unwrap().finish().unwrap();
    let window = ui.window("order", RECT, &column).unwrap();
    let (session_tx, session) = std::sync::mpsc::channel();
    let mut labels = Vec::new();
    neocogi_ui::events::map(b.activated(), session_tx.clone(), |event| event.label).unwrap();
    neocogi_ui::events::map(a.activated(), session_tx.clone(), |event| event.label).unwrap();
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
    ui.update_with_events(SIZE, &session, |message| {
        labels.push(message);
        Ok(())
    })
    .unwrap();
    assert_eq!(labels.len(), 3);
    let (renderer, _) = renderer();
    ui.render(SIZE, renderer.as_com_ref()).unwrap();
    window.close().unwrap();
    assert!(session.try_recv().is_err());
}

/// One application protocol combines activation and lifecycle facts without per-event handler types.
enum Mutation {
    /// Replaces the old window through ordinary COM editing capabilities.
    Replace,
    /// Records a lifecycle transition emitted during replacement.
    Window(WindowEvent),
}

/// Borrowed stack state may edit the UI after traversal releases its context.
#[test]
fn eventful_runtime_releases_context_and_routes_later_input_to_the_new_tree() {
    let ui = nc_ui_nodes::bind_builtin_nodes(runtime()).unwrap();
    let button = ui.widgets().button("replace window").unwrap();
    let previous = ui.window("first", RECT, &button).unwrap();
    let (renderer, probe) = renderer();
    let (events_tx, events) = std::sync::mpsc::channel();
    neocogi_ui::events::map(button.activated(), events_tx.clone(), |_| Mutation::Replace).unwrap();
    neocogi_ui::events::map(neocogi_ui::events::window_events(previous.as_com_ref()).unwrap(), events_tx.clone(), |event| {
        Mutation::Window(event)
    })
    .unwrap();
    let mut next = None;
    let mut editor = None;
    let mut observations = Vec::new();
    for input in [KeyCode::Tab, KeyCode::Space, KeyCode::Tab] {
        key(&ui, input);
    }
    ui.input(UiInput::Text, "arrived after replacement").unwrap();
    ui.update_with_events(SIZE, &events, |message| {
        match message {
            Mutation::Replace => {
                assert_eq!(ui.update(SIZE), Err(UI_E_BUSY));
                assert_eq!(ui.render(SIZE, renderer.as_com_ref()), Err(UI_E_BUSY));
                previous.close()?;
                let replacement = ui.widgets().textbox("")?;
                next = Some(ui.window("replacement", RECT, &replacement)?);
                editor = Some(replacement);
                observations.push("activated");
            }
            Mutation::Window(WindowEvent::Destroyed) => observations.push("destroyed"),
            Mutation::Window(_) => {}
        }
        Ok(())
    })
    .unwrap();
    assert_eq!(probe.calls.get(), 0);
    assert_eq!(button.label(), Err(UI_E_CLOSED));
    assert_eq!(observations, ["activated", "destroyed"]);
    assert_eq!(editor.unwrap().text().unwrap(), "arrived after replacement");
    ui.render(SIZE, renderer.as_com_ref()).unwrap();
    next.unwrap().close().unwrap();
}

/// Dropping the consumer leaves application state independent and cannot retain the runtime.
#[test]
fn dropping_inbox_and_application_releases_runtime_without_a_source_sink_cycle() {
    let ui = nc_ui_nodes::bind_builtin_nodes(runtime()).unwrap();
    let button = ui.widgets().button("owned runtime").unwrap();
    let window = ui.window("owner", RECT, &button).unwrap();
    let (events_tx, events) = std::sync::mpsc::channel();
    neocogi_ui::events::map(button.activated(), events_tx.clone(), |event| event.label).unwrap();
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
    let mut labels = Vec::new();
    let mut checkbox = ui.widgets().checkbox("check", false).unwrap();
    let window = ui.window("checkbox", RECT, &checkbox).unwrap();
    neocogi_ui::events::map(checkbox.changed(), session_tx.clone(), |event| format!("checked:{}", event.checked)).unwrap();
    checkbox.set_checked(true).unwrap();
    ui.update_with_events(SIZE, &session, |message| {
        labels.push(message);
        Ok(())
    })
    .unwrap();
    assert!(labels.is_empty());
    key(&ui, KeyCode::Tab);
    key(&ui, KeyCode::Space);
    ui.update_with_events(SIZE, &session, |message| {
        labels.push(message);
        Ok(())
    })
    .unwrap();
    assert_eq!(&labels, &["checked:false"]);
    window.close().unwrap();

    let mut slider = ui
        .widgets()
        .slider(SliderOptions {
            value: 1.0,
            low: 0.0,
            high: 10.0,
            step: 1.0,
            precision: 0,
        })
        .unwrap();
    let window = ui.window("slider", RECT, &slider).unwrap();
    neocogi_ui::events::map(slider.changed(), session_tx.clone(), |event| format!("numeric:{}", event.value)).unwrap();
    slider.set_value(5.0).unwrap();
    ui.update_with_events(SIZE, &session, |message| {
        labels.push(message);
        Ok(())
    })
    .unwrap();
    assert_eq!(labels.len(), 1);
    key(&ui, KeyCode::Tab);
    key(&ui, KeyCode::ArrowRight);
    ui.update_with_events(SIZE, &session, |message| {
        labels.push(message);
        Ok(())
    })
    .unwrap();
    assert_eq!(labels.last().unwrap(), "numeric:6");
    window.close().unwrap();

    let number = ui.widgets().number(NumberOptions { value: 3.0, step: 1.0, precision: 0 }).unwrap();
    let window = ui.window("number", RECT, &number).unwrap();
    neocogi_ui::events::map(number.changed(), session_tx.clone(), |event| format!("numeric:{}", event.value)).unwrap();
    key(&ui, KeyCode::Tab);
    // Numeric text controls use Up/Down; horizontal sliders use Left/Right.
    key(&ui, KeyCode::ArrowUp);
    ui.update_with_events(SIZE, &session, |message| {
        labels.push(message);
        Ok(())
    })
    .unwrap();
    assert_eq!(labels.last().unwrap(), "numeric:4");
    window.close().unwrap();

    let item = ui.widgets().list_item("item").unwrap();
    let list_box = ui.widgets().list_box("box").unwrap();
    let mut column = ui.widgets().linear(FlowDirection::TopToBottom, CrossSize::Stretch).unwrap();
    column.insert(0, &item, LinearEdge::new(LayoutTrack::Content)).unwrap().finish().unwrap();
    column.insert(1, &list_box, LinearEdge::new(LayoutTrack::Content)).unwrap().finish().unwrap();
    let window = ui.window("lists", RECT, &column).unwrap();
    neocogi_ui::events::map(item.activated(), session_tx.clone(), |event| event.label).unwrap();
    neocogi_ui::events::map(list_box.activated(), session_tx.clone(), |event| event.label).unwrap();
    for input in [KeyCode::Tab, KeyCode::Space, KeyCode::Tab, KeyCode::Space] {
        key(&ui, input);
    }
    ui.update_with_events(SIZE, &session, |message| {
        labels.push(message);
        Ok(())
    })
    .unwrap();
    assert!(labels.ends_with(&["item".to_owned(), "box".to_owned()]));
    window.close().unwrap();

    let mut bar = ui
        .widgets()
        .scrollbar(ScrollRange {
            axis: ScrollAxis::Horizontal,
            viewport: 20,
            content: 100,
            offset: 0,
        })
        .unwrap();
    let window = ui.window("bar", RECT, &bar).unwrap();
    neocogi_ui::events::map(bar.changed(), session_tx.clone(), |event| format!("scroll:{}", event.offset)).unwrap();
    let previous = labels.len();
    bar.set_offset(0).unwrap();
    ui.update_with_events(SIZE, &session, |message| {
        labels.push(message);
        Ok(())
    })
    .unwrap();
    assert_eq!(labels.len(), previous);
    let position = Point { x: 300, y: 100 };
    for event in [
        UiInput::Pointer(position),
        UiInput::Down { position, buttons: 1 },
        UiInput::Up { position, buttons: 1 },
    ] {
        ui.input(event, "").unwrap();
    }
    ui.update_with_events(SIZE, &session, |message| {
        labels.push(message);
        Ok(())
    })
    .unwrap();
    assert!(bar.range().unwrap().offset > 0);
    assert_eq!(labels.last().unwrap(), &format!("scroll:{}", bar.range().unwrap().offset));
    window.close().unwrap();
}

/// Both editor roots expose independent typed ports, and popup dismissal uses the manager's real lifecycle source.
#[test]
fn text_editor_and_popup_event_capabilities_preserve_their_independent_native_sources() {
    let ui = nc_ui_nodes::bind_builtin_nodes(runtime()).unwrap();
    let (session_tx, session) = std::sync::mpsc::channel();
    let mut labels = Vec::new();
    let mut textbox = ui.widgets().textbox("").unwrap();
    let window = ui.window("textbox", RECT, &textbox).unwrap();
    neocogi_ui::events::map(textbox.changed(), session_tx.clone(), |event| {
        format!("changed:{}:{}", event.text, event.cursor)
    })
    .unwrap();
    // Replacing the destination succeeds; only this mapper receives later edits.
    neocogi_ui::events::map(textbox.changed(), session_tx.clone(), |event| {
        format!("changed:{}:{}", event.text, event.cursor)
    })
    .unwrap();
    neocogi_ui::events::map(textbox.submitted(), session_tx.clone(), |event| format!("submitted:{}", event.text)).unwrap();
    textbox.set_text("a").unwrap();
    ui.update_with_events(SIZE, &session, |message| {
        labels.push(message);
        Ok(())
    })
    .unwrap();
    assert!(labels.is_empty());
    key(&ui, KeyCode::Tab);
    ui.input(UiInput::Text, "é").unwrap();
    key(&ui, KeyCode::Enter);
    ui.update_with_events(SIZE, &session, |message| {
        labels.push(message);
        Ok(())
    })
    .unwrap();
    assert_eq!(&labels, &["changed:aé:3", "submitted:aé"]);
    window.close().unwrap();

    let area = ui.widgets().text_area("", TextWrapping::Word, true).unwrap();
    let window = ui.window("textarea", RECT, &area).unwrap();
    neocogi_ui::events::map(area.changed(), session_tx.clone(), |event| format!("changed:{}:{}", event.text, event.cursor)).unwrap();
    neocogi_ui::events::map(area.submitted(), session_tx.clone(), |event| format!("submitted:{}", event.text)).unwrap();
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
    ui.update_with_events(SIZE, &session, |message| {
        labels.push(message);
        Ok(())
    })
    .unwrap();
    assert!(labels.ends_with(&["changed:☃:3".to_owned(), "submitted:☃".to_owned()]));
    let popup_content = ui.widgets().text_block("popup", TextWrapping::None).unwrap();
    let mut popup = ui.popup(window.as_com_ref(), "popup", &popup_content).unwrap();
    neocogi_ui::events::map(neocogi_ui::events::popup_events(popup.as_com_ref()).unwrap(), session_tx.clone(), |_| {
        "dismissed".to_owned()
    })
    .unwrap();
    popup.show_at(Rect { x: 100, y: 100, width: 20, height: 20 }).unwrap();
    ui.update_with_events(SIZE, &session, |message| {
        labels.push(message);
        Ok(())
    })
    .unwrap();
    popup.hide().unwrap();
    ui.update_with_events(SIZE, &session, |message| {
        labels.push(message);
        Ok(())
    })
    .unwrap();
    assert_eq!(labels.last().unwrap(), "dismissed");
    window.close().unwrap();
    ui.update_with_events(SIZE, &session, |message| {
        labels.push(message);
        Ok(())
    })
    .unwrap();
}

/// A failed consumer invalidates the frame without replaying the attempt or losing later raw input.
#[test]
fn failed_consumer_recovers_on_next_update_and_preserves_remaining_native_input() {
    let ui = nc_ui_nodes::bind_builtin_nodes(runtime()).unwrap();
    let button = ui.widgets().button("fail once").unwrap();
    let editor = ui.widgets().textbox("").unwrap();
    let mut column = ui.widgets().linear(FlowDirection::TopToBottom, CrossSize::Stretch).unwrap();
    column.insert(0, &button, LinearEdge::new(LayoutTrack::Content)).unwrap().finish().unwrap();
    column.insert(1, &editor, LinearEdge::new(LayoutTrack::Content)).unwrap().finish().unwrap();
    let window = ui.window("recovery", RECT, &column).unwrap();
    let (events_tx, events) = std::sync::mpsc::channel();
    neocogi_ui::events::map(button.activated(), events_tx.clone(), |_| ()).unwrap();
    let mut attempts = 0;
    for input in [KeyCode::Tab, KeyCode::Space, KeyCode::Tab] {
        key(&ui, input);
    }
    ui.input(UiInput::Text, "still queued").unwrap();
    assert_eq!(
        ui.update_with_events(SIZE, &events, |_| {
            attempts += 1;
            Err(NC_E_INVALID_ARG)
        }),
        Err(NC_E_INVALID_ARG)
    );
    assert_eq!(attempts, 1);
    assert_eq!(editor.text().unwrap(), "");
    let (renderer, _) = renderer();
    assert_eq!(ui.render(SIZE, renderer.as_com_ref()), Err(UI_E_UPDATE_REQUIRED));
    ui.update_with_events(SIZE, &events, |_| {
        attempts += 1;
        Ok(())
    })
    .unwrap();
    assert_eq!(attempts, 1);
    assert_eq!(editor.text().unwrap(), "still queued");
    key(&ui, KeyCode::Tab);
    key(&ui, KeyCode::Space);
    ui.update_with_events(SIZE, &events, |_| {
        attempts += 1;
        Ok(())
    })
    .unwrap();
    assert_eq!(attempts, 2);
    ui.render(SIZE, renderer.as_com_ref()).unwrap();
    window.close().unwrap();
}

/// Newly posted lifecycle messages remain FIFO and settle in the same runtime update.
#[test]
fn real_lifecycle_cascade_is_deferred_until_each_consumer_returns() {
    let ui = nc_ui_nodes::bind_builtin_nodes(runtime()).unwrap();
    let button = ui.widgets().button("cascade").unwrap();
    let window = ui.window("cascade", RECT, &button).unwrap();
    let (events_tx, events) = std::sync::mpsc::channel();
    neocogi_ui::events::map(neocogi_ui::events::window_events(window.as_com_ref()).unwrap(), events_tx.clone(), |event| {
        event
    })
    .unwrap();
    window.set_visibility(Visibility::Hidden).unwrap();
    let mut observed = 0;
    ui.update_with_events(SIZE, &events, |event| {
        if event == WindowEvent::Hidden {
            observed += 1;
            if observed < 3 {
                window.set_visibility(Visibility::Visible)?;
                window.set_visibility(Visibility::Hidden)?;
            }
        }
        Ok(())
    })
    .unwrap();
    assert_eq!(observed, 3);
    assert!(events.try_recv().is_err());
    window.close().unwrap();
}

#[com_object(interfaces(IEventDispatcher))]
/// A foreign consumer fails while the runtime is settling the frame.
struct FailingDispatcher;
impl IEventDispatcher for FailingDispatcher {
    /// Propagates the consumer failure through the actual COM dispatcher boundary.
    fn dispatch(&mut self) -> Result<bool, NcStatus> {
        Err(NC_E_INVALID_ARG)
    }
}

/// Failed foreign consumers leave the frame uncommitted and allow an ordinary successful retry.
#[test]
fn runtime_preserves_uncommitted_frame_on_foreign_consumer_failure() {
    let runtime = runtime();
    let ui = nc_ui_nodes::bind_builtin_nodes(runtime.clone()).unwrap();
    let button = ui.widgets().button("count").unwrap();
    let window = ui.window("count", RECT, &button).unwrap();
    let dispatcher: ComPtr<IEventDispatcherAbi> = FailingDispatcher::from_value(FailingDispatcher).query().unwrap();
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
        let mut delivered = 0;
        let mut remaining = 2;
        let result = neocogi_com::events::with_dispatcher(
            &mut || match events.try_recv() {
                Ok(()) => {
                    delivered += 1;
                    if repeat {
                        sender.send(()).unwrap();
                    }
                    Ok(true)
                }
                Err(_) => Ok(false),
            },
            |dispatcher| runtime.dispatch_boundary(Some(dispatcher), Dimensioni::new(SIZE.width, SIZE.height), &mut remaining),
        );
        assert_eq!(result, Err(UI_E_EVENT_LIMIT));
        assert_eq!(remaining, 0);
        assert_eq!(delivered, 2);
        assert_eq!(events.try_iter().count(), if repeat { 2 } else { 0 });
    }
}

/// A generated receiver's domain failure must propagate out of native widget routing.
#[test]
fn failed_post_preserves_committed_widget_state_and_requires_a_new_frame() {
    /// Deliberately rejects an otherwise valid numeric fact at the foreign receiver boundary.
    fn reject(_: neocogi_ui::events::NumericChanged) -> Result<(), NcStatus> {
        Err(NC_E_INVALID_ARG)
    }
    let ui = nc_ui_nodes::bind_builtin_nodes(runtime()).unwrap();
    let number = ui.widgets().number(NumberOptions { value: 9.0, step: 1.0, precision: 0 }).unwrap();
    let window = ui.window("post failure", RECT, &number).unwrap();
    let (events_tx, events) = std::sync::mpsc::channel();
    number.changed()(Box::new(reject)).unwrap();
    let (renderer, _) = renderer();
    ui.update(SIZE).unwrap();
    key(&ui, KeyCode::Tab);
    key(&ui, KeyCode::ArrowUp);
    assert_eq!(ui.update(SIZE), Err(NC_E_INVALID_ARG));
    assert_eq!(number.value(), Ok(10.0));
    assert!(events.try_recv().is_err());
    assert_eq!(ui.render(SIZE, renderer.as_com_ref()), Err(UI_E_UPDATE_REQUIRED));
    neocogi_ui::events::map(number.changed(), events_tx.clone(), |event| event.value).unwrap();
    ui.update(SIZE).unwrap();
    ui.render(SIZE, renderer.as_com_ref()).unwrap();
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
    let button = ui.widgets().button("lifecycle").unwrap();
    let window = ui.window("post failure", RECT, &button).unwrap();
    let source = window.query::<IWindowEventsAbi>().unwrap();
    let (events_tx, events) = std::sync::mpsc::channel();
    source.changed()(Box::new(reject)).unwrap();
    assert_eq!(window.set_visibility(Visibility::Hidden), Err(NC_E_INVALID_ARG));
    assert_eq!(window.snapshot().unwrap().visibility, Visibility::Hidden);
    assert!(events.try_recv().is_err());
    neocogi_ui::events::map(source.changed(), events_tx.clone(), |event| event).unwrap();
    window.close().unwrap();
}
