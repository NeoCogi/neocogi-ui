//! Application-owned typed handlers and registration-order delivery over concrete COM event contracts.
//!
//! A session owns its state and leases; producer queues own only immutable event
//! snapshots. The runtime borrows the dispatcher while returning its context
//! between input phases, so handlers can use ordinary widget/window methods.
//! Typed ports delegate to generated, consumed source projections;
//! the generic COM session claims registration before sink construction or a
//! foreign subscription call. The source-specific operation is not a delivery callback.
//!
//! ```no_run
//! use neocogi_ui::{ButtonWidget, EventSession, Size, Ui};
//! use neocogi_com::NcStatus;
//! use neocogi_ui::events::Activation;
//!
//! fn update(ui: &Ui, button: &ButtonWidget, size: Size) -> Result<u32, NcStatus> {
//!     let events = EventSession::new(0u32);
//!     events.subscribe(button.activated(), |count: &mut u32, _: &Activation| {
//!         *count += 1;
//!         Ok(())
//!     })?;
//!     ui.update_with_events(size, &events)?;
//!     let count = *events.state()?;
//!     Ok(count) // Dropping events cancels its leases, even when separately retained.
//! }
//! ```
//!
//! Delivery is FIFO within each source and registration ordered between sources.
//! Finish state borrows before dispatch. Handler errors consume the attempted
//! event, preserve unattempted events, and do not roll back prior callback edits.
//! Geometry is synchronized after handlers; behavior requiring an ordinary widget
//! update (for example a newly requested caret reveal) waits for the next eligible
//! update. Recursive update/render and session dispatch/registration report BUSY.
//! See `docs/COM_AUTHORS.md` for lifecycle, bounded cascades and author obligations.
/// One owner/session implementation serves applications and nested components.
pub use neocogi_com_events::{EventDelivery, EventEndpoint, EventHandler, EventPayload, EventPort, EventSession, EventSource, EVENT_DISPATCH_LIMIT};

/// Native queue access is reserved for isolated widget algorithm tests.
#[cfg(feature = "test-support")]
pub use neocogi_com_events::test_support;

use crate::com::*;
use neocogi_com::{ComRef, NC_E_INVALID_ARG, NcStatus};
use neocogi_ui_nodes_contracts::*;

/// Canonical generated activation snapshot shared by built-ins and external controls.
pub use neocogi_ui_core_contracts::ActivationMessage as Activation;
/// Canonical generated text-and-cursor snapshot.
pub use neocogi_ui_core_contracts::TextChangesMessage as TextChanged;
/// Canonical generated complete text submission.
pub use neocogi_ui_core_contracts::TextSubmissionMessage as TextSubmitted;
/// Canonical generated lifecycle snapshot containing its WindowEvent value.
pub use neocogi_ui_core_contracts::WindowLifecycleMessage as WindowChanged;
/// Canonical generated popup dismissal fact.
pub use neocogi_ui_core_contracts::PopupDismissalMessage as PopupDismissed;
/// Canonical generated choice snapshot containing its ChoiceSelection value.
pub use neocogi_ui_core_contracts::ChoiceChangesMessage as ChoiceChanged;
/// Canonical generated checkbox state captured on user activation.
pub use neocogi_ui_nodes_contracts::CheckboxChangesMessage as CheckboxChanged;
/// Canonical generated finite numeric value.
pub use neocogi_ui_nodes_contracts::NumericChangesMessage as NumericChanged;
/// Canonical generated nonnegative scrollbar offset.
pub use neocogi_ui_nodes_contracts::ScrollbarChangesMessage as ScrollbarChanged;

