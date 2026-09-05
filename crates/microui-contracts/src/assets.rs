//! Immutable resource transfer: bulk copies during installation, local lookup during frames.
use crate::{Rect, Size};
use nc_com::{ComPtr, ComRef, NcCopyToBufferOffset, NcStatus, com_enum, com_interface, com_int_enum, com_struct};

/// Bulk asset semantic schema, including canonical appearance role/state table order.
/// Bump when table ordering, mandatory resource names or implicit interpretation changes.
pub const ASSET_SCHEMA_VERSION: u32 = 1;

/// Maximum uncompressed RGBA storage accepted by all asset consumers.
pub const MAX_ASSET_PIXELS_BYTES: usize = 64 * 1024 * 1024;
/// Maximum glyph records in a complete bundle, across every font.
pub const MAX_ASSET_GLYPHS: u32 = 1024 * 1024;
/// Maximum distinct font names in one bundle.
pub const MAX_ASSET_FONTS: u32 = 4096;
/// Maximum distinct icon names in one bundle.
pub const MAX_ASSET_ICONS: u32 = 65536;
/// Maximum combined UTF-8 bytes occupied by resource names.
pub const MAX_ASSET_NAMES: u32 = 4 * 1024 * 1024;

#[com_struct]
#[derive(Debug, Default, PartialEq, Eq)]
/// Checked contiguous range in a bundle's name bytes or glyph records.
pub struct AssetRange {
    /// First byte or record; arithmetic is checked before indexing.
    pub first: u32,
    /// Number of bytes or records, without a terminator.
    pub count: u32,
}

#[com_struct]
#[derive(Debug, Default, PartialEq, Eq)]
/// Allocation bounds read before copying any variable-length table.
pub struct AssetInfo {
    /// Exact ASSET_SCHEMA_VERSION; reject stale semantic table layouts before allocation.
    pub schema: u32,
    /// Positive atlas dimensions, with exactly width * height * 4 RGBA bytes.
    pub size: Size,
    /// Number of font records, bounded by MAX_ASSET_FONTS.
    pub fonts: u32,
    /// Number of icon records, bounded by MAX_ASSET_ICONS.
    pub icons: u32,
    /// Number of glyph records, bounded by MAX_ASSET_GLYPHS.
    pub glyphs: u32,
    /// Combined UTF-8 name storage, bounded by MAX_ASSET_NAMES.
    pub names: u32,
}

impl AssetInfo {
    /// Checks the shared allocation policy before any consumer reserves or decodes storage.
    pub fn rgba_bytes(self) -> Result<usize, NcStatus> {
        if self.schema != ASSET_SCHEMA_VERSION
            || self.size.width <= 0
            || self.size.height <= 0
            || self.fonts == 0
            || self.fonts > MAX_ASSET_FONTS
            || self.icons == 0
            || self.icons > MAX_ASSET_ICONS
            || self.glyphs > MAX_ASSET_GLYPHS
            || self.names > MAX_ASSET_NAMES
        {
            return Err(nc_com::NC_E_INVALID_ARG);
        }
        let bytes = (self.size.width as usize)
            .checked_mul(self.size.height as usize)
            .and_then(|pixels| pixels.checked_mul(4))
            .ok_or(nc_com::NC_E_INVALID_ARG)?;
        if bytes > MAX_ASSET_PIXELS_BYTES {
            return Err(nc_com::NC_E_INVALID_ARG);
        }
        Ok(bytes)
    }
}

#[com_struct]
#[derive(Debug, Default, PartialEq, Eq)]
/// Font metrics and an exact partition of the bulk glyph table.
pub struct AssetFont {
    /// Nonempty UTF-8 font name, unique among fonts in this catalog.
    pub name: AssetRange,
    /// Contiguous glyph records; fonts partition the table in declaration order.
    pub glyphs: AssetRange,
    /// Positive distance between text baselines in pixels.
    pub line_size: i32,
    /// Baseline offset from the top of the line.
    pub baseline: i32,
    /// Positive requested pixel size used to construct this font.
    pub font_size: i32,
}

