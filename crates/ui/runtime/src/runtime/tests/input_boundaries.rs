//! Input bookkeeping brackets actual routing and all its messages without a traversal borrow.
use super::*;
use neocogi_ui::{Ui, events::testing::TestApp};

/// Failures cover each admission/dispatch/completion edge and precedence when two hooks fail.
#[derive(Clone, Copy)]
enum Failure {
    /// Ordinary scope and message handling succeed.
    None,
    /// The first Begin consumes its attempted input before widget routing.
    Begin,
    /// A widget edit commits, but consuming its first semantic message fails.
    Handler,
    /// Successful routing/dispatch is followed by a failed End notification.
    End,
    /// Begin's invalid argument must survive End's different out-of-memory error.
    Both,
}

/// Owned fixture state records real callback order and checks nested-update admission.
struct State {
    /// Concrete runtime services remain callable outside traversal, with the outer update guarded.
    ui: Ui,
    /// Callback ordering independent of COM lifetime.
    trace: Vec<&'static str>,
    /// Exactly the requested first scope can reject; recovery uses ordinary success paths.
    failure: Failure,
}
impl State {
    /// Message errors consume only that message; the producer's text edit remains committed.
    fn handle(&mut self, (): ()) -> Result<(), NcStatus> {
        self.trace.push("message");
        match self.failure {
            Failure::Handler => {
                self.failure = Failure::None;
                Err(NC_E_INVALID_ARG)
            }
            _ => Ok(()),
        }
    }
    /// Bookkeeping executes outside context ownership, while nested updates remain forbidden.
    fn boundary(&mut self, boundary: InputBoundary, _: &()) -> Result<(), NcStatus> {
        assert_eq!(self.ui.update(SIZE), Err(UI_E_BUSY));
        match boundary {
            InputBoundary::Begin { event } => {
                self.trace.push(match event {
                    WidgetEvent::Key(key) if key.key == KeyCode::Tab => "tab",
                    WidgetEvent::Key(key) if key.key == KeyCode::Space => "space",
                    WidgetEvent::Text => "text",
                    _ => panic!("unexpected fixture input"),
                });
                match self.failure {
                    Failure::Begin => {
                        self.failure = Failure::None;
                        Err(NC_E_INVALID_ARG)
                    }
                    Failure::Both => Err(NC_E_INVALID_ARG),
                    _ => Ok(()),
                }
            }
            InputBoundary::End => {
                self.trace.push("end");
                match self.failure {
                    Failure::End => {
                        self.failure = Failure::None;
                        Err(NC_E_INVALID_ARG)
                    }
                    Failure::Both => {
                        self.failure = Failure::None;
                        Err(neocogi_com::NC_E_OUT_OF_MEMORY)
                    }
                    _ => Ok(()),
                }
            }
        }
    }
}

/// One outer update contains separate scopes, with each semantic message preceding its End.
#[test]
fn scopes_follow_actual_input_and_settle_messages_before_end() {
    let ui = nc_ui_nodes::bind_builtin_nodes(runtime()).unwrap();
    let (send, messages) = std::sync::mpsc::channel();
    let queue = neocogi_ui::test_event_queue!(neocogi_ui::ButtonAbi, (), send, |_queue, _origin, _event| ()).unwrap();
    let button = ui.widgets().button_with_queue("scope", Some(queue.as_com_ref())).unwrap();
    let window = ui.window("input scopes", RECT, button).unwrap();
    let app = TestApp::new(
        State {
            ui: ui.clone(),
            trace: Vec::new(),
            failure: Failure::None,
        },
        messages,
        State::handle,
    )
    .with_input_boundary(State::boundary);
    app.update(&ui, SIZE).unwrap();
    assert!(app.state().trace.is_empty(), "eventless work is outside an input scope");
    for key in [KeyCode::Tab, KeyCode::Space] {
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
    app.update(&ui, SIZE).unwrap();
    assert_eq!(app.state().trace, ["tab", "end", "space", "message", "end"]);
    window.close().unwrap();
}

/// Every failed attempt still ends its scope, preserves the first error and leaves later input queued.
#[test]
fn failed_begin_handler_and_end_close_scopes_and_allow_next_input_recovery() {
    for failure in [Failure::Begin, Failure::Handler, Failure::End, Failure::Both] {
        let ui = nc_ui_nodes::bind_builtin_nodes(runtime()).unwrap();
        let (send, messages) = std::sync::mpsc::channel();
        let queue = neocogi_ui::test_event_queue!(neocogi_ui::TextboxAbi, (), send, |_queue, _origin, _event| ()).unwrap();
        let text = ui.widgets().textbox_with_queue("", Some(queue.as_com_ref())).unwrap();
        let observed = text.handle();
        let window = ui.window("failed scopes", RECT, text).unwrap();
        ui.input(
            UiInput::Key(KeyStroke {
                key: KeyCode::Tab,
                action: neocogi_ui::KeyAction::Pressed { repeat: false },
                modifiers: 0,
            }),
            "",
        )
        .unwrap();
        ui.update(SIZE).unwrap();
        let app = TestApp::new(
            State {
                ui: ui.clone(),
                trace: Vec::new(),
                failure,
            },
            messages,
            State::handle,
        )
        .with_input_boundary(State::boundary);
        ui.input(UiInput::Text, "x").unwrap();
        ui.input(UiInput::Text, "y").unwrap();
        assert_eq!(app.update(&ui, SIZE), Err(NC_E_INVALID_ARG));
        let rejected_before_routing = matches!(failure, Failure::Begin | Failure::Both);
        assert_eq!(observed.text().unwrap(), if rejected_before_routing { "" } else { "x" });
        assert_eq!(app.state().trace.last(), Some(&"end"));
        app.update(&ui, SIZE).unwrap();
        assert_eq!(observed.text().unwrap(), if rejected_before_routing { "y" } else { "xy" });
        let state = app.state();
        assert_eq!(state.trace.iter().filter(|&&entry| entry == "text").count(), 2);
        assert_eq!(state.trace.iter().filter(|&&entry| entry == "end").count(), 2);
        window.close().unwrap();
    }
}
