# Typed events

Application dispatch lives in `microui_redux::EventSession<State>`. A session owns
concrete handlers and COM subscriptions; a producer owns only its event snapshots.
The UI runtime borrows the session's dispatcher between traversal phases and
commits callback edits before routing the next input event.

```rust,no_run
use microui_redux::{ButtonWidget, EventHandler, EventSession, Size, Ui};
use microui_redux::events::Activation;
use nc_com::NcStatus;

struct Count;
impl EventHandler<u32, Activation> for Count {
    fn handle(&mut self, count: &mut u32, _event: &Activation) -> Result<(), NcStatus> {
        *count += 1;
        Ok(())
    }
}
fn update(ui: &Ui, button: &ButtonWidget, size: Size) -> Result<(), NcStatus> {
    let events = EventSession::new(0u32);
    events.subscribe_activation(button, Count)?;
    ui.update_with_events(size, &events)
}
```

Independent producers reuse `microui_redux::events::EventSource<E>`. Standard
snapshots already implement `EventPayload`, which names their concrete COM sink.
A custom snapshot implements that trait for its own declared sink interface.
`emit` never invokes application code. `subscribe` returns the application-owned
`IEventSubscription`; the source does not retain the sink or keep the widget alive.

There is one subscriber per source. Delivery preserves FIFO within a source and
registration order across a session. A failed callback consumes its attempted
snapshot and preserves the unattempted remainder ahead of newer emissions.
Cancellation releases callback ownership even if another caller retains the lease.
Source closure discards ordinary interaction work; explicit terminal events and
finished request outcomes follow their documented distinct lifetime rules.

Programmatic setters remain silent unless their concrete contract says otherwise.
Applications must synchronize UI changes before rendering, and explicitly arrange
future updates for timers, animation or external data. Event delivery does not
schedule frames or make hidden widgets update automatically.

The full ownership, reentry, cascade-budget, error, terminal-delivery and author
obligations are in [COM_AUTHORS.md](../../../docs/COM_AUTHORS.md#typed-events-and-application-state).
The [independent extension](../../../plugins/extension-example) demonstrates a real
derived COM widget using this shared producer. Native algorithm callback fixtures
live under `src/test_support`; there is no production `Context::subscribe` or
application-state parameter on the retained engine.
