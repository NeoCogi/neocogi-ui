//! Concrete built-in editing contracts; implementation state stays in its COM object.
use nc_com::{ComPtr, ComRef, INcStringAbi, NcCopyFromBufferOffset, NcStatus, com_class, com_interface, com_int_enum, com_struct};

#[com_interface(auto_guid = "microui.TextContent:v1")]
/// Shared whole-text editing inherited by text controls and labelled list entries.
pub trait ITextContent: crate::IWidget {
    /// Returns an independently owned immutable UTF-8 snapshot.
    fn text(&self) -> Result<ComPtr<INcStringAbi>, NcStatus>;
    /// Replaces complete UTF-8 text at offset zero, applying the control's line policy.
    fn set_text(&self, text: NcCopyFromBufferOffset<'_, u8>) -> Result<(), NcStatus>;
}

#[com_interface(auto_guid = "microui.TextEditor:v1")]
/// Common byte-cursor editing on the same allocation as the text/widget bases.
pub trait ITextEditor: ITextContent {
    /// Returns the byte offset at a Unicode scalar boundary, independent of glyph width.
    fn cursor(&self) -> Result<u64, NcStatus>;
    /// Clamps to the preceding Unicode scalar boundary and the current text length.
    fn set_cursor(&self, cursor: u64) -> Result<(), NcStatus>;
}

#[com_interface(auto_guid = "microui.Textbox:v1")]
/// Single-line editor; construction, replacement and paste remove CR and LF.
pub trait ITextbox: ITextEditor {}

com_int_enum! {
    /// Concrete multiline wrapping policy shared by display and editing controls.
    pub enum TextWrapping: u8 {
        /// Preserve explicit line breaks without inserting visual breaks.
        NONE = 0,
        /// Break at ASCII spaces, retaining an oversized word as one visual span.
        WORD = 1,
    }
}

#[com_interface(auto_guid = "microui.TextBlock:v1")]
/// Multiline display text with mutable wrapping and inherited common text editing.
pub trait ITextBlock: ITextContent {
    /// Returns the current wrapping policy.
    fn wrapping(&self) -> Result<TextWrapping, NcStatus>;
    /// Changes wrapping and records a sticky measurement change when it differs.
    fn set_wrapping(&self, wrapping: TextWrapping) -> Result<(), NcStatus>;
}

#[com_class(auto_guid = "microui.Checkbox:v1")]
/// Constructible boolean control with no redundant Checkbox-only interface.
pub trait Checkbox: crate::IWidget + crate::ILeafLayout {
    /// Creates complete unmounted state owned by the supplied UI host.
    #[constructor]
    fn new(
        host: ComRef<'_, crate::IWidgetHostAbi>,
        checked: bool,
        label: NcCopyFromBufferOffset<'_, u8>,
    ) -> Result<Self, NcStatus>;

