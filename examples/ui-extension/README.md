# Independent widget and container example

This optional assembly depends only on the facade, UI contracts, example contracts,
and neocogi-com. It contains a counter and a one-child inset container. It is built and
loaded separately from the host, UI, and renderer; ordinary demo packages omit it.

```sh
cargo neocogi run --assembly smoke --variant glow --variant extension -- --smoke
```

The same event path runs in a plain-Cargo monolith with embedded resources:

```sh
cargo run -p neocogi-demo-smoke --bin neocogi-demo-smoke --features monolith,extension,embed-assets -- --smoke
```

For a headless Glow smoke run, set `SDL_VIDEODRIVER=offscreen`.

Use `vulkan` or `wgpu` for the other native renderers. The same crate also compiles
for `wasm32-unknown-unknown`; static applications register its descriptor explicitly.

The contracts live in [the demo IDL module](../../demos/contracts/idl/neocogi/demo.ncidl).
`Counter: IWidget + ILeafLayout + IActivationEvents + IWidgetProbe` and
`IInset: IContainer` are callable interfaces.
The registered `ExtensionProvider` factory constructs both with a typed
`IWidgetHost` argument. Factories are the only parameterless activation roots.

During attachment the runtime queries and caches each control's layout capability
beside `IWidget`; later update, layout, and paint calls use those cached pointers.
External controls share the built-in ownership and routing rules.

The [implementation](src/lib.rs) uses `WidgetState` for common options, mount-once
ownership, logical closure and sticky invalidation. The counter stores its value
once and shares the same event transport as built-ins. Its custom value event is
declared in NCIDL, so the contract generator supplies the typed payload, sink
adapter, and lease-bearing subscription signature:

```rust,ignore
submitted: EventSource<Activation>,
changed: EventSource<CounterChangesMessage>,

// After committing the count:
self.submitted.emit(Activation { label: self.count.to_string() });
self.changed.emit(CounterChangesMessage { value: self.count });

// Counter::subscribe_changed is generated from an NCIDL event port:
self.changed.subscribe(sink)
```

Emission queues an owned snapshot. The producer does not retain the application's
handler. The [acceptance host](../../demos/smoke/src/extension.rs) constructs the
generated `counter.changed()` and passes it to
`events.subscribe(port, RecordCount)`. The same typed session owns its lease, and also uses an activation handler to edit the counter during
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
