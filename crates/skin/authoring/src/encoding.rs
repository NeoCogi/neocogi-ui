//! Concrete JSON output adapters for generated GFX records.
//!
//! These borrowed Serde adapters define file syntax only. Authoring stores the
//! generated types themselves; no reflection, erased values, or second runtime
//! representation is introduced into the contracts.

use std::collections::BTreeMap;
use neocogi_gfx_contracts::{AssetMetrics, Color, FlatPalette, Insets};
use serde::{Serialize, Serializer};
use serde::ser::{SerializeMap, Error as _};
use crate::serde_com::DocumentKey;

/// Encodes one color in the existing four-channel JSON array notation.
pub(super) fn color<S: Serializer>(value: &Color, serializer: S) -> Result<S::Ok, S::Error> {
    [value.r, value.g, value.b, value.a].serialize(serializer)
}

/// Preserves an absent content/tint override instead of manufacturing a default.
pub(super) fn optional_color<S: Serializer>(value: &Option<Color>, serializer: S) -> Result<S::Ok, S::Error> {
    value.map(|value| [value.r, value.g, value.b, value.a]).serialize(serializer)
}

/// Encodes all nine cells in the same row-major order used by the inspector.
pub(super) fn cells<S: Serializer>(value: &[Color; 9], serializer: S) -> Result<S::Ok, S::Error> {
    value.map(|value| [value.r, value.g, value.b, value.a]).serialize(serializer)
}

/// Encodes the four named source/destination inset components.
pub(super) fn insets<S: Serializer>(value: &Insets, serializer: S) -> Result<S::Ok, S::Error> {
    InsetsJson::serialize(value, serializer)
}

/// Preserves inheritance when no role or state geometry has been authored.
pub(super) fn optional_insets<S: Serializer>(value: &Option<Insets>, serializer: S) -> Result<S::Ok, S::Error> {
    value.map(InsetValue).serialize(serializer)
}

/// One local serialization wrapper used only by the optional-insets adapter.
#[derive(Serialize)]
#[serde(transparent)]
struct InsetValue(
    /// Concrete geometry encoded without changing the generated GFX type.
    #[serde(serialize_with = "insets")]
    Insets,
);

/// Serializes typed role keys through their generated canonical spellings.
pub(super) fn role_map<S, K, V>(value: &BTreeMap<K, V>, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
    K: DocumentKey,
    V: Serialize,
{
    let mut output = serializer.serialize_map(Some(value.len()))?;
    for (key, entry) in value {
        // Raw ABI enum values outside the declared domain cannot become file keys.
        let name = key.name().ok_or_else(|| S::Error::custom("unknown appearance role"))?;
        output.serialize_entry(name, entry)?;
    }
    output.end()
}

/// Writes concrete layout metrics with nested named inset records.
pub(super) fn metrics<S: Serializer>(value: &AssetMetrics, serializer: S) -> Result<S::Ok, S::Error> {
    MetricsJson::serialize(value, serializer)
}

/// Writes every palette field, including disabled foreground and background colors.
pub(super) fn palette<S: Serializer>(value: &FlatPalette, serializer: S) -> Result<S::Ok, S::Error> {
    PaletteJson::serialize(value, serializer)
}

/// File-format adapter borrowing the generated `Insets` fields during serialization.
#[derive(Serialize)]
#[serde(remote = "Insets")]
struct InsetsJson {
    /// Leading horizontal thickness.
    left: i32,
    /// Leading vertical thickness.
    top: i32,
    /// Trailing horizontal thickness.
    right: i32,
    /// Trailing vertical thickness.
    bottom: i32,
}

/// File-format adapter borrowing the generated `AssetMetrics` fields during serialization.
#[derive(Serialize)]
#[serde(remote = "AssetMetrics")]
struct MetricsJson {
    /// Preferred width when a layout supplies none.
    default_cell_width: i32,
    /// Inner control padding.
    padding: i32,
    /// Window body padding independent of controls.
    #[serde(serialize_with = "insets")]
    window_content_insets: Insets,
    /// Distance between neighboring cells.
    spacing: i32,
    /// Nested content indentation.
    indent: i32,
    /// Window title height.
    title_height: i32,
    /// Structural top-level border thickness.
    #[serde(serialize_with = "insets")]
    window_border: Insets,
    /// Scrollbar transverse size.
    scrollbar_size: i32,
    /// Minimum scrollbar and slider thumb size.
    thumb_size: i32,
}

/// File-format adapter borrowing the generated `FlatPalette` fields during serialization.
#[derive(Serialize)]
#[serde(remote = "FlatPalette")]
struct PaletteJson {
    /// Ordinary control text and semantic glyph color.
    #[serde(serialize_with = "color")]
    text: Color,
    /// Generic flat frame and control border color.
    #[serde(serialize_with = "color")]
    border: Color,
    /// Window and dialog client-area background.
    #[serde(serialize_with = "color")]
    window_background: Color,
    /// Base window title background used when the active chrome role is not selected.
    #[serde(serialize_with = "color")]
    title_background: Color,
    /// Window title and caption foreground.
    #[serde(serialize_with = "color")]
    title_foreground: Color,
    /// Panel and scroll-viewport background.
    #[serde(serialize_with = "color")]
    panel_background: Color,
    /// Ordinary raised-control fill.
    #[serde(serialize_with = "color")]
    button: Color,
    /// Hovered raised-control fill.
    #[serde(serialize_with = "color")]
    button_hovered: Color,
    /// Ordinary recessed-control fill.
    #[serde(serialize_with = "color")]
    input: Color,
    /// Hovered recessed-control fill.
    #[serde(serialize_with = "color")]
    input_hovered: Color,
    /// Scrollbar track fill.
    #[serde(serialize_with = "color")]
    scrollbar_track: Color,
    /// Scrollbar thumb fill.
    #[serde(serialize_with = "color")]
    scrollbar_thumb: Color,
    /// Focused control border or fill accent.
    #[serde(serialize_with = "color")]
    control_focus: Color,
    /// Background used by hovered, keyboard-focused, open, or otherwise selected items.
    #[serde(serialize_with = "color")]
    selection_background: Color,
    /// Text and glyph color paired with [`Self::selection_background`].
    #[serde(serialize_with = "color")]
    selection_foreground: Color,
    /// Active window frame and title accent.
    #[serde(serialize_with = "color")]
    window_active: Color,
    /// Menu text and marker foreground.
    #[serde(serialize_with = "color")]
    menu_foreground: Color,
    /// Menu bar and popup background.
    #[serde(serialize_with = "color")]
    menu_background: Color,
    /// Disabled control and client-area background.
    #[serde(serialize_with = "color")]
    disabled_background: Color,
    /// Disabled body, menu, and control foreground.
    #[serde(serialize_with = "color")]
    disabled_foreground: Color,
    /// Disabled title and caption foreground.
    #[serde(serialize_with = "color")]
    disabled_title_foreground: Color,
}
