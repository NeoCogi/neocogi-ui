// Generated from a validated UI document. Keep application handlers in another module.
/// Captured public events; private visual connections remain inside the loaded component.
pub enum Event {
    /// Public output "save".
    Output0(neocogi_ui::events::Activation),
    /// Public output "type".
    Output1(neocogi_ui::events::CheckboxChanged),
    /// Public output "quote\"\n// x".
    Output2(neocogi_ui::textbox::Event),
    /// Public output "value".
    Output3(neocogi_ui::events::NumericChanged),
    /// Public output "pixel-offset".
    Output4(neocogi_ui::events::ScrollbarChanged),
    /// Public output "pixel_offset".
    Output5(neocogi_ui::events::SplitChanged),
    /// Public output "selection".
    Output6(neocogi_ui::events::ChoiceSelection),
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
    fn on_quote_x_2(&mut self, value: neocogi_ui::textbox::Event) -> Result<(), neocogi_com::NcStatus>;
    /// Handles public output "value" at the application's safe update boundary.
    fn on_value_3(&mut self, value: neocogi_ui::events::NumericChanged) -> Result<(), neocogi_com::NcStatus>;
    /// Handles public output "pixel-offset" at the application's safe update boundary.
    fn on_pixel_offset_4(&mut self, value: neocogi_ui::events::ScrollbarChanged) -> Result<(), neocogi_com::NcStatus>;
    /// Handles public output "pixel_offset" at the application's safe update boundary.
    fn on_pixel_offset_5(&mut self, value: neocogi_ui::events::SplitChanged) -> Result<(), neocogi_com::NcStatus>;
    /// Handles public output "selection" at the application's safe update boundary.
    fn on_selection_6(&mut self, value: neocogi_ui::events::ChoiceSelection) -> Result<(), neocogi_com::NcStatus>;
}
impl Event {
    /// Dispatch only after checking the enclosing message's generation.
    pub fn dispatch(self, _handlers: &mut impl Handlers) -> Result<(), neocogi_com::NcStatus> {
        match self {
            Self::Output0(value) => _handlers.on_save_0(value),
            Self::Output1(value) => _handlers.on_type_1(value),
            Self::Output2(value) => _handlers.on_quote_x_2(value),
            Self::Output3(value) => _handlers.on_value_3(value),
            Self::Output4(value) => _handlers.on_pixel_offset_4(value),
            Self::Output5(value) => _handlers.on_pixel_offset_5(value),
            Self::Output6(value) => _handlers.on_selection_6(value),
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
/// Checked concrete handles for every authored widget and the exported commands.
pub struct Form {
    /// Public command endpoints retained for message-based integrations.
    pub inputs: Inputs,
    /// Authored widget "root"; the concrete COM interface is checked when binding.
    pub widget_root_0: neocogi_ui::Widget<neocogi_ui::LinearAbi>,
    /// Authored widget "button"; the concrete COM interface is checked when binding.
    pub widget_button_1: neocogi_ui::Widget<neocogi_ui::ButtonAbi>,
    /// Authored widget "check"; the concrete COM interface is checked when binding.
    pub widget_check_2: neocogi_ui::Widget<neocogi_ui::CheckboxAbi>,
    /// Authored widget "text"; the concrete COM interface is checked when binding.
    pub widget_text_3: neocogi_ui::Widget<neocogi_ui::TextboxAbi>,
    /// Authored widget "slider"; the concrete COM interface is checked when binding.
    pub widget_slider_4: neocogi_ui::Widget<neocogi_ui::SliderAbi>,
    /// Authored widget "scroll"; the concrete COM interface is checked when binding.
    pub widget_scroll_5: neocogi_ui::Widget<neocogi_ui::ScrollbarAbi>,
    /// Authored widget "split"; the concrete COM interface is checked when binding.
    pub widget_split_6: neocogi_ui::Widget<neocogi_ui::SplitViewAbi>,
    /// Authored widget "combo"; the concrete COM interface is checked when binding.
    pub widget_combo_7: neocogi_ui::Widget<neocogi_ui::ComboAbi>,
    /// Authored widget "disclosure"; the concrete COM interface is checked when binding.
    pub widget_disclosure_8: neocogi_ui::Widget<neocogi_ui::DisclosureAbi>,
}
/// Resolves inputs and queues public outputs during candidate preparation.
/// Use inside ReloadSlot::reload; failed preparation leaves the previous component published.
pub fn bind(
    _component: &neocogi_ui_document::Component,
    _generation: neocogi_ui_document::Generation,
    _send: std::sync::mpsc::Sender<Message>,
) -> Result<Form, neocogi_com::NcStatus> {
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
    neocogi_com::events::bind_output(outputs.as_com_ref(), "quote\"\n// x", _send.clone(), move |value: neocogi_ui::textbox::Event| Message { generation: _generation, event: Event::Output2(value) })?;
    neocogi_com::events::bind_output(outputs.as_com_ref(), "value", _send.clone(), move |value: neocogi_ui::events::NumericChanged| Message { generation: _generation, event: Event::Output3(value) })?;
    neocogi_com::events::bind_output(outputs.as_com_ref(), "pixel-offset", _send.clone(), move |value: neocogi_ui::events::ScrollbarChanged| Message { generation: _generation, event: Event::Output4(value) })?;
    neocogi_com::events::bind_output(outputs.as_com_ref(), "pixel_offset", _send.clone(), move |value: neocogi_ui::events::SplitChanged| Message { generation: _generation, event: Event::Output5(value) })?;
    neocogi_com::events::bind_output(outputs.as_com_ref(), "selection", _send.clone(), move |value: neocogi_ui::events::ChoiceSelection| Message { generation: _generation, event: Event::Output6(value) })?;
    Ok(Form {
        inputs,
        widget_root_0: _component.widget::<neocogi_ui::LinearAbi>("root")?,
        widget_button_1: _component.widget::<neocogi_ui::ButtonAbi>("button")?,
        widget_check_2: _component.widget::<neocogi_ui::CheckboxAbi>("check")?,
        widget_text_3: _component.widget::<neocogi_ui::TextboxAbi>("text")?,
        widget_slider_4: _component.widget::<neocogi_ui::SliderAbi>("slider")?,
        widget_scroll_5: _component.widget::<neocogi_ui::ScrollbarAbi>("scroll")?,
        widget_split_6: _component.widget::<neocogi_ui::SplitViewAbi>("split")?,
        widget_combo_7: _component.widget::<neocogi_ui::ComboAbi>("combo")?,
        widget_disclosure_8: _component.widget::<neocogi_ui::DisclosureAbi>("disclosure")?,
    })
}
