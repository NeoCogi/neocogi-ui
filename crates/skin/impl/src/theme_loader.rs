//! Typed Skin-to-GFX composition for external JSON theme construction.

use super::Diagnostic;
use neocogi_com::{ComPtr, ComRef, INcStringAbi, NcStatus, com_factory};
use neocogi_gfx_contracts::{AssetLoaderAbi, AssetPublisherAbi, IAssetBundleAbi};
use neocogi_platform_contracts::INativePathAbi;
use neocogi_skin_contracts::{ISkinDiagnostics, ThemeLoader};

/// External theme compiler retaining the one explicit GFX construction capability it needs.
#[com_factory(neocogi_skin_contracts::ThemeLoader)]
pub struct ThemeLoaderObject {
    /// GFX owns all PNG decoding, font rasterization, atlas packing, and bundle construction.
    assets: ComPtr<AssetLoaderAbi>,
    /// GFX owns base copying, complete validation, and immutable revision publication.
    publisher: ComPtr<AssetPublisherAbi>,
    /// JSON, filesystem, and downstream GFX failures survive as owned diagnostic snapshots.
    diagnostic: Diagnostic,
}

impl ThemeLoaderObject {
    /// Retains a typed GFX loader supplied by the Skin plugin factory caller.
    pub fn new(assets: ComRef<'_, AssetLoaderAbi>, publisher: ComRef<'_, AssetPublisherAbi>) -> Self {
        Self {
            assets: assets.add_ref(),
            publisher: publisher.add_ref(),
            diagnostic: Diagnostic::default(),
        }
    }
}

impl ThemeLoader for ThemeLoaderObject {
    /// Parses one external theme and delegates its complete resource build to GFX.
    fn load_theme(&self, path: ComRef<'_, INativePathAbi>, base: ComRef<'_, IAssetBundleAbi>) -> Result<ComPtr<IAssetBundleAbi>, NcStatus> {
        let path = crate::path::decode(path)?;
        crate::theme::load_with_gfx(&path, base, self.assets.as_com_ref(), self.publisher.as_com_ref()).map_err(|error| self.diagnostic.failure(error))
    }
}

impl ISkinDiagnostics for ThemeLoaderObject {
    /// Copies the latest failure text without exposing mutable diagnostic storage.
    fn diagnostic(&self) -> Result<ComPtr<INcStringAbi>, NcStatus> {
        Ok(self.diagnostic.snapshot())
    }
}
