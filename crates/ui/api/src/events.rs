//! Application-owned typed handlers and registration-order delivery over concrete COM event contracts.
//!
//! A session owns its state and leases; producer queues own only immutable event
//! snapshots. The runtime borrows the dispatcher while returning its context
//! between input phases, so handlers can use ordinary widget/window methods.
//! Subscription helpers delegate to generated, consumed `EventConnector` types;
//! the generic COM session claims registration before sink construction or a
//! foreign subscription call. The source-specific operation is not a delivery callback.
//!
//! ```no_run
//! use neocogi_ui::{ButtonWidget, EventHandler, EventSession, Size, Ui};
//! use neocogi_com::NcStatus;
//! use neocogi_ui::events::Activation;
//!
//! struct Count;
//! impl EventHandler<u32, Activation> for Count {
//!     fn handle(&mut self, count: &mut u32, _event: &Activation) -> Result<(), NcStatus> {
//!         *count += 1;
//!         Ok(())
//!     }
//! }
//! fn update(ui: &Ui, button: &ButtonWidget, size: Size) -> Result<u32, NcStatus> {
//!     let events = EventSession::new(0u32);
//!     events.subscribe_activation(button, Count)?;
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
/// Static producer queues shared by built-ins, independent plugins and application components.
pub use neocogi_com_events::{EventEndpoint, EventPayload, EventSource};

/// Internal queue access for isolated semantic-algorithm tests, absent from normal builds.
/// Applications and plugin authors subscribe through EventSource/EventEndpoint instead.
#[cfg(feature = "test-support")]
pub mod test_support {
    pub use neocogi_com_events::test_support::{EventBatch, EventLease, SubscribeError};
}

use crate::com::*;
use neocogi_ui_core_contracts::IChoiceChangedSinkEvent;
#[cfg(test)]
use neocogi_ui_core_contracts::create_i_text_changed_sink_checked_event_sink;
use neocogi_ui_nodes_contracts::{
    connect_button_subscribe, connect_checkbox_subscribe, connect_combo_subscribe_changed, connect_list_box_subscribe, connect_list_item_subscribe,
    connect_number_subscribe_checked, connect_scrollbar_subscribe_checked, connect_slider_subscribe_checked, connect_text_area_subscribe_changed_checked,
    connect_text_area_subscribe_submitted, connect_textbox_subscribe_changed_checked, connect_textbox_subscribe_submitted,
};
#[cfg(test)]
use neocogi_ui_nodes_contracts::create_i_numeric_sink_checked_event_sink;
use neocogi_com::{ComPtr, ComRef, NC_E_INVALID_ARG, NcStatus};
use std::ops::Deref;

pub use neocogi_com_events::{EventDelivery, EventHandler, EVENT_DISPATCH_LIMIT};

/// UI-family convenience facade over the COM-owned application event session.
/// The inner session owns state, leases, and registration-order dispatch; this
/// facade adds only typed connections to the UI NCIDL event families.
pub struct EventSession<State: 'static>(
    /// COM-owned transport core, never retained by a widget producer.
    neocogi_com_events::EventSession<State>,
);

impl<State: 'static> EventSession<State> {
    /// Creates an empty UI event facade with application-owned state.
    pub fn new(state: State) -> Self {
        Self(neocogi_com_events::EventSession::new(state))
    }
}

impl<State: 'static> Deref for EventSession<State> {
    type Target = neocogi_com_events::EventSession<State>;
    /// Exposes generic state and dispatch operations without duplicating them.
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<State: 'static> neocogi_com_events::EventDispatchSource for EventSession<State> {
    /// Supplies the COM-owned dispatcher without exposing UI family wiring.
    fn as_dispatcher(&self) -> ComRef<'_, IEventDispatcherAbi> {
        self.0.as_dispatcher()
    }
}

/// Canonical generated activation snapshot shared by built-ins and plugins.
pub use neocogi_ui_core_contracts::IActivationSinkEvent as Activation;

/// Typed adapter for concrete widgets or an explicitly queried activation capability.
pub trait ActivationSource {
    /// Uses the generated connector for this concrete source capability.
    fn connect_activation<State: 'static, Handler: EventHandler<State, Activation> + 'static>(
        &self,
        session: &neocogi_com_events::EventSession<State>,
        handler: Handler,
    ) -> Result<ComPtr<IEventSubscriptionAbi>, NcStatus>;
}

