//! The full existing combo header remains one concrete COM widget; popup data stays with its composing owner.
use super::*;
use super::widget::{HostedWidget, set_direct_font_role, set_direct_named_font};
use nc_com::{INcStringAbi, NcString};

direct_widget_phase_impl!(crate::Combo);
direct_leaf_layout_impl!(crate::Combo);

impl crate::Combo {
    /// Constructs the class for the host-bound factory through the registry's typed contract.
    pub(super) fn create_com(host: ComPtr<IWidgetHostAbi>, resources: super::widget::SharedAssets) -> Result<ComPtr<ComboAbi>, NcStatus> {
        let value = Self::build(host, Some(&resources))?;
        Ok(Self::from_value(value).query().expect("Combo declares its callable class contract"))
    }

    /// Builds one combo header with either directly imported or runtime-shared resources.
    fn build(host: ComPtr<IWidgetHostAbi>, resources: Option<&super::widget::SharedAssets>) -> Result<Self, NcStatus> {
        let parameters = crate::ComboParameters::new();
        let runtime = super::widget::WidgetRuntime::for_resources(host, parameters.opt, resources)?;
        Ok(Self::from_runtime(runtime, parameters))
    }
}

impl microui_contracts::Combo for crate::Combo {
    /// Initializes the established empty Combo state after validating host mutation authority.
    fn new(host: ComRef<'_, IWidgetHostAbi>) -> Result<Self, NcStatus> {
        Self::build(host.add_ref(), None)
    }
    /// Reads the same position used by the semantic selection algorithm.
    fn selected(&self) -> Result<u64, NcStatus> {
        self.runtime().state.readable()?;
        Ok(self.selected() as u64)
    }
    /// Copies the current header label without exposing the plugin's string allocation.
    fn label(&self) -> Result<ComPtr<INcStringAbi>, NcStatus> {
        self.runtime().state.readable()?;
        Ok(NcString::from_utf8_str(self.label()))
    }
    /// Reports semantic popup intent independently of the application's actual popup capability.
    fn is_open(&self) -> Result<bool, NcStatus> {
        self.runtime().state.readable()?;
        Ok(self.is_open())
    }
    /// Ordinary popup dismissal can synchronize intent without fabricating another request.
    fn set_open(&mut self, open: bool) -> Result<(), NcStatus> {
        self.runtime().state.writable()?;
        if open {
            self.open_popup();
        } else {
            self.close_popup();
        }
        Ok(())
    }
    /// Programmatic list synchronization remains silent and validates before changing either field.
    fn set_selection(&mut self, index: u64, label: NcCopyFromBufferOffset<'_, u8>) -> Result<(), NcStatus> {
        self.selection(index, label, false)
    }
    /// An actual popup action explicitly publishes the captured user selection after committing it.
    fn submit_selection(&mut self, index: u64, label: NcCopyFromBufferOffset<'_, u8>) -> Result<(), NcStatus> {
        self.selection(index, label, true)
    }
    /// Validates a semantic font role before changing the Combo's measurement input.
    fn set_font_role(&mut self, role: TextFontRole) -> Result<(), NcStatus> {
        set_direct_font_role(self, role)
    }
    /// Resolves one exact named font before replacing the Combo's selection.
    fn set_named_font(&mut self, name: NcCopyFromBufferOffset<'_, u8>) -> Result<(), NcStatus> {
        set_direct_named_font(self, name)
    }
    /// Connects the Combo's captured-selection source directly through its class contract.
    fn subscribe_changed(&mut self, sink: ComRef<'_, IComboChangedSinkAbi>) -> Result<ComPtr<IEventSubscriptionAbi>, NcStatus> {
        self.runtime().state.writable()?;
        let endpoint = <Self as crate::TypedWidget<crate::ComboChanged>>::event(self);
        super::events::subscribe(endpoint, sink)
    }
    /// Connects popup coordination independently from captured selection delivery.
    fn subscribe_popup(&mut self, sink: ComRef<'_, IComboPopupSinkAbi>) -> Result<ComPtr<IEventSubscriptionAbi>, NcStatus> {
        self.runtime().state.writable()?;
        let endpoint = <Self as crate::TypedWidget<crate::ComboSubmitted>>::event(self);
        super::events::subscribe(endpoint, sink)
    }
}
impl crate::Combo {
    /// One replacement path validates the complete snapshot and preserves sticky geometry invalidation.
    fn selection(&mut self, index: u64, label: NcCopyFromBufferOffset<'_, u8>, submit: bool) -> Result<(), NcStatus> {
        self.runtime().state.writable()?;
        let index = usize::try_from(index).map_err(|_| NC_E_INVALID_ARG)?;
        let label = text(label)?;
        let changed_label = self.label() != label;
        self.com_set_selection(index, label, submit);
        self.runtime_mut()
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
