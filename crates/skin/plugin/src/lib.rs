//! Deployable appearance and external-theme compilation plugin.
//!
//! One coarse-grained factory owns flat style derivation and optional JSON theme
//! loading. The plugin does not expose its parser, atlas drafts, or decoded images.

#![deny(missing_docs)]

use neocogi_skin_impl::StyleCompilerObject;
#[cfg(feature = "theme-json")]
use neocogi_skin_impl::ThemeLoaderObject;
use neocogi_com::{ComPtr, ComRef, NcGuid, NcStatus, guid_from_qualified_name};
use neocogi_gfx_contracts::{AssetLoaderAbi, AssetPublisherAbi};
use neocogi_skin_contracts::*;

/// Hard provider dependency retained by the assembly planner before Skin activation.
const GFX_PLUGIN_ID: NcGuid = guid_from_qualified_name("neocogi.gfx.runtime");

/// Stateless activation root for cohesive skin compilation services.
#[neocogi_com::com_factory(neocogi_skin_contracts::SkinPluginFactory)]
struct SkinPluginFactoryObject;

impl SkinPluginFactory for SkinPluginFactoryObject {
    /// Creates a compiler that delegates generic revision publication to the supplied GFX root.
    fn style_compiler(&self, publisher: ComRef<'_, AssetPublisherAbi>) -> Result<ComPtr<StyleCompilerAbi>, NcStatus> {
        StyleCompilerObject::from_value(StyleCompilerObject::new(publisher)).query()
    }

    /// Creates the optional external JSON compiler selected by the deployment.
    fn theme_loader(&self, assets: ComRef<'_, AssetLoaderAbi>, publisher: ComRef<'_, AssetPublisherAbi>) -> Result<ComPtr<ThemeLoaderAbi>, NcStatus> {
        #[cfg(feature = "theme-json")]
        {
            ThemeLoaderObject::from_value(ThemeLoaderObject::new(assets, publisher)).query()
        }
        #[cfg(not(feature = "theme-json"))]
        {
            let _ = (assets, publisher);
            Err(neocogi_com::NC_E_NOT_FOUND)
        }
    }
}

neocogi_com::factory_plugin! {
    name = "neocogi.skin.runtime",
    version = "0.9.0-alpha.0",
    dependencies = [GFX_PLUGIN_ID],
    contracts = [neocogi_skin_contracts::MODULE],
    factory = SkinPluginFactoryObject,
}

// Exports the dynamic entrypoint only from an assembly-generated native library.
#[cfg(feature = "dynamic-plugin")]
neocogi_com::export_factory_plugin!();
