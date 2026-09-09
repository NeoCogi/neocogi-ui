//! Concrete built-in editing contracts; implementation state stays in its COM object.
use neocogi_com::{ComPtr, ComRef, INcStringAbi, NcCopyFromBufferOffset, NcStatus, com_class, com_interface, com_int_enum, com_struct};

#[com_interface(auto_guid = "neocogi.ui.TextContent:v1")]
/// Shared whole-text editing inherited by text controls and labelled list entries.
pub trait ITextContent: crate::IWidget {
    /// Returns an independently owned immutable UTF-8 snapshot.
    fn text(&self) -> Result<ComPtr<INcStringAbi>, NcStatus>;
    /// Replaces complete UTF-8 text at offset zero, applying the control's line policy.
    fn set_text(&mut self, text: NcCopyFromBufferOffset<'_, u8>) -> Result<(), NcStatus>;
}

#[com_interface(auto_guid = "neocogi.ui.TextEditor:v1")]
/// Common byte-cursor editing on the same allocation as the text/widget bases.
pub trait ITextEditor: ITextContent {
    /// Returns the byte offset at a Unicode scalar boundary, independent of glyph width.
    fn cursor(&self) -> Result<u64, NcStatus>;
    /// Clamps to the preceding Unicode scalar boundary and the current text length.
    fn set_cursor(&mut self, cursor: u64) -> Result<(), NcStatus>;
}

#[com_class(auto_guid = "neocogi.ui.Textbox:v1")]
/// Constructible single-line editor whose complete concrete API is the class contract.
pub trait Textbox: crate::IWidget + crate::ILeafLayout {
    /// Creates normalized unmounted text state owned by the supplied UI host.
    #[constructor]
    fn new(host: ComRef<'_, crate::IWidgetHostAbi>, text: NcCopyFromBufferOffset<'_, u8>) -> Result<Self, NcStatus>;
    /// Returns an independently owned immutable UTF-8 snapshot.
    fn text(&self) -> Result<ComPtr<INcStringAbi>, NcStatus>;
    /// Replaces complete UTF-8 text after applying the single-line normalization policy.
    fn set_text(&mut self, text: NcCopyFromBufferOffset<'_, u8>) -> Result<(), NcStatus>;
    /// Returns the byte cursor at a Unicode scalar boundary.
    fn cursor(&self) -> Result<u64, NcStatus>;
    /// Clamps to the preceding Unicode scalar boundary and current text length.
    fn set_cursor(&mut self, cursor: u64) -> Result<(), NcStatus>;
    /// Selects a semantic font role and invalidates measurement only when it differs.
    fn set_font_role(&mut self, role: TextFontRole) -> Result<(), NcStatus>;
    /// Resolves an exact named font in the current host bundle before committing.
    fn set_named_font(&mut self, name: NcCopyFromBufferOffset<'_, u8>) -> Result<(), NcStatus>;
    /// Connects the independent changed-text subscriber.
    fn subscribe_changed(&mut self, sink: ComRef<'_, crate::ITextChangedSinkAbi>) -> Result<ComPtr<crate::IEventSubscriptionAbi>, NcStatus>;
    /// Connects the independent submitted-text subscriber.
    fn subscribe_submitted(&mut self, sink: ComRef<'_, crate::ITextSubmittedSinkAbi>) -> Result<ComPtr<crate::IEventSubscriptionAbi>, NcStatus>;
}

com_int_enum! {
    /// Concrete multiline wrapping policy shared by display and editing controls.
    pub enum TextWrapping: u8 {
        /// Preserve explicit line breaks without inserting visual breaks.
        NONE = 0,
        /// Break at ASCII spaces, retaining an oversized word as one visual span.
        WORD = 1,
    }
}

#[com_class(auto_guid = "neocogi.ui.TextBlock:v1")]
/// Constructible multiline display text with its complete concrete API in one class contract.
pub trait TextBlock: crate::IWidget + crate::ILeafLayout {
    /// Creates unmounted display text with an explicit wrapping policy.
    #[constructor]
    fn new(host: ComRef<'_, crate::IWidgetHostAbi>, text: NcCopyFromBufferOffset<'_, u8>, wrapping: TextWrapping) -> Result<Self, NcStatus>;
    /// Returns an independently owned immutable UTF-8 snapshot.
    fn text(&self) -> Result<ComPtr<INcStringAbi>, NcStatus>;
    /// Replaces complete UTF-8 text at offset zero.
    fn set_text(&mut self, text: NcCopyFromBufferOffset<'_, u8>) -> Result<(), NcStatus>;
    /// Returns the current wrapping policy.
    fn wrapping(&self) -> Result<TextWrapping, NcStatus>;
    /// Changes wrapping and records a sticky measurement change when it differs.
    fn set_wrapping(&mut self, wrapping: TextWrapping) -> Result<(), NcStatus>;
    /// Selects a semantic font role and invalidates measurement only when it differs.
    fn set_font_role(&mut self, role: TextFontRole) -> Result<(), NcStatus>;
    /// Resolves an exact named font in the current host bundle before committing.
    fn set_named_font(&mut self, name: NcCopyFromBufferOffset<'_, u8>) -> Result<(), NcStatus>;
}

#[com_class(auto_guid = "neocogi.ui.Checkbox:v1")]
/// Constructible boolean control with no redundant Checkbox-only interface.
pub trait Checkbox: crate::IWidget + crate::ILeafLayout {
    /// Creates complete unmounted state owned by the supplied UI host.
    #[constructor]
    fn new(host: ComRef<'_, crate::IWidgetHostAbi>, checked: bool, label: NcCopyFromBufferOffset<'_, u8>) -> Result<Self, NcStatus>;

    /// Returns the current boolean value.
    fn checked(&self) -> Result<bool, NcStatus>;
    /// Replaces the value without publishing user interaction or dirtying geometry.
    fn set_checked(&mut self, checked: bool) -> Result<(), NcStatus>;
    /// Returns an owned immutable label snapshot.
    fn label(&self) -> Result<ComPtr<INcStringAbi>, NcStatus>;
    /// Replaces the complete UTF-8 label and invalidates preferred size when changed.
    fn set_label(&mut self, label: NcCopyFromBufferOffset<'_, u8>) -> Result<(), NcStatus>;
    /// Selects a semantic font role and invalidates measurement only when it differs.
    fn set_font_role(&mut self, role: TextFontRole) -> Result<(), NcStatus>;
    /// Resolves an exact named font in the current host bundle before committing.
    fn set_named_font(&mut self, name: NcCopyFromBufferOffset<'_, u8>) -> Result<(), NcStatus>;
    /// Connects the exclusive checked-state subscriber and returns its ownership lease.
    fn subscribe(&mut self, sink: ComRef<'_, crate::ICheckboxSinkAbi>) -> Result<ComPtr<crate::IEventSubscriptionAbi>, NcStatus>;
}

#[com_struct]
/// Complete slider arithmetic inputs, validated together before object construction.
pub struct SliderOptions {
    /// Finite initial value; clamped to the configured bounds.
    pub value: f32,
    /// Finite inclusive lower bound.
    pub low: f32,
    /// Finite inclusive upper bound, at least low; the span must remain finite.
    pub high: f32,
    /// Finite nonnegative snap/increment step; zero selects continuous movement.
    pub step: f32,
    /// Fractional decimal digits, from zero through f32::DIGITS.
    pub precision: u8,
}

#[com_struct]
/// Complete standalone numeric-entry arithmetic inputs.
pub struct NumberOptions {
    /// Finite initial numeric value.
    pub value: f32,
    /// Finite nonnegative increment/drag step.
    pub step: f32,
    /// Fractional decimal digits, from zero through f32::DIGITS.
    pub precision: u8,
}

#[com_class(auto_guid = "neocogi.ui.Slider:v1")]
/// Constructible bounded numeric control whose concrete API is one class contract.
pub trait Slider: crate::IWidget + crate::ILeafLayout {
    /// Validates all arithmetic inputs and creates an unmounted framed slider.
    #[constructor]
    fn new(host: ComRef<'_, crate::IWidgetHostAbi>, options: SliderOptions) -> Result<Self, NcStatus>;
    /// Returns the current finite value after range clamping and step snapping.
    fn value(&self) -> Result<f32, NcStatus>;
    /// Replaces a finite value silently after applying the configured range and step.
    fn set_value(&mut self, value: f32) -> Result<(), NcStatus>;
    /// Reports whether the control currently accepts inline text editing.
    fn is_editing(&self) -> Result<bool, NcStatus>;
    /// Selects a semantic font role and invalidates measurement only when it differs.
    fn set_font_role(&mut self, role: TextFontRole) -> Result<(), NcStatus>;
    /// Resolves an exact named font in the current host bundle before committing.
    fn set_named_font(&mut self, name: NcCopyFromBufferOffset<'_, u8>) -> Result<(), NcStatus>;
    /// Connects the exclusive finite-value subscriber and returns its ownership lease.
    fn subscribe(&mut self, sink: ComRef<'_, crate::INumericSinkAbi>) -> Result<ComPtr<crate::IEventSubscriptionAbi>, NcStatus>;
}

#[com_class(auto_guid = "neocogi.ui.Number:v1")]
/// Constructible unbounded numeric editor whose concrete API is one class contract.
pub trait Number: crate::IWidget + crate::ILeafLayout {
    /// Validates arithmetic inputs and creates an unmounted numeric editor.
    #[constructor]
    fn new(host: ComRef<'_, crate::IWidgetHostAbi>, options: NumberOptions) -> Result<Self, NcStatus>;
    /// Returns the current finite value.
    fn value(&self) -> Result<f32, NcStatus>;
    /// Replaces a finite value silently using the configured precision.
    fn set_value(&mut self, value: f32) -> Result<(), NcStatus>;
    /// Reports whether the control currently accepts inline text editing.
    fn is_editing(&self) -> Result<bool, NcStatus>;
    /// Selects a semantic font role and invalidates measurement only when it differs.
    fn set_font_role(&mut self, role: TextFontRole) -> Result<(), NcStatus>;
    /// Resolves an exact named font in the current host bundle before committing.
    fn set_named_font(&mut self, name: NcCopyFromBufferOffset<'_, u8>) -> Result<(), NcStatus>;
    /// Connects the exclusive finite-value subscriber and returns its ownership lease.
    fn subscribe(&mut self, sink: ComRef<'_, crate::INumericSinkAbi>) -> Result<ComPtr<crate::IEventSubscriptionAbi>, NcStatus>;
}

#[com_class(auto_guid = "neocogi.ui.ColorSwatch:v1")]
/// Constructible RGBA preview whose optional label is part of the concrete class API.
pub trait ColorSwatch: crate::IWidget + crate::ILeafLayout {
    /// Creates an unmounted color preview from packed RGBA and complete UTF-8 label state.
    #[constructor]
    fn new(host: ComRef<'_, crate::IWidgetHostAbi>, rgba: u32, label: NcCopyFromBufferOffset<'_, u8>) -> Result<Self, NcStatus>;
    /// Returns an independently owned immutable UTF-8 label snapshot.
    fn text(&self) -> Result<ComPtr<INcStringAbi>, NcStatus>;
    /// Replaces the complete UTF-8 label and invalidates preferred size when changed.
    fn set_text(&mut self, text: NcCopyFromBufferOffset<'_, u8>) -> Result<(), NcStatus>;
    /// Returns RGBA packed with red in the least significant byte.
    fn fill(&self) -> Result<u32, NcStatus>;
    /// Changes paint only and does not publish a user event.
    fn set_fill(&mut self, rgba: u32) -> Result<(), NcStatus>;
    /// Selects a semantic font role and invalidates measurement only when it differs.
    fn set_font_role(&mut self, role: TextFontRole) -> Result<(), NcStatus>;
    /// Resolves an exact named font in the current host bundle before committing.
    fn set_named_font(&mut self, name: NcCopyFromBufferOffset<'_, u8>) -> Result<(), NcStatus>;
}

#[com_class(auto_guid = "neocogi.ui.ListItem:v1")]
/// Constructible labelled row; generic application data remains in the static event adapter.
pub trait ListItem: crate::IWidget + crate::ILeafLayout {
    /// Creates one unmounted labelled row without embedding application data in COM.
    #[constructor]
    fn new(host: ComRef<'_, crate::IWidgetHostAbi>, label: NcCopyFromBufferOffset<'_, u8>) -> Result<Self, NcStatus>;
    /// Returns an independently owned immutable UTF-8 label snapshot.
    fn text(&self) -> Result<ComPtr<INcStringAbi>, NcStatus>;
    /// Replaces the complete UTF-8 label and invalidates preferred size when changed.
    fn set_text(&mut self, text: NcCopyFromBufferOffset<'_, u8>) -> Result<(), NcStatus>;
    /// Selects a semantic font role and invalidates measurement only when it differs.
    fn set_font_role(&mut self, role: TextFontRole) -> Result<(), NcStatus>;
    /// Resolves an exact named font in the current host bundle before committing.
    fn set_named_font(&mut self, name: NcCopyFromBufferOffset<'_, u8>) -> Result<(), NcStatus>;
    /// Selects a semantic icon or removes it while retaining the label.
    fn set_icon_role(&mut self, icon: ContentIcon) -> Result<(), NcStatus>;
    /// Resolves an exact named icon before replacing the existing selection.
    fn set_named_icon(&mut self, name: NcCopyFromBufferOffset<'_, u8>) -> Result<(), NcStatus>;
    /// Connects the exclusive activation subscriber and returns its ownership lease.
    fn subscribe(&mut self, sink: ComRef<'_, crate::IActivationSinkAbi>) -> Result<ComPtr<crate::IEventSubscriptionAbi>, NcStatus>;
}

#[com_class(auto_guid = "neocogi.ui.ListBox:v1")]
/// Constructible button-like list entry with direct label, image, and activation operations.
pub trait ListBox: crate::IWidget + crate::ILeafLayout {
    /// Creates one unmounted label-only entry; image state may be installed afterward.
    #[constructor]
    fn new(host: ComRef<'_, crate::IWidgetHostAbi>, label: NcCopyFromBufferOffset<'_, u8>) -> Result<Self, NcStatus>;
    /// Returns an independently owned immutable UTF-8 label snapshot.
    fn text(&self) -> Result<ComPtr<INcStringAbi>, NcStatus>;
    /// Replaces the complete UTF-8 label and invalidates preferred size when changed.
    fn set_text(&mut self, text: NcCopyFromBufferOffset<'_, u8>) -> Result<(), NcStatus>;
    /// Selects a semantic font role and invalidates measurement only when it differs.
    fn set_font_role(&mut self, role: TextFontRole) -> Result<(), NcStatus>;
    /// Resolves an exact named font in the current host bundle before committing.
    fn set_named_font(&mut self, name: NcCopyFromBufferOffset<'_, u8>) -> Result<(), NcStatus>;
    /// Installs a positive-sized immutable texture and optionally fits it to content width.
    fn set_image(&mut self, image: ComRef<'_, crate::ITextureAbi>, fit_width: bool) -> Result<(), NcStatus>;
    /// Removes image content while retaining the current label and font.
    fn clear_image(&mut self) -> Result<(), NcStatus>;
    /// Connects the exclusive activation subscriber and returns its ownership lease.
    fn subscribe(&mut self, sink: ComRef<'_, crate::IActivationSinkAbi>) -> Result<ComPtr<crate::IEventSubscriptionAbi>, NcStatus>;
}

com_int_enum! {
    /// Scrollbar orientation, fixed for the lifetime of one control.
    pub enum ScrollAxis: u8 {
        /// Left-to-right scrolling and horizontal thumb movement.
        HORIZONTAL = 0,
        /// Top-to-bottom scrolling and vertical thumb movement.
        VERTICAL = 1,
    }
}

#[com_struct]
#[derive(Debug, Eq, PartialEq)]
/// Copied scrollbar range; lengths are nonnegative and offset is clamped.
pub struct ScrollRange {
    /// Immutable direction of the control.
    pub axis: ScrollAxis,
    /// Visible extent in pixels.
    pub viewport: i32,
    /// Complete content extent in pixels.
    pub content: i32,
    /// Current offset in zero through max(content - viewport, zero).
    pub offset: i32,
}

#[com_class(auto_guid = "neocogi.ui.Scrollbar:v1")]
/// Constructible scrollbar with its concrete editing and event surface in one class contract.
pub trait Scrollbar: crate::IWidget + crate::ILeafLayout {
    /// Creates complete unmounted scrollbar state owned by the supplied UI host.
    #[constructor]
    fn new(host: ComRef<'_, crate::IWidgetHostAbi>, range: ScrollRange) -> Result<Self, NcStatus>;

    /// Returns a coherent copied range snapshot.
    fn range(&self) -> Result<ScrollRange, NcStatus>;
    /// Replaces nonnegative lengths together and clamps the existing offset.
    fn set_lengths(&mut self, viewport: i32, content: i32) -> Result<(), NcStatus>;
    /// Clamps a requested offset to the current range without publishing user input.
    fn set_offset(&mut self, offset: i32) -> Result<(), NcStatus>;
    /// Connects the exclusive offset subscriber and returns its ownership lease.
    fn subscribe(&mut self, sink: ComRef<'_, crate::IScrollbarSinkAbi>) -> Result<ComPtr<crate::IEventSubscriptionAbi>, NcStatus>;
}

com_int_enum! {
    /// Semantic font selection resolved by the current immutable resource revision.
    pub enum TextFontRole: u8 {
        /// Ordinary control and body text.
        BODY = 0,
        /// Compact supporting text.
        SMALL = 1,
        /// Window-title typography.
        TITLE = 2,
        /// Larger display typography.
        HEADING = 3,
        /// Monospace typography.
        MONO = 4,
    }
}

#[com_interface(auto_guid = "neocogi.ui.FontStyle:v1")]
/// Optional typography capability on concrete text-bearing widget objects.
pub trait IFontStyle {
    /// Selects a semantic role and invalidates measurement only when it differs.
    fn set_font_role(&mut self, role: TextFontRole) -> Result<(), NcStatus>;
    /// Resolves an exact font in this widget's runtime bundle before committing.
    /// Unknown names fail without replacing the previous selection.
    fn set_named_font(&mut self, name: NcCopyFromBufferOffset<'_, u8>) -> Result<(), NcStatus>;
}

com_int_enum! {
    /// Optional semantic icon selected in the widget's own resource bundle.
    pub enum ContentIcon: u8 {
        /// Remove the adjacent icon.
        NONE = 0,
        /// Close affordance.
        CLOSE = 1,
        /// Collapsed disclosure affordance.
        EXPAND = 2,
        /// Expanded disclosure affordance.
        COLLAPSE = 3,
        /// Checked value marker.
        CHECK = 4,
        /// Dropdown affordance.
        EXPAND_DOWN = 5,
        /// Open folder.
        OPEN_FOLDER = 6,
        /// Closed folder.
        CLOSED_FOLDER = 7,
        /// Regular file.
        FILE = 8,
    }
}

#[com_struct]
#[derive(Debug, Eq, PartialEq)]
/// Interaction states in which an unframed button fills its center.
pub struct FillStates {
    /// Union of NORMAL, HOVER and FOCUS; unknown bits are rejected.
    pub bits: u8,
}

impl FillStates {
    /// No center fill.
    pub const NONE: Self = Self { bits: 0 };
    /// Idle or disabled center fill.
    pub const NORMAL: Self = Self { bits: 1 };
    /// Hovered center fill.
    pub const HOVER: Self = Self { bits: 2 };
    /// Focused or pressed center fill.
    pub const FOCUS: Self = Self { bits: 4 };
    /// Center fill in every interaction state.
    pub const ALL: Self = Self { bits: 7 };
}

#[com_interface(auto_guid = "neocogi.ui.MultilineEditor:v1")]
/// Multiline editing and wrapping, implemented by the real editor and forwarded by its composed root.
pub trait IMultilineEditor: ITextEditor {
    /// Returns the current explicit wrapping policy.
    fn wrapping(&self) -> Result<TextWrapping, NcStatus>;
    /// Changes wrapping and schedules both geometry refresh and caret reveal.
    fn set_wrapping(&mut self, wrapping: TextWrapping) -> Result<(), NcStatus>;
}

#[com_class(auto_guid = "neocogi.ui.TextArea:v1")]
/// Constructible fixed scrolling editor whose class forwards to one actual internal editor.
pub trait TextArea: crate::IWidget + crate::IContainerLayout + crate::IScrolling {
    /// Creates a fixed editor composition with normalized text and explicit wrapping/scroll policy.
    #[constructor]
    fn new(host: ComRef<'_, crate::IWidgetHostAbi>, text: NcCopyFromBufferOffset<'_, u8>, wrapping: TextWrapping, scrolling: bool) -> Result<Self, NcStatus>;
    /// Returns an independently owned immutable UTF-8 snapshot from the actual editor.
    fn text(&self) -> Result<ComPtr<INcStringAbi>, NcStatus>;
    /// Replaces normalized text on the actual editor and schedules caret reveal.
    fn set_text(&mut self, text: NcCopyFromBufferOffset<'_, u8>) -> Result<(), NcStatus>;
    /// Returns the actual editor's scalar-aligned byte cursor.
    fn cursor(&self) -> Result<u64, NcStatus>;
    /// Clamps and installs the actual editor's byte cursor.
    fn set_cursor(&mut self, cursor: u64) -> Result<(), NcStatus>;
    /// Returns the actual editor's wrapping policy.
    fn wrapping(&self) -> Result<TextWrapping, NcStatus>;
    /// Changes wrapping and schedules text geometry and caret refresh.
    fn set_wrapping(&mut self, wrapping: TextWrapping) -> Result<(), NcStatus>;
    /// Selects a semantic font role on the actual editor.
    fn set_font_role(&mut self, role: TextFontRole) -> Result<(), NcStatus>;
    /// Resolves an exact named font in the current host bundle before committing.
    fn set_named_font(&mut self, name: NcCopyFromBufferOffset<'_, u8>) -> Result<(), NcStatus>;
    /// Connects the actual editor's independent changed-text subscriber.
    fn subscribe_changed(&mut self, sink: ComRef<'_, crate::ITextChangedSinkAbi>) -> Result<ComPtr<crate::IEventSubscriptionAbi>, NcStatus>;
    /// Connects the actual editor's independent submitted-text subscriber.
    fn subscribe_submitted(&mut self, sink: ComRef<'_, crate::ITextSubmittedSinkAbi>) -> Result<ComPtr<crate::IEventSubscriptionAbi>, NcStatus>;
}

#[com_class(auto_guid = "neocogi.ui.Combo:v1")]
/// Constructible combo header; generic list data and popup content remain with its composing owner.
pub trait Combo: crate::IWidget + crate::ILeafLayout {
    /// Creates an empty unmounted header owned by the supplied UI host.
    #[constructor]
    fn new(host: ComRef<'_, crate::IWidgetHostAbi>) -> Result<Self, NcStatus>;

    /// Returns the selected application-list position, with zero also used by an empty list.
    fn selected(&self) -> Result<u64, NcStatus>;
    /// Returns the captured display label, independently of generic application data.
    fn label(&self) -> Result<ComPtr<INcStringAbi>, NcStatus>;
    /// Reports the header's intended popup state, not an independently observed window visibility.
    fn is_open(&self) -> Result<bool, NcStatus>;
    /// Synchronizes popup state silently after ordinary popup dismissal or programmatic coordination.
    fn set_open(&mut self, open: bool) -> Result<(), NcStatus>;
    /// Atomically installs a valid index/label snapshot without publishing user selection.
    fn set_selection(&mut self, index: u64, label: NcCopyFromBufferOffset<'_, u8>) -> Result<(), NcStatus>;
    /// Commits an actual popup selection, closes the header, and emits changed only when index or label differs.
    fn submit_selection(&mut self, index: u64, label: NcCopyFromBufferOffset<'_, u8>) -> Result<(), NcStatus>;
    /// Selects a semantic font role and invalidates measurement only when it differs.
    fn set_font_role(&mut self, role: TextFontRole) -> Result<(), NcStatus>;
    /// Resolves an exact named font in the current host bundle before committing.
    fn set_named_font(&mut self, name: NcCopyFromBufferOffset<'_, u8>) -> Result<(), NcStatus>;
    /// Connects the independent captured-selection subscriber.
    fn subscribe_changed(&mut self, sink: ComRef<'_, crate::IComboChangedSinkAbi>) -> Result<ComPtr<crate::IEventSubscriptionAbi>, NcStatus>;
    /// Connects the independent popup-coordination subscriber.
    fn subscribe_popup(&mut self, sink: ComRef<'_, crate::IComboPopupSinkAbi>) -> Result<ComPtr<crate::IEventSubscriptionAbi>, NcStatus>;
}
