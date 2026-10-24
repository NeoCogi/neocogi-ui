//! Rust conveniences layered over the generated `neocogi.gfx` ABI declarations.
//!
//! Defaults and validation policy live with the types they describe so lower
//! layers never depend on the retained UI contract for resource semantics.

use crate::abi::*;
use neocogi_com::NcStatus;
use neocogi_render_core::abi::{Color, Rect, Size};

/// A GFX construction session was consumed and cannot accept more edits.
pub const GFX_E_CLOSED: NcStatus = NcStatus::from_code(-1000);

/// A GFX operation overlaps or recursively reenters an active transaction.
pub const GFX_E_BUSY: NcStatus = NcStatus::from_code(-1001);

impl Default for AssetRange {
    /// Returns an empty range at the start of its table.
    fn default() -> Self {
        Self { first: 0, count: 0 }
    }
}

impl Default for AssetInfo {
    /// Returns an empty, invalid resource manifest suitable for output storage.
    fn default() -> Self {
        Self {
            schema: 0,
            size: Size::default(),
            fonts: 0,
            icons: 0,
            glyphs: 0,
            names: 0,
        }
    }
}

impl Default for AssetFont {
    /// Returns an empty font record suitable for caller-owned output storage.
    fn default() -> Self {
        Self {
            name: AssetRange::default(),
            glyphs: AssetRange::default(),
            line_size: 0,
            baseline: 0,
            font_size: 0,
        }
    }
}

impl Default for AssetGlyph {
    /// Returns an empty glyph record suitable for caller-owned output storage.
    fn default() -> Self {
        Self {
            scalar: 0,
            offset_x: 0,
            offset_y: 0,
            advance_x: 0,
            advance_y: 0,
            rect: Rect::default(),
        }
    }
}

impl Default for AssetIcon {
    /// Returns an empty icon record suitable for caller-owned output storage.
    fn default() -> Self {
        Self {
            name: AssetRange::default(),
            rect: Rect::default(),
        }
    }
}

impl Default for AssetMetrics {
    /// Returns the standard layout metrics for an authored or installed skin.
    fn default() -> Self {
        // The generated type owns this standard-trait implementation under
        // Rust's coherence rules; no separate handwritten mirror type exists.
        Self {
            default_cell_width: 68,
            padding: 5,
            window_content_insets: Insets::uniform(5),
            spacing: 4,
            indent: 24,
            title_height: 24,
            window_border: Insets::uniform(1),
            scrollbar_size: 12,
            thumb_size: 8,
        }
    }
}

impl Default for Insets {
    /// Returns zero thickness on every edge.
    fn default() -> Self {
        Self::ZERO
    }
}

/// Bulk asset semantic schema, including role order and structured appearance interpretation.
/// Bump when role ordering, named appearance fields, or mandatory resource meaning changes.
/// Schema 3 permits oversized nonnegative image slices and requires shrink diagnostics.
pub const ASSET_SCHEMA_VERSION: u32 = 3;

/// Maximum uncompressed RGBA storage accepted by all asset consumers.
pub const MAX_ASSET_PIXELS_BYTES: usize = neocogi_render_core::abi::MAX_IMAGE_BYTES;

/// Maximum glyph records in a complete bundle, across every font.
pub const MAX_ASSET_GLYPHS: u32 = 1024 * 1024;

/// Maximum named assets in one bundle, counting font and icon records together.
/// Glyphs belong to fonts and retain their separate `MAX_ASSET_GLYPHS` storage bound.
pub const MAX_ASSETS: u32 = 16 * 1024;

/// Maximum UTF-8 bytes in each nonempty font or icon name, without a terminator.
/// The bound counts bytes rather than Unicode characters.
pub const MAX_ASSET_NAME_BYTES: u32 = 2048;

/// Maximum combined name storage, derived from the record count and per-name limits.
/// Builders enforcing both admission limits cannot exceed this 32 MiB bound at finish.
pub const MAX_ASSET_NAMES: u32 = MAX_ASSETS * MAX_ASSET_NAME_BYTES;

