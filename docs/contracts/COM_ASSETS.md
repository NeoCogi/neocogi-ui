# Immutable COM assets and themes

| Owner | Responsibility |
| --- | --- |
| `neocogi-renderer-contracts` | Renderer-neutral geometry, images, textures and bulk frame interfaces. |
| `neocogi-ui-core-contracts` | Shared widget/layout, window, popup, menu, and file-dialog protocols. |
| `neocogi-ui-nodes-contracts` | Concrete built-in widgets, containers, specialized events, and factory. |
| `neocogi-gfx-contracts` / `neocogi-gfx` | Portable bundle records, PNG upload, font/icon rasterization, and atlas packing. |
| `neocogi-skin-contracts` / `neocogi-skin` | Typed appearance derivation and optional external JSON themes. |
| `neocogi-gfx-support` | Bounded portable data validation, immutable publication, and embedded expansion; no plugin identity. |
| `crates/gfx/impl` | Private decoder, atlas, bundle, and source-export implementation; no plugin identity. |
| `crates/appearance` | Pure typed appearance compilation shared with offline resource construction. |
| `crates/skin/impl` | Private style and optional JSON-theme implementation; no plugin identity. |
| `crates/skin/authoring` | Concrete theme-file schema, Serde codecs, authoring state and appearance compilation. |
| `neocogi-ui-support` | Non-plugin table retention, pure geometry/text algorithms, and phase-local presentation helpers; no retained tree or concrete classes. |
| `neocogi-ui-runtime` | Core retained layout, painting, surfaces, and menu policy. |
| `neocogi-ui-nodes` | Built-in controls and standard compositions over one bound runtime resource source. |
| `crates/render/plugin` and renderers | Pixel upload, GPU resources and ordered frame execution. |

The GFX plugin has no production dependency on either the UI facade or runtime;
the skin plugin has no production dependency on `neocogi-ui-runtime`. UI construction
fixtures construct canonical `AssetData` directly and pass through its GFX validator.
Runtime UI code retains the resulting `AssetTables` collection of generated NCIDL records without exporting or reimporting
a native atlas model. Full pixel copies needed to publish fixture revisions live in
explicit test setup and do not alter runtime import.

`neocogi-gfx` registers `PngDecoder`; its optional `builder` feature also
registers `AssetLoader`. `PngDecoder::upload` accepts one complete, nonempty PNG at offset zero and borrows an
`ITextureFactory` for the upload. Encoded input and decoded RGBA storage each have
a 64 MiB ceiling; dimensions are checked before pixel allocation and animated PNG
is rejected. Invalid input never calls the factory. Factory errors preserve their
original status, and `IGfxDiagnostics` returns an owned failure snapshot. No
factory or temporary decoded buffer is retained after the call. Calls may reenter
diagnostics because the decoder holds no mutable borrow across the upload.

`neocogi-skin` registers `StyleCompiler`; its optional `theme-json` feature adds
`ThemeLoader` for external schema and appearance compilation. `StyleCompiler`
retains a narrow GFX `AssetPublisher`; `ThemeLoader` additionally retains the
optional `AssetLoader` used for external resources. The Skin plugin declares GFX
as a hard provider dependency. Skin reads each bounded external font or PNG and
submits its bytes to the loader; it does not link another copy of the font
rasterizer, PNG decoder, atlas packer, bundle reader, validator, or publisher.
After resolving its closed appearance records, Skin calls GFX once to validate
and publish the immutable revision. Native applications can therefore ship GFX
services without a JSON/schema surface, while a JSON-enabled Skin deployment
selects GFX as its one resource owner. Browser assemblies register the same
selected descriptors statically. The [GFX decoder implementation](../../crates/gfx/impl/src/png.rs)
produces packed RGBA directly. Neither plugin links the UI runtime implementation.

The JSON feature uses the concrete schema in `neocogi-skin-authoring`.
[Theme decoding](../../crates/skin/authoring/src/theme.rs) and
[record codecs](../../crates/skin/authoring/src/encoding.rs) translate Serde input
into generated GFX records using explicit field and enum definitions. GFX
contracts themselves have no Serde dependency. The former generic Serde-to-COM
visitor bridge is removed; COM's local persistence protocol is independent of
the theme-file format. Unknown and duplicate fields and
[typed role keys](../../crates/skin/authoring/src/role_keys.rs) are rejected during decoding.

