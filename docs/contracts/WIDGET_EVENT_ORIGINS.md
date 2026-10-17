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
let state = WidgetState::new(host, options).with_events(queue);
// Inside update or private dispatch, after committing semantic state:
if state.is_observed() {
    state.post(events::activated(label)?)?;
}
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
An unobserved state allocates no snapshot when its caller checks `is_observed`.

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

FileDialog also supports immediate completion from explicit `cancel()` and
`close()` calls outside widget processing. Its request retains a weak dialog
completion identity for that reason. Replacing it with ambient widget context
would reject valid calls or defer/drop committed results during removal. It
retains cyclic initialization for this narrow purpose; it cannot push or pop the
runtime's origin stack. The remaining `#[com_object]` construction choices are a
separate architectural discussion.

## Forms and stale facts

The envelope remains `EventMessage { origin, event }`. Generated Rust forms and
loaded forms use the same receiver protocol; neither needs generated origin
binding code. Loaded documents keep their generation and current-widget checks.
Weak origins do not keep removed widgets alive, and immutable queued payloads
remain readable after removal. Scope restoration never rewrites queued origins.
