//! Constructible Button implementation over one inline semantic state.
use super::*;
use nc_com::{INcStringAbi, NcString};

#[com_object(microui_contracts::Button)]
/// Dynamically registered implementation of the public Button class contract.
pub struct ButtonObject {
    /// Inline semantic state and host-backed phase execution; no companion Button interface exists.
    widget: super::widget::WidgetCore<crate::Button>,
}

impl ButtonObject {
    /// Constructs the class directly for the factory shorthand without bypassing contract validation.
    pub(super) fn create(
        host: ComPtr<IWidgetHostAbi>,
        label: NcCopyFromBufferOffset<'_, u8>,
        options: WidgetOptions,
    ) -> Result<ComPtr<ButtonAbi>, NcStatus> {
        let value = <Self as Button>::new(host.as_com_ref(), label, options)?;
        Ok(Self::from_value(value)
            .query()
            .expect("ButtonObject declares the Button class contract"))
    }

    /// Applies the complete semantic algorithm to copied input.
    fn update_widget(
        &self,
        state: WidgetUpdate,
        event: WidgetEventAbi,
        text: NcCopyFromBufferOffset<'_, u8>,
    ) -> Result<GeometryChange, NcStatus> {
        self.widget.update(state, event, text)
    }

    /// Commits logical closure before final COM destruction.
    fn close_widget(&self) -> Result<(), NcStatus> {
        self.widget.close()
    }
}

widget_phase_impl!(ButtonObject);
leaf_layout_impl!(ButtonObject);

impl Button for ButtonObject {
    /// Validates complete construction input and retains only the supplied host capability.
    fn new(
        host: ComRef<'_, IWidgetHostAbi>,
        label: NcCopyFromBufferOffset<'_, u8>,
        options: WidgetOptions,
    ) -> Result<Self, NcStatus> {
        options.validate()?;
        let label = text(label)?;
        let native_options = crate::WidgetOption::from_bits_retain(options.bits);
        let core = crate::Button::from(crate::ButtonParameters::new(label));
        Ok(Self {
            widget: super::widget::WidgetCore::new(host.add_ref(), native_options, core)?,
        })
    }

    /// Validates before committing and only dirties geometry when text actually differs.
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

    /// Returns an owned immutable snapshot, never a borrow into widget storage.
    fn label(&self) -> Result<ComPtr<INcStringAbi>, NcStatus> {
        self.widget.state.readable()?;
        Ok(NcString::from_utf8_str(self.widget.take()?.com_label()))
    }

    /// Returns an application-owned lease over the Button's concrete activation source.
    fn subscribe(&self, sink: ComRef<'_, IActivationSinkAbi>) -> Result<ComPtr<IEventSubscriptionAbi>, NcStatus> {
        self.widget.subscribe::<crate::ButtonSubmitted>(sink)
    }

    /// Validates a semantic role before changing the Button's font selection.
    fn set_font_role(&self, role: TextFontRole) -> Result<(), NcStatus> {
        self.widget.set_font_role(role)
    }

    /// Resolves and retains a stable named font identity at the setter boundary.
    fn set_named_font(&self, name: NcCopyFromBufferOffset<'_, u8>) -> Result<(), NcStatus> {
        self.widget.set_named_font(name)
    }

    /// Validates a semantic icon before replacing the Button's adjacent visual.
    fn set_icon_role(&self, icon: ContentIcon) -> Result<(), NcStatus> {
        self.widget.set_icon_role(icon)
    }

    /// Resolves one exact named icon before replacing the Button's adjacent visual.
    fn set_named_icon(&self, name: NcCopyFromBufferOffset<'_, u8>) -> Result<(), NcStatus> {
        self.widget.set_named_icon(name)
    }

    /// Reports the current fill mask from the sole concrete Button state.
    fn fill_states(&self) -> Result<FillStates, NcStatus> {
        self.widget.state.readable()?;
        Ok(FillStates {
            bits: self.widget.take()?.com_fill().bits() as u8,
        })
    }

    /// Rejects unknown flags before committing a paint-only change.
    fn set_fill_states(&self, states: FillStates) -> Result<(), NcStatus> {
        self.widget.state.writable()?;
        let fill = crate::WidgetFillOption::from_bits(u32::from(states.bits)).ok_or(NC_E_INVALID_ARG)?;
        self.widget.take()?.com_set_fill(fill);
        Ok(())
    }

    /// Validates and retains the renderer-owned image before changing semantic content.
    fn set_image(&self, image: ComRef<'_, ITextureAbi>, fit_width: bool) -> Result<(), NcStatus> {
        self.widget.state.writable()?;
        let mut core = self.widget.take()?;
        let image = microui_redux::Image::from_interface(image.add_ref())?;
        // A hostile dimension provider may reenter lifetime authority while
        // semantic state is detached; revalidate the host before committing.
        self.widget.state.writable()?;
        if super::widget::ImageState::set_image(&mut *core, Some(image), fit_width) {
            self.widget.state.record_change(GeometryChange::CHANGED);
        }
        Ok(())
    }

    /// Clears owned image content while retaining the same label and event source.
    fn clear_image(&self) -> Result<(), NcStatus> {
        self.widget.state.writable()?;
        if super::widget::ImageState::set_image(&mut *self.widget.take()?, None, false) {
            self.widget.state.record_change(GeometryChange::CHANGED);
        }
        Ok(())
    }
}
