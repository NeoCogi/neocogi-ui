# COM widget and application contracts

These are the author-facing rules for the migration described in [COM_DESIGN.md](COM_DESIGN.md). [COM_MIGRATION.md](COM_MIGRATION.md) records which implementation milestones are complete. The contract crate contains fixed interfaces and values; the application-facing Rust facade contains typed conveniences. Neither exposes implementation state through `Any`, untyped properties, or native Rust GPU objects.

## Linking and ownership

Native applications link the Rust facade and one shared COM runtime, then load the UI and selected platform/rendering plugins. Asset construction/theme parsing is an optional plugin. The final shared-library assembly exports its descriptor explicitly. A browser build statically registers the same plugin descriptors and links one runtime implementation. Do not link a separate runtime implementation into each native plugin. Plugins remain loaded for the process lifetime; this design does not support hot unloading.

A `ComPtr` keeps an allocation alive. It does not keep a widget attached, a logical window open, or a subscription connected. Closing a window/subtree ends its input, layout, editing, and painting eligibility even when the application retains typed pointers. Further operations report the documented closed status. Final Release frees allocation storage and must not submit GPU work, invoke application callbacks, or substitute for a fallible close/finish operation.

Initially each widget mounts once, under one parent in one runtime. Attachment checks duplicate membership, cycles, and owner compatibility before publishing an edit. Container track/span settings belong to the parent-child relationship. Do not place them in a generic widget property map. Foreign-runtime widgets and foreign-renderer resources are errors, even if their numerical IDs happen to match.

IDs are scoped to their owner. A static counter compiled into several shared libraries is several counters. Do not assume it provides process-wide identity. QueryInterface's canonical IUnknown projection identifies a live COM object; arbitrary interface-header addresses can differ on the same object.

Default UI and GPU pointers are thread-local. Atomic reference counts do not make UI methods, OS windows, or GPU contexts thread-safe. Do not strengthen a threading marker without the corresponding implementation guarantee, including final destruction on another thread.

## Update, measurement, and external state

The update result describes geometry: preferred measurement or child-placement inputs changed, or they did not. It is not a generic "something happened" flag. A widget may emit an event, change hover/focus painting, or update a paint cache while reporting no geometry change.

The runtime has additional independent invalidation sources:

- Programmatic setters record sticky geometry changes. Several edits before update may coalesce, but the change must survive until the runtime observes it.
- A different measurement constraint, including a resize, invalidates the corresponding cached measurement.
- A skin/font/atlas revision can change glyph, border, icon, padding, or other measurement inputs.
- Child insertion/removal and parent-owned track/span changes invalidate ancestor measurement and placement.
- A descendant's pending change must be collected before accepting a parent's cached result, including when that descendant's update was skipped.

Widget authors must report changes honestly. A no-change result cannot detect mutations to application-owned state read behind a separate shared reference. If a model, shared string, image geometry, font choice, or other external input changes without going through a widget setter, the author must explicitly synchronize the widget and request geometry invalidation before relying on layout. The runtime does not inspect arbitrary user state or infer its dependencies.

For example, changing a button from "Save" to "Save all documents" changes its preferred width. Its setter marks geometry dirty. Changing only a caret blink phase normally affects paint and does not change preferred size. A model containing the button's future text changing elsewhere is invisible until the application applies the new label or explicitly invalidates its measurement.

Do not rely on hidden, clipped, disabled, or otherwise ineligible widgets receiving an update call on every application frame. Update eligibility follows routing/traversal policy, not a timer service. Keep required model synchronization in application code, and carry its geometry invalidation until the runtime collects it. A widget returning no change does not suppress descendant updates or automatically certify the entire subtree's cache.

Call the runtime's update/synchronization operation after programmatic changes and resize before using layout results or rendering. Painting is observational: it may update private rendering caches, but it must not change semantic state, topology, focus/capture, measurement inputs, or publish application-coordination events. A missing or invalidated update commit causes rendering to report that update is required.

Measurement constraints and allocated rectangles have different meanings. Unbounded is different from bounded zero. A child reports desired size under constraints; the parent then assigns an exact rectangle. Children must not silently reapply a sizing policy to that final allocation.

## Borrowed calls, paint, and layout

`ComRef` borrows a live interface for a call; it can be retained with `add_ref()`. Therefore it is not an exclusive, non-retainable Rust borrow. Never create a COM implementation containing a stack `&mut Painter`, borrowed child storage, a native frame borrow, or a borrowed application `&mut State` and assume a ComRef prevents it escaping.

