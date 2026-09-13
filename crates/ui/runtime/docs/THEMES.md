# JSON themes

Enable `theme-json` on `neocogi-skin` to compile strict, versioned JSON
into portable resource tables. It is optional; the UI implementation has no JSON
feature or file-loading API. Native applications construct `ThemeLoader` with an
explicit GFX `AssetLoader` and `AssetPublisher`, then pass an explicit original
base bundle. Skin reads and validates the document while GFX alone bakes referenced fonts and PNGs, copies
the base resource bundle, validates references, and publishes the revision. Build
tools call `neocogi_skin_impl::theme::load` or `load_bytes`; the latter reads referenced
assets through an explicit `AssetSource` implementation. Their offline `LoadedTheme`
contains a display name and validated `AssetData`, with no native UI skin or resource IDs.

Loading prepares an immutable candidate. Through a mutable `Ui` reference,
`Ui::install_assets` validates its catalog and resources, asks the renderer to upload
its pixels, then publishes the complete revision. Loading alone never changes a UI,
so applications can prepare several choices before selection.

All PNG paths are relative to the JSON file. Repeated state entries that resolve to the same path
share one decoded atlas region; their source insets, destination insets, and tints remain
independent typed patch metadata. Theme nine-patches therefore join text, icons, and flat fills in
the ordinary atlas vertex batch. External textures remain reserved for application-owned images.
A failed load leaves the active renderer and its external textures untouched.

## Bundled example themes

The repository includes [`themes/windows-3.11/theme.json`](../../../themes/windows-3.11/theme.json),
[`themes/windows-95/theme.json`](../../../themes/windows-95/theme.json), and
[`themes/mac-os-9/theme.json`](../../../themes/mac-os-9/theme.json). All use original BSD-licensed pixel
artwork authored for this project; their directory READMEs identify the GTK, platform-guideline,
and gallery references used for visual research and explicitly document that no third-party theme
files were copied.

The full demo loads the Default Skin and every bundled file once at startup. Choose them from
`View > Theme`; each selection installs a pristine editable copy, so the existing Skin Editor can
modify it without changing the stored base theme. Native file packages enable the
skin plugin's `theme-json` feature and stage the theme directories beside the
executable. Embedded and browser packages compile
those same themes on Cargo's host and ship resolved tables.

## Minimal schema

```json
{
  "schema_version": 1,
  "name": "Example",
  "fonts": {
    "texture_width": 512,
    "texture_height": 256,
    "body": { "path": "body.ttf", "size": 12 },
    "small": { "path": "body.ttf", "size": 10 },
    "title": { "path": "title.ttf", "size": 12 },
    "heading": { "path": "body.ttf", "size": 18 },
    "mono": { "path": "mono.ttf", "size": 14 }
  },
  "icons": {
    "close": { "path": "icon-close.png" },
    "expand": { "path": "icon-expand.png" },
    "collapse": { "path": "icon-collapse.png" },
    "check": { "path": "icon-check.png" },
    "expand_down": { "path": "icon-expand-down.png" },
    "open_folder": { "path": "icon-open-folder.png" },
    "closed_folder": { "path": "icon-closed-folder.png" },
    "file": { "path": "icon-file.png" }
  },
  "skin": {
    "metrics": {
      "padding": 4,
      "window_content_insets": { "left": 0, "top": 0, "right": 0, "bottom": 0 },
      "spacing": 4,
      "title_height": 20
    },
    "fallback_frame_insets": { "left": 1, "top": 1, "right": 1, "bottom": 1 },
    "palette": {
      "text": [0, 0, 0, 255],
      "border": [0, 0, 0, 255],
      "button": [192, 192, 192, 255],
      "button_hovered": [208, 208, 208, 255],
      "control_focus": [0, 0, 0, 255],
      "selection_background": [0, 0, 128, 255],
      "selection_foreground": [255, 255, 255, 255],
      "window_active": [0, 0, 128, 255]
    },
    "window_chrome": {
      "title_alignment": "leading"
    }
  },
  "appearances": {
    "control": {
      "button": {
        "insets": { "left": 2, "top": 2, "right": 2, "bottom": 2 },
        "enabled": {
          "normal": {
            "patch": {
              "type": "image",
              "path": "button-normal.png",
              "source_insets": { "left": 2, "top": 2, "right": 2, "bottom": 2 }
            }
          },
          "hovered": { "patch": { "type": "image", "path": "button-hovered.png" } },
          "pressed": {
            "content_color": [255, 255, 255, 255],
            "patch": { "type": "image", "path": "button-pressed.png" }
          }
        },
        "focused": {
          "normal": { "patch": { "type": "image", "path": "button-focused.png" } },
          "hovered": { "patch": { "type": "image", "path": "button-hovered-focused.png" } },
          "pressed": { "patch": { "type": "image", "path": "button-pressed-focused.png" } }
        },
        "disabled": {
          "patch": {
            "type": "image",
            "path": "button-disabled.png",
            "tint": [255, 255, 255, 160]
          }
        }
      }
    }
  }
}
```

