//! Shared implementation for the separately deployable GFX and skin plugins.
//!
//! This crate exports concrete implementation objects and offline compiler APIs;
//! it does not publish a plugin identity. `neocogi-gfx` owns pixel/resource
//! activation and `neocogi-skin` owns appearance/theme activation.
#![deny(missing_docs)]
use neocogi_com::{ComPtr, INcStringAbi, NcStatus, NcString, NC_E_INVALID_ARG};
use std::cell::RefCell;

pub use neocogi_gfx_support::{AssetData, AssetTables, ChromeState, ControlState, MenuState, PointerState, SurfaceState};

#[cfg(feature = "builder")]
pub mod atlas;
#[cfg(any(feature = "builder", feature = "skin"))]
mod style;
#[cfg(feature = "skin")]
pub use style::StyleCompilerObject;
#[cfg(feature = "builder")]
mod builder;
#[cfg(feature = "builder")]
pub use builder::AssetLoaderObject;
#[cfg(feature = "theme-json")]
pub use builder::ThemeLoaderObject;
#[cfg(feature = "builder")]
mod path;
#[cfg(feature = "gfx")]
mod png;
#[cfg(feature = "theme-json")]
mod serde_com;
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

/// Optional JSON authoring compiler producing ordinary portable resource tables.
#[cfg(feature = "theme-json")]
pub mod theme;

/// Optional atomic Rust source generation for already resolved resource tables.
#[cfg(feature = "save-to-rust")]
pub mod export;

/// Test inputs share the demo's names and file paths without its runtime loader dependency.
#[cfg(all(test, feature = "builder"))]
#[path = "../../../demos/assets/src/recipe_data.rs"]
mod recipe_data;
/// The same recipe exercises both direct and COM construction in asset tests.
#[cfg(all(test, feature = "builder"))]
#[path = "../../../demos/assets/src/recipe.rs"]
mod test_recipe;
