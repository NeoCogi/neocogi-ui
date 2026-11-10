//! Portable bundle values consumed by retained UI resource import.
//!
//! Portable compressed tables live in `gfx`; construction belongs to the static SDK.
//! This facade exposes only the unconditionally available data representation
//! used at the typed UI boundary and its bounded immutable glyph lookup helper.

pub use neocogi_ui_contracts::gfx::{AssetData, AssetTables, AsciiGlyphIndex};
