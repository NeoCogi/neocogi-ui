# Typed consumer inboxes

`Events<Message>` owns one FIFO and its source connections. Application state stays
in an ordinary Rust value. Map each port into an application message, then provide
one mutable consumer to the UI update:

```rust,no_run
use neocogi_ui::{ButtonWidget, Events, Size, Ui};
use neocogi_com::NcStatus;

/// Application commands are local Rust data, never a plugin ABI.
enum Message { Activated(String) }

fn update(ui: &Ui, button: &ButtonWidget, size: Size) -> Result<(), NcStatus> {
    let mut events = Events::new();
    events.map(button.activated(), |event| Message::Activated(event.label))?;
    let mut count = 0;
    ui.update_with_events(size, &mut events, |message| {
        match message {
            Message::Activated(label) => { count += 1; println!("{label}: {count}"); }
        }
        Ok(())
    })
}
```

Keep the inbox and mappings alive across frames. The example groups construction
and one update only to show their types. A normal application implements the match
in `AppState::update(&mut self, message)` and passes `|message| app.update(message)`.
It needs no handler types, borrowed-state session, or update-cycle protocol.

## Posting and ownership

A generated NCIDL `port` defines a concrete protocol and owned message type.
A named binding such as `completed: FileDialogCompletionPort;` generates the
`.completed()` accessor and its typed connection operation. UI convenience getters
also install domain validation, including native-path and numeric checks.

`events.map(port, mapper)` creates a generated receiver and retains the successful
`IEventSubscription` lease. The temporary port releases its source after setup.
There is one subscriber per source. Producers use `EventSource<E>` with no queue:

```text
producer commits state → protocol receiver copies/validates arguments
                       → mapper translates the owned protocol message
                       → message enters the consumer's FIFO
```

The mapper runs synchronously at emission. It should only translate data or attach
routing IDs; it must not perform UI work or consume application state. Rust cannot
prove closure purity. Captures must be owned or otherwise `'static`. The generated
mutable receiver protects ordinary `FnMut` captures without extra cells. Application
behavior runs later, after traversal releases its borrows.

A source retains a weak connection; its lease owns the receiver, and the receiver
has weak posting access. Neither prolongs the consumer. `emit` returns receiver
errors. Built-in controls preserve committed state and return posting failures
through the enclosing UI operation. No event is retained without a subscriber.

Every port mapped into one inbox shares **emission order**. There is no global
ordering across different consumers. Cancelling a lease or closing a producer
prevents future posts and releases its mapper. Already posted facts remain in the
surviving consumer's FIFO. Dropping an inbox cancels its connections and drops its
pending messages. Request completions need no separate finished/terminal queue.

## Nested components and timing

A behavioral composite has a private message enum, mappings from child ports, and
an ordinary mutable update method. It emits only its declared public results.
Internal messages do not automatically bubble through ancestors. Structural
containers such as `ScrollArea` need no inbox just because they have children.

`Events::component_dispatcher` creates a separate framework driver for a component.
The mounted root exposes it through `IComponentEventProvider`; the runtime discovers
and drives it automatically, including during ordinary `Ui::update`. The driver
holds weak ownership and no copied widget tree. Returning the root's own COM
identity is rejected. Component authors do not write queue-draining loops.

The runtime owns the complete schedule:

1. Synchronize layout and settle pending messages before routing input.
2. Process one raw input, then release traversal borrows.
3. Visit private inboxes descendant-first in stable tree order. Each visit consumes
   the FIFO prefix present at its start; newly posted work waits for another visit.
4. Repeat component/layout work until quiet, then consume one application message.
   Settle any resulting private work before consuming the next application message.
5. Route the next raw input. Publish a renderable frame only after successful completion.

A newly mounted component joins the next pass. Removed mounts retire their driver
tickets before later visits. An empty input queue still receives an eventless update.
One finite budget spans the whole update, including component and application work;
exhaustion leaves unattempted messages queued and the frame uncommitted. Consumer
errors consume only the attempted message, retain successors, and do not roll back
state. Recursive runtime update/render and recursive mutable dispatcher calls reject.

`Ui::update_with_events` borrows the application consumer only for that call. Its
internal scoped COM adapter revokes every retained dispatcher projection on return
or unwind. Application messages and state never cross the plugin boundary.

Programmatic setters remain silent unless specified otherwise. Immutable asset
candidates may be prepared during consumption, but installation requires the frame
boundary after update returns. Posting does not schedule a future frame; timers and
external data still need host scheduling. Dynamic plugins and static monoliths use
the same transport. Plugin descriptor and runtime table ABIs remain v1; generated
Rust event bindings require support API 4.