Applications supply one `IAssetBundle` through the typed `UiRuntime` factory and
pass `bundle.image()` to `Platform::create_window`. Neither implementation
embeds demo assets. Version 4 of `IAssetBundle` returns a separate immutable
`IRgbaImage` capability instead of inheriting the pixel interface. Appearance-only
revisions retain exactly that image identity; resource rebuilds publish a new image.
The image owns its bytes independently and retains no bundle or publication chain. The UI copies and validates
`AssetTables` once, retains that exact generated snapshot, and samples only the required white-tile
rows during validation. It never copies or retains the complete image. The platform
receives `IRgbaImage`, reads only its dimensions and packed RGBA bytes, then uploads
them without importing a native atlas or skin. GPU backends retain textures and the
renderer retains only the image projection; temporary CPU upload bytes are released
after the synchronous upload. Frames pass that cached projection, and the renderer
skips import/upload when the same image is already current.

The generated font, glyph, icon, metric, chrome, and appearance records remain the sole UI resource
model. `AtlasHandle` adds opaque catalog-scoped IDs in canonical table order, sorted font/icon name
slots, and a scalar-sorted glyph-slot index for each unchanged font partition. It neither copies
records into native structs nor introduces randomized hash maps. `Skin` borrows metrics and chrome
directly from the snapshot and selects a generated appearance record with typed state matching;
only the short-lived renderer-facing `Visual` is projected at the call site.

GFX support separates `AssetTables` metadata from complete `AssetData`
pixel-bearing compiler output. The UI facade keeps compatibility re-exports, but
lower layers depend directly on `neocogi-gfx-support`. `AssetData` produces the same `IAssetBundle` object from
borrowed static tables or owned builder output. Neither type contains a UI implementation,
font rasterizer, PNG decoder or JSON parser. Separate `neocogi-gfx-impl` and
`neocogi-skin-impl` packages supply private objects to their corresponding
plugin roots. The small `neocogi-appearance` package contains only pure typed
appearance compilation used by Skin/offline resource construction and canonical test fixtures.
The objects expose ordinary typed interfaces and keep
all native builder/path/decoder state in their owning binary. The
[JSON compiler](../../crates/skin/authoring/src/theme.rs) submits declared resources to
GFX and compiles complete appearance tables without copying the bundle into a runtime skin model.
GFX owns bundle transfer, validation, and publication through the format-neutral
`AssetPublisher::revise_appearance` transaction. Skin resolves one private typed `ThemeRecipe`
for both deployment modes. Offline build tools statically instantiate the same GFX loader and
publisher, execute that recipe, and copy the completed bundle back into `AssetData` for source
export; Skin contains no second native packing path. Plain geometry and semantic resource names
live in the GFX contract. The UI resolves those records into its own runtime
IDs only when a bundle is installed. In-memory offline compilation accepts an
explicit `AssetSource` implementation for referenced files; runtime filesystem
compilation uses the bounded `FileAssets` source.

```rust,ignore
let assets = neocogi_demo_assets::load_bundle()?;
let atlas = assets.image()?;
let native = platform.create_window(
    NcCopyFromBufferOffset::new(0, b"Application"),
    Size { width: 800, height: 600 },
    atlas.as_com_ref(),
)?;
let renderer = native.query::<IRendererAbi>()?;
let runtime = neocogi_com::Runtime::global();
let gfx = runtime.activate_plugin_factory::<GfxPluginFactoryAbi>(
    neocogi_com::guid_from_qualified_name("neocogi.gfx.runtime"),
)?;
let runtime_factory = runtime.activate_plugin_factory::<UiPluginFactoryAbi>(
    neocogi_com::guid_from_qualified_name("neocogi.ui.runtime"),
)?;
let ui_runtime = runtime_factory.create_runtime(assets.as_com_ref(), gfx.as_com_ref())?;
let node_provider = runtime.activate_plugin_factory::<NodesPluginFactoryAbi>(
    neocogi_com::guid_from_qualified_name("neocogi.ui.nodes.runtime"),
)?;
let factory = node_provider.bind(ui_runtime.as_com_ref())?;
let mut ui = Ui::from_interfaces(ui_runtime, factory)?;
```

## Installation and ownership

