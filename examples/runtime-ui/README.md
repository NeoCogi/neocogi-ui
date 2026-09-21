# Runtime UI loading example

This application consumes an opaque component contract: output `save_requested`
(`Activation`) and input `title` (`SetText`). It has no handles to the component's
private controls. Clicking Save sends an application event; the application
updates the title through its named input. File → Reload replaces the UI only
after loading and connecting a complete candidate.

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
application intentionally requires its public contract; layout changes are
allowed, while removing or changing a required output fails binding and preserves
the current view. Browser builds start with the built-in document; byte fetching
is an application transport concern. Resource image keys require a host-supplied
`Resources` directory; this minimal example supplies none.

See [the runtime document guide](../../crates/ui/document/README.md) for schema,
resource ownership, typed inputs, generation filtering, and save semantics.
