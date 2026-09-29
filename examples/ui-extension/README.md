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
`class Counter: IWidget + ILeafLayout + IWidgetProbe`
is the concrete counter contract; `IInset: IContainer` is a reusable capability.
The registered `ExtensionProvider` factory constructs both with a typed
`IWidgetHost` argument; Counter also receives `EventBinding<Counter, CounterEvent>`. Factories are the only parameterless activation roots.

During attachment the runtime queries and caches each control's layout capability
beside `IWidget`; later update, layout, and paint calls use those cached pointers.
External controls share the built-in ownership and routing rules.

The [implementation](src/lib.rs) uses `WidgetState` for common options, mount-once
ownership, logical closure and sticky invalidation. The counter stores its value
once and owns a `CounterEventPort` initialized before the factory publishes it.
`CounterEvent` is an ordinary IDL enum containing `Activated(Activation)` and
`Changed(CounterChanged)`; the latter nests the committed count. The factory
receives one typed queue for that whole enum. There is no later attachment API.
The counter posts both variants after committing state. Composite close retires
children first, then local resources and common state; final Drop calls close.

The [acceptance host](../../demos/smoke/src/extension.rs) creates its queue through
`neocogi_com::events::mapped_queue` and passes it into `ExtensionProvider::counter`.
Its application handler matches `CounterEvent` after traversal releases widget
borrows. `CounterAbi::IDL_INTERFACE` and `CounterEvent::IDL_TYPE` supply generated
metadata. The root `ExtensionProvider` also
implements the shared `IDesignWidgetCatalog` COM interface. Its `describe`
method copies the counter's typed authoring descriptor through COM, and
`create_widget` accepts the host and optional COM receiver, queries that receiver
for `CounterEventQueueAbi`, and only then constructs the counter. A wrong queue is
rejected by the plugin even when a caller bypasses registry preflight.
The unlinked native designer can load this library with `--plugin` and discover
the counter without a Rust registration hook. A UI document stores only its kind
and typed property values; it does not restate the event enum.

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
