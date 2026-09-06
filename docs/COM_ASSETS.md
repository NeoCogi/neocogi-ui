# Immutable COM assets and themes

| Owner | Responsibility |
| --- | --- |
| `microui-contracts` | Plain resource records, semantic roles and typed COM interfaces. |
| `microui-redux::assets` | Bounded portable data validation and immutable bundle publication. |
| `plugins/assets` | PNG decoding, font rasterization, packing, theme/style compilation and source export. |
| `microui-ui` | Local IDs, text metrics, resolved appearances and retained layout/painting. |
| `plugins/platform` and renderers | Pixel upload, GPU resources and ordered frame execution. |

The asset plugin has no production dependency on `microui-ui`. UI construction
fixtures and a native flat-palette reference remain test-only; they provide an
independent check of portable compiler output. Runtime UI code only imports
resolved resources.

`microui-plugin-assets` without features registers `IPngDecoder` and
`IStyleCompiler`. The decoder's `upload` method accepts one complete, nonempty PNG at offset zero and borrows an
`ITextureFactory` for the upload. Encoded input and decoded RGBA storage each have
a 64 MiB ceiling; dimensions are checked before pixel allocation and animated PNG
is rejected. Invalid input never calls the factory. Factory errors preserve their
original status, and `IAssetDiagnostics` returns an owned failure snapshot. No
factory or temporary decoded buffer is retained after the call. Calls may reenter
diagnostics because the decoder holds no mutable borrow across the upload.

