//! Concrete editing implementations over each built-in's one semantic state.
use super::*;
use nc_com::{INcStringAbi, NcString};

callable_leaf_class!(CheckboxObject, crate::Checkbox, microui_contracts::Checkbox);
callable_leaf_class!(TextboxObject, crate::Textbox, microui_contracts::Textbox);
callable_leaf_class!(TextBlockObject, crate::TextBlock, microui_contracts::TextBlock);
callable_leaf_class!(SliderObject, crate::Slider, microui_contracts::Slider);
callable_leaf_class!(NumberObject, crate::Number, microui_contracts::Number);
leaf_class!(ColorSwatch, crate::ColorSwatch, IColorSwatch, IColorSwatchAbi, IFontStyle);
leaf_class!(ListItem, crate::ListItem, IListItem, IListItemAbi, IFontStyle, IIconContent, IActivationEvents);
leaf_class!(ListBox, crate::ListBox, IListBox, IListBoxAbi, IFontStyle, IActivationEvents, IImageContent);
callable_leaf_class!(ScrollbarObject, crate::Scrollbar, microui_contracts::Scrollbar);

impl CheckboxObject {
    /// Constructs the class for the host-bound factory through the same typed contract used by the registry.
    pub(super) fn create(
        host: ComPtr<IWidgetHostAbi>,
        checked: bool,
        label: NcCopyFromBufferOffset<'_, u8>,
    ) -> Result<ComPtr<CheckboxAbi>, NcStatus> {
        let value = <Self as Checkbox>::new(host.as_com_ref(), checked, label)?;
        Ok(Self::from_value(value)
            .query()
            .expect("CheckboxObject declares the Checkbox class contract"))
    }
}

impl Checkbox for CheckboxObject {
    /// Validates the label and initializes the established Checkbox defaults in one allocation.
    fn new(
        host: ComRef<'_, IWidgetHostAbi>,
        checked: bool,
        label: NcCopyFromBufferOffset<'_, u8>,
    ) -> Result<Self, NcStatus> {
        let parameters = crate::CheckboxParameters::new(text(label)?, checked);
        Ok(Self {
            widget: super::widget::WidgetCore::new(host.add_ref(), parameters.opt, crate::Checkbox::from(parameters))?,
        })
    }
    /// Reads the same boolean state used by pointer/Space updates and paint.
    fn checked(&self) -> Result<bool, NcStatus> {
        self.widget.state.readable()?;
        Ok(self.widget.take()?.checked())
    }
    /// Checked state changes paint only; both states reserve the same glyph space.
    fn set_checked(&self, checked: bool) -> Result<(), NcStatus> {
        self.widget.state.writable()?;
        self.widget.take()?.set_checked(checked);
        Ok(())
    }
    /// Copies a label into an independently owned immutable COM string.
    fn label(&self) -> Result<ComPtr<INcStringAbi>, NcStatus> {
        self.widget.state.readable()?;
        Ok(NcString::from_utf8_str(self.widget.take()?.com_label()))
    }
    /// Commits valid UTF-8 atomically and preserves a prior pending change.
    fn set_label(&self, input: NcCopyFromBufferOffset<'_, u8>) -> Result<(), NcStatus> {
        self.widget.state.writable()?;
        let input = text(input)?;
        let mut core = self.widget.take()?;
        if core.com_label() != input {
            core.com_set_label(input);
            self.widget.state.record_change(GeometryChange::CHANGED);
        }
        Ok(())
    }
    /// Validates a semantic font role before changing the Checkbox's measurement input.
    fn set_font_role(&self, role: TextFontRole) -> Result<(), NcStatus> {
        self.widget.set_font_role(role)
    }
    /// Resolves one exact named font before replacing the Checkbox's selection.
    fn set_named_font(&self, name: NcCopyFromBufferOffset<'_, u8>) -> Result<(), NcStatus> {
        self.widget.set_named_font(name)
    }
    /// Connects the Checkbox's existing semantic event source directly through its class contract.
    fn subscribe(&self, sink: ComRef<'_, ICheckboxSinkAbi>) -> Result<ComPtr<IEventSubscriptionAbi>, NcStatus> {
        self.widget.subscribe::<crate::CheckboxChanged>(sink)
    }
}

