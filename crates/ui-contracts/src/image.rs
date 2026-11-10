//! Thin owning image client caches only the dimensions guaranteed immutable by ITexture.
use crate::{ITextureAbi, Size};
use neocogi_com::{ComPtr, ComRef, NcStatus};

/// One retained texture plus its checked immutable measurement dimensions.
#[derive(Clone)]
pub struct Image {
    /// The renderer-owned resource keeps its device alive through final release.
    interface: ComPtr<ITextureAbi>,
    /// Read once before publication; ITexture forbids subsequent size mutation.
    size: Size,
}
impl Image {
    /// Validates a foreign texture's immutable size before a widget commits the new content.
    pub fn from_interface(interface: ComPtr<ITextureAbi>) -> Result<Self, NcStatus> {
        let size = interface.size()?;
        if size.width <= 0 || size.height <= 0 {
            return Err(neocogi_com::NC_E_INVALID_ARG);
        }
        Ok(Self { interface, size })
    }
    /// Returns the original validated dimensions with no per-measurement COM call.
    pub fn size(&self) -> Size {
        self.size
    }
    /// Borrows the owned resource for scoped drawing without an implicit reference increment.
    pub fn as_interface(&self) -> ComRef<'_, ITextureAbi> {
        self.interface.as_com_ref()
    }
    /// Transfers this wrapper's proven shared owner without another acquisition.
    pub fn into_interface(self) -> ComPtr<ITextureAbi> {
        self.interface
    }
}