impl ActivationSource for ComPtr<IActivationEventsAbi> {
    /// External widgets subscribe through the standard capability queried once during wiring.
    fn connect_activation<State: 'static, Handler: EventHandler<State, Activation> + 'static>(
        &self,
        session: &neocogi_com_events::EventSession<State>,
        handler: Handler,
    ) -> Result<ComPtr<IEventSubscriptionAbi>, NcStatus> {
        connect_i_activation_events_subscribe(session, self.as_com_ref(), handler)
    }
}

impl ActivationSource for ButtonWidget {
    /// Buttons expose subscription directly on their callable class contract.
    fn connect_activation<State: 'static, Handler: EventHandler<State, Activation> + 'static>(
        &self,
        session: &neocogi_com_events::EventSession<State>,
        handler: Handler,
    ) -> Result<ComPtr<IEventSubscriptionAbi>, NcStatus> {
        connect_button_subscribe(session, self.as_interface(), handler)
    }
}

impl ActivationSource for ListItemWidget {
    /// List rows expose subscription directly on their callable class contract.
    fn connect_activation<State: 'static, Handler: EventHandler<State, Activation> + 'static>(
        &self,
        session: &neocogi_com_events::EventSession<State>,
        handler: Handler,
    ) -> Result<ComPtr<IEventSubscriptionAbi>, NcStatus> {
        connect_list_item_subscribe(session, self.as_interface(), handler)
    }
}

impl ActivationSource for ListBoxWidget {
    /// List boxes expose subscription directly on their callable class contract.
    fn connect_activation<State: 'static, Handler: EventHandler<State, Activation> + 'static>(
        &self,
        session: &neocogi_com_events::EventSession<State>,
        handler: Handler,
    ) -> Result<ComPtr<IEventSubscriptionAbi>, NcStatus> {
        connect_list_box_subscribe(session, self.as_interface(), handler)
    }
}

impl<State: 'static> EventSession<State> {
    /// Connects the widget's concrete event capability and registers its lease in deterministic order.
    /// Registration during an active session transaction reports BUSY before claiming the producer's port.
    pub fn subscribe_activation<Source: ActivationSource, Handler: EventHandler<State, Activation> + 'static>(
        &self,
        widget: &Source,
        handler: Handler,
    ) -> Result<ComPtr<IEventSubscriptionAbi>, NcStatus> {
        widget.connect_activation(&self.0, handler)
    }
}

/// Canonical generated checkbox snapshot committed by user activation.
pub use neocogi_ui_nodes_contracts::ICheckboxSinkEvent as CheckboxChanged;
impl<State: 'static> EventSession<State> {
    /// Connects the widget's concrete event capability and registers its lease in deterministic order.
    /// Registration during an active session transaction reports BUSY before claiming the producer's port.
    pub fn subscribe_checkbox<Handler: EventHandler<State, CheckboxChanged> + 'static>(
        &self,
        widget: &crate::CheckboxWidget,
        handler: Handler,
    ) -> Result<ComPtr<IEventSubscriptionAbi>, NcStatus> {
        connect_checkbox_subscribe(&self.0, widget.as_interface(), handler)
    }
}

/// Canonical generated numeric snapshot from a Slider or Number.
pub use neocogi_ui_nodes_contracts::INumericSinkEvent as NumericChanged;

/// Static facade hook shared by the concrete Slider and Number class wrappers.
pub trait NumericEventSource {
    /// Uses the class-specific generated checked connector without a second sink object.
    fn connect_numeric<State: 'static, Handler: EventHandler<State, NumericChanged> + 'static>(
        &self,
        session: &neocogi_com_events::EventSession<State>,
        handler: Handler,
    ) -> Result<ComPtr<IEventSubscriptionAbi>, NcStatus>;
}

impl NumericEventSource for crate::SliderWidget {
    /// Delegates directly to the Slider class projection without an event-interface query.
    fn connect_numeric<State: 'static, Handler: EventHandler<State, NumericChanged> + 'static>(
        &self,
        session: &neocogi_com_events::EventSession<State>,
        handler: Handler,
    ) -> Result<ComPtr<IEventSubscriptionAbi>, NcStatus> {
        connect_slider_subscribe_checked(session, self.as_interface(), handler, valid_numeric)
    }
}

impl NumericEventSource for crate::NumberWidget {
    /// Delegates directly to the Number class projection without an event-interface query.
    fn connect_numeric<State: 'static, Handler: EventHandler<State, NumericChanged> + 'static>(
        &self,
        session: &neocogi_com_events::EventSession<State>,
        handler: Handler,
    ) -> Result<ComPtr<IEventSubscriptionAbi>, NcStatus> {
        connect_number_subscribe_checked(session, self.as_interface(), handler, valid_numeric)
    }
}

