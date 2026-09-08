//! Disclosure owns two real COM children; its header shares only explicit expansion data with layout.
use super::*;
use super::widget::{HostedWidget, WidgetRuntime};

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

direct_widget_phase_impl!(crate::Disclosure);
direct_container_layout_impl!(crate::Disclosure);

impl crate::Disclosure {
    /// Constructs the class for the host-bound factory through the registry's typed contract.
    pub(super) fn create_com(
        host: ComPtr<IWidgetHostAbi>,
        label: NcCopyFromBufferOffset<'_, u8>,
        style: DisclosureStyle,
        expanded: bool,
    ) -> Result<ComPtr<DisclosureAbi>, NcStatus> {
        let value = <Self as microui_contracts::Disclosure>::new(host.as_com_ref(), label, style, expanded)?;
        Ok(Self::from_value(value)
            .query()
            .expect("Disclosure declares its callable class contract"))
    }
}

impl microui_contracts::Disclosure for crate::Disclosure {
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
        let runtime = WidgetRuntime::new(host.add_ref(), crate::WidgetOption::NO_INTERACT)?;
        let children = prepare_children(host.add_ref(), model.clone(), label.to_owned(), variant)?;
        Ok(Self::from_runtime(
            runtime,
            children.editor,
            children.header,
            children.body,
            model,
            variant,
        ))
    }
    /// Observes the same expansion bit used by header input and container layout.
    fn expanded(&self) -> Result<bool, NcStatus> {
        self.runtime().state.readable()?;
        Ok(self.is_expanded())
    }
    /// Changes the shared bit through the concrete algorithm; its sticky marker survives skipped updates.
    fn set_expanded(&mut self, expanded: bool) -> Result<(), NcStatus> {
        self.runtime().state.writable()?;
        if expanded {
            self.expand();
        } else {
            self.collapse();
        }
        Ok(())
    }
    /// Returns the actual header whose inherited widget options control its presentation and input.
    fn header(&self) -> Result<ComPtr<ITextContentAbi>, NcStatus> {
        self.runtime().state.readable()?;
        Ok(self.header.clone())
    }
    /// Returns the actual body whose typed edits preserve child/metadata cardinality.
    fn body(&self) -> Result<ComPtr<LinearAbi>, NcStatus> {
        self.runtime().state.readable()?;
        Ok(self.body.clone())
    }
}

/// Completed fixed disclosure topology consumed by either construction path.
pub(crate) struct DisclosureChildren {
    /// Writer owning the header and body in their permanent traversal order.
    pub(crate) editor: ComPtr<IChildrenEditorAbi>,
    /// Text capability of the actual header child returned by the public class.
    pub(crate) header: ComPtr<ITextContentAbi>,
    /// Concrete Linear capability of the actual body child returned by the public class.
    pub(crate) body: ComPtr<LinearAbi>,
}

/// Builds the two real structural children and their fixed authoritative sequence.
pub(crate) fn prepare_children(
    host: ComPtr<IWidgetHostAbi>,
    model: Rc<crate::ui_node::DisclosureState>,
    label: String,
    variant: crate::ui_node::DisclosureVariant,
) -> Result<DisclosureChildren, NcStatus> {
    // Construct both children before publishing the sequence, so callers never observe a partial
    // disclosure. The shared scalar model contains no parent or child object backpointer.
    let header = crate::ui_node::DisclosureHeader::create_com(
        host.clone(),
        variant.header_options(),
        model,
        label,
        variant,
    )?;
    let body = super::Linear::create_com(host.clone(), FlowDirection::TOP_TO_BOTTOM, CrossSize::Stretch.into())?;
    let editor = host.create_children()?;
    editor.insert(0, header.query::<IWidgetAbi>()?.as_com_ref())?.finish()?;
    editor.insert(1, body.query::<IWidgetAbi>()?.as_com_ref())?.finish()?;
    Ok(DisclosureChildren { editor, header, body })
}
