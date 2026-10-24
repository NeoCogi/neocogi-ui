# COM events and widget-owned dispatch

The [event object contract](../../../../docs/contracts/UI_EVENT_INTERFACES.md)
defines a common `IEventReceiver`, exclusive `ComBox<IEvent>` transfer, and weak
widget/service origins. Any number of widgets share the same receiver projection.
The application owns its inbox; a behavioral composite can own a private inbox
and expose `IWidgetDispatcher::dispatch_next` on its widget allocation.

## Application construction and frames

The application owns its processor and receiver in one persistent dispatcher. Resolve
its `IApplicationDispatcher` projection once and borrow it for each
`ui.update_with_events(size, dispatcher.as_com_ref())` call.

`MessageProcessor::input_boundary` is optional Rust bookkeeping backed by
`IApplicationDispatcher` version 3. Begin/End follow actual consumed input, not
the host's earlier polling callback. Descendant and application dispatch settles
before End; End is attempted after failures too, preserving the first error.
These hooks record ownership only, without geometry edits or more UI input.
For deferred decoding, put markers and event snapshots in the same concrete
FIFO and capture each command's group before consuming later markers. Eventless
work has no scope. Each consumed scope with an application costs one unit of
the shared update budget. See the [dispatch contract](../../../../docs/contracts/WIDGET_EVENT_DISPATCH.md).

The [complete Button walkthrough](../../api/docs/button_events.rs) is the exact
source included in the facade's `events` rustdoc and compiled into the runtime
regression. It mounts a real Button, routes Tab/Space input, dispatches its
message, then removes the window while another activation waits in the inbox.
The second fact still owns its payload, but its `WeakComBox<ButtonAbi>` origin
has expired and the example model deliberately ignores it.

Queue `post` captures a checked `EventOrigin` and moves the COM event owner.
An event stays readable when its source expires. Its handler decides whether to
ignore a removed source or consume the captured fact. `EventMessage::query`
returns a `ComRef` borrowed from the message's event owner. An observer that must
outlive the message explicitly uses `message.event.query_weak`; it expires when
the event owner is dropped. Neither operation clones the event's ownership.

Run the exact walkthrough from the workspace root:

```sh
cargo test -p neocogi-ui-runtime --lib documented_button_walkthrough
```

## Widget methods and event notifications

Getters and setters belong to the concrete widget class or its appropriate COM
interface. A getter returns a current snapshot; a setter validates and applies
an application request synchronously, returning its result. Event queues receive
the original COM event owner and weak origin for deferred application handling.
Ordinary state setters do not synthesize user edits.

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

Concrete event interfaces derive from `IEvent` and expose captured data through
immutable getters. `EventBinding` selects delivery policy; its Observed case
contains the common receiver. `ui_event_sources!` supplies authoring metadata. A published widget has no receiver-installation method.

```rust,no_run
use neocogi_ui::{Ui, events::IEventReceiverAbi};
use neocogi_com::{ComRef, NcStatus};
fn example(ui: &Ui, queue: ComRef<'_, IEventReceiverAbi>) -> Result<(), NcStatus> {
    let name = ui.widgets().textbox_with_queue("", Some(queue))?;
    Ok(())
}
```

`textbox_unobserved(text)` permanently disables delivery. User edits still construct
their snapshots; mutable posting drops those owners immediately.
The standard `events::channel()` helper receives `EventMessage` directly, with no
widget-specific receiver implementation. A custom receiver may add an application
envelope, but keeps the exclusive COM event until handling.

```text
runtime scopes current widget for update/private dispatch
  → widget commits state and captures immutable event
  → WidgetState posts through its existing IWidgetHost
  → host captures current weak origin and transfers ComBox<IEvent> through IEventReceiver
  → receiver moves EventMessage into its inbox
  → runtime reaches a safe dispatch boundary
  → processor queries event interfaces and handles the message
```

The widget-facing host exposes posting only; it cannot push/pop the origin
stack. Ordinary controls need no cached weak self or cyclic construction.
TextArea and Combo own private inboxes and republish their captured facts during
composite dispatch, under their public identities. Shared services and immediate
FileDialog cancel/close completion retain explicit weak identities because those
calls also run outside widget processing. See the
[origin contract](../../../../docs/contracts/WIDGET_EVENT_ORIGINS.md).

Posting runs during the producer's call. Keep channel senders and immutable policy
in the receiver; defer command selection and widget calls to the handler. Neither
threads nor `Message: Send` are needed when endpoints stay on the UI thread.
Capacity and overflow behavior belong to the application. A blocking bounded send
would prevent that thread from reaching its receiver. Posting does not wake the host.

Each inbox preserves its sequential UI-thread order; separate inboxes have no
global FIFO. The standard channel returns `NC_E_CLOSED` with the original event
owner on disconnection. Other receivers choose their own error policy. Posting
errors do not undo committed producer changes. Widget destruction releases the
receiver even if weak editing handles survive.

A weak origin identifies an allocation, not current form membership. Dropping the
exclusive widget owner expires it, even if weak editing fields remain. Rebuilt controls
must be checked against current fields or command membership, and reloaded forms
also check their generation. A completed dialog result can remain useful after its
origin dies, so expiration is not a universal reason to discard all events.

Node controls, choice menus, file dialogs, windows, popups, and menu items all
receive their sole optional queue during construction. No public operation can
install or replace it later. Teardown releases it permanently; an unobserved
preview discards events for its entire lifetime.

