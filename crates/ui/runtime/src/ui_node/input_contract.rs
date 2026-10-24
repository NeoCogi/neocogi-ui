//! Host enqueue policy; shared event decoding lives in provider-independent presentation support.
use crate::boundary::text;
use neocogi_com::{NcCopyFromBufferOffset, NcStatus, NC_E_INVALID_ARG};
use neocogi_ui::abi::core::*;
pub(crate) use neocogi_ui_support::input_contract::{event, snapshot};
use neocogi_ui_support::input_contract::host_key;

/// Validates one host event before changing the queue, then reuses existing normalization.
pub(crate) fn enqueue(context: &mut crate::window_manager::WindowManager, event: UiInput, bytes: NcCopyFromBufferOffset<'_, u8>) -> Result<(), NcStatus> {
    let text = text(bytes)?;
    if !matches!(event, UiInput::Text) && !text.is_empty() {
        return Err(NC_E_INVALID_ARG);
    }
    if text.len() > crate::input::MAX_PENDING_TEXT_BYTES {
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
