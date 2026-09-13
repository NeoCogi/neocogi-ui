//! Private implementation for the deployable GFX plugin and offline asset tools.
//!
//! This crate exports concrete image, atlas, and bundle objects but no plugin
//! identity. Skin schema and theme implementation live in a separate package.
#![deny(missing_docs)]
use neocogi_com::{ComPtr, INcStringAbi, NcStatus, NcString, NC_E_INVALID_ARG};
use std::cell::RefCell;

pub use neocogi_gfx_support::{AssetData, AssetTables, ChromeState, ControlState, MenuState, PointerState, SurfaceState};

#[cfg(feature = "builder")]
pub mod atlas;
#[cfg(feature = "builder")]
mod builder;
#[cfg(feature = "builder")]
pub use builder::AssetLoaderObject;
#[cfg(feature = "publisher")]
mod publisher;
#[cfg(feature = "publisher")]
pub use publisher::AssetPublisherObject;
#[cfg(feature = "builder")]
mod path;
#[cfg(feature = "gfx")]
mod png;
#[cfg(feature = "gfx")]
pub use png::PngDecoderObject;

/// Shared owned diagnostics contain no decoder borrow or platform-native error object.
#[derive(Default)]
struct Diagnostic {
    /// Last failure is replaced before returning its status; clients receive immutable snapshots.
    message: RefCell<String>,
}
impl Diagnostic {
    /// Captures the underlying cause while returning the standard recoverable asset-input failure.
    fn failure(&self, error: impl std::fmt::Display) -> NcStatus {
        *self.message.borrow_mut() = error.to_string();
        NC_E_INVALID_ARG
    }
    /// Returns independent text storage, unaffected by later decoding or builder operations.
    fn snapshot(&self) -> ComPtr<INcStringAbi> {
        NcString::from_utf8_str(&self.message.borrow())
    }
}

/// Optional atomic Rust source generation for already resolved resource tables.
#[cfg(feature = "save-to-rust")]
pub mod export;

/// Test inputs share the demo's names and file paths without its runtime loader dependency.
#[cfg(all(test, feature = "builder"))]
#[path = "../../../../demos/assets/src/recipe_data.rs"]
mod recipe_data;
/// The same recipe exercises both direct and COM construction in asset tests.
#[cfg(all(test, feature = "builder"))]
#[path = "../../../../demos/assets/src/recipe.rs"]
mod test_recipe;
