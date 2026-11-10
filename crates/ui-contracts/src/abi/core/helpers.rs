//! Rust conveniences layered over the generated `neocogi.ui.core` ABI declarations.
//!
//! None of these helpers contribute to module identity or native contract metadata.

use crate::abi::core::*;
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

/// Maximum UTF-8 bytes in a retained window, dialog, child, or popup title.
///
/// Chrome measures the complete title during layout and paint, before drawing operations are
/// recorded. Bounding source text at mutation prevents uncharged glyph walks in those phases.
pub const MAX_WINDOW_TITLE_BYTES: usize = 4096;

/// Borrows a complete title after offset, byte-bound and UTF-8 admission, in that order.
/// The caller retains the original storage; this helper neither allocates nor changes UI state.
/// Window setters and composite requests share this policy before any publication or callback.
pub fn validate_window_title<'a>(title: neocogi_com::NcCopyFromBufferOffset<'a, u8>) -> Result<&'a str, NcStatus> {
    if title.offset() != 0 {
        return Err(NC_E_INVALID_ARG);
    }
    if title.as_slice().len() > MAX_WINDOW_TITLE_BYTES {
        return Err(neocogi_com::NC_E_OUT_OF_RANGE);
    }
    core::str::from_utf8(title.as_slice()).map_err(|_| NC_E_INVALID_ARG)
}

/// A runtime, window, collection or transport service has closed.
/// Its COM capability may still exist. Calls through a widget observer whose
/// exclusive owner was dropped instead report `neocogi_com::NC_E_DROPPED`.
pub const UI_E_CLOSED: NcStatus = neocogi_com::NC_E_CLOSED;

/// An operation is forbidden in the current phase or would reenter a traversal.
pub const UI_E_BUSY: NcStatus = NcStatus::from_code(-1001);

/// The object/resource belongs to a different UI or renderer instance.
pub const UI_E_OWNER: NcStatus = NcStatus::from_code(-1002);

/// A retained paint/layout invocation has ended; its token never becomes valid again.
pub const UI_E_EXPIRED_SCOPE: NcStatus = NcStatus::from_code(-1003);

/// Painting requires update/layout to be synchronized after a mutation or resize.
pub const UI_E_UPDATE_REQUIRED: NcStatus = NcStatus::from_code(-1004);

/// Backend failure; the operation's returned `RenderError` owns its status and detail.
pub const UI_E_RENDER: NcStatus = NcStatus::from_code(-1005);

/// A live surface rejects the requested structural, modal or managed-layer operation.
pub const UI_E_POLICY: NcStatus = NcStatus::from_code(-1006);

/// A UI update exhausted its finite work budget before establishing quiescence.
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

/// Validates a native event and its matching whole-text argument.
/// The COM codec has already checked the wire tag and active field bit patterns.
/// The returned text borrow lasts only as long as the caller's original buffer.
pub fn decode_widget_event<'a>(event: WidgetEvent, text: neocogi_com::NcCopyFromBufferOffset<'a, u8>) -> Result<(WidgetEvent, &'a str), NcStatus> {
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

/// Shared work budget across one outer update: private/application dispatch,
/// reconciliation and consumed input scopes with application bookkeeping.
pub const EVENT_DISPATCH_LIMIT: u32 = 1_000_000;

/// Initial native-picker geometry shared by static request defaults and hidden plugin construction.
pub const FILE_DIALOG_RECT: crate::abi::core::Rect = crate::abi::core::Rect { x: 40, y: 40, width: 600, height: 400 };

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
        if self.keyboard & !1023 != 0 || self.frame.raw() > 16 {
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

#[cfg(test)]
mod title_tests {
    use super::*;
    use neocogi_com::NcCopyFromBufferOffset;

    /// Length admission counts UTF-8 bytes, not characters, and precedes scanning.
    #[test]
    fn title_byte_bound_and_error_precedence_are_shared() {
        let exact = "é".repeat(MAX_WINDOW_TITLE_BYTES / 2);
        assert_eq!(validate_window_title(NcCopyFromBufferOffset::new(0, exact.as_bytes())), Ok(exact.as_str()));
        assert_eq!(validate_window_title(NcCopyFromBufferOffset::new(0, b"")), Ok(""));
        for bytes in [vec![b'x'; MAX_WINDOW_TITLE_BYTES + 1], vec![255; MAX_WINDOW_TITLE_BYTES + 1]] {
            assert_eq!(
                validate_window_title(NcCopyFromBufferOffset::new(0, &bytes)),
                Err(neocogi_com::NC_E_OUT_OF_RANGE)
            );
            assert_eq!(validate_window_title(NcCopyFromBufferOffset::new(1, &bytes)), Err(NC_E_INVALID_ARG));
        }
        assert_eq!(validate_window_title(NcCopyFromBufferOffset::new(0, &[255])), Err(NC_E_INVALID_ARG));
    }
}
