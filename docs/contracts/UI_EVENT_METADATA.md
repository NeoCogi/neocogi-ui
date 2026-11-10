# UI event publication

IDL describes ordinary callable interfaces and source value graphs. UI owns
`IEvent`, `IErrorEvent`, `EventOrigin`, `IEventReceiver`, and `EventBinding` in
`neocogi.ui.core`. The COM compiler and host-owned runtime have no event semantics.

```ncidl
extern neocogi.ui.core;
use neocogi.ui.core::{IEvent, EventBinding};
#[version(1)] interface IChanged: IEvent {
    fn value(&self) -> Result<u32>;
}
#[version(1)] interface Counter {}
#[version(1)] interface Counters {
    fn create(&self, events: &EventBinding) -> Result<ComBox<Counter>>;
}
```

The contract crate publishes its UI outputs beside the generated bindings:

```rust,ignore
neocogi_ui::abi::core::ui_event_sources! {
    CounterAbi => [IChangedAbi];
}
```

`UiEventSource::EVENT_SOURCE` is the one authoritative producer/event list.
Use an empty list for a constructible silent widget. Lists belong to their
concrete producer; inheritance does not imply event publication. The macro
implements a Rust trait and references ordinary `IDL_INTERFACE` metadata. It
generates no handlers, queue aliases, or native registration. A provider owns
its snapshot implementation and the events it actually sends.

`DesignWidgetManifest::from_source(kind, defaults, &CounterAbi::EVENT_SOURCE)`
validates publication before copying an owned catalog record. Import
`neocogi_ui::abi::core::UiEventSource` to access the trait constant. For
generic snapshots, declare an ordinary concrete IDL interface derived from the
closed template, then list its generated ABI marker in the UI declaration.
Every event must
derive from the actual UI `IEvent` identity, including every signature byte. Every
inherited method must use `&self`, take no arguments, and return repeatable data.
Transfers, borrowed outputs, buffer operations, duplicate event identities and
ambiguous getter names are rejected by UI. Legal ordinary COM interfaces remain
usable regardless of whether UI admits them as snapshots.

The generic records live in `neocogi_com::idl_metadata`.
Names and complete digests use ordinary static Rust borrows; UI conversion copies
them without reading raw physical descriptor pointers. The producer's identity
must match its first chain entry. An incompatible required IEvent ancestor is
rejected, while a catalog may advertise another concrete event revision; queries
against actual event objects negotiate compatibility with that revision.
`IDL_INTERFACE.methods` describes generic calls, receiver access, parameter names
and ownership, and result types. Shared plain or closed-generic ancestors appear
once per canonical IID in base-first order; each method retains the type scope
of its supplying declaration. `IDL_INTERFACE.values` contains definitions
reachable through ordinary methods and bases. Catalog publication merges the
producer and event value graphs, preferring the producer's Rust projections and
rejecting conflicting canonical shapes. Named recursion stays finite.

The owned catalog schema remains version **12**: event identity, signature,
getter fields and reachable values cross COM as bounded owned data. Rust static
references stay in the defining binary. The designer and generated forms consume
this same validated description; runtime event queries negotiate actual ABIs.

Event lists are UI metadata and do not change a producer's callable ABI signature.
Changing an event's ordinary getter contract still changes that event's signature.
Canonical module encoding **v13** removes the old application event edges; rebuild
all contracts and consumers together. Core object ABI and runtime discovery are
unchanged. Native plugins continue to share the single global `neocogi_com_host` registry.

`#[events(...)]` is no longer IDL syntax. Its former compiler validation and
`IdlEvent` records have been removed rather than retained as compatibility paths.

[UI admission tests](../../crates/ui-contracts/tests/event_metadata.rs) compile legal
ordinary interfaces and reject invalid snapshots, ambiguous generic getters and
duplicate lists at publication. [Diamond tests](../../crates/ui-contracts/tests/event_diamonds.rs)
exercise real exclusive getters, imported projections and catalog round trips.
[Construction tests](../../crates/ui-contracts/tests/construction_events.rs) verify
receiver retention, permanent delivery policy and rejected owner transfers.
[Catalog path tests](../../crates/ui/tests/runtime/catalog_paths.rs) exercise
independent Cargo owners, raw identifiers and compiled form exports.
