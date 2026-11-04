//! Concrete immutable GFX bundle objects for compiled tables and builder output.
//!
//! This is reusable Rust support, not a plugin or another ABI module. Static
//! tables may be borrowed for the process lifetime; generated tables are owned.

#![deny(missing_docs)]

extern crate self as neocogi_gfx_support;
extern crate alloc;
use neocogi_gfx_support::abi::*;
use neocogi_render_core::abi::{IRgbaImageAbi, IRgbaImageVtblFor};
use neocogi_com::{ComPtr, ComRef, NcOutputWriter, NcStatus, NC_E_INVALID_ARG, com_object};
use std::borrow::Cow;

mod glyph_index;
/// CPU graphics assembly and command observations for integration tests and CPU benchmarks.
#[cfg(feature = "test-support")]
pub mod testing;
mod validation;
mod vertex;
pub use vertex::{MAX_UI_VERTICES, MAX_UI_BATCHES};
pub use glyph_index::AsciiGlyphIndex;

/// Concrete appearance states used while compiling and resolving complete records.
pub mod appearance;
pub use appearance::{ChromeState, ControlState, MenuState, PointerState, SurfaceState};

/// Portable metadata and resolved appearances, independent of atlas pixel storage.
///
/// UI consumers retain this value directly and add only process-local resource identities and
/// sorted lookup indices. Keeping pixels out of the type prevents a metadata consumer from
/// allocating or retaining the GPU's complete upload input.
#[derive(Clone)]
pub struct AssetTables {
    /// Positive atlas dimensions whose bundle supplies bounded tightly packed RGBA storage.
    pub size: Size,
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
    /// One named state record for every surface role in IDL declaration order.
    pub surfaces: Cow<'static, [AssetSurfaceVisuals]>,
    /// One named nested state record for every control role in IDL declaration order.
    pub controls: Cow<'static, [AssetControlVisuals]>,
    /// One named state record for every menu role in IDL declaration order.
    pub menus: Cow<'static, [AssetMenuVisuals]>,
    /// One named state record for every chrome role in IDL declaration order.
    pub window_chrome: Cow<'static, [AssetChromeVisuals]>,
}

/// Complete portable asset revision used by compilers and immutable bundle publication.
///
/// Construction services own both halves. UI consumers read [`AssetTables`] alone, while the
/// platform copies the packed RGBA bytes directly to its selected GPU backend.
#[derive(Clone)]
pub struct AssetData {
    /// Row-major RGBA bytes; compiled bundles may borrow a static include-bytes array.
    pub pixels: Cow<'static, [u8]>,
    /// Resource metadata and resolved appearances paired with these pixels.
    pub tables: AssetTables,
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

    /// Derives bounded counts and verifies the exact paired pixel length.
    pub fn info(&self) -> Result<AssetInfo, NcStatus> {
        let info = self.tables.info()?;
        if info.rgba_bytes()? != self.pixels.len() {
            return Err(NC_E_INVALID_ARG);
        }
        Ok(info)
    }

    /// Publishes exact table storage; receiving consumers validate record semantics.
    fn publish(self, catalog: ComPtr<IAssetCatalogAbi>) -> Result<ComPtr<IAssetBundleAbi>, NcStatus> {
        let info = self.info()?;
        // Metadata and pixels have independent COM identities. A later appearance
        // revision can retain this image without retaining any previous bundle.
        let image = Image::from_value(Image { size: info.size, pixels: self.pixels })
            .query()
            .expect("Image declares IRgbaImage");
        self.tables.publish(catalog, image)
    }
}

impl AssetTables {
    /// Publishes metadata over the base revision's unchanged immutable atlas storage.
    /// Resource rebuilds use `AssetData::into_revision` instead, since their pixels
    /// and packed coordinates must acquire a new image identity together.
    pub fn into_revision(self, base: ComRef<'_, IAssetBundleAbi>) -> Result<ComPtr<IAssetBundleAbi>, NcStatus> {
        let image = base.image()?;
        self.validate()?;
        self.validate_white_source(image.as_com_ref())?;
        self.publish(base.catalog()?, image)
    }

    /// Pairs checked dimensions with an owned image; foreign consumers still validate
    /// all record semantics before installation, as for newly constructed bundles.
    fn publish(self, catalog: ComPtr<IAssetCatalogAbi>, image: ComPtr<IRgbaImageAbi>) -> Result<ComPtr<IAssetBundleAbi>, NcStatus> {
        let info = self.info()?;
        if image.size()? != info.size {
            return Err(NC_E_INVALID_ARG);
        }
        Ok(Bundle::from_value(Bundle { catalog, info, tables: self, image })
            .query()
            .expect("Bundle declares IAssetBundle"))
    }

