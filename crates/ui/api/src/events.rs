//! Typed COM output mappings into standard Rust channels.
//! Mappers translate owned snapshots immediately. The UI invokes the application's
//! ordinary mutable consumer later, after releasing traversal borrows.
//!
//! ```no_run
//! use neocogi_ui::{IApplicationDispatcherAbi, Size, Ui, events::{self, Controller, EventModel}};
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
//!     let app = Controller::into_local(Controller::new(Counter { count: 0 }, messages));
//!     let owner = app.query::<IApplicationDispatcherAbi>()?;
//!     // Keep both handles across frames; neither update creates an adapter.
//!     ui.update_with_events(size, owner.as_com_ref())?;
//!     ui.update_with_events(size, owner.as_com_ref())?;
//!     let count = app.try_borrow()?.model.count;
//!     Ok(count)
//! }
//! ```
/// Installs a typed COM mapping into a standard application or component channel.
pub use neocogi_com::events::mapped_queue;

/// Owned application fixtures for integration tests, without borrowed callback adapters.
#[cfg(feature = "test-support")]
pub mod testing;

mod controller;
pub use controller::{Controller, EventModel};

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

/// Adds the standard channel transport to one generated queue contract.
/// The mapper executes during posting; owner state is handled when its inbox drains.
macro_rules! widget_queue {
    ($module:ident, $event:path, $abi:ident, $validate:expr) => {
        /// Construction-time queue and owned event variants for this widget.
        pub mod $module {
            use super::*;
            /// The IDL-derived local sum of this widget's output payloads.
            pub use $event as Event;

            /// Creates one queue containing its sender and mapper inline.
            /// A disconnected owner discards future messages without failing UI input.
            pub fn queue<M: 'static>(
                sender: std::sync::mpsc::Sender<M>,
                mut map: impl FnMut(Event) -> M + 'static,
            ) -> Result<neocogi_com::ComPtr<$abi>, NcStatus> {
                filter_queue(sender, move |event| Some(map(event)))
            }

            /// Checks the concrete event domain independently of its consumer or transport.
            /// Form-owned queues call this same boundary check before queueing events.
            pub fn validate(event: &Event) -> Result<(), NcStatus> {
                ($validate)(event)
            }

            /// Creates an queue that can ignore output variants before queueing.
            /// Domain checks run before the mapper, including for ignored variants.
            pub fn filter_queue<M: 'static>(
                sender: std::sync::mpsc::Sender<M>,
                mut map: impl FnMut(Event) -> Option<M> + 'static,
            ) -> Result<neocogi_com::ComPtr<$abi>, NcStatus> {
                <$abi as neocogi_com::events::EventQueueInterface>::receiver(move |_origin, event| {
                    validate(&event)?;
                    if let Some(message) = map(event) {
                        let _ = sender.send(message);
                    }
                    Ok(())
                })
            }
        }
    };
}
widget_queue!(button, neocogi_ui_nodes_contracts::ButtonEvent, ButtonEventQueueAbi, |_: &Event| Ok::<
    (),
    NcStatus,
>(()));
widget_queue!(checkbox, neocogi_ui_nodes_contracts::CheckboxEvent, CheckboxEventQueueAbi, |_: &Event| Ok::<
    (),
    NcStatus,
>(()));
widget_queue!(
    textbox,
    neocogi_ui_nodes_contracts::TextboxEvent,
    TextboxEventQueueAbi,
    |event: &Event| match event {
        Event::Changed(value) => valid_text_cursor(&value.text, value.cursor),
        Event::Submitted(_) => Ok(()),
    }
);
widget_queue!(
    text_area,
    neocogi_ui_nodes_contracts::TextAreaEvent,
    TextAreaEventQueueAbi,
    |event: &Event| match event {
        Event::Changed(value) => valid_text_cursor(&value.text, value.cursor),
        Event::Submitted(_) => Ok(()),
    }
);
widget_queue!(
    slider,
    neocogi_ui_nodes_contracts::SliderEvent,
    SliderEventQueueAbi,
    |event: &Event| match event {
        Event::Changed(value) => valid_numeric(value),
    }
);
widget_queue!(
    number,
    neocogi_ui_nodes_contracts::NumberEvent,
    NumberEventQueueAbi,
    |event: &Event| match event {
        Event::Changed(value) => valid_numeric(value),
    }
);
widget_queue!(list_item, neocogi_ui_nodes_contracts::ListItemEvent, ListItemEventQueueAbi, |_: &Event| Ok::<
    (),
    NcStatus,
>(()));
widget_queue!(list_box, neocogi_ui_nodes_contracts::ListBoxEvent, ListBoxEventQueueAbi, |_: &Event| Ok::<
    (),
    NcStatus,
>(()));
widget_queue!(
    scrollbar,
    neocogi_ui_nodes_contracts::ScrollbarEvent,
    ScrollbarEventQueueAbi,
    |event: &Event| match event {
        Event::Changed(value) => valid_scrollbar(value),
    }
);
widget_queue!(
    split_view,
    neocogi_ui_nodes_contracts::SplitViewEvent,
    SplitViewEventQueueAbi,
    |event: &Event| match event {
        Event::Changed(value) => valid_split(value),
    }
);
widget_queue!(combo, neocogi_ui_core_contracts::IChoiceMenuEvent, ComboEventQueueAbi, |_: &Event| Ok::<
    (),
    NcStatus,
>(()));
widget_queue!(
    file_dialog,
    neocogi_ui_core_contracts::IFileDialogEvent,
    IFileDialogEventQueueAbi,
    |event: &Event| match event {
        Event::Completed(value) => valid_file_dialog_path(value),
    }
);

widget_queue!(window, neocogi_ui_core_contracts::WindowEvent, IWindowEventQueueAbi, valid_window_event);
widget_queue!(popup, neocogi_ui_core_contracts::PopupDismissed, IPopupEventQueueAbi, |_: &Event| Ok::<
    (),
    NcStatus,
>(()));
widget_queue!(activation, neocogi_ui_core_contracts::Activation, IMenuItemEventQueueAbi, |_: &Event| Ok::<
    (),
    NcStatus,
>(()));
