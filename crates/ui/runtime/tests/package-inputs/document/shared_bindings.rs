// Generated optional widget fields from a validated UI document.
/// Checked widget fields expose their own getters and setters.
/// Resolve a new Form after replacement; old handles observe widget closure.
pub struct Form {
    /// Authored widget "root"; the concrete COM interface is checked when binding.
    pub widget_root_0: neocogi_ui::Widget<neocogi_ui::LinearAbi>,
    /// Authored widget "first"; the concrete COM interface is checked when binding.
    pub widget_first_1: neocogi_ui::Widget<neocogi_ui::ButtonAbi>,
    /// Authored widget "second"; the concrete COM interface is checked when binding.
    pub widget_second_2: neocogi_ui::Widget<neocogi_ui::ButtonAbi>,
    /// Authored widget "text"; the concrete COM interface is checked when binding.
    pub widget_text_3: neocogi_ui::Widget<neocogi_ui::TextboxAbi>,
}
/// Resolves concrete fields after construction has installed every receiver.
pub fn bind(
    component: &neocogi_ui_document::Component,
) -> Result<Form, neocogi_com::NcStatus> {
    let form = Form {
        widget_root_0: component.widget::<neocogi_ui::LinearAbi>("root")?,
        widget_first_1: component.widget::<neocogi_ui::ButtonAbi>("first")?,
        widget_second_2: component.widget::<neocogi_ui::ButtonAbi>("second")?,
        widget_text_3: component.widget::<neocogi_ui::TextboxAbi>("text")?,
    };
    Ok(form)
}