    /// Derives bounded counts and verifies one structured appearance per declared role.
    pub fn info(&self) -> Result<AssetInfo, NcStatus> {
        let info = AssetInfo {
            schema: ASSET_SCHEMA_VERSION,
            size: self.size,
            fonts: u32::try_from(self.fonts.len()).map_err(|_| NC_E_INVALID_ARG)?,
            glyphs: u32::try_from(self.glyphs.len()).map_err(|_| NC_E_INVALID_ARG)?,
            icons: u32::try_from(self.icons.len()).map_err(|_| NC_E_INVALID_ARG)?,
            names: u32::try_from(self.names.len()).map_err(|_| NC_E_INVALID_ARG)?,
        };
        info.rgba_bytes()?;
        if self.surfaces.len() != SurfaceRole::COUNT
            || self.controls.len() != ControlRole::COUNT
            || self.menus.len() != MenuRole::COUNT
            || self.window_chrome.len() != ChromeRole::COUNT
        {
            return Err(NC_E_INVALID_ARG);
        }
        Ok(info)
    }
}

#[com_object(interfaces(IAssetCatalog))]
/// Immutable namespace shared by revisions without retaining any provider or earlier revision.
struct Catalog;
impl IAssetCatalog for Catalog {}

#[com_object(interfaces(IAssetBundle))]
/// Single concrete implementation serves both static and dynamically constructed tables.
struct Bundle {
    /// Canonical namespace ownership remains independent of application and provider lifetimes.
    catalog: ComPtr<IAssetCatalogAbi>,
    /// Counts were derived from checked owned/static storage before publication.
    info: AssetInfo,
    /// Immutable metadata is never borrowed directly by foreign code.
    tables: AssetTables,
    /// Independent immutable pixels may be shared by appearance-only revisions.
    image: ComPtr<IRgbaImageAbi>,
}

impl IAssetBundle for Bundle {
    /// Retains exactly the image capability paired with this immutable revision.
    fn image(&self) -> Result<ComPtr<IRgbaImageAbi>, NcStatus> {
        Ok(self.image.clone())
    }
    /// Retains the namespace used to authenticate compatible UI replacements.
    fn catalog(&self) -> Result<ComPtr<IAssetCatalogAbi>, NcStatus> {
        Ok(self.catalog.clone())
    }
    /// Returns exact immutable table bounds.
    fn info(&self) -> Result<AssetInfo, NcStatus> {
        Ok(self.info)
    }
    /// Copies font metrics without exposing a borrowed Rust slice.
    fn fonts(&self, output: NcOutputWriter<'_, AssetFont>) -> Result<(), NcStatus> {
        copy(&self.tables.fonts, output)
    }
    /// Copies exact Unicode records in the producer's immutable order.
    fn glyphs(&self, output: NcOutputWriter<'_, AssetGlyph>) -> Result<(), NcStatus> {
        copy(&self.tables.glyphs, output)
    }
    /// Copies named image rectangles in bundle-local table order.
    fn icons(&self, output: NcOutputWriter<'_, AssetIcon>) -> Result<(), NcStatus> {
        copy(&self.tables.icons, output)
    }
    /// Copies UTF-8 bytes, with byte offsets and no implicit terminator.
    fn names(&self, output: NcOutputWriter<'_, u8>) -> Result<(), NcStatus> {
        copy(&self.tables.names, output)
    }
    /// Returns complete geometry without parser-specific defaults.
    fn metrics(&self) -> Result<AssetMetrics, NcStatus> {
        Ok(self.tables.metrics)
    }
    /// Returns the complete concrete caption recipe.
    fn chrome(&self) -> Result<AssetChrome, NcStatus> {
        Ok(self.tables.chrome)
    }
    /// Copies complete named surface states without a family tag or flattened state axis.
    fn surface_visuals(&self, output: NcOutputWriter<'_, AssetSurfaceVisuals>) -> Result<(), NcStatus> {
        copy(&self.tables.surfaces, output)
    }
    /// Copies complete named control states, preserving enabled and focused pointer branches.
    fn control_visuals(&self, output: NcOutputWriter<'_, AssetControlVisuals>) -> Result<(), NcStatus> {
        copy(&self.tables.controls, output)
    }
    /// Copies complete named menu states without relying on numeric state offsets.
    fn menu_visuals(&self, output: NcOutputWriter<'_, AssetMenuVisuals>) -> Result<(), NcStatus> {
        copy(&self.tables.menus, output)
    }
    /// Copies complete named chrome states without relying on numeric state offsets.
    fn chrome_visuals(&self, output: NcOutputWriter<'_, AssetChromeVisuals>) -> Result<(), NcStatus> {
        copy(&self.tables.window_chrome, output)
    }
}

#[com_object(interfaces(IRgbaImage))]
/// Pixel storage has no back-reference to bundle metadata or to a publication service.
struct Image {
    /// Positive dimensions validated against the tightly packed byte count at construction.
    size: Size,
    /// One owned or process-static allocation retained until the last image reference retires.
    pixels: Cow<'static, [u8]>,
}

