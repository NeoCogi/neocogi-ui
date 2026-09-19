//! Typed protocol mappings into application-owned message inboxes.
//! Mappers translate owned snapshots immediately. The UI invokes the application's
//! ordinary mutable consumer later, after releasing traversal borrows.
//!
//! ```no_run
//! use neocogi_ui::{ButtonWidget, Events, Size, Ui};
//! use neocogi_com::NcStatus;
//!
//! fn update(ui: &Ui, button: &ButtonWidget, size: Size) -> Result<u32, NcStatus> {
//!     let mut events = Events::new();
//!     events.map(button.activated(), |_| ())?;
//!     let mut count = 0;
//!     ui.update_with_events(size, &mut events, |_| { count += 1; Ok(()) })?;
//!     Ok(count)
//! }
//! ```
/// Shared typed transport for applications and nested components.
pub use neocogi_com_events::{Events, EventSender, PortRef, Port, EVENT_DISPATCH_LIMIT};

/// Isolated native algorithm collectors using the production inbox transport.
#[cfg(feature = "test-support")]
pub use neocogi_com_events::test_support;

use crate::com::*;
use neocogi_com::{ComRef, NC_E_INVALID_ARG, NcStatus};
use neocogi_ui_nodes_contracts::*;

/// Core output values are the contract data itself; no facade message wrappers are introduced.
pub use neocogi_ui_core_contracts::{Activation, TextChanged, TextSubmitted, WindowEvent, PopupDismissed, ChoiceSelection};
/// Node-specific outputs preserve their concrete contract types and field documentation.
pub use neocogi_ui_nodes_contracts::{CheckboxChanged, NumericChanged, ScrollbarChanged};

/// Defines a source-side convenience getter while leaving all consumer ownership in COM.
/// Each expansion selects an exact generated port and, where needed, its domain check.
macro_rules! widget_port {
    ($widget:ty, $method:ident, $event:ty $(, $check:ident)?) => {
        impl $widget {
            /// Returns the typed event port for subscription by an application or component.
            /// Taking this capability never mounts the control or invokes its handler.
            pub fn $method(&self) -> PortRef<$event> {
                self.as_interface().$method()$(.checked($check))?
            }
        }
    };
}

widget_port!(ButtonWidget, activated, Activation);
widget_port!(ListItemWidget, activated, Activation);
widget_port!(ListBoxWidget, activated, Activation);
widget_port!(CheckboxWidget, changed, CheckboxChanged);
widget_port!(crate::SliderWidget, changed, NumericChanged, valid_numeric);
widget_port!(crate::NumberWidget, changed, NumericChanged, valid_numeric);
widget_port!(crate::ScrollbarWidget, changed, ScrollbarChanged, valid_scrollbar);
widget_port!(crate::TextboxWidget, changed, TextChanged, valid_text_cursor);
widget_port!(crate::TextboxWidget, submitted, TextSubmitted);
widget_port!(crate::TextAreaWidget, changed, TextChanged, valid_text_cursor);
widget_port!(crate::TextAreaWidget, submitted, TextSubmitted);
widget_port!(crate::ComboWidget, changed, ChoiceSelection);

/// Queries the actual window's lifecycle source without retaining it after subscription.
pub fn window_events(window: ComRef<'_, IWindowAbi>) -> Result<PortRef<WindowEvent>, NcStatus> {
    Ok(window.add_ref().query::<IWindowEventsAbi>()?.changed().checked(valid_window_event))
}

/// Queries the popup's independent dismissal source; delivery describes a captured fact.
pub fn popup_events(popup: ComRef<'_, IPopupAbi>) -> Result<PortRef<PopupDismissed>, NcStatus> {
    Ok(popup.add_ref().query::<IPopupEventsAbi>()?.dismissed())
}

impl crate::MenuItem {
    /// Queries this row's actual activation port, shared by native and plugin clients.
    pub fn activated(&self) -> Result<PortRef<Activation>, NcStatus> {
        Ok(self.as_interface().add_ref().query::<IActivationEventsAbi>()?.activated())
    }
}

impl crate::FileDialog {
    /// Returns checked native-path completions, including outcomes already posted before closure.
    pub fn completed(&self) -> PortRef<FileDialogCompletion> {
        self.as_interface().completed().checked(valid_file_dialog_path)
    }
}

/// Rejects a non-finite delivered value before message translation.
fn valid_numeric(event: &NumericChanged) -> Result<(), NcStatus> {
    if event.value.is_finite() { Ok(()) } else { Err(NC_E_INVALID_ARG) }
}

/// Rejects a negative offset before message translation.
fn valid_scrollbar(event: &ScrollbarChanged) -> Result<(), NcStatus> {
    if event.offset >= 0 { Ok(()) } else { Err(NC_E_INVALID_ARG) }
}

/// Checks the captured cursor against the complete generated UTF-8 text snapshot.
fn valid_text_cursor(event: &TextChanged) -> Result<(), NcStatus> {
    let position = usize::try_from(event.cursor).map_err(|_| NC_E_INVALID_ARG)?;
    if event.text.is_char_boundary(position) {
        Ok(())
    } else {
        Err(NC_E_INVALID_ARG)
    }
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

/// Generated connectors are exercised against checked foreign producer calls.
#[cfg(test)]
mod connector_tests;
