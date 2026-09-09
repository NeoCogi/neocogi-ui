# Independent widget and container example

This optional assembly depends only on the facade, UI contracts, example contracts,
and neocogi-com. It contains a counter and a one-child inset container. It is built and
loaded separately from the host, UI, and renderer; ordinary demo packages omit it.

```sh
cargo run -p microui-redux-tools --bin com-build -- --demo extension --backend glow
target/com-native/debug/glow/files/extension/microui-demo-com --smoke
```

Use `vulkan` or `wgpu` for the other native renderers. The same crate also compiles
for `wasm32-unknown-unknown`; static applications register its descriptor explicitly.

The contracts live in [demo-contracts](../../demos/demo-contracts/src/extension.rs).
`Counter: IWidget + ILeafLayout + IActivationEvents + IWidgetProbe` is a callable
class contract, while `IInset: IContainer` demonstrates ordinary interface
inheritance. The host constructs the registered counter directly with a typed
`IWidgetHost` argument; the provider remains only as the inset's shorthand factory.
Both implementations authenticate attachment to the requesting UI.

The [implementation](src/lib.rs) uses `WidgetState` for common options, mount-once
ownership, logical closure and sticky invalidation. The counter stores its value
once and shares the same event transport as built-ins:

```rust,ignore
submitted: EventSource<Activation>,

// After committing the count:
self.submitted.emit(Activation { label: self.count.to_string() });

// IActivationEvents::subscribe returns the application-owned lease:
self.submitted.subscribe(sink)
```

Emission queues an owned snapshot. The producer does not retain the application's
handler. The [acceptance host](../../demos/com-slice/src/extension.rs) uses a concrete
`EventHandler` to edit the counter during delivery; the UI commits that edit's
measurement before returning from the same update.

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