Enable `builder` to add font/atlas construction, or `theme-json` to add that builder
and JSON themes. Native applications can ship PNG and style services independently of
their precompiled atlas. Browser assemblies register the same decoder descriptor
statically. The [asset plugin's decoder](../plugins/assets/src/png.rs) produces
packed RGBA bytes directly. None of the asset plugin's production feature sets
links the UI implementation. Font and theme construction remain optional features.

Applications supply one `IAssetBundle` to `IUiProvider::create_runtime` and
`IPlatform::create_window`. Neither implementation embeds demo assets. The UI imports
metadata into its local font/icon tables once and samples only the required white-tile
rows while validating it. It never copies or retains the complete image. The platform
reads the bundle header and packed RGBA bytes, then uploads them without importing a
native atlas or skin. GPU backends retain textures and the platform retains the source
revision identity; temporary CPU upload bytes are released after the synchronous
upload. Frames pass the exact bundle used to expand their geometry, and the renderer
skips import/upload when that bundle is already current.

The facade separates `assets::AssetTables` metadata from complete `assets::AssetData`
pixel-bearing compiler output. `AssetData` produces the same `IAssetBundle` object from
borrowed static tables or owned builder output. Neither type contains a UI implementation,
font rasterizer, PNG decoder or JSON parser. `plugins/assets` packages the optional native
builder and, with `theme-json`, the theme compiler. Its objects expose ordinary typed
interfaces and keep all native builder/path/decoder state in that binary.
The [JSON compiler](../plugins/assets/src/theme.rs) reads a validated `AssetData`,
rebuilds declared resources, and writes complete appearance tables directly.
Plain geometry and semantic resource names live in contracts. The UI resolves
those records into its own runtime IDs only when a bundle is installed.
In-memory compilation accepts an explicit `AssetSource` implementation for
referenced files; filesystem compilation uses the bounded `FileAssets` source.

```rust,ignore
let assets = microui_redux_demo_assets::load_bundle()?;
let native = platform.create_window(
    NcCopyFromBufferOffset::new(0, b"Application"),
    Size { width: 800, height: 600 },
    assets.as_com_ref(),
)?;
let renderer = native.query::<IRendererAbi>()?;
let ui = Ui::from_interface(provider.create_runtime(assets.as_com_ref())?)?;
```

## Installation and ownership

`ui.install_assets(candidate, renderer)` owns one runtime transaction. It validates
and imports bounded metadata, asks the renderer to copy and upload the complete image,
then publishes only local lookup tables and resolved painting state. Only success
publishes the new revision to the window manager, built-in widgets, text metrics and
scoped layout services together. Failed validation or upload leaves the previous UI
resources and GPU atlas usable. The renderer is borrowed for the call; the UI retains
neither a platform/window nor a second CPU image merely to remember its resources.

After a changed revision succeeds, call `update` or `update_with_events` before
`render`. Old measurement entries, including hidden trees, are invalidated by the
shared skin revision. Installing the identical bundle does not invalidate unchanged
UI geometry, though the supplied renderer may need to switch back from another UI's
atlas. Application event handlers may install a revision synchronously: dispatch returns
the runtime context, and the enclosing update commits the changed layout before routing
later input or returning. No extra update is required after that successful transaction.
Manually dispatched handlers still require an ordinary update before rendering.
Installation from widget update, paint, layout or another installation returns
`UI_E_BUSY`. Read-only access to the old installed bundle remains available
while upload is in progress. No foreign call runs with a live runtime or GPU
`RefCell` guard.

`Ui::assets()` and `IWidgetHost::assets()` retain the current immutable revision.
Independent widget implementations can obtain the latter through their
`WidgetState::host()`. They must refresh any locally cached glyph metrics, icon
rectangles, colors or UVs when that bundle identity changes. Built-in widgets share
the runtime's cache; independent widgets may share a concrete local cache among
themselves. Fetch a bundle/table per revision, not per glyph. The ordinary
`ITextMetrics::button_size` and `IPainter::text` conveniences still use body-font
button styling; custom typography uses the full bundle records and bulk triangles.

Bundle producers must keep **every** method result immutable for the allocation's
lifetime. Mutation requires a new bundle object, even when pixels alone change.
Keep bundles and catalog identities independent of UI trees, application handlers
and native renderer ownership; otherwise an application can create its own COM
reference cycle. Retaining an old bundle or a copied diagnostic is supported and
does not retain any borrowed frame or builder draft.

## Catalogs and named resources

Each independent base bundle owns an `IAssetCatalog` namespace. Derived bundles
retain that same canonical COM identity. A receiving UI preserves its local typed
IDs only after authenticating this namespace and finding every original font and
icon name. An unrelated bundle with identical names and pixels is rejected with
`UI_E_OWNER`; an incomplete revision in the right namespace is rejected before GPU
preparation. No binary-local integer resource ID crosses COM.

`AssetData::into_bundle()` creates a new namespace. `into_revision(base)` retains
the base namespace; it is the producer's obligation to preserve the meaning of
existing names. Local builder/theme helpers also validate their original catalog
before publishing derived data. Keep the initial application bundle as the base
when loading unrelated themes; deriving repeatedly from the current theme would
make its private artwork part of the next derivation's input.

Built-in named font/icon setters resolve against the **original application
catalog**. Theme-private images remain revision-local appearance inputs. They are
not additional permanent names exposed by those setters. This prevents a retained
widget from carrying an image ID into a later sibling theme that lacks it. Semantic
font roles resolve `body`, `small`, `title`, `heading`, `mono`; optional font names
fall back to `body`. Required semantic icons are `close`, `expand`, `collapse`,
`check`, `expand_down`, `open_folder`, `closed_folder`, and `file`. The separate
`white` icon is the fully opaque white tile used for solid geometry.

## Portable data rules

`AssetInfo` supplies dimensions, allocation counts and `ASSET_SCHEMA_VERSION`.
Reject a different schema before allocation. The semantic schema includes role/state
table order and mandatory resource interpretation; changing those rules requires a
schema bump even when the C record layout is unchanged. The ordinary nc-com
interface hashes separately validate the actual pointer/vtable/record ABI.

| Storage | Bound and interpretation |
| --- | --- |
| Pixels | Positive `i32` dimensions; checked `width * height * 4`, at most 64 MiB; row-major uncompressed RGBA |
| Fonts | 1–4096 distinct names; positive size and baseline spacing, checked native coordinate metrics |
| Glyphs | At most 1,048,576 records total; valid Unicode scalars, no duplicates within a font, mandatory `_` fallback |
| Icons | 1–65,536 distinct names; positive rectangles wholly inside the atlas |
| Names | At most 4 MiB of nonempty UTF-8 names, without terminators |
| Visuals | Complete surface/control/menu/chrome families, role-major then state-major using `appearance::*::ALL` |

Fonts partition the glyph table in declaration order. Names partition their byte
table in font-then-icon order. Gaps, overlapping ranges, arithmetic overflow,
invalid UTF-8/Unicode and duplicate keys are rejected. Glyph rectangles may be empty
but must remain nonnegative and inside the atlas; icons must be positive. Every pixel
in the white tile must be opaque white. Portable layout and slicing dimensions are
nonnegative; source slices must fit their image. Boolean bytes accept only zero or
one; unknown family, caption and patch tags are rejected. Image patches refer to a
bundle-local icon table position, never an opaque native handle.

All copy offsets and lengths count elements of the declared type; name/pixel offsets
therefore count bytes. Producers validate the entire range before writing any
destination element, then write the exact requested count. Tables remain owned by
their binary. Construction services use `AssetData::read` when they need metadata and
the complete RGBA allocation. The UI uses `AssetTables::read`, which copies and validates
portable metadata and reads only white-tile rows from the pixel method. Both paths cover
duplicate keys, fallback glyphs, rectangle bounds, the white tile and resolved theme
records without depending on the UI implementation. UI installation additionally
authenticates its catalog and preserves its local resource identities.

The native builder currently rasterizes printable ASCII. Prebuilt/other producers
may supply any Unicode scalar, and runtime text substitutes `_` for a missing glyph.
This boundary does not add text shaping, font fallback chains or grapheme handling.
The receiving GPU may impose a smaller texture dimension limit than the portable
64 MiB policy; that upload is fallible and leaves the previous atlas installed.

## Optional construction and themes

`IAssetLoader::create_builder(size, white_path)` starts a native builder.
Its [implementation](../plugins/assets/src/atlas.rs) packs RGBA pixels and font
records directly into `AssetData`; it constructs no UI atlas handles or skins.
`add_icon(name, path)` and `add_font(name, path, pixels)` add distinct names;
duplicates are rejected. Native filesystem paths retain exact Unix bytes or Windows
UTF-16. A failed edit restores the draft. `finish()` consumes it even when final
validation fails; later calls return `UI_E_CLOSED`. Calls reentered from a supplied
path object return `UI_E_BUSY`. `derive_builder(size, base)` copies a validated base
catalog into a new packing area and may add private resources.

```rust,ignore
let draft = asset_loader.derive_builder(
    Size { width: 1024, height: 512 }, assets.as_com_ref(),
)?;
draft.add_icon(
    NcCopyFromBufferOffset::new(0, b"application-image"),
    NativePath::new(Path::new("image.png")).as_interface(),
)?;
let expanded = draft.finish()?;
ui.install_assets(expanded.as_com_ref(), renderer.as_com_ref())?;
ui.update(size)?;
```

The example's new image belongs to that revision; introduce permanent application
names in the original base instead when built-in named setters must retain them
through arbitrary later themes.

`IThemeLoader::load_theme(native_path, base)` performs JSON, font and PNG processing
entirely in the optional asset plugin. The result is an ordinary immutable bundle:

```rust,ignore
let theme = theme_loader.load_theme(
    NativePath::new(Path::new("themes/windows-95/theme.json")).as_interface(),
    assets.as_com_ref(),
)?;
ui.install_assets(theme.as_com_ref(), renderer.as_com_ref())?;
ui.update_with_events(size, &events)?;
```

Recoverable failures return statuses. `IAssetDiagnostics` retains actual native,
decoder or compiler detail; argument validation may supply no additional text.
Diagnostics are immutable snapshots and remain inspectable after a consuming failed
finish. Renderer upload details use `IRenderDiagnostics`.

## Live metric and palette edits

Activate `STYLE_COMPILER_CLASS` as `IStyleCompiler`. Its `derive_style` method takes
an explicit base bundle, complete metrics and a closed `PaletteEdit` value.
`Preserve` retains existing appearance tables; `Flat` compiles all role/state
values using the base frame geometry and updates an enabled title backdrop.
The result retains the base catalog and does not install itself. Applications
pass it to `ui.install_assets` through the same transaction as any loaded theme.
Compilation remains usable after a UI closes because the service owns no UI state.

## Compiled resources and packaging

`microui_plugin_assets::export::write_rust(&data, path)` (feature `save-to-rust`)
atomically writes one self-contained source
file from validated `AssetData`. The UI crate has no builder, PNG, JSON or source
export features. Cargo build scripts and command-line tools call the same asset
compiler directly. `load_bundle()` creates a fresh catalog; `bundle_data()` supplies the same
tables for `into_revision(base)` when exporting a compatible theme. Both functions
return `Result`; use `bundle_data()?.into_revision(base)`. Generated code requires
facade `compiled-assets`, borrows names/glyphs/font/icon records, expands compressed
pixels into one owned allocation, and owns the resolved visual arrays. No generated
file exposes `AtlasHandle`, native IDs, UI implementation storage or parser APIs.

```sh
cargo run -p microui-redux-tools --bin atlas-export -- --output bundle.rs
cargo com-build --demo full --backend glow
cargo com-build --demo full --backend glow --assets embedded
```

The stage is `target/com-native/{profile}/{backend}/{asset-mode}/{demo}`. Native
`files` is the default: it contains the runtime, UI/platform/asset libraries and
adjacent assets. The asset plugin builds the atlas from font/icon files and loads
JSON themes; the host does not embed the photo, mesh or compiled atlases. Register
the plugin catalog before loading a bundle. Resource paths resolve against the
executable, so the complete staged directory can run from another working directory.

`embedded` explicitly enables the demo's `embed-assets` Cargo feature. It compiles
atlases and themes during the build and embeds the full demo's photo and mesh.
The full demo still ships the asset plugin's PNG decoder for the photograph and
style compiler for live edits; simpler embedded demos need no asset plugin.
Browser `webgl` explicitly enables embedding.
The shared recipe gives both modes identical resource names and font sizes.

The smoke executable's `--rebuild-assets` and `--theme themes/windows-95/theme.json`
options require the file package's construction interfaces. Both can be combined
with `--smoke` from another cwd.

Compiled pixels use zlib, while the COM asset contract still copies ordinary RGBA.
The generated loader validates the exact output length and shared 64 MiB limit,
checksum and stream completion before constructing a bundle. It rejects truncation,
trailing bytes and shorter/longer output. Pixel storage is freed with the bundle;
there is no process-global decoded cache. Each fresh load or theme candidate performs
its own expansion, so applications should retain their installed bundle rather than
reloading it per frame. The complete publication/upload path remains transactional.

Build-host font/image/theme parsers remain absent from a compiled-only application
target. The optional small inflater has an activation-time and owned-memory cost;
executable bytes, every shared library, external assets, startup and loaded memory
all belong in deployment accounting. The full demo's separate sample PNG is decoded
by its asset plugin and is not part of the generated pixel-stream format.
