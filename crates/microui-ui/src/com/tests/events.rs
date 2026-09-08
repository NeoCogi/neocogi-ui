//! End-to-end application sessions use the public facade and ordinary runtime/window capabilities.
use super::*;
use microui_redux::{EventHandler, EventSession};
use microui_redux::events::Activation;

/// Queues a concrete key press without fabricating semantic widget events.
fn key(ui: &microui_redux::Ui, key: KeyCode) {
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
/// Concrete handler records immutable activation labels into application-owned state.
struct RecordActivation;
impl EventHandler<Vec<String>, Activation> for RecordActivation {
    /// Records the supplied snapshot without reading a live button label.
    fn handle(&mut self, state: &mut Vec<String>, event: &Activation) -> Result<(), NcStatus> {
        state.push(event.label.clone());
        Ok(())
    }
}

/// Registration order governs cross-source delivery while each source retains FIFO across budget boundaries.
#[test]
fn static_session_preserves_registration_order_snapshot_values_and_bounded_progress() {
    let runtime = runtime();
    let ui = microui_redux::Ui::from_interface(runtime.clone()).unwrap();
    let a = ui.widgets().button("a").unwrap();
    let b = ui.widgets().button("b").unwrap();
    let column = ui.widgets().linear(FlowDirection::TOP_TO_BOTTOM, CrossSize::Stretch).unwrap();
    column.insert(0, &a, LinearEdge::new(LayoutTrack::Content)).unwrap().finish().unwrap();
    column.insert(1, &b, LinearEdge::new(LayoutTrack::Content)).unwrap().finish().unwrap();
    let window = ui.window("order", RECT, &column).unwrap();
    let session = EventSession::new(Vec::<String>::new());
    session.subscribe_activation(&b, RecordActivation).unwrap();
    session.subscribe_activation(&a, RecordActivation).unwrap();
    key(&ui, KeyCode::TAB);
    key(&ui, KeyCode::SPACE);
    ui.update(SIZE).unwrap();
    a.set_label("a2").unwrap();
    key(&ui, KeyCode::SPACE);
    ui.update(SIZE).unwrap();
    key(&ui, KeyCode::TAB);
    key(&ui, KeyCode::SPACE);
    ui.update(SIZE).unwrap();
    a.set_label("later a").unwrap();
    b.set_label("later b").unwrap();
    assert_eq!(session.dispatch_bounded(2), Err(UI_E_EVENT_LIMIT));
    assert_eq!(&*session.state().unwrap(), &["b", "a"]);
    assert_eq!(session.dispatch_bounded(1), Ok(1));
    assert_eq!(&*session.state().unwrap(), &["b", "a", "a2"]);
    assert_eq!(session.dispatch_bounded(0), Ok(0));
    // Silent setters create no semantic event and do not make the next sweep deliver anything.
    a.set_label("silent").unwrap();
    ui.update_with_events(SIZE, &session).unwrap();
    assert_eq!(session.state().unwrap().len(), 3);
    let (renderer, _) = renderer();
    ui.render(SIZE, renderer.as_com_ref()).unwrap();
    window.close().unwrap();
    assert_eq!(session.dispatch(), Ok(0));
}

/// Application state may own UI capabilities because no producer queue owns a sink or session.
struct MutationState {
    /// Ordinary static UI facade used for callback edits and deliberate runtime reentry.
    ui: microui_redux::Ui,
    /// The initial window is closed by the activation handler.
    previous: ComPtr<IWindowAbi>,
    /// Actual renderer used to prove callbacks cannot submit an incompletely updated frame.
    renderer: ComPtr<IRendererAbi>,
    /// Newly constructed window is retained for deterministic teardown after the test.
    next: Option<ComPtr<IWindowAbi>>,
    /// Real replacement editor receives raw input queued after the activating event.
    editor: Option<microui_redux::TextboxWidget>,
    /// Ordered semantic observations from activation and terminal window delivery.
    observations: Vec<&'static str>,
}
/// Concrete callback replaces a window while retaining no runtime traversal or node borrow.
struct ReplaceWindow;
impl EventHandler<MutationState, Activation> for ReplaceWindow {
    /// Ordinary COM edits succeed at this boundary; update/render recursion rejects without side effects.
    fn handle(&mut self, state: &mut MutationState, _event: &Activation) -> Result<(), NcStatus> {
        assert_eq!(state.ui.update(SIZE), Err(UI_E_BUSY));
        assert_eq!(state.ui.render(SIZE, state.renderer.as_com_ref()), Err(UI_E_BUSY));
        state.previous.close()?;
        let editor = state.ui.widgets().textbox("")?;
        state.next = Some(state.ui.window("replacement", RECT, &editor)?);
        state.editor = Some(editor);
        state.observations.push("activated");
        Ok(())
    }
}
/// Window lifecycle shares the same application state but its own concrete sink contract.
struct RecordWindow;
impl EventHandler<MutationState, WindowEvent> for RecordWindow {
    /// Captures the terminal event even though the old window is already permanently closed.
    fn handle(&mut self, state: &mut MutationState, event: &WindowEvent) -> Result<(), NcStatus> {
        if *event == WindowEvent::Destroyed {
            state.observations.push("destroyed");
        }
        Ok(())
    }
}

/// A callback-created tree must be measured before subsequent already-queued keyboard/text events are routed.
#[test]
fn eventful_runtime_releases_context_for_handlers_and_routes_later_input_to_their_new_tree() {
    let runtime = runtime();
    let ui = microui_redux::Ui::from_interface(runtime.clone()).unwrap();
    let button = ui.widgets().button("replace window").unwrap();
    let previous = ui.window("first", RECT, &button).unwrap();
    let (renderer, probe) = renderer();
    let session = EventSession::new(MutationState {
        ui: microui_redux::Ui::from_interface(runtime.clone()).unwrap(),
        previous: previous.clone(),
        renderer: renderer.clone(),
        next: None,
        editor: None,
        observations: Vec::new(),
    });
    let activation = session.subscribe_activation(&button, ReplaceWindow).unwrap();
    let lifecycle = session.subscribe_window(previous.as_com_ref(), RecordWindow).unwrap();
    for input in [KeyCode::TAB, KeyCode::SPACE, KeyCode::TAB] {
        key(&ui, input);
    }
    ui.input(UiInput::Text, "arrived after replacement").unwrap();
    ui.update_with_events(SIZE, &session).unwrap();
    assert_eq!(probe.calls.get(), 0);
    assert_eq!(button.label(), Err(UI_E_CLOSED));
    {
        let state = session.state().unwrap();
        assert_eq!(state.observations, ["activated", "destroyed"]);
        assert_eq!(state.editor.as_ref().unwrap().text().unwrap(), "arrived after replacement");
    }
    assert_eq!(activation.active(), Ok(false));
    assert_eq!(lifecycle.active(), Ok(false));
    ui.render(SIZE, renderer.as_com_ref()).unwrap();
    session.state().unwrap().next.as_ref().unwrap().close().unwrap();
    session.cancel().unwrap();
}

/// Owned application state observes final destruction independently of retained lease or widget pointers.
struct OwnerState {
    /// This is deliberately the final strong runtime owner after setup ends.
    _ui: microui_redux::Ui,
    /// External lifetime observation contains no backreference to this state.
    drops: Rc<Cell<u32>>,
}
impl Drop for OwnerState {
    /// Exactly one release proves producer queues cannot retain the application through their callbacks.
    fn drop(&mut self) {
        self.drops.set(self.drops.get() + 1);
    }
}
/// Concrete inert callback makes ownership independent of captured closures or callback effects.
struct ObserveOwner;
impl EventHandler<OwnerState, Activation> for ObserveOwner {
    /// No callback mutation is required for the source/lease/session ownership test.
    fn handle(&mut self, _state: &mut OwnerState, _event: &Activation) -> Result<(), NcStatus> {
        Ok(())
    }
}

/// Session destruction cancels externally retained leases and releases state which itself owns the UI runtime.
#[test]
fn dropping_session_releases_application_owned_runtime_without_a_source_sink_cycle() {
    let ui = microui_redux::Ui::from_interface(runtime()).unwrap();
    let button = ui.widgets().button("owned runtime").unwrap();
    let window = ui.window("owner", RECT, &button).unwrap();
    let drops = Rc::new(Cell::new(0));
    let session = EventSession::new(OwnerState { _ui: ui, drops: drops.clone() });
    let lease = session.subscribe_activation(&button, ObserveOwner).unwrap();
    let retained_driver = session.as_dispatcher().add_ref();
    drop(session);
    assert_eq!(drops.get(), 1);
    assert_eq!(lease.active(), Ok(false));
    assert_eq!(retained_driver.dispatch(1), Err(UI_E_CLOSED));
    assert_eq!(button.label(), Err(UI_E_CLOSED));
    assert_eq!(window.title().unwrap_err(), UI_E_CLOSED);
}

/// Records a complete checkbox snapshot through the approved local callback adapter.
fn record_checkbox(state: &mut Vec<String>, event: &microui_redux::events::CheckboxChanged) -> Result<(), NcStatus> {
    state.push(format!("checked:{}", event.checked));
    Ok(())
}
/// Both numeric controls deliver the same concrete finite-value event shape.
fn record_numeric(state: &mut Vec<String>, event: &microui_redux::events::NumericChanged) -> Result<(), NcStatus> {
    state.push(format!("numeric:{}", event.value));
    Ok(())
}
/// Captures the installed clamped offset rather than recomputing it from current geometry.
fn record_scroll(state: &mut Vec<String>, event: &microui_redux::events::ScrollbarChanged) -> Result<(), NcStatus> {
    state.push(format!("scroll:{}", event.offset));
    Ok(())
}

/// Every scalar and labelled-action source publishes only actual user interaction through the shared transport.
#[test]
fn scalar_and_list_event_capabilities_publish_real_user_changes_and_keep_setters_silent() {
    let ui = microui_redux::Ui::from_interface(runtime()).unwrap();
    let session = EventSession::new(Vec::<String>::new());
    let checkbox = ui.widgets().checkbox("check", false).unwrap();
    let window = ui.window("checkbox", RECT, &checkbox).unwrap();
    session.subscribe_checkbox(&checkbox, record_checkbox).unwrap();
    checkbox.set_checked(true).unwrap();
    ui.update_with_events(SIZE, &session).unwrap();
    assert!(session.state().unwrap().is_empty());
    key(&ui, KeyCode::TAB);
    key(&ui, KeyCode::SPACE);
    ui.update_with_events(SIZE, &session).unwrap();
    assert_eq!(&*session.state().unwrap(), &["checked:false"]);
    window.close().unwrap();

    let slider = ui
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
    session.subscribe_numeric(&slider, record_numeric).unwrap();
    slider.set_value(5.0).unwrap();
    ui.update_with_events(SIZE, &session).unwrap();
    assert_eq!(session.state().unwrap().len(), 1);
    key(&ui, KeyCode::TAB);
    key(&ui, KeyCode::ARROW_RIGHT);
    ui.update_with_events(SIZE, &session).unwrap();
    assert_eq!(session.state().unwrap().last().unwrap(), "numeric:6");
    window.close().unwrap();

    let number = ui.widgets().number(NumberOptions { value: 3.0, step: 1.0, precision: 0 }).unwrap();
    let window = ui.window("number", RECT, &number).unwrap();
    session.subscribe_numeric(&number, record_numeric).unwrap();
    key(&ui, KeyCode::TAB);
    // Numeric text controls use Up/Down; horizontal sliders use Left/Right.
    key(&ui, KeyCode::ARROW_UP);
    ui.update_with_events(SIZE, &session).unwrap();
    assert_eq!(session.state().unwrap().last().unwrap(), "numeric:4");
    window.close().unwrap();

    let item = ui.widgets().list_item("item").unwrap();
    let list_box = ui.widgets().list_box("box").unwrap();
    let column = ui.widgets().linear(FlowDirection::TOP_TO_BOTTOM, CrossSize::Stretch).unwrap();
    column.insert(0, &item, LinearEdge::new(LayoutTrack::Content)).unwrap().finish().unwrap();
    column.insert(1, &list_box, LinearEdge::new(LayoutTrack::Content)).unwrap().finish().unwrap();
    let window = ui.window("lists", RECT, &column).unwrap();
    session.subscribe_activation(&item, RecordActivation).unwrap();
    session.subscribe_activation(&list_box, RecordActivation).unwrap();
    for input in [KeyCode::TAB, KeyCode::SPACE, KeyCode::TAB, KeyCode::SPACE] {
        key(&ui, input);
    }
    ui.update_with_events(SIZE, &session).unwrap();
    assert!(session.state().unwrap().ends_with(&["item".to_owned(), "box".to_owned()]));
    window.close().unwrap();

    let bar = ui
        .widgets()
        .scrollbar(ScrollRange {
            axis: ScrollAxis::HORIZONTAL,
            viewport: 20,
            content: 100,
            offset: 0,
        })
        .unwrap();
    let window = ui.window("bar", RECT, &bar).unwrap();
    session.subscribe_scrollbar(&bar, record_scroll).unwrap();
    let previous = session.state().unwrap().len();
    bar.set_offset(0).unwrap();
    ui.update_with_events(SIZE, &session).unwrap();
    assert_eq!(session.state().unwrap().len(), previous);
    let position = Point { x: 300, y: 100 };
    for event in [
        UiInput::Pointer(position),
        UiInput::Down { position, buttons: 1 },
        UiInput::Up { position, buttons: 1 },
    ] {
        ui.input(event, "").unwrap();
    }
    ui.update_with_events(SIZE, &session).unwrap();
    assert!(bar.range().unwrap().offset > 0);
    assert_eq!(session.state().unwrap().last().unwrap(), &format!("scroll:{}", bar.range().unwrap().offset));
    window.close().unwrap();
}

/// Changed snapshots pair text and cursor from the same semantic edit.
fn record_text_change(state: &mut Vec<String>, event: &microui_redux::events::TextChanged) -> Result<(), NcStatus> {
    state.push(format!("changed:{}:{}", event.text, event.cursor));
    Ok(())
}
/// Submission is independent of the editor's changed port.
fn record_text_submission(state: &mut Vec<String>, event: &microui_redux::events::TextSubmitted) -> Result<(), NcStatus> {
    state.push(format!("submitted:{}", event.text));
    Ok(())
}
/// Ordinary dismissal records a fact independently of whether the popup was subsequently shown again.
fn record_popup(state: &mut Vec<String>, _event: &microui_redux::events::PopupDismissed) -> Result<(), NcStatus> {
    state.push("dismissed".to_owned());
    Ok(())
}

/// Both editor roots expose independent typed ports, and popup dismissal uses the manager's real lifecycle source.
#[test]
fn text_editor_and_popup_event_capabilities_preserve_their_independent_native_sources() {
    let ui = microui_redux::Ui::from_interface(runtime()).unwrap();
    let session = EventSession::new(Vec::<String>::new());
    let textbox = ui.widgets().textbox("").unwrap();
    let window = ui.window("textbox", RECT, &textbox).unwrap();
    session.subscribe_text_changed(&textbox, record_text_change).unwrap();
    assert_eq!(session.subscribe_text_changed(&textbox, record_text_change).unwrap_err(), UI_E_BUSY);
    session.subscribe_text_submitted(&textbox, record_text_submission).unwrap();
    textbox.set_text("a").unwrap();
    ui.update_with_events(SIZE, &session).unwrap();
    assert!(session.state().unwrap().is_empty());
    key(&ui, KeyCode::TAB);
    ui.input(UiInput::Text, "é").unwrap();
    key(&ui, KeyCode::ENTER);
    ui.update_with_events(SIZE, &session).unwrap();
    assert_eq!(&*session.state().unwrap(), &["changed:aé:3", "submitted:aé"]);
    window.close().unwrap();

    let area = ui.widgets().text_area("", TextWrapping::WORD, true).unwrap();
    let window = ui.window("textarea", RECT, &area).unwrap();
    session.subscribe_text_changed(&area, record_text_change).unwrap();
    session.subscribe_text_submitted(&area, record_text_submission).unwrap();
    key(&ui, KeyCode::TAB);
    ui.input(UiInput::Text, "☃").unwrap();
    ui.input(
        UiInput::Key(KeyStroke {
            key: KeyCode::ENTER,
            value: 0,
            pressed: true,
            repeat: false,
            modifiers: 2,
        }),
        "",
    )
    .unwrap();
    ui.update_with_events(SIZE, &session).unwrap();
    assert!(session.state().unwrap().ends_with(&["changed:☃:3".to_owned(), "submitted:☃".to_owned()]));
    let popup_content = ui.widgets().text_block("popup", TextWrapping::NONE).unwrap();
    let popup = ui.popup(window.as_com_ref(), "popup", &popup_content).unwrap();
    let lease = session.subscribe_popup(popup.as_com_ref(), record_popup).unwrap();
    popup.show(Rect { x: 100, y: 100, width: 20, height: 20 }).unwrap();
    ui.update_with_events(SIZE, &session).unwrap();
    popup.hide().unwrap();
    ui.update_with_events(SIZE, &session).unwrap();
    assert_eq!(session.state().unwrap().last().unwrap(), "dismissed");
    window.close().unwrap();
    ui.update_with_events(SIZE, &session).unwrap();
    assert_eq!(lease.active(), Ok(false));
}

/// One failed handler invocation remains consumed while the concrete handler retains its updated retry state.
struct FailOnce {
    /// Cleared before returning the deliberate error, so a later genuine activation can succeed.
    fail: bool,
}
impl EventHandler<u32, Activation> for FailOnce {
    /// Records every attempted invocation, including the one returning a recoverable application error.
    fn handle(&mut self, attempts: &mut u32, _event: &Activation) -> Result<(), NcStatus> {
        *attempts += 1;
        if core::mem::replace(&mut self.fail, false) {
            Err(NC_E_INVALID_ARG)
        } else {
            Ok(())
        }
    }
}

/// Callback failure invalidates the partial frame without discarding later native input or replaying the failed event.
#[test]
fn failed_callback_recovers_on_next_update_and_preserves_remaining_native_input() {
    let ui = microui_redux::Ui::from_interface(runtime()).unwrap();
    let button = ui.widgets().button("fail once").unwrap();
    let editor = ui.widgets().textbox("").unwrap();
    let column = ui.widgets().linear(FlowDirection::TOP_TO_BOTTOM, CrossSize::Stretch).unwrap();
    column.insert(0, &button, LinearEdge::new(LayoutTrack::Content)).unwrap().finish().unwrap();
    column.insert(1, &editor, LinearEdge::new(LayoutTrack::Content)).unwrap().finish().unwrap();
    let window = ui.window("recovery", RECT, &column).unwrap();
    let session = EventSession::new(0);
    session.subscribe_activation(&button, FailOnce { fail: true }).unwrap();
    for input in [KeyCode::TAB, KeyCode::SPACE, KeyCode::TAB] {
        key(&ui, input);
    }
    ui.input(UiInput::Text, "still queued").unwrap();
    assert_eq!(ui.update_with_events(SIZE, &session), Err(NC_E_INVALID_ARG));
    assert_eq!(*session.state().unwrap(), 1);
    assert_eq!(editor.text().unwrap(), "");
    let (renderer, _) = renderer();
    assert_eq!(ui.render(SIZE, renderer.as_com_ref()), Err(UI_E_UPDATE_REQUIRED));
    ui.update_with_events(SIZE, &session).unwrap();
    assert_eq!(*session.state().unwrap(), 1);
    assert_eq!(editor.text().unwrap(), "still queued");
    key(&ui, KeyCode::TAB);
    key(&ui, KeyCode::SPACE);
    ui.update_with_events(SIZE, &session).unwrap();
    assert_eq!(*session.state().unwrap(), 2);
    ui.render(SIZE, renderer.as_com_ref()).unwrap();
    window.close().unwrap();
}

/// Explicit lifecycle cascade uses actual window facts and the ordinary facade-owned dispatcher.
struct CascadeState {
    /// Repeated show/hide transitions produce new Hidden snapshots during delivery.
    window: ComPtr<IWindowAbi>,
    /// Retained projection is deliberately used to exercise recursive session dispatch rejection.
    dispatcher: ComPtr<IEventDispatcherAbi>,
    /// Finite additional transitions required by this test.
    remaining: u32,
    /// Number of immutable Hidden snapshots already observed.
    observed: u32,
}
/// Concrete handler emits another lifecycle fact without replacing or registering a source.
struct Cascade;
impl EventHandler<Option<CascadeState>, WindowEvent> for Cascade {
    /// Recursive delivery rejects before borrowing state; ordinary window edits remain available.
    fn handle(&mut self, state: &mut Option<CascadeState>, event: &WindowEvent) -> Result<(), NcStatus> {
        let state = state.as_mut().unwrap();
        if *event == WindowEvent::Hidden {
            assert_eq!(state.dispatcher.dispatch(1), Err(UI_E_BUSY));
            state.observed += 1;
            if state.remaining != 0 {
                state.remaining -= 1;
                state.window.set_visibility(Visibility::VISIBLE)?;
                state.window.set_visibility(Visibility::HIDDEN)?;
            }
        }
        Ok(())
    }
}

/// One budget spans successive queue snapshots; exhausting it preserves the next cascade event for explicit retry.
#[test]
fn real_lifecycle_cascade_obeys_global_budget_and_rejects_recursive_dispatch() {
    let ui = microui_redux::Ui::from_interface(runtime()).unwrap();
    let button = ui.widgets().button("cascade").unwrap();
    let window = ui.window("cascade", RECT, &button).unwrap();
    let session = EventSession::new(None);
    *session.state_mut().unwrap() = Some(CascadeState {
        window: window.clone(),
        dispatcher: session.as_dispatcher().add_ref(),
        remaining: 2,
        observed: 0,
    });
    session.subscribe_window(window.as_com_ref(), Cascade).unwrap();
    window.set_visibility(Visibility::HIDDEN).unwrap();
    assert_eq!(session.dispatch_bounded(2), Err(UI_E_EVENT_LIMIT));
    assert_eq!(session.state().unwrap().as_ref().unwrap().observed, 2);
    assert_eq!(session.dispatch_bounded(1), Ok(1));
    assert_eq!(session.state().unwrap().as_ref().unwrap().observed, 3);
    assert_eq!(session.dispatch_bounded(0), Ok(0));
    window.close().unwrap();
}

#[com_object(interfaces(IEventDispatcher))]
/// A foreign dispatcher deliberately violates the requested callback bound.
struct InvalidDispatcher;
impl IEventDispatcher for InvalidDispatcher {
    /// Runtime must reject this impossible report rather than accepting an unchecked frame boundary.
    fn dispatch(&self, limit: u32) -> Result<u32, NcStatus> {
        Ok(limit + 1)
    }
}

/// Invalid plugin reports leave the frame uncommitted and allow an ordinary successful retry.
#[test]
fn runtime_rejects_dispatch_count_larger_than_its_requested_bound() {
    let runtime = runtime();
    let ui = microui_redux::Ui::from_interface(runtime.clone()).unwrap();
    let button = ui.widgets().button("count").unwrap();
    let window = ui.window("count", RECT, &button).unwrap();
    let dispatcher: ComPtr<IEventDispatcherAbi> = InvalidDispatcher::from_value(InvalidDispatcher).query().unwrap();
    assert_eq!(runtime.update_with_events(SIZE, dispatcher.as_com_ref()), Err(NC_E_INVALID_ARG));
    let (renderer, _) = renderer();
    assert_eq!(ui.render(SIZE, renderer.as_com_ref()), Err(UI_E_UPDATE_REQUIRED));
    ui.update(SIZE).unwrap();
    ui.render(SIZE, renderer.as_com_ref()).unwrap();
    window.close().unwrap();
}
