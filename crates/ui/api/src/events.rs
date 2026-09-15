//! Application-owned typed handlers and registration-order delivery over concrete COM event contracts.
//!
//! A session owns its state and leases; producer queues own only immutable event
//! snapshots. The runtime borrows the dispatcher while returning its context
//! between input phases, so handlers can use ordinary widget/window methods.
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
use neocogi_com::{ComPtr, ComRef, NC_E_INVALID_ARG, NcCopyFromBufferOffset, NcStatus, com_object};
use std::ops::Deref;

/// Copies one complete offset-zero IDL string view into application-owned text.
fn copied_text(value: NcCopyFromBufferOffset<'_, u8>) -> Result<String, NcStatus> {
    // Event snapshots cross synchronously as UTF-8 bytes. Rejecting a nonzero
    // logical offset prevents a producer from presenting a partial string as a
    // complete semantic value.
    if value.offset() != 0 {
        return Err(NC_E_INVALID_ARG);
    }
    std::str::from_utf8(value.as_slice()).map(str::to_owned).map_err(|_| NC_E_INVALID_ARG)
}

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
    /// Connects one sink through the concrete class or shared interface that owns the source.
    fn subscribe_activation_sink(&self, sink: ComRef<'_, IActivationSinkAbi>) -> Result<ComPtr<IEventSubscriptionAbi>, NcStatus>;
}

impl ActivationSource for ComPtr<IActivationEventsAbi> {
    /// External widgets subscribe through the standard capability queried once during wiring.
    fn subscribe_activation_sink(&self, sink: ComRef<'_, IActivationSinkAbi>) -> Result<ComPtr<IEventSubscriptionAbi>, NcStatus> {
        self.subscribe(sink)
    }
}

impl ActivationSource for ButtonWidget {
    /// Buttons expose subscription directly on their callable class contract.
    fn subscribe_activation_sink(&self, sink: ComRef<'_, IActivationSinkAbi>) -> Result<ComPtr<IEventSubscriptionAbi>, NcStatus> {
        self.as_interface().subscribe(sink)
    }
}

impl ActivationSource for ListItemWidget {
    /// List rows expose subscription directly on their callable class contract.
    fn subscribe_activation_sink(&self, sink: ComRef<'_, IActivationSinkAbi>) -> Result<ComPtr<IEventSubscriptionAbi>, NcStatus> {
        self.as_interface().subscribe(sink)
    }
}

impl ActivationSource for ListBoxWidget {
    /// List boxes expose subscription directly on their callable class contract.
    fn subscribe_activation_sink(&self, sink: ComRef<'_, IActivationSinkAbi>) -> Result<ComPtr<IEventSubscriptionAbi>, NcStatus> {
        self.as_interface().subscribe(sink)
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
        self.0.register(|| {
            let sink = create_i_activation_sink_event_sink(&self.0, handler)?;
            let lease = widget.subscribe_activation_sink(sink.as_com_ref())?;
            Ok(lease)
        })
    }
}

/// Canonical generated checkbox snapshot committed by user activation.
pub use neocogi_ui_core_contracts::ICheckboxSinkEvent as CheckboxChanged;
impl<State: 'static> EventSession<State> {
    /// Connects the widget's concrete event capability and registers its lease in deterministic order.
    /// Registration during an active session transaction reports BUSY before claiming the producer's port.
    pub fn subscribe_checkbox<Handler: EventHandler<State, CheckboxChanged> + 'static>(
        &self,
        widget: &crate::CheckboxWidget,
        handler: Handler,
    ) -> Result<ComPtr<IEventSubscriptionAbi>, NcStatus> {
        self.0.register(|| {
            let sink = create_i_checkbox_sink_event_sink(&self.0, handler)?;
            let lease = widget.as_interface().subscribe(sink.as_com_ref())?;
            Ok(lease)
        })
    }
}

/// Canonical generated numeric snapshot from a Slider or Number.
pub use neocogi_ui_core_contracts::INumericSinkEvent as NumericChanged;