impl IRgbaImage for Image {
    /// Returns the dimensions paired with the immutable tightly packed pixel storage.
    fn size(&self) -> Result<Size, NcStatus> {
        Ok(self.size)
    }

    /// Copies tightly packed RGBA bytes after validating the complete requested range.
    fn pixels(&self, output: NcOutputWriter<'_, u8>) -> Result<(), NcStatus> {
        copy(&self.pixels, output)
    }
}

/// Rejects invalid whole ranges before changing any destination element.
fn copy<T: neocogi_com::ComInlineValue>(values: &[T], mut output: NcOutputWriter<'_, T>) -> Result<(), NcStatus> {
    let first = usize::try_from(output.offset()).map_err(|_| NC_E_INVALID_ARG)?;
    let end = first.checked_add(output.capacity()).ok_or(NC_E_INVALID_ARG)?;
    output.extend_from_slice(values.get(first..end).ok_or(NC_E_INVALID_ARG)?)?;
    Ok(())
}

/// Expands one generated zlib stream into exactly the declared RGBA byte count.
///
/// Enabled by `compiled-assets`; no font, image-file or theme parser is involved. The
/// returned allocation belongs to AssetData and is freed with its immutable bundle.
/// Malformed, truncated, trailing, checksum-invalid and size-mismatched data fail
/// before publication. Output cannot grow past the shared 64 MiB asset limit.
#[cfg(feature = "compiled-assets")]
pub fn decode_pixels(compressed: &[u8], rgba_bytes: usize) -> Result<Vec<u8>, NcStatus> {
    use miniz_oxide::inflate::{
        TINFLStatus,
        core::{decompress, DecompressorOxide, inflate_flags::*},
    };
    if rgba_bytes == 0 || !rgba_bytes.is_multiple_of(4) || rgba_bytes > MAX_ASSET_PIXELS_BYTES {
        return Err(NC_E_INVALID_ARG);
    }
    // Decode directly into the final allocation: no growing buffer, temporary copy or global cache.
    let mut pixels = vec![0; rgba_bytes];
    let (status, consumed, written) = decompress(
        &mut DecompressorOxide::new(),
        compressed,
        &mut pixels,
        0,
        TINFL_FLAG_PARSE_ZLIB_HEADER | TINFL_FLAG_USING_NON_WRAPPING_OUTPUT_BUF,
    );
    if status != TINFLStatus::Done || consumed != compressed.len() || written != rgba_bytes {
        return Err(NC_E_INVALID_ARG);
    }
    Ok(pixels)
}

#[cfg(all(test, feature = "compiled-assets"))]
mod compiled_tests {
    use super::*;

    /// Exact-size expansion rejects corruption and never accepts a partially decoded candidate.
    #[test]
    fn compressed_pixels_require_one_complete_exact_sized_stream() {
        // Python zlib.compress(bytes([255, 255, 255, 255])) gives this independent stream.
        let compressed = [120, 156, 251, 255, 255, 255, 127, 0, 9, 250, 3, 253];
        assert_eq!(decode_pixels(&compressed, 4).unwrap(), [255; 4]);
        for size in [0, 3, 8, MAX_ASSET_PIXELS_BYTES + 4, usize::MAX] {
            assert_eq!(decode_pixels(&compressed, size), Err(NC_E_INVALID_ARG));
        }
        for end in 0..compressed.len() {
            assert_eq!(decode_pixels(&compressed[..end], 4), Err(NC_E_INVALID_ARG));
        }
        // A valid stream that expands beyond the declared buffer must also fail.
        assert_eq!(decode_pixels(&[120, 156, 251, 255, 31, 2, 0, 35, 228, 7, 249], 4), Err(NC_E_INVALID_ARG));
        let mut extra = compressed.to_vec();
        extra.push(0);
        assert_eq!(decode_pixels(&extra, 4), Err(NC_E_INVALID_ARG));
        let mut corrupt = compressed;
        corrupt[11] ^= 1;
        assert_eq!(decode_pixels(&corrupt, 4), Err(NC_E_INVALID_ARG));
        assert_eq!(decode_pixels(&[0; 16], 4), Err(NC_E_INVALID_ARG));
    }
}

/// Shared IDL bindings and their Rust conveniences.
pub mod abi;

/// Permissions delegated to a UI compositor on either a framebuffer or an image surface.
/// UI writes color and resources and imports images; depth and attachment clears remain denied.
pub const fn compositor_permissions() -> neocogi_render_core::abi::RenderClientPermissions {
    neocogi_render_core::abi::RenderClientPermissions {
        write_color: true,
        write_resources: true,
        import_images: true,
        ..neocogi_render_core::abi::RenderClientPermissions::none()
    }
}
