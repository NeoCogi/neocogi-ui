//! Optional CPU asset work: compact PNG decoding, optional font/atlas construction and theme JSON.
//! Native paths, decoder state and construction data never escape this plugin through native handles.
#![deny(missing_docs)]
use microui_contracts::*;
use nc_com::{ComPtr, INcStringAbi, NcStatus, NcString, NC_E_INVALID_ARG};
use std::cell::RefCell;

#[cfg(feature = "builder")]
pub mod atlas;
mod style;
pub use style::StyleCompilerObject;
#[cfg(feature = "builder")]
mod builder;
#[cfg(feature = "builder")]
pub use builder::AssetLoader;
#[cfg(feature = "theme-json")]
pub use builder::ThemeLoader;
mod png;
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

#[cfg(not(feature = "builder"))]
nc_com::plugin! {
    plugin_id(auto_guid = "microui.plugin.assets"), name = "microui.plugin.assets",
    version = "0.9.0-alpha.0", classes = [PngDecoderObject, StyleCompilerObject],
}
#[cfg(all(feature = "builder", not(feature = "theme-json")))]
nc_com::plugin! {
    plugin_id(auto_guid = "microui.plugin.assets"), name = "microui.plugin.assets",
    version = "0.9.0-alpha.0", classes = [PngDecoderObject, StyleCompilerObject, AssetLoader],
}
#[cfg(feature = "theme-json")]
nc_com::plugin! {
    plugin_id(auto_guid = "microui.plugin.assets"), name = "microui.plugin.assets",
    version = "0.9.0-alpha.0", classes = [PngDecoderObject, StyleCompilerObject, AssetLoader, ThemeLoader],
}
#[cfg(feature = "dynamic-plugin")]
nc_com::export_plugin!();

/// Optional JSON authoring compiler producing ordinary portable resource tables.
#[cfg(feature = "theme-json")]
pub mod theme;

/// Optional atomic Rust source generation for already resolved resource tables.
#[cfg(feature = "save-to-rust")]
pub mod export;

/// Test inputs share the demo's names and file paths without its runtime loader dependency.
#[cfg(all(test, feature = "builder"))]
#[path = "../../../demos/demo-assets/src/recipe_data.rs"]
mod recipe_data;
/// The same recipe exercises both direct and COM construction in asset tests.
#[cfg(all(test, feature = "builder"))]
#[path = "../../../demos/demo-assets/src/recipe.rs"]
mod test_recipe;