Applications prepare `EventBindings` targeting their common receiver before
document construction. Optional generated forms query the completed component's
concrete widget fields. The application checks reload generations and weak origins,
queries concrete event interfaces, and handles queued facts at its dispatch boundary.
See [class event streams](../../../../docs/contracts/CLASS_EVENT_STREAMS.md).

## Composite ownership

A behavioral composite owns its model, receiver and child handles inside its
concrete widget allocation. It implements `IWidgetDispatcher::dispatch_next(&mut self)`;
ordinary controls and structural containers expose no dispatcher and own no inbox.
Application content opts in with `DispatchingLeafContent` and
`WidgetFactory::dispatching_leaf`. Private content publishes committed geometry
through the callback's borrowed `LeafDispatchContext::geometry_changed`; it must
not wait until a later ordinary update to affect the next queued input. Passive content uses `LeafContent` and `leaf`.
Independent plugins advertise the optional interface on their same exclusive
allocation. Only an actual NOINTERFACE response means the capability is absent.

Node ownership retains the typed weak projection. Each update collects a temporary
postorder snapshot from actual widget/surface parent edges, using reverse chronology
between siblings. Surface raising and callback topology changes invalidate that
selection before another receiver runs. Every returned exit clears its weak entries;
queue-free hosts skip collection entirely. No runtime or tree borrow spans a handler.

Construct child controls with the composite's common `IEventReceiver`. The
composite emits declared public results into its enclosing behavioral parent's
channel. Private messages do not automatically bubble through structural
containers. One output stream still has one constructor-selected consumer.

Window creation and child insertion consume `ComBox<IWidget>` directly. There is no
`Mount` value, separate event-owner identity, `Component::new(view, owner)`, or
second mount/retire state machine. The runtime validates host, layout and topology
before calling the widget's one-way `mount` operation. Reattachment remains an
error, including after removal.

`FileDialog` implements `IFileDialog`, `IWidgetDispatcher`, `IWidget` and `IContainerLayout` on one COM
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
2. Visit optional private dispatchers in actual ancestry order, child first. After one message, settle topology and geometry
   before reconsidering descendants.
3. When widget work is quiet, call the optional `IApplicationDispatcher` once.
   Its version 3 result distinguishes `MessageHandled`, `PendingChangesApplied`,
   and `Idle`. The dispatcher handles one message, or calls the processor's pending-change
   hook when no message is available. Both work outcomes consume budget and revisit
   private work/layout before another application visit or later raw input. `Idle`
   also requires quiet geometry/topology notifications before the boundary ends.
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
rebuilds the current tree. See the [dispatch and iterator contract](../../../../docs/contracts/WIDGET_EVENT_DISPATCH.md)
and [status meanings](../../../../docs/contracts/COM_LIFETIME_ERRORS.md).

Passive widgets advertise no private dispatcher and receive no empty COM calls.
Nodes own typed weak capability projections, while a conservative zero-dispatcher
count skips collection. Dispatch entries remain transient; no ready-inbox system
or second lifetime registry exists. Application dispatchers can synchronize outer
geometry in `MessageProcessor::apply_pending_changes`, called when their inbox is empty
or disconnected. `ApplyOutcome::ChangesApplied` reports a consumed request;
`NothingPending` reports an idle invocation. The hook can run several times in one
update, including before the first input. Independent
providers must retain sticky changes and conservatively notify
`IWidgetHost::mark_geometry_changed` on committed geometry edits. The runtime is
the sole sticky-collection owner, including retries and render admission.

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
menu reconciliation and consumed input scopes with application bookkeeping. Exhaustion returns `UI_E_EVENT_LIMIT` before further
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

Concrete receivers, channels and message processors compile into the binary that
owns them. `IEvent` and `IEventReceiver` belong to UI core; concrete snapshot interfaces
belong to the UI core, nodes or plugin contracts that define their facts. `IWidget`
and `IApplicationDispatcher` belong to UI core contracts. Its `dispatch_next`
method executes deferred messages; `input_boundary` records notifications around
each consumed input outside traversal borrows. The two methods retain their
distinct dispatch and input-bookkeeping responsibilities.
The COM process runtime has no UI event loop. Hosts and plugins use the same
contracts and must be rebuilt together after this ABI change.

[Widget dispatch](../../../../docs/contracts/WIDGET_EVENT_DISPATCH.md) records the design
and ownership rules. The [channel experiment](../../../../docs/experiments/README.md#standard-channel)
describes a superseded implementation rather than current event performance.

## Caption intent and parent policy

`IWindowCloseRequestedEvent` means that the user clicked the close control. The
runtime leaves visibility and ownership unchanged. A missing receiver or failed
post does not hide the window. The parent can return `Ok(())` without changing it,
show a confirmation UI and decide later, hide it for reuse, or remove it. The
existing `IWindow::close` is the parent's **immediate removal operation**, not an
intent-posting API or a method on the widget itself. `IWidget` has no close method.

```rust,ignore
if let Some(origin) = message.origin.service::<IWindowAbi>()? {
    if message.query::<IWindowCloseRequestedEventAbi>()?.is_some() && may_discard_changes() {
        if let Some(window) = origin.upgrade()? {
            window.close()?; // Parent approves; Drop completes here.
        }
    }
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
failures after complete removal. See the [shutdown contract](../../../../docs/contracts/UI_RUNTIME_SHUTDOWN.md)
for the ownership boundary, callback behavior, history and regression coverage.
