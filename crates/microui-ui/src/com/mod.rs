//! Shared COM boundary services and the UI plugin's public object catalog.
//!
//! Concrete built-in widget and container objects implement their callable contracts beside their
//! semantic state in `ui_node`. This module retains only cross-object lifecycle, traversal,
//! children, event, layout, paint, rendering, runtime, window, menu, dialog, and factory services.
//! Runtime geometry remains private and foreign calls receive only copied values or owned scopes.

use microui_contracts::Rect;
use nc_com::{NcCopyFromBufferOffset, NcStatus, NC_E_INVALID_ARG};


/// Validates a whole UTF-8 replacement before touching any live object state.
pub(crate) fn text(input: NcCopyFromBufferOffset<'_, u8>) -> Result<&str, NcStatus> {
    if input.offset() != 0 {
        return Err(NC_E_INVALID_ARG);
    }
    std::str::from_utf8(input.as_slice()).map_err(|_| NC_E_INVALID_ARG)
}

/// Copies canonical ABI geometry into the implementation's concrete math type.
pub(crate) fn local_rect(rect: Rect) -> crate::Recti {
    crate::rect(rect.x, rect.y, rect.width, rect.height)
}

/// Copies legacy geometry without passing a foreign crate's Rust type through COM.
pub(crate) fn copied_rect(rect: crate::Recti) -> Rect {
    Rect {
        x: rect.x,
        y: rect.y,
        width: rect.width,
        height: rect.height,
    }
}

pub(crate) use crate::ui_node::frame_contract;
