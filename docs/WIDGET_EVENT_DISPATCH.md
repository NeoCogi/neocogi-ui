# Widget-owned event dispatch

`IWidget::dispatch_next(&mut self) -> Result<bool>` is the common private-message
operation. A composite owns its model and receiving endpoint directly. Each call
attempts at most one message; `true` means one message was handled and `false`
means no work was available. A handler error consumes the attempted message and
leaves its successors queued. The COM mutable borrow gate protects the same
allocation used for widget editing and rendering.

Ordinary controls forward their constructor-mapped events to their behavioral
parent. They do not acquire private queues merely by implementing this method.
The built-in `HostedWidget` helper and application `LeafContent` helper provide
no-op defaults. Independent COM implementations return `Ok(false)` when they
have no inbox; IDL methods remain declarations, without implementation bodies.

Dispatch belongs outside input, layout and paint traversal. A handler can edit
other widgets through COM while using ordinary `&mut self` for its own state.
It must not call back into its own COM projection to obtain another mutable
borrow. A request to remove its own subtree must stop subsequent dispatch and
let its active call return before running terminal cleanup.

Closure remains the widget's existing descendant-first, idempotent operation.
Eventful composites additionally disconnect their private receiver, discard its
remaining messages, and release their constructor-selected output mapping.
Messages already posted to the application's inbox belong to the application.


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

`Loader::compile` constructs a document recipe and typed lookup handles. It has
no event interface or hidden lifetime guard. Dropping the result releases its
references; a retained actual root may still be mounted or already belong to the
tree. Unreferenced roots close descendants on final destruction. Reload owns
its publication rollback explicitly and closes rejected candidates even when
validation retained their handles.

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

Busy removed widgets wait for closure in descendant-first order. Application-held
LocalCom guards must be released before immediate cleanup can finish. Final
runtime destruction cannot reenter such guards; it closes the host and releases
pending ownership so retained widgets cannot form a cycle through their host.
Their later explicit close or final release completes local resource cleanup.

See the [event guide](../crates/ui/runtime/docs/EVENTS.md) for error semantics,
work budgets, compiled-form lifetime, and application examples, and
[finding 09](COM_UI_ADVERSARIAL_REVIEW_2026-09-26.md#09-ieventowner-means-both-behavior-and-a-lifetime-holder--fixed)
for the superseded design and its history.

## Verification on 2026-09-28

- `cargo test --workspace --offline`: 797 passed, 22 ignored, no failures.
- Offscreen designer and theme-editor library suites with monolith/embedded assets,
  ignored acceptance tests enabled and timing benchmark excluded: 32 and 17 passed.
- `scripts/test-extension.sh glow` with the offscreen SDL driver: independently
  built shared plugins loaded, and extension/COM smoke scenarios passed.
- WebGL test-target compilation for the full demo, UI designer and theme editor
  on `wasm32-unknown-unknown`: passed. This is not a browser execution result.
- Workspace all-target checking, changed Markdown link/fence validation and
  `git diff --check`: passed. Existing Cargo filename-collision and dead-code
  warnings do not constitute new performance measurements or test failures.

The deployment check also caught three feature-gated smoke calls using the old
mount wrapper; they now pass the widget directly. Browser test compilation
exposed a pre-existing `ComPtr::as_ptr` assertion, updated to the object identity
comparison used by the native test and production check.
