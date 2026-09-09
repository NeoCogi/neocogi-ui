//! Explicit conversion from the retained router to copied COM input contracts.
use crate::com::*;
use microui_contracts::*;
use nc_com::{NcCopyFromBufferOffset, NcStatus, NC_E_INVALID_ARG};
use crate::{KeyboardAction, Key, KeyEvent, UiInputEvent, WidgetUpdateCtx};

/// Copies local point coordinates without exporting the math crate's Rust layout.
fn point(value: crate::Vec2i) -> Point {
    Point { x: value.x, y: value.y }
}

/// Preserves the complete key identity and modifier snapshot at the ABI boundary.
fn key(event: KeyEvent) -> KeyStroke {
    let (key, value) = match event.key {
        Key::Character(value) => (KeyCode::CHARACTER, value as u32),
        Key::Function(value) => (KeyCode::FUNCTION, u32::from(value)),
        key => (
            NAMED_KEYS.iter().find(|(_, candidate)| *candidate == key).expect("all named keys are mapped").0,
            0,
        ),
    };
    KeyStroke {
        key,
        value,
        pressed: event.is_pressed(),
        repeat: event.repeat,
        modifiers: event.modifiers.bits(),
    }
}

/// Copies the tagged event and borrows its existing text allocation for this call.
pub(super) fn event(input: Option<&UiInputEvent>) -> (WidgetEventAbi, &[u8]) {
    let (event, text): (WidgetEvent, &[u8]) = match input {
        None => (WidgetEvent::None, &[]),
        Some(UiInputEvent::Cancel) => (WidgetEvent::Cancel, &[]),
        Some(UiInputEvent::MouseMove { pos, delta }) => (
            WidgetEvent::Move {
                position: point(*pos),
                delta: point(*delta),
            },
            &[],
        ),
        Some(UiInputEvent::MouseDrag { pos, delta, buttons }) => (
            WidgetEvent::Drag {
                position: point(*pos),
                delta: point(*delta),
                buttons: buttons.bits() as u8,
            },
            &[],
        ),
        Some(UiInputEvent::MouseDown { pos, button }) => (
            WidgetEvent::Down {
                position: point(*pos),
                buttons: button.bits() as u8,
            },
            &[],
        ),
        Some(UiInputEvent::MouseUp { pos, button }) => (
            WidgetEvent::Up {
                position: point(*pos),
                buttons: button.bits() as u8,
            },
            &[],
        ),
        Some(UiInputEvent::Scroll { pos, delta }) => (
            WidgetEvent::Scroll {
                position: point(*pos),
                delta: point(*delta),
            },
            &[],
        ),
        Some(UiInputEvent::Key { event }) => (WidgetEvent::Key(key(*event)), &[]),
        Some(UiInputEvent::Text { text }) => (WidgetEvent::Text, text.as_bytes()),
    };
    (event.into(), text)
}

/// Reuses the authoritative shared action mapping instead of repeating key rules in plugins.
pub(super) fn snapshot(ctx: &WidgetUpdateCtx<'_>, input: Option<&UiInputEvent>, keyboard: crate::KeyboardBehavior) -> WidgetUpdate {
    let action = match ctx.action(input, keyboard) {
        None => WidgetAction::NONE,
        Some(KeyboardAction::Activate) => WidgetAction::ACTIVATE,
        Some(KeyboardAction::Decrease) => WidgetAction::DECREASE,
        Some(KeyboardAction::Increase) => WidgetAction::INCREASE,
        Some(KeyboardAction::Expand) => WidgetAction::EXPAND,
        Some(KeyboardAction::Collapse) => WidgetAction::COLLAPSE,
    };
    WidgetUpdate {
        in_hover_root: ctx.in_hover_root(),
        rect: copied_rect(ctx.local_rect()),
        clip: copied_rect(ctx.local_clip()),
        screen: copied_rect(ctx.screen_content_rect()),
        hovered: ctx.hovered(),
        focused: ctx.focused(),
        clicked: ctx.clicked(),
        active: ctx.active(),
        action,
        mouse_buttons: ctx.mouse_buttons().bits() as u8,
        modifiers: ctx.modifiers().bits(),
    }
}

