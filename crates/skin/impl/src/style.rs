//! Typed Skin policy and composition over the narrow GFX publication capability.

use crate::Diagnostic;
use neocogi_com::{ComPtr, ComRef, INcStringAbi, NcCopyFromBufferOffset, NcCopyToBufferOffset, NcStatus, com_factory};
use neocogi_gfx_contracts::*;
use neocogi_skin_contracts::{ISkinDiagnostics, PaletteEdit, StyleCompiler};
use neocogi_appearance::{AppearanceTables, rgba};

/// Copies the current fixed appearance tables without importing fonts, icons, or pixels.
fn read_appearance(base: ComRef<'_, IAssetBundleAbi>) -> Result<AppearanceTables, NcStatus> {
    // A complete flat table provides initialized storage without inventing invalid enum or
    // union representations. Exact-count COM copies replace every record before it is read.
    let mut tables = AppearanceTables::flat(Insets::default(), &FlatPalette::default());
    base.surface_visuals(NcCopyToBufferOffset::new(0, &mut tables.surfaces))?;
    base.control_visuals(NcCopyToBufferOffset::new(0, &mut tables.controls))?;
    base.menu_visuals(NcCopyToBufferOffset::new(0, &mut tables.menus))?;
    base.chrome_visuals(NcCopyToBufferOffset::new(0, &mut tables.window_chrome))?;
    Ok(tables)
}

/// Delegates validation and immutable revision publication to the owning GFX capability.
pub(crate) fn publish_appearance(
    tables: &AppearanceTables,
    publisher: ComRef<'_, AssetPublisherAbi>,
    base: ComRef<'_, IAssetBundleAbi>,
    metrics: AssetMetrics,
    chrome: AssetChrome,
) -> Result<ComPtr<IAssetBundleAbi>, NcStatus> {
    publisher.revise_appearance(
        base,
        AssetAppearanceRevision { metrics, chrome },
        NcCopyFromBufferOffset::new(0, &tables.surfaces),
        NcCopyFromBufferOffset::new(0, &tables.controls),
        NcCopyFromBufferOffset::new(0, &tables.menus),
        NcCopyFromBufferOffset::new(0, &tables.window_chrome),
    )
}

#[com_factory(neocogi_skin_contracts::StyleCompiler)]
/// Compiles resource revisions without borrowing a UI or retaining a renderer.
pub struct StyleCompilerObject {
    /// GFX owns base validation, bundle copying, and immutable revision publication.
    publisher: ComPtr<AssetPublisherAbi>,
    /// Owned diagnostics remain inspectable independently of each immutable result.
    diagnostic: Diagnostic,
}

impl StyleCompilerObject {
    /// Retains the explicit GFX revision capability supplied by the Skin plugin caller.
    pub fn new(publisher: ComRef<'_, AssetPublisherAbi>) -> Self {
        Self {
            publisher: publisher.add_ref(),
            diagnostic: Diagnostic::default(),
        }
    }
}

impl StyleCompiler for StyleCompilerObject {
    /// Compiles appearance policy locally and delegates generic bundle publication to GFX.
    fn derive_style(&self, base: ComRef<'_, IAssetBundleAbi>, metrics: AssetMetrics, palette: PaletteEdit) -> Result<ComPtr<IAssetBundleAbi>, NcStatus> {
        // The COM thunk already validated the edit tag and active payload.
        let mut tables = read_appearance(base)?;
        let mut chrome = base.chrome()?;
        if let PaletteEdit::Flat(palette) = palette {
            let insets = tables.surfaces[SurfaceRole::GenericFrame.raw() as usize].normal.insets;
            tables = AppearanceTables::flat(insets, &palette);
            if chrome.has_backdrop == 1 {
                chrome.backdrop_rgba = rgba(palette.title_background);
            }
        }
        // GFX checks all records against the base's actual resource table before publishing.
        publish_appearance(&tables, self.publisher.as_com_ref(), base, metrics, chrome).inspect_err(|error| {
            self.diagnostic.failure(error);
        })
    }
}

impl ISkinDiagnostics for StyleCompilerObject {
    /// Returns an owned failure snapshot without a borrow into the compiler.
    fn diagnostic(&self) -> Result<ComPtr<INcStringAbi>, NcStatus> {
        Ok(self.diagnostic.snapshot())
    }
}