#[com_struct]
#[derive(Debug, Default, PartialEq, Eq)]
/// One Unicode scalar's immutable measurement and texture coordinates.
pub struct AssetGlyph {
    /// Valid Unicode scalar; duplicate characters in a font are rejected.
    pub scalar: u32,
    /// Horizontal offset from the text cursor in pixels.
    pub offset_x: i32,
    /// Vertical offset from the text cursor in pixels.
    pub offset_y: i32,
    /// Horizontal cursor advance in pixels.
    pub advance_x: i32,
    /// Vertical cursor advance in pixels.
    pub advance_y: i32,
    /// Nonnegative rectangle wholly inside the atlas; empty glyphs are permitted.
    pub rect: Rect,
}

#[com_struct]
#[derive(Debug, Default, PartialEq, Eq)]
/// A named positive atlas rectangle; the required white icon must be opaque white.
pub struct AssetIcon {
    /// Nonempty UTF-8 name, unique among icons in this catalog.
    pub name: AssetRange,
    /// Positive rectangle wholly inside the atlas.
    pub rect: Rect,
}

#[com_struct]
#[derive(Debug, Default, PartialEq, Eq)]
/// Four nonnegative pixel insets used by resolved theme geometry.
pub struct Insets {
    /// Leading horizontal thickness.
    pub left: i32,
    /// Leading vertical thickness.
    pub top: i32,
    /// Trailing horizontal thickness.
    pub right: i32,
    /// Trailing vertical thickness.
    pub bottom: i32,
}

#[com_struct]
#[derive(Debug, Default, PartialEq, Eq)]
/// Complete scalar layout recipe; installation rejects negative dimensions.
pub struct AssetMetrics {
    /// Preferred width when a layout supplies none.
    pub default_cell_width: i32,
    /// Inner control padding.
    pub padding: i32,
    /// Window body padding independent of controls.
    pub window_content_insets: Insets,
    /// Distance between neighboring cells.
    pub spacing: i32,
    /// Nested content indentation.
    pub indent: i32,
    /// Window title height.
    pub title_height: i32,
    /// Structural top-level border thickness.
    pub window_border: Insets,
    /// Scrollbar transverse size.
    pub scrollbar_size: i32,
    /// Minimum scrollbar and slider thumb size.
    pub thumb_size: i32,
}

com_int_enum! {
    /// Window title placement within the available caption geometry.
    pub enum TitleAlignment: u8 {
        /// Start at the leading edge.
        LEADING = 0,
        /// Center within the title, reserving symmetric caption banks.
        CENTERED = 1,
    }
}

com_int_enum! {
    /// Caption bank containing one window control.
    pub enum CaptionSide: u8 {
        /// Place in the leading bank.
        LEADING = 0,
        /// Place in the trailing bank.
        TRAILING = 1,
    }
}

#[com_struct]
#[derive(Debug, PartialEq, Eq)]
/// Complete caption and title geometry; boolean fields accept only zero or one.
pub struct AssetChrome {
    /// Leading or centered title placement.
    pub title_alignment: TitleAlignment,
    /// Leading or trailing close button bank.
    pub close_side: CaptionSide,
    /// Leading or trailing minimize button bank.
    pub minimize_side: CaptionSide,
    /// Leading or trailing maximize button bank.
    pub maximize_side: CaptionSide,
    /// Pixels above and below square captions.
    pub vertical_spacing: i32,
    /// Minimum caption extent.
    pub minimum_extent: i32,
    /// Gap between title and nonempty caption banks.
    pub inner_spacing: i32,
    /// Whether inactive captions remain visible and hittable, zero or one.
    pub show_without_activation: u8,
    /// Body height retained when minimized.
    pub minimized_content_height: i32,
    /// Whether the active title has a flat backdrop, zero or one.
    pub has_backdrop: u8,
    /// Backdrop color, packed R | G<<8 | B<<16 | A<<24.
    pub backdrop_rgba: u32,
    /// Additional pixels on each side of backdrop text.
    pub backdrop_padding: i32,
}