text_content!(ColorSwatch, label, set_label);
text_content!(ListItem, label, set_label);
text_content!(ListBox, com_label, com_set_label);

impl TextboxObject {
    /// Constructs the class for the host-bound factory through the registry's typed contract.
    pub(super) fn create(
        host: ComPtr<IWidgetHostAbi>,
        text: NcCopyFromBufferOffset<'_, u8>,
    ) -> Result<ComPtr<TextboxAbi>, NcStatus> {
        let value = <Self as Textbox>::new(host.as_com_ref(), text)?;
        Ok(Self::from_value(value)
            .query()
            .expect("TextboxObject declares the Textbox class contract"))
    }
}

impl Textbox for TextboxObject {
    /// Normalizes the complete input before allocating the established Textbox defaults.
    fn new(host: ComRef<'_, IWidgetHostAbi>, input: NcCopyFromBufferOffset<'_, u8>) -> Result<Self, NcStatus> {
        let parameters = crate::TextboxParameters::new(text(input)?);
        Ok(Self {
            widget: super::widget::WidgetCore::new(host.add_ref(), parameters.opt, crate::Textbox::from(parameters))?,
        })
    }
    /// Returns an immutable snapshot without exposing the semantic String allocation.
    fn text(&self) -> Result<ComPtr<INcStringAbi>, NcStatus> {
        self.widget.state.readable()?;
        Ok(NcString::from_utf8_str(self.widget.take()?.text()))
    }
    /// Applies single-line normalization before recording an actual geometry change.
    fn set_text(&self, input: NcCopyFromBufferOffset<'_, u8>) -> Result<(), NcStatus> {
        self.widget.state.writable()?;
        let input = text(input)?;
        let mut core = self.widget.take()?;
        let previous = core.text().to_owned();
        core.set_text(input);
        if core.text() != previous {
            self.widget.state.record_change(GeometryChange::CHANGED);
        }
        Ok(())
    }
    /// Reports a byte cursor rather than a glyph or grapheme index.
    fn cursor(&self) -> Result<u64, NcStatus> {
        self.widget.state.readable()?;
        Ok(self.widget.take()?.cursor() as u64)
    }
    /// Clamps oversize input before applying the shared Unicode-boundary algorithm.
    fn set_cursor(&self, cursor: u64) -> Result<(), NcStatus> {
        self.widget.state.writable()?;
        self.widget.take()?.set_cursor(usize::try_from(cursor).unwrap_or(usize::MAX));
        Ok(())
    }
    /// Validates a semantic font role before changing the Textbox's measurement input.
    fn set_font_role(&self, role: TextFontRole) -> Result<(), NcStatus> {
        self.widget.set_font_role(role)
    }
    /// Resolves one exact named font before replacing the Textbox's selection.
    fn set_named_font(&self, name: NcCopyFromBufferOffset<'_, u8>) -> Result<(), NcStatus> {
        self.widget.set_named_font(name)
    }
    /// Connects the existing changed-text source directly through the class contract.
    fn subscribe_changed(&self, sink: ComRef<'_, ITextChangedSinkAbi>) -> Result<ComPtr<IEventSubscriptionAbi>, NcStatus> {
        self.widget.subscribe::<crate::TextboxChanged>(sink)
    }
    /// Connects the independent submitted-text source directly through the class contract.
    fn subscribe_submitted(&self, sink: ComRef<'_, ITextSubmittedSinkAbi>) -> Result<ComPtr<IEventSubscriptionAbi>, NcStatus> {
        self.widget.subscribe::<crate::TextboxSubmitted>(sink)
    }
}

impl TextBlockObject {
    /// Constructs the class for the host-bound factory through the registry's typed contract.
    pub(super) fn create(
        host: ComPtr<IWidgetHostAbi>,
        text: NcCopyFromBufferOffset<'_, u8>,
        wrapping: TextWrapping,
    ) -> Result<ComPtr<TextBlockAbi>, NcStatus> {
        let value = <Self as TextBlock>::new(host.as_com_ref(), text, wrapping)?;
        Ok(Self::from_value(value)
            .query()
            .expect("TextBlockObject declares the TextBlock class contract"))
    }
}