UI construction takes a shareable `GfxPluginFactory`. After validating the supplied
bundle, the runtime asks that factory for a fresh compositor and keeps its mutable
capability private. Each runtime owns its resource selection, frame state, and GFX
stream storage. Applications can reuse the factory and immutable bundles across a
tools UI and multiple preview UIs; creating, editing, or closing a preview cannot
change another runtime's compositor resources. Different runtimes may also start
with unrelated asset catalogs.

Standalone GFX clients can still call `gfx.create_compositor(assets)` directly.
Cloning a compositor reference aliases one mutable session; it does not copy the
session. Draw errors preserve complete vertex/batch coverage; direct-client
continuation and frame cleanup follow the [render contract](RENDER_SURFACES.md).
A compositor can no longer be supplied to UI construction. Migrate former
`create_runtime(assets, compositor)` calls to `create_runtime(assets, gfx)` and remove
the caller's compositor allocation. `UiPluginFactory` is now version 3, so rebuild
the application and plugins together. Shared libraries, native monoliths, and
WebAssembly all use this same factory contract; core UI links only GFX contracts.

`ui.install_assets(candidate, renderer)` requires mutable access to the `Ui` because it
replaces the resource value shared by that runtime's nodes and services. The call owns one
transaction: it validates and imports bounded metadata, asks the renderer to prepare
the image (copying/uploading only if its identity changed),
installs the same catalog-compatible revision in its private GFX compositor,
then publishes the retained canonical tables and their local lookup indices. Only success
publishes the new revision to `IWidgetHost`, the window manager, text metrics, and
scoped layout services together. Failed validation or upload leaves the previous UI
resources and GPU atlas usable. The renderer is borrowed for the call; the UI retains
neither a platform/window nor a second CPU image merely to remember its resources.
The compositor retains the narrow image projection used for final renderer submission.

After a changed revision succeeds, call `update` or `update_with_events` before
`render`. Old measurement entries, including hidden trees, are invalidated by the
shared skin revision. Installing the identical bundle does not invalidate unchanged
UI geometry, though the supplied renderer may need to switch back from another UI's
atlas. Event handlers may prepare and retain an immutable candidate, but they install it
only after `update_with_events` returns because that call still holds shared runtime access.
Installation from event dispatch, widget update, paint, layout or another installation
returns `NC_E_BUSY`. The renderer may observe the old installed bundle through an
independent `IWidgetHost`; overlapping calls through the same `UiRuntime` allocation remain
excluded until upload and publication finish. No foreign call runs with a live resource-cell
borrow.

`Ui::assets()` and `IWidgetHost::assets()` retain the current immutable revision.
Independent widget implementations can obtain the latter through their
`WidgetState::host()`. They must refresh any locally cached glyph metrics, icon
rectangles, colors or UVs when that bundle identity changes. Built-in widgets follow
this same public host boundary and keep a local cache per immutable revision;
independent widgets may share their own concrete cache. Fetch a bundle/table per
revision, not per glyph. The ordinary
`ITextMetrics::button_size` and `IPainter::text` conveniences still use body-font
button styling; custom typography uses the full bundle records and bulk triangles.
The core runtime imports these records once. A bound nodes `UiFactory` creates one
provider-local `Rc<RefCell<ImportedAssets>>` from the public host resource capability and shares
it with every built-in product and fixed internal child. Stable widget phases do not decode the
portable tables again; a phase refreshes that cache only after the host publishes a new revision.
The `RefCell` is intentional here because independent COM objects share the cache and their
generated receivers cannot borrow one factory mutably for every product's lifetime. Semantic
widget fields continue to use exclusive `&mut self` methods.

Refresh takes a short cache borrow to copy an owned `ImportedCatalog`: the
authenticated original namespace and original local names/IDs. Its clones use
local `Rc` operations, including for the retained COM identity, so acquiring the
basis invokes no foreign lifetime method. It retains no obsolete bundle/image
and copies no metadata tables. The cache borrow ends before `catalog`, `image`,
bulk getters or renderer preparation run. Core installation uses the same owned
basis. The core cache holds an immutable revision through a local `Rc`; host
getters clone that local owner before foreign `AddRef`, and identical-bundle
renderer preparation uses it after releasing the cache borrow. The slot and
the selected immutable value have separate lifetimes, without duplicating tables.

