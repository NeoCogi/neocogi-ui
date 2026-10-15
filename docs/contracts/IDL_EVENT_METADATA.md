# Advertised event interfaces

A producer declares the immutable COM interfaces it emits independently of its
constructors. This is source metadata, not an additional interface projection on
the widget and not a runtime subscription mechanism.

```ncidl
extern neocogi.com.events;
use neocogi.com.events::{IEvent, EventBinding};
#[version(1)] interface IChanged: IEvent {
    fn value(&self) -> Result<u32>;
}
#[version(1)] #[events(IChanged)] interface Counter {}
#[version(1)] interface Counters {
    fn create(&self, events: &EventBinding) -> Result<ComBox<Counter>>;
}
```

The attribute accepts one nonempty list on an interface, class or factory.
Each entry resolves to an interface derived from the canonical
`neocogi.com.events.IEvent`. Imported and closed generic interfaces are supported.
Duplicate identities are rejected. Every getter, including inherited getters,
must use `&self`, accept no arguments, and return a repeatable value. Exclusive
owners, borrowed outputs and buffer operations are rejected. Getter names must
be unique across the interface ancestry. These checks report `EIDL065` before
Rust generation.

Diamond inheritance is supported. An event's inherited getters are included once
per canonical interface IID, in base-first order, with types resolved in the module
where each getter was declared. Separate consumer-local projections of the same
closed generic interface contribute the same ancestor once; Rust declaration paths
do not define distinct contracts. Conflicting signatures for the same IID remain
invalid, and distinct interfaces with duplicate getter names remain ambiguous.
Emitted-event lists belong to their declaring producer; a derived producer declares
its own list. The compiler
checks shape, while the provider is responsible for snapshot immutability and
which events it actually emits. Hosts validate any domain constraints they rely on
when consuming foreign getter results.

`*Abi::IDL_INTERFACE.events` is an ordered slice of `IdlEvent` records containing
qualified identity, physical ABI descriptor, and getter names/result types.
Reachable named value definitions appear in `IDL_INTERFACE.values`. The owned
`DesignWidgetManifest` format is version **12**, with `DesignEvent` records carrying
the IID, signature, and getter fields across COM. Catalog admission checks identities,
duplicates, field types, finite graphs and size limits. Event signatures negotiate
against actual event objects at query time; the constructor receiver remains common.

Event references contribute to canonical module signatures (encoding v9). They
are semantic dependencies, not widget vtable inheritance. Changing event declarations
or their transitive contracts requires rebuilding consumers. Core object ABI and
Cargo source-graph schema are unchanged by this metadata change.

No constructor inference, per-product binding/port alias, native event association,
or generated application handler remains. Factories use the common `EventBinding`;
shared services may store the common Rust `EventPort`. Former helper-like names
are ordinary legal declarations. Generic IDL values remain supported for other uses.

The removed constructor-name contract documented machinery that no longer exists.
[Resolver tests](../../crates/com/idl/tests/event_metadata.rs) cover attributes,
inheritance, imported scopes, generics, malformed getters and removal of reserved
helper names. [Compiled diamond tests](../../crates/com/idl/tests/event_diamonds.rs)
exercise ordinary and cross-module generic ancestry, real exclusive getter calls,
canonical projection identity, imported value types and owned catalog round-trips.
[Compiled construction tests](../../crates/com/idl/tests/construction_events.rs)
exercise common bindings, real COM getters, weak origins and rejected owner transfers.
[Relocated-contract tests](../../crates/com/idl/tests/rust_paths.rs) compile optional
form fields and event getters across separate Cargo packages.
