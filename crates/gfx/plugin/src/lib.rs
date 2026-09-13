//! Deployable CPU image decoding and portable resource-baking plugin.
//!
//! The plugin deliberately groups PNG normalization, font rasterization, and
//! atlas packing because they share one bounded pixel pipeline. Skin schemas and
//! appearance-selection policy live in the separate `neocogi.skin` plugin.

#![deny(missing_docs)]

#[cfg(feature = "builder")]
use neocogi_assets::AssetLoaderObject;
use neocogi_assets::{AssetPublisherObject, PngDecoderObject};
use neocogi_com::{ComPtr, NcStatus};
use neocogi_gfx_contracts::*;

/// Stateless activation root for cohesive CPU graphics construction services.
#[neocogi_com::com_factory(neocogi_gfx_contracts::GfxPluginFactory)]
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
}

neocogi_com::factory_plugin! {
    name = "neocogi.gfx.runtime",
    version = "0.9.0-alpha.0",
    contracts = [neocogi_gfx_contracts::MODULE],
    factory = GfxPluginFactoryObject,
}

// Exports the dynamic entrypoint only from an assembly-generated native library.
#[cfg(feature = "dynamic-plugin")]
neocogi_com::export_factory_plugin!();
