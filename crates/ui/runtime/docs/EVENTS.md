# Typed class streams and owned event controllers

Widgets post typed facts into their owner's standard Rust channel. A nested
component consumes its children's messages and emits its own public results
upward. The application consumes the final public messages. Built-in widget constructors accept the owner's sender and a typed mapper. Behavioral owners use `Controller<Model>` and the
same `IEventOwner` contract at every level.

## Application construction and frames

The application owns its model and receiver in one persistent controller. The
host resolves its COM projection once and borrows it on each update:

```rust,no_run
use neocogi_com::NcStatus;
use neocogi_ui::{events::{self, Controller, EventModel}, IEventOwnerAbi, Size, Ui};

/// Application commands stay in the host binary.
enum Message { Activated(String) }
/// Plain fields are mutated under the controller's existing COM gate.
struct App { count: u32 }
impl EventModel for App {
    type Message = Message;
    fn handle(&mut self, message: Message) -> Result<(), NcStatus> {
        match message {
            Message::Activated(label) => {
                self.count += 1;
                println!("{label}: {}", self.count);
            }
        }
        Ok(())
    }
}

fn example(ui: &Ui, size: Size) -> Result<(), NcStatus> {
    let (send, messages) = std::sync::mpsc::channel();
    let button = ui.widgets().button("Count", send,
        |neocogi_ui::button::Event::Activated(event)| Message::Activated(event.label))?;
    // Insert button into the application's ordinary widget tree before updating.
    let app = Controller::into_local(Controller::root(App { count: 0 }, messages));
    let owner = app.query::<IEventOwnerAbi>()?;

    // Construction happens once; both handles survive every frame.
    for _ in 0..2 {
        ui.update_with_events(size, owner.as_com_ref())?;
        println!("{} activations", app.try_borrow()?.model.count);
    }
    Ok(())
}
```

`EventModel::handle(&mut self, message)` receives exclusive access to ordinary
owned fields. `LocalCom` allows typed access between COM calls; it remembers the
implementation type from construction and uses the same allocation-wide borrow
gate as foreign calls. It does not downcast an interface. End local guards before
calling an update that will dispatch that controller. A conflicting call returns
`NC_E_BUSY` before receiving anything.

`Controller::root` starts claimed by the host and cannot attach to a widget.
The runtime borrows this root only for the update and never retains it, so the
application may own a `Ui` without creating a runtime/controller reference cycle.
The demo host wraps `AppState` with this controller automatically: `Host::run`
still accepts an owned state and receiver. Frame/input/presentation hooks take
short local guards; those guards end before event dispatch.

A frame performs no controller allocation, revocation-state allocation, interface
query, or root reference-count change in this adapter. It still crosses a COM
method and polls the receiver. Channel blocks and owned message payloads may
allocate. An allocation regression compares 64 warmed ordinary updates with
64 updates using an empty persistent root and checks for zero extra allocations.

## Construction and posting

Declare cross-COM facts as concrete NCIDL structs/enums. The factory accepts
`EventBinding<Event>`; several event kinds are variants of one message, such as
`TextboxEvent::Changed` and `TextboxEvent::Submitted`. Its behavioral parent
supplies the typed receiver before construction. A published widget has no
receiver-installation method, including through inherited projections.

The Rust facade supplies the standard channel adapter:

```rust,no_run
use neocogi_ui::{textbox, Ui};
use neocogi_com::NcStatus;
enum Message { Name(String), Submit(String) }
fn example(ui: &Ui) -> Result<(), NcStatus> {
    let (send, messages) = std::sync::mpsc::channel();
    let name = ui.widgets().textbox("", send, |event| match event {
        textbox::Event::Changed(value) => Message::Name(value.text),
        textbox::Event::Submitted(value) => Message::Submit(value.text),
    })?;
    // Retain messages in the owner's normal controller and mount name in its view.
    Ok(())
}
```

`textbox::sink(sender, mapper)` creates the same adapter separately, for
`textbox_with_sink(text, Some(sink.as_com_ref()))`. `filter_sink` permits dropping
selected variants before enqueueing. `textbox_unobserved(text)` leaves the
widget permanently unobserved and allocates no receiver.

An observed convenience constructor creates one COM receiver holding the mapper
and sender inline, then passes it through the COM factory's typed binding value.
There is no extra event queue per widget. Construction failure releases temporary
captures. Custom factories use the same `EventBinding<E>`; applications can build
the receiver with `neocogi_com::events::mapped_sink`.

```text
widget commits state
  → calls post on its one concrete IMessageSink<Event>
  → generated codec validates/copies/retains the concrete message
  → UI domain validation precedes the owner's typed mapper
  → Sender::send queues the owned private message
  → runtime invokes the owner's dispatch_next at a safe boundary
  → Controller receives one value and calls model.handle(&mut self, value)
```

Mappers execute synchronously inside the producer's call. They translate values
and attach routing information; UI behavior belongs in the later model handler.
Captures must be owned or otherwise `'static`. The ordinary channel adapter adds
no `Any`, local pointer cast, or handwritten unsafe callback machinery. COM object
allocation, vtables and wire codecs still use the shared checked ABI machinery.
The existing `LocalCom` controller access separately contains unsafe pointer
reconstruction under its borrow gate; the stream adapter does not replace it.

Use `std::sync::mpsc::channel()`. Clone its sender for each observed child.
Neither threads nor `Message: Send` are needed while endpoints stay on the UI
thread. A blocking bounded sender could prevent that thread from reaching its
receiver. Posting does not wake the host; external work still needs scheduling.

