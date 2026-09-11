//! Rust conveniences layered over the generated `neocogi.ui` ABI declarations.
//!
//! None of these helpers contribute to module identity or native contract metadata.

use crate::*;
use neocogi_com::{NC_E_INVALID_ARG, NcStatus};
use rs_math3d::{Color4b, Vec2f, color4b};

impl Default for Size {
    /// Returns zero width and height for caller-owned output initialization.
    fn default() -> Self {
        Self { width: 0, height: 0 }
    }
}

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

impl Default for Rect {
    /// Returns an empty rectangle at the local origin.
    fn default() -> Self {
        Self { x: 0, y: 0, width: 0, height: 0 }
    }
}

impl Default for Color {
    /// Returns transparent black.
    fn default() -> Self {
        Self { r: 0, g: 0, b: 0, a: 0 }
    }
}

impl Default for Point {
    /// Returns the coordinate-system origin.
    fn default() -> Self {
        Self { x: 0, y: 0 }
    }
}

impl Default for WidgetOptions {
    /// Returns an empty option bit set.
    fn default() -> Self {
        Self::NONE
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

impl Default for RenderVertex {
    /// Returns a transparent textured vertex at the origin.
    fn default() -> Self {
        Self { x: 0.0, y: 0.0, u: 0.0, v: 0.0, rgba: 0 }
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
            LimitKind::FINITE if self.extent >= 0 => Ok(()),
            LimitKind::UNBOUNDED if self.extent == 0 => Ok(()),
            _ => Err(neocogi_com::NC_E_INVALID_ARG),
        }
    }
}

impl WindowOptions {
    /// No optional presentation behavior.
    pub const NONE: Self = Self { bits: 0 };
    /// Show the minimize caption button.
    pub const MINIMIZE_BUTTON: Self = Self { bits: 1 };
    /// Show the maximize caption button.
    pub const MAXIMIZE_BUTTON: Self = Self { bits: 2 };
    /// Keep visible while disabling the complete structural family.
    pub const DISABLED: Self = Self { bits: 4 };
    /// Remove the ordinary content inset.
    pub const NO_PADDING: Self = Self { bits: 8 };
    /// Prevent interactive resizing.
    pub const NO_RESIZE: Self = Self { bits: 16 };
    /// Hide the close caption button.
    pub const NO_CLOSE: Self = Self { bits: 64 };
    /// Hide the title bar.
    pub const NO_TITLE: Self = Self { bits: 128 };
    /// Derive width from preferred content measurement.
    pub const AUTO_WIDTH: Self = Self { bits: 256 };
    /// Derive height from preferred content measurement.
    pub const AUTO_HEIGHT: Self = Self { bits: 512 };
    /// Derive both dimensions from preferred content measurement.
    pub const AUTO_SIZE: Self = Self { bits: 768 };
    /// Use the theme-owned outer border and content inset.
    pub const FRAME: Self = Self { bits: 1024 };

    /// Every declared presentation bit; aliases such as AUTO_SIZE introduce no new bits.
    const KNOWN_BITS: u32 = Self::MINIMIZE_BUTTON.bits
        | Self::MAXIMIZE_BUTTON.bits
        | Self::DISABLED.bits
        | Self::NO_PADDING.bits
        | Self::NO_RESIZE.bits
        | Self::NO_CLOSE.bits
        | Self::NO_TITLE.bits
        | Self::AUTO_WIDTH.bits
        | Self::AUTO_HEIGHT.bits
        | Self::FRAME.bits;

    /// Validates the complete bit set at the component boundary.
    pub fn validate(self) -> Result<(), NcStatus> {
        if self.bits & !Self::KNOWN_BITS == 0 {
            Ok(())
        } else {
            Err(neocogi_com::NC_E_INVALID_ARG)
        }
    }
}

impl core::ops::BitOr for WindowOptions {
    type Output = Self;
    /// Combines concrete options without exposing a typeless property collection.
    fn bitor(self, rhs: Self) -> Self {
        Self { bits: self.bits | rhs.bits }
    }
}

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

/// Convenience constructor for [`Color`].
pub fn color(r: u8, g: u8, b: u8, a: u8) -> Color {
    Color { r, g, b, a }
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
            KeyCode::CHARACTER => char::from_u32(self.value).is_some(),
            KeyCode::FUNCTION => (1..=24).contains(&self.value),
            key if key.raw() <= KeyCode::SUPER.raw() => self.value == 0,
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
            || self.action.raw() > WidgetAction::COLLAPSE.raw()
        {
            return Err(NC_E_INVALID_ARG);
        }
        Ok(())
    }
}

/// Shared allocation ceiling for one complete frame, checked before reserving receiver storage.
pub const MAX_RENDER_VERTICES: u32 = 16 * 1024 * 1024;

/// Shared ceiling for ordered records; adjacent atlas triangles should occupy one batch.
pub const MAX_RENDER_BATCHES: u32 = 1024 * 1024;

