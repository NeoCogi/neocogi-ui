//! Shared, allocation-free COM contracts for UI, platform/rendering, and assets.
//!
//! These are binary boundaries: Rust state, closures, trait objects, references
//! into implementation storage, and backend-native GPU types stay inside their
//! owning binary. Generic application conveniences live in the static facade.
//! Core ABI generation and each interface's IID/hash are checked by nc-com.

#![no_std]
#![deny(missing_docs)]

pub mod widget;
pub use widget::{WidgetOptions, WidgetProperties, WidgetFrame};
mod input;
pub use input::{
    KeyCode, KeyStroke, Point, WidgetEvent, WidgetEventAbi, WidgetEventTag, WidgetUpdate, WidgetAction, decode_widget_event, UiInput, UiInputAbi, UiInputTag,
};

use nc_com::{ComPtr, ComRef, NcCopyFromBufferOffset, NcCopyToBufferOffset, NcStatus, com_interface, com_int_enum, com_struct};

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
    /// Records UTF-8 text with the current body font and button content style.
    fn text(&self, scope: PaintScope, rect: Rect, text: NcCopyFromBufferOffset<'_, u8>) -> Result<(), NcStatus>;
}

#[com_interface(auto_guid = "microui.Widget:v1")]
/// Common retained widget capability shared by all concrete widget interfaces.
pub trait IWidget {
    /// Returns the allocating runtime's identity service, which owns no tree.
    fn host(&self) -> Result<ComPtr<IWidgetHostAbi>, NcStatus>;
    /// Claims the only tree attachment; a second call fails without changing state.
    fn mount(&self) -> Result<(), NcStatus>;
    /// Logically closes the widget without requiring its final reference release.
    fn close(&self) -> Result<(), NcStatus>;

