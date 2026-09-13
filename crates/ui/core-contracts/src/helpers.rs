//! Rust conveniences layered over the generated `neocogi.ui.core` ABI declarations.
//!
//! None of these helpers contribute to module identity or native contract metadata.

use crate::*;
use neocogi_com::{NC_E_INVALID_ARG, NcStatus};

impl Default for PaintFont {
    /// Returns the first font slot.
    fn default() -> Self {
        Self { index: 0 }
    }
}

impl Default for PaintIcon {
    /// Returns the first icon slot.
    fn default() -> Self {
        Self { index: 0 }
    }
}

impl Default for Point {
    /// Returns the coordinate-system origin.
    fn default() -> Self {
        Self { x: 0, y: 0 }
    }
}

impl Default for WidgetPaint {
    /// Returns empty inactive paint state.
    fn default() -> Self {
        Self {
            rect: Rect::default(),
            clip: Rect::default(),
            enabled: false,
            hovered: false,
            focused: false,
            clicked: false,
            active: false,
            window_active: false,
        }
    }
}

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
        // The generated type must own this standard-trait implementation under
        // Rust's coherence rules; no separate handwritten contract type exists.
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

impl Default for PaintPoint {
    /// Returns the floating-point origin.
    fn default() -> Self {
        Self { x: 0.0, y: 0.0 }
    }
}

impl Default for ColoredVertex {
    /// Returns a transparent vertex at the origin.
    fn default() -> Self {
        Self { x: 0.0, y: 0.0, rgba: 0 }
    }
}

/// An operation requires an object which has already been logically closed.
pub const UI_E_CLOSED: NcStatus = NcStatus::from_code(-1000);

/// An operation is forbidden in the current phase or would reenter a traversal.
pub const UI_E_BUSY: NcStatus = NcStatus::from_code(-1001);

/// The object/resource belongs to a different UI or renderer instance.
pub const UI_E_OWNER: NcStatus = NcStatus::from_code(-1002);

/// A retained paint/layout invocation has ended; its token never becomes valid again.
pub const UI_E_EXPIRED_SCOPE: NcStatus = NcStatus::from_code(-1003);

/// Painting requires update/layout to be synchronized after a mutation or resize.
pub const UI_E_UPDATE_REQUIRED: NcStatus = NcStatus::from_code(-1004);

/// Backend failure; diagnostics belong to the platform implementation.
pub const UI_E_RENDER: NcStatus = NcStatus::from_code(-1005);

/// A live surface rejects the requested structural, modal or managed-layer operation.
pub const UI_E_POLICY: NcStatus = NcStatus::from_code(-1006);

/// A bounded event transaction exhausted its callback budget; undispatched queued events remain pending.
pub const UI_E_EVENT_LIMIT: NcStatus = NcStatus::from_code(-1007);

/// A native file/directory operation failed; the component's diagnostic preserves the concrete OS error.
pub const UI_E_IO: NcStatus = NcStatus::from_code(-1008);

impl AxisLimit {
    /// Rejects malformed tags, negative bounds, and meaningless unbounded extents.
    pub fn validate(self) -> Result<(), NcStatus> {
        match self.kind {
            LimitKind::Finite if self.extent >= 0 => Ok(()),
            LimitKind::Unbounded if self.extent == 0 => Ok(()),
            _ => Err(neocogi_com::NC_E_INVALID_ARG),
        }
    }
}

impl WindowOptions {
    /// Validates the complete bit set at the component boundary.
    pub fn validate(self) -> Result<(), NcStatus> {
        if self.is_valid() { Ok(()) } else { Err(neocogi_com::NC_E_INVALID_ARG) }
    }
}

/// Bulk asset semantic schema, including role order and structured appearance interpretation.
/// Bump when role ordering, named appearance fields, or mandatory resource meaning changes.
pub const ASSET_SCHEMA_VERSION: u32 = 2;

/// Maximum uncompressed RGBA storage accepted by all asset consumers.
pub const MAX_ASSET_PIXELS_BYTES: usize = neocogi_renderer_contracts::MAX_IMAGE_BYTES;

