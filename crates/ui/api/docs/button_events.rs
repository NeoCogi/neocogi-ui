use neocogi_com::{ComRef, NcStatus, NcValueRef, WeakComBox};
use neocogi_ui::{
    ButtonAbi, ButtonEventQueueAbi, IApplicationDispatcherAbi, KeyCode, KeyStroke, Rect, Size, Ui, UiInput, button,
    events::{Controller, EventModel},
};

/// Application messages own their payload but only observe the emitting widget.
type Message = (WeakComBox<ButtonAbi>, button::Event);

#[neocogi_com::com_object(neocogi_ui::nodes::ButtonEventQueue)]
/// One concrete COM receiver forwards facts into the application's inbox.
struct AppQueue {
    /// Sending never retains the application's mutable model.
    send: std::sync::mpsc::Sender<Message>,
}
impl neocogi_ui::nodes::ButtonEventQueue for AppQueue {
    /// Capture the event while its producer storage is valid; behavior runs later.
    fn post(&mut self, origin: ComRef<'_, ButtonAbi>, value: NcValueRef<'_, button::Event>) -> Result<(), NcStatus> {
        let event = value.to_owned()?;
        button::validate(&event)?;
        // A Button has one ComBox owner. It cannot promote into a shared ComPtr.
        let origin = origin.downgrade_box()?;
        // This application's policy discards facts after its receiver is gone.
        let _ = self.send.send((origin, event));
        Ok(())
    }
}

/// Ordinary state changed only by the controller's exclusive handler call.
struct Counter {
    /// Activations whose originating button still exists.
    count: u32,
    /// Records discarded stale events so the walkthrough can verify that path.
    ignored: u32,
}
impl EventModel for Counter {
    type Message = Message;
    /// This UI-thread policy ignores work from removed buttons. Dialog results
    /// may require a different policy: expiration does not erase captured facts.
    fn handle(&mut self, (origin, event): Message) -> Result<(), NcStatus> {
        if !origin.is_alive() {
            self.ignored += 1;
            return Ok(());
        }
        // is_alive is an observation, not a lease; any later widget call still
        // performs its own lifetime/borrow check before entering the provider.
        match event {
            button::Event::Activated(_) => self.count += 1,
        }
        Ok(())
    }
}

/// Supplies the same key events a platform host would forward to the UI.
fn press(ui: &Ui, key: KeyCode) -> Result<(), NcStatus> {
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
}

/// Run with an initialized, otherwise empty Ui and room for a 400-by-200 window.
/// Mount, activate and remove a real button while retaining a queued weak origin.
pub fn walkthrough(ui: &Ui, size: Size) -> Result<(), NcStatus> {
    let (send, messages) = std::sync::mpsc::channel();
    let queue = AppQueue::from_value(AppQueue { send }).query::<ButtonEventQueueAbi>()?;
    let button = ui.widgets().button_with_queue("Count", Some(queue.as_com_ref()))?;
    let observed = button.as_interface().downgrade();
    // Insertion transfers ownership to the window. On construction failure this
    // caller drops its newly created, unmounted candidate and returns the status.
    let window = ui
        .window("Counter", Rect { x: 20, y: 20, width: 400, height: 200 }, button)
        .map_err(|(status, _unmounted)| status)?;
    let app = Controller::into_local(Controller::new(Counter { count: 0, ignored: 0 }, messages));
    let dispatcher = app.query::<IApplicationDispatcherAbi>()?;

    press(ui, KeyCode::Tab)?;
    press(ui, KeyCode::Space)?;
    ui.update_with_events(size, dispatcher.as_com_ref())?;
    assert_eq!(app.try_borrow()?.model.count, 1);

    // Ordinary update routes input but does not dispatch this application inbox.
    press(ui, KeyCode::Space)?;
    ui.update(size)?;
    window.close()?; // The parent removes the window and drops its widget owner.
    assert!(!observed.is_alive());
    assert_eq!(observed.label().unwrap_err(), neocogi_com::NC_E_DROPPED);

    ui.update_with_events(size, dispatcher.as_com_ref())?;
    let state = app.try_borrow()?;
    assert_eq!(state.model.count, 1);
    assert_eq!(state.model.ignored, 1);
    Ok(())
}