impl TextBlock for TextBlockObject {
    /// Validates text and wrapping before allocating the established TextBlock defaults.
    fn new(
        host: ComRef<'_, IWidgetHostAbi>,
        input: NcCopyFromBufferOffset<'_, u8>,
        wrapping: TextWrapping,
    ) -> Result<Self, NcStatus> {
        let parameters = crate::TextBlockParameters::with_wrap(text(input)?, local_wrap(wrapping)?);
        Ok(Self {
            widget: super::widget::WidgetCore::new(host.add_ref(), parameters.opt, crate::TextBlock::from(parameters))?,
        })
    }
    /// Returns an immutable snapshot without exposing the semantic String allocation.
    fn text(&self) -> Result<ComPtr<INcStringAbi>, NcStatus> {
        self.widget.state.readable()?;
        Ok(NcString::from_utf8_str(self.widget.take()?.text()))
    }
    /// Replaces complete text and records only an actual geometry change.
    fn set_text(&self, input: NcCopyFromBufferOffset<'_, u8>) -> Result<(), NcStatus> {
        self.widget.state.writable()?;
        let input = text(input)?;
        let mut core = self.widget.take()?;
        let previous = core.text().to_owned();
        core.set_text(input);
        if core.text() != previous {
            self.widget.state.record_change(GeometryChange::CHANGED);
        }
        Ok(())
    }
    /// Encodes exactly the wrapping behavior used by existing text measurement.
    fn wrapping(&self) -> Result<TextWrapping, NcStatus> {
        self.widget.state.readable()?;
        Ok(match self.widget.take()?.com_wrap() {
            crate::TextWrap::None => TextWrapping::NONE,
            crate::TextWrap::Word => TextWrapping::WORD,
        })
    }
    /// Rejects unknown policies before mutating semantic state or its geometry marker.
    fn set_wrapping(&self, wrapping: TextWrapping) -> Result<(), NcStatus> {
        self.widget.state.writable()?;
        let wrap = local_wrap(wrapping)?;
        let mut core = self.widget.take()?;
        if core.com_wrap() != wrap {
            core.com_set_wrap(wrap);
            self.widget.state.record_change(GeometryChange::CHANGED);
        }
        Ok(())
    }
    /// Validates a semantic font role before changing the TextBlock's measurement input.
    fn set_font_role(&self, role: TextFontRole) -> Result<(), NcStatus> {
        self.widget.set_font_role(role)
    }
    /// Resolves one exact named font before replacing the TextBlock's selection.
    fn set_named_font(&self, name: NcCopyFromBufferOffset<'_, u8>) -> Result<(), NcStatus> {
        self.widget.set_named_font(name)
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

impl SliderObject {
    /// Constructs the class for the host-bound factory through the registry's typed contract.
    pub(super) fn create(host: ComPtr<IWidgetHostAbi>, options: SliderOptions) -> Result<ComPtr<SliderAbi>, NcStatus> {
        let value = <Self as Slider>::new(host.as_com_ref(), options)?;
        Ok(Self::from_value(value).query().expect("SliderObject declares the Slider class contract"))
    }
}

impl Slider for SliderObject {
    /// Validates the complete arithmetic tuple before allocating the established framed control.
    fn new(host: ComRef<'_, IWidgetHostAbi>, options: SliderOptions) -> Result<Self, NcStatus> {
        let parameters = crate::SliderParameters::with_opt(
            options.value,
            options.low,
            options.high,
            options.step,
            crate::DecimalPrecision::try_from(usize::from(options.precision)).map_err(|_| NC_E_INVALID_ARG)?,
            crate::WidgetOption::FRAME,
        )
        .map_err(|_| NC_E_INVALID_ARG)?;
        Ok(Self {
            widget: super::widget::WidgetCore::new(host.add_ref(), parameters.opt, crate::Slider::from(parameters))?,
        })
    }
    /// Reads the finite value owned by the slider's concrete semantic algorithm.
    fn value(&self) -> Result<f32, NcStatus> {
        self.widget.state.readable()?;
        Ok(self.widget.take()?.value())
    }
    /// Rejects nonfinite input and dirties geometry only when clamping or snapping changes the value.
    fn set_value(&self, value: f32) -> Result<(), NcStatus> {
        self.widget.state.writable()?;
        if !value.is_finite() {
            return Err(NC_E_INVALID_ARG);
        }
        let mut core = self.widget.take()?;
        let previous = core.value();
        core.set_value(value);
        if core.value() != previous {
            self.widget.state.record_change(GeometryChange::CHANGED);
        }
        Ok(())
    }
    /// Exposes the same edit mode that controls routed text input.
    fn is_editing(&self) -> Result<bool, NcStatus> {
        self.widget.state.readable()?;
        Ok(self.widget.take()?.is_editing())
    }
    /// Validates a semantic font role before changing the slider's measurement input.
    fn set_font_role(&self, role: TextFontRole) -> Result<(), NcStatus> {
        self.widget.set_font_role(role)
    }
    /// Resolves one exact named font before replacing the slider's selection.
    fn set_named_font(&self, name: NcCopyFromBufferOffset<'_, u8>) -> Result<(), NcStatus> {
        self.widget.set_named_font(name)
    }
    /// Connects the slider's existing semantic event source directly through the class contract.
    fn subscribe(&self, sink: ComRef<'_, INumericSinkAbi>) -> Result<ComPtr<IEventSubscriptionAbi>, NcStatus> {
        self.widget.subscribe::<crate::SliderChanged>(sink)
    }
}

impl NumberObject {
    /// Constructs the class for the host-bound factory through the registry's typed contract.
    pub(super) fn create(host: ComPtr<IWidgetHostAbi>, options: NumberOptions) -> Result<ComPtr<NumberAbi>, NcStatus> {
        let value = <Self as Number>::new(host.as_com_ref(), options)?;
        Ok(Self::from_value(value).query().expect("NumberObject declares the Number class contract"))
    }
}

impl Number for NumberObject {
    /// Validates the complete arithmetic tuple before allocating the numeric editor.
    fn new(host: ComRef<'_, IWidgetHostAbi>, options: NumberOptions) -> Result<Self, NcStatus> {
        let parameters = crate::NumberParameters::new(
            options.value,
            options.step,
            crate::DecimalPrecision::try_from(usize::from(options.precision)).map_err(|_| NC_E_INVALID_ARG)?,
        )
        .map_err(|_| NC_E_INVALID_ARG)?;
        Ok(Self {
            widget: super::widget::WidgetCore::new(host.add_ref(), parameters.opt, crate::Number::from(parameters))?,
        })
    }
    /// Reads the finite value owned by the number editor's concrete semantic algorithm.
    fn value(&self) -> Result<f32, NcStatus> {
        self.widget.state.readable()?;
        Ok(self.widget.take()?.value())
    }
    /// Rejects nonfinite input and dirties geometry only when the committed value changes.
    fn set_value(&self, value: f32) -> Result<(), NcStatus> {
        self.widget.state.writable()?;
        if !value.is_finite() {
            return Err(NC_E_INVALID_ARG);
        }
        let mut core = self.widget.take()?;
        let previous = core.value();
        core.set_value(value);
        if core.value() != previous {
            self.widget.state.record_change(GeometryChange::CHANGED);
        }
        Ok(())
    }
    /// Exposes the same edit mode that controls routed text input.
    fn is_editing(&self) -> Result<bool, NcStatus> {
        self.widget.state.readable()?;
        Ok(self.widget.take()?.is_editing())
    }
    /// Validates a semantic font role before changing the number editor's measurement input.
    fn set_font_role(&self, role: TextFontRole) -> Result<(), NcStatus> {
        self.widget.set_font_role(role)
    }
    /// Resolves one exact named font before replacing the number editor's selection.
    fn set_named_font(&self, name: NcCopyFromBufferOffset<'_, u8>) -> Result<(), NcStatus> {
        self.widget.set_named_font(name)
    }
    /// Connects the number editor's existing semantic event source directly through the class contract.
    fn subscribe(&self, sink: ComRef<'_, INumericSinkAbi>) -> Result<ComPtr<IEventSubscriptionAbi>, NcStatus> {
        self.widget.subscribe::<crate::NumberChanged>(sink)
    }
}

impl IListItem for ListItem {}
impl IListBox for ListBox {}

impl IColorSwatch for ColorSwatch {
    /// Packs canonical RGBA without relying on machine endianness.
    fn fill(&self) -> Result<u32, NcStatus> {
        self.widget.state.readable()?;
        let c = self.widget.take()?.fill();
        Ok(u32::from_le_bytes([c.r, c.g, c.b, c.a]))
    }
    /// Color is a paint-only input and emits no user event.
    fn set_fill(&self, rgba: u32) -> Result<(), NcStatus> {
        self.widget.state.writable()?;
        self.widget.take()?.set_fill(super::render::color(rgba));
        Ok(())
    }
}

impl ScrollbarObject {
    /// Builds a private structural scrollbar around an already shared axis model.
    pub(super) fn from_core(
        host: ComPtr<IWidgetHostAbi>,
        options: crate::WidgetOption,
        core: crate::Scrollbar,
    ) -> Result<ComPtr<ScrollbarAbi>, NcStatus> {
        Ok(Self::from_value(Self {
            widget: super::widget::WidgetCore::new(host, options, core)?,
        })
        .query()
        .expect("ScrollbarObject declares the Scrollbar class contract"))
    }

    /// Constructs the class for the host-bound factory through the same typed contract used by the registry.
    pub(super) fn create(host: ComPtr<IWidgetHostAbi>, range: ScrollRange) -> Result<ComPtr<ScrollbarAbi>, NcStatus> {
        let value = <Self as Scrollbar>::new(host.as_com_ref(), range)?;
        Ok(Self::from_value(value)
            .query()
            .expect("ScrollbarObject declares the Scrollbar class contract"))
    }
}

impl Scrollbar for ScrollbarObject {
    /// Validates orientation and lengths before allocating the established Scrollbar state.
    fn new(host: ComRef<'_, IWidgetHostAbi>, range: ScrollRange) -> Result<Self, NcStatus> {
        let axis = match range.axis {
            ScrollAxis::HORIZONTAL => crate::ScrollbarAxis::Horizontal,
            ScrollAxis::VERTICAL => crate::ScrollbarAxis::Vertical,
            _ => return Err(NC_E_INVALID_ARG),
        };
        if range.viewport < 0 || range.content < 0 {
            return Err(NC_E_INVALID_ARG);
        }
        let parameters = crate::ScrollbarParameters::new(axis).range(range.viewport, range.content, range.offset);
        Ok(Self {
            widget: super::widget::WidgetCore::new(host.add_ref(), parameters.opt, crate::Scrollbar::from(parameters))?,
        })
    }
    /// Reads all range fields from one semantic borrow.
    fn range(&self) -> Result<ScrollRange, NcStatus> {
        self.widget.state.readable()?;
        let core = self.widget.take()?;
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
        self.widget.state.writable()?;
        if viewport < 0 || content < 0 {
            return Err(NC_E_INVALID_ARG);
        }
        self.widget.take()?.set_lengths(viewport, content);
        Ok(())
    }
    /// Offset affects thumb paint only; the control's preferred size is fixed by its axis.
    fn set_offset(&self, offset: i32) -> Result<(), NcStatus> {
        self.widget.state.writable()?;
        let mut core = self.widget.take()?;
        let offset = offset.clamp(0, core.max_offset());
        core.set_offset(offset);
        Ok(())
    }
    /// Connects the Scrollbar's existing semantic event source directly through its class contract.
    fn subscribe(&self, sink: ComRef<'_, IScrollbarSinkAbi>) -> Result<ComPtr<IEventSubscriptionAbi>, NcStatus> {
        self.widget.subscribe::<crate::ScrollbarChanged>(sink)
    }
}

font_style_impl!(ColorSwatch);

font_style_impl!(ListBox);

font_style_impl!(ListItem);

icon_content_impl!(ListItem);

impl IActivationEvents for ListBox {
    /// Connects the one existing native semantic port without creating a parallel event source.
    fn subscribe(&self, sink: ComRef<'_, IActivationSinkAbi>) -> Result<ComPtr<IEventSubscriptionAbi>, NcStatus> {
        self.widget.subscribe::<crate::ListBoxSubmitted>(sink)
    }
}

impl IActivationEvents for ListItem {
    /// Connects the one existing native semantic port without creating a parallel event source.
    fn subscribe(&self, sink: ComRef<'_, IActivationSinkAbi>) -> Result<ComPtr<IEventSubscriptionAbi>, NcStatus> {
        self.widget.subscribe::<crate::ListItemSubmitted>(sink)
    }
}

super::widget::image_content_impl!(ListBox);
