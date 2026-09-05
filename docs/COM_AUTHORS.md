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

Semantic events have concrete sink interfaces and typed payloads. An application owns its state and its subscriptions. Source queues contain event snapshots, not application callbacks. A subscription owns its source connection and typed sink; the application's event driver owns those subscriptions. The runtime borrows the driver during a transaction rather than permanently owning a path back to application handlers that may retain UI objects.

Generic adapters are statically linked and own their state, for example through an application-local `Rc<RefCell<State>>` and a concrete handler trait. They must not hide a borrowed stack state pointer inside a retainable COM object. Generic Rust list data stays local to the application; cross-plugin domain data needs a concrete domain interface and owner-scoped keys.

Dispatch follows deterministic registration-order sweeps and FIFO order within a source, with a finite cascade bound. Detach a delivery batch before invoking handlers, so no producer/tree borrow is held while application code runs. Programmatic setters remain silent unless their concrete API explicitly documents an event. Replacing a value must not manufacture a user submission.

Cancellation and final subscription release disconnect without invoking user code. Closing a source discards ordinary queued work and preserves only the documented terminal fact. The driver can deliver that fact after the producer has logically closed. Do not create a strong ownership cycle between a runtime, its source, a handler, and retained widget/runtime capabilities.

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