During lazy node refresh, a provider getter may reenter a different widget sharing
the cache. That sibling may finish and publish immediately. The outer import
rechecks the installed host identity before publication and preserves an already
published matching sibling revision. If a getter installs a third host revision,
the stale outer candidate is discarded and returns `NC_E_BUSY`; the next call
imports or reuses the current source. There is no retry loop inside the getter.
Failed candidates publish nothing and do not roll back a nested sibling commit.
Replaced or losing values are dropped after releasing the cache borrow because
provider destruction can also reenter a sibling. With a stable host, later sibling
setters, measurement and paint reuse the completed import without decoding again.

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
Reject a different schema before allocation. The semantic schema includes dense role
order, named appearance-state fields, and mandatory resource interpretation; changing
those rules requires a schema bump even when the C record layout is unchanged. The
ordinary neocogi-com interface signatures separately validate the actual
pointer/vtable/record ABI.

The current bulk asset schema is **3**. Schema 3 replaces source-slice rejection
with proportional shrinking and a visible diagnostic in GFX. Older semantic
schemas are rejected before table allocation. The authoring JSON schema remains
separate and unchanged.

| Storage | Bound and interpretation |
| --- | --- |
| Pixels | Positive `i32` dimensions; checked `width * height * 4`, at most 64 MiB; row-major uncompressed RGBA |
| Fonts | Nonempty; distinct names, positive size and baseline spacing, checked native coordinate metrics |
| Glyphs | At most 1,048,576 records total; valid Unicode scalars, no duplicates within a font, mandatory `_` fallback |
| Icons | Nonempty; distinct names, positive rectangles wholly inside the atlas |
| Named assets | Fonts and icons combined: at most 16,384 records, including retained inherited resources |
| Names | Each nonempty UTF-8 name: at most 2,048 bytes, without terminators; total storage is bounded by the actual named-asset count times 2,048 (at most 32 MiB) |
| Visuals | One generated named-state record per dense surface/control/menu/chrome role; control pointer states remain nested under enabled and focused branches |

Fonts partition the glyph table in declaration order. Names partition their byte
table in font-then-icon order. Gaps, overlapping ranges, arithmetic overflow,
invalid UTF-8/Unicode and duplicate keys are rejected. Glyph rectangles may be empty
but must remain nonnegative and inside the atlas; icons must be positive. Every pixel
in the white tile must be opaque white. Portable layout and slicing dimensions are
nonnegative. Oversized opposing source slices remain valid and are retained as
authored for the compositor's diagnostic rule below. Boolean bytes accept only zero or
one; unknown caption and patch tags are rejected. Image patches refer to a
bundle-local icon table position, never an opaque native handle.

Builder admission checks the name length and combined font/icon count before
source reading, decoding or packing. Derived drafts count the resources they
retain; explicitly omitted semantic replacements free their slots. Font insertion
also checks the inherited-plus-new glyph budget before rasterization. Rejected
additions preserve the draft. The name-pool ceiling is derived from the count and
per-name limits, so successful additions cannot violate a separate smaller pool
budget at publication. Final validation remains mandatory and `finish` remains
terminal, including failure due to missing required resources or other causes.

GFX partitions each source and destination axis independently. Opposing sides
that exceed an extent shrink proportionally, rounding the leading side once and
assigning the exact remainder to the trailing side. The requested sum, rounded
product and division stay in `i64`; only bounded final cells narrow to `i32`.
This applies even to two `i32::MAX` sides, without reducing the accepted domain.

When either **source** axis requires shrinking, each surviving visible image cell
is followed by a straight-alpha pink (`#FFC0CB`) overlay sampled from the validated
opaque-white tile. Opacity 0.5 is rounded to alpha byte 128 (128/255). The image's
original tint is preserved; source-over blending makes pink visible even on black
artwork. Destination-only shrinking and exact source fits receive no overlay.
Empty, hidden-center, zero-source and alpha-zero image cells remain absent; the
diagnostic covers surviving cells, including transparent pixels inside them.
It creates no missing source pixels or new stretchable center. Image and diagnostic
quads are staged and appended atomically with the existing clipping/budget rules.
Stored appearances, authored themes and direct `NinePatchDraw` operations share
this behavior; negative insets and invalid image indices still fail validation.