/// Static facade hook shared by the concrete Slider and Number class wrappers.
pub trait NumericEventSource {
    /// Connects a numeric sink through the concrete class contract.
    fn subscribe_numeric_sink(&self, sink: ComRef<'_, INumericSinkAbi>) -> Result<ComPtr<IEventSubscriptionAbi>, NcStatus>;
}

impl NumericEventSource for crate::SliderWidget {
    /// Delegates directly to the Slider class projection without an event-interface query.
    fn subscribe_numeric_sink(&self, sink: ComRef<'_, INumericSinkAbi>) -> Result<ComPtr<IEventSubscriptionAbi>, NcStatus> {
        self.as_interface().subscribe(sink)
    }
}

impl NumericEventSource for crate::NumberWidget {
    /// Delegates directly to the Number class projection without an event-interface query.
    fn subscribe_numeric_sink(&self, sink: ComRef<'_, INumericSinkAbi>) -> Result<ComPtr<IEventSubscriptionAbi>, NcStatus> {
        self.as_interface().subscribe(sink)
    }
}

#[com_object(interfaces(INumericSink))]
/// Concrete local sink specialization stores application state and its exact handler type directly.
struct NumericSink<State: 'static, Handler: EventHandler<State, NumericChanged> + 'static> {
    /// One shared local invocation helper; no erased handler or Rust value crosses COM.
    handler: EventDelivery<State, Handler>,
}
impl<State: 'static, Handler: EventHandler<State, NumericChanged> + 'static> INumericSink for NumericSink<State, Handler> {
    /// Validates and copies the concrete foreign snapshot before invoking application behavior.
    fn changed(&self, value: f32) -> Result<(), NcStatus> {
        if !value.is_finite() {
            return Err(NC_E_INVALID_ARG);
        }
        self.handler.deliver(NumericChanged { value })
    }
}
impl<State: 'static> EventSession<State> {
    /// Connects the widget's concrete event capability and registers its lease in deterministic order.
    /// Registration during an active session transaction reports BUSY before claiming the producer's port.
    pub fn subscribe_numeric<Source: NumericEventSource, Handler: EventHandler<State, NumericChanged> + 'static>(
        &self,
        widget: &Source,
        handler: Handler,
    ) -> Result<ComPtr<IEventSubscriptionAbi>, NcStatus> {
        self.0.register(|| {
            let sink: ComPtr<INumericSinkAbi> = NumericSink::from_value(NumericSink { handler: self.0.delivery(handler) }).query()?;
            let lease = widget.subscribe_numeric_sink(sink.as_com_ref())?;
            Ok(lease)
        })
    }
}

/// Canonical generated clamped scrollbar snapshot.
pub use neocogi_ui_core_contracts::IScrollbarSinkEvent as ScrollbarChanged;
#[com_object(interfaces(IScrollbarSink))]
/// Concrete local sink specialization stores application state and its exact handler type directly.
struct ScrollbarSink<State: 'static, Handler: EventHandler<State, ScrollbarChanged> + 'static> {
    /// One shared local invocation helper; no erased handler or Rust value crosses COM.
    handler: EventDelivery<State, Handler>,
}
impl<State: 'static, Handler: EventHandler<State, ScrollbarChanged> + 'static> IScrollbarSink for ScrollbarSink<State, Handler> {
    /// Validates and copies the concrete foreign snapshot before invoking application behavior.
    fn changed(&self, offset: i32) -> Result<(), NcStatus> {
        if offset < 0 {
            return Err(NC_E_INVALID_ARG);
        }
        self.handler.deliver(ScrollbarChanged { offset })
    }
}
impl<State: 'static> EventSession<State> {
    /// Connects the widget's concrete event capability and registers its lease in deterministic order.
    /// Registration during an active session transaction reports BUSY before claiming the producer's port.
    pub fn subscribe_scrollbar<Handler: EventHandler<State, ScrollbarChanged> + 'static>(
        &self,
        widget: &crate::ScrollbarWidget,
        handler: Handler,
    ) -> Result<ComPtr<IEventSubscriptionAbi>, NcStatus> {
        self.0.register(|| {
            let sink: ComPtr<IScrollbarSinkAbi> = ScrollbarSink::from_value(ScrollbarSink { handler: self.0.delivery(handler) }).query()?;
            let lease = widget.as_interface().subscribe(sink.as_com_ref())?;
            Ok(lease)
        })
    }
}