/// Defines a source-side convenience getter while leaving all session policy in COM.
/// Each expansion selects an exact generated port and, where needed, its domain check.
macro_rules! widget_port {
    ($widget:ty, $method:ident, $event:ty $(, $check:ident)?) => {
        impl $widget {
            /// Returns the typed event port for subscription by an application or component.
            /// Taking this capability never mounts the control or invokes its handler.
            pub fn $method(&self) -> impl EventPort<Event = $event> {
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
widget_port!(crate::ComboWidget, changed, ChoiceChanged);

/// Queries the actual window's lifecycle source without retaining it after subscription.
pub fn window_events(window: ComRef<'_, IWindowAbi>) -> Result<impl EventPort<Event = WindowChanged>, NcStatus> {
    Ok(window.add_ref().query::<IWindowEventsAbi>()?.changed().checked(valid_window_event))
}

/// Queries the popup's independent dismissal source; delivery describes a captured fact.
pub fn popup_events(popup: ComRef<'_, IPopupAbi>) -> Result<impl EventPort<Event = PopupDismissed>, NcStatus> {
    Ok(popup.add_ref().query::<IPopupEventsAbi>()?.dismissed())
}

impl crate::MenuItem {
    /// Queries this row's actual activation queue, shared by native and plugin clients.
    pub fn activated(&self) -> Result<impl EventPort<Event = Activation>, NcStatus> {
        Ok(self.as_interface().add_ref().query::<IActivationEventsAbi>()?.activated())
    }
}

impl crate::FileDialog {
    /// Returns checked native-path completions, including outcomes retained after closure.
    pub fn completed(&self) -> impl EventPort<Event = FileDialogCompletionMessage> {
        self.as_interface().completed().checked(valid_file_dialog_path)
    }
}

/// Rejects a non-finite delivered value before generated delivery borrows application state.
fn valid_numeric(event: &NumericChanged) -> Result<(), NcStatus> {
    if event.value.is_finite() { Ok(()) } else { Err(NC_E_INVALID_ARG) }
}

/// Rejects a negative offset before the generated sink borrows application state.
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

/// Checks copied window geometry before the generated sink borrows application state.
fn valid_window_event(event: &WindowLifecycleMessage) -> Result<(), NcStatus> {
    match event.event {
        WindowEvent::GeometryChanged { rect } | WindowEvent::Minimized { rect } | WindowEvent::Maximized { rect } | WindowEvent::Restored { rect }
            if rect.width < 0 || rect.height < 0 =>
        {
            Err(NC_E_INVALID_ARG)
        }
        _ => Ok(()),
    }
}

/// Cancellation yields `None`; lossy display text is never used as path identity.
pub fn completed_file_path(event: &FileDialogCompletionMessage) -> Result<Option<std::path::PathBuf>, NcStatus> {
    match event {
        FileDialogCompletionMessage::Accepted { path } => crate::path::decode(path.as_com_ref()).map(Some),
        FileDialogCompletionMessage::Cancelled => Ok(None),
    }
}

/// Rejects a foreign native-path capability before generated delivery borrows application state.
fn valid_file_dialog_path(event: &FileDialogCompletionMessage) -> Result<(), NcStatus> {
    completed_file_path(event).map(|_| ())
}

#[cfg(test)]
mod checked_adapter_tests {
    use super::*;
    use neocogi_ui_core_contracts::create_text_changes_port_sink;
    use neocogi_com::NcCopyFromBufferOffset;

    /// Concrete counter behavior; these tests require no hidden captured dependencies.
    struct Count;

    impl EventHandler<u32, NumericChanged> for Count {
        /// Counts one validated numeric delivery after the state borrow succeeds.
        fn handle(&mut self, count: &mut u32, _event: &NumericChanged) -> Result<(), NcStatus> {
            *count += 1;
            Ok(())
        }
    }

    impl EventHandler<u32, TextChanged> for Count {
        /// Counts one decoded, cursor-validated text delivery after the state borrow succeeds.
        fn handle(&mut self, count: &mut u32, _event: &TextChanged) -> Result<(), NcStatus> {
            *count += 1;
            Ok(())
        }
    }

    /// Invalid numeric facts are rejected before a held application-state borrow
    /// can change their error status; valid facts still obey the session borrow gate.
    #[test]
    fn generated_numeric_adapter_checks_before_state_borrow() {
        let session = EventSession::new(0u32);
        let mut sink = create_numeric_changes_port_sink(&session, Count, Some(valid_numeric)).unwrap();
        let held = session.state_mut().unwrap();
        assert_eq!(sink.changed(f32::NAN), Err(NC_E_INVALID_ARG));
        assert_eq!(sink.changed(1.0), Err(neocogi_com_events::EVENT_E_BUSY));
        drop(held);
        sink.changed(1.0).unwrap();
        assert_eq!(*session.state().unwrap(), 1);
    }

    /// Generated UTF-8 decoding and the domain cursor check both precede
    /// invocation, so neither a malformed string nor split scalar reaches state.
    #[test]
    fn generated_text_adapter_rejects_offset_and_split_cursor() {
        let session = EventSession::new(0u32);
        let mut sink = create_text_changes_port_sink(&session, Count, Some(valid_text_cursor)).unwrap();
        assert_eq!(sink.changed(NcCopyFromBufferOffset::new(1, "é".as_bytes()), 2), Err(NC_E_INVALID_ARG));
        assert_eq!(sink.changed(NcCopyFromBufferOffset::new(0, "é".as_bytes()), 1), Err(NC_E_INVALID_ARG));
        sink.changed(NcCopyFromBufferOffset::new(0, "é".as_bytes()), 2).unwrap();
        assert_eq!(*session.state().unwrap(), 1);
    }
}

/// Generated connectors are exercised against checked foreign producer calls.
#[cfg(test)]
mod connector_tests;
