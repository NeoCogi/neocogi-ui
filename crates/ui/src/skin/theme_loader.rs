//! Typed Skin-to-GFX composition for external JSON theme construction.

use super::Diagnostic;
use neocogi_com::{ComPtr, ComRef, INcStringAbi, NcStatus, com_object};
use neocogi_ui_contracts::gfx::abi::{AssetLoaderAbi, AssetPublisherAbi, IAssetBundleAbi};
use neocogi_platform_contracts::render::platform::INativePathAbi;
use neocogi_ui_contracts::appearance::skin::{ISkinDiagnostics, ThemeLoader};

/// External theme compiler retaining the one explicit GFX construction capability it needs.
#[com_object(neocogi_ui_contracts::appearance::skin::ThemeLoader)]
pub struct ThemeLoaderObject {
    /// GFX owns all PNG decoding, font rasterization, atlas packing, and bundle construction.
    assets: ComPtr<AssetLoaderAbi>,
    /// GFX owns base copying, complete validation, and immutable revision publication.
    publisher: ComPtr<AssetPublisherAbi>,
    /// JSON, filesystem, and downstream GFX failures survive as owned diagnostic snapshots.
    diagnostic: Diagnostic,
}

impl ThemeLoaderObject {
    /// Retains supplied GFX capabilities, rejecting non-shareable borrows before publication.
    pub fn new(assets: ComRef<'_, AssetLoaderAbi>, publisher: ComRef<'_, AssetPublisherAbi>) -> Result<Self, NcStatus> {
        Ok(Self {
            assets: assets.try_add_ref()?,
            publisher: publisher.try_add_ref()?,
            diagnostic: Diagnostic::default(),
        })
    }
}

impl ThemeLoader for ThemeLoaderObject {
    /// Parses one external theme and delegates its complete resource build to GFX.
    fn load_theme(&self, path: ComRef<'_, INativePathAbi>, base: ComRef<'_, IAssetBundleAbi>) -> Result<ComPtr<IAssetBundleAbi>, NcStatus> {
        let path = crate::skin::path::decode(path)?;
        crate::skin::theme::load_with_gfx(&path, base, self.assets.as_com_ref(), self.publisher.as_com_ref()).map_err(|error| self.diagnostic.failure(error))
    }
}

impl ISkinDiagnostics for ThemeLoaderObject {
    /// Copies the latest failure text without exposing mutable diagnostic storage.
    fn diagnostic(&self) -> Result<ComPtr<INcStringAbi>, NcStatus> {
        Ok(self.diagnostic.snapshot())
    }
}
