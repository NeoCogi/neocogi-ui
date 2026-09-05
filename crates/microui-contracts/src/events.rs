//! Concrete semantic event consumers and application-owned delivery leases.
use nc_com::{ComPtr, ComRef, INcStringAbi, NcStatus, com_interface};

/// Shared finite callback budget for one runtime dispatch boundary across all sources and cascade sweeps.
pub const EVENT_DISPATCH_LIMIT: u32 = 1_000_000;

#[com_interface(auto_guid = "microui.ActivationSink:v1")]
/// Concrete labelled activation consumer shared by Button, ListItem and ListBox.
pub trait IActivationSink {
    /// Receives an immutable submission snapshot after widget traversal releases its borrows.
    fn submitted(&self, label: ComRef<'_, nc_com::INcStringAbi>) -> Result<(), NcStatus>;
}

#[com_interface(auto_guid = "microui.EventSubscription:v1")]
/// Lease owning one queue and typed consumer; the driver retains this lease.
pub trait IEventSubscription {
    /// Delivers at most limit callbacks from one detached FIFO prefix; zero rejects only when events are pending.
    /// A callback error consumes that attempted event and preserves unattempted ordinary events ahead of later emissions.
    fn dispatch(&self, limit: u32) -> Result<u32, NcStatus>;
    /// Reports a live source or an undelivered terminal fact; cancelled and closed-empty leases are inactive.
    fn active(&self) -> Result<bool, NcStatus>;
    /// Disconnects explicitly; final object release also disconnects without invoking user code.
    fn cancel(&self) -> Result<(), NcStatus>;
}

#[com_interface(auto_guid = "microui.ActivationEvents:v1")]
/// Shared labelled activation source; the producer never owns the returned lease or its sink.
pub trait IActivationEvents {
    /// Connects the exclusive subscriber and returns its ownership lease.
    fn subscribe(&self, sink: ComRef<'_, IActivationSinkAbi>) -> Result<ComPtr<IEventSubscriptionAbi>, NcStatus>;
}

#[com_interface(auto_guid = "microui.CheckboxSink:v1")]
/// Committed user checkbox state; programmatic setters do not publish it.
pub trait ICheckboxSink {
    /// Receives the captured value after retained traversal releases its owned state.
    fn changed(&self, checked: bool) -> Result<(), NcStatus>;
}
#[com_interface(auto_guid = "microui.CheckboxEvents:v1")]
/// Exclusive typed subscription capability for this concrete event shape.
pub trait ICheckboxEvents {
    /// Returns an application-owned queue/sink lease; the producer retains neither callback nor application state.
    fn subscribe(&self, sink: ComRef<'_, ICheckboxSinkAbi>) -> Result<ComPtr<IEventSubscriptionAbi>, NcStatus>;
}

#[com_interface(auto_guid = "microui.NumericSink:v1")]
/// Committed finite user value shared by Slider and Number; programmatic setters do not publish it.
pub trait INumericSink {
    /// Receives the captured value after retained traversal releases its owned state.
    fn changed(&self, value: f32) -> Result<(), NcStatus>;
}
#[com_interface(auto_guid = "microui.NumericEvents:v1")]
/// Exclusive typed subscription capability for this concrete event shape.
pub trait INumericEvents {
    /// Returns an application-owned queue/sink lease; the producer retains neither callback nor application state.
    fn subscribe(&self, sink: ComRef<'_, INumericSinkAbi>) -> Result<ComPtr<IEventSubscriptionAbi>, NcStatus>;
}

#[com_interface(auto_guid = "microui.ScrollbarSink:v1")]
/// Committed clamped scrollbar offset from pointer input; programmatic setters do not publish it.
pub trait IScrollbarSink {
    /// Receives the captured value after retained traversal releases its owned state.
    fn changed(&self, offset: i32) -> Result<(), NcStatus>;
}
#[com_interface(auto_guid = "microui.ScrollbarEvents:v1")]
/// Exclusive typed subscription capability for this concrete event shape.
pub trait IScrollbarEvents {
    /// Returns an application-owned queue/sink lease; the producer retains neither callback nor application state.
    fn subscribe(&self, sink: ComRef<'_, IScrollbarSinkAbi>) -> Result<ComPtr<IEventSubscriptionAbi>, NcStatus>;
}

#[com_interface(auto_guid = "microui.TextChangedSink:v1")]
/// Complete normalized text and scalar-aligned UTF-8 byte cursor at the moment of a user edit.
pub trait ITextChangedSink {
    /// Receives an owned immutable text capability borrowed only for this synchronous delivery.
    fn changed(&self, text: ComRef<'_, INcStringAbi>, cursor: u64) -> Result<(), NcStatus>;
}
#[com_interface(auto_guid = "microui.TextSubmittedSink:v1")]
/// Complete normalized text at submission, independent of later edits.
pub trait ITextSubmittedSink {
    /// Receives the already captured submission value after traversal.
    fn submitted(&self, text: ComRef<'_, INcStringAbi>) -> Result<(), NcStatus>;
}
#[com_interface(auto_guid = "microui.TextEvents:v1")]
/// Shared editor event family; changed and submitted are independent exclusive ports.
pub trait ITextEvents {
    /// Connects one changed consumer without claiming the separate submitted port.
    fn subscribe_changed(&self, sink: ComRef<'_, ITextChangedSinkAbi>) -> Result<ComPtr<IEventSubscriptionAbi>, NcStatus>;
    /// Connects one submitted consumer without claiming the separate changed port.
    fn subscribe_submitted(&self, sink: ComRef<'_, ITextSubmittedSinkAbi>) -> Result<ComPtr<IEventSubscriptionAbi>, NcStatus>;
}

#[com_interface(auto_guid = "microui.EventDispatcher:v1")]
/// Borrowed application-owned dispatcher; the UI runtime must never retain this capability.
pub trait IEventDispatcher {
    /// Sweeps registered sources in deterministic order until empty or the callback budget is exhausted.
    fn dispatch(&self, limit: u32) -> Result<u32, NcStatus>;
}

#[nc_com::com_enum(tag = u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
/// Captured window lifecycle or user chrome interaction; Destroyed is the one terminal fact.
pub enum WindowEvent {
    /// A user move or resize committed this complete outer rectangle.
    GeometryChanged {
        /// Captured outer geometry, independent of subsequent window edits.
        rect: crate::Rect,
    },
    /// The user activated the close affordance; the application decides whether to close.
    CloseRequested,
    /// Visibility became false; later callback code may observe a subsequent show.
    Hidden,
    /// The window and its descendants were logically destroyed; delivered once after producer closure.
    Destroyed,
    /// The user minimized to this complete compact rectangle.
    Minimized {
        /// Captured compact outer geometry.
        rect: crate::Rect,
    },
    /// The user maximized into this inherited viewport rectangle.
    Maximized {
        /// Captured maximized outer geometry.
        rect: crate::Rect,
    },
    /// The user restored this previously saved rectangle.
    Restored {
        /// Captured restored outer geometry.
        rect: crate::Rect,
    },
}
#[com_interface(auto_guid = "microui.WindowSink:v1")]
/// Concrete consumer of copied window events, including the terminal Destroyed fact.
pub trait IWindowSink {
    /// Receives the exact committed lifecycle/chrome snapshot after the manager releases traversal ownership.
    fn event(&self, event: WindowEventAbi) -> Result<(), NcStatus>;
}
#[com_interface(auto_guid = "microui.WindowEvents:v1")]
/// Subscription capability on the actual logical window object; the manager owns its existing native source.
pub trait IWindowEvents {
    /// Connects the current source without making a retained window reference prolong native tree ownership.
    fn subscribe(&self, sink: ComRef<'_, IWindowSinkAbi>) -> Result<ComPtr<IEventSubscriptionAbi>, NcStatus>;
}
#[com_interface(auto_guid = "microui.PopupSink:v1")]
/// Concrete consumer of ordinary popup dismissal; destruction does not fabricate replay history.
pub trait IPopupSink {
    /// Reports removal from the active popup branch, independently of later activation.
    fn dismissed(&self) -> Result<(), NcStatus>;
}
#[com_interface(auto_guid = "microui.PopupEvents:v1")]
/// Subscription capability on the actual popup object and its existing manager-owned source.
pub trait IPopupEvents {
    /// Returns an exclusive application-owned dismissal lease without retaining application state on the producer.
    fn subscribe(&self, sink: ComRef<'_, IPopupSinkAbi>) -> Result<ComPtr<IEventSubscriptionAbi>, NcStatus>;
}

#[com_interface(auto_guid = "microui.ComboChangedSink:v1")]
/// Captured user selection committed by a combo's composing popup owner.
pub trait IComboChangedSink {
    /// Receives the matching application-list position and display label from one selection.
    fn changed(&self, selected: u64, label: ComRef<'_, INcStringAbi>) -> Result<(), NcStatus>;
}
#[com_interface(auto_guid = "microui.ComboPopupSink:v1")]
/// Header activation requests ordinary popup coordination using copied routed geometry.
pub trait IComboPopupSink {
    /// Receives the desired open state and screen-space anchor captured at header activation.
    fn submitted(&self, open: bool, anchor: crate::Rect) -> Result<(), NcStatus>;
}
#[com_interface(auto_guid = "microui.ComboEvents:v1")]
/// Independent selection and popup-coordination event ports on the actual combo header.
pub trait IComboEvents {
    /// Connects one concrete selection consumer without claiming popup coordination.
    fn subscribe_changed(&self, sink: ComRef<'_, IComboChangedSinkAbi>) -> Result<ComPtr<IEventSubscriptionAbi>, NcStatus>;
    /// Connects one concrete popup coordinator without claiming the changed source.
    fn subscribe_popup(&self, sink: ComRef<'_, IComboPopupSinkAbi>) -> Result<ComPtr<IEventSubscriptionAbi>, NcStatus>;
}