Paint/layout services own their storage and issue invocation tokens. The static `Painter<'a>`/layout convenience wrapper hides the token from ordinary drawing calls, but the implementation checks it on every ABI operation. The token expires on every exit, including errors. An old token never becomes valid again in a later frame. Retaining the object remains memory-safe and retaining its expired authority does not permit further painting/layout.

Scoped layout permits a container to measure/place its indexed children. It does not permit topology edits during layout or unrestricted access to the runtime tree. Recursive child calls restore the enclosing scope correctly before continuing the parent. Applications and plugins must not fabricate authority or treat an invocation token as a durable resource identifier.

Release mutable RefCell/lock borrows before calling an external widget, container, painter, or application handler. Reentrant traversal and edits forbidden during painting/layout produce a contract error. An implementation must not trigger a RefCell panic across the C ABI while trying to reject such a call. Panics are implementation faults and follow the COM abort policy; they are not converted into recoverable statuses.

## Typed events and application state

`EventSession<State>` is the static application adapter. It owns the concrete
application state and one registration sequence. Each handler is an owned
`EventHandler<State, Event>` implementation stored inline in a local generic COM
sink. The approved `FnMut(&mut State, &Event) -> Result<(), NcStatus>` convenience
implements that same trait; captures and Rust state never cross the ABI. Generic
Rust list data also stays in the application; cross-plugin domain data needs a
concrete domain interface and owner-scoped keys.

```rust
use microui_redux::{Button, EventHandler, EventSession};
use nc_com::NcStatus;
use microui_redux::events::Activation;

struct Submitted { button: Button }
impl EventHandler<u32, Activation> for Submitted {
    fn handle(&mut self, count: &mut u32, event: &Activation) -> Result<(), NcStatus> {
        *count += 1;
        // The event owns the label captured at activation, even after this setter.
        self.button.set_label(&format!("{}: {}", event.label, count))
    }
}

let events = EventSession::new(0u32);
let lease = events.subscribe_activation(&button, Submitted { button: button.clone() })?;
ui.update_with_events(drawable_size, &events)?;
println!("{} submissions", *events.state()?);
ui.render(drawable_size, renderer.as_com_ref())?;
lease.cancel()?; // Explicit cancellation; dropping the session also cancels its leases.
```

The event capabilities are separate from editing capabilities and may be queried
once when subscribing. The session offers these concrete families:

| Registration | Producers | Captured payload |
| --- | --- | --- |
| `subscribe_activation` | Button, ListItem, ListBox | Submitted label |
| `subscribe_checkbox` | Checkbox | Checked boolean |
| `subscribe_numeric` | Slider, Number | Finite committed value |
| `subscribe_scrollbar` | Scrollbar | Nonnegative clamped offset |
| `subscribe_text_changed` | Textbox, TextArea | Normalized text and matching UTF-8 byte cursor |
| `subscribe_text_submitted` | Textbox, TextArea | Submitted normalized text |
| `subscribe_window` | Logical window | Typed lifecycle/chrome event with copied geometry where relevant |
| `subscribe_popup` | Popup | Dismissal fact |

A source permits one subscriber per port. Editor changed/submitted ports are
independent. Subscribing to an occupied port returns `UI_E_BUSY`; subscribing to
a closed producer returns `UI_E_CLOSED`. A failed registration does not replace
the current subscriber. TextArea forwards both capabilities to its actual editor;
there is one queue for each event, without an outer duplicate.

Producer queues contain immutable snapshots and never own application callbacks,
sinks, leases or sessions. A subscription owns its queue connection and typed
sink; the session owns subscriptions. The runtime **borrows** `IEventDispatcher`
synchronously during `update_with_events` and must not retain it. State and
handlers may therefore own runtime/widget capabilities. Application-created
ownership cycles still require explicit cancellation. `EventSession` destruction
cancels every registered lease, including leases retained by other app code, and
releases their sinks. A retained dispatcher subsequently reports `UI_E_CLOSED`.
Final subscription release disconnects; final producer release closes its queue.
Neither invokes application handlers.