/// Canonical generated text-and-cursor snapshot from an editor.
pub use neocogi_ui_core_contracts::ITextChangedSinkEvent as TextChanged;

/// Typed facade hook shared by built-ins and explicitly queried text-event capabilities.
pub trait TextEventSource {
    /// Connects a changed-text sink through the concrete class contract.
    fn subscribe_changed_sink(&self, sink: ComRef<'_, ITextChangedSinkAbi>) -> Result<ComPtr<IEventSubscriptionAbi>, NcStatus>;
    /// Connects a submitted-text sink through the concrete class contract.
    fn subscribe_submitted_sink(&self, sink: ComRef<'_, ITextSubmittedSinkAbi>) -> Result<ComPtr<IEventSubscriptionAbi>, NcStatus>;
}

impl TextEventSource for ComPtr<ITextEventsAbi> {
    /// External editors subscribe through the standard capability queried once during wiring.
    fn subscribe_changed_sink(&self, sink: ComRef<'_, ITextChangedSinkAbi>) -> Result<ComPtr<IEventSubscriptionAbi>, NcStatus> {
        self.as_com_ref().subscribe_changed(sink)
    }
    /// External editors reuse the same capability for their independent submission port.
    fn subscribe_submitted_sink(&self, sink: ComRef<'_, ITextSubmittedSinkAbi>) -> Result<ComPtr<IEventSubscriptionAbi>, NcStatus> {
        self.as_com_ref().subscribe_submitted(sink)
    }
}

impl TextEventSource for crate::TextboxWidget {
    /// Delegates directly to the Textbox class projection without an event-interface query.
    fn subscribe_changed_sink(&self, sink: ComRef<'_, ITextChangedSinkAbi>) -> Result<ComPtr<IEventSubscriptionAbi>, NcStatus> {
        self.as_interface().subscribe_changed(sink)
    }
    /// Delegates directly to the Textbox class projection without an event-interface query.
    fn subscribe_submitted_sink(&self, sink: ComRef<'_, ITextSubmittedSinkAbi>) -> Result<ComPtr<IEventSubscriptionAbi>, NcStatus> {
        self.as_interface().subscribe_submitted(sink)
    }
}

impl TextEventSource for crate::TextAreaWidget {
    /// Delegates directly to the TextArea class projection without an event-interface query.
    fn subscribe_changed_sink(&self, sink: ComRef<'_, ITextChangedSinkAbi>) -> Result<ComPtr<IEventSubscriptionAbi>, NcStatus> {
        self.as_interface().subscribe_changed(sink)
    }
    /// Delegates directly to the TextArea class projection without an event-interface query.
    fn subscribe_submitted_sink(&self, sink: ComRef<'_, ITextSubmittedSinkAbi>) -> Result<ComPtr<IEventSubscriptionAbi>, NcStatus> {
        self.as_interface().subscribe_submitted(sink)
    }
}

#[com_object(interfaces(ITextChangedSink))]
/// Concrete local sink specialization stores application state and its exact handler type directly.
struct TextChangedSink<State: 'static, Handler: EventHandler<State, TextChanged> + 'static> {
    /// One shared local invocation helper; no erased handler or Rust value crosses COM.
    handler: EventDelivery<State, Handler>,
}
impl<State: 'static, Handler: EventHandler<State, TextChanged> + 'static> ITextChangedSink for TextChangedSink<State, Handler> {
    /// Validates and copies the concrete foreign snapshot before invoking application behavior.
    fn changed(&self, text: NcCopyFromBufferOffset<'_, u8>, cursor: u64) -> Result<(), NcStatus> {
        let text = copied_text(text)?;
        let position = usize::try_from(cursor).map_err(|_| NC_E_INVALID_ARG)?;
        if !text.is_char_boundary(position) {
            return Err(NC_E_INVALID_ARG);
        }
        self.handler.deliver(TextChanged { text, cursor })
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
        self.0.register(|| {
            let sink: ComPtr<ITextChangedSinkAbi> = TextChangedSink::from_value(TextChangedSink { handler: self.0.delivery(handler) }).query()?;
            let lease = widget.subscribe_changed_sink(sink.as_com_ref())?;
            Ok(lease)
        })
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
        self.0.register(|| {
            let sink = create_i_text_submitted_sink_event_sink(&self.0, handler)?;
            let lease = widget.subscribe_submitted_sink(sink.as_com_ref())?;
            Ok(lease)
        })
    }
}

#[com_object(interfaces(IWindowSink))]
/// Concrete application window handler uses the same generic ownership as ordinary widget handlers.
struct WindowSink<State: 'static, Handler: EventHandler<State, WindowEvent> + 'static> {
    /// Stored application state and handler remain local to this binary.
    handler: EventDelivery<State, Handler>,
}
impl<State: 'static, Handler: EventHandler<State, WindowEvent> + 'static> IWindowSink for WindowSink<State, Handler> {
    /// Validates the complete tagged event and copied geometry before invoking application code.
    fn event(&self, event: WindowEventAbi) -> Result<(), NcStatus> {
        let event = WindowEvent::try_from(event).map_err(|_| NC_E_INVALID_ARG)?;
        match event {
            WindowEvent::GeometryChanged { rect } | WindowEvent::Minimized { rect } | WindowEvent::Maximized { rect } | WindowEvent::Restored { rect }
                if rect.width < 0 || rect.height < 0 =>
            {
                return Err(NC_E_INVALID_ARG);
            }
            _ => {}
        }
        self.handler.deliver(event)
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
        self.0.register(|| {
            let sink: ComPtr<IWindowSinkAbi> = WindowSink::from_value(WindowSink { handler: self.0.delivery(handler) }).query()?;
            let lease = window.add_ref().query::<IWindowEventsAbi>()?.subscribe(sink.as_com_ref())?;
            Ok(lease)
        })
    }
    /// Registers the actual popup's dismissal source; later visibility is read independently from this snapshot.
    pub fn subscribe_popup<Handler: EventHandler<State, PopupDismissed> + 'static>(
        &self,
        popup: ComRef<'_, IPopupAbi>,
        handler: Handler,
    ) -> Result<ComPtr<IEventSubscriptionAbi>, NcStatus> {
        self.0.register(|| {
            let sink = create_i_popup_sink_event_sink(&self.0, handler)?;
            let lease = popup.add_ref().query::<IPopupEventsAbi>()?.subscribe(sink.as_com_ref())?;
            Ok(lease)
        })
    }
}

#[com_object(interfaces(IComboChangedSink))]
/// Application-local specialization owns its concrete generated-selection handler and shared state.
struct ComboSelectionSink<State: 'static, Handler: EventHandler<State, ComboSelection> + 'static> {
    /// Shared owned invocation discipline used by every event family.
    handler: EventDelivery<State, Handler>,
}
impl<State: 'static, Handler: EventHandler<State, ComboSelection> + 'static> IComboChangedSink for ComboSelectionSink<State, Handler> {
    /// Validates the generated tagged selection before borrowing application state.
    fn changed(&self, selection: ComboSelectionAbi) -> Result<(), NcStatus> {
        let selection = ComboSelection::try_from(selection).map_err(|_| NC_E_INVALID_ARG)?;
        self.handler.deliver(selection)
    }
}
impl<State: 'static> EventSession<State> {
    /// Registers the generated selection source without claiming the header's private popup port.
    pub fn subscribe_combo_changed<Handler: EventHandler<State, ComboSelection> + 'static>(
        &self,
        combo: &crate::ComboWidget,
        handler: Handler,
    ) -> Result<ComPtr<IEventSubscriptionAbi>, NcStatus> {
        self.0.register(|| {
            let sink: ComPtr<IComboChangedSinkAbi> = ComboSelectionSink::from_value(ComboSelectionSink { handler: self.0.delivery(handler) }).query()?;
            let lease = combo.as_interface().subscribe_changed(sink.as_com_ref())?;
            Ok(lease)
        })
    }
}

