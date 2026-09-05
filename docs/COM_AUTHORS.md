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
