//! Typed COM output mappings into standard Rust channels.
//! Mappers translate owned snapshots immediately. The UI invokes the application's
//! ordinary mutable consumer later, after releasing traversal borrows.
//!
//! ```no_run
//! use neocogi_ui::{IEventOwnerAbi, Size, Ui, events::{self, Controller, EventModel}};
//! use neocogi_com::NcStatus;
//!
//! /// Ordinary application state is owned by one persistent controller.
//! struct Counter { count: u32 }
//! impl EventModel for Counter {
//!     type Message = ();
//!     fn handle(&mut self, (): ()) -> Result<(), NcStatus> {
//!         self.count += 1;
//!         Ok(())
//!     }
//! }
//! fn example(ui: &Ui, size: Size) -> Result<u32, NcStatus> {
//!     let (send, messages) = std::sync::mpsc::channel();
//!     let button = ui.widgets().button("Count", send, |_| ())?;
//!     let app = Controller::into_local(Controller::root(Counter { count: 0 }, messages));
//!     let owner = app.query::<IEventOwnerAbi>()?;
//!     // Keep both handles across frames; neither update creates an adapter.
//!     ui.update_with_events(size, owner.as_com_ref())?;
//!     ui.update_with_events(size, owner.as_com_ref())?;
//!     let count = app.try_borrow()?.model.count;
//!     Ok(count)
//! }
//! ```
/// Installs a typed COM mapping into a standard application or component channel.
pub use neocogi_com::events::bind;

/// Owned application fixtures for integration tests, without borrowed callback adapters.
#[cfg(feature = "test-support")]
pub mod testing;

mod controller;
pub use controller::{Controller, EventModel};

use crate::com::*;
use neocogi_com::{ComRef, NC_E_INVALID_ARG, NcStatus};

/// Core output values are the contract data itself; no facade message wrappers are introduced.
pub use neocogi_ui_core_contracts::{Activation, TextChanged, TextSubmitted, WindowEvent, PopupDismissed, ChoiceSelection, EVENT_DISPATCH_LIMIT};
/// Node-specific outputs preserve their concrete contract types and field documentation.
pub use neocogi_ui_nodes_contracts::{CheckboxChanged, NumericChanged, ScrollbarChanged, SplitChanged};

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

/// Adds the standard channel transport to one generated observer contract.
/// The mapper executes during posting; owner state is handled when its inbox drains.
macro_rules! widget_observer {
    ($module:ident, $event:path, $abi:ident, $validate:expr) => {
        /// Construction-time observer and owned event variants for this widget.
        pub mod $module {
            use super::*;
            /// The IDL-derived local sum of this widget's output payloads.
            pub use $event as Event;

            /// Creates one observer containing its sender and mapper inline.
            /// A disconnected owner discards future messages without failing UI input.
            pub fn observer<M: 'static>(
                sender: std::sync::mpsc::Sender<M>,
                mut map: impl FnMut(Event) -> M + 'static,
            ) -> Result<neocogi_com::ComPtr<$abi>, NcStatus> {
                filter_observer(sender, move |event| Some(map(event)))
            }

            /// Creates an observer that can ignore output variants before queueing.
            /// Domain checks run before the mapper, including for ignored variants.
            pub fn filter_observer<M: 'static>(
                sender: std::sync::mpsc::Sender<M>,
                mut map: impl FnMut(Event) -> Option<M> + 'static,
            ) -> Result<neocogi_com::ComPtr<$abi>, NcStatus> {
                Event::observer(move |event| {
                    ($validate)(&event)?;
                    if let Some(message) = map(event) {
                        let _ = sender.send(message);
                    }
                    Ok(())
                })
            }
        }
    };
}
widget_observer!(
    button,
    neocogi_ui_nodes_contracts::ButtonEvent,
    ButtonObserverAbi,
    |_: &Event| Ok::<(), NcStatus>(())
);
widget_observer!(checkbox, neocogi_ui_nodes_contracts::CheckboxEvent, CheckboxObserverAbi, |_: &Event| Ok::<
    (),
    NcStatus,
>(()));
widget_observer!(
    textbox,
    neocogi_ui_nodes_contracts::TextboxEvent,
    TextboxObserverAbi,
    |event: &Event| match event {
        Event::Changed(value) => valid_text_cursor(value),
        Event::Submitted(_) => Ok(()),
    }
);
widget_observer!(
    text_area,
    neocogi_ui_nodes_contracts::TextAreaEvent,
    TextAreaObserverAbi,
    |event: &Event| match event {
        Event::Changed(value) => valid_text_cursor(value),
        Event::Submitted(_) => Ok(()),
    }
);
widget_observer!(
    slider,
    neocogi_ui_nodes_contracts::SliderEvent,
    SliderObserverAbi,
    |event: &Event| match event {
        Event::Changed(value) => valid_numeric(value),
    }
);
widget_observer!(
    number,
    neocogi_ui_nodes_contracts::NumberEvent,
    NumberObserverAbi,
    |event: &Event| match event {
        Event::Changed(value) => valid_numeric(value),
    }
);
widget_observer!(list_item, neocogi_ui_nodes_contracts::ListItemEvent, ListItemObserverAbi, |_: &Event| Ok::<
    (),
    NcStatus,
>(()));
widget_observer!(list_box, neocogi_ui_nodes_contracts::ListBoxEvent, ListBoxObserverAbi, |_: &Event| Ok::<
    (),
    NcStatus,
>(()));
widget_observer!(
    scrollbar,
    neocogi_ui_nodes_contracts::ScrollbarEvent,
    ScrollbarObserverAbi,
    |event: &Event| match event {
        Event::Changed(value) => valid_scrollbar(value),
    }
);
widget_observer!(
    split_view,
    neocogi_ui_nodes_contracts::SplitViewEvent,
    SplitViewObserverAbi,
    |event: &Event| match event {
        Event::Changed(value) => valid_split(value),
    }
);
widget_observer!(combo, neocogi_ui_core_contracts::IChoiceMenuEvent, IChoiceMenuObserverAbi, |_: &Event| Ok::<
    (),
    NcStatus,
>(()));
widget_observer!(
    file_dialog,
    neocogi_ui_core_contracts::IFileDialogEvent,
    IFileDialogObserverAbi,
    |event: &Event| match event {
        Event::Completed(value) => valid_file_dialog_path(value),
    }
);
