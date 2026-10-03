> Current origin/lifetime contract: [typed event queues and weak origins](../../../../docs/CLASS_EVENT_STREAMS.md).
> A receiver borrows the typed emitting object and event, then queues an owned
> snapshot with a weak origin. One application COM queue can serve many widgets.

# Typed events and widget-owned dispatch

Widgets post typed facts to their constructor-selected application COM queue. A nested
component consumes its children's messages and emits its own public results
upward. The application consumes the final public messages. A concrete application queue
can implement several `*EventQueue` traits and share one allocation across widgets
through `*_with_queue`. Applications may use `Controller<Model>`; a behavioral composite owns its inbox
and implements `IWidgetDispatcher::dispatch_next` on the same concrete widget.

## Application construction and frames

The application owns its model and receiver in one persistent controller. Resolve
its `IApplicationDispatcher` projection once and borrow it for each
`ui.update_with_events(size, dispatcher.as_com_ref())` call.

The [complete Button walkthrough](../../api/docs/button_events.rs) is the exact
source included in the facade's `events` rustdoc and compiled into the runtime
regression. It mounts a real Button, routes Tab/Space input, dispatches its
message, then removes the window while another activation waits in the inbox.
The second fact still owns its payload, but its `WeakComBox<ButtonAbi>` origin
has expired and the example model deliberately ignores it.

Queue `post` receives `ComRef<'_, ButtonAbi>` and captures an exclusive observer
with `origin.downgrade_box()?`. Shared services instead use `WeakComPtr` and
`origin.downgrade()?`. Interface type alone does not select ownership policy;
using shared downgrade for an exclusive Button returns `NC_E_INVALID_ARG`.
A compile-only example cannot prove that runtime requirement.

Run the exact walkthrough from the workspace root:

```sh
cargo test -p neocogi-ui-runtime --lib documented_button_walkthrough
```

## Widget methods and event notifications

Getters and setters belong to the concrete widget class or its appropriate COM
interface. A getter returns a current snapshot; a setter validates and applies
an application request synchronously, returning its result. Event queues receive
facts emitted by the widget and enqueue the behavioral parent's owned messages. Similar argument types do not make these
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

The Rust facade accepts the queue's concrete projection:

```rust,no_run
use neocogi_ui::{Ui, TextboxEventQueueAbi};
use neocogi_com::{ComRef, NcStatus};
fn example(ui: &Ui, queue: ComRef<'_, TextboxEventQueueAbi>) -> Result<(), NcStatus> {
    let name = ui.widgets().textbox_with_queue("", Some(queue))?;
    // Mount name in the view; the application owns the queue's receiver and model.
    Ok(())
}
```

`textbox_unobserved(text)` leaves the widget permanently unobserved and allocates
no receiver. The old sender/mapper constructors and callback receiver helpers are
removed. A queue implementation can match `Changed` and `Submitted` before posting
if its application deliberately ignores one variant; validate both variants first.

```text
widget commits state
  → calls post(origin, event) on its one concrete IEventQueue<Object, Event>
  → application queue acquires the checked event and validates its domain
  → queue captures a weak origin when later identity is needed
  → Sender::send queues the owned private message
  → runtime invokes the application's dispatch_next at a safe boundary
  → Controller receives one value and calls model.handle(&mut self, value)
```

`post` runs synchronously inside the producer's call. It acquires input facts;
UI behavior belongs in the later model handler. Store channel senders and any
immutable queue policy directly in the concrete receiver. Keep mutable command
selection in the application model and change it through `&mut self`. Never call
back into the active emitting widget or retain the behavioral parent's model.

The examples use `std::sync::mpsc::channel()` and one shared queue per behavioral
inbox. Neither threads nor `Message: Send` are needed while endpoints stay on the
UI thread. Channel capacity and overflow behavior belong to the application; a
blocking bounded send could prevent that thread from reaching its receiver.
Posting does not wake the host; external work still needs scheduling.

One channel preserves its sequential UI-thread send order; separate channels
have no global FIFO. Already queued facts retain their payload after producer
closure. The repository's concrete queues treat a disconnected receiver as a
successful no-op. An independent queue can report its own post error, which does
not undo committed producer state. Owner destruction releases the widget queue even while weak editing handles survive. Explicit queue handles retain only their own fields.

A weak origin identifies an allocation, not current form membership. Dropping the
exclusive widget owner expires it, even if weak editing fields remain. Rebuilt controls
must be checked against current fields or command membership, and reloaded forms
also check their generation. A completed dialog result can remain useful after its
origin dies, so expiration is not a universal reason to discard all events.

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
concrete widget allocation. It implements `IWidgetDispatcher::dispatch_next(&mut self)`;
ordinary controls and structural containers expose no dispatcher and own no inbox.
Application content opts in with `DispatchingLeafContent` and
`WidgetFactory::dispatching_leaf`; passive content uses `LeafContent` and `leaf`.
Independent plugins advertise the optional interface on their same exclusive
allocation. Only an actual NOINTERFACE response means the capability is absent.

Node ownership retains the typed weak projection. Each update collects a temporary
postorder snapshot from actual widget/surface parent edges, using reverse chronology
between siblings. Surface raising and callback topology changes invalidate that
selection before another receiver runs. Every returned exit clears its weak entries;
queue-free hosts skip collection entirely. No runtime or tree borrow spans a handler.

