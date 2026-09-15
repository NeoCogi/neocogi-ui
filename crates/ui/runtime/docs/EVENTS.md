# Typed events

Application dispatch lives in `neocogi_ui::EventSession<State>`. A session owns
concrete handlers and COM subscriptions; a producer owns only its event snapshots.
The UI runtime borrows the session's dispatcher between traversal phases and
commits callback edits before routing the next input event.

```rust,no_run
use neocogi_ui::{ButtonWidget, EventHandler, EventSession, Size, Ui};
use neocogi_ui::events::Activation;
use neocogi_com::NcStatus;

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

Independent producers reuse `neocogi_com_events::EventSource<E>` (also reexported
from `neocogi_ui::events`). The generated `neocogi.com.events` module owns the
lease and dispatcher ABI; the support crate owns typed queues and application
sessions. Standard snapshots are generated from UI NCIDL `event` declarations
and already implement `EventPayload`, which names their concrete COM sink.
Each source port generates plain and checked registration connectors. Checked
adapters run a concrete domain validator after decoding and before application-
state borrowing, replacing handwritten COM sink objects without putting UI
rules into generic event transport.
`emit` never invokes application code. `subscribe` returns the application-owned
`IEventSubscription`; the source does not retain the sink or keep the widget alive.

Custom composites can publish first-class `event` sinks and ports in their
NCIDL contract. If a mounted component has private child-event handlers, it
implements `IComponentEventProvider` and returns a separately allocated
`IEventDispatcher`. The runtime rejects a provider that returns its own widget
identity, then caches the driver at the mounted node. Drivers are copied in
descendant-first order after traversal, invoked before the application session,
and skipped if an earlier callback removed their owning mount. Both kinds of
callback share the same finite dispatch budget. `Ui::update_with_events`
accepts the generic `neocogi_com_events::EventSession<State>` as well as the UI
family facade through `EventDispatchSource`.

There is one subscriber per source. Delivery preserves FIFO within a source and
registration order across a session. A failed callback consumes its attempted
snapshot and preserves the unattempted remainder ahead of newer emissions.
Cancellation releases callback ownership even if another caller retains the lease.
Source closure discards ordinary interaction work; explicit terminal events and
finished request outcomes follow their documented distinct lifetime rules.

Handlers may prepare immutable asset candidates, but `Ui::install_assets` requires
exclusive runtime access and therefore runs only after `update_with_events` returns.
The full demo publishes its latest prepared candidate in the host frame boundary before
the following synchronization pass.

Programmatic setters remain silent unless their concrete contract says otherwise.
Applications must synchronize UI changes before rendering, and explicitly arrange
future updates for timers, animation or external data. Event delivery does not
schedule frames or make hidden widgets update automatically.

The full ownership, reentry, cascade-budget, error, terminal-delivery and author
obligations are in [COM_AUTHORS.md](../../../../docs/COM_AUTHORS.md#typed-events-and-application-state).
The [independent extension](../../../../examples/ui-extension) demonstrates a real
derived COM widget using this shared producer. Native algorithm callback fixtures
live under `src/test_support`; there is no production `Context::subscribe` or
application-state parameter on the retained engine.
