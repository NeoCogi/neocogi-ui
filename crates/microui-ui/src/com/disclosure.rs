//! Disclosure owns two real COM children; its header shares only explicit expansion data with layout.
use super::*;
use super::widget::{HostedWidget, WidgetRuntime};
use super::widget::WidgetCore;

direct_widget_phase_impl!(crate::ui_node::DisclosureHeader);
direct_leaf_layout_impl!(crate::ui_node::DisclosureHeader);

impl crate::ui_node::DisclosureHeader {
    /// Creates the internal header capability without introducing a wrapper allocation.
    fn create_com(
        host: ComPtr<IWidgetHostAbi>,
        options: crate::WidgetOption,
        model: Rc<crate::ui_node::DisclosureState>,
        label: String,
        variant: crate::ui_node::DisclosureVariant,
    ) -> Result<ComPtr<ITextContentAbi>, NcStatus> {
        let runtime = WidgetRuntime::new(host, options)?;
        Ok(Self::from_value(Self::from_runtime(runtime, model, label, variant))
            .query()
            .expect("DisclosureHeader declares its text and leaf capabilities"))
    }
}

impl ITextContent for crate::ui_node::DisclosureHeader {
    /// Copies the label used by this same header's measurement and painting.
    fn text(&self) -> Result<ComPtr<nc_com::INcStringAbi>, NcStatus> {
        self.runtime().state.readable()?;
        Ok(nc_com::NcString::from_utf8_str(self.com_label()))
    }

    /// Replaces the concrete header label and records intrinsic geometry invalidation.
    fn set_text(&mut self, input: NcCopyFromBufferOffset<'_, u8>) -> Result<(), NcStatus> {
        self.runtime().state.writable()?;
        let label = text(input)?;
        if self.com_label() != label {
            self.com_set_label(label);
            self.runtime_mut().state.record_change(GeometryChange::CHANGED);
        }
        Ok(())
    }
}

#[com_object(microui_contracts::Disclosure)]
/// One fixed disclosure identity retains its actual header and editable body capabilities.
pub struct DisclosureObject {
    /// Existing indentation, header measurement and hidden-body policy over shared expansion state.
    widget: WidgetCore<crate::Disclosure>,
    /// Authoritative fixed header/body ownership; public interfaces never grant removal of these roles.
    editor: ComPtr<IChildrenEditorAbi>,
    /// Cached capability for the actual first child, without a duplicate label or common state.
    header: ComPtr<ITextContentAbi>,
    /// Cached capability for the actual second child, without a second child sequence.
    body: ComPtr<LinearAbi>,
}
fixed_phases!(DisclosureObject);

impl DisclosureObject {
    /// Constructs the class for the host-bound factory through the registry's typed contract.
    pub(super) fn create(
        host: ComPtr<IWidgetHostAbi>,
        label: NcCopyFromBufferOffset<'_, u8>,
        style: DisclosureStyle,
        expanded: bool,
    ) -> Result<ComPtr<DisclosureAbi>, NcStatus> {
        let value = <Self as Disclosure>::new(host.as_com_ref(), label, style, expanded)?;
        Ok(Self::from_value(value)
            .query()
            .expect("DisclosureObject declares the Disclosure class contract"))
    }
}

impl Disclosure for DisclosureObject {
    /// Validates presentation and label before constructing the complete fixed composition.
    fn new(host: ComRef<'_, IWidgetHostAbi>, label: NcCopyFromBufferOffset<'_, u8>, style: DisclosureStyle, expanded: bool) -> Result<Self, NcStatus> {
        let label = text(label)?;
        host.check_mutation()?;
        let variant = match style {
            DisclosureStyle::HEADER => crate::ui_node::DisclosureVariant::Header,
            DisclosureStyle::TREE => crate::ui_node::DisclosureVariant::Tree,
            _ => return Err(NC_E_INVALID_ARG),
        };
        let model = crate::ui_node::DisclosureState::new(expanded);
        let header = crate::ui_node::DisclosureHeader::create_com(
            host.add_ref(),
            variant.header_options(),
            model.clone(),
            label.to_owned(),
            variant,
        )?;
        let body = super::Linear::create_com(host.add_ref(), FlowDirection::TOP_TO_BOTTOM, CrossSize::Stretch.into())?;
        let editor = host.create_children()?;
        editor.insert(0, header.query::<IWidgetAbi>()?.as_com_ref())?.finish()?;
        editor.insert(1, body.query::<IWidgetAbi>()?.as_com_ref())?.finish()?;
        Ok(Self {
            widget: WidgetCore::new(host.add_ref(), crate::WidgetOption::NO_INTERACT, crate::Disclosure::com_new(model, variant))?,
            editor,
            header,
            body,
        })
    }
    /// Observes the same expansion bit used by header input and container layout.
    fn expanded(&self) -> Result<bool, NcStatus> {
        self.widget.state.readable()?;
        Ok(self.widget.core.is_expanded())
    }
    /// Changes the shared bit through the concrete algorithm; its sticky marker survives skipped updates.
    fn set_expanded(&mut self, expanded: bool) -> Result<(), NcStatus> {
        self.widget.state.writable()?;
        let core = &mut self.widget.core;
        if expanded {
            core.expand();
        } else {
            core.collapse();
        }
        Ok(())
    }
    /// Returns the actual header whose inherited widget options control its presentation and input.
    fn header(&self) -> Result<ComPtr<ITextContentAbi>, NcStatus> {
        self.widget.state.readable()?;
        Ok(self.header.clone())
    }
    /// Returns the actual body whose typed edits preserve child/metadata cardinality.
    fn body(&self) -> Result<ComPtr<LinearAbi>, NcStatus> {
        self.widget.state.readable()?;
        Ok(self.body.clone())
    }
}
