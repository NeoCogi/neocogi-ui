//! Real generated widget calls distinguish receiver expiry from behavior errors.
use super::*;
use super::composite_fixture::Composite;
use neocogi_ui::{LeafContent, Painter, events::MessageProcessor};
use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};

/// Both supported private-behavior entry points must obey the same scheduler contract.
#[derive(Clone, Copy)]
enum Kind {
    /// Public application leaf helper delegates dispatch through generated IWidget.
    Leaf,
    /// Concrete composite owns a model and inbox alongside real container geometry.
    Composite,
}

/// Owned commands make every attempted and unattempted message observable.
enum Command {
    /// Return a handler error while the scheduled widget remains alive.
    Fail(NcStatus),
    /// Access a separate widget whose owner has already been destroyed.
    Read(WeakComBox<ButtonAbi>),
    /// Commit an unrelated subtree removal, then fail without rolling it back.
    RemoveThenFail(ComPtr<IWindowAbi>, NcStatus),
    /// Record later work that must wait for another update after a failure.
    Record(&'static str),
}

/// Plain mutable behavior sends observations without interior-mutability fixtures.
struct Behavior {
    /// The test owns the receiver; no observation retains application/widget owners.
    trace: Sender<&'static str>,
}
impl MessageProcessor for Behavior {
    /// Commands are dequeued once by the leaf or composite's own receiver.
    type Message = Command;