Dispatch sweeps sources in registration order, with FIFO inside each source.
Each source detaches its batch before calling handlers; no producer, retained
tree, registry or sink-slot borrow crosses the callback. Cross-source order is
registration order, not a global timestamp order. For example, if B was
registered before A, A produced `a1, a2`, and B produced `b1`, a sweep delivers
`b1, a1, a2`. Callback emissions participate in subsequent source visits or
sweeps until the queues are empty. Registration, session cancellation, and
recursive dispatch during a session transaction return `UI_E_BUSY`; cancelling
an individual lease from its own handler is supported and stops the remainder
of its batch. A cancelled old lease cannot disconnect a later replacement.

`EVENT_DISPATCH_LIMIT` is 1,000,000 callbacks per runtime dispatch boundary,
including all sources and cascade sweeps. `dispatch_bounded(limit)` selects an
explicit smaller budget for manual delivery. Exhaustion reports
`UI_E_EVENT_LIMIT` (-1007) and preserves unattempted events. Delivering exactly
the limit succeeds when nothing remains; limit zero succeeds only for empty
queues. A handler error consumes the attempted event, stops delivery, and
restores unattempted ordinary events ahead of events emitted by that callback.
Retry never replays the failed attempt. Changes already made by a handler are
not rolled back. Panics follow the COM abort policy and are not recoverable
handler errors. A custom dispatcher must report its actual successful callback
count and honor the supplied bound.

Ordinary widget setters are silent; changing a value does not manufacture a
user submission. Window visibility transitions and popup dismissal explicitly
publish lifecycle facts even when initiated by application code. Logical source
closure discards ordinary queued work, including the remainder of a detached
batch. Window destruction preserves exactly one terminal `Destroyed` fact for
its existing subscriber; it can be delivered after the window is closed. A new
subscriber cannot replay historical events. Cancellation discards terminal facts
too. Terminal delivery is attempted once, even if its handler returns an error.

`Ui::update_with_events` initially synchronizes geometry, dispatches already
queued events, then processes each native input followed by semantic dispatch
and layout before routing the next input. With no native input, eligible widgets
receive one ordinary update. At handler boundaries the context is available for
ordinary setters, window creation/removal, topology edits, and input enqueueing.
Recursive update/render reports `UI_E_BUSY`. Callback failure leaves the frame
uncommitted, so render reports `UI_E_UPDATE_REQUIRED`; another update can finish
remaining queued input. If a handler closes the runtime, the outer update
reports `UI_E_CLOSED`.

`Ui::update` supports polling without event dispatch. `EventSession::dispatch`
only delivers semantic events: it neither processes native input nor commits
layout. After manual callbacks mutate geometry, call update before rendering.
Directly dispatching a raw lease bypasses session ordering and its global budget;
applications using this lower-level API own those obligations themselves.

Finish `state()`/`state_mut()` borrows before dispatch or an eventful update. A
handler needs mutable access to the same state; incompatible borrows produce
`UI_E_BUSY`. As with any handler error, an event attempted while that state is
unavailable is consumed. Do not hide borrowed stack state inside a retainable
COM object to work around this rule.

### Geometry synchronization does not repeat ordinary widget behavior

A setter invoked by a handler can run after that widget's ordinary update for
the current input. The following layout sees its dirty geometry, but the runtime
does not automatically rerun already visited widgets. Requests implemented by
ordinary update, such as TextArea caret reveal and preferred-column reset, wait
for that widget's next eligible update. If immediate caret visibility is required
after a handler changes cursor, text, font or wrapping, perform another update
after the outer update returns (or allow a later queued input to supply that
update). Hidden/ineligible editors retain requests until eligible again.

This is an author obligation: retain such requests explicitly and distinguish
geometry invalidation from behavior that needs another update. A geometry commit
alone cannot certify completion of arbitrary pending model work. External/shared
state still requires explicit synchronization, as described above; neither a
handler return nor `Changed | NoChange` observes undocumented dependencies.

## Paths, buffers, resources, and errors

An immutable COM string is UTF-8 display text. It is not a native filesystem path. Native path capabilities distinguish Unix bytes from Windows UTF-16 code units, and the static facade converts those to `Path`/`PathBuf`. Preserve exact native selection independently of its potentially lossy display label. Directory reads and asset replacement validate their complete input before publishing a new live state.

Copy buffers are typed, borrowed for one call, and define explicit offset/count semantics. A whole-string replacement requires offset zero, validates UTF-8, and copies before returning. Validate bounds and overflow before writes or allocations. Immutable returned snapshots own their storage and release through their producer's COM reference counting.