impl AssetInfo {
    /// Checks the shared allocation policy before any consumer reserves or decodes storage.
    pub fn rgba_bytes(self) -> Result<usize, NcStatus> {
        let assets = self
            .fonts
            .checked_add(self.icons)
            .filter(|&count| count <= MAX_ASSETS)
            .ok_or(neocogi_com::NC_E_INVALID_ARG)?;
        if self.schema != ASSET_SCHEMA_VERSION
            || self.size.width <= 0
            || self.size.height <= 0
            || self.fonts == 0
            || self.icons == 0
            || self.glyphs > MAX_ASSET_GLYPHS
            // Check the actual declared count before a reader allocates the name byte table.
            || self.names > assets * MAX_ASSET_NAME_BYTES
        {
            return Err(neocogi_com::NC_E_INVALID_ARG);
        }
        let bytes = (self.size.width as usize)
            .checked_mul(self.size.height as usize)
            .and_then(|pixels| pixels.checked_mul(4))
            .ok_or(neocogi_com::NC_E_INVALID_ARG)?;
        if bytes > MAX_ASSET_PIXELS_BYTES {
            return Err(neocogi_com::NC_E_INVALID_ARG);
        }
        Ok(bytes)
    }
}

impl Insets {
    /// Insets that leave the complete destination to the center cell.
    pub const ZERO: Self = Self::uniform(0);

    /// Creates four equal insets from one convenient scalar value.
    pub const fn uniform(value: i32) -> Self {
        // Store the exact value. Geometry normalization is deferred so public
        // skin mutation is transparent and every consumer applies one policy.
        Self {
            left: value,
            top: value,
            right: value,
            bottom: value,
        }
    }

    /// Creates independently configurable left, top, right, and bottom insets.
    pub const fn new(left: i32, top: i32, right: i32, bottom: i32) -> Self {
        // A plain constructor keeps JSON and programmatic construction on the
        // same concrete representation without a builder-only mirror type.
        Self { left, top, right, bottom }
    }

    /// Returns an equivalent value whose four components are non-negative.
    pub fn normalized(self) -> Self {
        // Clamp before layout or painting so malformed authored inputs cannot
        // create different geometry policy in the two consumers.
        Self {
            left: self.left.max(0),
            top: self.top.max(0),
            right: self.right.max(0),
            bottom: self.bottom.max(0),
        }
    }

    /// Returns the combined horizontal inset using saturating arithmetic.
    pub fn horizontal_extent(self) -> i32 {
        // Saturation keeps the public i32 style domain from overflowing while
        // layout constraints are derived.
        let insets = self.normalized();
        insets.left.saturating_add(insets.right)
    }

    /// Returns the combined vertical inset using saturating arithmetic.
    pub fn vertical_extent(self) -> i32 {
        // Match horizontal policy so equivalent axis inputs behave uniformly.
        let insets = self.normalized();
        insets.top.saturating_add(insets.bottom)
    }

    /// Returns the widest component for scalar effects such as a focus outline.
    pub fn maximum_component(self) -> i32 {
        // Negative style components contribute no visible or structural width.
        let insets = self.normalized();
        insets.left.max(insets.top).max(insets.right).max(insets.bottom)
    }

    /// Raises every normalized component to at least `minimum`.
    pub fn at_least(self, minimum: i32) -> Self {
        // Active outlines use this to remain visible when an ordinary frame has
        // zero width. Normalizing the minimum prevents negative geometry.
        let insets = self.normalized();
        let minimum = minimum.max(0);
        Self {
            left: insets.left.max(minimum),
            top: insets.top.max(minimum),
            right: insets.right.max(minimum),
            bottom: insets.bottom.max(minimum),
        }
    }
}

