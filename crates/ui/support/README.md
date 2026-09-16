# UI provider support

This is a non-deployable Rust helper crate, not another UI implementation or a COM plugin.
It shares binary-local imports of canonical resource contracts, pure geometry/text algorithms,
and phase-local drawing helpers. It has no retained tree, node allocation, host authority,
window policy, runtime factory, concrete widget catalog, or plugin entrypoint.

Core keeps ownership of retained traversal and GFX replay/submission. The nodes provider uses
the generated core host, layout, and painter interfaces, not core implementation storage.
Neither provider links the other provider's implementation. Shared Rust helpers do not create
an ABI: provider interaction is always through the generated NCIDL contracts.

The opt-in `test-support` feature supplies deterministic canonical assets and snapshot
constructors. It does not select a different runtime, dispatcher, layout engine, or compositor.
Pure helper unit tests live beside their production algorithms in this crate.
