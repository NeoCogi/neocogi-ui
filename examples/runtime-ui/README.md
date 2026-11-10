# Runtime UI loading example

This application binds the `save` button's `IActivatedEvent` output directly and
checks the authored `title` node as a concrete `TextBlockAbi` widget. Clicking
Save queues an application message; its handler updates the title through that
typed widget handle at the normal update boundary. File → Reload replaces the UI
only after loading and connecting a complete candidate.

One `events::channel()` receiver serves buttons and menu/window services and is
reused across reloads. Messages retain the exclusive COM event and weak origin.
Before applying Save, the application checks origin
liveness and compares it with the current component's `save` field. A queued event
from a removed form is ignored; observers cannot retain the removed widget.
The reload command is likewise checked against the actual static menu entry.
There are no per-widget mapping callbacks or receiver allocations.

Application startup owns `ReloadIds` alongside the channel and passes it to the
slot constructor. Additional or recreated slots sharing this message flow must
reuse that allocator. Reload tokens contain `(slot_id, attempt)`; this example's
Save handler continues to authenticate the widget origin directly. See the
[reload identity contract](../../docs/contracts/RELOAD_GENERATION_IDENTITY.md).

The `neocogi-runtime-ui-example` Cargo package builds `neocogi-example-ui`.
Its build stages the selected native providers beside that executable.

```sh
cargo build -p neocogi-runtime-ui-example
cargo run -p neocogi-runtime-ui-example
cargo run -p neocogi-runtime-ui-example -- path/to/form.ui.json
cargo build -p neocogi-runtime-ui-example --no-default-features --features monolith,glow,embed-assets
cargo check -p neocogi-runtime-ui-example --target wasm32-unknown-unknown --no-default-features --features webgl
```

With no path the same loader receives the built-in authored document. Native
paths may contain editable JSON or the binary exported by the designer. The
application requires the Save event and concrete title widget; layout changes
are allowed, while removing or changing either capability fails candidate binding
and preserves the current view. Browser builds start with the built-in document; byte fetching
is an application transport concern. Resource image keys require a host-supplied
`Resources` directory; this minimal example supplies none.

See [the runtime document guide](../../crates/ui/README.md) for schema,
resource ownership, generated typed forms, generation filtering, and save semantics.
