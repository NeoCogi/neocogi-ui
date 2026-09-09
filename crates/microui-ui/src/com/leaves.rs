//! Concrete editing implementations over each built-in's one semantic state.
use super::*;
use super::widget::{
    HostedWidget, ImageState, set_direct_font_role, set_direct_icon_role, set_direct_named_font,
    set_direct_named_icon,
};
use nc_com::{INcStringAbi, NcString};

direct_widget_phase_impl!(crate::Checkbox);
direct_leaf_layout_impl!(crate::Checkbox);
direct_widget_phase_impl!(crate::TextBlock);
direct_leaf_layout_impl!(crate::TextBlock);
direct_widget_phase_impl!(crate::ColorSwatch);
direct_leaf_layout_impl!(crate::ColorSwatch);
direct_widget_phase_impl!(crate::ListItem);
direct_leaf_layout_impl!(crate::ListItem);
direct_widget_phase_impl!(crate::ListBox);
direct_leaf_layout_impl!(crate::ListBox);
direct_widget_phase_impl!(crate::Scrollbar);
direct_leaf_layout_impl!(crate::Scrollbar);
direct_widget_phase_impl!(crate::Textbox);
direct_leaf_layout_impl!(crate::Textbox);
direct_widget_phase_impl!(crate::Slider);
direct_leaf_layout_impl!(crate::Slider);
direct_widget_phase_impl!(crate::Number);
direct_leaf_layout_impl!(crate::Number);

impl crate::Checkbox {
    /// Constructs the class for the host-bound factory through the same typed contract used by the registry.
    pub(super) fn create_com(
        host: ComPtr<IWidgetHostAbi>,
        resources: super::widget::SharedAssets,
        checked: bool,
        label: NcCopyFromBufferOffset<'_, u8>,
    ) -> Result<ComPtr<CheckboxAbi>, NcStatus> {
        let value = Self::build(host, checked, label, Some(&resources))?;
        Ok(Self::from_value(value).query().expect("Checkbox declares its callable class contract"))
    }

    /// Builds one checkbox with either directly imported or runtime-shared resources.
    fn build(
        host: ComPtr<IWidgetHostAbi>,
        checked: bool,
        label: NcCopyFromBufferOffset<'_, u8>,
        resources: Option<&super::widget::SharedAssets>,
    ) -> Result<Self, NcStatus> {
        let parameters = crate::CheckboxParameters::new(text(label)?, checked);
        let runtime = super::widget::WidgetRuntime::for_resources(host, parameters.opt, resources)?;
        Ok(Self::from_runtime(runtime, parameters))
    }
}

impl microui_contracts::Checkbox for crate::Checkbox {
    /// Validates the label and initializes the established Checkbox defaults in one allocation.
    fn new(host: ComRef<'_, IWidgetHostAbi>, checked: bool, label: NcCopyFromBufferOffset<'_, u8>) -> Result<Self, NcStatus> {
        Self::build(host.add_ref(), checked, label, None)
    }
    /// Reads the same boolean state used by pointer/Space updates and paint.
    fn checked(&self) -> Result<bool, NcStatus> {
        self.runtime().state.readable()?;
        Ok(self.checked())
    }
    /// Checked state changes paint only; both states reserve the same glyph space.
    fn set_checked(&mut self, checked: bool) -> Result<(), NcStatus> {
        self.runtime().state.writable()?;
        self.set_checked(checked);
        Ok(())
    }
    /// Copies a label into an independently owned immutable COM string.
    fn label(&self) -> Result<ComPtr<INcStringAbi>, NcStatus> {
        self.runtime().state.readable()?;
        Ok(NcString::from_utf8_str(self.com_label()))
    }
    /// Commits valid UTF-8 atomically and preserves a prior pending change.
    fn set_label(&mut self, input: NcCopyFromBufferOffset<'_, u8>) -> Result<(), NcStatus> {
        self.runtime().state.writable()?;
        let input = text(input)?;
        if self.com_label() != input {
            self.com_set_label(input);
            self.runtime_mut().state.record_change(GeometryChange::CHANGED);
        }
        Ok(())
    }
    /// Validates a semantic font role before changing the Checkbox's measurement input.
    fn set_font_role(&mut self, role: TextFontRole) -> Result<(), NcStatus> {
        set_direct_font_role(self, role)
    }
    /// Resolves one exact named font before replacing the Checkbox's selection.
    fn set_named_font(&mut self, name: NcCopyFromBufferOffset<'_, u8>) -> Result<(), NcStatus> {
        set_direct_named_font(self, name)
    }
    /// Connects the Checkbox's existing semantic event source directly through its class contract.
    fn subscribe(&mut self, sink: ComRef<'_, ICheckboxSinkAbi>) -> Result<ComPtr<IEventSubscriptionAbi>, NcStatus> {
        self.runtime().state.writable()?;
        let endpoint = <Self as crate::TypedWidget<crate::CheckboxChanged>>::event(self);
        super::events::subscribe(endpoint, sink)
    }
}