Logical asset metadata and font tables may be cached per immutable bundle revision. Do not introduce a COM call per glyph or per vertex. Rendering crosses the boundary in bulk, ordered frames/batches; resource references are typed capabilities with validated renderer ownership. Preserve source ordering across ordinary UI geometry and custom render items.

Native GPU integrations belong in the same platform/render plugin as their backend. Rust `wgpu::RenderPass`, `CommandEncoder`, Ash owners, SDL windows, and `Arc<glow::Context>` do not become ABI-safe because the application facade is generic. Keep the backend-specific Rust integration local and expose concrete scene/mesh/camera/render-item interfaces outward. OS-window/context lifetime must cover GPU resource destruction.

A frame finishes explicitly and reports submission/presentation errors. Dropping an unfinished frame abandons it. A successful upload/replacement is published only after validation and backend preparation succeed. On failure, the previous valid bundle remains active. NcStatus reports the recoverable failure category; preserve useful backend diagnostics through the documented diagnostic capability.


Window capabilities expose current manager state through copied snapshots. A child inherits its family root's fixed layer; changing a managed child/dialog layer fails. Hiding a parent excludes its structural family, and showing it restores child-local visibility intent without reopening modal dialogs. Popups have a distinct interface, obey the active owner's modal policy, and retain their definition when hidden. Explicit close destroys the corresponding subtree and closes its widgets even when the application retains handles. Parent capabilities are authenticated by their manager-owned COM identity before a widget's mount-once claim is committed. Unknown option bits, malformed visibility/clip tags, invalid dimensions and invalid fixed layers are rejected before mutation.

## Widget interface inheritance

Declare specialized contracts as `IButton: IWidget` (or another single widget
base). The COM class lists `interfaces(IButton, ...)`; nc-com supplies IWidget
and every ancestor automatically in the same allocation. Compose one concrete
widget-state field into the class. Interface inheritance shares behavior and
identity; it does not create a second widget or copy Rust fields.

`button.take_change()` and borrowed inherited calls use the base vtable prefix
without reference-count traffic. When an API takes `ComRef<IWidgetAbi>`, query
IWidget once, retain that projection for as long as needed, and pass its borrow.
The derived header still advertises IButton, so a raw cast is not a valid base
projection. The static generic facade hides these checked projections from app
code. Use distinct names for base and specialized methods; local names take
precedence during Rust method lookup. Definitions across crates should name a
base through its module path so generated companion types resolve together.

## Common state, input and node ownership

`microui_contracts::widget::WidgetState` is a static implementation helper stored
inside a widget class. It owns the host authority, common options, mount-once
claim, logical closed flag and sticky geometry flag. It owns no runtime tree,
children, application callback or additional widget allocation. Specialized
interfaces inherit IWidget; IWidget itself now includes host/mount/close,
properties/set_options, update, take_change and paint. Leaf measurement remains
a separately cached ILeafLayout capability. No activation-only or separate
widget-lifetime interface is required.

A semantic setter checks `state.writable()` before borrowing its data, validates
all replacement inputs, commits atomically, and calls
`state.record_change(GeometryChange::CHANGED)` when measurement/placement inputs
actually differ. Common option setters use `state.set_options`. Return the
update's own geometry result; NO_CHANGE must never erase a prior setter marker.
The runtime collects markers across the authoritative forest, including hidden
surfaces and descendants, before using measurement caches. Hidden/clipped widgets
still skip their ordinary update; authors must not rely on that update to notice
externally shared mutations or to flush a setter's pending marker.

The app queues raw input and then updates explicitly:

```rust,ignore
ui.input(UiInput::Key(KeyStroke {
    key: KeyCode::TAB, value: 0, pressed: true, repeat: false, modifiers: 0,
}), "")?;
ui.input(UiInput::Text, "complete Unicode commit ☃")?;
ui.update(drawable_size)?;
ui.render(drawable_size, renderer.as_com_ref())?;
```

UiInput uses drawable coordinates and carries no invented drag or pointer delta.
The runtime preserves queue order, computes held state/deltas, selects recipients
and localizes WidgetEvent pointer payloads to content coordinates. Modifier bits
are Alt=1, Ctrl=2, Shift=4, Super=8; printable logical keys are separate from text
commits. Function keys use numbers 1–24. Key-repeat is valid only on presses.
A cancel event clears held/focus/capture state in order; widgets must also handle
an eventless update with changed focus/active snapshots. Update every visible
widget even when it received no routed event: animation, shared-state
synchronization and editor blur commits can change geometry then.