/// Rejects a non-finite delivered value before generated delivery borrows application state.
fn valid_numeric(event: &NumericChanged) -> Result<(), NcStatus> {
    if event.value.is_finite() { Ok(()) } else { Err(NC_E_INVALID_ARG) }
}
impl<State: 'static> EventSession<State> {
    /// Connects the widget's concrete event capability and registers its lease in deterministic order.
    /// Registration during an active session transaction reports BUSY before claiming the producer's port.
    pub fn subscribe_numeric<Source: NumericEventSource, Handler: EventHandler<State, NumericChanged> + 'static>(
        &self,
        widget: &Source,
        handler: Handler,
    ) -> Result<ComPtr<IEventSubscriptionAbi>, NcStatus> {
        widget.connect_numeric(&self.0, handler)
    }
}

/// Canonical generated clamped scrollbar snapshot.
pub use neocogi_ui_nodes_contracts::IScrollbarSinkEvent as ScrollbarChanged;
/// Rejects a negative offset before the generated sink borrows application state.
fn valid_scrollbar(event: &ScrollbarChanged) -> Result<(), NcStatus> {
    if event.offset >= 0 { Ok(()) } else { Err(NC_E_INVALID_ARG) }
}
impl<State: 'static> EventSession<State> {
    /// Connects the widget's concrete event capability and registers its lease in deterministic order.
    /// Registration during an active session transaction reports BUSY before claiming the producer's port.
    pub fn subscribe_scrollbar<Handler: EventHandler<State, ScrollbarChanged> + 'static>(
        &self,
        widget: &crate::ScrollbarWidget,
        handler: Handler,
    ) -> Result<ComPtr<IEventSubscriptionAbi>, NcStatus> {
        connect_scrollbar_subscribe_checked(&self.0, widget.as_interface(), handler, valid_scrollbar)
    }
}

/// Canonical generated text-and-cursor snapshot from an editor.
pub use neocogi_ui_core_contracts::ITextChangedSinkEvent as TextChanged;

/// Typed facade hook shared by built-ins and explicitly queried text-event capabilities.
pub trait TextEventSource {
    /// Connects its independent changed port with the generated checked adapter.
    fn connect_changed<State: 'static, Handler: EventHandler<State, TextChanged> + 'static>(
        &self,
        session: &neocogi_com_events::EventSession<State>,
        handler: Handler,
    ) -> Result<ComPtr<IEventSubscriptionAbi>, NcStatus>;
    /// Connects its independent submitted port with the generated adapter.
    fn connect_submitted<State: 'static, Handler: EventHandler<State, TextSubmitted> + 'static>(
        &self,
        session: &neocogi_com_events::EventSession<State>,
        handler: Handler,
    ) -> Result<ComPtr<IEventSubscriptionAbi>, NcStatus>;
}

impl TextEventSource for ComPtr<ITextEventsAbi> {
    /// External editors subscribe through the standard capability queried once during wiring.
    fn connect_changed<State: 'static, Handler: EventHandler<State, TextChanged> + 'static>(
        &self,
        session: &neocogi_com_events::EventSession<State>,
        handler: Handler,
    ) -> Result<ComPtr<IEventSubscriptionAbi>, NcStatus> {
        connect_i_text_events_subscribe_changed_checked(session, self.as_com_ref(), handler, valid_text_cursor)
    }
    /// External editors reuse the same capability for their independent submission port.
    fn connect_submitted<State: 'static, Handler: EventHandler<State, TextSubmitted> + 'static>(
        &self,
        session: &neocogi_com_events::EventSession<State>,
        handler: Handler,
    ) -> Result<ComPtr<IEventSubscriptionAbi>, NcStatus> {
        connect_i_text_events_subscribe_submitted(session, self.as_com_ref(), handler)
    }
}

impl TextEventSource for crate::TextboxWidget {
    /// Delegates directly to the Textbox class projection without an event-interface query.
    fn connect_changed<State: 'static, Handler: EventHandler<State, TextChanged> + 'static>(
        &self,
        session: &neocogi_com_events::EventSession<State>,
        handler: Handler,
    ) -> Result<ComPtr<IEventSubscriptionAbi>, NcStatus> {
        connect_textbox_subscribe_changed_checked(session, self.as_interface(), handler, valid_text_cursor)
    }
    /// Delegates directly to the Textbox class projection without an event-interface query.
    fn connect_submitted<State: 'static, Handler: EventHandler<State, TextSubmitted> + 'static>(
        &self,
        session: &neocogi_com_events::EventSession<State>,
        handler: Handler,
    ) -> Result<ComPtr<IEventSubscriptionAbi>, NcStatus> {
        connect_textbox_subscribe_submitted(session, self.as_interface(), handler)
    }
}