/// Maximum glyph records in a complete bundle, across every font.
pub const MAX_ASSET_GLYPHS: u32 = 1024 * 1024;

/// Maximum distinct font names in one bundle.
pub const MAX_ASSET_FONTS: u32 = 4096;

/// Maximum distinct icon names in one bundle.
pub const MAX_ASSET_ICONS: u32 = 65536;

/// Maximum combined UTF-8 bytes occupied by resource names.
pub const MAX_ASSET_NAMES: u32 = 4 * 1024 * 1024;

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
        // Store the caller's exact value. Geometry normalization is intentionally deferred so
        // public skin mutation remains transparent and every consumer applies the same policy.
        Self {
            left: value,
            top: value,
            right: value,
            bottom: value,
        }
    }

    /// Creates independently configurable left, top, right, and bottom insets.
    pub const fn new(left: i32, top: i32, right: i32, bottom: i32) -> Self {
        // A plain constructor keeps theme deserialization and programmatic skin construction on
        // the same concrete representation without introducing builder-only mirror types.
        Self { left, top, right, bottom }
    }

    /// Returns an equivalent value whose four components are non-negative.
    pub fn normalized(self) -> Self {
        // Clamp once before either layout or painting arithmetic. This prevents the two paths from
        // disagreeing when an application installs a skin containing a negative inset.
        Self {
            left: self.left.max(0),
            top: self.top.max(0),
            right: self.right.max(0),
            bottom: self.bottom.max(0),
        }
    }

    /// Returns the combined horizontal inset using saturating arithmetic.
    pub fn horizontal_extent(self) -> i32 {
        // Normalization makes both operands non-negative; saturation still protects the public i32
        // style domain from overflowing while constraints are derived.
        let insets = self.normalized();
        insets.left.saturating_add(insets.right)
    }

    /// Returns the combined vertical inset using saturating arithmetic.
    pub fn vertical_extent(self) -> i32 {
        // Match horizontal extent policy so measurement cannot produce axis-dependent overflow
        // behavior for otherwise equivalent theme values.
        let insets = self.normalized();
        insets.top.saturating_add(insets.bottom)
    }

    /// Returns the widest component for scalar effects such as a focus outline.
    pub fn maximum_component(self) -> i32 {
        // The maximum is derived after normalization because a negative style component contributes
        // no visible or structural thickness anywhere else in the patch pipeline.
        let insets = self.normalized();
        insets.left.max(insets.top).max(insets.right).max(insets.bottom)
    }

    /// Raises every normalized component to at least `minimum`.
    pub fn at_least(self, minimum: i32) -> Self {
        // Active outlines use this to remain visible when an ordinary frame has zero thickness.
        // Normalize both inputs so the helper cannot reintroduce negative patch geometry.
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
    /// Returns the crate's neutral dark flat-skin recipe.
    fn default() -> Self {
        // Every value is named at construction, preventing positional palette indices from leaking
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

impl ContainerGeometry {
    /// Starts with the full content viewport, no translation and ordinary overflow propagation.
    pub fn new(rect: Rect) -> Self {
        Self {
            content: Size { width: rect.width, height: rect.height },
            viewport: rect,
            offset_x: 0,
            offset_y: 0,
            propagate_overflow: true,
        }
    }
    /// Rejects negative extents before the runtime publishes geometry.
    pub fn validate(self) -> Result<(), NcStatus> {
        if self.content.width < 0 || self.content.height < 0 || self.viewport.width < 0 || self.viewport.height < 0 {
            Err(neocogi_com::NC_E_INVALID_ARG)
        } else {
            Ok(())
        }
    }
}

impl ChildEditResult {
    /// Reports teardown only after the caller commits its matching relationship metadata.
    pub fn finish(self) -> Result<(), NcStatus> {
        if self.teardown.is_err() { Err(self.teardown) } else { Ok(()) }
    }
}

impl LinearEdge {
    /// Creates an ordinary relationship that follows the Linear line's resolved cross extent.
    pub fn new(main: LayoutTrack) -> Self {
        Self {
            main: main.into(),
            cross: ChildCrossSize::Stretch.into(),
        }
    }
    /// Requests an exact cross extent; the receiving container validates it before mutation.
    pub fn with_fixed_cross(mut self, extent: i32) -> Self {
        self.cross = ChildCrossSize::Fixed { extent }.into();
        self
    }
}

impl KeyStroke {
    /// Rejects unknown keys, invalid Unicode, and inconsistent modifier/repeat data.
    pub fn validate(self) -> Result<(), NcStatus> {
        if self.modifiers & !15 != 0 || (self.repeat && !self.pressed) {
            return Err(NC_E_INVALID_ARG);
        }
        let valid = match self.key {
            KeyCode::Character => char::from_u32(self.value).is_some(),
            KeyCode::Function => (1..=24).contains(&self.value),
            key if key.raw() <= KeyCode::Super.raw() => self.value == 0,
            _ => false,
        };
        if valid { Ok(()) } else { Err(NC_E_INVALID_ARG) }
    }
}

/// Decodes a tagged event and validates its matching whole-text argument.
/// The returned text borrow lasts only as long as the caller's original buffer.
pub fn decode_widget_event<'a>(event: WidgetEventAbi, text: neocogi_com::NcCopyFromBufferOffset<'a, u8>) -> Result<(WidgetEvent, &'a str), NcStatus> {
    let event = WidgetEvent::try_from(event)?;
    if text.offset() != 0 {
        return Err(NC_E_INVALID_ARG);
    }
    let text = core::str::from_utf8(text.as_slice()).map_err(|_| NC_E_INVALID_ARG)?;
    if !matches!(event, WidgetEvent::Text) && !text.is_empty() {
        return Err(NC_E_INVALID_ARG);
    }
    match event {
        WidgetEvent::Key(key) => key.validate()?,
        WidgetEvent::Down { buttons, .. } | WidgetEvent::Up { buttons, .. } | WidgetEvent::Drag { buttons, .. } if buttons & !7 != 0 => {
            return Err(NC_E_INVALID_ARG);
        }
        _ => {}
    }
    Ok((event, text))
}

impl WidgetUpdate {
    /// Validates copied geometry and held-state tags before semantic mutation.
    pub fn validate(self) -> Result<(), NcStatus> {
        if self.rect.x != 0
            || self.rect.y != 0
            || self.rect.width < 0
            || self.rect.height < 0
            || self.screen.width != self.rect.width
            || self.screen.height != self.rect.height
            || self.clip.width < 0
            || self.clip.height < 0
            || self.mouse_buttons & !7 != 0
            || self.modifiers & !15 != 0
            || self.action.raw() > WidgetAction::Collapse.raw()
        {
            return Err(NC_E_INVALID_ARG);
        }
        Ok(())
    }
}

/// Shared finite callback budget for one runtime dispatch boundary across all sources and cascade sweeps.
pub const EVENT_DISPATCH_LIMIT: u32 = 1_000_000;

/// Initial native-picker geometry shared by static request defaults and hidden plugin construction.
pub const FILE_DIALOG_RECT: crate::Rect = crate::Rect { x: 40, y: 40, width: 600, height: 400 };

impl WidgetOptions {
    /// Rejects undefined flags before callers commit a replacement.
    pub fn validate(self) -> Result<(), NcStatus> {
        if self.is_valid() { Ok(()) } else { Err(NC_E_INVALID_ARG) }
    }
}

impl WidgetProperties {
    /// Rejects unsupported routing/appearance values at the component boundary.
    pub fn validate(self) -> Result<(), NcStatus> {
        self.options.validate()?;
        if self.keyboard & !511 != 0 || self.frame.raw() > 16 {
            return Err(NC_E_INVALID_ARG);
        }
        Ok(())
    }
}

impl WidgetPaint {
    /// Rejects malformed local geometry before a component records any operations.
    pub fn validate(self) -> Result<(), NcStatus> {
        if self.rect.x != 0 || self.rect.y != 0 || self.rect.width < 0 || self.rect.height < 0 || self.clip.width < 0 || self.clip.height < 0 {
            return Err(NC_E_INVALID_ARG);
        }
        Ok(())
    }
}
