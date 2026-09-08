//! Shared, allocation-free COM contracts for UI, platform/rendering, and assets.
//!
//! These are binary boundaries: Rust state, closures, trait objects, references
//! into implementation storage, and backend-native GPU types stay inside their
//! owning binary. Generic application conveniences live in the static facade.
//! Core ABI generation and each interface's IID/hash are checked by nc-com.

#![no_std]
#![deny(missing_docs)]

pub mod appearance;
mod assets;
pub use assets::*;
pub mod widget;
pub use widget::{WidgetOptions, WidgetProperties, WidgetFrame, WidgetPaint};
mod container;
pub use container::*;
mod events;
pub use events::*;
mod file_dialog;
pub use file_dialog::*;
mod menu;
pub use menu::*;
mod render;
pub use render::*;
mod path;
pub use path::*;
mod leaves;
pub use leaves::*;
mod platform;
pub use platform::*;
mod input;
pub use input::{
    KeyCode, KeyStroke, Point, WidgetEvent, WidgetEventAbi, WidgetEventTag, WidgetUpdate, WidgetAction, decode_widget_event, UiInput, UiInputAbi, UiInputTag,
};

use nc_com::{ComPtr, ComRef, NcCopyFromBufferOffset, NcStatus, com_class, com_interface, com_int_enum, com_struct};

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
/// Active paint invocation plus a caller-narrowable local clip, checked by the recording service.
pub struct PaintScope {
    /// Non-reused invocation number; zero never identifies an active call.
    pub invocation: u64,
    /// Requested local clip. The service always intersects it with the widget's actual clip;
    /// modifying this copied field cannot grant drawing authority outside the widget.
    pub clip: Rect,
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
    /// Records a nonnegative-width solid line with finite local endpoints. Zero width draws nothing.
    fn stroke_line(&self, scope: PaintScope, from: PaintPoint, to: PaintPoint, width: f32, rgba: u32) -> Result<(), NcStatus>;
    /// Records finite local polygon points at offset zero; fewer than three points draw nothing.
    /// Concave polygon tessellation and clipping use the same UI implementation as built-in drawing.
    fn fill_polygon(&self, scope: PaintScope, points: NcCopyFromBufferOffset<'_, PaintPoint>, rgba: u32) -> Result<(), NcStatus>;
    /// Records solid triangles in widget-local coordinates. The complete input starts at offset zero,
    /// has a multiple of three vertices and contains only finite positions. Atlas mapping belongs
    /// to the UI implementation and follows the currently installed resource revision.
    fn colored_triangles(&self, scope: PaintScope, vertices: NcCopyFromBufferOffset<'_, ColoredVertex>) -> Result<(), NcStatus>;
    /// Records complete atlas triangles in local coordinates; offset is zero,
    /// length is divisible by three, and every position/UV component is finite.
    fn triangles(&self, scope: PaintScope, vertices: NcCopyFromBufferOffset<'_, RenderVertex>) -> Result<(), NcStatus>;
    /// Retains an immutable renderer-owned RGBA texture and draws its normalized source region in local coordinates.
    /// Negative extents are invalid; zero extents draw nothing. Ownership is validated before GPU execution.
    fn texture(&self, scope: PaintScope, rect: Rect, source: TextureRegion, rgba: u32, texture: ComRef<'_, ITextureAbi>) -> Result<(), NcStatus>;
    /// Retains a renderer-owned custom item at this exact painter-order position.
    /// The UI translates the rectangle and intersects its clip before submission.
    fn item(&self, scope: PaintScope, rect: Rect, item: ComRef<'_, IRenderItemAbi>) -> Result<(), NcStatus>;
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
    fn paint(&self, scope: PaintScope, state: WidgetPaint, painter: ComRef<'_, IPainterAbi>) -> Result<(), NcStatus>;
}

#[com_interface(auto_guid = "microui.Children:v1")]
/// Ordered child access, independent of concrete container layout policy.
pub trait IChildren {
    /// Returns the current number of direct children.
    fn child_count(&self) -> Result<u32, NcStatus>;
    /// Returns an owned reference to one direct child; topology ownership is a separate contract.
    fn child(&self, index: u32) -> Result<ComPtr<IWidgetAbi>, NcStatus>;
}

#[com_class(auto_guid = "microui.Button:v1")]
/// Constructible button class with only its shared widget and leaf-layout protocols separated.
pub trait Button: IWidget + ILeafLayout {
    /// Creates an unmounted button owned by `host` from complete initial state.
    #[constructor]
    fn new(
        host: ComRef<'_, IWidgetHostAbi>,
        label: NcCopyFromBufferOffset<'_, u8>,
        options: WidgetOptions,
    ) -> Result<Self, NcStatus>;

    /// Replaces the complete UTF-8 label; the logical input offset must be zero.
    fn set_label(&self, label: NcCopyFromBufferOffset<'_, u8>) -> Result<(), NcStatus>;

    /// Returns the current label as an independently owned immutable string object.
    fn label(&self) -> Result<ComPtr<nc_com::INcStringAbi>, NcStatus>;

    /// Connects the exclusive activation subscriber and returns its ownership lease.
    fn subscribe(&self, sink: ComRef<'_, IActivationSinkAbi>) -> Result<ComPtr<IEventSubscriptionAbi>, NcStatus>;

    /// Selects a semantic font role and invalidates measurement only when it differs.
    fn set_font_role(&self, role: TextFontRole) -> Result<(), NcStatus>;

    /// Resolves an exact named font in the current host bundle before committing.
    fn set_named_font(&self, name: NcCopyFromBufferOffset<'_, u8>) -> Result<(), NcStatus>;

    /// Selects a semantic icon or removes it while retaining the label.
    fn set_icon_role(&self, icon: ContentIcon) -> Result<(), NcStatus>;

    /// Resolves an exact named icon in the current host bundle before committing.
    fn set_named_icon(&self, name: NcCopyFromBufferOffset<'_, u8>) -> Result<(), NcStatus>;

    /// Returns the interaction states that fill an unframed button's center.
    fn fill_states(&self) -> Result<FillStates, NcStatus>;

    /// Replaces the validated paint-only center-fill mask.
    fn set_fill_states(&self, states: FillStates) -> Result<(), NcStatus>;

    /// Installs a positive-sized renderer-owned image, optionally fitted to width.
    fn set_image(&self, image: ComRef<'_, ITextureAbi>, fit_width: bool) -> Result<(), NcStatus>;

    /// Removes image content while retaining the current label and font.
    fn clear_image(&self) -> Result<(), NcStatus>;
}

#[com_class(auto_guid = "microui.UiFactory:v1")]
/// Dynamically implemented construction shorthand over registered widget contracts.
pub trait UiFactory {
    /// Creates a factory bound to one non-owning UI host authority.
    #[constructor]
    fn new(host: ComRef<'_, IWidgetHostAbi>) -> Result<Self, NcStatus>;

    /// Returns this factory's non-owning authority for inline WidgetState in independent widget classes.
    fn host(&self) -> Result<ComPtr<IWidgetHostAbi>, NcStatus>;
    /// Creates an empty combo header; its composing owner supplies generic list data and popup content.
    fn create_combo(&self) -> Result<ComPtr<ComboAbi>, NcStatus>;
    /// Creates a fixed scrolling multiline editor; CRLF and CR normalize to LF and the cursor starts at the end.
    fn create_text_area(&self, text: NcCopyFromBufferOffset<'_, u8>, wrapping: TextWrapping, scrolling: bool) -> Result<ComPtr<TextAreaAbi>, NcStatus>;
    /// Creates an empty disclosure with a real text header and editable vertical Linear body.
    fn create_disclosure(&self, label: NcCopyFromBufferOffset<'_, u8>, style: DisclosureStyle, expanded: bool) -> Result<ComPtr<IDisclosureAbi>, NcStatus>;
    /// Creates a framed scroll composite around one unmounted content object; generic options can subsequently change its frame.
    fn create_scroll_area(&self, content: ComRef<'_, IWidgetAbi>, enabled: bool) -> Result<ComPtr<IScrollAreaAbi>, NcStatus>;
    /// Creates an empty directional container after validating its complete initial policy.
    fn create_linear(&self, direction: FlowDirection, cross: CrossSizeAbi) -> Result<ComPtr<LinearAbi>, NcStatus>;
    /// Creates an empty Grid with validated complete column and row policies; input offsets must be zero.
    fn create_grid(
        &self,
        columns: NcCopyFromBufferOffset<'_, LayoutTrackAbi>,
        rows: NcCopyFromBufferOffset<'_, LayoutTrackAbi>,
    ) -> Result<ComPtr<GridAbi>, NcStatus>;
    /// Constructs an unmounted boolean control with body-font presentation.
    fn create_checkbox(&self, checked: bool, label: NcCopyFromBufferOffset<'_, u8>) -> Result<ComPtr<CheckboxAbi>, NcStatus>;
    /// Constructs an unmounted normalized single-line editor.
    fn create_textbox(&self, text: NcCopyFromBufferOffset<'_, u8>) -> Result<ComPtr<TextboxAbi>, NcStatus>;
    /// Constructs multiline display text using the explicit wrapping policy.
    fn create_text_block(&self, text: NcCopyFromBufferOffset<'_, u8>, wrapping: TextWrapping) -> Result<ComPtr<TextBlockAbi>, NcStatus>;
    /// Constructs a slider after validating every arithmetic input.
    fn create_slider(&self, options: SliderOptions) -> Result<ComPtr<SliderAbi>, NcStatus>;
    /// Constructs standalone numeric entry after validating arithmetic inputs.
    fn create_number(&self, options: NumberOptions) -> Result<ComPtr<NumberAbi>, NcStatus>;
    /// Constructs an RGBA swatch with an optional label.
    fn create_color_swatch(&self, rgba: u32, label: NcCopyFromBufferOffset<'_, u8>) -> Result<ComPtr<ColorSwatchAbi>, NcStatus>;
    /// Constructs one list row; application data is attached in its static adapter.
    fn create_list_item(&self, label: NcCopyFromBufferOffset<'_, u8>) -> Result<ComPtr<ListItemAbi>, NcStatus>;
    /// Constructs a button-like list entry.
    fn create_list_box(&self, label: NcCopyFromBufferOffset<'_, u8>) -> Result<ComPtr<ListBoxAbi>, NcStatus>;
    /// Constructs a scrollbar with a validated orientation and normalized range.
    fn create_scrollbar(&self, range: ScrollRange) -> Result<ComPtr<ScrollbarAbi>, NcStatus>;
    /// Constructs an initialized, unmounted Button through its concrete class contract.
    fn create_button(
        &self,
        label: NcCopyFromBufferOffset<'_, u8>,
        options: WidgetOptions,
    ) -> Result<ComPtr<ButtonAbi>, NcStatus>;
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
/// Backend failure; diagnostics belong to the platform implementation.
pub const UI_E_RENDER: NcStatus = NcStatus::from_code(-1005);
/// A live surface rejects the requested structural, modal or managed-layer operation.
pub const UI_E_POLICY: NcStatus = NcStatus::from_code(-1006);
/// A bounded event transaction exhausted its callback budget; undispatched queued events remain pending.
pub const UI_E_EVENT_LIMIT: NcStatus = NcStatus::from_code(-1007);
/// A native file/directory operation failed; the component's diagnostic preserves the concrete OS error.
pub const UI_E_IO: NcStatus = NcStatus::from_code(-1008);

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

#[com_class(auto_guid = "microui.UiRuntime:v1")]
/// One constructible retained UI with input, rendering, and resource ownership.
pub trait UiRuntime: IInputSink + IWindowManager + IMenus + IFileDialogs {
    /// Imports one immutable asset bundle into a new independent UI.
    #[constructor]
    fn new(assets: ComRef<'_, IAssetBundleAbi>) -> Result<Self, NcStatus>;

    /// Returns the dynamically implemented construction shorthand bound to this UI.
    fn factory(&self) -> Result<ComPtr<UiFactoryAbi>, NcStatus>;

    /// Retains the currently installed immutable revision, suitable for restoration or another consumer.
    fn assets(&self) -> Result<ComPtr<IAssetBundleAbi>, NcStatus>;
    /// Validates compatible resources and prepares the renderer upload before publishing either UI
    /// projection. Failure preserves the old bundle. Success requires update before rendering.
    /// Application event handlers may install at the dispatch boundary; the enclosing update then
    /// commits layout. Widget update/layout/paint and upload reentry reject with BUSY. The renderer
    /// is borrowed only for this call.
    fn install_assets(&self, assets: ComRef<'_, IAssetBundleAbi>, renderer: ComRef<'_, IRendererAbi>) -> Result<(), NcStatus>;
    /// Drains queued input, or one eventless update, and synchronizes setters/resize without an application dispatcher.
    fn update(&self, size: Size) -> Result<(), NcStatus>;
    /// Borrows application dispatch at each safe boundary; handlers may mutate windows/topology before the next input is routed.
    /// The runtime never retains dispatcher; update/render reentry returns BUSY even while the context is available to handlers.
    fn update_with_events(&self, size: Size, dispatcher: ComRef<'_, IEventDispatcherAbi>) -> Result<(), NcStatus>;
    /// Returns the committed editable focus rectangle in drawable coordinates, or None while
    /// geometry/input is pending. Hosts synchronize native IME after each successful update.
    fn text_input_area(&self) -> Result<TextInputAreaAbi, NcStatus>;
    /// Paints only after update has synchronized geometry for this drawable size.
    fn render(&self, size: Size, renderer: ComRef<'_, IRendererAbi>) -> Result<(), NcStatus>;
    /// Logically closes every mounted subtree; COM allocations may outlive closure.
    fn close(&self) -> Result<(), NcStatus>;
}

/// Stable platform activation identity independent of the selected native backend.
pub const PLATFORM_CLASS: nc_com::NcGuid = nc_com::guid_from_qualified_name("microui.Platform");

#[com_interface(auto_guid = "microui.WidgetHost:v1")]
/// Runtime identity and mutation authority without a reference back to its tree.
pub trait IWidgetHost {
    /// Retains the current immutable resource revision without owning the runtime tree.
    /// Custom widgets cache bulk metadata by this bundle identity and refresh it during the next
    /// measure/paint after replacement; never retain UVs or glyph metrics across a different revision.
    fn assets(&self) -> Result<ComPtr<IAssetBundleAbi>, NcStatus>;
    /// Creates an empty private child editor; containers expose only its read-only view.
    fn create_children(&self) -> Result<ComPtr<IChildrenEditorAbi>, NcStatus>;
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

/// Plain color values also used by renderer and optional JSON authoring code.
mod color;
pub use color::{Color, color};
/// Named flat-palette authoring values; the asset plugin owns their resolved appearance compiler.
pub mod palette;
pub use palette::FlatPalette;

/// Semantic names shared by asset recipes and runtime resource resolution.
mod resource_roles;
pub use resource_roles::{FontRole, IconRole};
/// Plain layout and window geometry used by theme authoring and resolved skins.
mod skin;
pub use skin::{CaptionButtonSide, CaptionButtonsSkin, SkinMetrics, TitleBackdropSkin, WindowChromeSkin, WindowTitleAlignment};