An owner receives its channel's sequential UI-thread send order. Separate channels
have no global FIFO. Already queued facts keep their values and routing after
producer closure. Dropping the receiver discards pending work. Subsequent valid
posts still run the mapper and discard the translated message on disconnection;
this is a successful producer no-op. Logical widget closure releases its sink,
including when editing handles remain alive. Explicitly retained sink handles
keep their own captures alive. Capture routing data rather than the producer or
owning UI to avoid reference cycles.

Node controls, choice menus, file dialogs, windows, popups, and menu items all
receive their sole optional sink during construction. No public operation can
install or replace it later. Teardown releases it permanently; an unobserved
preview discards events for its entire lifetime.

Generated forms prepare constructor bindings targeting the application's queue,
then query the completed component's concrete widget fields. The loader supplies
no intermediate receiver and performs no visual actions. The application
implements required handler methods and checks reload generations before
dispatching queued facts. See
[class event streams](../../../../docs/CLASS_EVENT_STREAMS.md).

## Composite ownership

A behavioral composite uses `Controller::new(model, receiver)` before mounting.
Construct its children with sinks targeting that receiver. Its model emits declared public outputs
into its enclosing owner's channel. Exposing a particular child output directly
to the application is an explicit binding choice; private messages do not bubble
automatically through unrelated containers. A structural `ScrollArea`, layout,
or chrome needs no event owner solely because it contains children.

The static `Component::new(view, owner)` facade carries a normal widget and the
controller's `IEventOwner` projection. Pass it to the same window, insertion, or
replacement APIs used for ordinary widgets. Across COM, these calls borrow a
typed `Mount` message:

- `Mount::Widget { view }` attaches an ordinary view whose outputs target an enclosing owner.
- `Mount::Component { view, owner }` also attaches persistent local behavior.

The view and controller must have different COM identities so a handler can edit
its view without recursively entering its own allocation. The controller may
implement public editing interfaces on its own identity. The file dialog exposes
`IFileDialog` and `IEventOwner` on one allocation and delegates owner methods to
its embedded `Controller<Model>`. Its ordinary Linear view needs no forwarding
widget or event-provider capability.

The runtime validates host, layout, child ownership and identity before calling
`owner.mount(view)` as the final attachment step. The controller checks its own
phase, calls `view.mount()`, then commits its phase. A rejection preserves the
unclaimed side. Attachments are single-use; retained handles do not permit
reparenting. Clone a `Component` only to retain access to the same relationship.

The mounted node retains its owner until retirement. Component models should
retain widget/window/construction capabilities whose back-references are weak,
rather than their owning `Ui`; retaining that UI would create a strong cycle.
The file dialog follows this rule and survives releasing every client handle
while its view remains mounted.

## Scheduling, removal and failures

The runtime uses the same dispatch contract for mounted components and the
application. Mounted owners also run during ordinary `Ui::update`:

1. Synchronize geometry and finish deferred retirement outside traversal borrows.
2. Visit mounted owners descendant-first. After one delivered message, return to
   geometry/retirement and reconsider descendants before another owner runs.
3. Once component work is quiet, deliver one application message. Settle any new
   private work before delivering the application's successor.
4. Route one queued raw input and repeat these boundaries before later input.
5. Publish a renderable frame after successful completion. With no raw input,
   perform an eventless update and settle its work too.

The owner order is cached until topology changes; its vector capacity is reused.
The runtime retains one local delivery ticket while calling out and holds no
traversal or cache borrow across a handler. New mounts join the next boundary.
An empty inbox still requires a poll; this is not a ready-owner notification system.

Removal first invalidates the mounted delivery ticket, closes descendants and
the view, and queues model retirement. Retirement runs after the active handler
returns, including on a recoverable handler error. `Controller::retire` commits
its terminal phase and drops its inbox before invoking `EventModel::retire` once.
An active file request posts one cancellation; already completed parent messages
remain in their original channel. Retained COM handles cannot resume private
delivery. Explicit retirement and final controller destruction are idempotent.

A local guard that blocks the retirement call yields `NC_E_BUSY`; the runtime
keeps that retirement pending and retries after the guard ends. A closed runtime
can finish such pending work on another `close` call. A model's terminal cleanup
error is reported without repeating its committed cleanup body.

One update-wide budget (`EVENT_DISPATCH_LIMIT`) covers component, application and
menu reconciliation work. Reaching the ceiling returns `UI_E_EVENT_LIMIT`, even
if that message emptied the queue: proving quiescence would require another
foreign call. Unattempted messages remain queued and the frame is uncommitted.
A handler error consumes its attempted message, preserves successors and later
raw input, and does not roll back state. Update/render reentry rejects with
`UI_E_BUSY`; conflicting calls to an active controller reject with `NC_E_BUSY`.
Install immutable asset candidates after update returns.

## Static and dynamic linkage

Typed mappings, channels and concrete controllers compile into their owner's
binary. `IEventOwner` and `Mount` belong to UI core contracts. Closed generic stream and sink
contracts live alongside each producer; domain-checking channel shims belong to
the static UI facade. Generic stream binding belongs to the COM facade. The COM process runtime has no event
loop or UI dispatch adapter. No additional event shared library is introduced.
Dynamic plugins and monoliths use the same contracts. Rebuild hosts and plugins
together after these signature changes; no legacy dispatcher bridge remains.

See owned-event measurements for
before/after min-size builds and validation. The older [channel experiment](../../../../docs/EVENT_CHANNEL_EXPERIMENT.md)
records a different historical change and is not the current controller design.
