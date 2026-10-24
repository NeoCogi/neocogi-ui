# Independent widget and container example

This optional provider uses the UI API, demo bindings exposed by the sample
library and the COM facade. UI owns the event channel. It contains a counter and a one-child inset container. It is built and
loaded separately from the host, UI, and renderer; ordinary demo packages omit it.

```sh
cargo run -p neocogi-demo-smoke --no-default-features --features dynamic-extension,glow -- --smoke
```

The extension itself also builds with ordinary Cargo:

```sh
cargo rustc -p neocogi-example-ui-extension --lib --crate-type cdylib --features dynamic-plugin
cargo rustc -p neocogi-example-ui-extension --lib --crate-type staticlib
```

Rust monoliths link its `rlib`; the second command additionally produces a native
static archive. The same event path runs in a monolith with embedded resources:

```sh
cargo run -p neocogi-demo-smoke --no-default-features --features monolith-extension,glow,embed-assets -- --smoke
```

For a headless Glow smoke run, set `SDL_VIDEODRIVER=offscreen`.

Use `vulkan` or `wgpu` for the other native renderers. The same crate also compiles
for `wasm32-unknown-unknown`; static applications register its descriptor explicitly.

The contracts live in [the demo IDL module](../../demos/schemas/neocogi/demo.ncidl).
`interface Counter: IWidget + ILeafLayout + IWidgetProbe`
is the concrete counter contract; `IInset: IContainer` is a reusable capability.
The registered `ExtensionProvider` factory constructs both with a typed
`IWidgetHost` argument; Counter also receives `EventBinding`. Factories are the only parameterless activation roots.

During attachment the runtime queries and caches each control's layout capability
beside `IWidget`; later update, layout, and paint calls use those cached pointers.
External controls share the built-in ownership and routing rules.

The [implementation](src/lib.rs) uses `WidgetState` for common options, mount-once
attachment and sticky invalidation. The counter stores its value
once. `WidgetState` retains its optional receiver and posts through the existing
host; the owning runtime supplies the origin during update/dispatch. No cached
weak self-reference or cyclic construction is needed, including across DLLs.
`ui_event_sources! { CounterAbi => [IActivatedEventAbi, ICounterChangedEventAbi]; }` advertises the captured facts.
The counter captures each fact as an immutable `ComBox<IEvent>`, exposing the
common `IActivatedEvent` or custom `ICounterChangedEvent` interface. Both move
through the same `IEventReceiver` after state commits. Factories transfer widget
`ComBox` owners; destruction releases descendants and receiver references.

The [acceptance host](../../demos/smoke/src/extension.rs) uses `events::channel()`.
Its handler queries the public event interfaces and reads their getters after
traversal returns. `CounterAbi::EVENT_SOURCE.events` identifies ordinary interfaces; UI derives their getter
metadata to authoring tools. `ExtensionProvider` implements `IDesignWidgetCatalog`;
its common constructor receives `EventBinding`. Catalog registration validates
the event identities and getter value graph.
The unlinked native designer can load this library with `--plugin` and discover
the counter without a Rust registration hook. A UI document stores only its kind
and opaque NCB settings; it does not restate event contracts.

The inset exclusively owns one `ComBox<IChildrenEditor>` and exposes its read-only view. It reads
the authoritative child count instead of maintaining a second topology flag. Its
layout methods call indexed `ILayoutService` operations using only the supplied
scope; they retain neither that service nor its invocation token. Mutable contract
receivers and the generated object-wide call gate protect the object's inline state
across outward calls without an implementation-specific guard.

`IWidgetProbe` counts actual phase attempts for acceptance and measurement. Counts
are available while the owner exists. After removal the host verifies expiration
through retained `WeakComBox` projections; probes cannot preserve widget resources. These are example-specific diagnostics,
not a second widget state model or a required toolkit interface.

Authors remain responsible for truthful `CHANGED` results, external-state updates,
hidden-widget synchronization, observational layout/paint, event ownership and
explicit redraw scheduling. The complete obligations and failure rules are in
[COM_AUTHORS.md](../../docs/contracts/COM_AUTHORS.md).

The catalog and ordinary typed constructor share the same `Settings::default()`.
[settings.rs](src/settings.rs) owns a versioned record with a positive increment
and a nested `Mode::Unlimited | Mode::Bounded { minimum, maximum }` enum. Both the
runtime constructor and draft editor use that same checked decoder. Activation
increments by the configured step, clamps at the bound, and emits the existing
typed event stream. Application setters reject values outside a configured range.

The optional COM editor builds ordinary textboxes and a checkbox through the
host's `UiFactory`; it links no node implementation. Controls keep their own draft
text. Apply reads weak handles, parses all relevant fields, validates the complete
record, then encodes NCB. Invalid text/ranges remain editable. No event queue,
`RefCell`, dynamic property values or application-layout metadata is needed here.

`encode_settings`/`decode_settings` enforce the same 64 KiB NCB policy. Direct COM
calls reject nonzero offsets, unsupported revisions, malformed/trailing data and
invalid semantics before construction. Catalog descriptions retain their separate
bounded JSON encoding. See [PLUGIN_DESIGNER_SETTINGS.md](../../docs/contracts/PLUGIN_DESIGNER_SETTINGS.md).
