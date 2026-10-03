# Widget-owned event dispatch

`IWidgetDispatcher::dispatch_next(&mut self) -> Result<bool>` is the optional private-message
operation queried on the widget's same concrete allocation. A composite owns its
model and receiving endpoint directly. Each call
attempts at most one message; `true` means one message was handled and `false`
means no work was available. A handler error consumes the attempted message and
leaves its successors queued. The COM mutable borrow gate protects the same
allocation used for widget editing and rendering.

Ordinary controls forward constructor-queued events to their behavioral parent.
Passive controls, `HostedWidget` and `LeafContent` do not advertise a dispatcher.
A custom `DispatchingLeafContent` uses `WidgetFactory::dispatching_leaf`; its
controller shares the leaf's existing exclusive COM gate. Independent composites
advertise `IWidgetDispatcher` on their widget allocation only when they own private
behavior. IDL methods remain declarations, without implementation bodies.

Dispatch belongs outside input, layout and paint traversal. A handler can edit
other widgets through COM while using ordinary `&mut self` for its own state.
It must not call back into its own COM projection to obtain another mutable
borrow. Removal of its own subtree is requested through its behavioral parent's
channel; only the parent decides after the active call returns. Posting intent
alone does not discard or stop the widget's remaining private messages.

The sole `ComBox` owner replaces a widget close phase. Dropping it rejects new
calls and destroys its value, private inbox and owned descendants before returning.
A self-removal request runs only after the current handler has returned. Already posted application facts
remain in the application inbox. See [exclusive ownership](EXCLUSIVE_WIDGET_OWNERSHIP.md).

Whole-runtime shutdown is a terminal application decision: `Ui::close()` drops
its context without publishing individual window/popup lifecycle events or
executing pending dispatch. It leaves application-owned queued facts intact.
Required request completion precedes shutdown; see the
[context shutdown contract](UI_RUNTIME_SHUTDOWN.md).

## Ownership and publication

A composite is the mounted widget. FileDialog embeds its Linear root, model and
receiver in one allocation and exposes IWidget, IWidgetDispatcher,
IContainerLayout and IFileDialog.
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

The runtime follows actual surface ancestry, visiting dispatcher descendants before
parents and reverse forest chronology between unrelated siblings. It settles
private work before the next application message or raw input event. Topology
changes invalidate the snapshot before its next entry, regardless of the
callback's reported message flag. Its widget references are cleared after each
update, while vector capacity is reused.

`DispatchWidgets` owns that buffer and exposes an iterator, without an external
index or cursor. Each `next()` skips weak observers that have already expired and
yields an owned weak handle for a live receiver. This is the thread-local UI:
iteration does not pin values, and generated COM entry points retain their normal
lifetime/borrow admission checks. Liveness is checked when advancing, not once at
snapshot construction.

The update checks the scratch collection out of its existing cache and passes it
through scheduling with `&mut`. No `RefCell` borrow spans a callback. On success
or any returned error, the update clears all weak entries, restores the buffer for
capacity reuse and marks the next update for a fresh tree collection.

For a yielded receiver the scheduler executes `dispatcher.dispatch_next()?`. Every
handler error stops the update, including `NC_E_DROPPED` from an expired dependency
and `NC_E_CLOSED` from a closed service. Neither status identifies which object
failed. A dequeued message remains consumed; its successors, later widgets,
application messages and unattempted raw input wait for a later update. No partial
frame is committed and no already-performed handler mutation is rolled back.
See the [lifetime error contract](COM_LIFETIME_ERRORS.md) for exact status meanings.

After a handled message or a topology change, iteration stops so layout can settle
and a new pass can begin. A changed tree is rebuilt first; merely skipping expired
entries would miss newly inserted widgets. A handler that changes another idle
subtree and then fails still propagates its error. The changed tree survives and
the next update rebuilds from it.

```mermaid
flowchart TD
    N[Next observer] --> L{Receiver alive?}
    L -->|no| N
    L -->|yes| D[Call dispatch_next]
    D -->|error| E[Stop update; release snapshot; return error]
    D -->|success| C{Message handled or tree changed?}
    C -->|yes| R[Settle layout; rebuild changed tree; restart iteration]
    R --> N
    C -->|no| N
```

The diagram describes a pass while entries remain. Exhausting the iterator follows
the ordinary outer scheduling rule: settle any reconciliation work, otherwise
attempt the next application message.

Nodes cache a typed weak dispatcher projection at construction. A conservative
host-owned count includes unmounted dispatchers and permits a zero-count fast path;
a positive count always selects from the authoritative mounted forest. This is
capability discovery, not a ready-inbox protocol. No delivery tickets or second
lifetime registry are introduced. Passive trees perform no private dispatch calls.

