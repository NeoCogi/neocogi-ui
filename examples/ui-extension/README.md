# Independent widget and container example

This optional assembly depends only on the facade, UI contracts, example contracts,
and neocogi-com. It contains a counter and a one-child inset container. It is built and
loaded separately from the host, UI, and renderer; ordinary demo packages omit it.

```sh
cargo neocogi run --assembly smoke --variant glow --variant extension -- --smoke
```

Use `vulkan` or `wgpu` for the other native renderers. The same crate also compiles
for `wasm32-unknown-unknown`; static applications register its descriptor explicitly.

The contracts live in [the demo IDL module](../../demos/contracts/idl/neocogi/demo.ncidl).
`Counter: IWidget + ILeafLayout + IActivationEvents + IWidgetProbe` is a callable
class contract, while `IInset: IContainer` demonstrates ordinary interface
inheritance. The host constructs the registered counter directly with a typed
`IWidgetHost` argument; the provider remains only as the inset's shorthand factory.
Both implementations authenticate attachment to the requesting UI.

Registration is used only to construct the extension class. During attachment the
UI runtime queries and caches the counter's `ILeafLayout` or the inset's
`IContainerLayout` beside `IWidget`; subsequent update, layout, and paint phases
call those cached pointers directly. They perform no class-registry lookup and do
not distinguish these external classes from built-in nodes.

The [implementation](src/lib.rs) uses `WidgetState` for common options, mount-once
ownership, logical closure and sticky invalidation. The counter stores its value
once and shares the same event transport as built-ins. Its custom value event is
declared in NCIDL, so the contract generator supplies the typed payload, sink
adapter, and lease-bearing subscription signature:

```rust,ignore
submitted: EventSource<Activation>,
changed: EventSource<ICounterChangedSinkEvent>,

// After committing the count:
self.submitted.emit(Activation { label: self.count.to_string() });
self.changed.emit(ICounterChangedSinkEvent { value: self.count });

// Counter::subscribe_changed is generated from an NCIDL event port:
self.changed.subscribe(sink)
```

Emission queues an owned snapshot. The producer does not retain the application's
handler. The [acceptance host](../../demos/smoke/src/extension.rs) constructs the
generated sink against its own typed application `EventSession`, subscribes to the
custom port, and also uses an activation handler to edit the counter during
delivery. The value callback still observes the originally committed value;
the UI commits the later edit's measurement before returning from the same update.

The inset retains one `IChildrenEditor` and exposes its read-only view. It reads
the authoritative child count instead of maintaining a second topology flag. Its
layout methods call indexed `ILayoutService` operations using only the supplied
scope; they retain neither that service nor its invocation token. Mutable contract
receivers and the generated object-wide call gate protect the object's inline state
across outward calls without an implementation-specific guard.

`IWidgetProbe` counts actual phase attempts for acceptance and measurement. Counts
remain readable after logical closure, so the host can verify each subtree closes
once while editing handles remain retained. These are example-specific diagnostics,
not a second widget state model or a required toolkit interface.

Authors remain responsible for truthful `CHANGED` results, external-state updates,
hidden-widget synchronization, observational layout/paint, event ownership and
explicit redraw scheduling. The complete obligations and failure rules are in
[COM_AUTHORS.md](../../docs/COM_AUTHORS.md).