Construct child controls with projections of the composite's concrete queue. The
composite emits declared public results into its enclosing behavioral parent's
channel. Private messages do not automatically bubble through structural
containers. One output stream still has one constructor-selected consumer.

Window creation and child insertion consume `ComBox<IWidget>` directly. There is no
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

1. Synchronize geometry outside traversal.
2. Visit widgets descendant-first. After one message, settle topology and geometry
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

The cache is checked out into local update-owned scratch storage. Its iterator
checks each weak receiver on `next()`; scheduling uses `&mut` scratch access and
holds no cache `RefCell` borrow through a handler. Both success and returned error
clear observers and restore reusable capacity.

Every error returned by a live handler stops the update. `NC_E_DROPPED` can come
from another object the handler tried to use; `NC_E_CLOSED` can come from a closed
service. Neither is a scheduler instruction to skip the receiver. The attempted
message stays consumed, while successors, later widget/application work and later
raw input remain queued. Completed handler edits are not rolled back. Retry
rebuilds the current tree. See the [dispatch and iterator contract](../../../../docs/WIDGET_EVENT_DISPATCH.md)
and [status meanings](../../../../docs/COM_LIFETIME_ERRORS.md).

This simple protocol polls ordinary widgets' no-op methods too. It avoids an
additional discovery/registration contract, but is not a ready-inbox notification
system. No frame-time or binary-size improvement is claimed without measurement.

Owner Drop rejects new calls and destroys the value and owned descendants before
returning. Concurrent admitted COM calls finish first; their borrow guards only
release access and never destroy the value. Private inboxes and output queues
are ordinary owned fields. Weak references retain empty storage, not widgets.

Thread-local UI removal checks the subtree's actual borrow state before changing
ownership. An active parent or descendant returns `UI_E_BUSY` and leaves the tree
intact. A child posts close intent through its existing typed parent/application
channel. It does not approve or schedule its own destruction. The parent handles
that message after the child returns and may decline, hide, or call the ordinary
container/window removal operation. Handler errors do not approve pending intent.
There is no `request_close`, runtime removal list, descendant close callback or
deferred-destructor queue. Posting intent does not discard other private messages;
only actual owner removal destroys the inbox and drops what remains pending. Scoped weak
local access follows the same rule: its closure must not destroy its own owner.
Explicitly thread-safe COM owner Drop waits for concurrent admitted calls; the
dropping thread must not hold a resource those calls need to finish.

Explicit file-dialog cancellation posts a result before removal. Destruction
alone disconnects it without posting a new result. Already queued application
facts retain their captured values.

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
`take_root()` transfers its exclusive root once; `root()` returns a weak observer.
Mounting a component consumes that same owner. Dropping recipe metadata after
transfer cannot remove the mounted tree. Dropping an unmounted component destroys
its tree automatically. Failed construction and rejected reload candidates release
their owners, even when validation captured weak handles. Successful replacement
drops the previous root; there is no separate teardown result.

## Static and dynamic linkage

Concrete queue implementations, channels and models compile into the binary that owns them.
`IWidget` and the single-method `IApplicationDispatcher` belong to UI core
contracts. Closed queue specializations belong alongside their event types.
The COM process runtime has no UI event loop. Hosts and plugins use the same
contracts and must be rebuilt together after this ABI change.

[Widget dispatch](../../../../docs/WIDGET_EVENT_DISPATCH.md) records the design
and ownership rules. Owned-event measurements
and the [channel experiment](../../../../docs/EVENT_CHANNEL_EXPERIMENT.md) describe
superseded implementations; their measurements are not results for this change.

## Caption intent and parent policy

`WindowEvent::CloseRequested` means that the user clicked the close control. The
runtime leaves visibility and ownership unchanged. A missing receiver or failed
post does not hide the window. The parent can return `Ok(())` without changing it,
show a confirmation UI and decide later, hide it for reuse, or remove it. The
existing `IWindow::close` is the parent's **immediate removal operation**, not an
intent-posting API or a method on the widget itself. `IWidget` has no close method.

```rust,ignore
match message {
    AppMessage::Window(origin, WindowEvent::CloseRequested) => {
        if may_discard_changes() {
            if let Some(window) = origin.upgrade()? {
                window.close()?; // Parent approves; Drop completes here.
            }
        }
    },
    _ => {},
}
```

The standard file-dialog model interprets its window's caption intent as cancelling
its current file request. It hides the modal, posts one completion, and retains
the widget for reuse. A successful reopen discards old private interaction
messages, including old caption intent; already-posted application completions
remain. Explicit facade `FileDialog::close` is a parent-side convenience that
finishes `cancel()` first and then removes the window, outside the widget call.
The platform/native-window `AppState::close_requested() -> Result<bool, NcStatus>`
is separate and already lets the application approve or decline host shutdown.


## Whole-context shutdown

`Ui::close()` terminates the context after busy preflight and drops its complete
owned state. It does not replay individual window/popup removal, post `Destroyed`
or dismissal events, or dispatch private/application messages. The application
must settle required requests before shutdown. Already queued application facts
remain application-owned; widget-private inboxes and pending raw input are dropped
with their owners. Repeated shutdown succeeds without additional work.

Ordinary `IWindow::close()` still publishes its terminal facts and reports posting
failures after complete removal. See the [shutdown contract](../../../../docs/UI_RUNTIME_SHUTDOWN.md)
for the ownership boundary, callback behavior, history and regression coverage.
