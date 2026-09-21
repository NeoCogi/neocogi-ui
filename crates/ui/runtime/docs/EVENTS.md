# Typed messages through standard channels

The application owns a `std::sync::mpsc::Receiver<Message>`. Each installed COM
mapping owns a cloned `Sender<Message>`. The standard channel stores pending
messages; there is no framework queue type, sender wrapper or subscription object.
Application state stays in its ordinary owner.

```rust,no_run
use std::sync::mpsc;
use neocogi_ui::{events, ButtonWidget, Size, Ui};
use neocogi_com::NcStatus;

/// Application commands are local Rust data, never a plugin ABI.
enum Message { Activated(String) }

fn update(ui: &Ui, button: &ButtonWidget, size: Size) -> Result<(), NcStatus> {
    let (send, messages) = mpsc::channel();
    events::map(button.activated(), send, |event| Message::Activated(event.label))?;
    let mut count = 0;
    ui.update_with_events(size, &messages, |message| {
        match message {
            Message::Activated(label) => { count += 1; println!("{label}: {count}"); }
        }
        Ok(())
    })
}
```

Keep the receiver alive across frames. This example groups construction and one
update to show their types. A normal application implements the match in
`AppState::update(&mut self, message)` and passes `|message| app.update(message)`.
Clone the sender when installing several mappings into the same channel. Neither
threads nor `Message: Send` are required when both endpoints stay on the UI thread.
Use the unbounded `channel()`; a blocking send into a full `sync_channel()` could
prevent the same UI thread from reaching its later receive operation.

## Posting and ownership

NCIDL declares concrete `#[message]` structs/enums and named outputs such as
`completed: FileDialogCompletion;`. The generated `.completed()` returns a
one-shot installation closure. `events::map(picker.completed(), send, mapper)`
calls it and returns `Result<()>`. UI accessors add domain checks such as native-path
and finite-number validation.

A producer stores only `Option<ComPtr<__FileDialogCompletionReceiverAbi>>` for
that output. Its generated interface requires an ordinary mutable
`__map_completed(destination)` setter. The implementation checks eligibility,
then assigns `Some(destination.add_ref())`. Structural forwarding calls the
child's setter directly. The object macro has no event-specific binding syntax.

```text
producer commits state → concrete_message.post_to(&mut destination)
                       → generated receiver decodes/copies/retains the message
                       → mapper constructs the consumer's Rust message
                       → Sender::send appends it to the standard channel
                       → runtime later calls Receiver::try_recv and the consumer
```

The mapper runs synchronously at emission and must only translate data or attach
routing IDs. UI operations and application behavior belong in the later mutable
consumer. Captures must be owned or otherwise `'static`. The concrete event,
mapper, sender and application message stay in the receiving binary; only checked
C message data crosses COM. The COM mutable call gate rejects recursive entry.

The producer owns the adapter and its mapper/sender captures. The application or
component owns the receiver; the channel implementation owns its buffer. A second
map replaces future delivery without changing already queued values or routing IDs.
Producer closure releases the adapter without retracting messages already sent.
Dropping the receiver discards queued messages. Later valid emissions still run
the mapper, then discard the translated value when `send` reports disconnection.
That teardown is a successful producer no-op. Mapper captures remain until the
producer closes, is destroyed, or replaces the destination. Capture routing data,
not the producer itself, to avoid a reference cycle.

Outputs targeting one channel preserve sequential UI-thread send order. There is
no global FIFO across channels. Decoding, validation and installation failures
propagate; committed producer state is not rolled back. Request completions use
the same channel rules. Programmatic setters remain silent unless specified otherwise.

## Nested components and timing

A behavioral composite owns a private Rust message enum and a standard receiver
in its model. Child mappings hold senders to that receiver. A model that installs
new child mappings later, such as a file dialog rebuilding directory rows, also
retains a sender. It emits only its declared public results; private messages do
not automatically bubble through ancestors. Structural containers such as
`ScrollArea` need no channel merely because they contain children.

`neocogi_com::events::component_dispatcher(&model, |model| &model.messages, Model::update)`
creates the separate COM driver exposed by the mounted root through
`IComponentEventProvider`. The driver keeps a weak model reference. It claims the
model's `Rc<RefCell<Model>>` borrow before receiving, so a busy model loses no
message. Shared model access is needed when separate controller and mounted-driver
identities access it; its methods otherwise use ordinary `&mut self`. The channel
requires no additional `Rc`, `RefCell`, weak pointer or queue wrapper.

The runtime discovers and schedules these drivers automatically, including during
ordinary `Ui::update`. Component authors supply a receiver and mutable update
method; they do not write queue-draining loops. The schedule is:

1. Synchronize layout and settle pending messages before routing input.
2. Process one raw input, then release traversal borrows.
3. Visit private receivers descendant-first in stable tree order, consuming at most
   one message per component per pass. Skip removed mounts before later delivery.
4. Repeat component/layout work until quiet, then consume one application message.
   Settle its resulting private work before consuming the next application message.
5. Route the next raw input. Publish a renderable frame only after successful completion.

`IEventDispatcher::dispatch() -> Result<bool>` reports whether one message was
consumed. It does no batching, queue-length query or zero-budget probe. The runtime
owns one finite budget for the entire update, including component and application
work. Reaching the exact ceiling returns `EVENT_E_LIMIT` immediately, even if the
last message emptied the channel: proving quiescence requires a later update.
Unattempted messages stay queued and the frame remains uncommitted. Consumer errors
consume only the attempted message, preserve successors, and do not roll back state.
New mounts join the next pass; removed mounts retire their driver tickets. An empty
input queue still gets an eventless update. Recursive update/render and mutable
dispatcher calls reject.

`Ui::update_with_events` borrows the receiver and application consumer for that call.
Its scoped COM adapter revokes retained dispatcher projections on return or unwind.
Immutable asset candidates may be prepared during consumption; installation waits
until update returns. Sending does not schedule a future platform frame; timers
and external data still need host scheduling. Dynamic plugins and static monoliths
use the same transport. Both deployment ABIs remain v1; generated message/output
bindings depend on COM alone and can also be consumed without the channel helper.

See the [min-size experiment results](../../../../docs/EVENT_CHANNEL_EXPERIMENT.md)
for the measured source reduction and deployed binary-size increase.