/// One mapping table serves both normalization directions; character/function keys carry data.
const NAMED_KEYS: &[(KeyCode, Key)] = &[
    (KeyCode::BACKSPACE, Key::Backspace),
    (KeyCode::DELETE, Key::Delete),
    (KeyCode::ENTER, Key::Enter),
    (KeyCode::ESCAPE, Key::Escape),
    (KeyCode::SPACE, Key::Space),
    (KeyCode::TAB, Key::Tab),
    (KeyCode::INSERT, Key::Insert),
    (KeyCode::HOME, Key::Home),
    (KeyCode::END, Key::End),
    (KeyCode::PAGE_UP, Key::PageUp),
    (KeyCode::PAGE_DOWN, Key::PageDown),
    (KeyCode::ARROW_UP, Key::ArrowUp),
    (KeyCode::ARROW_DOWN, Key::ArrowDown),
    (KeyCode::ARROW_LEFT, Key::ArrowLeft),
    (KeyCode::ARROW_RIGHT, Key::ArrowRight),
    (KeyCode::ALT, Key::Alt),
    (KeyCode::CONTROL, Key::Control),
    (KeyCode::SHIFT, Key::Shift),
    (KeyCode::SUPER, Key::Super),
];

/// Converts one validated copied key without exposing native codes or Rust enum layout.
fn host_key(event: KeyStroke) -> Result<KeyEvent, NcStatus> {
    event.validate()?;
    let key = match event.key {
        KeyCode::CHARACTER => Key::Character(char::from_u32(event.value).ok_or(NC_E_INVALID_ARG)?),
        KeyCode::FUNCTION => Key::Function(event.value as u8),
        code => NAMED_KEYS.iter().find(|(candidate, _)| *candidate == code).ok_or(NC_E_INVALID_ARG)?.1,
    };
    Ok(KeyEvent {
        key,
        state: if event.pressed { crate::KeyState::Pressed } else { crate::KeyState::Released },
        modifiers: crate::Modifiers::from_bits_retain(event.modifiers),
        repeat: event.repeat,
    })
}

/// Validates one host event before changing the queue, then reuses existing normalization.
pub(crate) fn enqueue(context: &mut crate::Context<crate::com::render::Backend>, event: UiInputAbi, bytes: NcCopyFromBufferOffset<'_, u8>) -> Result<(), NcStatus> {
    let event = UiInput::try_from(event)?;
    let text = text(bytes)?;
    if !matches!(event, UiInput::Text) && !text.is_empty() {
        return Err(NC_E_INVALID_ARG);
    }
    match event {
        UiInput::Cancel => context.cancel_input(),
        UiInput::Pointer(position) => context.mousemove(position.x, position.y),
        UiInput::Down { position, buttons } => context.mousedown(
            position.x,
            position.y,
            crate::MouseButton::from_bits(u32::from(buttons)).ok_or(NC_E_INVALID_ARG)?,
        ),
        UiInput::Up { position, buttons } => context.mouseup(
            position.x,
            position.y,
            crate::MouseButton::from_bits(u32::from(buttons)).ok_or(NC_E_INVALID_ARG)?,
        ),
        UiInput::Scroll(delta) => context.scroll(delta.x, delta.y),
        UiInput::Key(event) => context.key(host_key(event)?),
        UiInput::Text => context.text(text),
    }
    Ok(())
}

/// Decodes copied COM input for the private built-in algorithms, owning only text.
pub(crate) fn local_event(event: WidgetEventAbi, bytes: NcCopyFromBufferOffset<'_, u8>) -> Result<Option<UiInputEvent>, NcStatus> {
    let (event, text) = decode_widget_event(event, bytes)?;
    /// Copies one ABI point into the private math value.
    fn point(value: Point) -> crate::Vec2i {
        crate::Vec2i::new(value.x, value.y)
    }
    Ok(Some(match event {
        WidgetEvent::None => return Ok(None),
        WidgetEvent::Cancel => UiInputEvent::Cancel,
        WidgetEvent::Move { position, delta } => UiInputEvent::MouseMove {
            pos: point(position),
            delta: point(delta),
        },
        WidgetEvent::Drag { position, delta, buttons } => UiInputEvent::MouseDrag {
            pos: point(position),
            delta: point(delta),
            buttons: crate::MouseButton::from_bits_retain(u32::from(buttons)),
        },
        WidgetEvent::Down { position, buttons } => UiInputEvent::MouseDown {
            pos: point(position),
            button: crate::MouseButton::from_bits_retain(u32::from(buttons)),
        },
        WidgetEvent::Up { position, buttons } => UiInputEvent::MouseUp {
            pos: point(position),
            button: crate::MouseButton::from_bits_retain(u32::from(buttons)),
        },
        WidgetEvent::Scroll { position, delta } => UiInputEvent::Scroll {
            pos: point(position),
            delta: point(delta),
        },
        WidgetEvent::Key(event) => UiInputEvent::Key { event: host_key(event)? },
        WidgetEvent::Text => UiInputEvent::Text { text: text.to_owned() },
    }))
}
