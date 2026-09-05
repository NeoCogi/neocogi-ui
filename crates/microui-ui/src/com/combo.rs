//! The full existing combo header remains one concrete COM widget; popup data stays with its composing owner.
use super::*;
use nc_com::{INcStringAbi, NcString};

leaf_class!(Combo, crate::Combo, ICombo, IComboAbi, IFontStyle, IComboEvents);
impl ICombo for Combo {
    /// Reads the same position used by the semantic selection algorithm.
    fn selected(&self) -> Result<u64, NcStatus> {
        self.widget.state.readable()?;
        Ok(self.widget.take()?.selected() as u64)
    }
    /// Copies the current header label without exposing the plugin's string allocation.
    fn label(&self) -> Result<ComPtr<INcStringAbi>, NcStatus> {
        self.widget.state.readable()?;
        Ok(NcString::from_utf8_str(self.widget.take()?.label()))
    }
    /// Reports semantic popup intent independently of the application's actual popup capability.
    fn is_open(&self) -> Result<bool, NcStatus> {
        self.widget.state.readable()?;
        Ok(self.widget.take()?.is_open())
    }
    /// Ordinary popup dismissal can synchronize intent without fabricating another request.
    fn set_open(&self, open: bool) -> Result<(), NcStatus> {
        self.widget.state.writable()?;
        let mut core = self.widget.take()?;
        if open {
            core.open_popup();
        } else {
            core.close_popup();
        }
        Ok(())
    }
    /// Programmatic list synchronization remains silent and validates before changing either field.
    fn set_selection(&self, index: u64, label: NcCopyFromBufferOffset<'_, u8>) -> Result<(), NcStatus> {
        self.selection(index, label, false)
    }
    /// An actual popup action explicitly publishes the captured user selection after committing it.
    fn submit_selection(&self, index: u64, label: NcCopyFromBufferOffset<'_, u8>) -> Result<(), NcStatus> {
        self.selection(index, label, true)
    }
}
impl Combo {
    /// One replacement path validates the complete snapshot and preserves sticky geometry invalidation.
    fn selection(&self, index: u64, label: NcCopyFromBufferOffset<'_, u8>, submit: bool) -> Result<(), NcStatus> {
        self.widget.state.writable()?;
        let index = usize::try_from(index).map_err(|_| NC_E_INVALID_ARG)?;
        let label = text(label)?;
        let mut core = self.widget.take()?;
        let changed_label = core.label() != label;
        core.com_set_selection(index, label, submit);
        self.widget
            .state
            .record_change(if changed_label { GeometryChange::CHANGED } else { GeometryChange::NO_CHANGE });
        Ok(())
    }
}
impl IComboEvents for Combo {
    /// Both independent ports use the same application-owned transport as other built-in controls.
    fn subscribe_changed(&self, sink: ComRef<'_, IComboChangedSinkAbi>) -> Result<ComPtr<IEventSubscriptionAbi>, NcStatus> {
        self.widget.subscribe::<crate::ComboChanged>(sink)
    }
    /// Captured anchors are delivered after runtime traversal releases the context for popup edits.
    fn subscribe_popup(&self, sink: ComRef<'_, IComboPopupSinkAbi>) -> Result<ComPtr<IEventSubscriptionAbi>, NcStatus> {
        self.widget.subscribe::<crate::ComboSubmitted>(sink)
    }
}
impl super::events::ComEvent for crate::ComboChanged {
    type Sink = IComboChangedSinkAbi;
    /// The label belongs to this event rather than the possibly changed header.
    fn deliver(&self, sink: ComRef<'_, Self::Sink>) -> Result<(), NcStatus> {
        let label = NcString::from_utf8_str(&self.label);
        sink.changed(self.selected as u64, label.as_com_ref())
    }
}
impl super::events::ComEvent for crate::ComboSubmitted {
    type Sink = IComboPopupSinkAbi;
    /// Converts the existing routed anchor once at the ABI boundary.
    fn deliver(&self, sink: ComRef<'_, Self::Sink>) -> Result<(), NcStatus> {
        sink.submitted(self.open, copied_rect(self.anchor))
    }
}

font_style_impl!(Combo);
