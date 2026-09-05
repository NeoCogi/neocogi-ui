//! Concrete button editing and typed subscription over its one existing semantic event source.
use super::*;
use nc_com::{INcStringAbi, NcString};

leaf_class!(
    Button,
    crate::Button,
    IButton,
    IButtonAbi,
    IActivationEvents,
    IFontStyle,
    IIconContent,
    IButtonStyle,
    IImageContent
);

impl IButton for Button {
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
}

impl IActivationEvents for Button {
    /// Returns an application-owned lease; the button's queue never owns its sink.
    fn subscribe(&self, sink: ComRef<'_, IActivationSinkAbi>) -> Result<ComPtr<IEventSubscriptionAbi>, NcStatus> {
        self.widget.subscribe::<crate::ButtonSubmitted>(sink)
    }
}

font_style_impl!(Button);

icon_content_impl!(Button);
impl IButtonStyle for Button {
    /// Reports the current fill mask from the sole concrete button state.
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
}

super::widget::image_content_impl!(Button);
