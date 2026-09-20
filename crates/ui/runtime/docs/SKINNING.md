# Skin architecture

Skinning has one canonical resolved runtime model. Built-in widgets, manager-owned window chrome,
JSON themes, and programmatic revisions all use the `AssetTables` snapshot of generated NCIDL records retained by
the active `Skin`. There is no
property bag, `Any` payload, string-keyed runtime lookup, local override cascade, or parallel
patch and semantic-content-color model.

## Runtime ownership

The UI runtime, retained GFX compositor, and renderer install one immutable
`IAssetBundle` together. UI imports its canonical catalog identity and retains the validated
table collection without translating its generated records to native resource records. GFX imports the tables it
needs for final composition, while the renderer
prepares the matching image before publication. `IAssetBundle::image()` has
independent immutable identity, so an appearance-only revision reuses its atlas
without another full pixel copy or upload. Failed validation or upload preserves
the current revision. All windows, widgets and scoped services use that same committed
resource identity.

A producer derives revisions from the original resource catalog, preserving every
application font/icon identity and meaning. Loading a theme from the currently selected
theme would accumulate private artwork across switches, so the skin client retains the
original catalog independently of the installed revision.

## Concrete types

| Type | Responsibility |
| --- | --- |
| `Skin` | Typed state selectors and one revision identity bound to the canonical snapshot. |
| `AssetMetrics` | Generated layout, spacing, window inset, border, title, scrollbar, and thumb geometry. |
| `Skin::surface` / `control` / `menu` / `chrome` | Resolves one complete `Visual { patch, content_color }` in a concrete family. |
| `SurfaceRole` / `ControlRole` | Concrete structural and interactive widget-facing roles. |
| `MenuRole` / `ChromeRole` | Concrete menu and manager-owned window-decoration roles. |
| `SurfaceState` | Structural availability: normal or disabled. |
| `ControlState` / `PointerState` | Disabled, or enabled/focused with a nested normal, hovered, or pressed pointer state. |
| `MenuState` | Menu-local normal, hovered, pressed, focused, open, and disabled states. |
| `ChromeState` | Window-local base, active, and disabled states. |
| `AssetChrome` | Generated title alignment, caption placement, sizing, and backdrop policy. |
| `FontRef` / `IconRef` | Stable semantic or named references resolved against the active bundle. |
| `SkinBundle` | Atomic publication of an intrinsically atlas-bound skin snapshot. |

IDL role enums expose generated `ALL` and `COUNT` metadata because they are the only dense table
axes. Native state enums expose `ALL` for complete compilation, while generated family records
retain named fields—including nested enabled/focused pointer branches for controls. Exhaustive
matches select those fields directly. Code therefore cannot pass menu-open state to a widget or
pointer state to a window frame, and no payload state needs a numeric index. Paint projects one
short-lived `Visual`, keeping a state's artwork and content color adjacent without retaining an
erased value or a second universal role/state catalog.

## Programmatic construction

Ask the asset compiler for an immutable candidate in the UI's current catalog, then install it through
the same transactional asset boundary used for a complete theme replacement:

```rust,no_run
use neocogi_ui::{FlatPalette, IRendererAbi, Ui};
use neocogi_skin_contracts::{StyleCompilerAbi, PaletteEdit};
use neocogi_com::{ComRef, NcStatus};
fn customize(ui: &mut Ui, renderer: ComRef<'_, IRendererAbi>, compiler: ComRef<'_, StyleCompilerAbi>) -> Result<(), NcStatus> {
    let mut metrics = ui.assets()?.metrics()?;
    metrics.padding = 8;
    let current = ui.assets()?;
    let candidate = compiler.derive_style(current.as_com_ref(), metrics, PaletteEdit::Flat(FlatPalette::default()).into())?;
    ui.install_assets(candidate.as_com_ref(), renderer)
}
```

`PaletteEdit::Flat` regenerates the complete flat visual catalog. Use
`PaletteEdit::Preserve` to change metrics while retaining image-backed appearance
families. Derivation does not install the candidate. An installation error preserves
the old CPU and GPU resources. Successful installation requires a subsequent UI
update before rendering. The full demo's style editor shows concrete handlers for
all palette roles and metrics.

## Stable resources

ABI resource identifiers belong to an authenticated bundle catalog. The static facade
uses named/semantic font and icon selections; the implementation resolves these to its
private FontRef and IconRef values:

- a semantic reference contains a closed `FontRole` or `IconRole`;
- a named reference contains a checked application resource name created through
  the validated asset bundle catalog.

Measurement and paint resolve those references against the current bundle. Named application fonts
and icons are copied into every rebuilt JSON-theme atlas, while a theme may replace the complete
five-role semantic font catalog and/or complete eight-role semantic icon catalog. Replacement
preserves each resource's stable ID, so already-retained exact references remain valid in the
derived atlas. Custom renderer code that caches atlas rectangles or UVs must still refresh those
allocation-bound values after a bundle switch.

## Global skin and caches

The UI runtime owns the sole active resource revision. Window chrome and every retained
node use that complete value. Widgets and containers do not store local skin overrides.
`Ui::install_assets` takes mutable `Ui` access and replaces the revision for the entire UI
through one transaction.

Each validated skin receives one private, non-reused revision. Retained measurement caches key
their results by constraints and that complete revision, so a global replacement cannot reuse
measurements produced with an earlier skin whose scalar fields happened to look similar.

## JSON compilation

The optional `theme-json` feature adds a strict authoring format documented in
[JSON themes](THEMES.md). Loading has three boundaries:

1. Decode one strict, self-contained document and resolve its asset paths relative to that file.
2. Deserialize family-local appearance keys directly into concrete role enums and reject unknown
   keys or invalid schema data before atlas work.
3. Build one immutable `AssetData` value, compile flat fallbacks and authored state images into its
   generated tables, then publish one validated bundle.

The runtime never retains the source document, palette, or string appearance keys. Those are
compiler inputs only.

## Live authoring tool

The [theme editor](../../../../tools/theme-editor/README.md) keeps its normal UI
and edits a `ThemeDocument` for a separate preview runtime/private compositor.
Typed commands address concrete fields, roles, and valid states. Resource changes
compile from one stable application base; geometry and color edits reuse the
resource build. A failed candidate preserves the last valid preview. The gallery
retains widget values and arrangement across successful installations.
