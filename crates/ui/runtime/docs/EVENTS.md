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
    events.subscribe(button.activated(), Count)?;
    ui.update_with_events(size, &events)
}
```

Independent producers reuse `neocogi_com_events::EventSource<E>` (also reexported
from `neocogi_ui::events`). The generated `neocogi.com.events` module owns the
lease and dispatcher ABI; the support crate owns typed queues and application
sessions. Standard snapshots are generated from UI NCIDL `port` declarations
and already implement `EventPayload`, which names their concrete COM sink.
Each `name: Protocol;` generates a `.name()` accessor returning an `EventPort`
with a concrete associated event and an optional domain validator. Import the
source's generated `Ports` trait to use it on owned or borrowed COM handles.
Accessors do not query or connect; the session calls the generated shared-receiver
subscription operation. Applications call `session.subscribe(port, handler)`;
UI getters install their domain checks before exposing the port. The generic COM
session claims registration before constructing the sink or calling the source,
restores order on failure, and retains only the successful lease. The port's owned
source projection is released after setup. The file dialog declares
`completed: FileDialogCompletionPort;` directly on `IFileDialog`, so
`session.subscribe(picker.completed(), handler)` needs no extra capability query
or fallible getter. The UI wrapper still validates native-path completions.

Generated sinks use `&mut self`, so their handlers are ordinary concrete fields.
Named structs and enums implement `EventHandler`; closures are not implicitly
adapted. The one session implementation is reexported by UI. Rust binding protocol
3 enforces compiler/support agreement; rebuild plugins for the changed sink
signatures. Plugin descriptor and runtime table ABIs remain v1.

`emit` never invokes application code. `subscribe` returns the application-owned
`IEventSubscription`; the source does not retain the sink or keep the widget alive.

Custom composites can publish port protocols and named bindings in their
NCIDL contract. If a mounted component has private child-event handlers, it
implements `IComponentEventProvider` and returns a separately allocated
`IEventDispatcher`. The runtime rejects a provider that returns its own widget
identity, then caches the driver at the mounted node. Drivers are copied in
descendant-first order after traversal, invoked before the application session,
and skipped if an earlier callback removed their owning mount. Both kinds of
callback share the same finite dispatch budget. `Ui::update_with_events`
accepts the same generic `EventSession<State>` for applications and components.
Ordinary `Ui::update` also drives mounted private sessions automatically. Nesting
has no special depth limit: child outputs can reach parent handlers and then the
application in one update. A callback-mounted child joins the next sweep, while
ancestor removal retires all descendant tickets before subsequent delivery.

A composite may use one private message enum and an `&mut self` update method;
the file picker does so without adding a second event queue. Only declared public
outputs cross component boundaries. Internal child events do not implicitly bubble
through every ancestor; raw input routing remains a separate concern.

There is one subscriber per source. Delivery preserves FIFO within a source and
registration order across a session. A dispatch window fixes the eligible prefix length; each callback pops one
snapshot, leaving unattempted events in place on failure. Newly emitted events
wait for a later source visit, so callback recursion cannot extend the current window.
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