Widget update implementations call `decode_widget_event(event, text)` before
using input. This checks the ABI tag, complete UTF-8 with offset zero, key and
button values, and the rule that only Text has a nonempty text argument. A text
borrow expires on return; copy it if it must be retained. WidgetUpdate is an
owned snapshot of content geometry, clipping, interaction and the runtime's
shared semantic action. It contains no painter or pointer into the runtime.
Properties are refreshed after update even when geometry is unchanged, so
mode-dependent focus/text-input policy is visible to the next routed event.

The runtime takes ownership of its Context for each transaction and restores it
on every return path. No runtime RefCell guard spans a foreign call. Reentrant
runtime operations receive BUSY while the slot is occupied; ordinary peer
widget setters remain available during update. Measurement, painting and
property/change observation reject mutation through the common host authority.
Explicit close also respects observational phases. Node removal invokes logical
widget closure once; retained COM handles then report CLOSED. A component close
failure is returned by explicit window/runtime teardown after the logical
removal has committed. Final COM destruction cannot report an error and must
infallibly mark its event source closed, including when it happens during an
observational phase.

A failed measurement does not publish a zero-size preference. The runtime reports
the component error, leaves the node and its ancestor path dirty, and retries on
a later update. Widget authors should return an error for temporarily unavailable
measurement inputs and resume returning a valid nonnegative size after recovery.
Do not return a fabricated successful size to hide a failed resource lookup.

## Built-in editing and painting

Built-in leaf classes compose one concrete semantic value into their COM
allocation. `ITextContent` shares text snapshots/replacement; `ITextEditor` adds
byte-cursor editing, and `ITextbox` inherits both before reaching `IWidget`.
Slider and Number similarly inherit `INumericValue`. Calls through those bases
edit the same state used by input, measurement and painting.

A Textbox removes CR and LF during construction, replacement and committed text
input. Its cursor is a byte offset at a Unicode scalar boundary, not a glyph,
grapheme or visual column. Replacing text moves the cursor to the end even if
normalization leaves the text unchanged; that cursor-only operation does not
invalidate measurement. TextBlock normalizes CR/CRLF to LF. Word wrapping breaks
at ASCII spaces and keeps an oversized word intact. These are explicit existing
policies, not promises of grapheme editing or a full Unicode line-break engine.

Numeric constructors validate finite values, finite ascending slider spans,
nonnegative steps and precision through `f32::DIGITS`. Setters reject nonfinite
values; sliders clamp valid values to their range. Checkbox checked state, swatch
color, scrollbar offset/range and button center-fill selection affect paint only.
Programmatic setters do not manufacture user-originated events. Label, font,
icon, wrapping and numeric-label changes retain a sticky geometry marker.

Font and icon capabilities select semantic roles or resolve an exact named
resource once in the widget's runtime bundle. Unknown names leave the previous
selection intact. Normal measurement and painting retain stable resolved
identities and perform no COM calls per glyph. FillStates controls unframed
button center fill; the outer FRAME option remains owned by retained traversal.

Call `WidgetUpdate::validate()` or `WidgetPaint::validate()` before consuming a
copied phase snapshot, and decode event/text input with `decode_widget_event`.
Painting receives effective clipping, enabled/hover/focus/press state and logical
window activation as copied values. It has no mutable runtime context.

`IPainter::triangles` records a complete local atlas batch with one copied call.
The input offset is zero, its length is a multiple of three, and every position
and UV component is finite. The runtime validates the whole batch before
publishing any geometry, applies the widget origin and final viewport clipping,
and preserves painter order. Built-in classes reuse their existing UI geometry
algorithms before making this call; they release semantic and scratch borrows
first. A recorder error restores the reusable workspace. This atlas path does
not represent application GPU scenes or external texture ownership; those use
the renderer capabilities specified in the renderer migration.


## Container topology and edge metadata

An independently implemented container exposes `IWidget` and `IContainerLayout`
on one COM object. It exposes exactly one layout family: adding `ILeafLayout`
to that object is rejected. Obtain a private `IChildrenEditor` from the widget's
host during construction, retain it in the container, and return its `IChildren`
view from `IContainerLayout::children`. That view's canonical identity must stay
unchanged for the container's lifetime. The runtime accepts only collections
issued by that exact host. One collection can belong to only one mounted
container. Arbitrary look-alike collection objects and shared collections are
rejected before attachment.