#[com_enum(tag = u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
/// Resolved nine-patch artwork; colors use the renderer's packed RGBA convention.
pub enum AssetPatch {
    /// Nine independent cells; alpha zero means no visible cell.
    Flat {
        /// Top-left corner color.
        top_left: u32,
        /// Top edge color.
        top: u32,
        /// Top-right corner color.
        top_right: u32,
        /// Left edge color.
        left: u32,
        /// Center color.
        center: u32,
        /// Right edge color.
        right: u32,
        /// Bottom-left corner color.
        bottom_left: u32,
        /// Bottom edge color.
        bottom: u32,
        /// Bottom-right corner color.
        bottom_right: u32,
    },
    /// One bundle-local icon sliced into nine source cells.
    Image {
        /// Index in this bundle's icon table, never a native texture handle.
        icon: u32,
        /// Source-space slicing thickness.
        source: Insets,
        /// Color modulation applied to every sampled cell.
        tint: u32,
    },
}

#[com_struct]
/// One resolved state value; geometry and content color are transferred together.
pub struct AssetVisual {
    /// Destination-space slicing thickness.
    pub insets: Insets,
    /// Whether to include the stretchable center, zero or one.
    pub center_visible: u8,
    /// Text and semantic-icon color in packed RGBA order.
    pub content_color: u32,
    /// Validated flat cells or a bundle-local image reference.
    pub patch: AssetPatchAbi,
}

com_int_enum! {
    /// Family whose table is role-major, then state-major, using appearance::* ALL order.
    pub enum AppearanceFamily: u8 {
        /// SurfaceRole × SurfaceState values.
        SURFACE = 0,
        /// ControlRole × ControlState values.
        CONTROL = 1,
        /// MenuRole × MenuState values.
        MENU = 2,
        /// ChromeRole × ChromeState values.
        CHROME = 3,
    }
}

#[com_interface(auto_guid = "microui.AssetCatalog:v1")]
/// Logical resource namespace authenticated by canonical COM identity.
/// Derived bundles retain this capability and preserve the meaning of existing names.
/// Independently created catalogs remain unrelated even when their names match.
pub trait IAssetCatalog {}