impl TextEventSource for crate::TextAreaWidget {
    /// Delegates directly to the TextArea class projection without an event-interface query.
    fn connect_changed<State: 'static, Handler: EventHandler<State, TextChanged> + 'static>(
        &self,
        session: &neocogi_com_events::EventSession<State>,
        handler: Handler,
    ) -> Result<ComPtr<IEventSubscriptionAbi>, NcStatus> {
        connect_text_area_subscribe_changed_checked(session, self.as_interface(), handler, valid_text_cursor)
    }
    /// Delegates directly to the TextArea class projection without an event-interface query.
    fn connect_submitted<State: 'static, Handler: EventHandler<State, TextSubmitted> + 'static>(
        &self,
        session: &neocogi_com_events::EventSession<State>,
        handler: Handler,
    ) -> Result<ComPtr<IEventSubscriptionAbi>, NcStatus> {
        connect_text_area_subscribe_submitted(session, self.as_interface(), handler)
    }
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
impl<State: 'static> EventSession<State> {
    /// Connects the widget's concrete event capability and registers its lease in deterministic order.
    /// Registration during an active session transaction reports BUSY before claiming the producer's port.
    pub fn subscribe_text_changed<Source: TextEventSource, Handler: EventHandler<State, TextChanged> + 'static>(
        &self,
        widget: &Source,
        handler: Handler,
    ) -> Result<ComPtr<IEventSubscriptionAbi>, NcStatus> {
        widget.connect_changed(&self.0, handler)
    }
}

/// Canonical generated normalized text submission snapshot.
pub use neocogi_ui_core_contracts::ITextSubmittedSinkEvent as TextSubmitted;
impl<State: 'static> EventSession<State> {
    /// Connects the widget's concrete event capability and registers its lease in deterministic order.
    /// Registration during an active session transaction reports BUSY before claiming the producer's port.
    pub fn subscribe_text_submitted<Source: TextEventSource, Handler: EventHandler<State, TextSubmitted> + 'static>(
        &self,
        widget: &Source,
        handler: Handler,
    ) -> Result<ComPtr<IEventSubscriptionAbi>, NcStatus> {
        widget.connect_submitted(&self.0, handler)
    }
}

/// Checks copied window geometry before the generated sink borrows application state.
fn valid_window_event(event: &IWindowSinkEvent) -> Result<(), NcStatus> {
    match event.event {
        WindowEvent::GeometryChanged { rect } | WindowEvent::Minimized { rect } | WindowEvent::Maximized { rect } | WindowEvent::Restored { rect }
            if rect.width < 0 || rect.height < 0 =>
        {
            Err(NC_E_INVALID_ARG)
        }
        _ => Ok(()),
    }
}

/// Thin application projection from a generated delivery wrapper to its canonical NCIDL event.
struct WindowProject<Handler> {
    /// Exact concrete handler stays inside the application binary.
    handler: Handler,
}
impl<State, Handler: EventHandler<State, WindowEvent>> EventHandler<State, IWindowSinkEvent> for WindowProject<Handler> {
    /// Forwards the validated canonical event without copying a second geometry model.
    fn handle(&mut self, state: &mut State, event: &IWindowSinkEvent) -> Result<(), NcStatus> {
        self.handler.handle(state, &event.event)
    }
}
/// Canonical generated dismissal fact; it does not imply permanent destruction.
pub use neocogi_ui_core_contracts::IPopupSinkEvent as PopupDismissed;
impl<State: 'static> EventSession<State> {
    /// Registers the actual window's lifecycle source, including terminal delivery after its closure.
    pub fn subscribe_window<Handler: EventHandler<State, WindowEvent> + 'static>(
        &self,
        window: ComRef<'_, IWindowAbi>,
        handler: Handler,
    ) -> Result<ComPtr<IEventSubscriptionAbi>, NcStatus> {
        let source = window.add_ref().query::<IWindowEventsAbi>()?;
        connect_i_window_events_subscribe_checked(&self.0, source.as_com_ref(), WindowProject { handler }, valid_window_event)
    }
    /// Registers the actual popup's dismissal source; later visibility is read independently from this snapshot.
    pub fn subscribe_popup<Handler: EventHandler<State, PopupDismissed> + 'static>(
        &self,
        popup: ComRef<'_, IPopupAbi>,
        handler: Handler,
    ) -> Result<ComPtr<IEventSubscriptionAbi>, NcStatus> {
        let source = popup.add_ref().query::<IPopupEventsAbi>()?;
        connect_i_popup_events_subscribe(&self.0, source.as_com_ref(), handler)
    }
}

