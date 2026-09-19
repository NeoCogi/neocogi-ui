//! Typed COM output mappings into standard Rust channels.
//! Mappers translate owned snapshots immediately. The UI invokes the application's
//! ordinary mutable consumer later, after releasing traversal borrows.
//!
//! ```no_run
//! use neocogi_ui::{ButtonWidget, Size, Ui};
//! use neocogi_com::NcStatus;
//!
//! fn update(ui: &Ui, button: &ButtonWidget, size: Size) -> Result<u32, NcStatus> {
//!     let (events_tx, events) = std::sync::mpsc::channel();
//!     neocogi_ui::events::map(button.activated(), events_tx.clone(), |_| ())?;
//!     let mut count = 0;
//!     ui.update_with_events(size, &events, |_| { count += 1; Ok(()) })?;
//!     Ok(count)
//! }
//! ```
/// Installs a typed COM mapping into a standard application or component channel.
pub use neocogi_com_events::map;

use crate::com::*;
use neocogi_com::{ComRef, NC_E_INVALID_ARG, NcStatus};
use neocogi_ui_nodes_contracts::*;

/// Core output values are the contract data itself; no facade message wrappers are introduced.
pub use neocogi_ui_core_contracts::{Activation, TextChanged, TextSubmitted, WindowEvent, PopupDismissed, ChoiceSelection, EVENT_DISPATCH_LIMIT};
/// Node-specific outputs preserve their concrete contract types and field documentation.
pub use neocogi_ui_nodes_contracts::{CheckboxChanged, NumericChanged, ScrollbarChanged};

/// Delegates setup directly to the generated producer operation.
macro_rules! widget_output {
    ($widget:ty, $method:ident, $event:ty) => {
        impl $widget {
            /// Installs one destination when passed to `events::map`.
            pub fn $method(&self) -> impl FnOnce(Box<dyn FnMut($event) -> Result<(), NcStatus>>) -> Result<(), NcStatus> + use<> {
                self.as_interface().add_ref().$method()
            }
        }
    };
    ($widget:ty, $method:ident, $event:ty, $check:ident) => {
        impl $widget {
            /// Checks domain values before translating them into consumer messages.
            pub fn $method(&self) -> impl FnOnce(Box<dyn FnMut($event) -> Result<(), NcStatus>>) -> Result<(), NcStatus> + use<> {
                checked(self.as_interface().add_ref().$method(), $check)
            }
        }
    };
}

/// Adds one domain check to a setup closure; no output identity or lifecycle is introduced.
fn checked<E: 'static>(
    install: impl FnOnce(Box<dyn FnMut(E) -> Result<(), NcStatus>>) -> Result<(), NcStatus>,
    validate: fn(&E) -> Result<(), NcStatus>,
) -> impl FnOnce(Box<dyn FnMut(E) -> Result<(), NcStatus>>) -> Result<(), NcStatus> {
    move |mut consume| {
        install(Box::new(move |event| {
            validate(&event)?;
            consume(event)
        }))
    }
}

widget_output!(ButtonWidget, activated, Activation);
widget_output!(ListItemWidget, activated, Activation);
widget_output!(ListBoxWidget, activated, Activation);
widget_output!(CheckboxWidget, changed, CheckboxChanged);
widget_output!(crate::SliderWidget, changed, NumericChanged, valid_numeric);
widget_output!(crate::NumberWidget, changed, NumericChanged, valid_numeric);
widget_output!(crate::ScrollbarWidget, changed, ScrollbarChanged, valid_scrollbar);
widget_output!(crate::TextboxWidget, changed, TextChanged, valid_text_cursor);
widget_output!(crate::TextboxWidget, submitted, TextSubmitted);
widget_output!(crate::TextAreaWidget, changed, TextChanged, valid_text_cursor);
widget_output!(crate::TextAreaWidget, submitted, TextSubmitted);
widget_output!(crate::ComboWidget, changed, ChoiceSelection);

/// Obtains lifecycle output installation; setup temporarily retains the actual window.
pub fn window_events(
    window: ComRef<'_, IWindowAbi>,
) -> Result<impl FnOnce(Box<dyn FnMut(WindowEvent) -> Result<(), NcStatus>>) -> Result<(), NcStatus> + use<>, NcStatus> {
    Ok(checked(window.add_ref().query::<IWindowEventsAbi>()?.changed(), valid_window_event))
}

/// Obtains dismissal output installation; delivery describes a captured fact.
pub fn popup_events(
    popup: ComRef<'_, IPopupAbi>,
) -> Result<impl FnOnce(Box<dyn FnMut(PopupDismissed) -> Result<(), NcStatus>>) -> Result<(), NcStatus> + use<>, NcStatus> {
    Ok(popup.add_ref().query::<IPopupEventsAbi>()?.dismissed())
}

impl crate::MenuItem {
    /// Obtains this row's activation installation operation through its actual COM interface.
    pub fn activated(&self) -> Result<impl FnOnce(Box<dyn FnMut(Activation) -> Result<(), NcStatus>>) -> Result<(), NcStatus> + use<>, NcStatus> {
        Ok(self.as_interface().add_ref().query::<IActivationEventsAbi>()?.activated())
    }
}

impl crate::FileDialog {
    /// Returns checked native-path completions, including outcomes already posted before closure.
    pub fn completed(&self) -> impl FnOnce(Box<dyn FnMut(FileDialogCompletion) -> Result<(), NcStatus>>) -> Result<(), NcStatus> + use<> {
        checked(self.as_interface().add_ref().completed(), valid_file_dialog_path)
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
