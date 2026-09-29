> Current origin/lifetime contract: [typed event queues and weak origins](../../../../docs/CLASS_EVENT_STREAMS.md).
> A receiver borrows the typed emitting object and event, then queues an owned
> snapshot with a weak origin. One application COM queue can serve many widgets.

# Typed events and widget-owned dispatch

Widgets post typed facts to their constructor-selected application COM queue. A nested
component consumes its children's messages and emits its own public results
upward. The application consumes the final public messages. A concrete application queue
can implement several `*EventQueue` traits and share one allocation across widgets
through `*_with_queue`. Convenience constructors additionally accept a sender and
typed mapper, allocating a callback receiver. Applications may use `Controller<Model>`; a behavioral composite owns its inbox
and implements `IWidget::dispatch_next` on the same concrete widget.

## Application construction and frames

The application owns its model and receiver in one persistent controller. The
host resolves its COM projection once and borrows it on each update:

```rust,no_run
use neocogi_com::NcStatus;
use neocogi_ui::{events::{self, Controller, EventModel}, IApplicationDispatcherAbi, Size, Ui};

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
    let app = Controller::into_local(Controller::new(App { count: 0 }, messages));
    let owner = app.query::<IApplicationDispatcherAbi>()?;

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

`Controller::new` transfers the model and receiver into application ownership.
It has no attachment or retirement phase. The runtime borrows its dispatcher
for one update without retaining it, so the application may own a `Ui` without
a runtime/controller reference cycle.
The demo host wraps `AppState` with this controller automatically: `Host::run`
still accepts an owned state and receiver. Frame/input/presentation hooks take
short local guards; those guards end before event dispatch.

A frame performs no controller allocation, revocation-state allocation, interface
query, or root reference-count change in this adapter. It still crosses a COM
method and polls the receiver. Channel blocks and owned message payloads may
allocate. The standard-channel helpers and generated forms use unbounded `mpsc::channel`
transport: dispatch work limits and per-value decode budgets do not bound total
queued memory. An application needing a bound must implement its concrete queue
with an explicit nonblocking overflow policy; `post` must not wait for the same
UI thread to drain its inbox. An allocation regression compares 64 warmed ordinary updates with
64 updates using an empty persistent root and checks for zero extra allocations.

## Widget methods and event notifications

Getters and setters belong to the concrete widget class or its appropriate COM
interface. A getter returns a current snapshot; a setter validates and applies
an application request synchronously, returning its result. Event queues receive
facts emitted by the widget and, through the standard adapter, enqueue the
behavioral parent's mapped messages. Similar argument types do not make these
contracts interchangeable. Ordinary state setters do not synthesize user edits.

Selective operations preserve the widget's other current settings. Use
`set_flags`/`clear_flags` for requested interaction bits rather than writing a
snapshot of effective flags back into the widget. Scrolling's own `set_enabled`
controls its scroll model and is separate from those surface flags.
`SplitView::set_position` changes only its nonnegative requested position in one
COM call; it preserves the current axis, minima and handle width. A changed
position cancels divider dragging and invalidates layout without emitting an
event. `set_settings` remains the explicit whole-policy replacement. The combo's
`replace_items`/`replace_menu` validate and commit labels and selection together.

## Construction and posting

Declare cross-COM facts as concrete NCIDL structs/enums. The factory accepts
`EventBinding<Object, Event>`; several event kinds are variants of one message, such as
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

`textbox::queue(sender, mapper)` creates the same adapter separately, for
`textbox_with_queue(text, Some(queue.as_com_ref()))`. `filter_queue` permits dropping
selected variants before enqueueing. `textbox_unobserved(text)` leaves the
widget permanently unobserved and allocates no receiver.

An observed convenience constructor creates one COM receiver holding the mapper
and sender inline, then passes it through the COM factory's typed binding value.
There is no extra event queue per widget. Construction failure releases temporary
captures. Custom factories use the same `EventBinding<Object, Event>`; applications can build
the receiver with `neocogi_com::events::mapped_queue`.

```text
widget commits state
  → calls post(origin, event) on its one concrete IEventQueue<Object, Event>
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
this is a successful producer no-op. Logical widget closure releases its queue,
including when editing handles remain alive. Explicitly retained queue handles
keep their own captures alive. Capture routing data rather than the producer or
owning UI to avoid reference cycles.

Node controls, choice menus, file dialogs, windows, popups, and menu items all
receive their sole optional queue during construction. No public operation can
install or replace it later. Teardown releases it permanently; an unobserved
preview discards events for its entire lifetime.

Generated forms prepare constructor bindings targeting the application's queue,
then query the completed component's concrete widget fields. The loader supplies
no intermediate receiver and performs no visual actions. The application
implements required handler methods and checks reload generations before
dispatching queued facts. See
[class event streams](../../../../docs/CLASS_EVENT_STREAMS.md).

## Composite ownership

