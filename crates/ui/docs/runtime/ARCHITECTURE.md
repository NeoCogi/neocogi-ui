# UI runtime architecture

For an overview of ownership, typed queues and behavioral scheduling, start with
the [widget and event model](../../../../docs/architecture/WIDGET_EVENT_MODEL.md).

The core UI provider owns the retained tree, layout traversal, input routing and
surface policy. The nodes provider owns concrete controls, containers and standard
compositions. Both built-in and independently compiled widgets use the same COM
protocols at the retained-node boundary.

## Source and deployment ownership

| Owner | Responsibility |
| --- | --- |
| `neocogi-ui-contracts` | Application facade, concrete handles and application message dispatchers. |
| `neocogi-ui-contracts` | Shared widget, layout, surface, menu and window protocols. |
| `neocogi_ui_contracts::abi::nodes` | Built-in event values and callable constructor protocols. |
| `neocogi_ui::runtime` | Retained topology, authoritative child stores, traversal, scopes and surfaces. |
| `neocogi_ui::nodes` | Widget algorithms, semantic state, containers and file dialog. |
| `neocogi_ui::support` | Resource imports and pure geometry/text/presentation helpers. |
| GFX and Skin | Portable immutable resources, appearance and optional theme compilation. |
| Renderer | Native platform/GPU objects and ordered frame execution. |

Runtime and nodes share the static SDK package and one `neocogi.ui` service root.
The node binder captures only public COM capabilities from the supplied runtime.
Its independently owned factory authenticates host identity; it never borrows
private retained storage. A class is not a DLL.

Each built-in family module owns its concrete state, constructor and algorithms.
`widget_object.rs` supplies their common COM phase projections; `layout_client.rs`
borrows the runtime's scoped layout service. The latter stores only scope, service
and child count, and returns each operation's error directly. It has no first-error
latch. `dialogs.rs` owns the standard file picker and its private event receiver.

The support module owns no retained tree, runtime, host authority or concrete widget.
Installed resources retain the generated asset records plus process-local lookup
data. The [asset guide](../../../../docs/contracts/COM_ASSETS.md) defines resource ownership
and validation.

## Typed construction and attachment

`NodesPluginFactory` binds a supplied `UiRuntime` and returns a `UiFactory`.
Core exposes `IConstructionHost` through `construction_host()`. The facade's
`Ui::from_interfaces` checks that factory and runtime identify the same host.

Applications call typed factory methods returning `ComBox` widgets. Construction
accepts the final event queue; initialization policy belongs to the provider.
[Constructor defaults](../../../../docs/contracts/WIDGET_CONSTRUCTOR_DEFAULTS.md) defines
how the document loader and designer preserve that policy. There is no class
registry or erased constructor argument frame.

Attachment consumes the owner once. `NodeFactory` authenticates its host, queries
exactly one layout role (`ILeafLayout` or `IContainerLayout`), validates a branch's
child view, and copies routing properties before committing the mount. The
[mounted representation](../../src/runtime/ui_node/mounted_node.rs) contains:

- One `ComBox<IWidgetAbi>`, which owns the actual widget value.
- A concrete layout enum containing weak leaf or container projections.
- For a branch, its authenticated child store and view.
- Geometry/invalidation state and scoped resource/layout/paint services.

Capability discovery happens at attachment. Later update, measurement, placement
and paint calls use those cached projections. Concrete class identity does not
select a separate built-in dispatch path.

## Ownership, removal and errors

Containers' authoritative child stores own descendant nodes and their boxes.
Applications hold `WeakComBox` editing observations. Removing a subtree drops its
unique owner, children and private resources synchronously. New calls fail with
`NC_E_DROPPED`; weak storage cannot keep the widget value alive or upgrade into
an owner. The [exclusive ownership contract](../../../../docs/contracts/EXCLUSIVE_WIDGET_OWNERSHIP.md)
defines admission, synchronous destruction and weak allocation storage.

A widget cannot remove its own active ancestor. Thread-local topology edits check
the existing subtree borrow state and return Busy before committing such removal.
A close request is an ordinary event to the parent, which decides whether to remove
the child after its call returns. `IWindow::close` is a surface-removal operation;
there is no generic `IWidget::close`. Runtime shutdown is a distinct terminal
operation, described in the [event guide](EVENTS.md).