impl crate::Textbox {
    /// Constructs the class for the host-bound factory through the registry's typed contract.
    pub(super) fn create_com(
        host: ComPtr<IWidgetHostAbi>,
        resources: super::widget::SharedAssets,
        text: NcCopyFromBufferOffset<'_, u8>,
    ) -> Result<ComPtr<TextboxAbi>, NcStatus> {
        let value = Self::build(host, text, Some(&resources))?;
        Ok(Self::from_value(value).query().expect("Textbox declares its callable class contract"))
    }

    /// Builds one textbox with either directly imported or runtime-shared resources.
    fn build(
        host: ComPtr<IWidgetHostAbi>,
        input: NcCopyFromBufferOffset<'_, u8>,
        resources: Option<&super::widget::SharedAssets>,
    ) -> Result<Self, NcStatus> {
        let parameters = crate::TextboxParameters::new(text(input)?);
        let runtime = super::widget::WidgetRuntime::for_resources(host, parameters.opt, resources)?;
        Ok(Self::from_runtime(runtime, parameters))
    }
}

impl microui_contracts::Textbox for crate::Textbox {
    /// Normalizes the complete input before allocating the established Textbox defaults.
    fn new(host: ComRef<'_, IWidgetHostAbi>, input: NcCopyFromBufferOffset<'_, u8>) -> Result<Self, NcStatus> {
        Self::build(host.add_ref(), input, None)
    }
    /// Returns an immutable snapshot without exposing the semantic String allocation.
    fn text(&self) -> Result<ComPtr<INcStringAbi>, NcStatus> {
        self.runtime().state.readable()?;
        Ok(NcString::from_utf8_str(self.text()))
    }
    /// Applies single-line normalization before recording an actual geometry change.
    fn set_text(&mut self, input: NcCopyFromBufferOffset<'_, u8>) -> Result<(), NcStatus> {
        self.runtime().state.writable()?;
        let input = text(input)?;
        let previous = self.text().to_owned();
        self.set_text(input);
        if self.text() != previous {
            self.runtime_mut().state.record_change(GeometryChange::CHANGED);
        }
        Ok(())
    }
    /// Reports a byte cursor rather than a glyph or grapheme index.
    fn cursor(&self) -> Result<u64, NcStatus> {
        self.runtime().state.readable()?;
        Ok(self.cursor() as u64)
    }
    /// Clamps oversize input before applying the shared Unicode-boundary algorithm.
    fn set_cursor(&mut self, cursor: u64) -> Result<(), NcStatus> {
        self.runtime().state.writable()?;
        self.set_cursor(usize::try_from(cursor).unwrap_or(usize::MAX));
        Ok(())
    }
    /// Validates a semantic font role before changing the Textbox's measurement input.
    fn set_font_role(&mut self, role: TextFontRole) -> Result<(), NcStatus> {
        set_direct_font_role(self, role)
    }
    /// Resolves one exact named font before replacing the Textbox's selection.
    fn set_named_font(&mut self, name: NcCopyFromBufferOffset<'_, u8>) -> Result<(), NcStatus> {
        set_direct_named_font(self, name)
    }
    /// Connects the existing changed-text source directly through the class contract.
    fn subscribe_changed(&mut self, sink: ComRef<'_, ITextChangedSinkAbi>) -> Result<ComPtr<IEventSubscriptionAbi>, NcStatus> {
        self.runtime().state.writable()?;
        let endpoint = <Self as crate::TypedWidget<crate::TextboxChanged>>::event(self);
        super::events::subscribe(endpoint, sink)
    }
    /// Connects the independent submitted-text source directly through the class contract.
    fn subscribe_submitted(&mut self, sink: ComRef<'_, ITextSubmittedSinkAbi>) -> Result<ComPtr<IEventSubscriptionAbi>, NcStatus> {
        self.runtime().state.writable()?;
        let endpoint = <Self as crate::TypedWidget<crate::TextboxSubmitted>>::event(self);
        super::events::subscribe(endpoint, sink)
    }
}

