//! Concrete editing implementations over each built-in's one semantic state.
use super::*;
use nc_com::{INcStringAbi, NcString};

leaf_class!(Checkbox, crate::Checkbox, ICheckbox, ICheckboxAbi, IFontStyle);
leaf_class!(Textbox, crate::Textbox, ITextbox, ITextboxAbi, IFontStyle);
leaf_class!(TextBlock, crate::TextBlock, ITextBlock, ITextBlockAbi, IFontStyle);
leaf_class!(Slider, crate::Slider, ISlider, ISliderAbi, IFontStyle);
leaf_class!(Number, crate::Number, INumber, INumberAbi, IFontStyle);
leaf_class!(ColorSwatch, crate::ColorSwatch, IColorSwatch, IColorSwatchAbi, IFontStyle);
leaf_class!(ListItem, crate::ListItem, IListItem, IListItemAbi, IFontStyle, IIconContent);
leaf_class!(ListBox, crate::ListBox, IListBox, IListBoxAbi, IFontStyle);
leaf_class!(Scrollbar, crate::Scrollbar, IScrollbar, IScrollbarAbi);

impl ICheckbox for Checkbox {
    /// Reads the same boolean state used by pointer/Space updates and paint.
    fn checked(&self) -> Result<bool, NcStatus> {
        self.leaf.state.readable()?;
        Ok(self.leaf.core.try_borrow().map_err(|_| UI_E_BUSY)?.checked())
    }
    /// Checked state changes paint only; both states reserve the same glyph space.
    fn set_checked(&self, checked: bool) -> Result<(), NcStatus> {
        self.leaf.state.writable()?;
        self.leaf.core.try_borrow_mut().map_err(|_| UI_E_BUSY)?.set_checked(checked);
        Ok(())
    }
    /// Copies a label into an independently owned immutable COM string.
    fn label(&self) -> Result<ComPtr<INcStringAbi>, NcStatus> {
        self.leaf.state.readable()?;
        Ok(NcString::from_utf8_str(self.leaf.core.try_borrow().map_err(|_| UI_E_BUSY)?.com_label()))
    }
    /// Commits valid UTF-8 atomically and preserves a prior pending change.
    fn set_label(&self, input: NcCopyFromBufferOffset<'_, u8>) -> Result<(), NcStatus> {
        self.leaf.state.writable()?;
        let input = text(input)?;
        let mut core = self.leaf.core.try_borrow_mut().map_err(|_| UI_E_BUSY)?;
        if core.com_label() != input {
            core.com_set_label(input);
            self.leaf.state.record_change(GeometryChange::CHANGED);
        }
        Ok(())
    }
}

/// Shares whole-text snapshots/replacement while retaining each control's line policy.
macro_rules! text_content {
    ($name:ident, $get:ident, $set:ident) => {
        impl ITextContent for $name {
            /// Returns an immutable snapshot without exposing the semantic String allocation.
            fn text(&self) -> Result<ComPtr<INcStringAbi>, NcStatus> {
                self.leaf.state.readable()?;
                Ok(NcString::from_utf8_str(self.leaf.core.try_borrow().map_err(|_| UI_E_BUSY)?.$get()))
            }
            /// Applies the existing normalization/cursor policy, then compares committed text.
            fn set_text(&self, input: NcCopyFromBufferOffset<'_, u8>) -> Result<(), NcStatus> {
                self.leaf.state.writable()?;
                let input = text(input)?;
                let mut core = self.leaf.core.try_borrow_mut().map_err(|_| UI_E_BUSY)?;
                // Compare normalized output, not raw CR/LF input. An editor may
                // still reset its cursor on a text-identical replacement.
                let previous = core.$get().to_owned();
                core.$set(input);
                if core.$get() != previous {
                    self.leaf.state.record_change(GeometryChange::CHANGED);
                }
                Ok(())
            }
        }
    };
}
text_content!(Textbox, text, set_text);
text_content!(TextBlock, text, set_text);
text_content!(ColorSwatch, label, set_label);
text_content!(ListItem, label, set_label);
text_content!(ListBox, com_label, com_set_label);

impl ITextbox for Textbox {}
impl ITextEditor for Textbox {
    /// Reports a byte cursor rather than a glyph or grapheme index.
    fn cursor(&self) -> Result<u64, NcStatus> {
        self.leaf.state.readable()?;
        Ok(self.leaf.core.try_borrow().map_err(|_| UI_E_BUSY)?.cursor() as u64)
    }
    /// Clamps oversize input before applying the shared Unicode-boundary algorithm.
    fn set_cursor(&self, cursor: u64) -> Result<(), NcStatus> {
        self.leaf.state.writable()?;
        self.leaf
            .core
            .try_borrow_mut()
            .map_err(|_| UI_E_BUSY)?
            .set_cursor(usize::try_from(cursor).unwrap_or(usize::MAX));
        Ok(())
    }
}

