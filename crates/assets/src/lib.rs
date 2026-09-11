//! Optional CPU asset work: compact PNG decoding, optional font/atlas construction and theme JSON.
//! Native paths, decoder state and construction data never escape this plugin through native handles.
#![deny(missing_docs)]
use neocogi_ui_contracts::*;
use neocogi_com::{ComPtr, INcStringAbi, NcStatus, NcString, NC_E_INVALID_ARG};
use std::cell::RefCell;

#[cfg(feature = "builder")]
pub mod atlas;
mod style;
pub use style::StyleCompilerObject;
#[cfg(feature = "builder")]
mod builder;
#[cfg(feature = "builder")]
pub use builder::AssetLoaderObject;
#[cfg(feature = "theme-json")]
pub use builder::ThemeLoaderObject;
mod png;
#[cfg(feature = "theme-json")]
mod serde_com;
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

/// Stateless process entry that creates the asset services selected by this assembly's features.
#[neocogi_com::com_factory(neocogi_ui_contracts::AssetsPluginFactory)]
struct AssetsPluginFactoryObject;

impl AssetsPluginFactory for AssetsPluginFactoryObject {
    /// Creates one independent decoder with its own diagnostic history.
    fn png_decoder(&self) -> Result<ComPtr<PngDecoderAbi>, NcStatus> {
        PngDecoderObject::from_value(PngDecoderObject::default()).query()
    }

    /// Creates one independent style compiler with its own diagnostic history.
    fn style_compiler(&self) -> Result<ComPtr<StyleCompilerAbi>, NcStatus> {
        StyleCompilerObject::from_value(StyleCompilerObject::default()).query()
    }

    /// Creates the optional atlas builder service when that implementation was compiled in.
    fn asset_loader(&self) -> Result<ComPtr<AssetLoaderAbi>, NcStatus> {
        #[cfg(feature = "builder")]
        {
            AssetLoaderObject::from_value(AssetLoaderObject::default()).query()
        }
        #[cfg(not(feature = "builder"))]
        {
            Err(neocogi_com::NC_E_NOT_FOUND)
        }
    }

    /// Creates the optional JSON theme service when that implementation was compiled in.
    fn theme_loader(&self) -> Result<ComPtr<ThemeLoaderAbi>, NcStatus> {
        #[cfg(feature = "theme-json")]
        {
            ThemeLoaderObject::from_value(ThemeLoaderObject::default()).query()
        }
        #[cfg(not(feature = "theme-json"))]
        {
            Err(neocogi_com::NC_E_NOT_FOUND)
        }
    }
}

neocogi_com::factory_plugin! {
    name = "neocogi.assets",
    version = "0.9.0-alpha.0",
    contracts = [neocogi_ui_contracts::MODULE],
    factory = AssetsPluginFactoryObject,
}
#[cfg(feature = "dynamic-plugin")]
neocogi_com::export_factory_plugin!();

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
