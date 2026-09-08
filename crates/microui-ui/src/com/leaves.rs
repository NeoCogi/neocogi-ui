//! Concrete editing implementations over each built-in's one semantic state.
use super::*;
use super::widget::{FontState, HostedWidget, IconState, ImageState};
use nc_com::{INcStringAbi, NcString};

callable_leaf_class!(TextboxObject, crate::Textbox, microui_contracts::Textbox);
callable_leaf_class!(SliderObject, crate::Slider, microui_contracts::Slider);
callable_leaf_class!(NumberObject, crate::Number, microui_contracts::Number);

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

impl crate::Checkbox {
    /// Constructs the class for the host-bound factory through the same typed contract used by the registry.
    pub(super) fn create_com(host: ComPtr<IWidgetHostAbi>, checked: bool, label: NcCopyFromBufferOffset<'_, u8>) -> Result<ComPtr<CheckboxAbi>, NcStatus> {
        let value = <Self as microui_contracts::Checkbox>::new(host.as_com_ref(), checked, label)?;
        Ok(Self::from_value(value).query().expect("Checkbox declares its callable class contract"))
    }
}

impl microui_contracts::Checkbox for crate::Checkbox {
    /// Validates the label and initializes the established Checkbox defaults in one allocation.
    fn new(host: ComRef<'_, IWidgetHostAbi>, checked: bool, label: NcCopyFromBufferOffset<'_, u8>) -> Result<Self, NcStatus> {
        let parameters = crate::CheckboxParameters::new(text(label)?, checked);
        let runtime = super::widget::WidgetRuntime::new(host.add_ref(), parameters.opt)?;
        Ok(Self::from_runtime(runtime, parameters))
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

/// Commits one validated semantic font role directly on a concrete widget allocation.
fn set_direct_font_role<S: HostedWidget + FontState>(widget: &mut S, role: TextFontRole) -> Result<(), NcStatus> {
    widget.runtime().state.writable()?;
    let font = crate::FontRef::Role(super::widget::local_font_role(role)?);
    if widget.font() != &font {
        widget.set_font(font);
        widget.runtime_mut().state.record_change(GeometryChange::CHANGED);
    }
    Ok(())
}

/// Resolves one application font name against the widget host's current resource revision.
fn set_direct_named_font<S: HostedWidget + FontState>(widget: &mut S, name: NcCopyFromBufferOffset<'_, u8>) -> Result<(), NcStatus> {
    let font = widget.runtime_mut().named_font(name)?;
    if widget.font() != &font {
        widget.set_font(font);
        widget.runtime_mut().state.record_change(GeometryChange::CHANGED);
    }
    Ok(())
}

/// Commits one validated semantic icon directly on a concrete widget allocation.
fn set_direct_icon_role<S: HostedWidget + IconState>(widget: &mut S, icon: ContentIcon) -> Result<(), NcStatus> {
    widget.runtime().state.writable()?;
    if widget.set_icon(super::widget::local_content_icon(icon)?) {
        widget.runtime_mut().state.record_change(GeometryChange::CHANGED);
    }
    Ok(())
}

/// Resolves and commits one exact named icon directly on a concrete widget allocation.
fn set_direct_named_icon<S: HostedWidget + IconState>(widget: &mut S, name: NcCopyFromBufferOffset<'_, u8>) -> Result<(), NcStatus> {
    let icon = widget.runtime_mut().named_icon(name)?;
    if widget.set_icon(Some(icon)) {
        widget.runtime_mut().state.record_change(GeometryChange::CHANGED);
    }
    Ok(())
}

impl TextboxObject {
    /// Constructs the class for the host-bound factory through the registry's typed contract.
    pub(super) fn create(host: ComPtr<IWidgetHostAbi>, text: NcCopyFromBufferOffset<'_, u8>) -> Result<ComPtr<TextboxAbi>, NcStatus> {
        let value = <Self as Textbox>::new(host.as_com_ref(), text)?;
        Ok(Self::from_value(value).query().expect("TextboxObject declares the Textbox class contract"))
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
        Ok(NcString::from_utf8_str(self.widget.core.text()))
    }
    /// Applies single-line normalization before recording an actual geometry change.
    fn set_text(&mut self, input: NcCopyFromBufferOffset<'_, u8>) -> Result<(), NcStatus> {
        self.widget.state.writable()?;
        let input = text(input)?;
        let core = &mut self.widget.core;
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
        Ok(self.widget.core.cursor() as u64)
    }
    /// Clamps oversize input before applying the shared Unicode-boundary algorithm.
    fn set_cursor(&mut self, cursor: u64) -> Result<(), NcStatus> {
        self.widget.state.writable()?;
        self.widget.core.set_cursor(usize::try_from(cursor).unwrap_or(usize::MAX));
        Ok(())
    }
    /// Validates a semantic font role before changing the Textbox's measurement input.
    fn set_font_role(&mut self, role: TextFontRole) -> Result<(), NcStatus> {
        self.widget.set_font_role(role)
    }
    /// Resolves one exact named font before replacing the Textbox's selection.
    fn set_named_font(&mut self, name: NcCopyFromBufferOffset<'_, u8>) -> Result<(), NcStatus> {
        self.widget.set_named_font(name)
    }
    /// Connects the existing changed-text source directly through the class contract.
    fn subscribe_changed(&mut self, sink: ComRef<'_, ITextChangedSinkAbi>) -> Result<ComPtr<IEventSubscriptionAbi>, NcStatus> {
        self.widget.subscribe::<crate::TextboxChanged>(sink)
    }
    /// Connects the independent submitted-text source directly through the class contract.
    fn subscribe_submitted(&mut self, sink: ComRef<'_, ITextSubmittedSinkAbi>) -> Result<ComPtr<IEventSubscriptionAbi>, NcStatus> {
        self.widget.subscribe::<crate::TextboxSubmitted>(sink)
    }
}

impl crate::TextBlock {
    /// Constructs the class for the host-bound factory through the registry's typed contract.
    pub(super) fn create_com(host: ComPtr<IWidgetHostAbi>, text: NcCopyFromBufferOffset<'_, u8>, wrapping: TextWrapping) -> Result<ComPtr<TextBlockAbi>, NcStatus> {
        let value = <Self as microui_contracts::TextBlock>::new(host.as_com_ref(), text, wrapping)?;
        Ok(Self::from_value(value).query().expect("TextBlock declares its callable class contract"))
    }
}

impl microui_contracts::TextBlock for crate::TextBlock {
    /// Validates text and wrapping before allocating the established TextBlock defaults.
    fn new(host: ComRef<'_, IWidgetHostAbi>, input: NcCopyFromBufferOffset<'_, u8>, wrapping: TextWrapping) -> Result<Self, NcStatus> {
        let parameters = crate::TextBlockParameters::with_wrap(text(input)?, local_wrap(wrapping)?);
        let runtime = super::widget::WidgetRuntime::new(host.add_ref(), parameters.opt)?;
        Ok(Self::from_runtime(runtime, parameters))
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
        Ok(self.widget.core.value())
    }
    /// Rejects nonfinite input and dirties geometry only when clamping or snapping changes the value.
    fn set_value(&mut self, value: f32) -> Result<(), NcStatus> {
        self.widget.state.writable()?;
        if !value.is_finite() {
            return Err(NC_E_INVALID_ARG);
        }
        let core = &mut self.widget.core;
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
        Ok(self.widget.core.is_editing())
    }
    /// Validates a semantic font role before changing the slider's measurement input.
    fn set_font_role(&mut self, role: TextFontRole) -> Result<(), NcStatus> {
        self.widget.set_font_role(role)
    }
    /// Resolves one exact named font before replacing the slider's selection.
    fn set_named_font(&mut self, name: NcCopyFromBufferOffset<'_, u8>) -> Result<(), NcStatus> {
        self.widget.set_named_font(name)
    }
    /// Connects the slider's existing semantic event source directly through the class contract.
    fn subscribe(&mut self, sink: ComRef<'_, INumericSinkAbi>) -> Result<ComPtr<IEventSubscriptionAbi>, NcStatus> {
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
        Ok(self.widget.core.value())
    }
    /// Rejects nonfinite input and dirties geometry only when the committed value changes.
    fn set_value(&mut self, value: f32) -> Result<(), NcStatus> {
        self.widget.state.writable()?;
        if !value.is_finite() {
            return Err(NC_E_INVALID_ARG);
        }
        let core = &mut self.widget.core;
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
        Ok(self.widget.core.is_editing())
    }
    /// Validates a semantic font role before changing the number editor's measurement input.
    fn set_font_role(&mut self, role: TextFontRole) -> Result<(), NcStatus> {
        self.widget.set_font_role(role)
    }
    /// Resolves one exact named font before replacing the number editor's selection.
    fn set_named_font(&mut self, name: NcCopyFromBufferOffset<'_, u8>) -> Result<(), NcStatus> {
        self.widget.set_named_font(name)
    }
    /// Connects the number editor's existing semantic event source directly through the class contract.
    fn subscribe(&mut self, sink: ComRef<'_, INumericSinkAbi>) -> Result<ComPtr<IEventSubscriptionAbi>, NcStatus> {
        self.widget.subscribe::<crate::NumberChanged>(sink)
    }
}

impl crate::ColorSwatch {
    /// Constructs the class for the host-bound factory through the registry's typed contract.
    pub(super) fn create_com(host: ComPtr<IWidgetHostAbi>, rgba: u32, label: NcCopyFromBufferOffset<'_, u8>) -> Result<ComPtr<ColorSwatchAbi>, NcStatus> {
        let value = <Self as microui_contracts::ColorSwatch>::new(host.as_com_ref(), rgba, label)?;
        Ok(Self::from_value(value)
            .query()
            .expect("ColorSwatch declares its callable class contract"))
    }
}

impl microui_contracts::ColorSwatch for crate::ColorSwatch {
    /// Validates the label and copies packed RGBA before allocating the established swatch state.
    fn new(host: ComRef<'_, IWidgetHostAbi>, rgba: u32, label: NcCopyFromBufferOffset<'_, u8>) -> Result<Self, NcStatus> {
        let parameters = crate::ColorSwatchParameters::new(super::render::color(rgba)).label(text(label)?);
        let runtime = super::widget::WidgetRuntime::new(host.add_ref(), parameters.opt)?;
        Ok(Self::from_runtime(runtime, parameters))
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
    pub(super) fn create_com(host: ComPtr<IWidgetHostAbi>, label: NcCopyFromBufferOffset<'_, u8>) -> Result<ComPtr<ListItemAbi>, NcStatus> {
        let value = <Self as microui_contracts::ListItem>::new(host.as_com_ref(), label)?;
        Ok(Self::from_value(value).query().expect("ListItem declares its callable class contract"))
    }
}

impl microui_contracts::ListItem for crate::ListItem {
    /// Validates the label before allocating the established application-data-free row state.
    fn new(host: ComRef<'_, IWidgetHostAbi>, label: NcCopyFromBufferOffset<'_, u8>) -> Result<Self, NcStatus> {
        let parameters = crate::ListItemParameters::new(text(label)?);
        let runtime = super::widget::WidgetRuntime::new(host.add_ref(), parameters.opt)?;
        Ok(Self::from_runtime(runtime, parameters))
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
    pub(super) fn create_com(host: ComPtr<IWidgetHostAbi>, label: NcCopyFromBufferOffset<'_, u8>) -> Result<ComPtr<ListBoxAbi>, NcStatus> {
        let value = <Self as microui_contracts::ListBox>::new(host.as_com_ref(), label)?;
        Ok(Self::from_value(value).query().expect("ListBox declares its callable class contract"))
    }
}

impl microui_contracts::ListBox for crate::ListBox {
    /// Validates the label before allocating the established label-only entry state.
    fn new(host: ComRef<'_, IWidgetHostAbi>, label: NcCopyFromBufferOffset<'_, u8>) -> Result<Self, NcStatus> {
        let parameters = crate::ListBoxParameters::new(text(label)?, None);
        let runtime = super::widget::WidgetRuntime::new(host.add_ref(), parameters.opt)?;
        Ok(Self::from_runtime(runtime, parameters))
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
    pub(super) fn create_com(host: ComPtr<IWidgetHostAbi>, range: ScrollRange) -> Result<ComPtr<ScrollbarAbi>, NcStatus> {
        let value = <Self as microui_contracts::Scrollbar>::new(host.as_com_ref(), range)?;
        Ok(Self::from_value(value).query().expect("Scrollbar declares its callable class contract"))
    }
}

impl microui_contracts::Scrollbar for crate::Scrollbar {
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
        let runtime = super::widget::WidgetRuntime::new(host.add_ref(), parameters.opt)?;
        Ok(Self::from_runtime(runtime, parameters))
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
