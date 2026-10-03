//! Typed event-domain validation and deferred application dispatch.
//! Applications implement concrete queue traits on an object containing their sender.
//! Each constructor borrows the appropriate queue projection; one behavioral inbox
//! can serve many widgets without allocating per-widget callback adapters.
//!
//! The walkthrough below uses an initialized, otherwise empty UI. Its keyboard
//! input exercises actual activation, then removal with an application event queued.
//! The runtime regression compiles this exact source file and executes it against
//! the production core and node providers.
#![doc = concat!("```no_run\n", include_str!("../docs/button_events.rs"), "\n```")]

/// Owned application fixtures for integration tests, without borrowed callback adapters.
#[cfg(feature = "test-support")]
pub mod testing;

mod controller;
pub use controller::{ApplyOutcome, Controller, EventModel};

use crate::com::*;
use neocogi_com::{NC_E_INVALID_ARG, NcStatus};

/// Core output values are the contract data itself; no facade message wrappers are introduced.
pub use neocogi_ui_core_contracts::{Activation, TextChanged, TextSubmitted, WindowEvent, PopupDismissed, ChoiceSelection, EVENT_DISPATCH_LIMIT};
/// Node-specific outputs preserve their concrete contract types and field documentation.
pub use neocogi_ui_nodes_contracts::{CheckboxChanged, NumericChanged, ScrollbarChanged, SplitChanged};

/// Rejects a non-finite delivered value before message translation.
fn valid_numeric(event: &NumericChanged) -> Result<(), NcStatus> {
    if event.value.is_finite() { Ok(()) } else { Err(NC_E_INVALID_ARG) }
}

/// Rejects a negative offset before message translation.
fn valid_scrollbar(event: &ScrollbarChanged) -> Result<(), NcStatus> {
    if event.offset >= 0 { Ok(()) } else { Err(NC_E_INVALID_ARG) }
}

/// Rejects impossible external divider positions before application event mapping.
fn valid_split(event: &SplitChanged) -> Result<(), NcStatus> {
    if event.position >= 0 { Ok(()) } else { Err(NC_E_INVALID_ARG) }
}

/// Checks the captured cursor against the complete generated UTF-8 text snapshot.
fn valid_text_cursor(text: &str, cursor: u64) -> Result<(), NcStatus> {
    let position = usize::try_from(cursor).map_err(|_| NC_E_INVALID_ARG)?;
    if text.is_char_boundary(position) { Ok(()) } else { Err(NC_E_INVALID_ARG) }
}

/// Checks copied window geometry before message translation.
fn valid_window_event(event: &WindowEvent) -> Result<(), NcStatus> {
    match event {
        WindowEvent::GeometryChanged { rect } | WindowEvent::Minimized { rect } | WindowEvent::Maximized { rect } | WindowEvent::Restored { rect }
            if rect.width < 0 || rect.height < 0 =>
        {
            Err(NC_E_INVALID_ARG)
        }
        _ => Ok(()),
    }
}

/// Cancellation yields `None`; lossy display text is never used as path identity.
pub fn completed_file_path(event: &FileDialogCompletion) -> Result<Option<std::path::PathBuf>, NcStatus> {
    match event {
        FileDialogCompletion::Accepted { path } => crate::path::decode(path.as_com_ref()).map(Some),
        FileDialogCompletion::Cancelled => Ok(None),
    }
}

/// Rejects a foreign native-path capability before message translation.
fn valid_file_dialog_path(event: &FileDialogCompletion) -> Result<(), NcStatus> {
    completed_file_path(event).map(|_| ())
}

/// Publishes the concrete event type and its one authoritative domain validator.
/// Queue objects invoke this check after acquiring their typed ABI payload.
macro_rules! widget_event {
    ($module:ident, $event:path, $validate:expr) => {
        /// Complete event variants and domain validation for this producer kind.
        pub mod $module {
            use super::*;
            /// The IDL-derived owned sum of this producer's output payloads.
            pub use $event as Event;

            /// Checks domain invariants independently of the consumer or transport.
            /// Generated forms and handwritten concrete queues share this boundary.
            pub fn validate(event: &Event) -> Result<(), NcStatus> {
                ($validate)(event)
            }
        }
    };
}
widget_event!(button, neocogi_ui_nodes_contracts::ButtonEvent, |_: &Event| Ok::<(), NcStatus>(()));
widget_event!(checkbox, neocogi_ui_nodes_contracts::CheckboxEvent, |_: &Event| Ok::<(), NcStatus>(()));
widget_event!(textbox, neocogi_ui_nodes_contracts::TextboxEvent, |event: &Event| match event {
    Event::Changed(value) => valid_text_cursor(&value.text, value.cursor),
    Event::Submitted(_) => Ok(()),
});
widget_event!(text_area, neocogi_ui_nodes_contracts::TextAreaEvent, |event: &Event| match event {
    Event::Changed(value) => valid_text_cursor(&value.text, value.cursor),
    Event::Submitted(_) => Ok(()),
});
widget_event!(slider, neocogi_ui_nodes_contracts::SliderEvent, |event: &Event| match event {
    Event::Changed(value) => valid_numeric(value),
});
widget_event!(number, neocogi_ui_nodes_contracts::NumberEvent, |event: &Event| match event {
    Event::Changed(value) => valid_numeric(value),
});
widget_event!(list_item, neocogi_ui_nodes_contracts::ListItemEvent, |_: &Event| Ok::<(), NcStatus>(()));
widget_event!(list_box, neocogi_ui_nodes_contracts::ListBoxEvent, |_: &Event| Ok::<(), NcStatus>(()));
widget_event!(scrollbar, neocogi_ui_nodes_contracts::ScrollbarEvent, |event: &Event| match event {
    Event::Changed(value) => valid_scrollbar(value),
});
widget_event!(split_view, neocogi_ui_nodes_contracts::SplitViewEvent, |event: &Event| match event {
    Event::Changed(value) => valid_split(value),
});
widget_event!(combo, neocogi_ui_core_contracts::IChoiceMenuEvent, |_: &Event| Ok::<(), NcStatus>(()));
widget_event!(file_dialog, neocogi_ui_core_contracts::IFileDialogEvent, |event: &Event| match event {
    Event::Completed(value) => valid_file_dialog_path(value),
});

widget_event!(window, neocogi_ui_core_contracts::WindowEvent, valid_window_event);
widget_event!(popup, neocogi_ui_core_contracts::PopupDismissed, |_: &Event| Ok::<(), NcStatus>(()));
widget_event!(activation, neocogi_ui_core_contracts::Activation, |_: &Event| Ok::<(), NcStatus>(()));
