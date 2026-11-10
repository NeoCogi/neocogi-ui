//! Small value conversions shared by independently owned foreign-facing implementations.
//!
//! Contract declarations remain in the shared UI IDL, generated in `neocogi_ui::abi::core`; this module only validates borrowed UTF-8
//! and copies geometry between the contract representation and the retained math representation.

use neocogi_ui::abi::core::Rect;
use neocogi_com::{NcCopyFromBufferOffset, NcStatus, NC_E_INVALID_ARG};

/// Validates a whole UTF-8 replacement before touching any live object state.
pub fn text(input: NcCopyFromBufferOffset<'_, u8>) -> Result<&str, NcStatus> {
    if input.offset() != 0 {
        return Err(NC_E_INVALID_ARG);
    }
    std::str::from_utf8(input.as_slice()).map_err(|_| NC_E_INVALID_ARG)
}

/// Copies canonical ABI geometry into the implementation's concrete math type.
pub fn local_rect(rect: Rect) -> crate::support::Recti {
    crate::support::rect(rect.x, rect.y, rect.width, rect.height)
}

/// Copies legacy geometry without passing a foreign crate's Rust type through COM.
pub fn copied_rect(rect: crate::support::Recti) -> Rect {
    Rect {
        x: rect.x,
        y: rect.y,
        width: rect.width,
        height: rect.height,
    }
}

/// Resolves one concrete font-role vocabulary for both widget and compact menu presentation.
pub fn local_font_role(role: neocogi_ui::abi::core::FontRole) -> Result<crate::support::FontRole, NcStatus> {
    match role {
        neocogi_ui::abi::core::FontRole::Body => Ok(crate::support::FontRole::Body),
        neocogi_ui::abi::core::FontRole::Small => Ok(crate::support::FontRole::Small),
        neocogi_ui::abi::core::FontRole::Title => Ok(crate::support::FontRole::Title),
        neocogi_ui::abi::core::FontRole::Heading => Ok(crate::support::FontRole::Heading),
        neocogi_ui::abi::core::FontRole::Mono => Ok(crate::support::FontRole::Mono),
        _ => Err(NC_E_INVALID_ARG),
    }
}