A behavioral composite owns its model, receiver and child handles inside its
concrete widget allocation. It implements `IWidget::dispatch_next(&mut self)`;
ordinary controls and structural containers return `Ok(false)` and own no inbox.
The built-in `HostedWidget` and application `LeafContent` helpers supply that
no-op default. IDL methods remain declarations; independent plugins implement
the common method explicitly.

Construct child controls with typed mappings into the composite's inbox. The
composite emits declared public results into its enclosing behavioral parent's
channel. Private messages do not automatically bubble through structural
containers. One output stream still has one constructor-selected consumer.

Window creation and container edits now borrow `&IWidget` directly. There is no
`Mount` value, separate event-owner identity, `Component::new(view, owner)`, or
second mount/retire state machine. The runtime validates host, layout and topology
before calling the widget's one-way `mount` operation. Reattachment remains an
error, including after removal.

`FileDialog` implements `IFileDialog`, `IWidget` and `IContainerLayout` on one COM
allocation. It embeds its Linear root as an ordinary Rust value, so layout
reuse needs no second root object. Its model and receiver are ordinary fields.
Handlers mutate local state through `&mut self` and call distinct child widgets
through COM. Calling the composite's own COM projection from its active handler
would reenter the borrow gate; use direct Rust helpers for self-mutation.

A mounted composite must not strongly retain its owning `Ui`. Use child handles,
weakly backed window capabilities and the construction host as the file dialog
does. Releasing application handles preserves a still-mounted dialog.

## Scheduling, removal and failures

Mounted widget dispatch also runs during ordinary `Ui::update`, without any
application dispatcher:

1. Synchronize geometry and finish deferred widget close outside traversal.
2. Visit widgets descendant-first. After one message, settle closure and geometry
   before reconsidering descendants.
3. When widget work is quiet, call the optional `IApplicationDispatcher` once.
   Settle newly posted private work before its next application message.
4. Route one queued raw input and repeat the boundary before later input.
5. Commit a renderable frame only after successful settlement.

The runtime caches actual widget handles within an update, rebuilding after any
topology change. A callback that removes another widget invalidates the snapshot
before its next entry, even when that callback returns `false`. The cache is
cleared at the end of every update; only its vector capacity survives. It cannot
keep a removed widget alive across idle frames. No separate per-widget delivery
ticket, event-owner allocation, or active bit exists.

This simple protocol polls ordinary widgets' no-op methods too. It avoids an
additional discovery/registration contract, but is not a ready-inbox notification
system. No frame-time or binary-size improvement is claimed without measurement.

Removal closes descendants and then the parent. When a handler removes itself,
its active COM borrow prevents recursive close. The runtime retains pending
close work in descendant-first order and finishes it after the call returns,
including on handler errors. No successor is dispatched to the removed subtree.
Composite close drops its receiver and pending private messages, then releases
its event mapping. An active file request settles cancellation once. Already
posted application messages stay in the application's queue.

A caller-held LocalCom guard can still prevent close at that boundary. Update
returns `NC_E_BUSY`, retains the pending work and retries after the guard ends;
an explicitly closed runtime can finish it on another `close()` call. Final
runtime destruction cannot forcibly enter a borrowed widget: it closes the host
and releases retry references to avoid an ownership cycle. Such an externally
retained widget rejects further work and finishes local cleanup on explicit
close or final release. Release local guards before closing the runtime when
immediate cleanup is required.

One update-wide budget covers handled messages, topology-changing dispatch calls
and menu reconciliation. Exhaustion returns `UI_E_EVENT_LIMIT` before further
work, even if the last message emptied an inbox. Unattempted messages and later
raw input remain queued; state mutations are not rolled back. Reentrant update
or render returns `UI_E_BUSY`; conflicting widget COM calls return `NC_E_BUSY`.
Standard channel capacity remains unbounded: the dispatch budget limits work
per update, not total resident message memory.

## Compiled documents

`Loader::compile` and `compile_with_events` construct a document; `load` and
`load_with_events` decode persisted source first. The result retains its authored
recipe and typed lookups, with no scheduled bookkeeping object or event relay.
`root()` is the actual mountable widget, not an inspection-only proxy.

Dropping the recipe releases its COM references. It never explicitly closes a
root still retained by the runtime or the application. If the root has no other
owner, its ordinary destructor closes descendants. Failed construction closes
partial trees; `ReloadSlot` explicitly closes a complete rejected candidate,
including handles retained by a failing field-validation callback. Successful
replacement leaves closure of the old subtree with the runtime.

## Static and dynamic linkage

Typed mappings, channels and models compile into the binary that owns them.
`IWidget` and the single-method `IApplicationDispatcher` belong to UI core
contracts. Closed queue specializations belong alongside their event types.
The COM process runtime has no UI event loop. Hosts and plugins use the same
contracts and must be rebuilt together after this ABI change.

[Widget dispatch](../../../../docs/WIDGET_EVENT_DISPATCH.md) records the design
and ownership rules. Owned-event measurements
and the [channel experiment](../../../../docs/EVENT_CHANNEL_EXPERIMENT.md) describe
superseded implementations; their measurements are not results for this change.
