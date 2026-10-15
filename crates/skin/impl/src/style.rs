//! Typed Skin policy and composition over the narrow GFX publication capability.

use crate::Diagnostic;
use neocogi_com::{ComPtr, ComRef, INcStringAbi, NcCopyFromBufferOffset, NcStatus, com_object};
use neocogi_gfx_contracts::*;
use neocogi_skin_contracts::{ISkinDiagnostics, PaletteEdit, StyleCompiler};
use neocogi_appearance::{AppearanceTables, rgba};

/// Copies the current fixed appearance tables without importing fonts, icons, or pixels.
fn read_appearance(base: ComRef<'_, IAssetBundleAbi>) -> Result<AppearanceTables, NcStatus> {
    // Each table is fresh output; no flat-palette placeholders cross COM.
    let mut surfaces = neocogi_com::NcOutputBuffer::new();
    let mut controls = neocogi_com::NcOutputBuffer::new();
    let mut menus = neocogi_com::NcOutputBuffer::new();
    let mut chrome = neocogi_com::NcOutputBuffer::new();
    base.surface_visuals(surfaces.output(0, SurfaceRole::COUNT))?;
    base.control_visuals(controls.output(0, ControlRole::COUNT))?;
    base.menu_visuals(menus.output(0, MenuRole::COUNT))?;
    base.chrome_visuals(chrome.output(0, ChromeRole::COUNT))?;
    let tables = AppearanceTables {
        surfaces: surfaces.into_vec().try_into().map_err(|_| neocogi_com::NC_E_INVALID_ARG)?,
        controls: controls.into_vec().try_into().map_err(|_| neocogi_com::NC_E_INVALID_ARG)?,
        menus: menus.into_vec().try_into().map_err(|_| neocogi_com::NC_E_INVALID_ARG)?,
        window_chrome: chrome.into_vec().try_into().map_err(|_| neocogi_com::NC_E_INVALID_ARG)?,
    };
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

#[com_object(neocogi_skin_contracts::StyleCompiler)]
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
