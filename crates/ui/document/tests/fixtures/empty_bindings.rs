// Generated from a validated UI document. Implement Handlers in application-owned code.
/// Complete captured widget events, one variant per event-producing widget.
pub enum Event {
    /// Complete event stream from authored widget "root".
    Widget0(neocogi_ui::events::Activation),
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
    /// Handles the event from widget "root" after traversal releases widget borrows.
    fn on_root_0(&mut self, event: neocogi_ui::events::Activation) -> Result<(), neocogi_com::NcStatus>;
}
impl Event {
    /// Dispatch only after checking the enclosing message generation.
    pub fn dispatch(self, handlers: &mut impl Handlers) -> Result<(), neocogi_com::NcStatus> {
        match self {
            Self::Widget0(event) => handlers.on_root_0(event),
        }
    }
}
/// Reconnect concrete command handles after every successful component replacement.
pub struct Inputs {
}
/// Checked concrete handles for every authored widget and the exported commands.
pub struct Form {
    /// Public command endpoints retained for message-based integrations.
    pub inputs: Inputs,
    /// Authored widget "root"; the concrete COM interface is checked when binding.
    pub widget_root_0: neocogi_ui::Widget<neocogi_ui::ButtonAbi>,
}
/// Checks widget fields and binds each complete event stream to the application queue.
/// Use inside ReloadSlot::reload; failed preparation preserves the published component.
pub fn bind(
    component: &neocogi_ui_document::Component,
    generation: neocogi_ui_document::Generation,
    send: std::sync::mpsc::Sender<Message>,
) -> Result<Form, neocogi_com::NcStatus> {
    let inputs = Inputs {
    };
    let form = Form {
        inputs,
        widget_root_0: component.widget::<neocogi_ui::ButtonAbi>("root")?,
    };
    let outputs = component.outputs()?;
    neocogi_com::events::bind_output(outputs.as_com_ref(), "root", send.clone(), move |event: neocogi_ui::events::Activation| Message { generation, event: Event::Widget0(event) })?;
    Ok(form)
}