The optional `fonts` object is all-or-nothing. When present, it declares atlas texture dimensions
and exact file/size recipes for the five semantic roles: `body`, `small`, `title`, `heading`, and
`mono`. The optional `icons` object is likewise all-or-nothing and supplies the eight closed
`IconRole` recipes: `close`, `expand`, `collapse`, `check`, `expand_down`, `open_folder`,
`closed_folder`, and `file`. All paths are relative to the JSON file. Loading copies application
resources that are not being replaced into a fresh atlas, preserves their catalog
names, then rasterizes fonts and packs semantic icons and unique image sources.
The UI assigns or preserves its local IDs when it imports the resulting revision.
Without a font recipe, an artwork-bearing theme reuses the application resource catalog's atlas
dimensions and repacks its baked glyphs. Without an icon recipe, it preserves every base icon. A
flat palette-only theme retains the source pixels and metadata without decoding or
repacking files. Pass the original application bundle to each independent theme
load so previously selected theme-private artwork is not accumulated. The
bundled classic themes all provide explicit font recipes and dimensions.

Local `FontId` and `IconId` values belong to the UI consumer. Semantic font roles
select a baked variant from the active theme; named resource references preserve
the same local identity when a compatible revision retains their catalog names.
Custom rendering code that caches UVs must refresh them after installing a new
revision. No native ID is transferred between plugins.

An appearance or state may be omitted. Every omitted state keeps its own flat-color fallback; it
does not borrow another state's patch. A state may set `content_color` without a patch to recolor
its text and semantic glyphs over that fallback. Conversely, a patch may omit `content_color` and
retain the fallback color. This makes partial themes predictable and lets a theme use images only
where they add value.

The optional `patch` is a tagged, closed choice. `{ "type": "solid", "color": [...] }` supplies a
flat RGBA patch; `{ "type": "framed", "border": [...], "center": [...] }` supplies a resource-free
three-by-three color patch; and `{ "type": "image", "path": "...", "source_insets": {...},
"tint": [...] }` supplies an atlas-backed three-by-three patch. A framed patch paints `border` into
all eight fixed cells and `center` into the stretchable middle cell using the role's destination
`insets`. `source_insets` and `tint` exist only on the image variant, so the schema cannot represent
an image path without an image patch or attach slicing metadata to a color patch. Image paths are
relative to the JSON file; omitted image tint is opaque white, and omitted source insets reuse the
role's destination `insets`.

`insets` controls destination layout and stretching. `source_insets` divides the PNG and defaults to
the role's destination insets. Source insets must be non-negative and opposing values must fit
inside the PNG. Destination insets may be larger than a runtime control; the renderer reduces
opposing sides proportionally for tiny destinations.

## Skin fields

The optional `skin.metrics` object is the concrete `SkinMetrics` value and accepts:

- `default_cell_width`
- `padding`
- `window_content_insets` (`left`, `top`, `right`, and `bottom` application-body insets)
- `spacing`
- `indent`
- `title_height`
- `window_border` (`left`, `top`, `right`, and `bottom` structural edge thicknesses)
- `scrollbar_size`
- `thumb_size`

The optional `skin.fallback_frame_insets` value supplies `left`, `top`, `right`, and `bottom`
destination geometry while constructing flat fallback patches. The optional `skin.palette` object
is the concrete `FlatPalette` and accepts RGBA byte arrays under these keys:

- `text`, `border`, `window_background`, `title_background`, `title_foreground`
- `disabled_foreground`, `disabled_background`, `disabled_title_foreground`
- `panel_background`, `button`, `button_hovered`, `input`, `input_hovered`
- `scrollbar_track`, `scrollbar_thumb`, `control_focus`
- `selection_background`, `selection_foreground`, `window_active`
- `menu_foreground`, `menu_background`

These colors construct Skin's complete family catalogs before any per-state patch or `content_color`
override is installed. Each family role/state cell is one concrete `Visual { patch, content_color }`;
patches and content colors cannot drift through parallel catalogs, and no erased or string-keyed
payload participates at runtime.

