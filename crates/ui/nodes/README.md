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
its copied scope and first-error value are ordinary fields, with mutation through `&mut self`.
Concrete controls communicate through cached `IWidget`, `ILeafLayout`, and `IContainerLayout`,
exactly like independent extensions. Their outputs use concrete message contracts and COM-owned
transport. Producers post directly into `Events<Message>` inboxes; ordinary application
state is borrowed only by its deferred consumer, outside the producer graph.

`neocogi-ui-support` supplies binary-local canonical resource imports and pure presentation
helpers. It contains no concrete widget or runtime. The only shared mutable resource cache in
node phase plumbing is provider-local and serves separate concrete COM allocations; it is not
core storage. Semantic fields remain ordinary concrete fields wherever ownership is exclusive.

Node algorithm and picker tests stay beside the sources they exercise. Fixtures retain an actual
production core runtime through public entrypoints, and the provider-boundary integration test
activates both plugins before routing and dispatching a real typed activation event.
