//! Deployable appearance and external-theme compilation plugin.
//!
//! One coarse-grained factory owns flat style derivation and optional JSON theme
//! loading. The plugin does not expose its parser, atlas drafts, or decoded images.

#![deny(missing_docs)]

use neocogi_skin_impl::StyleCompilerObject;
#[cfg(feature = "theme-json")]
use neocogi_skin_impl::ThemeLoaderObject;
use neocogi_com::{ComPtr, ComRef, NcGuid, NcStatus, guid_from_qualified_name};
use neocogi_gfx_support::abi::{AssetLoaderAbi, AssetPublisherAbi};
use neocogi_appearance::skin::*;

/// Hard provider dependency retained by the assembly planner before Skin activation.
const GFX_PLUGIN_ID: NcGuid = guid_from_qualified_name("neocogi.gfx.runtime");

/// Stateless activation root for cohesive skin compilation services.
#[neocogi_com::com_object(neocogi_appearance::skin::SkinPluginFactory)]
struct SkinPluginFactoryObject;

impl SkinPluginFactory for SkinPluginFactoryObject {
    /// Creates a compiler that delegates generic revision publication to the supplied GFX root.
    fn style_compiler(&self, publisher: ComRef<'_, AssetPublisherAbi>) -> Result<ComPtr<StyleCompilerAbi>, NcStatus> {
        StyleCompilerObject::from_value(StyleCompilerObject::new(publisher)?).query()
    }

    /// Creates the optional external JSON compiler selected by the deployment.
    fn theme_loader(&self, assets: ComRef<'_, AssetLoaderAbi>, publisher: ComRef<'_, AssetPublisherAbi>) -> Result<ComPtr<ThemeLoaderAbi>, NcStatus> {
        #[cfg(feature = "theme-json")]
        {
            ThemeLoaderObject::from_value(ThemeLoaderObject::new(assets, publisher)?).query()
        }
        #[cfg(not(feature = "theme-json"))]
        {
            let _ = (assets, publisher);
            Err(neocogi_com::NC_E_NOT_FOUND)
        }
    }
}

/// Acquires one owned root; object construction remains on its typed methods.
#[neocogi_com::plugin(name = "neocogi.skin.runtime",
    dependencies = [GFX_PLUGIN_ID],
    )]
fn acquire_root(_runtime: ::neocogi_com::Runtime) -> Result<neocogi_com::ComPtr<neocogi_appearance::skin::SkinPluginFactoryAbi>, neocogi_com::NcStatus> {
    SkinPluginFactoryObject::from_value(SkinPluginFactoryObject).query()
}

// Exports the dynamic entrypoint only from an assembly-generated native library.