impl crate::TextBlock {
    /// Constructs the class for the host-bound factory through the registry's typed contract.
    pub(super) fn create_com(
        host: ComPtr<IWidgetHostAbi>,
        resources: super::widget::SharedAssets,
        text: NcCopyFromBufferOffset<'_, u8>,
        wrapping: TextWrapping,
    ) -> Result<ComPtr<TextBlockAbi>, NcStatus> {
        let value = Self::build(host, text, wrapping, Some(&resources))?;
        Ok(Self::from_value(value).query().expect("TextBlock declares its callable class contract"))
    }

    /// Builds one text block with either directly imported or runtime-shared resources.
    fn build(
        host: ComPtr<IWidgetHostAbi>,
        input: NcCopyFromBufferOffset<'_, u8>,
        wrapping: TextWrapping,
        resources: Option<&super::widget::SharedAssets>,
    ) -> Result<Self, NcStatus> {
        let parameters = crate::TextBlockParameters::with_wrap(text(input)?, local_wrap(wrapping)?);
        let runtime = super::widget::WidgetRuntime::for_resources(host, parameters.opt, resources)?;
        Ok(Self::from_runtime(runtime, parameters))
    }
}

impl microui_contracts::TextBlock for crate::TextBlock {
    /// Validates text and wrapping before allocating the established TextBlock defaults.
    fn new(host: ComRef<'_, IWidgetHostAbi>, input: NcCopyFromBufferOffset<'_, u8>, wrapping: TextWrapping) -> Result<Self, NcStatus> {
        Self::build(host.add_ref(), input, wrapping, None)
    }
    /// Returns an immutable snapshot without exposing the semantic String allocation.
    fn text(&self) -> Result<ComPtr<INcStringAbi>, NcStatus> {
        self.runtime().state.readable()?;
        Ok(NcString::from_utf8_str(self.text()))
    }
    /// Replaces complete text and records only an actual geometry change.
    fn set_text(&mut self, input: NcCopyFromBufferOffset<'_, u8>) -> Result<(), NcStatus> {
        self.runtime().state.writable()?;
        let input = text(input)?;
        let previous = self.text().to_owned();
        self.set_text(input);
        if self.text() != previous {
            self.runtime_mut().state.record_change(GeometryChange::CHANGED);
        }
        Ok(())
    }
    /// Encodes exactly the wrapping behavior used by existing text measurement.
    fn wrapping(&self) -> Result<TextWrapping, NcStatus> {
        self.runtime().state.readable()?;
        Ok(match self.com_wrap() {
            crate::TextWrap::None => TextWrapping::NONE,
            crate::TextWrap::Word => TextWrapping::WORD,
        })
    }
    /// Rejects unknown policies before mutating semantic state or its geometry marker.
    fn set_wrapping(&mut self, wrapping: TextWrapping) -> Result<(), NcStatus> {
        self.runtime().state.writable()?;
        let wrap = local_wrap(wrapping)?;
        if self.com_wrap() != wrap {
            self.com_set_wrap(wrap);
            self.runtime_mut().state.record_change(GeometryChange::CHANGED);
        }
        Ok(())
    }
    /// Validates a semantic font role before changing the TextBlock's measurement input.
    fn set_font_role(&mut self, role: TextFontRole) -> Result<(), NcStatus> {
        set_direct_font_role(self, role)
    }
    /// Resolves one exact named font before replacing the TextBlock's selection.
    fn set_named_font(&mut self, name: NcCopyFromBufferOffset<'_, u8>) -> Result<(), NcStatus> {
        set_direct_named_font(self, name)
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

impl crate::Slider {
    /// Constructs the class for the host-bound factory through the registry's typed contract.
    pub(super) fn create_com(host: ComPtr<IWidgetHostAbi>, resources: super::widget::SharedAssets, options: SliderOptions) -> Result<ComPtr<SliderAbi>, NcStatus> {
        let value = Self::build(host, options, Some(&resources))?;
        Ok(Self::from_value(value).query().expect("Slider declares its callable class contract"))
    }

    /// Builds one slider with either directly imported or runtime-shared resources.
    fn build(
        host: ComPtr<IWidgetHostAbi>,
        options: SliderOptions,
        resources: Option<&super::widget::SharedAssets>,
    ) -> Result<Self, NcStatus> {
        let parameters = crate::SliderParameters::with_opt(
            options.value,
            options.low,
            options.high,
            options.step,
            crate::DecimalPrecision::try_from(usize::from(options.precision)).map_err(|_| NC_E_INVALID_ARG)?,
            crate::WidgetOption::FRAME,
        )
        .map_err(|_| NC_E_INVALID_ARG)?;
        let runtime = super::widget::WidgetRuntime::for_resources(host, parameters.opt, resources)?;
        Ok(Self::from_runtime(runtime, parameters))
    }
}

impl microui_contracts::Slider for crate::Slider {
    /// Validates the complete arithmetic tuple before allocating the established framed control.
    fn new(host: ComRef<'_, IWidgetHostAbi>, options: SliderOptions) -> Result<Self, NcStatus> {
        Self::build(host.add_ref(), options, None)
    }
    /// Reads the finite value owned by the slider's concrete semantic algorithm.
    fn value(&self) -> Result<f32, NcStatus> {
        self.runtime().state.readable()?;
        Ok(self.value())
    }
    /// Rejects nonfinite input and dirties geometry only when clamping or snapping changes the value.
    fn set_value(&mut self, value: f32) -> Result<(), NcStatus> {
        self.runtime().state.writable()?;
        if !value.is_finite() {
            return Err(NC_E_INVALID_ARG);
        }
        let previous = self.value();
        self.set_value(value);
        if self.value() != previous {
            self.runtime_mut().state.record_change(GeometryChange::CHANGED);
        }
        Ok(())
    }
    /// Exposes the same edit mode that controls routed text input.
    fn is_editing(&self) -> Result<bool, NcStatus> {
        self.runtime().state.readable()?;
        Ok(self.is_editing())
    }
    /// Validates a semantic font role before changing the slider's measurement input.
    fn set_font_role(&mut self, role: TextFontRole) -> Result<(), NcStatus> {
        set_direct_font_role(self, role)
    }
    /// Resolves one exact named font before replacing the slider's selection.
    fn set_named_font(&mut self, name: NcCopyFromBufferOffset<'_, u8>) -> Result<(), NcStatus> {
        set_direct_named_font(self, name)
    }
    /// Connects the slider's existing semantic event source directly through the class contract.
    fn subscribe(&mut self, sink: ComRef<'_, INumericSinkAbi>) -> Result<ComPtr<IEventSubscriptionAbi>, NcStatus> {
        self.runtime().state.writable()?;
        let endpoint = <Self as crate::TypedWidget<crate::SliderChanged>>::event(self);
        super::events::subscribe(endpoint, sink)
    }
}

impl crate::Number {
    /// Constructs the class for the host-bound factory through the registry's typed contract.
    pub(super) fn create_com(host: ComPtr<IWidgetHostAbi>, resources: super::widget::SharedAssets, options: NumberOptions) -> Result<ComPtr<NumberAbi>, NcStatus> {
        let value = Self::build(host, options, Some(&resources))?;
        Ok(Self::from_value(value).query().expect("Number declares its callable class contract"))
    }

    /// Builds one number editor with either directly imported or runtime-shared resources.
    fn build(
        host: ComPtr<IWidgetHostAbi>,
        options: NumberOptions,
        resources: Option<&super::widget::SharedAssets>,
    ) -> Result<Self, NcStatus> {
        let parameters = crate::NumberParameters::new(
            options.value,
            options.step,
            crate::DecimalPrecision::try_from(usize::from(options.precision)).map_err(|_| NC_E_INVALID_ARG)?,
        )
        .map_err(|_| NC_E_INVALID_ARG)?;
        let runtime = super::widget::WidgetRuntime::for_resources(host, parameters.opt, resources)?;
        Ok(Self::from_runtime(runtime, parameters))
    }
}

impl microui_contracts::Number for crate::Number {
    /// Validates the complete arithmetic tuple before allocating the numeric editor.
    fn new(host: ComRef<'_, IWidgetHostAbi>, options: NumberOptions) -> Result<Self, NcStatus> {
        Self::build(host.add_ref(), options, None)
    }
    /// Reads the finite value owned by the number editor's concrete semantic algorithm.
    fn value(&self) -> Result<f32, NcStatus> {
        self.runtime().state.readable()?;
        Ok(self.value())
    }
    /// Rejects nonfinite input and dirties geometry only when the committed value changes.
    fn set_value(&mut self, value: f32) -> Result<(), NcStatus> {
        self.runtime().state.writable()?;
        if !value.is_finite() {
            return Err(NC_E_INVALID_ARG);
        }
        let previous = self.value();
        self.set_value(value);
        if self.value() != previous {
            self.runtime_mut().state.record_change(GeometryChange::CHANGED);
        }
        Ok(())
    }
    /// Exposes the same edit mode that controls routed text input.
    fn is_editing(&self) -> Result<bool, NcStatus> {
        self.runtime().state.readable()?;
        Ok(self.is_editing())
    }
    /// Validates a semantic font role before changing the number editor's measurement input.
    fn set_font_role(&mut self, role: TextFontRole) -> Result<(), NcStatus> {
        set_direct_font_role(self, role)
    }
    /// Resolves one exact named font before replacing the number editor's selection.
    fn set_named_font(&mut self, name: NcCopyFromBufferOffset<'_, u8>) -> Result<(), NcStatus> {
        set_direct_named_font(self, name)
    }
    /// Connects the number editor's existing semantic event source directly through the class contract.
    fn subscribe(&mut self, sink: ComRef<'_, INumericSinkAbi>) -> Result<ComPtr<IEventSubscriptionAbi>, NcStatus> {
        self.runtime().state.writable()?;
        let endpoint = <Self as crate::TypedWidget<crate::NumberChanged>>::event(self);
        super::events::subscribe(endpoint, sink)
    }
}

impl crate::ColorSwatch {
    /// Constructs the class for the host-bound factory through the registry's typed contract.
    pub(super) fn create_com(
        host: ComPtr<IWidgetHostAbi>,
        resources: super::widget::SharedAssets,
        rgba: u32,
        label: NcCopyFromBufferOffset<'_, u8>,
    ) -> Result<ComPtr<ColorSwatchAbi>, NcStatus> {
        let value = Self::build(host, rgba, label, Some(&resources))?;
        Ok(Self::from_value(value)
            .query()
            .expect("ColorSwatch declares its callable class contract"))
    }

    /// Builds one color swatch with either directly imported or runtime-shared resources.
    fn build(
        host: ComPtr<IWidgetHostAbi>,
        rgba: u32,
        label: NcCopyFromBufferOffset<'_, u8>,
        resources: Option<&super::widget::SharedAssets>,
    ) -> Result<Self, NcStatus> {
        let parameters = crate::ColorSwatchParameters::new(super::render::color(rgba)).label(text(label)?);
        let runtime = super::widget::WidgetRuntime::for_resources(host, parameters.opt, resources)?;
        Ok(Self::from_runtime(runtime, parameters))
    }
}

impl microui_contracts::ColorSwatch for crate::ColorSwatch {
    /// Validates the label and copies packed RGBA before allocating the established swatch state.
    fn new(host: ComRef<'_, IWidgetHostAbi>, rgba: u32, label: NcCopyFromBufferOffset<'_, u8>) -> Result<Self, NcStatus> {
        Self::build(host.add_ref(), rgba, label, None)
    }
    /// Returns an immutable snapshot without exposing the semantic String allocation.
    fn text(&self) -> Result<ComPtr<INcStringAbi>, NcStatus> {
        self.runtime().state.readable()?;
        Ok(NcString::from_utf8_str(self.label()))
    }
    /// Validates UTF-8 and records geometry only when the concrete label changes.
    fn set_text(&mut self, input: NcCopyFromBufferOffset<'_, u8>) -> Result<(), NcStatus> {
        self.runtime().state.writable()?;
        let label = text(input)?;
        if self.label() != label {
            self.set_label(label);
            self.runtime_mut().state.record_change(GeometryChange::CHANGED);
        }
        Ok(())
    }
    /// Packs canonical RGBA without relying on machine endianness.
    fn fill(&self) -> Result<u32, NcStatus> {
        self.runtime().state.readable()?;
        let c = self.fill();
        Ok(u32::from_le_bytes([c.r, c.g, c.b, c.a]))
    }
    /// Color is a paint-only input and emits no user event.
    fn set_fill(&mut self, rgba: u32) -> Result<(), NcStatus> {
        self.runtime().state.writable()?;
        self.set_fill(super::render::color(rgba));
        Ok(())
    }
    /// Validates a semantic role before changing the swatch label's font.
    fn set_font_role(&mut self, role: TextFontRole) -> Result<(), NcStatus> {
        set_direct_font_role(self, role)
    }
    /// Resolves one exact named font before changing the swatch label's font.
    fn set_named_font(&mut self, name: NcCopyFromBufferOffset<'_, u8>) -> Result<(), NcStatus> {
        set_direct_named_font(self, name)
    }
}

impl crate::ListItem {
    /// Constructs the class for the host-bound factory through the registry's typed contract.
    pub(super) fn create_com(
        host: ComPtr<IWidgetHostAbi>,
        resources: super::widget::SharedAssets,
        label: NcCopyFromBufferOffset<'_, u8>,
    ) -> Result<ComPtr<ListItemAbi>, NcStatus> {
        let value = Self::build(host, label, Some(&resources))?;
        Ok(Self::from_value(value).query().expect("ListItem declares its callable class contract"))
    }

    /// Builds one list item with either directly imported or runtime-shared resources.
    fn build(
        host: ComPtr<IWidgetHostAbi>,
        label: NcCopyFromBufferOffset<'_, u8>,
        resources: Option<&super::widget::SharedAssets>,
    ) -> Result<Self, NcStatus> {
        let parameters = crate::ListItemParameters::new(text(label)?);
        let runtime = super::widget::WidgetRuntime::for_resources(host, parameters.opt, resources)?;
        Ok(Self::from_runtime(runtime, parameters))
    }
}

impl microui_contracts::ListItem for crate::ListItem {
    /// Validates the label before allocating the established application-data-free row state.
    fn new(host: ComRef<'_, IWidgetHostAbi>, label: NcCopyFromBufferOffset<'_, u8>) -> Result<Self, NcStatus> {
        Self::build(host.add_ref(), label, None)
    }
    /// Returns an immutable label snapshot from the concrete row.
    fn text(&self) -> Result<ComPtr<INcStringAbi>, NcStatus> {
        self.runtime().state.readable()?;
        Ok(NcString::from_utf8_str(self.label()))
    }
    /// Validates UTF-8 and records geometry only when the concrete label changes.
    fn set_text(&mut self, input: NcCopyFromBufferOffset<'_, u8>) -> Result<(), NcStatus> {
        self.runtime().state.writable()?;
        let label = text(input)?;
        if self.label() != label {
            self.set_label(label);
            self.runtime_mut().state.record_change(GeometryChange::CHANGED);
        }
        Ok(())
    }
    /// Validates a semantic role before changing the row label's font.
    fn set_font_role(&mut self, role: TextFontRole) -> Result<(), NcStatus> {
        set_direct_font_role(self, role)
    }
    /// Resolves one exact named font before changing the row label's font.
    fn set_named_font(&mut self, name: NcCopyFromBufferOffset<'_, u8>) -> Result<(), NcStatus> {
        set_direct_named_font(self, name)
    }
    /// Validates and applies one semantic icon selection directly through the class contract.
    fn set_icon_role(&mut self, icon: ContentIcon) -> Result<(), NcStatus> {
        set_direct_icon_role(self, icon)
    }
    /// Resolves one exact named icon before replacing the row's existing selection.
    fn set_named_icon(&mut self, name: NcCopyFromBufferOffset<'_, u8>) -> Result<(), NcStatus> {
        set_direct_named_icon(self, name)
    }
    /// Connects the row's existing semantic event source directly through the class contract.
    fn subscribe(&mut self, sink: ComRef<'_, IActivationSinkAbi>) -> Result<ComPtr<IEventSubscriptionAbi>, NcStatus> {
        self.runtime().state.writable()?;
        let endpoint = <Self as crate::TypedWidget<crate::ListItemSubmitted>>::event(self);
        super::events::subscribe(endpoint, sink)
    }
}

impl crate::ListBox {
    /// Constructs the class for the host-bound factory through the registry's typed contract.
    pub(super) fn create_com(
        host: ComPtr<IWidgetHostAbi>,
        resources: super::widget::SharedAssets,
        label: NcCopyFromBufferOffset<'_, u8>,
    ) -> Result<ComPtr<ListBoxAbi>, NcStatus> {
        let value = Self::build(host, label, Some(&resources))?;
        Ok(Self::from_value(value).query().expect("ListBox declares its callable class contract"))
    }

    /// Builds one list box with either directly imported or runtime-shared resources.
    fn build(
        host: ComPtr<IWidgetHostAbi>,
        label: NcCopyFromBufferOffset<'_, u8>,
        resources: Option<&super::widget::SharedAssets>,
    ) -> Result<Self, NcStatus> {
        let parameters = crate::ListBoxParameters::new(text(label)?, None);
        let runtime = super::widget::WidgetRuntime::for_resources(host, parameters.opt, resources)?;
        Ok(Self::from_runtime(runtime, parameters))
    }
}

impl microui_contracts::ListBox for crate::ListBox {
    /// Validates the label before allocating the established label-only entry state.
    fn new(host: ComRef<'_, IWidgetHostAbi>, label: NcCopyFromBufferOffset<'_, u8>) -> Result<Self, NcStatus> {
        Self::build(host.add_ref(), label, None)
    }
    /// Returns an immutable label snapshot from the concrete list box.
    fn text(&self) -> Result<ComPtr<INcStringAbi>, NcStatus> {
        self.runtime().state.readable()?;
        Ok(NcString::from_utf8_str(self.com_label()))
    }
    /// Validates UTF-8 and records geometry only when the concrete label changes.
    fn set_text(&mut self, input: NcCopyFromBufferOffset<'_, u8>) -> Result<(), NcStatus> {
        self.runtime().state.writable()?;
        let label = text(input)?;
        if self.com_label() != label {
            self.com_set_label(label);
            self.runtime_mut().state.record_change(GeometryChange::CHANGED);
        }
        Ok(())
    }
    /// Validates a semantic role before changing the list-box label's font.
    fn set_font_role(&mut self, role: TextFontRole) -> Result<(), NcStatus> {
        set_direct_font_role(self, role)
    }
    /// Resolves one exact named font before changing the list-box label's font.
    fn set_named_font(&mut self, name: NcCopyFromBufferOffset<'_, u8>) -> Result<(), NcStatus> {
        set_direct_named_font(self, name)
    }
    /// Validates and installs one immutable renderer-owned image before committing the edit.
    fn set_image(&mut self, image: ComRef<'_, ITextureAbi>, fit_width: bool) -> Result<(), NcStatus> {
        self.runtime().state.writable()?;
        let image = microui_redux::Image::from_interface(image.add_ref())?;
        // Revalidate lifetime authority after the foreign size query before committing the
        // prepared image; the generated object gate rejects recursive mutable entry.
        self.runtime().state.writable()?;
        if ImageState::set_image(self, Some(image), fit_width) {
            self.runtime_mut().state.record_change(GeometryChange::CHANGED);
        }
        Ok(())
    }
    /// Removes image content while preserving the label, font, and activation source.
    fn clear_image(&mut self) -> Result<(), NcStatus> {
        self.runtime().state.writable()?;
        if ImageState::set_image(self, None, false) {
            self.runtime_mut().state.record_change(GeometryChange::CHANGED);
        }
        Ok(())
    }
    /// Connects the entry's existing semantic event source directly through the class contract.
    fn subscribe(&mut self, sink: ComRef<'_, IActivationSinkAbi>) -> Result<ComPtr<IEventSubscriptionAbi>, NcStatus> {
        self.runtime().state.writable()?;
        let endpoint = <Self as crate::TypedWidget<crate::ListBoxSubmitted>>::event(self);
        super::events::subscribe(endpoint, sink)
    }
}

impl crate::Scrollbar {
    /// Converts an already host-bound structural scrollbar into its public COM capability.
    pub(super) fn into_com(self) -> ComPtr<ScrollbarAbi> {
        Self::from_value(self)
            .query()
            .expect("Scrollbar declares its callable class contract")
    }

    /// Constructs the class for the host-bound factory through the same typed contract used by the registry.
    pub(super) fn create_com(host: ComPtr<IWidgetHostAbi>, resources: super::widget::SharedAssets, range: ScrollRange) -> Result<ComPtr<ScrollbarAbi>, NcStatus> {
        let value = Self::build(host, range, Some(&resources))?;
        Ok(Self::from_value(value).query().expect("Scrollbar declares its callable class contract"))
    }

    /// Builds one scrollbar with either directly imported or runtime-shared resources.
    fn build(
        host: ComPtr<IWidgetHostAbi>,
        range: ScrollRange,
        resources: Option<&super::widget::SharedAssets>,
    ) -> Result<Self, NcStatus> {
        let axis = match range.axis {
            ScrollAxis::HORIZONTAL => crate::ScrollbarAxis::Horizontal,
            ScrollAxis::VERTICAL => crate::ScrollbarAxis::Vertical,
            _ => return Err(NC_E_INVALID_ARG),
        };
        if range.viewport < 0 || range.content < 0 {
            return Err(NC_E_INVALID_ARG);
        }
        let parameters = crate::ScrollbarParameters::new(axis).range(range.viewport, range.content, range.offset);
        let runtime = super::widget::WidgetRuntime::for_resources(host, parameters.opt, resources)?;
        Ok(Self::from_runtime(runtime, parameters))
    }
}

impl microui_contracts::Scrollbar for crate::Scrollbar {
    /// Validates orientation and lengths before allocating the established Scrollbar state.
    fn new(host: ComRef<'_, IWidgetHostAbi>, range: ScrollRange) -> Result<Self, NcStatus> {
        Self::build(host.add_ref(), range, None)
    }
    /// Reads all range fields from one consistent semantic snapshot.
    fn range(&self) -> Result<ScrollRange, NcStatus> {
        self.runtime().state.readable()?;
        let (viewport, content) = self.com_lengths();
        let axis = match self.axis() {
            crate::ScrollbarAxis::Horizontal => ScrollAxis::HORIZONTAL,
            crate::ScrollbarAxis::Vertical => ScrollAxis::VERTICAL,
        };
        Ok(ScrollRange {
            axis,
            viewport,
            content,
            offset: self.offset(),
        })
    }
    /// Validates both lengths before clamping the existing offset in one commit.
    fn set_lengths(&mut self, viewport: i32, content: i32) -> Result<(), NcStatus> {
        self.runtime().state.writable()?;
        if viewport < 0 || content < 0 {
            return Err(NC_E_INVALID_ARG);
        }
        self.set_lengths(viewport, content);
        Ok(())
    }
    /// Offset affects thumb paint only; the control's preferred size is fixed by its axis.
    fn set_offset(&mut self, offset: i32) -> Result<(), NcStatus> {
        self.runtime().state.writable()?;
        let offset = offset.clamp(0, self.max_offset());
        self.set_offset(offset);
        Ok(())
    }
    /// Connects the Scrollbar's existing semantic event source directly through its class contract.
    fn subscribe(&mut self, sink: ComRef<'_, IScrollbarSinkAbi>) -> Result<ComPtr<IEventSubscriptionAbi>, NcStatus> {
        self.runtime().state.writable()?;
        let endpoint = <Self as crate::TypedWidget<crate::ScrollbarChanged>>::event(self);
        super::events::subscribe(endpoint, sink)
    }
}
