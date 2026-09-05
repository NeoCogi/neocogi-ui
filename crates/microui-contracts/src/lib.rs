//! Shared, allocation-free COM contracts for UI, platform/rendering, and assets.
//!
//! These are binary boundaries: Rust state, closures, trait objects, references
//! into implementation storage, and backend-native GPU types stay inside their
//! owning binary. Generic application conveniences live in the static facade.
//! Core ABI generation and each interface's IID/hash are checked by nc-com.

#![no_std]
#![deny(missing_docs)]

use nc_com::{ComPtr, ComRef, NcCopyFromBufferOffset, NcStatus, com_interface, com_int_enum, com_struct};

com_int_enum! {
    /// Whether preferred measurement or child placement inputs changed.
    pub enum GeometryChange: u8 {
        /// Preferred measurement and placement inputs are unchanged.
        NO_CHANGE = 0,
        /// Preferred measurement or placement inputs changed and must be invalidated.
        CHANGED = 1,
    }
}

#[com_struct]
#[derive(Debug, Default, PartialEq, Eq)]
/// Canonical copied pixel dimensions for UI contracts.
pub struct Size {
    /// Horizontal extent in pixels.
    pub width: i32,
    /// Vertical extent in pixels.
    pub height: i32,
}

#[com_struct]
/// Opaque identity of one active paint invocation, checked by the recording service.
pub struct PaintScope {
    /// Non-reused invocation number; zero never identifies an active call.
    pub invocation: u64,
}

#[com_struct]
#[derive(Debug, Default, PartialEq, Eq)]
/// Copied rectangle in the current widget's local coordinate system.
pub struct Rect {
    /// Horizontal origin.
    pub x: i32,
    /// Vertical origin.
    pub y: i32,
    /// Nonnegative horizontal extent.
    pub width: i32,
    /// Nonnegative vertical extent.
    pub height: i32,
}

#[com_interface(auto_guid = "microui.Painter:v1")]
/// Owned recording service; it never stores references into a caller's stack.
pub trait IPainter {
    /// Records a fill only while this exact invocation remains active.
    fn fill(&self, scope: PaintScope, rect: Rect, rgba: u32) -> Result<(), NcStatus>;
}

#[com_interface(auto_guid = "microui.Widget:v1")]
/// Common retained widget capability shared by all concrete widget interfaces.
pub trait IWidget {
    /// Drains sticky geometry invalidation recorded by programmatic setters.
    fn take_change(&self) -> Result<GeometryChange, NcStatus>;
    /// Records this widget's visual output within a validated paint invocation.
    fn paint(&self, scope: PaintScope, rect: Rect, painter: ComRef<'_, IPainterAbi>) -> Result<(), NcStatus>;
}

#[com_interface(auto_guid = "microui.Children:v1")]
/// Ordered child access, independent of concrete container layout policy.
pub trait IChildren {
    /// Returns the current number of direct children.
    fn child_count(&self) -> Result<u32, NcStatus>;
    /// Returns an owned reference to one direct child; topology ownership is a separate contract.
    fn child(&self, index: u32) -> Result<ComPtr<IWidgetAbi>, NcStatus>;
}

#[com_interface(auto_guid = "microui.Button:v1")]
/// Concrete button editing contract; there is no generic property bag.
pub trait IButton {
    /// Replaces the complete UTF-8 label; the logical input offset must be zero.
    fn set_label(&self, label: NcCopyFromBufferOffset<'_, u8>) -> Result<(), NcStatus>;
    /// Returns the current label as an independently owned immutable string object.
    fn label(&self) -> Result<ComPtr<nc_com::INcStringAbi>, NcStatus>;
}

#[com_interface(auto_guid = "microui.ButtonSink:v1")]
/// Concrete semantic event consumer; supplied by the application's static code.
pub trait IButtonSink {
    /// Receives an immutable submission snapshot after widget traversal releases its borrows.
    fn submitted(&self, label: ComRef<'_, nc_com::INcStringAbi>) -> Result<(), NcStatus>;
}

#[com_interface(auto_guid = "microui.EventSubscription:v1")]
/// Lease owning one queue and typed consumer; the driver retains this lease.
pub trait IEventSubscription {
    /// Delivers one detached batch and returns the number of delivered events.
    fn dispatch(&self) -> Result<u32, NcStatus>;
    /// Disconnects explicitly; final object release also disconnects without invoking user code.
    fn cancel(&self) -> Result<(), NcStatus>;
}

#[com_interface(auto_guid = "microui.ButtonEvents:v1")]
/// Separate subscription capability avoids a cycle through a button's editing interface.
pub trait IButtonEvents {
    /// Connects the exclusive subscriber and returns its ownership lease.
    fn subscribe(&self, sink: ComRef<'_, IButtonSinkAbi>) -> Result<ComPtr<IEventSubscriptionAbi>, NcStatus>;
}

#[com_interface(auto_guid = "microui.UiFactory:v1")]
/// Runtime-bound widget factory implemented by the UI plugin.
pub trait IUiFactory {
    /// Constructs an unmounted button; initialization remains concrete and discoverable.
    fn create_button(&self) -> Result<ComPtr<IButtonAbi>, NcStatus>;
}


/// An operation requires an object which has already been logically closed.
pub const UI_E_CLOSED: NcStatus = NcStatus::from_code(-1000);
/// An operation is forbidden in the current phase or would reenter a traversal.
pub const UI_E_BUSY: NcStatus = NcStatus::from_code(-1001);
/// The object/resource belongs to a different UI or renderer instance.
pub const UI_E_OWNER: NcStatus = NcStatus::from_code(-1002);
/// A retained paint/layout invocation has ended; its token never becomes valid again.
pub const UI_E_EXPIRED_SCOPE: NcStatus = NcStatus::from_code(-1003);
/// Painting requires update/layout to be synchronized after a mutation or resize.
pub const UI_E_UPDATE_REQUIRED: NcStatus = NcStatus::from_code(-1004);