Geometry mutation has a separate conservative notification contract. Independent
providers keep their sticky `take_change` marker and also call
`IWidgetHost::mark_geometry_changed` after committed geometry edits, including
hidden and unmounted widgets. Notify even when later work fails after commit; a
pre-commit notification is harmless. The runtime alone collects sticky changes
when notified, including render admission and retries. Notifications during a
collection remain pending for the following collection.

Application controllers run `EventModel::settle` when their inbox becomes empty.
Tools prepare geometry/publication there under the existing application mutable
gate; the runtime settles notified work before routing more input or committing
the frame. The demo host calls one outer update. Its `after_update` hook performs
independent preview rendering and paint-only cache publication. `LeafContent::paint`
receives `&mut self` for these caches; host observation still rejects geometry edits.

Runtime dispatch snapshots contain weak handles and cannot retain removed values.
Owner Drop rejects new calls and destroys the complete owned value before returning.
Thread-local removal preflights the existing subtree borrow state and returns Busy
if it would recursively destroy an executing method. A widget posts close intent
to its existing behavioral parent channel. Only the parent decides whether and
when to remove it. Neither intent nor a returned error authorizes removal;
explicit edits already completed inside a failing handler remain committed.
The runtime has no special removal-request queue. Weak local construction
access uses scoped closures instead of escaping guards. ThreadSafe COM Drop waits
for concurrent admitted calls, then destroys on the dropping thread.

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

## Iterator and failure regressions

The September 30 review traced the status suppression to `2cb90ec9`, which replaced
strong dispatch snapshots with weak observers. Weak snapshots correctly stopped
retaining removed widgets, but the scheduler used a method error as an inferred
lifetime check. Separating status names alone did not fix live `CLOSED` errors.

The current regression suite covers:

- Expiry before and between iterator advances, live ordering, weak-only results,
  and scratch capacity reuse.
- Real mounted leaves and composites returning `CLOSED`, `DROPPED`, or an ordinary
  invalid-argument error; successors and application work wait until retry.
- A live handler accessing a genuinely destroyed widget through generated COM.
- Removing another subtree and then failing; its queued work is destroyed and
  retry uses the changed tree without replaying the failed message.
- Queued textbox input staying untouched until the failed dispatch is retried.
- Existing child-first delivery, callback insertion/removal, work-budget limits,
  reentrancy rejection and warmed idle-allocation checks.

These changes add no dispatch ticket, per-widget active flag, new close phase or
second queue. The older verification section above remains historical evidence.

## Executed acceptance — October 1, 2026

Revisions `bd213e1c` (status meanings) and `a805e1dc` (iterator and propagation)
implement this contract on `fix/widget-dispatch-iteration-errors`. Before the
scheduler change, three new regressions failed because live `NC_E_CLOSED` errors
were swallowed; the real expired-dependency test already passed after the status
split. All four behavior regressions and the iterator regression now pass. This
separately demonstrates why changing status names was insufficient.

Executed after the scheduler change:

| Verification | Result |
| --- | --- |
| Runtime and node library suites | 204 runtime + 91 node tests passed, including existing budget, topology, reentrancy and idle-allocation checks. |
| Native demo, theme editor and designer | 6 + 25 + 33 tests passed with `SDL_VIDEODRIVER=offscreen`, `monolith,embed-assets`, normally ignored tests included, benchmarks excluded and one test thread. The designer exercised its independent custom-plugin fixture. |
| WebGL consumers | Demo and both editors compiled with tests for `wasm32-unknown-unknown`, no default features and `webgl`. No browser was executed for this change. |
| Workspace | `cargo check --workspace --all-targets` passed; existing unrelated dead-code warnings remain. |
| Native C ABI | The generated ABI fixture passed again after its expiry-status parameter names were updated. |

The preceding status commit also passed COM core tests/doctests (52), focused
ABI/status/native-C integration tests (10 outer tests), shared/exclusive lifetime
and output-completion tests (44), compiled IDL tests (121), UI facade tests (3),
and cargo-neocogi module tests (3). Its runtime/node suite passed before the five
additional scheduler tests were added. These checks cover the changed status
consumers; they do not claim a new multi-compiler compatibility matrix, sanitizer
run, or binary-size result. Cargo artifacts were cleaned before and after each
commit.

## Inline leaf geometry publication

`DispatchingLeafContent::dispatch_next` receives a borrowed `LeafDispatchContext`
for the owning leaf's existing `WidgetState`. Call `geometry_changed()` immediately
when private behavior commits a preferred-size input. This retains the local
sticky change and notifies the host, allowing layout to settle before the next
queued input. Returning `true` describes private work, not geometry. A callback
returning `false` after a geometry edit still owes publication; returned errors
also retain conservative invalidation. A later ordinary update is not a substitute
for publication before hit testing. The context cannot outlive the admitted call.

Runtime-owned invalidation uses the same scheduling invariant: whenever a callback
failure marks a retained node dirty, it schedules the host collector. Pre-commit
provider failures owe no semantic mutation notification, but must still recover
on a later successful update without an unrelated setter or notification.