    /// Returns the current boolean value.
    fn checked(&self) -> Result<bool, NcStatus>;
    /// Replaces the value without publishing user interaction or dirtying geometry.
    fn set_checked(&self, checked: bool) -> Result<(), NcStatus>;
    /// Returns an owned immutable label snapshot.
    fn label(&self) -> Result<ComPtr<INcStringAbi>, NcStatus>;
    /// Replaces the complete UTF-8 label and invalidates preferred size when changed.
    fn set_label(&self, label: NcCopyFromBufferOffset<'_, u8>) -> Result<(), NcStatus>;
    /// Selects a semantic font role and invalidates measurement only when it differs.
    fn set_font_role(&self, role: TextFontRole) -> Result<(), NcStatus>;
    /// Resolves an exact named font in the current host bundle before committing.
    fn set_named_font(&self, name: NcCopyFromBufferOffset<'_, u8>) -> Result<(), NcStatus>;
    /// Connects the exclusive checked-state subscriber and returns its ownership lease.
    fn subscribe(&self, sink: ComRef<'_, crate::ICheckboxSinkAbi>) -> Result<ComPtr<crate::IEventSubscriptionAbi>, NcStatus>;
}

#[com_interface(auto_guid = "microui.NumericValue:v1")]
/// Shared concrete numeric editing on the same widget allocation.
pub trait INumericValue: crate::IWidget {
    /// Returns the current finite numeric value.
    fn value(&self) -> Result<f32, NcStatus>;
    /// Replaces the finite value silently; sliders clamp it to their configured range.
    fn set_value(&self, value: f32) -> Result<(), NcStatus>;
    /// Reports whether the control currently accepts inline text editing.
    fn is_editing(&self) -> Result<bool, NcStatus>;
}

#[com_interface(auto_guid = "microui.Slider:v1")]
/// Bounded numeric control supporting drag, wheel, keyboard and inline text editing.
pub trait ISlider: INumericValue {}

#[com_interface(auto_guid = "microui.Number:v1")]
/// Numeric entry without a slider range; step and precision are validated at construction.
pub trait INumber: INumericValue {}

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

#[com_interface(auto_guid = "microui.ColorSwatch:v1")]
/// RGBA swatch with an optional inherited text label.
pub trait IColorSwatch: ITextContent {
    /// Returns RGBA packed with red in the least significant byte.
    fn fill(&self) -> Result<u32, NcStatus>;
    /// Changes paint only and does not publish a user event.
    fn set_fill(&self, rgba: u32) -> Result<(), NcStatus>;
}

#[com_interface(auto_guid = "microui.ListItem:v1")]
/// One labelled row; generic application data remains in the static event adapter.
pub trait IListItem: ITextContent {}

#[com_interface(auto_guid = "microui.ListBox:v1")]
/// Button-like list entry with inherited text editing.
pub trait IListBox: ITextContent {}

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

#[com_class(auto_guid = "microui.Scrollbar:v1")]
/// Constructible scrollbar with its concrete editing and event surface in one class contract.
pub trait Scrollbar: crate::IWidget + crate::ILeafLayout {
    /// Creates complete unmounted scrollbar state owned by the supplied UI host.
    #[constructor]
    fn new(
        host: ComRef<'_, crate::IWidgetHostAbi>,
        range: ScrollRange,
    ) -> Result<Self, NcStatus>;

    /// Returns a coherent copied range snapshot.
    fn range(&self) -> Result<ScrollRange, NcStatus>;
    /// Replaces nonnegative lengths together and clamps the existing offset.
    fn set_lengths(&self, viewport: i32, content: i32) -> Result<(), NcStatus>;
    /// Clamps a requested offset to the current range without publishing user input.
    fn set_offset(&self, offset: i32) -> Result<(), NcStatus>;
    /// Connects the exclusive offset subscriber and returns its ownership lease.
    fn subscribe(&self, sink: ComRef<'_, crate::IScrollbarSinkAbi>) -> Result<ComPtr<crate::IEventSubscriptionAbi>, NcStatus>;
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

#[com_interface(auto_guid = "microui.FontStyle:v1")]
/// Optional typography capability on concrete text-bearing widget objects.
pub trait IFontStyle {
    /// Selects a semantic role and invalidates measurement only when it differs.
    fn set_font_role(&self, role: TextFontRole) -> Result<(), NcStatus>;
    /// Resolves an exact font in this widget's runtime bundle before committing.
    /// Unknown names fail without replacing the previous selection.
    fn set_named_font(&self, name: NcCopyFromBufferOffset<'_, u8>) -> Result<(), NcStatus>;
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

#[com_interface(auto_guid = "microui.IconContent:v1")]
/// Optional adjacent-icon capability on buttons and list rows.
pub trait IIconContent {
    /// Selects a semantic icon or removes it, retaining the existing text label.
    fn set_icon_role(&self, icon: ContentIcon) -> Result<(), NcStatus>;
    /// Resolves one exact named icon before replacing the existing visual selection.
    fn set_named_icon(&self, name: NcCopyFromBufferOffset<'_, u8>) -> Result<(), NcStatus>;
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

#[com_interface(auto_guid = "microui.MultilineEditor:v1")]
/// Multiline editing and wrapping, implemented by the real editor and forwarded by its composed root.
pub trait IMultilineEditor: ITextEditor {
    /// Returns the current explicit wrapping policy.
    fn wrapping(&self) -> Result<TextWrapping, NcStatus>;
    /// Changes wrapping and schedules both geometry refresh and caret reveal.
    fn set_wrapping(&self, wrapping: TextWrapping) -> Result<(), NcStatus>;
}

#[com_interface(auto_guid = "microui.TextArea:v1")]
/// Fixed scrolling editor root. Inherited text methods edit its actual internal editor.
/// Query IScrolling for offsets; IScrollArea is absent because the editor cannot be replaced.
pub trait ITextArea: IMultilineEditor {}

#[com_class(auto_guid = "microui.Combo:v1")]
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
    fn set_open(&self, open: bool) -> Result<(), NcStatus>;
    /// Atomically installs a valid index/label snapshot without publishing user selection.
    fn set_selection(&self, index: u64, label: NcCopyFromBufferOffset<'_, u8>) -> Result<(), NcStatus>;
    /// Commits an actual popup selection, closes the header, and emits changed only when index or label differs.
    fn submit_selection(&self, index: u64, label: NcCopyFromBufferOffset<'_, u8>) -> Result<(), NcStatus>;
    /// Selects a semantic font role and invalidates measurement only when it differs.
    fn set_font_role(&self, role: TextFontRole) -> Result<(), NcStatus>;
    /// Resolves an exact named font in the current host bundle before committing.
    fn set_named_font(&self, name: NcCopyFromBufferOffset<'_, u8>) -> Result<(), NcStatus>;
    /// Connects the independent captured-selection subscriber.
    fn subscribe_changed(&self, sink: ComRef<'_, crate::IComboChangedSinkAbi>) -> Result<ComPtr<crate::IEventSubscriptionAbi>, NcStatus>;
    /// Connects the independent popup-coordination subscriber.
    fn subscribe_popup(&self, sink: ComRef<'_, crate::IComboPopupSinkAbi>) -> Result<ComPtr<crate::IEventSubscriptionAbi>, NcStatus>;
}

#[com_interface(auto_guid = "microui.ImageContent:v1")]
/// Shared image editing for buttons and list-box entries; pixels stay owned by their renderer.
pub trait IImageContent {
    /// Installs a positive-sized immutable texture. Width fitting preserves aspect ratio and
    /// uses the allocated content width; false uses inline placement with dimensions bounded by the allocation.
    /// Image selection replaces any atlas icon while preserving the label.
    fn set_image(&self, image: nc_com::ComRef<'_, crate::ITextureAbi>, fit_width: bool) -> Result<(), NcStatus>;
    /// Removes image content while preserving the current label and font.
    fn clear_image(&self) -> Result<(), NcStatus>;
}