impl Default for FlatPalette {
    /// Returns the standard neutral dark flat-skin recipe.
    fn default() -> Self {
        // Named construction prevents positional palette indices from leaking
        // into loaders, editors, or widget paint code.
        Self {
            text: Color { r: 230, g: 230, b: 230, a: 255 },
            border: Color { r: 25, g: 25, b: 25, a: 255 },
            window_background: Color { r: 50, g: 50, b: 50, a: 255 },
            title_background: Color { r: 25, g: 25, b: 25, a: 255 },
            title_foreground: Color { r: 240, g: 240, b: 240, a: 255 },
            panel_background: Color { r: 0, g: 0, b: 0, a: 0 },
            button: Color { r: 75, g: 75, b: 75, a: 255 },
            button_hovered: Color { r: 95, g: 95, b: 95, a: 255 },
            input: Color { r: 30, g: 30, b: 30, a: 255 },
            input_hovered: Color { r: 35, g: 35, b: 35, a: 255 },
            scrollbar_track: Color { r: 43, g: 43, b: 43, a: 255 },
            scrollbar_thumb: Color { r: 30, g: 30, b: 30, a: 255 },
            control_focus: Color { r: 0, g: 120, b: 215, a: 255 },
            selection_background: Color { r: 0, g: 120, b: 215, a: 255 },
            selection_foreground: Color { r: 255, g: 255, b: 255, a: 255 },
            window_active: Color { r: 0, g: 120, b: 215, a: 255 },
            menu_foreground: Color { r: 230, g: 230, b: 230, a: 255 },
            menu_background: Color { r: 50, g: 50, b: 50, a: 255 },
            disabled_background: Color { r: 50, g: 50, b: 50, a: 255 },
            disabled_foreground: Color { r: 230, g: 230, b: 230, a: 255 },
            disabled_title_foreground: Color { r: 240, g: 240, b: 240, a: 255 },
        }
    }
}

#[cfg(test)]
mod limit_tests {
    use super::*;
    use neocogi_com::NC_E_INVALID_ARG;

    /// These metadata headers exercise preallocation admission independently of table contents.
    fn header(fonts: u32, icons: u32, names: u32) -> AssetInfo {
        AssetInfo {
            schema: ASSET_SCHEMA_VERSION,
            size: Size { width: 1, height: 1 },
            fonts,
            icons,
            glyphs: 0,
            names,
        }
    }

    /// Both resource types share the same count, including exact and overflow boundaries.
    #[test]
    fn combined_asset_count_rejects_overflow_and_accepts_the_exact_limit() {
        for fonts in [1, 4097, MAX_ASSETS / 2, MAX_ASSETS - 1] {
            let icons = MAX_ASSETS - fonts;
            assert_eq!(header(fonts, icons, MAX_ASSET_NAMES).rgba_bytes(), Ok(4));
            assert_eq!(header(fonts, icons + 1, 0).rgba_bytes(), Err(NC_E_INVALID_ARG));
        }
        assert_eq!(header(u32::MAX, 1, 0).rgba_bytes(), Err(NC_E_INVALID_ARG));
        assert_eq!(header(1, u32::MAX, 0).rgba_bytes(), Err(NC_E_INVALID_ARG));
        assert_eq!(header(0, 1, 0).rgba_bytes(), Err(NC_E_INVALID_ARG));
        assert_eq!(header(1, 0, 0).rgba_bytes(), Err(NC_E_INVALID_ARG));
    }

    /// A forged large name pool must fail before copying even when below the global 32 MiB maximum.
    #[test]
    fn name_pool_storage_is_derived_from_the_actual_declared_asset_count() {
        for assets in [2, 10, MAX_ASSETS] {
            let names = assets * MAX_ASSET_NAME_BYTES;
            assert_eq!(header(1, assets - 1, names).rgba_bytes(), Ok(4));
            assert_eq!(header(1, assets - 1, names + 1).rgba_bytes(), Err(NC_E_INVALID_ARG));
        }
    }
}
