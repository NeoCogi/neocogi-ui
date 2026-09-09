//! Shared COM boundary services and the UI plugin's public object catalog.
//!
//! Concrete built-in widget and container objects implement their callable contracts beside their
//! semantic state in `ui_node`. This module retains only cross-object lifecycle, traversal,
//! children, event, layout, paint, rendering, runtime, window, menu, dialog, and factory services.
//! Runtime geometry remains private and foreign calls receive only copied values or owned scopes.

use microui_contracts::*;
use nc_com::{ComPtr, ComRef, IUnknown, NcCopyFromBufferOffset, NcStatus, NC_E_INVALID_ARG, com_object};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

pub use crate::ui_node::{Grid, Linear};
pub use crate::ui_node::widgets::{Button, Checkbox, ColorSwatch, Combo, ListBox, ListItem, Number, Scrollbar, Slider, TextBlock, Textbox};
pub(crate) mod events;
pub use crate::ui_node::Disclosure;
mod runtime;
pub use crate::ui_node::ScrollArea;
pub use crate::ui_node::widgets::TextArea;
mod ui_factory;
pub use ui_factory::UiFactory;
pub use runtime::{Runtime, create_runtime_from_assets};
#[cfg(test)]
pub(crate) use runtime::create_runtime;

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

pub(crate) use crate::ui_node::{HostState, Observation, WidgetHost, frame_contract};

#[cfg(test)]
mod tests;
