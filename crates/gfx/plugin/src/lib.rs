//! Deployable CPU image decoding and portable resource-baking plugin.
//!
//! The plugin deliberately groups PNG normalization, font rasterization, and
//! atlas packing because they share one bounded pixel pipeline. Skin schemas and
//! appearance-selection policy live in the separate `neocogi.skin` plugin.

#![deny(missing_docs)]

#[cfg(feature = "builder")]
use neocogi_gfx_impl::AssetLoaderObject;
use neocogi_gfx_impl::{AssetPublisherObject, GfxCompositorObject, PngDecoderObject};
use neocogi_com::{ComPtr, NcStatus};
use neocogi_gfx_support::abi::*;

/// Stateless activation root for cohesive CPU graphics construction services.
#[neocogi_com::com_object(neocogi_gfx_support::abi::GfxPluginFactory)]
struct GfxPluginFactoryObject;

impl GfxPluginFactory for GfxPluginFactoryObject {
    /// Creates a decoder whose mutable diagnostic history belongs to the returned object.
    fn png_decoder(&self) -> Result<ComPtr<PngDecoderAbi>, NcStatus> {
        PngDecoderObject::from_value(PngDecoderObject::default()).query()
    }

    /// Creates an atlas builder when this deployment includes native font construction.
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

    /// Creates the narrow revision publisher available with or without native atlas construction.
    fn asset_publisher(&self) -> Result<ComPtr<AssetPublisherAbi>, NcStatus> {
        AssetPublisherObject::from_value(AssetPublisherObject::default()).query()
    }

    /// Imports one validated resource revision into an independently mutable compositor.
    fn create_compositor(&self, assets: neocogi_com::ComRef<'_, IAssetBundleAbi>) -> Result<ComPtr<GfxCompositorAbi>, NcStatus> {
        GfxCompositorObject::create(assets)
    }
}

/// Acquires one owned root; object construction remains on its typed methods.
#[neocogi_com::plugin(name = "neocogi.gfx.runtime")]
fn acquire_root(_runtime: ::neocogi_com::Runtime) -> Result<neocogi_com::ComPtr<neocogi_gfx_support::abi::GfxPluginFactoryAbi>, neocogi_com::NcStatus> {
    GfxPluginFactoryObject::from_value(GfxPluginFactoryObject).query()
}

// Exports the dynamic entrypoint only from an assembly-generated native library.
