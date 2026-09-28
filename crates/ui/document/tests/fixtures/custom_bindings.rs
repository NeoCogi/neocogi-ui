// Generated from a validated UI document. Implement Handlers in application-owned code.
/// Complete captured widget events, one variant per event-producing widget.
pub enum Event {
    /// Complete event stream from authored widget "save".
    Widget2(neocogi_ui::button::Event),
    /// Complete event stream from authored widget "extension".
    Widget3(neocogi_demo_contracts::CounterEvent),
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
    /// Handles the event from widget "save" after traversal releases widget borrows.
    fn on_save_2(&mut self, event: neocogi_ui::button::Event) -> Result<(), neocogi_com::NcStatus>;
    /// Handles the event from widget "extension" after traversal releases widget borrows.
    fn on_extension_3(&mut self, event: neocogi_demo_contracts::CounterEvent) -> Result<(), neocogi_com::NcStatus>;
}
impl Event {
    /// Dispatch only after checking the enclosing message generation.
    pub fn dispatch(self, handlers: &mut impl Handlers) -> Result<(), neocogi_com::NcStatus> {
        match self {
            Self::Widget2(event) => handlers.on_save_2(event),
            Self::Widget3(event) => handlers.on_extension_3(event),
        }
    }
}
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
    pub widget_extension_3: neocogi_ui::Widget<neocogi_demo_contracts::CounterAbi>,
}
/// Resolves concrete fields after construction has installed every receiver.
pub fn bind(
    component: &neocogi_ui_document::Component,
) -> Result<Form, neocogi_com::NcStatus> {
    let form = Form {
        widget_root_0: component.widget::<neocogi_ui::LinearAbi>("root")?,
        widget_title_1: component.widget::<neocogi_ui::TextBlockAbi>("title")?,
        widget_save_2: component.widget::<neocogi_ui::ButtonAbi>("save")?,
        widget_extension_3: component.widget::<neocogi_demo_contracts::CounterAbi>("extension")?,
    };
    Ok(form)
}
/// Prepares application receivers before Loader or ReloadSlot creates widgets.
pub fn events(
    generation: neocogi_ui_document::Generation,
    send: std::sync::mpsc::Sender<Message>,
) -> Result<neocogi_ui_document::EventBindings, neocogi_com::NcStatus> {
    let mut bindings = neocogi_ui_document::EventBindings::new();
    bindings.insert::<neocogi_ui::ButtonAbi, _>("save", send.clone(), move |event: neocogi_ui::button::Event| Message { generation, event: Event::Widget2(event) })?;
    bindings.insert::<neocogi_demo_contracts::CounterAbi, _>("extension", send.clone(), move |event: neocogi_demo_contracts::CounterEvent| Message { generation, event: Event::Widget3(event) })?;
    Ok(bindings)
}
