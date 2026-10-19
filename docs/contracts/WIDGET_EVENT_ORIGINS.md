# Owner-scoped widget event origins

The retained runtime already owns the widget being updated. It selects that
widget as the current event origin for the synchronous `IWidget::update` call,
and selects each composite for its `IWidgetDispatcher::dispatch_next` call.
Ordinary widgets keep their constructor-selected receiver, not a weak pointer to
themselves. This changes the pre-release UI contract without changing COM runtime
ABI v1, core ABI 17 or domain descriptor ABI 10.

## Posting and ownership

A Rust provider configures its existing state before publishing the widget:

```rust,ignore
let mut state = WidgetState::new(host, options).with_events(queue);
// Inside update or private dispatch, after committing semantic state:
state.post(events::activated()?)?;
// The containing widget is allocated with into_box(value), without cyclic setup.
```

`WidgetState` reuses its existing host reference. At the COM boundary, the
posting-only operation is:

```rust,ignore
interface IWidgetHost {
    fn post_event(&self, receiver: &IEventReceiver, event: ComBox<IEvent>) -> Result<()>;
    // Existing resource, lifetime and geometry operations.
}
```

This host operation reads the runtime's current origin and calls the ordinary
`IEventReceiver::post(origin, event)`. The receiver captures the weak identity
immediately and moves the exclusive event owner. It never looks up an origin
later when consuming the queue. No active origin returns `NC_E_INVALID_ARG` and
the original event owner. Layout/paint and closed-runtime checks still apply;
receiver rejection also returns the same owner. `WidgetState::post` reports the
status and drops a rejected snapshot; committed widget state remains committed.
Posting consumes an already constructed event. An absent destination drops it
immediately; there is no lazy constructor or producer observation check.

Native `Widget::update` returns `Result<Change, NcStatus>` and uses `?` directly.
`WidgetUpdateCtx` contains interaction/resource observations, with no `posted`
error accumulator or update `finish`. A widget records committed geometry in
its `WidgetState` before constructing or delivering a snapshot. On failure,
`IWidget::take_change` still reports that geometry, semantic edits remain
committed, and no later event in the same update is attempted. For example, a
ready paste followed by Enter captures changed then submitted on success;
rejection of changed stops before constructing submitted. Failure does not
replay that edit or its event on the next eventless update.

Recoverable failures may use `state.post(events::error(status, message)?)?` in
the same scope. The host attaches the offending widget exactly as for any other
fact; no widget label, extra source argument or alternate error channel is
needed. The queued `IErrorEvent` remains readable after that source expires.
Infrastructure failures still propagate a status, including failure to deliver
the report itself; there is no recursive report attempt. See
[reported failures](COM_EVENT_INTERFACES.md#reported-failures).

Directly calling an observed widget's update outside an owning dispatch scope
cannot manufacture an origin. Applications deliver input through `Ui::update`
or `update_with_events`. Native algorithm tests use the feature-gated runtime
origin driver to supply the owner boundary their direct calls deliberately omit.
This driver uses the production host state and exports no COM control interface.

## Scope authority

The runtime privately owns one `EventOriginOwner` in its host state. Its
`with_origin(origin, closure)` pushes a frame, invokes synchronous code and pops
on every return, including a Rust unwind. Nested scopes restore the enclosing
origin rather than resetting to an assumed idle value. COM panic policy remains
abort; the scope helper does not make foreign panics recoverable.

Only an owner holding that Rust value can establish a scope. The widget-facing
host and receiver provide no push, pop or scope interface, including through
QueryInterface. Constructing another owner creates a separate empty stack and
grants no authority over an existing host or upstream queue. Raw push/pop and
escaping guards are deliberately absent.

A short `RefCell` borrow is required because posting and synchronous nested
owner calls share the authority. No stack borrow spans receiver or widget code.
The vector retains capacity after a call; allocation failure when entering a new
frame returns `NC_E_OUT_OF_MEMORY` before entering the widget.

All independently loaded widget DLLs call the same owning UI host over COM.
There is no thread-local current-widget variable copied into each library and
no additional process COM runtime. Two UI runtimes have independent origin
stacks even when their widgets share an application receiver.

## Composite boundaries

TextArea owns its private editor inbox. Combo owns its private choice-menu
inbox. Private producers capture their facts into those inboxes; at a normal
runtime dispatch boundary, the public composite forwards the same event owner
through its own `WidgetState`. The runtime's current origin is then TextArea or
Combo. Neither needs a cached weak self-reference, and Combo needs no separate
forwarding COM object.

Forwarding moves the original immutable snapshot without rereading current
editor/menu state. Private FIFO order is preserved. Forwarding is scheduled
work and charges the existing shared dispatch budget. Public composite output
occurs during composite dispatch, after traversal/menu reconciliation; there is
no global ordering guarantee across separate private and application inboxes.
A receiver attempting to remove the currently dispatching composite gets Busy;
the owning application can retry removal after the call returns. Private child
owners/inboxes are dropped before the composite's upstream receiver.

This is explicit composite behavior, not automatic event bubbling. Ordinary
container children still identify themselves and use their selected receivers.
A composite's inbox grants no control of its upstream origin stack.

## Producers outside widget dispatch

Shared windows, popups, menu items and choice-menu services can publish from
manager operations or explicit application calls. Their existing `EventPort`
therefore continues carrying a weak service identity to the generic COM receiver.
Their factories allocate fully initialized objects normally, obtain their weak
interface identities, and connect the ports before exposing the capabilities.
Window/popup factories hold exclusive manager access across registration and
connection; registration itself emits no events. Menu items enter the declaration
only after their port and retained public capability have been connected.

FileDialog also supports immediate completion from explicit `cancel()` and
`close()` calls outside widget processing. Its request retains a weak dialog
completion identity for that reason. Replacing it with ambient widget context
would reject valid calls or defer/drop committed results during removal. Its
factory obtains that identity after ordinary allocation, connects completion
through checked local access, then registers the dialog's root with the runtime.
It cannot push or pop the runtime's origin stack. A weak completion identity
requires neither cyclic construction nor access to an uninitialized Rust value.

## Forms and stale facts

The envelope remains `EventMessage { origin, event }`. Generated Rust forms and
loaded forms use the same receiver protocol; neither needs generated origin
binding code. Loaded documents keep their generation and current-widget checks.
Weak origins do not keep removed widgets alive, and immutable queued payloads
remain readable after removal. Scope restoration never rewrites queued origins.
