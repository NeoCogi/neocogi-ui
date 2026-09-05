# Fonts and typography

- Atlas building supports multiple baked fonts and sizes through `atlas::builder::FontAsset`, and the same config can drive both runtime atlas construction and offline/prebuilt atlas export.
- A compiled UI bundle resolves conventional `body`, `small`, `title`, `heading` and `mono`
  font roles and the standard semantic icon roles. The assets plugin creates the bundle;
  `Ui::install_assets` installs it transactionally with the renderer.
- Text-bearing COM widgets expose typed font selection through the facade. Resolve named
  fonts against the bundle catalog; use semantic roles when the selected theme should
  determine the exact baked resource.
- Font sizes are selected by choosing another baked font variant, not by scaling one bitmap font at runtime.
- `demos/demo-full` uses this directly: `NORMAL.ttf` for control/body text, `BOLD.ttf` for window titles, and `CONSOLE.ttf` for the log window’s input/output text.

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

The built-in atlas builder bakes only printable ASCII (`U+0020` through `U+007E`), which includes
underscore. `AtlasSource` can describe arbitrary Unicode scalar values, so applications needing
broader coverage must provide their own glyph table and must include `_` in every font. Atlas
construction rejects a font without that explicit fallback. There is not yet a configurable
glyph-range option in `builder::Config`.

The text pipeline does not perform grapheme segmentation, script shaping, bidirectional
reordering, kerning, or fallback-font selection. `TextWrap::Word` wraps only at ASCII space
boundaries; an individual word is not split when it exceeds the available width.

A compiled UI atlas must have a `body` font and the standard semantic icon names
`close`, `expand`, `collapse`, `check`, `expand_down`, `open_folder`, `closed_folder`, and `file`.
Every atlas, including a fontless atlas prepared before UI construction, has a validated opaque
white tile named `white` for solid geometry; no resource depends on a numeric table position.
`AtlasHandle::try_from` validates dimensions, decoded pixels, unique names and glyphs, font
metrics, the mandatory underscore entries, every glyph/icon rectangle, and the opaque white tile
before returning a handle. `Skin::from_atlas` and `SkinBundle::new` enforce the standard UI's
semantic font/icon naming policy and validate that image visuals belong to the paired atlas.

`FontId` and `IconId` identify logical resources, independently of atlas allocation or table position.
Cloning an `AtlasHandle` retains those IDs. A builder derived from an existing atlas preserves IDs
when copying or replacing its named resources; independently loading even identical serialized
metadata allocates different IDs. A display list is valid only if the installed atlas contains its
exact IDs, and preflight rejects absent IDs before issuing draws.

Retained `FontRef` and `IconRef` values choose semantic roles or explicit catalog IDs. Semantic
roles resolve through the active skin; named catalog references keep the checked logical ID instead
of repeating a name lookup at every measurement. Asset installation rejects bundles missing any
application catalog font or icon before asking the backend to upload. Applications retaining
references to theme-private artwork must replace those references with the theme that owns them.
See the [theme replacement contract](THEMES.md) for that author responsibility.

```rust
# #[cfg(feature = "builder")]
# {
use microui_ui::{atlas::builder, prelude::*};

let icons = vec![
    builder::IconAsset { name: "close".into(), path: "assets/CLOSE.png".into() },
    builder::IconAsset { name: "expand".into(), path: "assets/PLUS.png".into() },
    builder::IconAsset { name: "collapse".into(), path: "assets/MINUS.png".into() },
    builder::IconAsset { name: "check".into(), path: "assets/CHECK.png".into() },
    builder::IconAsset { name: "expand_down".into(), path: "assets/EXPAND_DOWN.png".into() },
    builder::IconAsset { name: "open_folder".into(), path: "assets/OPEN_FOLDER_16.png".into() },
    builder::IconAsset { name: "closed_folder".into(), path: "assets/CLOSED_FOLDER_16.png".into() },
    builder::IconAsset { name: "file".into(), path: "assets/FILE_16.png".into() },
];

let fonts = vec![
    builder::FontAsset {
        name: "body".into(),
        path: "assets/NORMAL.ttf".into(),
        size: 12,
    },
    builder::FontAsset {
        name: "small".into(),
        path: "assets/NORMAL.ttf".into(),
        size: 10,
    },
    builder::FontAsset {
        name: "title".into(),
        path: "assets/BOLD.ttf".into(),
        size: 12,
    },
    builder::FontAsset {
        name: "heading".into(),
        path: "assets/NORMAL.ttf".into(),
        size: 18,
    },
    builder::FontAsset {
        name: "mono".into(),
        path: "assets/CONSOLE.ttf".into(),
        size: 14,
    },
    builder::FontAsset {
        name: "calculator-display".into(),
        path: "assets/CONSOLE.ttf".into(),
        size: 28,
    },
];

let config = builder::Config {
    texture_width: 512,
    texture_height: 256,
    white_icon: "assets/WHITE.png".into(),
    icons,
    fonts,
};

# }
```

`builder::Config::fonts` may be empty when an application needs to prepare atlas data separately.
A UI atlas must name one entry `body`; optional roles that are absent fall back to that same
atlas-owned body font.

After installing the compiled bundle, select a font on an actual COM widget:

```rust,no_run
use microui_redux::{TextBlock, TextFontRole, TextWrapping, Ui};
use nc_com::NcStatus;
fn heading(ui: &Ui) -> Result<TextBlock, NcStatus> {
    let title = ui.widgets().text_block("Inspector", TextWrapping::NONE)?;
    title.set_font_role(TextFontRole::HEADING)?;
    Ok(title)
}
```
