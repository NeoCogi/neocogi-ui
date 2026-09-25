# Runtime UI loading example

This application binds the form-owned `save_requested: Activation` event relay and
checks the authored `title` node as a concrete `TextBlockAbi` widget. Clicking
Save queues an application message; its handler updates the title through that
typed widget handle at the normal update boundary. File → Reload replaces the UI
only after loading and connecting a complete candidate.

The `runtime-ui` assembly stages `neocogi-example-ui`. Its Cargo package remains
`neocogi-runtime-ui-example`; use `--bin neocogi-example-ui` when selecting the
binary explicitly with ordinary Cargo.

```sh
cargo neocogi build --assembly runtime-ui
cargo neocogi run --assembly runtime-ui
cargo neocogi run --assembly runtime-ui -- path/to/form.ui.json
cargo neocogi build --assembly runtime-ui --variant monolith
cargo check -p neocogi-runtime-ui-example --target wasm32-unknown-unknown --no-default-features --features webgl
```

With no path the same loader receives the built-in authored document. Native
paths may contain editable JSON or the binary exported by the designer. The
application requires the Save event and concrete title widget; layout changes
are allowed, while removing or changing either capability fails candidate binding
and preserves the current view. Browser builds start with the built-in document; byte fetching
is an application transport concern. Resource image keys require a host-supplied
`Resources` directory; this minimal example supplies none.

See [the runtime document guide](../../crates/ui/document/README.md) for schema,
resource ownership, generated typed forms, generation filtering, and save semantics.