    /// Returns current frame, focus and pointer-routing policy as one copied record.
    fn properties(&self) -> Result<WidgetProperties, NcStatus>;
    /// Changes common options and records sticky geometry invalidation when they differ.
    fn set_options(&self, options: WidgetOptions) -> Result<(), NcStatus>;
    /// Processes one router-owned snapshot; the text argument is UTF-8 only for Text events.
    /// Return CHANGED for every modification to measurement or child placement inputs.
    fn update(&self, state: WidgetUpdate, event: WidgetEventAbi, text: NcCopyFromBufferOffset<'_, u8>) -> Result<GeometryChange, NcStatus>;

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
// The base slots belong to this same object. An inherited call borrows the
// button projection; passing a separate IWidget argument requires querying it.
pub trait IButton: IWidget {
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

#[com_interface(auto_guid = "microui.TextMetrics:v1")]
/// Immutable resource view shared for a complete layout transaction.
pub trait ITextMetrics {
    /// Measures a UTF-8 button label, including theme padding.
    fn button_size(&self, text: NcCopyFromBufferOffset<'_, u8>) -> Result<Size, NcStatus>;
}

com_int_enum! {
    /// Whether an axis has a finite measurement maximum.
    pub enum LimitKind: u8 {
        /// Extent is a nonnegative maximum; zero remains a real bound.
        FINITE = 0,
        /// Preferred size is requested without a finite maximum; extent must be zero.
        UNBOUNDED = 1,
    }
}

#[com_struct]
/// Copied axis constraint; the tag distinguishes zero from unbounded space.
pub struct AxisLimit {
    /// Explicit finite/unbounded interpretation.
    pub kind: LimitKind,
    /// Nonnegative bound for FINITE, zero for UNBOUNDED.
    pub extent: i32,
}

impl AxisLimit {
    /// Rejects malformed tags, negative bounds, and meaningless unbounded extents.
    pub fn validate(self) -> Result<(), NcStatus> {
        match self.kind {
            LimitKind::FINITE if self.extent >= 0 => Ok(()),
            LimitKind::UNBOUNDED if self.extent == 0 => Ok(()),
            _ => Err(nc_com::NC_E_INVALID_ARG),
        }
    }
}

#[com_struct]
/// Independent horizontal and vertical measurement constraints.
pub struct LayoutLimits {
    /// Available horizontal space.
    pub width: AxisLimit,
    /// Available vertical space.
    pub height: AxisLimit,
}

#[com_interface(auto_guid = "microui.LeafLayout:v1")]
/// Preferred-size capability for a leaf; measurement must not mutate UI state.
pub trait ILeafLayout {
    /// Computes preferred dimensions from explicit finite/unbounded axis constraints.
    fn measure(&self, bounds: LayoutLimits, metrics: ComRef<'_, ITextMetricsAbi>) -> Result<Size, NcStatus>;
}

#[com_struct]
#[derive(Debug, Default)]
/// Final atlas-backed vertex; tessellation and clipping already happened in the UI.
pub struct RenderVertex {
    /// Horizontal position in drawable pixels.
    pub x: f32,
    /// Vertical position in drawable pixels.
    pub y: f32,
    /// Normalized horizontal atlas coordinate.
    pub u: f32,
    /// Normalized vertical atlas coordinate.
    pub v: f32,
    /// Color packed as red in the least significant byte, then green, blue, alpha.
    pub rgba: u32,
}

#[com_struct]
/// Metadata for one synchronous renderer submission.
pub struct RenderInfo {
    /// Positive drawable dimensions.
    pub size: Size,
    /// Clear color in the same byte order as RenderVertex::rgba.
    pub clear: u32,
    /// Unique authority to read this frame's stream; expires when render returns.
    pub invocation: u64,
    /// Exact vertex count, always a multiple of three.
    pub vertex_count: u32,
}

#[com_interface(auto_guid = "microui.RenderStream:v1")]
/// Owned reusable vertex storage, readable only during its active submission.
pub trait IRenderStream {
    /// Copies a bounded range into caller-owned storage; offsets count vertices.
    fn vertices(&self, invocation: u64, output: NcCopyToBufferOffset<'_, RenderVertex>) -> Result<u32, NcStatus>;
}

#[com_interface(auto_guid = "microui.Renderer:v1")]
/// GPU resources and presentation belong to this object's native window.
pub trait IRenderer {
    /// Executes one complete frame synchronously; failure must not present it.
    fn render(&self, info: RenderInfo, stream: ComRef<'_, IRenderStreamAbi>) -> Result<(), NcStatus>;
}

com_int_enum! {
    /// Platform event kind; input stays explicit rather than using an erased payload.
    pub enum PlatformEventKind: u8 {
        /// No event is available.
        NONE = 0,
        /// The user requested application exit.
        QUIT = 1,
        /// Pointer motion in drawable coordinates.
        POINTER_MOVE = 2,
        /// Primary pointer button pressed.
        POINTER_DOWN = 3,
        /// Primary pointer button released.
        POINTER_UP = 4,
        /// Drawable size changed; the host must update before drawing again.
        RESIZE = 5,
        /// The focused control's primary keyboard action was requested.
        ACTIVATE = 6,
        /// Advance focus to the next tab stop.
        TAB = 7,
    }
}

#[com_struct]
/// One copied platform event; x/y are pointer coordinates or resize dimensions.
pub struct PlatformEvent {
    /// Explicit interpretation of the remaining fields.
    pub kind: PlatformEventKind,
    /// Horizontal pointer coordinate or drawable width.
    pub x: i32,
    /// Vertical pointer coordinate or drawable height.
    pub y: i32,
}

#[com_interface(auto_guid = "microui.PlatformWindow:v1")]
/// Native window, event pump and renderer share one COM identity and lifetime.
pub trait IPlatformWindow {
    /// Returns current drawable dimensions, including device scaling.
    fn size(&self) -> Result<Size, NcStatus>;
    /// Removes one queued native event without calling application code.
    fn poll(&self) -> Result<PlatformEvent, NcStatus>;
}

#[com_interface(auto_guid = "microui.Platform:v1")]
/// Platform plugin's concrete window factory.
pub trait IPlatform {
    /// Creates an independent native window and renderer; title is UTF-8 at offset zero.
    fn create_window(&self, title: NcCopyFromBufferOffset<'_, u8>, size: Size) -> Result<ComPtr<IPlatformWindowAbi>, NcStatus>;
}

com_int_enum! {
    /// Visibility intent or effective surface visibility, as documented by the operation.
    pub enum Visibility: u8 {
        /// Excluded from rendering and input.
        HIDDEN = 0,
        /// Eligible when structural and modal owner policy also permits it.
        VISIBLE = 1,
    }
}

com_int_enum! {
    /// How a parent clips its structural child windows.
    pub enum ChildClip: u8 {
        /// Inherit the ancestor/viewport clip without narrowing to the parent's body.
        NONE = 0,
        /// Clip complete child surfaces to the parent's application content area.
        CONTENT = 1,
    }
}

com_int_enum! {
    /// Structural layer category for a window or dialog.
    pub enum LayerKind: u8 {
        /// An independent or inherited application layer in 0..=15.
        FIXED = 0,
        /// Dedicated modal band above all application layers; number is zero.
        MODAL = 1,
    }
}

#[com_struct]
#[derive(Debug, PartialEq, Eq)]
/// Explicit fixed or manager-controlled modal stacking band.
pub struct WindowLayer {
    /// Interpretation of the numeric field.
    pub kind: LayerKind,
    /// Fixed application layer, or zero when modal.
    pub number: u8,
}

#[com_struct]
#[derive(Debug, PartialEq, Eq)]
/// Concrete window presentation bits; unknown bits are rejected before mutation.
pub struct WindowOptions {
    /// Union of the documented constants on this type.
    pub bits: u32,
}

impl WindowOptions {
    /// No optional presentation behavior.
    pub const NONE: Self = Self { bits: 0 };
    /// Show the minimize caption button.
    pub const MINIMIZE_BUTTON: Self = Self { bits: 1 };
    /// Show the maximize caption button.
    pub const MAXIMIZE_BUTTON: Self = Self { bits: 2 };
    /// Keep visible while disabling the complete structural family.
    pub const DISABLED: Self = Self { bits: 4 };
    /// Remove the ordinary content inset.
    pub const NO_PADDING: Self = Self { bits: 8 };
    /// Prevent interactive resizing.
    pub const NO_RESIZE: Self = Self { bits: 16 };
    /// Hide the close caption button.
    pub const NO_CLOSE: Self = Self { bits: 64 };
    /// Hide the title bar.
    pub const NO_TITLE: Self = Self { bits: 128 };
    /// Derive width from preferred content measurement.
    pub const AUTO_WIDTH: Self = Self { bits: 256 };
    /// Derive height from preferred content measurement.
    pub const AUTO_HEIGHT: Self = Self { bits: 512 };
    /// Derive both dimensions from preferred content measurement.
    pub const AUTO_SIZE: Self = Self { bits: 768 };
    /// Use the theme-owned outer border and content inset.
    pub const FRAME: Self = Self { bits: 1024 };

    /// Every declared presentation bit; aliases such as AUTO_SIZE introduce no new bits.
    const KNOWN_BITS: u32 = Self::MINIMIZE_BUTTON.bits
        | Self::MAXIMIZE_BUTTON.bits
        | Self::DISABLED.bits
        | Self::NO_PADDING.bits
        | Self::NO_RESIZE.bits
        | Self::NO_CLOSE.bits
        | Self::NO_TITLE.bits
        | Self::AUTO_WIDTH.bits
        | Self::AUTO_HEIGHT.bits
        | Self::FRAME.bits;

    /// Validates the complete bit set at the component boundary.
    pub fn validate(self) -> Result<(), NcStatus> {
        if self.bits & !Self::KNOWN_BITS == 0 {
            Ok(())
        } else {
            Err(nc_com::NC_E_INVALID_ARG)
        }
    }
}

impl core::ops::BitOr for WindowOptions {
    type Output = Self;
    /// Combines concrete options without exposing a typeless property collection.
    fn bitor(self, rhs: Self) -> Self {
        Self { bits: self.bits | rhs.bits }
    }
}

#[com_struct]
#[derive(Debug, PartialEq, Eq)]
/// Immutable copied observation of the manager's authoritative window state.
pub struct WindowSnapshot {
    /// Current outer screen rectangle.
    pub rect: Rect,
    /// Current presentation options.
    pub options: WindowOptions,
    /// Effective visibility including structural ancestors.
    pub visibility: Visibility,
    /// Fixed, inherited or modal stacking policy.
    pub layer: WindowLayer,
    /// Parent-owned clipping applied to this window's structural children.
    pub child_clip: ChildClip,
}

#[com_interface(auto_guid = "microui.Window:v1")]
/// Logical UI window retained independently of its runtime's tree membership.
pub trait IWindow {
    /// Copies current authoritative geometry, visibility and policy without caching app-side state.
    fn snapshot(&self) -> Result<WindowSnapshot, NcStatus>;
    /// Returns an immutable UTF-8 title snapshot.
    fn title(&self) -> Result<ComPtr<nc_com::INcStringAbi>, NcStatus>;
    /// Replaces the complete title silently; UTF-8 input offset must be zero.
    fn set_title(&self, title: NcCopyFromBufferOffset<'_, u8>) -> Result<(), NcStatus>;
    /// Changes local visibility intent; ancestors and modal policy determine effective visibility.
    fn set_visibility(&self, visibility: Visibility) -> Result<(), NcStatus>;
    /// Changes concrete presentation options and reconciles capture/disabled descendants.
    fn set_options(&self, options: WindowOptions) -> Result<(), NcStatus>;
    /// Selects a fixed layer in 0..=15; child/dialog layers are manager-controlled.
    fn set_layer(&self, layer: u8) -> Result<(), NcStatus>;
    /// Raises within the permitted structural stacking band.
    fn raise(&self) -> Result<(), NcStatus>;
    /// Changes the clipping applied to structural child windows.
    fn set_child_clip(&self, clip: ChildClip) -> Result<(), NcStatus>;
    /// Changes allocation and schedules layout; negative extents are rejected.
    fn set_rect(&self, rect: Rect) -> Result<(), NcStatus>;
    /// Removes the subtree; retained capabilities subsequently report closure.
    fn close(&self) -> Result<(), NcStatus>;
}

#[com_interface(auto_guid = "microui.WindowManager:v1")]
/// Window policy and topology are owned by the UI runtime object.
pub trait IWindowManager {
    /// Creates a visible structural child; a modal parent is rejected before claiming the widget.
    fn create_child(
        &self,
        parent: ComRef<'_, IWindowAbi>,
        title: NcCopyFromBufferOffset<'_, u8>,
        rect: Rect,
        root: ComRef<'_, IWidgetAbi>,
    ) -> Result<ComPtr<IWindowAbi>, NcStatus>;
    /// Creates a hidden modal dialog owned by an ordinary window.
    fn create_dialog(
        &self,
        parent: ComRef<'_, IWindowAbi>,
        title: NcCopyFromBufferOffset<'_, u8>,
        rect: Rect,
        root: ComRef<'_, IWidgetAbi>,
    ) -> Result<ComPtr<IWindowAbi>, NcStatus>;
    /// Creates a hidden auto-sized popup owned by a live window or dialog.
    fn create_popup(
        &self,
        parent: ComRef<'_, IWindowAbi>,
        title: NcCopyFromBufferOffset<'_, u8>,
        root: ComRef<'_, IWidgetAbi>,
    ) -> Result<ComPtr<IPopupAbi>, NcStatus>;
    /// Mounts one previously unmounted root and creates its logical window.
    fn create_window(&self, title: NcCopyFromBufferOffset<'_, u8>, rect: Rect, root: ComRef<'_, IWidgetAbi>) -> Result<ComPtr<IWindowAbi>, NcStatus>;
}

#[com_interface(auto_guid = "microui.UiRuntime:v1")]
/// One independent retained UI, also exposing its widget factory/window manager.
pub trait IUiRuntime {
    /// Processes one input event; NONE still synchronizes setters and resize.
    fn update(&self, size: Size) -> Result<(), NcStatus>;
    /// Queues one copied host event; Text borrows complete UTF-8 bytes only for this call.
    /// The next update applies queued events in order and synchronizes each input transaction.
    fn input(&self, event: UiInputAbi, text: NcCopyFromBufferOffset<'_, u8>) -> Result<(), NcStatus>;
    /// Paints only after update has synchronized geometry for this drawable size.
    fn render(&self, size: Size, renderer: ComRef<'_, IRendererAbi>) -> Result<(), NcStatus>;
    /// Logically closes every mounted subtree; COM allocations may outlive closure.
    fn close(&self) -> Result<(), NcStatus>;
}

#[com_interface(auto_guid = "microui.UiProvider:v1")]
/// UI plugin bootstrap; ordinary widgets are created by each returned runtime.
pub trait IUiProvider {
    /// Creates an independent UI with this plugin assembly's default resource bundle.
    fn create_runtime(&self) -> Result<ComPtr<IUiRuntimeAbi>, NcStatus>;
}

/// Stable activation identity independent of Rust module paths.
pub const UI_PROVIDER_CLASS: nc_com::NcGuid = nc_com::guid_from_qualified_name("microui.UiProvider");
/// Stable platform activation identity independent of the selected native backend.
pub const PLATFORM_CLASS: nc_com::NcGuid = nc_com::guid_from_qualified_name("microui.Platform");
/// Backend failure; diagnostics belong to the platform implementation.
pub const UI_E_RENDER: NcStatus = NcStatus::from_code(-1005);

#[com_interface(auto_guid = "microui.WidgetHost:v1")]
/// Runtime identity and mutation authority without a reference back to its tree.
pub trait IWidgetHost {
    /// Checks lifetime without forbidding observational reads during layout or painting.
    fn check_open(&self) -> Result<(), NcStatus>;
    /// Rejects closed runtimes and mutation during layout or painting.
    fn check_mutation(&self) -> Result<(), NcStatus>;
}

#[com_interface(auto_guid = "microui.Popup:v1")]
/// A popup has transient placement and cannot be passed to ordinary window operations.
pub trait IPopup {
    /// Reports whether this popup belongs to the manager's active visible branch.
    fn visibility(&self) -> Result<Visibility, NcStatus>;
    /// Atomically sets the screen anchor and activates the popup's input scope.
    fn show(&self, anchor: Rect) -> Result<(), NcStatus>;
    /// Dismisses this active branch while preserving the definition for later reuse.
    fn hide(&self) -> Result<(), NcStatus>;
    /// Changes popup framing, padding and preferred-size policy.
    fn set_options(&self, options: WindowOptions) -> Result<(), NcStatus>;
    /// Permanently removes this popup subtree and closes its retained widgets.
    fn close(&self) -> Result<(), NcStatus>;
}

/// A live surface rejects the requested structural, modal or managed-layer operation.
pub const UI_E_POLICY: NcStatus = NcStatus::from_code(-1006);
