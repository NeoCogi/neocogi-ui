//! Small value conversions shared by independently owned foreign-facing implementations.
//!
//! Contract declarations remain in `microui-contracts`; this module only validates borrowed UTF-8
//! and copies geometry between the contract representation and the retained math representation.

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