#[com_interface(auto_guid = "microui.AssetBundle:v1")]
/// Immutable, self-contained resource revision. Every copy writes the exact requested range.
/// Names partition their byte table in font-then-icon order. Fonts require an underscore
/// fallback; body and the standard semantic icon names are mandatory. Consumers import
/// once, validating all records and pixels before publication. No per-glyph COM calls occur.
pub trait IAssetBundle {
    /// Retains the logical namespace shared by compatible derived revisions.
    fn catalog(&self) -> Result<ComPtr<IAssetCatalogAbi>, NcStatus>;
    /// Returns bounded allocation counts and dimensions.
    fn info(&self) -> Result<AssetInfo, NcStatus>;
    /// Copies tightly packed RGBA bytes in row-major order.
    fn pixels(&self, output: NcCopyToBufferOffset<'_, u8>) -> Result<(), NcStatus>;
    /// Copies contiguous font records.
    fn fonts(&self, output: NcCopyToBufferOffset<'_, AssetFont>) -> Result<(), NcStatus>;
    /// Copies contiguous glyph records.
    fn glyphs(&self, output: NcCopyToBufferOffset<'_, AssetGlyph>) -> Result<(), NcStatus>;
    /// Copies contiguous icon records.
    fn icons(&self, output: NcCopyToBufferOffset<'_, AssetIcon>) -> Result<(), NcStatus>;
    /// Copies the exact UTF-8 name pool without NUL terminators.
    fn names(&self, output: NcCopyToBufferOffset<'_, u8>) -> Result<(), NcStatus>;
    /// Returns complete scalar layout metrics.
    fn metrics(&self) -> Result<AssetMetrics, NcStatus>;
    /// Returns complete window chrome policy.
    fn chrome(&self) -> Result<AssetChrome, NcStatus>;
    /// Copies one complete resolved appearance family in the documented fixed order.
    fn visuals(&self, family: AppearanceFamily, output: NcCopyToBufferOffset<'_, AssetVisual>) -> Result<(), NcStatus>;
}

#[com_interface(auto_guid = "microui.AssetBuilder:v1")]
/// Optional native font/PNG construction session; all mutation uses concrete named operations.
pub trait IAssetBuilder {
    /// Adds a PNG icon under a distinct UTF-8 name and exact native path; byte offset is zero.
    fn add_icon(&self, name: nc_com::NcCopyFromBufferOffset<'_, u8>, path: ComRef<'_, crate::INativePathAbi>) -> Result<(), NcStatus>;
    /// Adds a distinct font name at a positive pixel size; the native builder bakes printable ASCII.
    fn add_font(&self, name: nc_com::NcCopyFromBufferOffset<'_, u8>, path: ComRef<'_, crate::INativePathAbi>, pixels: u32) -> Result<(), NcStatus>;
    /// Consumes the session even on validation failure. A successful result has all required
    /// semantic resources and the ordinary immutable bundle form; nested operations return BUSY.
    fn finish(&self) -> Result<ComPtr<IAssetBundleAbi>, NcStatus>;
}

#[com_interface(auto_guid = "microui.AssetLoader:v1")]
/// Optional CPU construction root; no font rasterizer or PNG decoder is required in the UI/host.
pub trait IAssetLoader {
    /// Starts a fresh atlas using an opaque-white PNG tile; dimensions are validated before reading files.
    fn create_builder(&self, size: Size, white: ComRef<'_, crate::INativePathAbi>) -> Result<ComPtr<IAssetBuilderAbi>, NcStatus>;
    /// Copies an existing bundle into a new packing area while retaining its catalog namespace.
    fn derive_builder(&self, size: Size, base: ComRef<'_, IAssetBundleAbi>) -> Result<ComPtr<IAssetBuilderAbi>, NcStatus>;
}

#[com_interface(auto_guid = "microui.AssetDiagnostics:v1")]
/// Owned diagnostic snapshots supplement recoverable asset-operation statuses.
pub trait IAssetDiagnostics {
    /// Returns the latest recorded native/decoder/compiler cause, initially empty; argument validation may add no detail.
    fn diagnostic(&self) -> Result<ComPtr<nc_com::INcStringAbi>, NcStatus>;
}

#[com_interface(auto_guid = "microui.ThemeLoader:v1")]
/// Optional JSON/font/PNG operation; compiled themes use exactly the ordinary bundle interface.
pub trait IThemeLoader {
    /// Resolves a native theme path against the base's catalog, preserving original resource names.
    fn load_theme(&self, path: ComRef<'_, crate::INativePathAbi>, base: ComRef<'_, IAssetBundleAbi>) -> Result<ComPtr<IAssetBundleAbi>, NcStatus>;
}

/// Activation identity for the optional theme compiler.
pub const THEME_LOADER_CLASS: nc_com::NcGuid = nc_com::guid_from_qualified_name("microui.ThemeLoader");
/// Activation identity for optional font and PNG construction.
pub const ASSET_LOADER_CLASS: nc_com::NcGuid = nc_com::guid_from_qualified_name("microui.AssetLoader");

#[com_enum(tag = u8)]
#[derive(Clone, Copy, Debug)]
/// Explicit choice between preserving authored artwork and replacing it with a flat recipe.
pub enum PaletteEdit {
    /// Keep the current role/state visuals, including image-backed theme artwork.
    Preserve,
    /// Compile this complete named palette through the UI's shared flat-style rules.
    Flat(crate::FlatPalette),
}

#[com_interface(auto_guid = "microui.UiStyle:v1")]
/// Observational authoring capability on the same UI runtime that owns the installed resource catalog.
pub trait IUiStyle {
    /// Derives an immutable resource revision from current assets without installing it or touching
    /// the renderer. Invalid geometry or palette tags leave the installed revision unchanged.
    /// The caller installs the result through the ordinary transactional install_assets boundary.
    fn derive_style(&self, metrics: AssetMetrics, palette: PaletteEditAbi) -> Result<ComPtr<IAssetBundleAbi>, NcStatus>;
}
