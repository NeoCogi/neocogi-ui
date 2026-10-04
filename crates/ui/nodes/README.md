# Built-in nodes provider

`neocogi-ui-nodes` owns both the Rust implementations and the deployable
`neocogi.ui.nodes.runtime` provider. Concrete classes live in `src/widgets` and
`src/containers`; `src/factory.rs` owns their bound `UiFactory`, `src/widget_object.rs`
owns their common COM phase projections, and `src/dialogs.rs` owns the standard file picker.
`src/widget.rs` owns their statically dispatched `Widget`/`LeafWidget` algorithms; these native
traits are not part of either core or shared support.
There are no source includes or forwarding implementations pointing back to core.

The nodes provider declares a COM provider dependency on `neocogi.ui.runtime`, not a
production Rust dependency on its implementation. A host creates core `UiRuntime`, activates
`NodesPluginFactory`, and binds it to that runtime. The factory captures only generated public
host/resource, choice-menu, and window capabilities. `Ui::from_interfaces` checks canonical host
identity before combining the runtime and factory.

Core owns retained topology, root identity, authoritative child stores, traversal, layout/painter
scope validation, and frame submission. `src/layout_client.rs` is only a borrowed COM client:
it stores a scope, borrowed service and child count. Each operation returns its
`Result` directly; there is no error latch or later completion check.
Concrete controls communicate through cached `IWidget`, `ILeafLayout`, and `IContainerLayout`,
exactly like independent extensions. Their outputs expose concrete IDL event interfaces.
The parent's common receiver moves each `ComBox<IEvent>` into its inbox and retains
only a weak source identity. Native decoding happens during handling. The file dialog
implements `IFileDialog`, `IWidget` and `IContainerLayout` on one allocation,
embedding its Linear layout, ordinary model and private receiver. The runtime
calls its widget `dispatch_next` after traversal ends. Parent-controlled removal
drops the unique widget owner, its children and private inbox after any active
handler returns. Weak editing handles retain no widget value. Dropping those
observers leaves a mounted dialog alive. No separate controller, owner mount/retire state,
or shared mutable request/model wrapper is needed.

`neocogi-ui-support` supplies binary-local canonical resource imports and pure presentation
helpers. It contains no concrete widget or runtime. The only shared mutable resource cache in
node phase plumbing is provider-local and serves separate concrete COM allocations; it is not
core storage. Semantic fields remain ordinary concrete fields wherever ownership is exclusive.

Node algorithm and picker tests stay beside the sources they exercise. Fixtures retain an actual
production core runtime through public entrypoints, and the provider-boundary integration test
activates both plugins before routing and dispatching a real typed activation event.

Grid construction requires explicit immutable `GridLimits`. Each topology/span/
column edit prepares a complete non-overlapping plan and fallible storage before
changing child ownership. Rejection returns the incoming owner unchanged; after
host acceptance, metadata publication cannot fail. Linear and Form use the same
prepare/commit ordering with only an index check and vector reservation. See the
[Grid placement contract](../../../docs/contracts/GRID_PLACEMENT.md) for numeric policy,
right-edge clamping, removal reflow, and the regression matrix.

## Constructor configuration

Concrete providers choose initial widget flags. `UiFactory.create_button(label,
events)` constructs the normal framed Button, just as the other factory methods
choose their own initial presentation. Rust convenience methods forward the
construction request without supplying a second default mask. Application-specific
presentation uses `IWidget.replace_flags`, `set_flags`, or `clear_flags` afterward.

`IWidget.requested_flags()` reads that constructor/setter configuration through
COM. `IWidget.properties().options` describes effective runtime behavior and may
include derived input flags such as `GRAB_SCROLL`; never save it as configuration.
The requested getter obeys the same readable-lifetime checks as other properties.
Custom providers must implement it from the same state their flag setters edit.
The IDL only describes these ordinary methods: it contains no widget-default policy.

See the [provider-owned configuration contract](../../../docs/contracts/WIDGET_CONSTRUCTOR_DEFAULTS.md)
for document absence versus explicit zero, designer inspection, custom plugins,
and the history of the removed duplicate default policy.
