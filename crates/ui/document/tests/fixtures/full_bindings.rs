// Generated from a validated UI document. Keep application handlers in another module.
/// Captured public events; private visual connections remain inside the loaded component.
pub enum Event {
    /// Public output "save".
    Output0(neocogi_ui::events::Activation),
    /// Public output "type".
    Output1(neocogi_ui::events::CheckboxChanged),
    /// Public output "quote\"\n// x".
    Output2(neocogi_ui::events::TextChanged),
    /// Public output "text submitted".
    Output3(neocogi_ui::events::TextSubmitted),
    /// Public output "value".
    Output4(neocogi_ui::events::NumericChanged),
    /// Public output "pixel-offset".
    Output5(neocogi_ui::events::ScrollbarChanged),
    /// Public output "pixel_offset".
    Output6(neocogi_ui::events::SplitChanged),
    /// Public output "selection".
    Output7(neocogi_ui::events::ChoiceSelection),
}
/// A captured event belongs to exactly one component reload attempt.
pub struct Message {
    /// Reject stale messages with ReloadSlot::accepts before dispatch.
    pub generation: neocogi_ui_document::Generation,
    /// Concrete captured event payload.
    pub event: Event,
}
/// Implement every public handler in application-owned code; no callback bodies are overwritten.
pub trait Handlers {
    /// Handles public output "save" at the application's safe update boundary.
    fn on_save_0(&mut self, value: neocogi_ui::events::Activation) -> Result<(), neocogi_com::NcStatus>;
    /// Handles public output "type" at the application's safe update boundary.
    fn on_type_1(&mut self, value: neocogi_ui::events::CheckboxChanged) -> Result<(), neocogi_com::NcStatus>;
    /// Handles public output "quote\"\n// x" at the application's safe update boundary.
    fn on_quote_x_2(&mut self, value: neocogi_ui::events::TextChanged) -> Result<(), neocogi_com::NcStatus>;
    /// Handles public output "text submitted" at the application's safe update boundary.
    fn on_text_submitted_3(&mut self, value: neocogi_ui::events::TextSubmitted) -> Result<(), neocogi_com::NcStatus>;
    /// Handles public output "value" at the application's safe update boundary.
    fn on_value_4(&mut self, value: neocogi_ui::events::NumericChanged) -> Result<(), neocogi_com::NcStatus>;
    /// Handles public output "pixel-offset" at the application's safe update boundary.
    fn on_pixel_offset_5(&mut self, value: neocogi_ui::events::ScrollbarChanged) -> Result<(), neocogi_com::NcStatus>;
    /// Handles public output "pixel_offset" at the application's safe update boundary.
    fn on_pixel_offset_6(&mut self, value: neocogi_ui::events::SplitChanged) -> Result<(), neocogi_com::NcStatus>;
    /// Handles public output "selection" at the application's safe update boundary.
    fn on_selection_7(&mut self, value: neocogi_ui::events::ChoiceSelection) -> Result<(), neocogi_com::NcStatus>;
}
impl Event {
    /// Dispatch only after checking the enclosing message's generation.
    pub fn dispatch(self, _handlers: &mut impl Handlers) -> Result<(), neocogi_com::NcStatus> {
        match self {
            Self::Output0(value) => _handlers.on_save_0(value),
            Self::Output1(value) => _handlers.on_type_1(value),
            Self::Output2(value) => _handlers.on_quote_x_2(value),
            Self::Output3(value) => _handlers.on_text_submitted_3(value),
            Self::Output4(value) => _handlers.on_value_4(value),
            Self::Output5(value) => _handlers.on_pixel_offset_5(value),
            Self::Output6(value) => _handlers.on_pixel_offset_6(value),
            Self::Output7(value) => _handlers.on_selection_7(value),
        }
    }
}
/// Reconnect these concrete input handles after each successful component replacement.
pub struct Inputs {
    /// Public input "Text"; send its exact command at an ordinary update boundary.
    pub input_text_0: neocogi_ui_document::InputHandle<neocogi_ui_document::contracts::SetText>,
    /// Public input "Checked"; send its exact command at an ordinary update boundary.
    pub input_checked_1: neocogi_ui_document::InputHandle<neocogi_ui_document::contracts::SetChecked>,
    /// Public input "Value"; send its exact command at an ordinary update boundary.
    pub input_value_2: neocogi_ui_document::InputHandle<neocogi_ui_document::contracts::SetValue>,
    /// Public input "Enabled"; send its exact command at an ordinary update boundary.
    pub input_enabled_3: neocogi_ui_document::InputHandle<neocogi_ui_document::contracts::SetEnabled>,
    /// Public input "Expanded"; send its exact command at an ordinary update boundary.
    pub input_expanded_4: neocogi_ui_document::InputHandle<neocogi_ui_document::contracts::SetExpanded>,
    /// Public input "Choices"; send its exact command at an ordinary update boundary.
    pub input_choices_5: neocogi_ui_document::InputHandle<neocogi_ui_document::contracts::SetChoices>,
    /// Public input "Selection"; send its exact command at an ordinary update boundary.
    pub input_selection_6: neocogi_ui_document::InputHandle<neocogi_ui_document::contracts::SetSelection>,
    /// Public input "Offset"; send its exact command at an ordinary update boundary.
    pub input_offset_7: neocogi_ui_document::InputHandle<neocogi_ui_document::contracts::SetOffset>,
    /// Public input "Position"; send its exact command at an ordinary update boundary.
    pub input_position_8: neocogi_ui_document::InputHandle<neocogi_ui_document::contracts::SetPosition>,
}
/// Resolves inputs and queues public outputs during candidate preparation.
/// Use inside ReloadSlot::reload; failed preparation leaves the previous component published.
pub fn bind(
    _component: &neocogi_ui_document::Component,
    _generation: neocogi_ui_document::Generation,
    _send: std::sync::mpsc::Sender<Message>,
) -> Result<Inputs, neocogi_com::NcStatus> {
    let inputs = Inputs {
        input_text_0: _component.input::<neocogi_ui_document::contracts::SetText>("Text")?,
        input_checked_1: _component.input::<neocogi_ui_document::contracts::SetChecked>("Checked")?,
        input_value_2: _component.input::<neocogi_ui_document::contracts::SetValue>("Value")?,
        input_enabled_3: _component.input::<neocogi_ui_document::contracts::SetEnabled>("Enabled")?,
        input_expanded_4: _component.input::<neocogi_ui_document::contracts::SetExpanded>("Expanded")?,
        input_choices_5: _component.input::<neocogi_ui_document::contracts::SetChoices>("Choices")?,
        input_selection_6: _component.input::<neocogi_ui_document::contracts::SetSelection>("Selection")?,
        input_offset_7: _component.input::<neocogi_ui_document::contracts::SetOffset>("Offset")?,
        input_position_8: _component.input::<neocogi_ui_document::contracts::SetPosition>("Position")?,
    };
    let outputs = _component.outputs()?;
    neocogi_com::events::bind_output(outputs.as_com_ref(), "save", _send.clone(), move |value: neocogi_ui::events::Activation| Message { generation: _generation, event: Event::Output0(value) })?;
    neocogi_com::events::bind_output(outputs.as_com_ref(), "type", _send.clone(), move |value: neocogi_ui::events::CheckboxChanged| Message { generation: _generation, event: Event::Output1(value) })?;
    neocogi_com::events::bind_output(outputs.as_com_ref(), "quote\"\n// x", _send.clone(), move |value: neocogi_ui::events::TextChanged| Message { generation: _generation, event: Event::Output2(value) })?;
    neocogi_com::events::bind_output(outputs.as_com_ref(), "text submitted", _send.clone(), move |value: neocogi_ui::events::TextSubmitted| Message { generation: _generation, event: Event::Output3(value) })?;
    neocogi_com::events::bind_output(outputs.as_com_ref(), "value", _send.clone(), move |value: neocogi_ui::events::NumericChanged| Message { generation: _generation, event: Event::Output4(value) })?;
    neocogi_com::events::bind_output(outputs.as_com_ref(), "pixel-offset", _send.clone(), move |value: neocogi_ui::events::ScrollbarChanged| Message { generation: _generation, event: Event::Output5(value) })?;
    neocogi_com::events::bind_output(outputs.as_com_ref(), "pixel_offset", _send.clone(), move |value: neocogi_ui::events::SplitChanged| Message { generation: _generation, event: Event::Output6(value) })?;
    neocogi_com::events::bind_output(outputs.as_com_ref(), "selection", _send.clone(), move |value: neocogi_ui::events::ChoiceSelection| Message { generation: _generation, event: Event::Output7(value) })?;
    Ok(inputs)
}
