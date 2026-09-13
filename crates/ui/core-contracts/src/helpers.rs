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