/// Thin application projection from the generated one-field event to its NCIDL selection value.
struct SelectionProject<Handler> {
    /// Concrete application handler remains native and is never stored by the producer.
    handler: Handler,
}
impl<State, Handler: EventHandler<State, ChoiceSelection>> EventHandler<State, IChoiceChangedSinkEvent> for SelectionProject<Handler> {
    /// Forwards the already decoded selection without another COM object or parallel value type.
    fn handle(&mut self, state: &mut State, event: &IChoiceChangedSinkEvent) -> Result<(), NcStatus> {
        self.handler.handle(state, &event.selection)
    }
}
impl<State: 'static> EventSession<State> {
    /// Registers the generated selection fact without claiming the header's private popup port.
    pub fn subscribe_combo_changed<Handler: EventHandler<State, ChoiceSelection> + 'static>(
        &self,
        combo: &crate::ComboWidget,
        handler: Handler,
    ) -> Result<ComPtr<IEventSubscriptionAbi>, NcStatus> {
        connect_combo_subscribe_changed(&self.0, combo.as_interface(), SelectionProject { handler })
    }
}

impl<State: 'static> EventSession<State> {
    /// Registers a compact menu row's actual activation queue in the same deterministic sequence as widget events.
    pub fn subscribe_menu_item<Handler: EventHandler<State, Activation> + 'static>(
        &self,
        item: &crate::MenuItem,
        handler: Handler,
    ) -> Result<ComPtr<IEventSubscriptionAbi>, NcStatus> {
        let source = item.as_interface().add_ref().query::<IActivationEventsAbi>()?;
        connect_i_activation_events_subscribe(&self.0, source.as_com_ref(), handler)
    }
}

/// Decodes an accepted generated completion to an exact host-native path.
/// Cancellation yields `None`; lossy display text is never used as path identity.
pub fn completed_file_path(event: &IFileDialogSinkEvent) -> Result<Option<std::path::PathBuf>, NcStatus> {
    match event {
        IFileDialogSinkEvent::Accepted { path } => crate::path::decode(path.as_com_ref()).map(Some),
        IFileDialogSinkEvent::Cancelled => Ok(None),
    }
}

/// Rejects a foreign native-path capability before generated delivery borrows application state.
fn valid_file_dialog_path(event: &IFileDialogSinkEvent) -> Result<(), NcStatus> {
    completed_file_path(event).map(|_| ())
}
impl<State: 'static> EventSession<State> {
    /// Registers the picker's actual completion queue in the same deterministic application sequence.
    pub fn subscribe_file_dialog<Handler: EventHandler<State, IFileDialogSinkEvent> + 'static>(
        &self,
        dialog: &crate::FileDialog,
        handler: Handler,
    ) -> Result<ComPtr<IEventSubscriptionAbi>, NcStatus> {
        let source = dialog.as_interface().add_ref().query::<IFileDialogEventsAbi>()?;
        connect_i_file_dialog_events_subscribe_checked(&self.0, source.as_com_ref(), handler, valid_file_dialog_path)
    }
}

#[cfg(test)]
mod checked_adapter_tests {
    use super::*;
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
        let sink = create_i_numeric_sink_checked_event_sink(&session.0, Count, valid_numeric).unwrap();
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
        let sink = create_i_text_changed_sink_checked_event_sink(&session.0, Count, valid_text_cursor).unwrap();
        assert_eq!(sink.changed(NcCopyFromBufferOffset::new(1, "é".as_bytes()), 2), Err(NC_E_INVALID_ARG));
        assert_eq!(sink.changed(NcCopyFromBufferOffset::new(0, "é".as_bytes()), 1), Err(NC_E_INVALID_ARG));
        sink.changed(NcCopyFromBufferOffset::new(0, "é".as_bytes()), 2).unwrap();
        assert_eq!(*session.state().unwrap(), 1);
    }
}

/// Generated connectors are exercised against checked foreign producer calls.
#[cfg(test)]
mod connector_tests;
