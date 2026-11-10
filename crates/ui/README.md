# NeoCogi UI static SDK

`neocogi-ui` reexports canonical `neocogi-ui-contracts` clients and provides the optional implementations.
Enable `ui` for retained runtime and built-in nodes, `gfx` for graphics services, `builder` for atlas/font construction,
`skin` for appearance compilation, `theme-json` for external themes and `document` for UI documents.
Empty defaults keep implementation modules unlinked. `plugin_descriptor()` supplies one `neocogi.ui` root;
its typed service getters return `NC_E_NOT_FOUND` when their implementation feature is disabled.

The same code is linked by `neocogi-ui-dylib`. No implementation module owns a separate plugin identity.