The optional `skin.window_chrome` object is an asset-document recipe compiled directly into the
IDL-generated `AssetChrome` transfer record. It accepts
`title_alignment` (`leading` or `centered`), `minimized_content_height`, a `captions` object containing `close_side`,
`minimize_side`, `maximize_side`, `vertical_spacing`, `minimum_extent`, `inner_spacing`, and
`show_without_activation`, and an optional `active_title_backdrop` containing `color` and
`horizontal_padding`. Omitted fields produce the same values as the runtime's conventional
`WindowChromeSkin` default without sharing that runtime-only Rust model with the skin plugin.
`vertical_spacing` is the exact title-field clearance applied equally above and below every square
caption button.

Each theme is one self-contained document. Schema version 1 has no parent links, overlays, or merge
rules; an `extends` field is rejected like any other unknown field. Optional values select the
built-in flat or atlas-derived fallback described above rather than consulting another document.

The loader rejects unknown document fields, unknown skin/palette fields, unknown appearance
categories or role keys, unknown state fields, unsupported schema versions, and invalid names
before constructing an atlas. Family and role keys deserialize directly into concrete enums; no
string-keyed appearance map exists between syntax parsing and the typed family documents.

## Loading and selecting

Runtime theme construction is provided by the optional skin plugin. Its typed loader
retains the GFX `AssetLoader` explicitly supplied by the application, reads a
native-path document, and produces a complete immutable `IAssetBundle`. Skin owns
schema and appearance policy; GFX owns the transactional atlas draft. The app
installs that bundle through `Ui::install_assets`; CPU validation and GPU upload must
succeed before the resource revision is published. Keep the original bundle interface
when a selector needs to restore the initial theme.

For programmatic changes, copy the current AssetMetrics, edit explicit fields, derive a
revision with the skin plugin's `StyleCompiler::derive_style`, then install it. Stable named font/icon identities belong
to the bundle catalog; a derived revision cannot reinterpret existing resource meanings.
See [asset authoring and export](../../../../docs/COM_ASSETS.md) and the full demo style editor.
The palette editor is enabled only for the flat theme, while image-backed themes retain
metric editing; multicolor image pixels are not inferred to be named palette fields.

## Appearance roles

The `appearances` object accepts four exact family objects, each with its own closed role-key
vocabulary:

- `surface`: `window`, `generic_frame`, `panel`
- `control`: `button`, `checkbox`, `text_input`, `item`, `combo`, `slider_track`,
  `slider_thumb`, `scrollbar_track`, `scrollbar_thumb`, `close_button`, `minimize_button`,
  `maximize_button`, `restore_button`, `resize_grip`
- `menu`: `bar`, `title`, `popup`, `item`
- `chrome`: `window_frame`, `dialog_frame`, `title`

Category qualification makes local names such as `control.item` and `menu.item` unambiguous while
keeping widget implementation types out of the loader. Unknown categories, role names, and fields
are errors, so a misspelled state or control name cannot silently fall back to a flat appearance.

Each family accepts only states meaningful to it:

- surface roles contain `normal` and `disabled`;
- control roles contain `disabled`, plus `enabled` and `focused` objects whose nested fields are
  `normal`, `hovered`, and `pressed`;
- menu roles contain `normal`, `hovered`, `pressed`, `focused`, `open`, and `disabled`;
- chrome roles contain `base`, `active`, and `disabled`.

Nesting pointer state under the two available control branches makes disabled-hovered and
disabled-pressed combinations unrepresentable. Window activation and menu ownership stay in their
own domains rather than being encoded as duplicate role names.

`surface.window`, `surface.panel`, and container-owned `surface.generic_frame` are noninteractive
structure: their body and border resolve the normal appearance while the pointer moves across
them. Ordinary windows and modal dialogs intentionally share `surface.window`; they retain
independent `chrome.window_frame` and `chrome.dialog_frame` decorations. Losing
top-level activation selects the `base` state of `chrome.window_frame`, `chrome.dialog_frame`, and
`chrome.title` but does not rewrite enabled child widgets. A disabled widget or subtree
resolves `disabled` independently of activation; its content color uses `disabled_foreground` (or
`disabled_title_foreground` for chrome), and omitted patch states use
`disabled_background` as their flat fallback. The window surface remains in its normal state when
activation changes and selects disabled only when the complete root is disabled. Interactive
descendants, resize borders, caption controls, and the title remain free to resolve their own hover
and pressed states while enabled.

`WindowOption::DISABLED` is the explicit whole-window counterpart. It keeps the root visible and
keeps its retained update traversal running, but resolves the disabled frame/title states, intrinsic
menu bar, caption controls, and complete widget tree through `disabled`. It also rejects pointer,
keyboard, popup, move, resize, and caption input while retaining focus for later re-enabling. A
structural child window inherits a disabled structural parent; an owned modal dialog remains an
independent root so it can stay enabled above a deliberately disabled owner. This policy is
separate from ordinary activation: selecting another window never disables the former window's
children.

