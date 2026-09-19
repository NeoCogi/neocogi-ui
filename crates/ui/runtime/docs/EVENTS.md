# Typed consumer inboxes

`Events<Message>` owns one FIFO. Application state stays in an ordinary Rust value. Map each output into an application message, then provide
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

Keep the inbox alive across frames; producers own installed mappings. The example
groups construction and one update only to show their types. A normal application implements the match
in `AppState::update(&mut self, message)` and passes `|message| app.update(message)`.
It needs no handler types, borrowed-state session, or update-cycle protocol.

## Posting and ownership

NCIDL declares concrete `#[message]` structs/enums and named outputs such as
`completed: FileDialogCompletion;`. The generated `.completed()` returns a
one-shot installation closure. `events.map(picker.completed(), mapper)` calls it
and returns `Result<()>`. There is no persistent output-view object or subscription.
UI accessors add domain checks such as native-path and finite-number validation.

A producer stores only `Option<ComPtr<__FileDialogCompletionReceiverAbi>>` for
that output. Its ordinary generated COM interface requires a mutable
`__map_completed(destination)` setter. The implementation checks normal object
eligibility, then assigns `Some(destination.add_ref())`. Structural forwarding
calls the child's same setter directly. The object macro has no event-specific
binding syntax or dispatch path.

```text
producer commits state → concrete_message.post_to(&mut destination)
                       → generated receiver decodes/copies/retains the message
                       → mapper translates into the consumer's Rust enum
                       → enqueue in that consumer's FIFO
```

`post_to` is a generated convenience method for one borrowed `NcMessage<T>` COM
call; absent destinations are successful no-ops. Application and component-private
enums remain ordinary Rust declarations. Only published COM output messages need
NCIDL. Native code and plugins use the same checked C wire representation.

The mapper runs synchronously at emission. It should translate data or attach
routing IDs; UI work and application behavior belong in the later mutable consumer.
Captures must be owned or otherwise `'static`. The typed `FnMut(ConcreteMessage)`
callback keeps its concrete parameter type and stays inside its binary.
The existing COM mutable gate rejects recursive entry without a posting state machine.

The producer owns its adapter; the consumer owns its FIFO; the adapter has weak
FIFO access. A second `map` replaces the destination for future posts. Already
queued messages keep their original values and routing IDs. Producer closure or
destruction releases the adapter without retracting queued facts. Dropping the
consumer drops its pending messages, and later valid posts are ignored without
running its mapper. The inert adapter and its captures remain until producer
closure, destruction or replacement. This explicit ownership rule avoids tokens,
generations, cancellation registries and a second delivery lifecycle. Mappers
should capture data rather than their own producer, which would create an ordinary
reference cycle.

Every output mapped into one inbox shares **post order**. There is no global FIFO
across consumers. Posting errors propagate through the enclosing operation after
producer state has committed. Request completions follow the same rule as other
facts; there is no separate terminal queue.

## Nested components and timing

A behavioral composite has a private message enum, mappings from child outputs, and
an ordinary mutable update method. It emits only its declared public results.
Internal messages do not automatically bubble through ancestors. Structural
containers such as `ScrollArea` need no inbox just because they have children.

`Events::component_dispatcher` creates a separate framework driver for a component.
It accepts a weakly held `Rc<RefCell<Model>>` and an ordinary
`fn(&mut Model, Message) -> Result<(), NcStatus>` entry point. That single shared
borrow boundary is needed when separate COM controller and mounted-driver identities
both access the component; the model's fields and methods use ordinary mutable access.
The mounted root exposes it through `IComponentEventProvider`; the runtime discovers
and drives it automatically, including during ordinary `Ui::update`. The driver
holds weak ownership and no copied widget tree. Returning the root's own COM
identity is rejected. Component authors do not write queue-draining loops.

The runtime owns the complete schedule:

1. Synchronize layout and settle pending messages before routing input.
2. Process one raw input, then release traversal borrows.
3. Visit private inboxes descendant-first in stable tree order, consuming at most
   one message per component per pass. A removed mount is skipped before another
   message can be delivered, including a second message for the same component.
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
the same transport. Plugin descriptor and runtime table ABIs remain v1. Generated
message and output bindings depend on COM alone, independently of the consumer queue library.
