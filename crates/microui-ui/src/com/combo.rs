//! The full existing combo header remains one concrete COM widget; popup data stays with its composing owner.
use super::*;
use nc_com::{INcStringAbi, NcString};

callable_leaf_class!(ComboObject, crate::Combo, microui_contracts::Combo);

impl ComboObject {
    /// Constructs the class for the host-bound factory through the registry's typed contract.
    pub(super) fn create(host: ComPtr<IWidgetHostAbi>) -> Result<ComPtr<ComboAbi>, NcStatus> {
        let value = <Self as Combo>::new(host.as_com_ref())?;
        Ok(Self::from_value(value)
            .query()
            .expect("ComboObject declares the Combo class contract"))
    }
}

impl Combo for ComboObject {
    /// Initializes the established empty Combo state after validating host mutation authority.
    fn new(host: ComRef<'_, IWidgetHostAbi>) -> Result<Self, NcStatus> {
        let parameters = crate::ComboParameters::new();
        Ok(Self {
            widget: super::widget::WidgetCore::new(host.add_ref(), parameters.opt, crate::Combo::from(parameters))?,
        })
    }
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
    /// Validates a semantic font role before changing the Combo's measurement input.
    fn set_font_role(&self, role: TextFontRole) -> Result<(), NcStatus> {
        self.widget.set_font_role(role)
    }
    /// Resolves one exact named font before replacing the Combo's selection.
    fn set_named_font(&self, name: NcCopyFromBufferOffset<'_, u8>) -> Result<(), NcStatus> {
        self.widget.set_named_font(name)
    }
    /// Connects the Combo's captured-selection source directly through its class contract.
    fn subscribe_changed(&self, sink: ComRef<'_, IComboChangedSinkAbi>) -> Result<ComPtr<IEventSubscriptionAbi>, NcStatus> {
        self.widget.subscribe::<crate::ComboChanged>(sink)
    }
    /// Connects popup coordination independently from captured selection delivery.
    fn subscribe_popup(&self, sink: ComRef<'_, IComboPopupSinkAbi>) -> Result<ComPtr<IEventSubscriptionAbi>, NcStatus> {
        self.widget.subscribe::<crate::ComboSubmitted>(sink)
    }
}
impl ComboObject {
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
