//! Disclosure owns two real COM children; its header shares only explicit expansion data with layout.
use super::*;
use super::widget::WidgetCore;

leaf_class!(DisclosureHeader, crate::ui_node::DisclosureHeader, ITextContent, ITextContentAbi);
text_content!(DisclosureHeader, com_label, com_set_label);

#[com_class(interfaces(IDisclosure, IContainerLayout))]
/// One fixed disclosure identity retains its actual header and editable body capabilities.
pub(super) struct Disclosure {
    /// Existing indentation, header measurement and hidden-body policy over shared expansion state.
    widget: WidgetCore<crate::Disclosure>,
    /// Authoritative fixed header/body ownership; public interfaces never grant removal of these roles.
    editor: ComPtr<IChildrenEditorAbi>,
    /// Cached capability for the actual first child, without a duplicate label or common state.
    header: ComPtr<ITextContentAbi>,
    /// Cached capability for the actual second child, without a second child sequence.
    body: ComPtr<ILinearAbi>,
}
fixed_phases!(Disclosure);

impl Disclosure {
    /// Validates immutable presentation, then constructs the complete fixed composition before returning it.
    pub(super) fn create(
        host: ComPtr<IWidgetHostAbi>,
        label: &str,
        style: DisclosureStyle,
        expanded: bool,
    ) -> Result<ComPtr<IDisclosureAbi>, NcStatus> {
        host.check_mutation()?;
        let variant = match style {
            DisclosureStyle::HEADER => crate::ui_node::DisclosureVariant::Header,
            DisclosureStyle::TREE => crate::ui_node::DisclosureVariant::Tree,
            _ => return Err(NC_E_INVALID_ARG),
        };
        let model = crate::ui_node::DisclosureState::new(expanded);
        let header = DisclosureHeader::create(
            host.clone(),
            variant.header_options(),
            crate::ui_node::DisclosureHeader::com_new(model.clone(), label.to_owned(), variant),
        )?;
        let body = super::containers::Linear::create(
            host.clone(),
            crate::Linear::com_new(crate::LinearDirection::TopToBottom, crate::LinearCrossSize::Stretch),
        )?;
        let editor = host.create_children()?;
        editor.insert(0, header.query::<IWidgetAbi>()?.as_com_ref())?.finish()?;
        editor.insert(1, body.query::<IWidgetAbi>()?.as_com_ref())?.finish()?;
        Ok(Self::from_value(Self {
            widget: WidgetCore::new(host, crate::WidgetOption::NO_INTERACT, crate::Disclosure::com_new(model, variant))?,
            editor,
            header,
            body,
        })
        .query()
        .expect("disclosure declares IDisclosure"))
    }
}

impl IDisclosure for Disclosure {
    /// Observes the same expansion bit used by header input and container layout.
    fn expanded(&self) -> Result<bool, NcStatus> {
        self.widget.state.readable()?;
        Ok(self.widget.take()?.is_expanded())
    }
    /// Changes the shared bit through the concrete algorithm; its sticky marker survives skipped updates.
    fn set_expanded(&self, expanded: bool) -> Result<(), NcStatus> {
        self.widget.state.writable()?;
        let mut core = self.widget.take()?;
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
    fn body(&self) -> Result<ComPtr<ILinearAbi>, NcStatus> {
        self.widget.state.readable()?;
        Ok(self.body.clone())
    }
}
