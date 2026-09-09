# Fonts and typography

- Atlas building supports multiple baked fonts and sizes through `neocogi_assets::atlas::FontAsset`, and the same config can drive both runtime atlas construction and offline/prebuilt atlas export.
- A compiled UI bundle resolves conventional `body`, `small`, `title`, `heading` and `mono`
  font roles and the standard semantic icon roles. The assets plugin creates the bundle;
  mutable `Ui` access lets `Ui::install_assets` install it transactionally with the renderer.
- Text-bearing COM widgets expose typed font selection through the facade. Resolve named
  fonts against the bundle catalog; use semantic roles when the selected theme should
  determine the exact baked resource.
- Font sizes are selected by choosing another baked font variant, not by scaling one bitmap font at runtime.
- `demos/full` uses this directly: `NORMAL.ttf` for control/body text, `BOLD.ttf` for window titles, and `CONSOLE.ttf` for the log window’s input/output text.

## Text encoding and glyph coverage

Facade text uses Rust str/String values. Direct COM callers supply explicit byte buffers,
which implementations validate as UTF-8 before committing text.
Textbox removes CR and LF so its stored value always remains single-line. TextArea and TextBlock
canonicalize CRLF and lone CR to LF when text is constructed, replaced, or pasted. Textbox and text
area keep their byte cursor on Unicode scalar-value boundaries. Left/right movement and deletion
operate on one scalar value at a time, not on a user-perceived grapheme cluster. Combining
sequences and multi-scalar emoji can therefore require more than one cursor or deletion operation.

File-dialog requests and accepted results use native `PathBuf` values. Directory rows carry those
paths in typed submission events; only their display labels are converted lossily. Selecting a row
preserves its native identity, including non-UTF-8 bytes or line endings that an editable single-line
label cannot represent. Editing a path/filename creates a UTF-8 path instead. Acceptance is lexical;
the application validates existence and file type when required.

Rendering coverage is a separate atlas concern. Text measurement and drawing iterate Unicode
scalar values and use the same lookup rules:

- a character present in the selected atlas font uses its own glyph metrics and rectangle;
- a missing character uses the selected font's mandatory underscore (`_`) entry.

The asset plugin's atlas builder bakes only printable ASCII (`U+0020` through `U+007E`), which includes
underscore. `AssetGlyph` tables can describe arbitrary Unicode scalar values, so applications needing
broader coverage must provide their own glyph table and must include `_` in every font. Atlas
construction rejects a font without that explicit fallback. There is not yet a configurable
glyph-range option in `neocogi_assets::atlas::Config`.

The text pipeline does not perform grapheme segmentation, script shaping, bidirectional
reordering, kerning, or fallback-font selection. `TextWrap::Word` wraps only at ASCII space
boundaries; an individual word is not split when it exceeds the available width.

A compiled UI atlas must have a `body` font and the standard semantic icon names
`close`, `expand`, `collapse`, `check`, `expand_down`, `open_folder`, `closed_folder`, and `file`.
Every installable bundle has a validated opaque white tile named `white` for solid
geometry. `AssetData::read` validates bounded storage, names, font metrics,
underscore fallbacks, rectangles and appearances before the UI creates its local
lookup state. The asset compiler performs the same validation before publication.

`FontId` and `IconId` are local UI resource identities. A compatible COM revision
preserves them after catalog authentication and name matching; independently
created catalogs remain unrelated even with identical names and pixels. Display
lists require the installed atlas's exact local IDs before issuing draws. GPU
backends receive already expanded geometry and raw pixels, without font metadata.

Retained `FontRef` and `IconRef` values choose semantic roles or explicit catalog IDs. Semantic
roles resolve through the active skin; named catalog references keep the checked logical ID instead
of repeating a name lookup at every measurement. Asset installation rejects bundles missing any
application catalog font or icon before asking the backend to upload. Applications retaining
references to theme-private artwork must replace those references with the theme that owns them.
See the [theme replacement contract](THEMES.md) for that author responsibility.

Applications can add a baked variant to an asset-builder draft without linking a
font parser into the host. The draft must also contain the required body font and
semantic icons before `finish` can publish it:

```rust,no_run
use neocogi_ui::{IAssetBuilderAbi, NativePath};
use neocogi_com::{ComRef, NcCopyFromBufferOffset, NcStatus};
use std::path::Path;
fn add_heading(draft: ComRef<'_, IAssetBuilderAbi>, font: &Path) -> Result<(), NcStatus> {
    draft.add_font(
        NcCopyFromBufferOffset::new(0, b"heading"),
        NativePath::new(font).as_interface(),
        18,
    )
}
```

Optional semantic roles that are absent resolve to the same bundle's required
body font. See [asset construction](../../../../docs/COM_ASSETS.md) for complete
recipes and host-side source generation.

After installing the compiled bundle, select a font on an actual COM widget:

```rust,no_run
use neocogi_ui::{TextBlockWidget, TextFontRole, TextWrapping, Ui};
use neocogi_com::NcStatus;
fn heading(ui: &Ui) -> Result<TextBlockWidget, NcStatus> {
    let mut title = ui.widgets().text_block("Inspector", TextWrapping::NONE)?;
    title.set_font_role(TextFontRole::HEADING)?;
    Ok(title)
}
```
