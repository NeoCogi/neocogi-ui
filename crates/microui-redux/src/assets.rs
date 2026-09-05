//! Concrete immutable asset objects for compiled tables and optional builder output.
//! Static tables are borrowed for the process lifetime; generated tables are owned outright.
use crate::*;
use nc_com::{ComPtr, ComRef, NcCopyToBufferOffset, NcStatus, NC_E_INVALID_ARG, com_class};
use std::borrow::Cow;
use microui_contracts::appearance::*;

/// Portable owned-or-static tables used by both runtime builders and generated source files.
/// Creation checks storage lengths; consumers validate semantic contents before installation.
pub struct AssetData {
    /// Positive atlas dimensions with bounded tightly packed RGBA storage.
    pub size: Size,
    /// Row-major RGBA bytes; compiled bundles may borrow a static include_bytes array.
    pub pixels: Cow<'static, [u8]>,
    /// Font records partition the glyph table in declaration order.
    pub fonts: Cow<'static, [AssetFont]>,
    /// Unicode scalar metrics; underscore fallback and duplicates are checked by consumers.
    pub glyphs: Cow<'static, [AssetGlyph]>,
    /// Named positive rectangles including the required opaque white icon.
    pub icons: Cow<'static, [AssetIcon]>,
    /// UTF-8 names partitioned in font-then-icon order.
    pub names: Cow<'static, [u8]>,
    /// Complete resolved scalar geometry.
    pub metrics: AssetMetrics,
    /// Complete resolved title and caption policy.
    pub chrome: AssetChrome,
    /// SurfaceRole × SurfaceState in shared ALL order.
    pub surfaces: Cow<'static, [AssetVisual]>,
    /// ControlRole × ControlState in shared ALL order.
    pub controls: Cow<'static, [AssetVisual]>,
    /// MenuRole × MenuState in shared ALL order.
    pub menus: Cow<'static, [AssetVisual]>,
    /// ChromeRole × ChromeState in shared ALL order.
    pub window_chrome: Cow<'static, [AssetVisual]>,
}

impl AssetData {
    /// Publishes this immutable data in a new logical resource namespace.
    pub fn into_bundle(self) -> Result<ComPtr<IAssetBundleAbi>, NcStatus> {
        let catalog = Catalog::from_value(Catalog).query().expect("Catalog declares IAssetCatalog");
        self.publish(catalog)
    }

    /// Publishes a derived revision. The producer must preserve existing resource meanings;
    /// UI installation independently checks the catalog identity and original named resources.
    pub fn into_revision(self, base: ComRef<'_, IAssetBundleAbi>) -> Result<ComPtr<IAssetBundleAbi>, NcStatus> {
        self.publish(base.catalog()?)
    }

    /// Validates actual storage lengths before exposing immutable table access.
    fn publish(self, catalog: ComPtr<IAssetCatalogAbi>) -> Result<ComPtr<IAssetBundleAbi>, NcStatus> {
        let info = AssetInfo {
            schema: ASSET_SCHEMA_VERSION,
            size: self.size,
            fonts: u32::try_from(self.fonts.len()).map_err(|_| NC_E_INVALID_ARG)?,
            glyphs: u32::try_from(self.glyphs.len()).map_err(|_| NC_E_INVALID_ARG)?,
            icons: u32::try_from(self.icons.len()).map_err(|_| NC_E_INVALID_ARG)?,
            names: u32::try_from(self.names.len()).map_err(|_| NC_E_INVALID_ARG)?,
        };
        if info.rgba_bytes()? != self.pixels.len()
            || self.surfaces.len() != SurfaceRole::COUNT * SurfaceState::COUNT
            || self.controls.len() != ControlRole::COUNT * ControlState::COUNT
            || self.menus.len() != MenuRole::COUNT * MenuState::COUNT
            || self.window_chrome.len() != ChromeRole::COUNT * ChromeState::COUNT
        {
            return Err(NC_E_INVALID_ARG);
        }
        Ok(Bundle::from_value(Bundle { catalog, info, data: self })
            .query()
            .expect("Bundle declares IAssetBundle"))
    }
}

