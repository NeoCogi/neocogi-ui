// Generated from a validated UI document. Implement Handlers in application-owned code.
/// Complete captured widget events, one variant per event-producing widget.
pub enum Event {
    /// Complete event stream from authored widget "button".
    Widget1(neocogi_ui::events::Activation),
    /// Complete event stream from authored widget "check".
    Widget2(neocogi_ui::events::CheckboxChanged),
    /// Complete event stream from authored widget "text".
    Widget3(neocogi_ui::textbox::Event),
    /// Complete event stream from authored widget "slider".
    Widget4(neocogi_ui::events::NumericChanged),
    /// Complete event stream from authored widget "scroll".
    Widget5(neocogi_ui::events::ScrollbarChanged),
    /// Complete event stream from authored widget "split".
    Widget6(neocogi_ui::events::SplitChanged),
    /// Complete event stream from authored widget "combo".
    Widget7(neocogi_ui::events::ChoiceSelection),
}
/// A captured event belongs to exactly one component reload attempt.
pub struct Message {
    /// Reject stale messages with ReloadSlot::accepts before dispatch.
    pub generation: neocogi_ui_document::Generation,
    /// Complete typed widget event.
    pub event: Event,
}
/// Application-owned behavior; every emitting widget requires an implementation.
pub trait Handlers {
    /// Handles the event from widget "button" after traversal releases widget borrows.
    fn on_button_1(&mut self, event: neocogi_ui::events::Activation) -> Result<(), neocogi_com::NcStatus>;
    /// Handles the event from widget "check" after traversal releases widget borrows.
    fn on_check_2(&mut self, event: neocogi_ui::events::CheckboxChanged) -> Result<(), neocogi_com::NcStatus>;
    /// Handles the event from widget "text" after traversal releases widget borrows.
    fn on_text_3(&mut self, event: neocogi_ui::textbox::Event) -> Result<(), neocogi_com::NcStatus>;
    /// Handles the event from widget "slider" after traversal releases widget borrows.
    fn on_slider_4(&mut self, event: neocogi_ui::events::NumericChanged) -> Result<(), neocogi_com::NcStatus>;
    /// Handles the event from widget "scroll" after traversal releases widget borrows.
    fn on_scroll_5(&mut self, event: neocogi_ui::events::ScrollbarChanged) -> Result<(), neocogi_com::NcStatus>;
    /// Handles the event from widget "split" after traversal releases widget borrows.
    fn on_split_6(&mut self, event: neocogi_ui::events::SplitChanged) -> Result<(), neocogi_com::NcStatus>;
    /// Handles the event from widget "combo" after traversal releases widget borrows.
    fn on_combo_7(&mut self, event: neocogi_ui::events::ChoiceSelection) -> Result<(), neocogi_com::NcStatus>;
}
impl Event {
    /// Dispatch only after checking the enclosing message generation.
    pub fn dispatch(self, handlers: &mut impl Handlers) -> Result<(), neocogi_com::NcStatus> {
        match self {
            Self::Widget1(event) => handlers.on_button_1(event),
            Self::Widget2(event) => handlers.on_check_2(event),
            Self::Widget3(event) => handlers.on_text_3(event),
            Self::Widget4(event) => handlers.on_slider_4(event),
            Self::Widget5(event) => handlers.on_scroll_5(event),
            Self::Widget6(event) => handlers.on_split_6(event),
            Self::Widget7(event) => handlers.on_combo_7(event),
        }
    }
}
/// Reconnect concrete command handles after every successful component replacement.
pub struct Inputs {
    /// Public command "Text"; its receiver is checked when binding.
    pub input_text_0: neocogi_ui_document::InputHandle<neocogi_ui_document::contracts::SetText>,
    /// Public command "Checked"; its receiver is checked when binding.
    pub input_checked_1: neocogi_ui_document::InputHandle<neocogi_ui_document::contracts::SetChecked>,
    /// Public command "Value"; its receiver is checked when binding.
    pub input_value_2: neocogi_ui_document::InputHandle<neocogi_ui_document::contracts::SetValue>,
    /// Public command "Enabled"; its receiver is checked when binding.
    pub input_enabled_3: neocogi_ui_document::InputHandle<neocogi_ui_document::contracts::SetEnabled>,
    /// Public command "Expanded"; its receiver is checked when binding.
    pub input_expanded_4: neocogi_ui_document::InputHandle<neocogi_ui_document::contracts::SetExpanded>,
    /// Public command "Choices"; its receiver is checked when binding.
    pub input_choices_5: neocogi_ui_document::InputHandle<neocogi_ui_document::contracts::SetChoices>,
    /// Public command "Selection"; its receiver is checked when binding.
    pub input_selection_6: neocogi_ui_document::InputHandle<neocogi_ui_document::contracts::SetSelection>,
    /// Public command "Offset"; its receiver is checked when binding.
    pub input_offset_7: neocogi_ui_document::InputHandle<neocogi_ui_document::contracts::SetOffset>,
    /// Public command "Position"; its receiver is checked when binding.
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
/// Checks widget fields and binds each complete event stream to the application queue.
/// Use inside ReloadSlot::reload; failed preparation preserves the published component.
pub fn bind(
    component: &neocogi_ui_document::Component,
    generation: neocogi_ui_document::Generation,
    send: std::sync::mpsc::Sender<Message>,
) -> Result<Form, neocogi_com::NcStatus> {
    let inputs = Inputs {
        input_text_0: component.input::<neocogi_ui_document::contracts::SetText>("Text")?,
        input_checked_1: component.input::<neocogi_ui_document::contracts::SetChecked>("Checked")?,
        input_value_2: component.input::<neocogi_ui_document::contracts::SetValue>("Value")?,
        input_enabled_3: component.input::<neocogi_ui_document::contracts::SetEnabled>("Enabled")?,
        input_expanded_4: component.input::<neocogi_ui_document::contracts::SetExpanded>("Expanded")?,
        input_choices_5: component.input::<neocogi_ui_document::contracts::SetChoices>("Choices")?,
        input_selection_6: component.input::<neocogi_ui_document::contracts::SetSelection>("Selection")?,
        input_offset_7: component.input::<neocogi_ui_document::contracts::SetOffset>("Offset")?,
        input_position_8: component.input::<neocogi_ui_document::contracts::SetPosition>("Position")?,
    };
    let form = Form {
        inputs,
        widget_root_0: component.widget::<neocogi_ui::LinearAbi>("root")?,
        widget_button_1: component.widget::<neocogi_ui::ButtonAbi>("button")?,
        widget_check_2: component.widget::<neocogi_ui::CheckboxAbi>("check")?,
        widget_text_3: component.widget::<neocogi_ui::TextboxAbi>("text")?,
        widget_slider_4: component.widget::<neocogi_ui::SliderAbi>("slider")?,
        widget_scroll_5: component.widget::<neocogi_ui::ScrollbarAbi>("scroll")?,
        widget_split_6: component.widget::<neocogi_ui::SplitViewAbi>("split")?,
        widget_combo_7: component.widget::<neocogi_ui::ComboAbi>("combo")?,
        widget_disclosure_8: component.widget::<neocogi_ui::DisclosureAbi>("disclosure")?,
    };
    let outputs = component.outputs()?;
    neocogi_com::events::bind_output(outputs.as_com_ref(), "save", send.clone(), move |event: neocogi_ui::events::Activation| Message { generation, event: Event::Widget1(event) })?;
    neocogi_com::events::bind_output(outputs.as_com_ref(), "type", send.clone(), move |event: neocogi_ui::events::CheckboxChanged| Message { generation, event: Event::Widget2(event) })?;
    neocogi_com::events::bind_output(outputs.as_com_ref(), "quote\"\n// x", send.clone(), move |event: neocogi_ui::textbox::Event| Message { generation, event: Event::Widget3(event) })?;
    neocogi_com::events::bind_output(outputs.as_com_ref(), "value", send.clone(), move |event: neocogi_ui::events::NumericChanged| Message { generation, event: Event::Widget4(event) })?;
    neocogi_com::events::bind_output(outputs.as_com_ref(), "pixel-offset", send.clone(), move |event: neocogi_ui::events::ScrollbarChanged| Message { generation, event: Event::Widget5(event) })?;
    neocogi_com::events::bind_output(outputs.as_com_ref(), "pixel_offset", send.clone(), move |event: neocogi_ui::events::SplitChanged| Message { generation, event: Event::Widget6(event) })?;
    neocogi_com::events::bind_output(outputs.as_com_ref(), "selection", send.clone(), move |event: neocogi_ui::events::ChoiceSelection| Message { generation, event: Event::Widget7(event) })?;
    Ok(form)
}