    /// Report entry before each fallible operation to distinguish attempts from success.
    fn handle(&mut self, command: Command) -> Result<(), NcStatus> {
        match command {
            Command::Fail(error) => {
                self.trace.send("failed").unwrap();
                Err(error)
            }
            Command::Read(dependency) => {
                self.trace.send("dependency").unwrap();
                dependency.label()?;
                Ok(())
            }
            Command::RemoveThenFail(window, error) => {
                window.close()?;
                self.trace.send("removed").unwrap();
                Err(error)
            }
            Command::Record(name) => {
                self.trace.send(name).unwrap();
                Ok(())
            }
        }
    }
}

/// The leaf path owns the same behavior directly, without a second COM dispatcher.
struct Leaf {
    /// The concrete model is mutated only through its exclusive dispatch call.
    behavior: Behavior,
    /// Pending messages must survive a handler error until a later update.
    messages: Receiver<Command>,
}
impl LeafContent for Leaf {
    /// Fixed geometry isolates event sequencing from theme measurement details.
    fn measure(&self, _: LayoutLimits, _: ComRef<'_, ITextMetricsAbi>) -> Result<Size, NcStatus> {
        Ok(Size { width: 40, height: 24 })
    }
    /// Rendering has no role in this event-order fixture.
    fn paint(&mut self, _: WidgetPaint, _: &mut Painter<'_>) -> Result<(), NcStatus> {
        Ok(())
    }
}

/// Mounts one real widget and retains only its weak observation and logical window.
fn mount_probe(
    ui: &neocogi_ui::Ui,
    kind: Kind,
    name: &str,
    trace: Sender<&'static str>,
    messages: Receiver<Command>,
) -> (ComPtr<IWindowAbi>, WeakComBox<IWidgetAbi>) {
    let behavior = Behavior { trace };
    match kind {
        Kind::Leaf => {
            let root = ui.widgets().dispatching_leaf(Leaf { behavior, messages }, WidgetOptions::NONE).unwrap();
            let observer = root.node_interface().unwrap();
            (ui.window(name, RECT, root).unwrap(), observer)
        }
        Kind::Composite => {
            let (root, _) = Composite::build(ui, behavior, messages);
            let observer = root.query::<IWidgetAbi>().unwrap();
            (ui.window(name, RECT, root.into_interface::<IWidgetAbi>().unwrap()).unwrap(), observer)
        }
    }
}

/// No status returned by a live handler authorizes skipping its failure.
/// Later private work, later widgets and application messages all remain pending.
#[test]
fn live_errors_stop_before_successors_widgets_and_application_messages() {
    for kind in [Kind::Leaf, Kind::Composite] {
        for error in [neocogi_com::NC_E_CLOSED, neocogi_com::NC_E_DROPPED, NC_E_INVALID_ARG] {
            let ui = neocogi_ui::nodes::bind_builtin_nodes(runtime()).unwrap();
            let (trace, observed) = mpsc::channel();
            let (later, messages) = mpsc::channel();
            let (_later_window, _) = mount_probe(&ui, kind, "later", trace.clone(), messages);
            let (first, messages) = mpsc::channel();
            // Newest surface is dispatched first; both widgets are real mounted objects.
            let (_first_window, receiver) = mount_probe(&ui, kind, "first", trace.clone(), messages);
            let (application, messages) = mpsc::channel();
            let app = neocogi_ui::events::testing::TestApp::new(trace, messages, |trace, ()| {
                trace.send("application").unwrap();
                Ok(())
            });
            first.send(Command::Fail(error)).unwrap();
            first.send(Command::Record("successor")).unwrap();
            later.send(Command::Record("later widget")).unwrap();
            application.send(()).unwrap();

            assert_eq!(app.update(&ui, SIZE), Err(error));
            assert!(receiver.is_alive());
            assert!(receiver.properties().is_ok());
            assert_eq!(observed.try_iter().collect::<Vec<_>>(), ["failed"]);

            app.update(&ui, SIZE).unwrap();
            assert_eq!(observed.try_iter().collect::<Vec<_>>(), ["successor", "later widget", "application"]);
        }
    }
}

/// A real expired dependency must not be mistaken for expiry of its live caller.
#[test]
fn expired_dependency_error_propagates_from_live_leaf_and_composite() {
    for kind in [Kind::Leaf, Kind::Composite] {
        let ui = neocogi_ui::nodes::bind_builtin_nodes(runtime()).unwrap();
        let child = ui.widgets().button_unobserved("temporary child").unwrap();
        let dependency = child.as_interface().downgrade();
        let window = ui.window("removed dependency", RECT, child).unwrap();
        window.close().unwrap();
        assert!(!dependency.is_alive());
        let (trace, observed) = mpsc::channel();
        let (send, messages) = mpsc::channel();
        let (_window, receiver) = mount_probe(&ui, kind, "live caller", trace, messages);
        send.send(Command::Read(dependency)).unwrap();
        send.send(Command::Record("successor")).unwrap();

        assert_eq!(ui.update(SIZE), Err(neocogi_com::NC_E_DROPPED));
        assert!(receiver.is_alive());
        assert_eq!(observed.try_iter().collect::<Vec<_>>(), ["dependency"]);
        ui.update(SIZE).unwrap();
        assert_eq!(observed.try_iter().collect::<Vec<_>>(), ["successor"]);
    }
}

/// A failed callback's committed topology edit survives, and retry uses the current tree.
#[test]
fn topology_change_then_failure_rebuilds_before_retry() {
    for kind in [Kind::Leaf, Kind::Composite] {
        for error in [neocogi_com::NC_E_CLOSED, neocogi_com::NC_E_DROPPED] {
            let ui = neocogi_ui::nodes::bind_builtin_nodes(runtime()).unwrap();
            let (trace, observed) = mpsc::channel();
            let (removed, messages) = mpsc::channel();
            let (removed_window, removed_receiver) = mount_probe(&ui, kind, "removed", trace.clone(), messages);
            let (later, messages) = mpsc::channel();
            let (_later_window, _) = mount_probe(&ui, kind, "remaining", trace.clone(), messages);
            let (first, messages) = mpsc::channel();
            let (_first_window, _) = mount_probe(&ui, kind, "mutates then fails", trace, messages);
            removed.send(Command::Record("must never run")).unwrap();
            later.send(Command::Record("remaining")).unwrap();
            first.send(Command::RemoveThenFail(removed_window, error)).unwrap();
            first.send(Command::Record("successor")).unwrap();

            assert_eq!(ui.update(SIZE), Err(error));
            assert!(!removed_receiver.is_alive());
            assert!(removed.send(Command::Record("also impossible")).is_err());
            assert_eq!(observed.try_iter().collect::<Vec<_>>(), ["removed"]);
            ui.update(SIZE).unwrap();
            assert_eq!(observed.try_iter().collect::<Vec<_>>(), ["successor", "remaining"]);
        }
    }
}

/// Raw input remains queued when the preceding private dispatch boundary fails.
#[test]
fn failed_widget_dispatch_preserves_later_raw_text_input() {
    let ui = neocogi_ui::nodes::bind_builtin_nodes(runtime()).unwrap();
    let (trace, observed) = mpsc::channel();
    let (send, messages) = mpsc::channel();
    let (_probe_window, _) = mount_probe(&ui, Kind::Leaf, "behavior", trace, messages);
    let text = ui.widgets().textbox_unobserved("before").unwrap();
    let retained = text.handle();
    let _text_window = ui.window("input target", RECT, text).unwrap();
    ui.update(SIZE).unwrap();
    for pressed in [true, false] {
        ui.input(
            UiInput::Key(KeyStroke {
                key: KeyCode::Tab,
                action: if pressed {
                    neocogi_ui::KeyAction::Pressed { repeat: false }
                } else {
                    neocogi_ui::KeyAction::Released
                },
                modifiers: 0,
            }),
            "",
        )
        .unwrap();
    }
    ui.update(SIZE).unwrap();
    assert!(matches!(ui.text_input_area().unwrap(), TextInputArea::Area(_)));
    send.send(Command::Fail(neocogi_com::NC_E_CLOSED)).unwrap();
    ui.input(UiInput::Text, "!").unwrap();

    assert_eq!(ui.update(SIZE), Err(neocogi_com::NC_E_CLOSED));
    assert_eq!(retained.text().unwrap(), "before");
    assert_eq!(observed.try_iter().collect::<Vec<_>>(), ["failed"]);
    ui.update(SIZE).unwrap();
    assert_eq!(retained.text().unwrap(), "before!");
}

impl neocogi_ui::DispatchingLeafContent for Leaf {
    fn dispatch_next(&mut self, _: &mut neocogi_ui::LeafDispatchContext<'_>) -> Result<bool, NcStatus> {
        match self.messages.try_recv() {
            Ok(command) => {
                self.behavior.handle(command)?;
                Ok(true)
            }
            Err(TryRecvError::Empty | TryRecvError::Disconnected) => Ok(false),
        }
    }
}