The view and editor deliberately have different COM identities over one actual
child sequence. Querying the view cannot recover edit authority. Expose concrete
editing methods that keep Linear tracks, Grid spans or application-specific edge
metadata synchronized; do not return the private editor to callers who are not
responsible for that metadata. Child references do not transfer tree ownership.
Insertion consumes the child's sole attachment opportunity after validation.
Removal closes the entire subtree permanently. Retained application references
remain memory-safe and report closure; they cannot detach or reparent a widget.
Indices, owner identity, layout capabilities, collection ownership and prospective
cycles are checked before a claim commits. Reentrant topology edits during
attachment validation return `UI_E_BUSY`.

`ChildEditResult` separates a successful topology commit from a removed child's
fallible teardown. An editor call returning `Err` did not change topology. An
editor call returning `Ok(outcome)` did change topology, even when
`outcome.teardown` is an error. Publish matching metadata before calling
`outcome.finish()` or returning the outcome. For example, the essential order
inside a concrete replacement method is:

```rust
// The concrete model guard owns its state; it holds no RefCell borrow.
let mut model = self.take_model()?;
let edge = validate_edge(edge)?;
let outcome = self.editor.replace(index, child)?;
model.edges[index as usize] = edge;
// Metadata now describes the new child even if the old child's close failed.
outcome.finish()
```

Validate metadata before calling the editor. Acquire the concrete state guard
before any operation can commit. Do not attempt to acquire fallible state access
after the topology commit, and do not propagate teardown failure before updating
metadata. A concrete read or edit that reenters an already active semantic state
may return `UI_E_BUSY`. Common lifetime authority must remain separate from that
state. Closing a container during an active semantic edit must reject before
partially changing its lifetime.

Final release of the private editor immediately closes the collection, including
retained read-only views. If a layout invocation currently owns its children,
those nodes are retired when the owned transaction returns and actually closed
after the outer observational phase ends. They are no longer live tree edges.
The runtime does not permit semantic mutation during layout to make teardown
possible. Releasing the last editor during layout makes that layout result fail
with `UI_E_CLOSED`; the host must not submit it as a valid frame. Explicit
container removal closes its child storage even if a custom widget's own close
implementation subsequently reports an error.

## Scoped container layout

`IContainerLayout::measure` and `place` receive an `ILayoutService` and a
`LayoutScope`. The service owns the active child sequence. It never stores a
borrowed runtime node, mutable tree reference, or pointer to the caller's stack.
The container receives indexed operations and copied geometry only. The provided
scope is valid for that synchronous call; retaining the COM service does not
extend the scope. Tokens never wrap or become valid again.

Measurement permits `child_count` and `measure_child` only. Placement also permits
exact child allocation, copied content-extent reads, responsive participation and
explicit reporting of a changed child placement dependency. Placement-only calls
during measurement return `UI_E_BUSY`. A valid zero constraint remains different
from an unbounded constraint. Preferred child sizes include each child's own
frame. The container resolves tracks and relationships before passing an exact
rectangle to `place_child`; the runtime does not reinterpret that policy.

Nested child containers receive a fresh token. Their parent's token returns
`UI_E_EXPIRED_SCOPE` while the child token is active, then becomes valid again
when the child call returns. Direct reentry while a service method temporarily
owns its frame returns `UI_E_BUSY`. After the overall invocation ends, every
method rejects the old scope, including empty or otherwise harmless reads. All
success and failure paths restore the parent frame and authority.

Use the scoped service for child layout access. The separate `IChildren` view can
report its stable copied count while a layout owns the sequence, but `child(index)`
returns `UI_E_BUSY` then. A built-in algorithm validates the scoped count before
running, so an access error cannot be mistaken for an empty child sequence while
its edge metadata still contains entries.

Placement returns one `ContainerGeometry`: logical content extent, descendant
viewport and translation, and overflow propagation policy. The runtime clips the
viewport to the container's content allocation and publishes the result only
after the call succeeds. `HIDDEN` participation preserves ownership while gating
update, paint, input and focus for the entire subtree; `DISABLED` preserves update
and paint while excluding subtree input. Hidden children do not contribute active
overflow. Setter-change collection still visits hidden owned descendants.

