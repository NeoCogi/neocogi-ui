//! Portable bundle values consumed by retained UI resource import.
//!
//! Compression and asset construction remain owned by `neocogi-gfx-support`.
//! This facade exposes only the unconditionally available data representation
//! used at the typed UI boundary and its bounded immutable glyph lookup helper.

pub use neocogi_gfx_support::{AssetData, AssetTables, AsciiGlyphIndex};
