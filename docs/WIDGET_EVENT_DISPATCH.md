# Widget-owned event dispatch

`IWidget::dispatch_next(&mut self) -> Result<bool>` is the common private-message
operation. A composite owns its model and receiving endpoint directly. Each call
attempts at most one message; `true` means one message was handled and `false`
means no work was available. A handler error consumes the attempted message and
leaves its successors queued. The COM mutable borrow gate protects the same
allocation used for widget editing and rendering.

Ordinary controls forward their constructor-queued events to their behavioral
parent. They do not acquire private queues merely by implementing this method.
The built-in `HostedWidget` helper and application `LeafContent` helper provide
no-op defaults. Independent COM implementations return `Ok(false)` when they
have no inbox; IDL methods remain declarations, without implementation bodies.

Dispatch belongs outside input, layout and paint traversal. A handler can edit
other widgets through COM while using ordinary `&mut self` for its own state.
It must not call back into its own COM projection to obtain another mutable
borrow. A request to remove its own subtree must stop subsequent dispatch and
let its active call return before running terminal cleanup.

The sole `ComBox` owner replaces a widget close phase. Dropping it rejects new
calls; an admitted handler finishes before its value and private inbox are
destroyed. Child fields own their descendants. Already posted application facts
remain in the application inbox. See [exclusive ownership](EXCLUSIVE_WIDGET_OWNERSHIP.md).

## Ownership and publication

A composite is the mounted widget. FileDialog embeds its Linear root, model and
receiver in one allocation and exposes IWidget, IContainerLayout and IFileDialog.
It mutates its own fields directly and calls distinct children through COM.
`IEventOwner`, its mount/retire methods, `Mount`, and the separate view/controller
wrapper no longer exist. The existing widget mount claim still rejects movement
or second attachment.

The application is not required to be a widget. `IApplicationDispatcher` has
only `dispatch_next`; the runtime borrows it for one update and retains nothing.
The application-owned Controller has ordinary model/receiver fields and no
attachment/retirement phases.

`Loader::compile` constructs a document recipe, weak lookups and one unmounted
root owner. `take_root()` or a consuming mount transfers that owner to the tree.
The metadata can then be dropped independently. Rejected candidates are destroyed
automatically, including when validation retained weak handles.

## Scheduling and cost

The runtime visits actual widgets descendant-first outside traversal and settles
private work before the next application message or raw input event. Topology
changes invalidate the snapshot before its next entry, regardless of the
callback's reported message flag. Its widget references are cleared after each
update, while vector capacity is reused.

Every ordinary leaf therefore incurs a no-op COM dispatch call during a sweep.
This is an explicit simplicity tradeoff: there is no second capability query,
registration table, per-widget delivery ticket or readiness-notification system.
Binary size and frame-time improvements are not claimed without measurement.

Runtime dispatch snapshots contain weak handles and cannot retain removed values.
An admitted `LocalBox`/`WeakLocalBox` guard may delay its own value's destruction,
but removal does not wait, retry, or return busy because of that guard. New calls
are rejected immediately. Parents need not wait for an already executing child:
that child can safely remove its own subtree and return. UI remains thread-local;
thread-safe COM objects use the same nonblocking admission/removal contract.

See the [event guide](../crates/ui/runtime/docs/EVENTS.md) for error semantics,
work budgets, compiled-form lifetime, and application examples, and
[finding 09](COM_UI_ADVERSARIAL_REVIEW_2026-09-26.md#09-ieventowner-means-both-behavior-and-a-lifetime-holder--fixed)
for the superseded design and its history.

## Verification on 2026-09-29

After the exclusive-ownership migration (`2cb90ec9`):

- `cargo test --workspace --offline --no-fail-fast`: 868 passed, 23 ignored,
  no failures.
- Offscreen designer and theme-editor library suites with monolith/embedded assets,
  ignored acceptance tests enabled and the live-edit benchmark excluded: 33 and
  18 passed. These runs include independently built custom-catalog discovery.
- `SDL_VIDEODRIVER=offscreen scripts/test-extension.sh glow`: independently built
  shared plugins loaded, and extension/COM smoke scenarios passed.
- WebGL test-target compilation for the full demo, UI designer and theme editor
  on `wasm32-unknown-unknown`: passed. This is compilation, not browser execution.
- Changed Markdown link/fence validation and `git diff --check`: passed.

The browser compilation check covers its download/export dialogs and preview test
harness, which now transfer owners into windows and retain only weak editing
handles. The native popup regression removes a combo during event forwarding and
checks that its queue is released without a fallible close callback. No binary-size
or frame-time improvement is claimed by these functional checks.
