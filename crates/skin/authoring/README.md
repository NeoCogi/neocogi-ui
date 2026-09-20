# Shared theme authoring

`neocogi-skin-authoring` owns the theme schema and compilation policy shared by
the Skin plugin and tools. It consumes the generated GFX loader and publisher
interfaces; GFX still owns decoding, rasterization, packing, and bundle storage.

The default build has no GFX implementation dependency. Offline exporters may
enable `builder` to instantiate the same services locally. Existing callers of
`neocogi_skin_impl::theme` retain a compatibility re-export, so this extraction
does not change theme output or require a deployment contract change.

Run `cargo test -p neocogi-skin-authoring --features builder` for schema,
resource, bundled-theme, and offline/plugin equivalence checks. Skin's own
integration test additionally exercises the deployed loader boundary.