All copy offsets and lengths count elements of the declared type; name/pixel offsets
therefore count bytes. Producers validate the entire range before writing any
destination element, then write the exact requested count. Tables remain owned by
their binary. The GFX construction service uses `AssetData::read` when it needs
metadata and the complete RGBA allocation; Skin does not import that implementation.
The UI uses `AssetTables::read`, which copies and validates portable metadata, reads only
white-tile rows from the pixel method, and becomes the runtime's immutable metadata snapshot.
Both paths cover
duplicate keys, fallback glyphs, rectangle bounds, the white tile and resolved theme
records without depending on the UI implementation. UI installation additionally
authenticates its catalog and preserves its local resource identities.

The native builder currently rasterizes printable ASCII. Prebuilt/other producers
may supply any Unicode scalar, and runtime text substitutes `_` for a missing glyph.
This boundary does not add text shaping, font fallback chains or grapheme handling.
Text measurement and wrapping keep signed pen advance separate from glyph right
extent. Both whole-word fit and character fallback append canonical metrics to
the same horizontal run state; token ink widths are never added as spacing.
Fit comparisons use the mathematical extent before saturation, while each
reported line width equals atlas measurement of its exact UTF-8 source slice.
Wrapping preserves ASCII-space boundaries, missing-character fallback and the
editable trailing blank row. An individual oversized scalar remains indivisible.
Trials scan only the new token or scalar, keeping layout linear in text length.
The receiving GPU may impose a smaller texture dimension limit than the portable
64 MiB policy; that upload is fallible and leaves the previous atlas installed.

## Optional construction and themes

`AssetLoader::create_builder(size, white_path)` starts a native builder.
Its [implementation](../../crates/gfx/impl/src/atlas.rs) packs RGBA pixels and font
records directly into `AssetData`; it constructs no UI atlas handles or skins.
`add_icon(name, path)` and `add_font(name, path, pixels)` add distinct names;
duplicates are rejected. `add_icon_bytes` and `add_font_bytes` provide the same
bounded operations for a caller that already owns a complete encoded resource;
the builder does not retain those source bytes. Native filesystem paths retain
exact Unix bytes or Windows UTF-16. A failed edit restores the draft. `finish()`
consumes it even when final validation fails; later calls return `GFX_E_CLOSED`.
Calls reentered from a supplied path object return `GFX_E_BUSY`.
`derive_builder(size, base, options)` copies a validated base catalog into a new
packing area. Its closed options value explicitly chooses whether the semantic
font or icon catalogs will be replaced; unrelated application resources remain.
`AssetPublisher::revise_appearance(base, revision, ...)` accepts complete metrics
and chrome plus exactly one record per fixed role, validates all resource references inside GFX,
and publishes a same-catalog revision. It exposes neither Skin's JSON model nor
GFX's storage.

The Skin compiler converts validated JSON into one `ThemeRecipe`: canonical semantic names,
anchored paths, output dimensions, replacement options, deterministic private image names, and
appearance policy. One executor reads each bounded file immediately before its GFX call, so the
recipe does not retain every encoded font and PNG simultaneously. Runtime loading uses supplied
GFX capabilities. Offline `builder` mode statically constructs those same concrete services and
reads the final immutable bundle into `AssetData`; lightweight `theme-json` deployments do not
link the builder, font rasterizer, or PNG implementation into Skin.

