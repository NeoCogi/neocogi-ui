// Generated from a validated UI document. Keep application handlers in another module.
/// Captured public events; private visual connections remain inside the loaded component.
pub enum Event {
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
}
impl Event {
    /// Dispatch only after checking the enclosing message's generation.
    pub fn dispatch(self, _handlers: &mut impl Handlers) -> Result<(), neocogi_com::NcStatus> {
        match self {
        }
    }
}
/// Reconnect these concrete input handles after each successful component replacement.
pub struct Inputs {
}
/// Resolves inputs and queues public outputs during candidate preparation.
/// Use inside ReloadSlot::reload; failed preparation leaves the previous component published.
pub fn bind(
    _component: &neocogi_ui_document::Component,
    _generation: neocogi_ui_document::Generation,
    _send: std::sync::mpsc::Sender<Message>,
) -> Result<Inputs, neocogi_com::NcStatus> {
    let inputs = Inputs {
    };
    Ok(inputs)
}
