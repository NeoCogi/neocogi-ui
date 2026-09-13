//! Deployable appearance and external-theme compilation plugin.
//!
//! One coarse-grained factory owns flat style derivation and optional JSON theme
//! loading. The plugin does not expose its parser, atlas drafts, or decoded images.

#![deny(missing_docs)]

use neocogi_assets::StyleCompilerObject;
#[cfg(feature = "theme-json")]
use neocogi_assets::ThemeLoaderObject;
use neocogi_com::{ComPtr, NcStatus};
use neocogi_skin_contracts::*;

/// Stateless activation root for cohesive skin compilation services.
#[neocogi_com::com_factory(neocogi_skin_contracts::SkinPluginFactory)]
struct SkinPluginFactoryObject;

impl SkinPluginFactory for SkinPluginFactoryObject {
    /// Creates a compiler whose mutable diagnostic history belongs to the returned object.
    fn style_compiler(&self) -> Result<ComPtr<StyleCompilerAbi>, NcStatus> {
        StyleCompilerObject::from_value(StyleCompilerObject::default()).query()
    }

    /// Creates the optional external JSON compiler selected by the deployment.
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
    name = "neocogi.skin.runtime",
    version = "0.9.0-alpha.0",
    contracts = [neocogi_skin_contracts::MODULE],
    factory = SkinPluginFactoryObject,
}

// Exports the dynamic entrypoint only from an assembly-generated native library.
#[cfg(feature = "dynamic-plugin")]
neocogi_com::export_factory_plugin!();
