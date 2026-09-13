//! Compatibility access to portable bundle support owned by the GFX layer.
//!
//! New lower-level code may depend on `neocogi-gfx-support` directly. The UI
//! facade retains this module so applications need not coordinate two imports.

pub use neocogi_gfx_support::{AssetData, AssetTables};
#[cfg(feature = "compiled-assets")]
pub use neocogi_gfx_support::decode_pixels;
