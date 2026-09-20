# Shared theme authoring

`neocogi-skin-authoring` owns the theme schema and compilation policy shared by
the Skin plugin and tools. It consumes the generated GFX loader and publisher
interfaces; GFX still owns decoding, rasterization, packing, and bundle storage.

The default build has no GFX implementation dependency. Offline exporters may
enable `builder` to instantiate the same services locally. Existing callers of
`neocogi_skin_impl::theme` retain a compatibility re-export, so this extraction
does not change theme output or require a deployment contract change.

`theme::ThemeDocument` is the editable source model. `from_bytes` accepts
version-1 and version-2 documents and normalizes the schema version without
rewriting authored paths. `to_bytes` writes version 2. `compile` anchors a
temporary snapshot and delegates resources/publication to supplied GFX
capabilities, leaving the editable document unchanged. New visual fields cover
per-state insets, center visibility, and all nine independent flat cell colors.
Normal mutable field access keeps editor commands typed; invalid role values
and geometry are rejected before publication.

Run `cargo test -p neocogi-skin-authoring --features builder` for schema,
resource, bundled-theme, and offline/plugin equivalence checks. Skin's own
integration test additionally exercises the deployed loader boundary.
