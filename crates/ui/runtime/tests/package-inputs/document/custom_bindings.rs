// Generated optional widget fields from a validated UI document.
/// Checked widget fields expose their own getters and setters.
/// Resolve a new Form after replacement; old handles observe widget closure.
pub struct Form {
    /// Authored widget "root"; the concrete COM interface is checked when binding.
    pub widget_root_0: neocogi_ui::Widget<neocogi_ui::LinearAbi>,
    /// Authored widget "title"; the concrete COM interface is checked when binding.
    pub widget_title_1: neocogi_ui::Widget<neocogi_ui::TextBlockAbi>,
    /// Authored widget "save"; the concrete COM interface is checked when binding.
    pub widget_save_2: neocogi_ui::Widget<neocogi_ui::ButtonAbi>,
    /// Authored widget "extension"; the concrete COM interface is checked when binding.
    pub widget_extension_3: neocogi_ui::Widget<neocogi_ui_samples::abi::CounterAbi>,
}
/// Resolves concrete fields after construction has installed every receiver.
pub fn bind(
    component: &neocogi_ui_document::Component,
) -> Result<Form, neocogi_com::NcStatus> {
    let form = Form {
        widget_root_0: component.widget::<neocogi_ui::LinearAbi>("root")?,
        widget_title_1: component.widget::<neocogi_ui::TextBlockAbi>("title")?,
        widget_save_2: component.widget::<neocogi_ui::ButtonAbi>("save")?,
        widget_extension_3: component.widget::<neocogi_ui_samples::abi::CounterAbi>("extension")?,
    };
    Ok(form)
}