```rust,ignore
let draft = asset_loader.derive_builder(
    Size { width: 1024, height: 512 },
    assets.as_com_ref(),
    AssetDeriveOptions {
        replace_semantic_fonts: false,
        replace_semantic_icons: false,
    },
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

The host creates `ThemeLoader` with a typed `AssetLoader` from GFX.
`ThemeLoader::load_theme(native_path, base)` keeps JSON and appearance processing
inside Skin, while font, PNG, and atlas operations run in GFX. The result is an
ordinary immutable bundle:

```rust,ignore
let publisher = gfx_factory.asset_publisher()?;
let theme_loader = skin_factory.theme_loader(
    asset_loader.as_com_ref(),
    publisher.as_com_ref(),
)?;
let theme = theme_loader.load_theme(
    NativePath::new(Path::new("themes/windows-95/theme.json")).as_interface(),
    assets.as_com_ref(),
)?;
ui.install_assets(theme.as_com_ref(), renderer.as_com_ref())?;
ui.update_with_events(size, owner.as_com_ref())?;
```

Recoverable failures return statuses. `IGfxDiagnostics` retains decoder and
packing detail; `ISkinDiagnostics` retains schema, file, and appearance compiler
detail. Argument validation may supply no additional text.
Diagnostics are immutable snapshots and remain inspectable after a consuming failed
finish. Renderer upload details use `IRenderDiagnostics`.

## Live metric and palette edits

Construct `StyleCompilerAbi` through `skin_factory.style_compiler(asset_publisher)`.
Its `derive_style` method takes an explicit base bundle, complete metrics and a
closed `PaletteEdit` value. The retained GFX capability performs the final generic
metadata copy, validation, and publication; the compiler retains no resource storage.
The publisher reads only metadata and the required white tile, then retains the
base image. Repeated color/inset/metric edits neither copy the full pixel buffer nor
make independent renderer sessions upload their unchanged atlases again.
`Preserve` retains existing appearance tables; `Flat` compiles all role/state
values using the base frame geometry and updates an enabled title backdrop.
The result retains the base catalog and does not install itself. Applications
pass it to `ui.install_assets` through the same transaction as any loaded theme.
Compilation remains usable after a UI closes because the service owns no UI state.

## Compiled resources and packaging

`neocogi_gfx_impl::export::write_rust(&data, path)` (feature `save-to-rust`)
atomically writes one self-contained source
file from validated `AssetData`. The UI crate has no builder, PNG, JSON or source
export features. Cargo build scripts and command-line tools call the same asset
compiler directly. `load_bundle()` creates a fresh catalog; `bundle_data()` supplies the same
tables for `into_revision(base)` when exporting a compatible theme. Both functions
return `Result`; use `bundle_data()?.into_revision(base)`. Generated code requires
GFX support `compiled-assets`, borrows names/glyphs/font/icon records, expands compressed
pixels into one owned allocation, and owns the resolved visual arrays. No generated
file exposes `AtlasHandle`, native IDs, UI implementation storage or parser APIs.

```sh
cargo run -p neocogi-asset-tools --bin neocogi-atlas-export -- --output bundle.rs
cargo neocogi build --assembly full --variant glow
cargo neocogi build --assembly full --variant glow --variant embedded
```

The command prints its managed staging directory under `target/neocogi`. Native
file assets are the default: the assembly contains the runtime, UI, renderer, GFX,
and skin libraries plus adjacent assets. The Skin plugin reads JSON and external
resource files, then delegates font/icon atlas construction to the one GFX plugin;
the host does not embed the photo, mesh or compiled atlases. Register
the plugin catalog before loading a bundle. Resource paths resolve against the
executable, so the complete staged directory can run from another working directory.

`embedded` explicitly enables the demo's `embed-assets` Cargo feature. It compiles
atlases and themes during the build and embeds the full demo's photo and mesh.
The full demo still ships the GFX plugin's PNG decoder for the photograph and the
skin plugin's typed style compiler for live edits. GFX keeps its small
`AssetPublisher` in that deployment while omitting the font rasterizer and atlas
builder. Its compiled themes need no JSON loader; simpler embedded demos need
the compact GFX compositor but neither its builder nor the optional Skin plugin.
Browser `webgl` explicitly enables embedding.
The shared recipe gives both modes identical resource names and font sizes.

The smoke executable's `--rebuild-assets` and `--theme themes/windows-95/theme.json`
options require the file package's construction interfaces. Both can be combined
with `--smoke` from another cwd.

Compiled pixels use zlib, while the COM asset contract still copies ordinary RGBA.
The generated loader validates the exact output length and shared 64 MiB limit,
checksum and stream completion before constructing a bundle. It rejects truncation,
trailing bytes and shorter/longer output. Pixel storage is freed with its last image reference;
there is no process-global decoded cache. Each fresh load or theme candidate performs
its own expansion, so applications should retain their installed bundle rather than
reloading it per frame. The complete publication/upload path remains transactional.

Build-host font/image/theme parsers remain absent from a compiled-only application
target. The optional small inflater has an activation-time and owned-memory cost;
executable bytes, every shared library, external assets, startup and loaded memory
all belong in deployment accounting. The full demo's separate sample PNG is decoded
by its GFX plugin and is not part of the generated pixel-stream format.