Insertion and replacement prepare validation and fallible storage before accepting
the incoming owner. Failure returns that same `ComBox`; success transfers it.
The [Grid contract](../../../../docs/contracts/GRID_PLACEMENT.md) documents bounded placement
and metadata preparation. The [COM output contract](https://github.com/NeoCogi/neocogi-com/blob/92fa66ebc70888f71f2d07f2bbc14335a2ffd4e6/docs/contracts/COM_OUTPUT_COMPLETION.md)
defines the generated consuming-call boundary, including result publication.

`NC_E_DROPPED` and `NC_E_CLOSED` can also come from a live handler's dependencies.
The dispatch iterator determines whether its receiver still exists; it propagates
errors from live receivers. The [lifetime error contract](https://github.com/NeoCogi/neocogi-com/blob/92fa66ebc70888f71f2d07f2bbc14335a2ffd4e6/docs/contracts/COM_LIFETIME_ERRORS.md)
prevents interpreting any returned status as proof that the receiver died.

## Frame and event phases

The runtime synchronizes pending geometry, then handles queued input in order.
It routes an input, updates eligible widgets, releases traversal state, dispatches
widget/application messages and commits resulting geometry before the next input.
An empty input queue still performs an eventless update.

A producer posts its typed event to the constructor-supplied application queue
synchronously. That queue acquires an owned payload and, when needed, a
`WeakComBox` origin, then sends a private application message. Behavior runs later.
Composite widgets own their receiver and model and implement `IWidgetDispatcher::dispatch_next`;
the runtime schedules them descendant-first. A top-level `Dispatcher<Processor>`
implements the application's dispatcher. There is no separate event-owner
mount/retire lifecycle. The [event guide](EVENTS.md) defines ordering, errors and
application-specific stale-message policy.

The public [Button walkthrough](../../../ui-contracts/docs/button_events.rs) is also included
verbatim in facade rustdoc. The runtime's documentation regression executes it
through real keyboard input, delivery, parent removal and a queued expired origin.
It demonstrates the required ownership behavior rather than only compiling a queue.

Changed setters, reported update changes, topology edits, child changes, resize
and resource revisions invalidate geometry. Paint rejects missing or stale layout
before GPU acquisition. Animation and external-state scheduling remain explicit
author responsibilities; see [author guidance](../../../../docs/contracts/COM_AUTHORS.md)
and [layout](LAYOUT.md).

## Scoped layout and rendering

Containers keep semantic edge metadata such as tracks and spans. Core owns the
actual children and parent-local geometry. Layout uses bounded indexed
`ILayoutService` calls with an invocation scope. Painting uses a scoped
`IPainter` and copied geometry. Retaining a service cannot extend that scope;
expired tokens and forbidden reentry fail explicitly.

The display list preserves ordered atlas, image and custom render operations.
GFX expands complete typed nine-patch operations, clips and batches their geometry.
The renderer validates resource ownership before acquiring a frame. Immutable
bundle installation prepares and validates its candidate before publication.
See [rendering](RENDER.md), [backend ownership](BACKENDS.md) and
[asset installation](../../../../docs/contracts/COM_ASSETS.md).

## Surface policy

`WindowManager` owns a private `SurfaceForest`. Concrete enums distinguish
windows/dialogs and application or relational menu popups. A surface containing
ordinary widget content owns a retained root node; menu surfaces retain their
concrete menu policy. Derived lookup and visibility data do not own duplicate
surfaces.

Parent edges define child-window, dialog and popup relationships. Destroying a
parent removes its descendants. Hiding preserves applicable local visibility
intent without resurrecting dismissed transient surfaces. Popup activation,
focus restoration and modality belong to this surface layer; widget event queues
remain ordinary behavioral-parent inputs.

Detailed menu navigation and ownership live in the [menu guide](MENUS.md).
The source and its runtime tests are authoritative for surface transitions;
[this overview's event walkthrough test](../../src/runtime/runtime/tests/documentation.rs)
covers the application-facing ownership boundary.

## Geometry mutation notification

IWidgetHost::mark_geometry_changed and IWidget::take_change form one protocol for
all providers, including independently generated C plugins. Notify conservatively
for measurement, placement and routing policy changes on hidden/unmounted widgets
as well as visible ones. Keep local sticky changes until collection; return
NoChange without clearing prior mutations. Post-commit failures retain the marker
and notification. Pre-commit rejection may conservatively notify without mutation.
Observation may notify for recovery but still cannot mutate widget state.

RuntimeStorage owns forest synchronization. It takes the host notification before
collecting every retained tree; errors restore it, and notifications during
collection survive. Root measurement/placement consume the already synchronized
state and do not poll again. Truthful Changed update results and topology edits
notify internally. Unannounced external mutation is a provider contract violation.
Idle updates perform zero take_change calls once collection settles.

Each retained ComNode keeps one concrete factory service lease. Metrics, painter
and nested layout services are borrowed within synchronous calls; no per-node
service copies exist. The factory's child registry is weak and owns no forest.
Retained scope tokens still expire and nested foreign layout uses the same service
stack. This reduces ownership handles without imposing a node-size target.

The surface forest's types/index/chronology remain in roots.rs. Its lifecycle module
owns validated membership and visibility changes; interaction owns cross-surface
input/focus/menu policy; traversal owns committed geometry and paint; synchronization
owns forest marker collection and public copied policy. All derive from the same
forest; the split adds no secondary surface owner or lifecycle representation.

Application Dispatcher calls MessageProcessor::apply_pending_changes when its inbox is
empty or disconnected. The demo host forwards AppState::apply_pending_changes
and its ApplyOutcome without suppressing progress. Dispatcher maps a consumed
message to ApplicationDispatch::MessageHandled, applied pending changes to
PendingChangesApplied, and an idle visit to Idle. Both work outcomes charge the
existing bounded budget and revisit child work/layout before routing more input.
The runtime also reconciles geometry/topology notifications from an Idle visit.
The hook can run several times per update; an idle invocation must create no work. Outer
editor geometry changes occur there; after_update renders independent previews
and mutable leaf paint accepts completed images under existing scope authority.
The outer editor needs one update. Release gesture grouping ends only after every
command from the release frame has been consumed.