#[com_class(interfaces(IAssetCatalog))]
/// Immutable namespace shared by revisions without retaining any provider or earlier revision.
struct Catalog;
impl IAssetCatalog for Catalog {}

#[com_class(interfaces(IAssetBundle))]
/// Single concrete implementation serves both static and dynamically constructed tables.
struct Bundle {
    /// Canonical namespace ownership remains independent of application and provider lifetimes.
    catalog: ComPtr<IAssetCatalogAbi>,
    /// Counts were derived from checked owned/static storage before publication.
    info: AssetInfo,
    /// Immutable data is never borrowed directly by foreign code.
    data: AssetData,
}

impl IAssetBundle for Bundle {
    /// Retains the namespace used to authenticate compatible UI replacements.
    fn catalog(&self) -> Result<ComPtr<IAssetCatalogAbi>, NcStatus> {
        Ok(self.catalog.clone())
    }
    /// Returns exact immutable table bounds.
    fn info(&self) -> Result<AssetInfo, NcStatus> {
        Ok(self.info)
    }
    /// Copies tightly packed RGBA bytes after validating the entire range.
    fn pixels(&self, output: NcCopyToBufferOffset<'_, u8>) -> Result<(), NcStatus> {
        copy(&self.data.pixels, output)
    }
    /// Copies font metrics without exposing a borrowed Rust slice.
    fn fonts(&self, output: NcCopyToBufferOffset<'_, AssetFont>) -> Result<(), NcStatus> {
        copy(&self.data.fonts, output)
    }
    /// Copies exact Unicode records in the producer's immutable order.
    fn glyphs(&self, output: NcCopyToBufferOffset<'_, AssetGlyph>) -> Result<(), NcStatus> {
        copy(&self.data.glyphs, output)
    }
    /// Copies named image rectangles in bundle-local table order.
    fn icons(&self, output: NcCopyToBufferOffset<'_, AssetIcon>) -> Result<(), NcStatus> {
        copy(&self.data.icons, output)
    }
    /// Copies UTF-8 bytes, with byte offsets and no implicit terminator.
    fn names(&self, output: NcCopyToBufferOffset<'_, u8>) -> Result<(), NcStatus> {
        copy(&self.data.names, output)
    }
    /// Returns complete geometry without parser-specific defaults.
    fn metrics(&self) -> Result<AssetMetrics, NcStatus> {
        Ok(self.data.metrics)
    }
    /// Returns the complete concrete caption recipe.
    fn chrome(&self) -> Result<AssetChrome, NcStatus> {
        Ok(self.data.chrome)
    }
    /// Resolves a closed family tag before copying any caller-owned output.
    fn visuals(&self, family: AppearanceFamily, output: NcCopyToBufferOffset<'_, AssetVisual>) -> Result<(), NcStatus> {
        let values = match family {
            AppearanceFamily::SURFACE => &self.data.surfaces,
            AppearanceFamily::CONTROL => &self.data.controls,
            AppearanceFamily::MENU => &self.data.menus,
            AppearanceFamily::CHROME => &self.data.window_chrome,
            _ => return Err(NC_E_INVALID_ARG),
        };
        copy(values, output)
    }
}

/// Rejects invalid whole ranges before changing any destination element.
fn copy<T: Copy>(values: &[T], mut output: NcCopyToBufferOffset<'_, T>) -> Result<(), NcStatus> {
    let first = usize::try_from(output.offset()).map_err(|_| NC_E_INVALID_ARG)?;
    let end = first.checked_add(output.len()).ok_or(NC_E_INVALID_ARG)?;
    output.as_mut_slice().copy_from_slice(values.get(first..end).ok_or(NC_E_INVALID_ARG)?);
    Ok(())
}
