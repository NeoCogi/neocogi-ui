//! Concrete JSON adapters for generated GFX records.
//!
//! These concrete Serde adapters define file syntax only. Authoring stores the
//! generated types themselves; no reflection, erased values, or second runtime
//! representation is introduced into the contracts.

use std::collections::BTreeMap;
use neocogi_gfx_contracts::{AssetMetrics, Color, FlatPalette, Insets};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde::ser::{SerializeMap, Error as _};
use crate::role_keys::DocumentKey;

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
#[derive(Serialize, Deserialize)]
#[serde(transparent)]
struct InsetValue(
    /// Concrete geometry encoded without changing the generated GFX type.
    #[serde(serialize_with = "insets", deserialize_with = "read_insets")]
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

/// Concrete file-format adapter for the generated `Insets` record.
#[derive(Serialize, Deserialize)]
#[serde(remote = "Insets", deny_unknown_fields)]
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

/// Concrete file-format adapter preserving generated metric defaults.
#[derive(Serialize, Deserialize)]
#[serde(remote = "AssetMetrics", default = "AssetMetrics::default", deny_unknown_fields)]
struct MetricsJson {
    /// Preferred width when a layout supplies none.
    default_cell_width: i32,
    /// Inner control padding.
    padding: i32,
    /// Window body padding independent of controls.
    #[serde(serialize_with = "insets", deserialize_with = "read_insets")]
    window_content_insets: Insets,
    /// Distance between neighboring cells.
    spacing: i32,
    /// Nested content indentation.
    indent: i32,
    /// Window title height.
    title_height: i32,
    /// Structural top-level border thickness.
    #[serde(serialize_with = "insets", deserialize_with = "read_insets")]
    window_border: Insets,
    /// Scrollbar transverse size.
    scrollbar_size: i32,
    /// Minimum scrollbar and slider thumb size.
    thumb_size: i32,
}

/// Concrete file-format adapter preserving generated palette defaults.
#[derive(Serialize, Deserialize)]
#[serde(remote = "FlatPalette", default = "FlatPalette::default", deny_unknown_fields)]
struct PaletteJson {
    /// Ordinary control text and semantic glyph color.
    #[serde(serialize_with = "color", deserialize_with = "read_color")]
    text: Color,
    /// Generic flat frame and control border color.
    #[serde(serialize_with = "color", deserialize_with = "read_color")]
    border: Color,
    /// Window and dialog client-area background.
    #[serde(serialize_with = "color", deserialize_with = "read_color")]
    window_background: Color,
    /// Base window title background used when the active chrome role is not selected.
    #[serde(serialize_with = "color", deserialize_with = "read_color")]
    title_background: Color,
    /// Window title and caption foreground.
    #[serde(serialize_with = "color", deserialize_with = "read_color")]
    title_foreground: Color,
    /// Panel and scroll-viewport background.
    #[serde(serialize_with = "color", deserialize_with = "read_color")]
    panel_background: Color,
    /// Ordinary raised-control fill.
    #[serde(serialize_with = "color", deserialize_with = "read_color")]
    button: Color,
    /// Hovered raised-control fill.
    #[serde(serialize_with = "color", deserialize_with = "read_color")]
    button_hovered: Color,
    /// Ordinary recessed-control fill.
    #[serde(serialize_with = "color", deserialize_with = "read_color")]
    input: Color,
    /// Hovered recessed-control fill.
    #[serde(serialize_with = "color", deserialize_with = "read_color")]
    input_hovered: Color,
    /// Scrollbar track fill.
    #[serde(serialize_with = "color", deserialize_with = "read_color")]
    scrollbar_track: Color,
    /// Scrollbar thumb fill.
    #[serde(serialize_with = "color", deserialize_with = "read_color")]
    scrollbar_thumb: Color,
    /// Focused control border or fill accent.
    #[serde(serialize_with = "color", deserialize_with = "read_color")]
    control_focus: Color,
    /// Background used by hovered, keyboard-focused, open, or otherwise selected items.
    #[serde(serialize_with = "color", deserialize_with = "read_color")]
    selection_background: Color,
    /// Text and glyph color paired with [`Self::selection_background`].
    #[serde(serialize_with = "color", deserialize_with = "read_color")]
    selection_foreground: Color,
    /// Active window frame and title accent.
    #[serde(serialize_with = "color", deserialize_with = "read_color")]
    window_active: Color,
    /// Menu text and marker foreground.
    #[serde(serialize_with = "color", deserialize_with = "read_color")]
    menu_foreground: Color,
    /// Menu bar and popup background.
    #[serde(serialize_with = "color", deserialize_with = "read_color")]
    menu_background: Color,
    /// Disabled control and client-area background.
    #[serde(serialize_with = "color", deserialize_with = "read_color")]
    disabled_background: Color,
    /// Disabled body, menu, and control foreground.
    #[serde(serialize_with = "color", deserialize_with = "read_color")]
    disabled_foreground: Color,
    /// Disabled title and caption foreground.
    #[serde(serialize_with = "color", deserialize_with = "read_color")]
    disabled_title_foreground: Color,
}

/// Reads exactly four byte channels without a general-purpose sequence or COM decoder.
pub(super) fn read_color<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Color, D::Error> {
    let [r, g, b, a] = <[u8; 4]>::deserialize(deserializer)?;
    Ok(Color { r, g, b, a })
}

/// Preserves omitted/null overrides while validating present colors as fixed arrays.
pub(super) fn read_optional_color<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Option<Color>, D::Error> {
    Ok(Option::<[u8; 4]>::deserialize(deserializer)?.map(|[r, g, b, a]| Color { r, g, b, a }))
}

/// The nine-cell patch format has a fixed allocation and rejects missing or extra cells.
pub(super) fn read_cells<'de, D: Deserializer<'de>>(deserializer: D) -> Result<[Color; 9], D::Error> {
    Ok(<[[u8; 4]; 9]>::deserialize(deserializer)?.map(|[r, g, b, a]| Color { r, g, b, a }))
}

/// Reads all four required geometry fields; unknown and duplicate fields are errors.
pub(super) fn read_insets<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Insets, D::Error> {
    InsetsJson::deserialize(deserializer)
}

/// An absent geometry override retains inheritance rather than creating zero insets.
pub(super) fn read_optional_insets<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Option<Insets>, D::Error> {
    Ok(Option::<InsetValue>::deserialize(deserializer)?.map(|value| value.0))
}

/// Missing metric fields inherit the generated record's canonical defaults.
pub(super) fn read_metrics<'de, D: Deserializer<'de>>(deserializer: D) -> Result<AssetMetrics, D::Error> {
    MetricsJson::deserialize(deserializer)
}

/// Missing palette fields inherit the generated palette's canonical defaults.
pub(super) fn read_palette<'de, D: Deserializer<'de>>(deserializer: D) -> Result<FlatPalette, D::Error> {
    PaletteJson::deserialize(deserializer)
}

#[cfg(test)]
mod tests {
    use crate::theme::{AppearanceCatalogDocument, PatchDocument, ThemeSkinDocument, VisualOverrideDocument, WindowChromeDocument};
    use super::*;

    /// Partial remote records inherit canonical generated defaults, including nested geometry.
    #[test]
    fn partial_metrics_palette_and_chrome_preserve_defaults() {
        let skin: ThemeSkinDocument = serde_json::from_str(r#"{"metrics":{"padding":7},"palette":{"text":[1,2,3,4]}}"#).unwrap();
        let mut expected = ThemeSkinDocument::default();
        expected.metrics.padding = 7;
        expected.palette.text = Color { r: 1, g: 2, b: 3, a: 4 };
        assert_eq!(skin, expected);
        let chrome: WindowChromeDocument = serde_json::from_str(r#"{"captions":{"inner_spacing":3}}"#).unwrap();
        let mut expected = WindowChromeDocument::default();
        expected.captions.inner_spacing = 3;
        assert_eq!(chrome, expected);
    }

    /// Concrete records reject duplicate/unknown fields and incomplete required inset records.
    #[test]
    fn direct_records_keep_strict_shape_validation() {
        for json in [
            r#"{"metrics":{"padding":1,"padding":2}}"#,
            r#"{"metrics":{"unknown":1}}"#,
            r#"{"palette":{"text":[0,0,0,0],"text":[1,1,1,1]}}"#,
            r#"{"fallback_frame_insets":{"left":1}}"#,
            r#"{"window_chrome":{"captions":{"unknown":1}}}"#,
            r#"{"window_chrome":{"active_title_backdrop":{"color":[0,0,0,0],"horizontal_padding":1,"extra":0}}}"#,
        ] {
            assert!(serde_json::from_str::<ThemeSkinDocument>(json).is_err(), "{json}");
        }
    }

    /// Fixed arrays bound color/patch decoding; optional fields preserve inherited absence.
    #[test]
    fn colors_and_cells_enforce_exact_lengths_and_channel_domains() {
        for color in ["[1,2,3]", "[1,2,3,4,5]", "[0,0,0,256]", "[-1,0,0,0]"] {
            let json = format!(r#"{{"content_color":{color}}}"#);
            assert!(serde_json::from_str::<VisualOverrideDocument>(&json).is_err());
        }
        let absent: VisualOverrideDocument = serde_json::from_str(r#"{"insets":null,"content_color":null}"#).unwrap();
        assert_eq!(absent, VisualOverrideDocument::default());
        for count in [8, 9, 10] {
            let cells = vec![[0u8, 1, 2, 3]; count];
            let json = serde_json::json!({"type":"flat","cells":cells});
            assert_eq!(serde_json::from_value::<PatchDocument>(json).is_ok(), count == 9);
        }
    }

    /// Closed role maps reject duplicate identities before a later value can overwrite the first.
    #[test]
    fn role_maps_reject_duplicates_and_unknown_names() {
        for json in [r#"{"control":{"button":{},"button":{}}}"#, r#"{"control":{"buton":{}}}"#] {
            assert!(serde_json::from_str::<AppearanceCatalogDocument>(json).is_err());
        }
        let valid: AppearanceCatalogDocument = serde_json::from_str(r#"{"control":{"button":{}}}"#).unwrap();
        assert_eq!(valid.control.len(), 1);
    }
}
