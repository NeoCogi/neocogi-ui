//! COM dispatch for the concrete [`crate::Button`] widget.
//!
//! This module contains boundary translation only. The registered COM allocation and all semantic
//! state are the single `Button` type declared in `ui_node/widgets/button.rs`.
use super::*;
use super::widget::{
    HostedWidget, ImageState, set_direct_font_role, set_direct_icon_role, set_direct_named_font,
    set_direct_named_icon,
};
use nc_com::{INcStringAbi, NcString};

impl crate::Button {
    /// Constructs the class directly for the factory shorthand without bypassing contract validation.
    pub(super) fn create_com(host: ComPtr<IWidgetHostAbi>, label: NcCopyFromBufferOffset<'_, u8>, options: WidgetOptions) -> Result<ComPtr<ButtonAbi>, NcStatus> {
        let value = <Self as microui_contracts::Button>::new(host.as_com_ref(), label, options)?;
        Ok(Self::from_value(value).query().expect("Button declares its callable class contract"))
    }
}

direct_widget_phase_impl!(crate::Button);
direct_leaf_layout_impl!(crate::Button);

impl microui_contracts::Button for crate::Button {
    /// Validates complete construction input and retains only the supplied host capability.
    fn new(host: ComRef<'_, IWidgetHostAbi>, label: NcCopyFromBufferOffset<'_, u8>, options: WidgetOptions) -> Result<Self, NcStatus> {
        options.validate()?;
        let label = text(label)?;
        let native_options = crate::WidgetOption::from_bits_retain(options.bits);
        let runtime = super::widget::WidgetRuntime::new(host.add_ref(), native_options)?;
        Ok(Self::from_runtime(runtime, crate::ButtonParameters::new(label)))
    }

    /// Validates before committing and only dirties geometry when text actually differs.
    fn set_label(&mut self, input: NcCopyFromBufferOffset<'_, u8>) -> Result<(), NcStatus> {
        self.runtime().state.writable()?;
        let input = text(input)?;
        if self.com_label() != input {
            self.com_set_label(input);
            self.runtime_mut().state.record_change(GeometryChange::CHANGED);
        }
        Ok(())
    }

    /// Returns an owned immutable snapshot, never a borrow into widget storage.
    fn label(&self) -> Result<ComPtr<INcStringAbi>, NcStatus> {
        self.runtime().state.readable()?;
        Ok(NcString::from_utf8_str(self.com_label()))
    }

    /// Returns an application-owned lease over the Button's concrete activation source.
    fn subscribe(&mut self, sink: ComRef<'_, IActivationSinkAbi>) -> Result<ComPtr<IEventSubscriptionAbi>, NcStatus> {
        self.runtime().state.writable()?;
        let endpoint = <Self as crate::TypedWidget<crate::ButtonSubmitted>>::event(self);
        super::events::subscribe(endpoint, sink)
    }

    /// Validates a semantic role before changing the Button's font selection.
    fn set_font_role(&mut self, role: TextFontRole) -> Result<(), NcStatus> {
        set_direct_font_role(self, role)
    }

    /// Resolves and retains a stable named font identity at the setter boundary.
    fn set_named_font(&mut self, name: NcCopyFromBufferOffset<'_, u8>) -> Result<(), NcStatus> {
        set_direct_named_font(self, name)
    }

    /// Validates a semantic icon before replacing the Button's adjacent visual.
    fn set_icon_role(&mut self, icon: ContentIcon) -> Result<(), NcStatus> {
        set_direct_icon_role(self, icon)
    }

    /// Resolves one exact named icon before replacing the Button's adjacent visual.
    fn set_named_icon(&mut self, name: NcCopyFromBufferOffset<'_, u8>) -> Result<(), NcStatus> {
        set_direct_named_icon(self, name)
    }

    /// Reports the current fill mask from the sole concrete Button state.
    fn fill_states(&self) -> Result<FillStates, NcStatus> {
        self.runtime().state.readable()?;
        Ok(FillStates {
            bits: self.com_fill().bits() as u8,
        })
    }

    /// Rejects unknown flags before committing a paint-only change.
    fn set_fill_states(&mut self, states: FillStates) -> Result<(), NcStatus> {
        self.runtime().state.writable()?;
        let fill = crate::WidgetFillOption::from_bits(u32::from(states.bits)).ok_or(NC_E_INVALID_ARG)?;
        self.com_set_fill(fill);
        Ok(())
    }

    /// Validates and retains the renderer-owned image before changing semantic content.
    fn set_image(&mut self, image: ComRef<'_, ITextureAbi>, fit_width: bool) -> Result<(), NcStatus> {
        self.runtime().state.writable()?;
        let image = microui_redux::Image::from_interface(image.add_ref())?;
        // A hostile dimension provider may reenter through another interface while this mutable
        // call is active; revalidate host authority before committing the prepared image.
        self.runtime().state.writable()?;
        if ImageState::set_image(self, Some(image), fit_width) {
            self.runtime_mut().state.record_change(GeometryChange::CHANGED);
        }
        Ok(())
    }

    /// Clears owned image content while retaining the same label and event source.
    fn clear_image(&mut self) -> Result<(), NcStatus> {
        self.runtime().state.writable()?;
        if ImageState::set_image(self, None, false) {
            self.runtime_mut().state.record_change(GeometryChange::CHANGED);
        }
        Ok(())
    }
}