## Window borders and caption controls

`skin.window_border` is the window's structural border thickness. Its four values drive client
layout and the right/bottom one-axis resize hit regions. `chrome.window_frame.insets` and
`chrome.dialog_frame.insets` instead control the fixed visual corner span for their respective
three-by-three artwork; each active state is normalized to its corresponding base state's visual
authority. Keeping the values separate permits a four-pixel Windows 3.11 edge to carry a 23-pixel
L-shaped ordinary-window corner while a modal dialog uses a uniform four-pixel outline, without
reserving 23 pixels around either client. The bottom-right two-axis region remains larger for easy
input, but themes may leave `control.resize_grip` transparent when the frame corner itself is the
complete visible affordance. Frame patches contribute only their eight outer cells; their center is
never a second background source. `surface.window` fills the complete root below that decoration,
including transparent frame pixels and any space exposed by `window_content_insets`.

`skin.window_content_insets` is a separate four-edge inset around the application body. Root
geometry applies it after the frame, title, and menu bar have been allocated, so it never narrows
the menubar and never changes ordinary widget padding. The Windows 3.11 theme uses a two-pixel
margin; the other bundled classic themes use zero, and the default flat Skin retains five pixels.
`WindowOption::NO_PADDING` overrides the metric with zero for an individual root.

Application-authored popup windows use the `menu.popup` appearance for both their structural
client inset and their outer frame paint. This keeps combo/list popups aligned with themed menu
panels instead of borrowing ordinary window L-corners or `skin.window_border`; compact menu
popups already paint the same role as their complete manager-owned panel.

Window caption controls are enabled explicitly through `WindowOption::MINIMIZE_BUTTON` and
`WindowOption::MAXIMIZE_BUTTON`. The close button remains enabled unless `WindowOption::NO_CLOSE`
is present. Caption and resize controls receive nested enabled/hovered and enabled/pressed states
from manager-owned pointer capture just like widgets. A press dragged away from its originating
caption button is no longer painted pressed and does not activate on release.

The manager draws each caption control's close, minimize, maximize, or restore symbol using that
control state's `content_color`. Image-backed caption faces that already contain their complete symbol
set `content_color` alpha to zero. There are no separate caption-glyph roles or window-chrome switch,
so the same role/state visual fully describes each control without special handling in the loader.

`skin.window_chrome` supplies exact platform geometry without a loader-owned preset enum. The
skin plugin emits only `AssetChrome`; the UI runtime validates that record and constructs its
local `WindowChromeSkin`. A
conventional theme may omit it and receive leading text with trailing caption buttons. The bundled
Mac theme directly selects centered text, a compact leading close box, trailing
zoom/windowshade controls, and an active-title backdrop. Its caption PNGs are complete faces, so
these states use a transparent content color and do not show an additional manager-owned symbol. The
manager consumes only its concrete `WindowChromeSkin` fields and never branches on a theme or
platform mode.

Minimize keeps the retained window visible, saves its expanded height, reduces its application body
to `minimized_content_height`, and emits `WindowEvent::Minimized`. Activating minimize again restores
the saved height and emits `WindowEvent::Restored`. Maximize saves the exact normal outer rectangle,
tracks the complete inherited viewport, and emits `WindowEvent::Maximized`. Activating the same
caption position while maximized uses `control.restore_button`, restores the saved rectangle, and
emits `WindowEvent::Restored`.

## Compatibility with retained application resources

`Ui::install_assets` authenticates the candidate's catalog identity and verifies
that every original application font and icon is still present before requesting
GPU work. An unrelated bundle with identical names is rejected with `UI_E_OWNER`;
a missing required resource is rejected with `NC_E_INVALID_ARG`. Failed validation
or upload preserves the previous UI resources and GPU image.

`AssetLoader::derive_builder`, `AssetPublisher::revise_appearance`, and
`ThemeLoader::load_theme` publish in the base bundle's catalog.
`StyleCompiler::derive_style` changes metrics or compiles a flat palette in that
same namespace through its explicit GFX publisher. Build tools publish `AssetData::into_revision(base)`
when producing a compatible replacement. These services do not install their output.

The UI preserves typed local IDs for original application resources. Theme-private
artwork may disappear when another theme is selected; application references or
cached UVs for that artwork must change with its owning revision. Installation
protects the original catalog and does not inspect arbitrary application fields
to discover private dependencies. See [COM assets](../../../../docs/COM_ASSETS.md) for
the complete validation and publication contract.
