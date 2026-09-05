//! Copied input contracts shared by platform hosts and concrete widgets.
//!
//! Text stays a separate bounded UTF-8 argument so a complete committed string
//! crosses once without allocating a COM object for every input event.
use nc_com::{com_enum, com_int_enum, com_struct, NcStatus, NC_E_INVALID_ARG};
use crate::Rect;

#[com_struct]
#[derive(Debug, Default, Eq, PartialEq)]
/// Integer point or delta in the receiving widget's local coordinates.
pub struct Point {
    /// Horizontal coordinate or displacement.
    pub x: i32,
    /// Vertical coordinate or displacement.
    pub y: i32,
}

com_int_enum! {
    /// Logical keyboard identity, independent of native scan codes.
    pub enum KeyCode: u8 {
        /// Printable shortcut key; its scalar is carried in `KeyStroke::value`.
        CHARACTER = 0,
        /// Removes the previous character.
        BACKSPACE = 1,
        /// Removes the next character.
        DELETE = 2,
        /// Primary activation or line break.
        ENTER = 3,
        /// Cancellation or popup dismissal.
        ESCAPE = 4,
        /// Space-bar activation, separate from committed text.
        SPACE = 5,
        /// Sequential focus traversal.
        TAB = 6,
        /// Insert editing command.
        INSERT = 7,
        /// Beginning of content or line.
        HOME = 8,
        /// End of content or line.
        END = 9,
        /// Previous viewport.
        PAGE_UP = 10,
        /// Next viewport.
        PAGE_DOWN = 11,
        /// Upward navigation.
        ARROW_UP = 12,
        /// Downward navigation.
        ARROW_DOWN = 13,
        /// Leftward navigation.
        ARROW_LEFT = 14,
        /// Rightward navigation.
        ARROW_RIGHT = 15,
        /// Numbered function key; its number is carried in `KeyStroke::value`.
        FUNCTION = 16,
        /// Alt/Option itself, for distinguishing a tap from a chord.
        ALT = 17,
        /// Control modifier itself.
        CONTROL = 18,
        /// Shift modifier itself.
        SHIFT = 19,
        /// Command/Windows/Super modifier itself.
        SUPER = 20,
    }
}

#[com_struct]
#[derive(Debug, Eq, PartialEq)]
/// Complete logical key transition; text composition uses `WidgetEvent::Text`.
pub struct KeyStroke {
    /// Stable logical key family.
    pub key: KeyCode,
    /// Unicode scalar for CHARACTER, function number for FUNCTION, otherwise zero.
    pub value: u32,
    /// True for a press, false for a release.
    pub pressed: bool,
    /// True only for a platform-generated repeated press.
    pub repeat: bool,
    /// Complete modifier bits: Alt=1, Ctrl=2, Shift=4, Super=8.
    pub modifiers: u8,
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

#[com_enum(tag = u8)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
/// One routed input transaction; pointer payloads are already content-local.
pub enum WidgetEvent {
    /// Ordinary eventless update, including animation or state synchronization.
    None,
    /// Native focus or capture was lost.
    Cancel,
    /// Pointer motion without held buttons.
    Move {
        /// Current local position.
        position: Point,
        /// Change since the previous queued position.
        delta: Point,
    },
    /// Pointer motion under capture or with held buttons.
    Drag {
        /// Current local position.
        position: Point,
        /// Change since the previous queued position.
        delta: Point,
        /// Held buttons: left=1, right=2, middle=4.
        buttons: u8,
    },
    /// Pointer button press.
    Down {
        /// Local press position.
        position: Point,
        /// Buttons changed by this transition.
        buttons: u8,
    },
    /// Pointer button release.
    Up {
        /// Local release position.
        position: Point,
        /// Buttons changed by this transition.
        buttons: u8,
    },
    /// Wheel or equivalent scrolling input.
    Scroll {
        /// Current local pointer position.
        position: Point,
        /// Requested scroll amount.
        delta: Point,
    },
    /// One complete logical keyboard transition.
    Key(KeyStroke),
    /// The accompanying bounded UTF-8 argument carries the complete committed text.
    Text,
}

#[com_struct]
#[derive(Debug, Eq, PartialEq)]
/// Router-owned geometry and interaction snapshot for one widget update.
pub struct WidgetUpdate {
    /// Whether pointer hit tests belong to the currently hovered logical root.
    pub in_hover_root: bool,
    /// Content allocation with local origin (0, 0).
    pub rect: Rect,
    /// Effective content-local clip.
    pub clip: Rect,
    /// Content allocation in native drawable coordinates, for input-method placement.
    pub screen: Rect,
    /// Pointer currently hovers this surface.
    pub hovered: bool,
    /// This surface owns logical keyboard focus.
    pub focused: bool,
    /// An eligible primary-button press selected this surface in this transaction.
    pub clicked: bool,
    /// This surface owns pointer capture while the primary button is held.
    pub active: bool,
    /// Shared action derived from the routed event and declared keyboard capabilities.
    pub action: WidgetAction,
    /// Current held mouse buttons using the same bits as pointer event payloads.
    pub mouse_buttons: u8,
    /// Complete current modifier snapshot using KeyStroke's bit assignments.
    pub modifiers: u8,
}

com_int_enum! {
    /// Shared semantic keyboard/pointer action resolved by the runtime's routing rules.
    pub enum WidgetAction: u8 {
        /// No semantic action was requested.
        NONE = 0,
        /// Invoke the primary command.
        ACTIVATE = 1,
        /// Move one step toward the lower value.
        DECREASE = 2,
        /// Move one step toward the higher value.
        INCREASE = 3,
        /// Reveal a hierarchical branch or popup.
        EXPAND = 4,
        /// Hide a hierarchical branch or popup.
        COLLAPSE = 5,
    }
}

/// Decodes a tagged event and validates its matching whole-text argument.
/// The returned text borrow lasts only as long as the caller's original buffer.
pub fn decode_widget_event<'a>(event: WidgetEventAbi, text: nc_com::NcCopyFromBufferOffset<'a, u8>) -> Result<(WidgetEvent, &'a str), NcStatus> {
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

#[com_enum(tag = u8)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
/// Ordered host input before the runtime computes drag, hover and local coordinates.
pub enum UiInput {
    /// Native focus or capture was lost; clears held state in input order.
    Cancel,
    /// Absolute pointer position in drawable coordinates.
    Pointer(Point),
    /// Button press at an absolute drawable position.
    Down {
        /// Current drawable position.
        position: Point,
        /// Buttons changed by this transition.
        buttons: u8,
    },
    /// Button release at an absolute drawable position.
    Up {
        /// Current drawable position.
        position: Point,
        /// Buttons changed by this transition.
        buttons: u8,
    },
    /// Wheel delta at the last committed pointer position.
    Scroll(Point),
    /// Complete key identity, state and modifiers.
    Key(KeyStroke),
    /// Complete UTF-8 commit supplied in the accompanying bounded text argument.
    Text,
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
