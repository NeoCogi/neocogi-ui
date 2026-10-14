# Shared theme authoring

`neocogi-skin-authoring` owns the theme schema and compilation policy shared by
the Skin plugin and tools. It consumes the generated GFX loader and publisher
interfaces; GFX still owns decoding, rasterization, packing, and bundle storage.
It also owns artwork name allocation through `IAssetBuilder::add_artwork_bytes`.
Skin records a semantic icon role or an artwork purpose and uses the returned
placement; no generated private name string is retained in its recipe. Valid
application keys are preserved, including keys resembling artwork names. Failed
insertions consume neither names nor packing space. Offline and supplied-service
compilation use this same deterministic builder operation.

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

Live tools retain `ThemeCompiler`, which owns one stable application base and
one successful resource build. Colors, geometry, tint, and slicing reuse those
placements; changing resource recipes rebuilds from the original base instead
of accumulating generated images. Replacing bytes at an unchanged path requires
`invalidate_resources`. No implicit file polling or renderer state is retained.

Run `cargo test -p neocogi-skin-authoring --features builder` for schema,
resource, bundled-theme, and offline/plugin equivalence checks. Skin's own
integration test additionally exercises the deployed loader boundary.

Theme JSON uses concrete Serde adapters for generated metrics, palette, colors,
and insets. Ordinary authored chrome records derive Serde directly. There is no
generic COM-to-Serde decoding layer. Missing metric/palette fields inherit the
canonical generated defaults; duplicate and unknown record fields are rejected.
Colors contain exactly four byte channels and flat patches exactly nine colors.
The only custom map visitor resolves a closed role-name domain and rejects
duplicate identities before reading replacement values, bounding entries by the
actual number of roles. Runtime COM serialization remains independent.

Theme document clones share skin, appearance, font and icon sections through `Arc`.
Use `skin_mut`, `appearances_mut`, `fonts_mut` and `icons_mut` on an unpublished
candidate for copy-on-write changes. Published roots can remain immutable behind
`Arc<ThemeDocument>`. Sharing is per section, not per map entry; JSON and COM
contracts do not contain reference-counting state and their representation is unchanged.