impl RenderVertex {
    /// Creates one canonical renderer and COM transport vertex.
    pub fn new(position: Vec2f, tex_coord: Vec2f, color: Color4b) -> Self {
        Self {
            x: position.x,
            y: position.y,
            u: tex_coord.x,
            v: tex_coord.y,
            rgba: u32::from_le_bytes([color.x, color.y, color.z, color.w]),
        }
    }

    /// Returns the screen-space position used by clipping and native backends.
    pub fn position(&self) -> Vec2f {
        Vec2f::new(self.x, self.y)
    }

    /// Returns the normalized atlas or image coordinates.
    pub fn tex_coord(&self) -> Vec2f {
        Vec2f::new(self.u, self.v)
    }

    /// Decodes the canonical packed color into renderer channel storage.
    pub fn color(&self) -> Color4b {
        let [r, g, b, a] = self.rgba.to_le_bytes();
        color4b(r, g, b, a)
    }

    /// Linearly interpolates every attribute for geometry clipping.
    pub fn lerp(left: Self, right: Self, amount: f32) -> Self {
        // Clamp the intersection amount so numerical noise cannot extrapolate
        // a vertex beyond the caller's validated segment.
        let amount = amount.clamp(0.0, 1.0);
        let left_color = left.rgba.to_le_bytes();
        let right_color = right.rgba.to_le_bytes();
        Self {
            x: left.x + (right.x - left.x) * amount,
            y: left.y + (right.y - left.y) * amount,
            u: left.u + (right.u - left.u) * amount,
            v: left.v + (right.v - left.v) * amount,
            rgba: u32::from_le_bytes([
                Self::lerp_channel(left_color[0], right_color[0], amount),
                Self::lerp_channel(left_color[1], right_color[1], amount),
                Self::lerp_channel(left_color[2], right_color[2], amount),
                Self::lerp_channel(left_color[3], right_color[3], amount),
            ]),
        }
    }

    /// Interpolates one packed channel and rounds it back into byte storage.
    fn lerp_channel(left: u8, right: u8, amount: f32) -> u8 {
        // Inputs and the clamped interpolation amount keep the result non-negative,
        // so adding one half before integer truncation implements nearest rounding
        // without relying on the `std`-only floating-point `round` method.
        let interpolated = (left as f32) + (right as f32 - left as f32) * amount;
        (interpolated + 0.5).clamp(0.0, 255.0) as u8
    }
}

impl TextureRegion {
    /// Complete image, also used by the ordinary Painter::texture convenience.
    pub const FULL: Self = Self { u0: 0., v0: 0., u1: 1., v1: 1. };
    /// Rejects nonfinite, reversed and out-of-image coordinates before retaining a resource.
    pub fn validate(self) -> Result<(), neocogi_com::NcStatus> {
        if [self.u0, self.v0, self.u1, self.v1].iter().all(|v| v.is_finite() && (0. ..=1.).contains(v)) && self.u0 <= self.u1 && self.v0 <= self.v1 {
            Ok(())
        } else {
            Err(neocogi_com::NC_E_INVALID_ARG)
        }
    }
}

impl FillStates {
    /// No center fill.
    pub const NONE: Self = Self { bits: 0 };
    /// Idle or disabled center fill.
    pub const NORMAL: Self = Self { bits: 1 };
    /// Hovered center fill.
    pub const HOVER: Self = Self { bits: 2 };
    /// Focused or pressed center fill.
    pub const FOCUS: Self = Self { bits: 4 };
    /// Center fill in every interaction state.
    pub const ALL: Self = Self { bits: 7 };
}

/// Shared finite callback budget for one runtime dispatch boundary across all sources and cascade sweeps.
pub const EVENT_DISPATCH_LIMIT: u32 = 1_000_000;

/// Initial native-picker geometry shared by static request defaults and hidden plugin construction.
pub const FILE_DIALOG_RECT: crate::Rect = crate::Rect { x: 40, y: 40, width: 600, height: 400 };

impl WidgetOptions {
    /// No optional frame or routing override.
    pub const NONE: Self = Self { bits: 0 };
    /// Centers content within its allocation.
    pub const ALIGN_CENTER: Self = Self { bits: 1 };
    /// Aligns content with the right edge.
    pub const ALIGN_RIGHT: Self = Self { bits: 2 };
    /// Makes only this surface inert; enabled descendants may still receive input.
    pub const NO_INTERACT: Self = Self { bits: 4 };
    /// Consumes hovered wheel input.
    pub const GRAB_SCROLL: Self = Self { bits: 32 };
    /// Requests the theme-owned outer frame for the widget's semantic role.
    pub const FRAME: Self = Self { bits: 512 };

    /// Rejects undefined flags before callers commit a replacement.
    pub fn validate(self) -> Result<(), NcStatus> {
        if self.bits & !(1 | 2 | 4 | 32 | 512) == 0 {
            Ok(())
        } else {
            Err(NC_E_INVALID_ARG)
        }
    }
}

impl core::ops::BitOr for WidgetOptions {
    type Output = Self;
    /// Combines independent flags without hiding a stored closure or policy object.
    fn bitor(self, other: Self) -> Self {
        Self { bits: self.bits | other.bits }
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
