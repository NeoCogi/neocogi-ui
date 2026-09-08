//! Disclosure owns two real COM children; its header shares only explicit expansion data with layout.
use super::*;
use super::widget::WidgetCore;

leaf_class!(DisclosureHeader, crate::ui_node::DisclosureHeader, ITextContent, ITextContentAbi);
text_content!(DisclosureHeader, com_label, com_set_label);

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
        let header = DisclosureHeader::create(
            host.add_ref(),
            variant.header_options(),
            crate::ui_node::DisclosureHeader::com_new(model.clone(), label.to_owned(), variant),
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
