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

mod dispatcher;
pub use dispatcher::{ApplyOutcome, Dispatcher, MessageProcessor};

use crate::com::*;
use neocogi_com::NcStatus;

/// Core output values are the contract data itself; no facade message wrappers are introduced.
pub use neocogi_ui_core_contracts::{Activation, TextChanged, TextSubmitted, WindowEvent, PopupDismissed, ChoiceSelection, EVENT_DISPATCH_LIMIT};
/// Node-specific outputs preserve their concrete contract types and field documentation.
pub use neocogi_ui_nodes_contracts::{CheckboxChanged, NumericChanged, ScrollbarChanged, SplitChanged};

/// Cancellation yields `None`; lossy display text is never used as path identity.
pub fn completed_file_path(event: &FileDialogCompletion) -> Result<Option<std::path::PathBuf>, NcStatus> {
    match event {
        FileDialogCompletion::Accepted { path } => crate::path::decode(path.as_com_ref()).map(Some),
        FileDialogCompletion::Cancelled => Ok(None),
    }
}

/// Common COM event transport and optional native snapshot conversions.
pub use neocogi_com::events::{EventMessage, EventOrigin, EventPayload, IEvent, IEventAbi, IEventReceiver, IEventReceiverAbi, downcast_event};

/// Publishes a native convenience type without duplicating its domain validator.
macro_rules! widget_event {
    ($module:ident, $event:path) => {
        /// Optional native enum view of a producer's semantic COM events.
        pub mod $module {
            use super::*;
            /// The IDL-derived native convenience value for this producer kind.
            pub use $event as Event;
            /// Uses the snapshot's authoritative validator for native and foreign data.
            pub fn validate(event: &Event) -> Result<(), NcStatus> {
                EventPayload::validate(event)
            }
        }
    };
}
widget_event!(button, neocogi_ui_nodes_contracts::ButtonEvent);
widget_event!(checkbox, neocogi_ui_nodes_contracts::CheckboxEvent);
widget_event!(textbox, neocogi_ui_nodes_contracts::TextboxEvent);
widget_event!(text_area, neocogi_ui_nodes_contracts::TextAreaEvent);
widget_event!(slider, neocogi_ui_nodes_contracts::SliderEvent);
widget_event!(number, neocogi_ui_nodes_contracts::NumberEvent);
widget_event!(list_item, neocogi_ui_nodes_contracts::ListItemEvent);
widget_event!(list_box, neocogi_ui_nodes_contracts::ListBoxEvent);
widget_event!(scrollbar, neocogi_ui_nodes_contracts::ScrollbarEvent);
widget_event!(split_view, neocogi_ui_nodes_contracts::SplitViewEvent);
widget_event!(combo, neocogi_ui_core_contracts::IChoiceMenuEvent);
widget_event!(file_dialog, neocogi_ui_core_contracts::IFileDialogEvent);
widget_event!(window, neocogi_ui_core_contracts::WindowEvent);
widget_event!(popup, neocogi_ui_core_contracts::PopupDismissed);
widget_event!(activation, neocogi_ui_core_contracts::Activation);