impl ITextBlock for TextBlock {
    /// Encodes exactly the wrapping behavior used by existing text measurement.
    fn wrapping(&self) -> Result<TextWrapping, NcStatus> {
        self.leaf.state.readable()?;
        Ok(match self.leaf.core.try_borrow().map_err(|_| UI_E_BUSY)?.com_wrap() {
            crate::TextWrap::None => TextWrapping::NONE,
            crate::TextWrap::Word => TextWrapping::WORD,
        })
    }
    /// Rejects unknown policies before mutating semantic state or its geometry marker.
    fn set_wrapping(&self, wrapping: TextWrapping) -> Result<(), NcStatus> {
        self.leaf.state.writable()?;
        let wrap = local_wrap(wrapping)?;
        let mut core = self.leaf.core.try_borrow_mut().map_err(|_| UI_E_BUSY)?;
        if core.com_wrap() != wrap {
            core.com_set_wrap(wrap);
            self.leaf.state.record_change(GeometryChange::CHANGED);
        }
        Ok(())
    }
}

/// Converts the closed wrapping vocabulary at construction and mutation boundaries.
pub(super) fn local_wrap(wrap: TextWrapping) -> Result<crate::TextWrap, NcStatus> {
    match wrap {
        TextWrapping::NONE => Ok(crate::TextWrap::None),
        TextWrapping::WORD => Ok(crate::TextWrap::Word),
        _ => Err(NC_E_INVALID_ARG),
    }
}

/// Shares finite-value editing without duplicating slider/number arithmetic policy.
macro_rules! numeric_value {
    ($name:ident) => {
        impl INumericValue for $name {
            /// Reads the finite value owned by the concrete semantic algorithm.
            fn value(&self) -> Result<f32, NcStatus> {
                self.leaf.state.readable()?;
                Ok(self.leaf.core.try_borrow().map_err(|_| UI_E_BUSY)?.value())
            }
            /// Rejects nonfinite input and dirties only an actual committed value change.
            fn set_value(&self, value: f32) -> Result<(), NcStatus> {
                self.leaf.state.writable()?;
                if !value.is_finite() {
                    return Err(NC_E_INVALID_ARG);
                }
                let mut core = self.leaf.core.try_borrow_mut().map_err(|_| UI_E_BUSY)?;
                let previous = core.value();
                core.set_value(value);
                if core.value() != previous {
                    self.leaf.state.record_change(GeometryChange::CHANGED);
                }
                Ok(())
            }
            /// Exposes the same edit mode that controls routed text input.
            fn is_editing(&self) -> Result<bool, NcStatus> {
                self.leaf.state.readable()?;
                Ok(self.leaf.core.try_borrow().map_err(|_| UI_E_BUSY)?.is_editing())
            }
        }
    };
}
numeric_value!(Slider);
numeric_value!(Number);
impl ISlider for Slider {}
impl INumber for Number {}
impl IListItem for ListItem {}
impl IListBox for ListBox {}

impl IColorSwatch for ColorSwatch {
    /// Packs canonical RGBA without relying on machine endianness.
    fn fill(&self) -> Result<u32, NcStatus> {
        self.leaf.state.readable()?;
        let c = self.leaf.core.try_borrow().map_err(|_| UI_E_BUSY)?.fill();
        Ok(u32::from_le_bytes([c.r, c.g, c.b, c.a]))
    }
    /// Color is a paint-only input and emits no user event.
    fn set_fill(&self, rgba: u32) -> Result<(), NcStatus> {
        self.leaf.state.writable()?;
        self.leaf.core.try_borrow_mut().map_err(|_| UI_E_BUSY)?.set_fill(super::render::color(rgba));
        Ok(())
    }
}

impl IScrollbar for Scrollbar {
    /// Reads all range fields from one semantic borrow.
    fn range(&self) -> Result<ScrollRange, NcStatus> {
        self.leaf.state.readable()?;
        let core = self.leaf.core.try_borrow().map_err(|_| UI_E_BUSY)?;
        let (viewport, content) = core.com_lengths();
        let axis = match core.axis() {
            crate::ScrollbarAxis::Horizontal => ScrollAxis::HORIZONTAL,
            crate::ScrollbarAxis::Vertical => ScrollAxis::VERTICAL,
        };
        Ok(ScrollRange {
            axis,
            viewport,
            content,
            offset: core.offset(),
        })
    }
    /// Validates both lengths before clamping the existing offset in one commit.
    fn set_lengths(&self, viewport: i32, content: i32) -> Result<(), NcStatus> {
        self.leaf.state.writable()?;
        if viewport < 0 || content < 0 {
            return Err(NC_E_INVALID_ARG);
        }
        self.leaf.core.try_borrow_mut().map_err(|_| UI_E_BUSY)?.set_lengths(viewport, content);
        Ok(())
    }
    /// Offset affects thumb paint only; the control's preferred size is fixed by its axis.
    fn set_offset(&self, offset: i32) -> Result<(), NcStatus> {
        self.leaf.state.writable()?;
        self.leaf.core.try_borrow_mut().map_err(|_| UI_E_BUSY)?.set_offset(offset);
        Ok(())
    }
}

font_style_impl!(Checkbox);

font_style_impl!(Textbox);

font_style_impl!(TextBlock);

font_style_impl!(Slider);

font_style_impl!(Number);

font_style_impl!(ColorSwatch);

font_style_impl!(ListBox);

font_style_impl!(ListItem);

icon_content_impl!(ListItem);