impl<State: 'static> EventSession<State> {
    /// Registers a compact menu row's actual activation queue in the same deterministic sequence as widget events.
    pub fn subscribe_menu_item<Handler: EventHandler<State, Activation> + 'static>(
        &self,
        item: &crate::MenuItem,
        handler: Handler,
    ) -> Result<ComPtr<IEventSubscriptionAbi>, NcStatus> {
        self.0.register(|| {
            let sink = create_i_activation_sink_event_sink(&self.0, handler)?;
            let lease = item.as_interface().add_ref().query::<IActivationEventsAbi>()?.subscribe(sink.as_com_ref())?;
            Ok(lease)
        })
    }
}

/// One completed file-picker request; accepted native identity remains independent of display text.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FileDialogCompleted {
    /// Exact native path selected or lexically entered by the user.
    Accepted(std::path::PathBuf),
    /// The active request ended without a selected file.
    Cancelled,
}
#[com_object(interfaces(IFileDialogSink))]
/// Concrete local completion adapter owns the application's exact state and handler specialization.
struct FileDialogSink<State: 'static, Handler: EventHandler<State, FileDialogCompleted> + 'static> {
    /// Existing owned invocation helper; native Rust paths are decoded before borrowing application state.
    handler: EventDelivery<State, Handler>,
}
impl<State: 'static, Handler: EventHandler<State, FileDialogCompleted> + 'static> IFileDialogSink for FileDialogSink<State, Handler> {
    /// Rejects an incompatible encoding and preserves exact native units when constructing the local event.
    fn accepted(&self, path: ComRef<'_, INativePathAbi>) -> Result<(), NcStatus> {
        self.handler.deliver(FileDialogCompleted::Accepted(crate::path::decode(path)?))
    }
    /// Emits the concrete cancellation outcome with no fabricated empty-path sentinel.
    fn cancelled(&self) -> Result<(), NcStatus> {
        self.handler.deliver(FileDialogCompleted::Cancelled)
    }
}
impl<State: 'static> EventSession<State> {
    /// Registers the picker's actual completion queue in the same deterministic application sequence.
    pub fn subscribe_file_dialog<Handler: EventHandler<State, FileDialogCompleted> + 'static>(
        &self,
        dialog: &crate::FileDialog,
        handler: Handler,
    ) -> Result<ComPtr<IEventSubscriptionAbi>, NcStatus> {
        self.0.register(|| {
            let sink: ComPtr<IFileDialogSinkAbi> = FileDialogSink::from_value(FileDialogSink { handler: self.0.delivery(handler) }).query()?;
            let lease = dialog.as_interface().add_ref().query::<IFileDialogEventsAbi>()?.subscribe(sink.as_com_ref())?;
            Ok(lease)
        })
    }
}
impl EventPayload for FileDialogCompleted {
    type Sink = IFileDialogSinkAbi;
    /// Encodes native identity only at the boundary; lossy display text never becomes a selected path.
    fn deliver(&self, sink: ComRef<'_, Self::Sink>) -> Result<(), NcStatus> {
        match self {
            FileDialogCompleted::Accepted(path) => sink.accepted(crate::NativePath::new(path).as_interface()),
            FileDialogCompleted::Cancelled => sink.cancelled(),
        }
    }
}
