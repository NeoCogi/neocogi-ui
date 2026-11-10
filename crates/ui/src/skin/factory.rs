//! Appearance service owned by the consolidated UI implementation.
//!
//! One coarse-grained factory owns flat style derivation and optional JSON theme
//! loading. The plugin does not expose its parser, atlas drafts, or decoded images.

#![deny(missing_docs)]

use neocogi_ui::skin::StyleCompilerObject;
#[cfg(feature = "theme-json")]
use neocogi_ui::skin::ThemeLoaderObject;
use neocogi_com::{ComPtr, ComRef, NcStatus};
use neocogi_ui_contracts::gfx::abi::{AssetLoaderAbi, AssetPublisherAbi};
use neocogi_ui_contracts::appearance::skin::*;

/// Stateless activation root for cohesive skin compilation services.
#[neocogi_com::com_object(neocogi_ui_contracts::appearance::skin::SkinPluginFactory)]
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
pub(crate) fn factory() -> Result<neocogi_com::ComPtr<neocogi_ui_contracts::appearance::skin::SkinPluginFactoryAbi>, neocogi_com::NcStatus> {
    SkinPluginFactoryObject::from_value(SkinPluginFactoryObject).query()
}
