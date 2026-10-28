# Fonts and typography

- Atlas building supports multiple baked fonts and sizes through `neocogi_gfx_impl::atlas::FontAsset`, and the same config can drive both runtime atlas construction and offline/prebuilt atlas export.
- A compiled UI bundle resolves conventional `body`, `small`, `title`, `heading` and `mono`
  font roles and the standard semantic icon roles. The GFX plugin creates the bundle;
  mutable `Ui` access lets `Ui::install_assets` install it transactionally with the renderer.
- Text-bearing COM widgets expose typed font selection through the facade. Resolve named
  fonts against the bundle catalog; use semantic roles when the selected theme should
  determine the exact baked resource.
- Font sizes are selected by choosing another baked font variant, not by scaling one bitmap font at runtime.
- `examples/full` uses this directly: `NORMAL.ttf` for control/body text, `BOLD.ttf` for window titles, and `CONSOLE.ttf` for the log window’s input/output text.

## Text encoding and glyph coverage

Facade text uses Rust str/String values. Direct COM callers supply explicit byte buffers,
which implementations validate as UTF-8 before committing text.
Textbox removes CR and LF so its stored value always remains single-line. TextArea and TextBlock
canonicalize CRLF and lone CR to LF when text is constructed, replaced, or pasted. Textbox and text
area keep their byte cursor on Unicode scalar-value boundaries. Left/right movement and deletion
operate on one scalar value at a time, not on a user-perceived grapheme cluster. Combining
sequences and multi-scalar emoji can therefore require more than one cursor or deletion operation.

Textboxes, text areas, and numeric inline editors share ordinary selection and
local edit history. Shift+arrows and Shift+Home/End extend selection; multiline
Up/Down preserves the preferred visual column. Pointer dragging and Shift-click
also extend selection. Ctrl/Command+A selects all; typing or deletion replaces
the selected range. Plain Left/Right collapses to that selection edge. Selection
uses the focused-item theme colors and includes visible newline cells.

Ctrl/Command+Z undoes, Shift+Ctrl/Command+Z or Ctrl/Command+Y redoes. Each text
mutation records one snapshot; history retains at most 64 snapshots and 1 MiB
of prior text across both directions. Oversized edits close the retained timeline.
Programmatic text replacement resets selection/history; cursor setters collapse
selection. Long single-line text scrolls to keep the active caret visible.
Ctrl/Command+C, X, and V copy, cut, and paste through the host-installed
`ITextClipboard` service. The standard host installs the window provider: SDL on
native targets and the asynchronous Clipboard API in browsers. Other hosts may
install their own provider through `Ui::set_clipboard`; without one, these
shortcuts leave the editor unchanged.

Each editor retains at most one pending request and one receipt, bounded to
1 MiB of UTF-8 text. Cut deletes only after a successful write acknowledgement.
Editing, moving the selection, losing focus, or closing the widget cancels the
pending receipt, so delayed paste/cut results cannot change a different edit.
Accepted cut/paste mutations participate in ordinary undo history and newline
normalization. Clipboard denial or an unavailable browser capability preserves
both text and the current UI frame. Browser access depends on the browser's
secure-context, permission, and user-activation rules; keyboard shortcuts suppress
the hidden DOM editor's duplicate handling.

File-dialog requests and accepted results use native `PathBuf` values. Directory rows carry those
paths in typed submission events; only their display labels are converted lossily. Selecting a row
preserves its native identity, including non-UTF-8 bytes or line endings that an editable single-line
label cannot represent. Editing a path/filename creates a UTF-8 path instead. Acceptance is lexical;
the application validates existence and file type when required.

Rendering coverage is a separate atlas concern. Text measurement and drawing iterate Unicode
scalar values and use the same lookup rules:

- a character present in the selected atlas font uses its own glyph metrics and rectangle;
- a missing character uses the selected font's mandatory underscore (`_`) entry.

The GFX plugin's atlas builder bakes only printable ASCII (`U+0020` through `U+007E`), which includes
underscore. `AssetGlyph` tables can describe arbitrary Unicode scalar values, so applications needing
broader coverage must provide their own glyph table and must include `_` in every font. Atlas
construction rejects a font without that explicit fallback. There is not yet a configurable
glyph-range option in `neocogi_gfx_impl::atlas::Config`.

The text pipeline does not perform grapheme segmentation, script shaping, bidirectional
reordering, kerning, or fallback-font selection. `TextWrap::Word` wraps only at ASCII space
boundaries; an individual word is not split when it exceeds the available width.
`TextWrapping::WordOrCharacter` (internally `TextWrap::WordOrCharacter`) also
prefers ASCII spaces, but splits oversized words at UTF-8 character boundaries.
Use it for constrained messages containing long paths or identifiers. Measurement,
paint, and editable caret ranges share the same breaks, and the source text stays
unchanged. A single glyph wider than the line remains indivisible.

Wrapping and atlas measurement distinguish signed pen movement from the glyph's
right extent. An overhanging token does not add its ink width as interword spacing.
For example, glyphs with a two-pixel right offset and four-pixel advance measure
`"a a"` as 14 pixels; both word policies retain one row at a 14-pixel constraint.
Each emitted row reports the atlas width of its exact source slice. Fit checks
retain wide coordinates before saturation, so extreme advances still wrap and
later signed advances can cancel correctly. Layout scans new tokens/scalars a
bounded number of times rather than remeasuring growing prefixes.

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
body font. See [asset construction](../../../../docs/contracts/COM_ASSETS.md) for complete
recipes and host-side source generation.

After installing the compiled bundle, select a font on an actual COM widget:

```rust,no_run
use neocogi_ui::{FontRole, OwnedWidget, TextBlockAbi, TextWrapping, Ui};
use neocogi_com::NcStatus;
fn heading(ui: &Ui) -> Result<OwnedWidget<TextBlockAbi>, NcStatus> {
    let mut title = ui.widgets().text_block("Inspector", TextWrapping::None)?;
    title.set_font_role(FontRole::Heading)?;
    Ok(title)
}
```
