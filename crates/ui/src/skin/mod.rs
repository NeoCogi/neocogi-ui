//! Appearance implementation behind the consolidated UI root and offline tools.
//!
//! This module owns typed appearance policy and optional JSON schema processing.
//! It consumes generated GFX capabilities but contains no decoder, rasterizer,
//! atlas packer, bundle storage, or plugin identity.
#![deny(missing_docs)]

use neocogi_com::{ComPtr, INcStringAbi, NC_E_INVALID_ARG, NcStatus, NcString};
use std::cell::RefCell;

mod style;
pub use style::StyleCompilerObject;

#[cfg(feature = "theme-json")]
mod path;
#[cfg(feature = "theme-json")]
mod theme_loader;
#[cfg(feature = "theme-json")]
pub use theme_loader::ThemeLoaderObject;

/// Optional JSON authoring compiler producing ordinary portable resource tables.
#[cfg(feature = "theme-json")]
pub use neocogi_ui::authoring::theme;

/// Owned Skin diagnostics contain no parser borrow or downstream GFX object.
#[derive(Default)]
struct Diagnostic {
    /// Last failure is replaced before returning its status; clients receive immutable snapshots.
    message: RefCell<String>,
}

impl Diagnostic {
    /// Captures the underlying cause while returning the standard recoverable input failure.
    fn failure(&self, error: impl std::fmt::Display) -> NcStatus {
        *self.message.borrow_mut() = error.to_string();
        NC_E_INVALID_ARG
    }

    /// Returns independent text storage unaffected by later compiler operations.
    fn snapshot(&self) -> ComPtr<INcStringAbi> {
        NcString::from_utf8_str(&self.message.borrow())
    }
}

#[cfg(all(test, feature = "test-support", feature = "builder", feature = "theme-json"))]
mod tests;

mod factory;
pub(crate) use factory::factory;
