# Independent widget and container example

This optional assembly depends only on the facade, UI contracts, example contracts,
neocogi-com (including its event adapters). It contains a counter and a one-child inset container. It is built and
loaded separately from the host, UI, and renderer; ordinary demo packages omit it.

```sh
cargo neocogi run --assembly smoke --variant glow --variant extension -- --smoke
```

The extension itself also builds with ordinary Cargo:

```sh
cargo rustc -p neocogi-example-ui-extension --lib --crate-type cdylib --features dynamic-plugin
cargo rustc -p neocogi-example-ui-extension --lib --crate-type staticlib
```

Rust monoliths link its `rlib`; the second command additionally produces a native
static archive. The same event path runs in a monolith with embedded resources:

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
once and shares the same event transport as built-ins. Its NCIDL contract binds
`changed: CounterChanged;` to the concrete message
`#[message] struct CounterChanged { value: u32 }`.

The producer stores `Option<ComPtr<__CounterChangedReceiverAbi>>`. Its ordinary
`Counter::__map_changed` implementation checks widget access and assigns the
receiver. After committing the count it calls
`CounterChanged { value: self.count }.post_to(&mut self.changed)?`.
Activation independently stores its optional concrete receiver. Widget closure
clears those fields. No output projection, subscription token, generation counter
or producer queue is allocated.

The [acceptance host](../../demos/smoke/src/extension.rs) defines its own ordinary
Rust enum and maps the output with `events::bind(counter.changed(), sender, Message::Changed)`.
Mapping immediately translates the concrete value and posts it into that consumer's
FIFO. The later mutable consumer runs through `ui.update_with_events`, after
traversal borrows are released, and may edit the counter. Captured values remain
the committed facts; resulting geometry is synchronized before update returns.
The same implementation and generated contracts serve both build commands above.

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
