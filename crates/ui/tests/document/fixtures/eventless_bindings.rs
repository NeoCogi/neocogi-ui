// Generated optional widget fields from a validated UI document.
/// Checked widget fields expose their own getters and setters.
/// Resolve a new Form after replacement; old handles observe widget closure.
pub struct Form {
    /// Authored widget "label"; the concrete COM interface is checked when binding.
    pub widget_label_0: neocogi_ui::Widget<neocogi_ui::TextBlockAbi>,
}
/// Resolves concrete fields after construction has installed every receiver.
pub fn bind(
    component: &neocogi_ui::document::Component,
) -> Result<Form, neocogi_com::NcStatus> {
    let form = Form {
        widget_label_0: component.widget::<neocogi_ui::TextBlockAbi>("label")?,
    };
    Ok(form)
}