Report every changed measurement or placement dependency through a truthful
update result or sticky setter marker. This includes shared scrolling offsets,
responsive participation policy, expanded/collapsed state, and layout inputs
changed indirectly by another object. `child_placement_changed` is for an
already active placement pass whose descendant depends on shared state despite
receiving the same rectangle; it does not replace the writer's responsibility to
request the initial layout. Independent plugins must honor these same ownership,
phase and invalidation rules.

A failed child measurement cannot become a cached zero preference. Failed
container measurement and placement retain invalidation and require a successful
retry before rendering. An implementation may have placed some children before
an error, but that partial pass is never a valid renderable frame. Restore owned
semantic state on all error paths and do not retain references obtained from
short local context wrappers after returning through COM.

## Composed controls and indirect invalidation

A composed control owns ordinary COM children. It shares explicit scalar data
only where two of those children need the same state. ScrollArea's viewport and
its real scrollbars share offset/range data; Disclosure's header and layout share
expansion data. TextArea's actual editor requests caret reveal through that same
scroll model. These models hold no parent widget, runtime, application, or COM
subscription reference. A child must not need to upgrade a weak parent handle to
perform an ordinary edit.

The public capabilities preserve each composition's structural roles:

- `IScrollArea: IScrolling: IWidget` permits replacing its one application content
  root. It does not expose `IContainer` methods that could remove the transform
  surface or either scrollbar.
- `IDisclosure: IWidget` returns its actual text header and actual Linear body.
  Edit body children through the returned Linear capability, and edit the label
  or header options through the returned text capability. The fixed header/body
  pair itself cannot be removed through a generic container interface.
- `ITextArea: IMultilineEditor: ITextEditor: ITextContent: IWidget` is the composed
  root. Its inherited editing calls forward to one actual editor child. Querying
  this root for `IScrolling` returns the same outer identity; querying it for
  `IScrollArea` fails. There is no operation that replaces its fixed editor with
  arbitrary content. The child exposes `IMultilineEditor`, has its own ordinary
  widget identity, and is the composition's single keyboard Tab stop.

For example, a static client can edit actual child capabilities without mirroring
plugin state:

```rust
let section = ui.widgets().disclosure("Details", DisclosureStyle::TREE, false)?;
let body = section.body()?;
let editor = ui.widgets().text_area("first line\nsecond line", TextWrapping::WORD, true)?;
body.insert(0, &editor, LinearEdge::new(LayoutTrack::Content))?.finish()?;
section.header()?.set_text("Editable details")?;
section.set_expanded(true)?;
editor.set_cursor(0)?;
// Synchronize geometry and pending caret reveal before rendering.
ui.update(drawable_size)?;
```

`Changed | NoChange` from an ordinary update does not automatically detect changes
in shared models. In particular, clicking a disclosure header need not change the
header's own preferred size, and moving a text cursor need not change text size.
Nevertheless, expansion and caret reveal can change an ancestor's placement.
The writer of that shared model must retain a sticky notification, and the
owning layout widget must collect it through `take_change`. That notification
must survive skipped updates, collapsed ancestors, and intermediate render
preflight checks. The built-in models do this explicitly; independent plugins
must supply the same discipline for their own shared data.

A requested scroll offset may temporarily exceed the previous maximum. Placement
clamps it against freshly measured content. This lets `scroll_to_end` accompany a
content replacement without losing the new end to the old range. `ScrollInfo`
therefore distinguishes current requests from the last installed ranges and
viewport. Programmatic setters are silent: they do not fabricate user events.
The standalone scrollbar offset setter clamps immediately to its installed range.
Disabling a scroll composite clears its requests and ranges and changes the
structural children's participation on the next placement.

TextArea normalizes CRLF and lone CR to LF. Cursor offsets count UTF-8 bytes and
clamp to the preceding Unicode scalar boundary. Word wrapping breaks at ASCII
spaces and preserves an oversized word as one span. Text replacement, including
a text-identical replacement, requests caret reveal; so does cursor positioning.
These setters conservatively invalidate geometry because reveal can move the
viewport even when text dimensions are unchanged. Font and wrapping changes also
reset the preferred visual column and request reveal. Visible ordinary updates
resolve those pending requests; hidden editors retain them until eligible again.
Widget/application writers must run update before rendering after these edits.

Closing a composite closes its real descendants. A retained header, body, editor,
or bar reference remains memory-safe and reports logical closure; it cannot keep
an orphaned interactive subtree alive or be remounted elsewhere. Keeping such a
reference never transfers the parent's topology authority.
