use neocogi_com::NcStatus;
use neocogi_ui::{
    ButtonAbi, IApplicationDispatcherAbi, KeyCode, KeyStroke, Rect, Size, Ui, UiInput, IActivatedEventAbi,
    events::{channel, Dispatcher, EventMessage, MessageProcessor},
};

/// Ordinary state changed only by the controller's exclusive handler call.
struct Counter {
    /// Activations whose originating button still exists.
    count: u32,
    /// Records discarded stale events so the walkthrough can verify that path.
    ignored: u32,
}
impl MessageProcessor for Counter {
    type Message = EventMessage;
    /// This UI-thread policy ignores work from removed buttons. Dialog results
    /// may require a different policy: expiration does not erase captured facts.
    fn handle(&mut self, message: EventMessage) -> Result<(), NcStatus> {
        let Some(origin) = message.origin.widget::<ButtonAbi>()? else {
            return Ok(());
        };
        let Some(_activation) = message.query::<IActivatedEventAbi>()? else {
            return Ok(());
        };
        if !origin.is_alive() {
            self.ignored += 1;
            return Ok(());
        }
        // is_alive is an observation, not a lease; any later widget call still
        // performs its own lifetime/borrow check before entering the provider.
        self.count += 1;
        Ok(())
    }
}

/// Supplies the same key events a platform host would forward to the UI.
fn press(ui: &Ui, key: KeyCode) -> Result<(), NcStatus> {
    ui.input(
        UiInput::Key(KeyStroke {
            key,
            action: neocogi_ui::KeyAction::Pressed { repeat: false },
            modifiers: 0,
        }),
        "",
    )
}

/// Run with an initialized, otherwise empty Ui and room for a 400-by-200 window.
/// Mount, activate and remove a real button while retaining a queued weak origin.
pub fn walkthrough(ui: &Ui, size: Size) -> Result<(), NcStatus> {
    let (queue, messages) = channel();
    let button = ui.widgets().button_with_queue("Count", Some(queue.as_com_ref()))?;
    let observed = button.as_interface().downgrade();
    // Insertion transfers ownership to the window. On construction failure this
    // caller drops its newly created, unmounted candidate and returns the status.
    let window = ui
        .window("Counter", Rect { x: 20, y: 20, width: 400, height: 200 }, button)
        .map_err(|(status, _unmounted)| status)?;
    let app = Dispatcher::into_local(Dispatcher::new(Counter { count: 0, ignored: 0 }, messages));
    let dispatcher = app.query::<IApplicationDispatcherAbi>()?;

    press(ui, KeyCode::Tab)?;
    press(ui, KeyCode::Space)?;
    ui.update_with_events(size, dispatcher.as_com_ref())?;
    assert_eq!(app.try_borrow()?.processor.count, 1);

    // Ordinary update routes input but does not dispatch this application inbox.
    press(ui, KeyCode::Space)?;
    ui.update(size)?;
    window.close()?; // The parent removes the window and drops its widget owner.
    assert!(!observed.is_alive());
    assert_eq!(observed.label().unwrap_err(), neocogi_com::NC_E_DROPPED);

    ui.update_with_events(size, dispatcher.as_com_ref())?;
    let state = app.try_borrow()?;
    assert_eq!(state.processor.count, 1);
    assert_eq!(state.processor.ignored, 1);
    Ok(())
}
