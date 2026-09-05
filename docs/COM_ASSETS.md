# Immutable COM assets and themes

`microui-plugin-assets` without features now registers only `IPngDecoder`. Its
`upload` method accepts one complete, nonempty PNG at offset zero and borrows an
`ITextureFactory` for the upload. Encoded input and decoded RGBA storage each have
a 64 MiB ceiling; dimensions are checked before pixel allocation and animated PNG
is rejected. Invalid input never calls the factory. Factory errors preserve their
original status, and `IAssetDiagnostics` returns an owned failure snapshot. No
factory or temporary decoded buffer is retained after the call. Calls may reenter
diagnostics because the decoder holds no mutable borrow across the upload.

Enable `builder` to add font/atlas construction, or `theme-json` to add that builder
and JSON themes. Native applications can ship a PNG-only plugin independently of
their precompiled atlas. Browser assemblies register the same decoder descriptor
statically. This separation keeps optional construction code out of PNG-only
deployments while using the existing bounded image decoder.

Applications supply one `IAssetBundle` to `IUiProvider::create_runtime` and
`IPlatform::create_window`. Neither implementation embeds demo assets. The UI imports
metadata into its local font/icon tables once; the renderer imports and uploads its
own native atlas. Frames pass the exact bundle used to expand their geometry, and
the renderer skips import/upload when that bundle is already current.

The facade's concrete `assets::AssetData` produces the same `IAssetBundle` object
from borrowed static tables or owned builder output. It contains no UI implementation,
font rasterizer, PNG decoder or JSON parser. `plugins/assets` packages the optional
native builder and, with `theme-json`, the theme compiler. Its objects expose ordinary
typed interfaces and keep all native builder/path/decoder state in that binary.

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
the complete candidate, prepares its built-in painting workspace, then asks the
renderer to upload. Only success publishes the new revision to the window manager,
built-in widgets, text metrics and scoped layout services together. Failed validation
or upload leaves the previous UI resources and GPU atlas usable. The renderer is
borrowed for the call; the UI does not retain a platform/window merely to remember
its resources.

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
their binary. Importers use the common atlas finalizer for duplicate keys, fallback
glyphs, rectangle bounds and white-tile validation, rather than maintaining another
semantic validator for COM.

The native builder currently rasterizes printable ASCII. Prebuilt/other producers
may supply any Unicode scalar, and runtime text substitutes `_` for a missing glyph.
This boundary does not add text shaping, font fallback chains or grapheme handling.
The receiving GPU may impose a smaller texture dimension limit than the portable
64 MiB policy; that upload is fallible and leaves the previous atlas installed.

## Optional construction and themes

`IAssetLoader::create_builder(size, white_path)` starts a native builder.
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

## Compiled resources and packaging

`SkinBundle::to_com_rust_files(path)` atomically writes one self-contained source
file. `load_bundle()` creates a fresh catalog; `bundle_data()` supplies the same
tables for `into_revision(base)` when exporting a compatible theme. Both functions
return `Result`; use `bundle_data()?.into_revision(base)`. Generated code requires
facade `compiled-assets`, borrows names/glyphs/font/icon records, expands compressed
pixels into one owned allocation, and owns the resolved visual arrays. No generated
file exposes `AtlasHandle`, native IDs, UI implementation storage or parser APIs.

```sh
cargo run -p microui-redux-tools --bin atlas-export -- --output bundle.rs
cargo run -p microui-redux-tools --bin com-build -- --backend glow --assets prebuilt
cargo run -p microui-redux-tools --bin com-build -- --backend glow --assets builder
cargo run -p microui-redux-tools --bin com-build -- --backend glow --assets theme-json
```

The stage is `target/com-native/{profile}/{backend}/{asset-mode}/{demo}`. `prebuilt`
contains the executable, runtime, UI and platform libraries. `builder` adds the
font/PNG asset plugin. `theme-json` adds the same plugin with JSON support plus the
bundled theme/font source files. The host accepts `--rebuild-assets` and
`--theme themes/windows-95/theme.json`; relative theme paths resolve against its
staged executable directory. Both can be combined with `--smoke` from another cwd.

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
