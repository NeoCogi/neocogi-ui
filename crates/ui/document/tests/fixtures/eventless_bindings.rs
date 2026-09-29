// Generated from a validated UI document. Implement Handlers in application-owned code.
/// Complete captured widget events, one variant per event-producing widget.
pub enum Event {
}
/// A captured event belongs to exactly one component reload attempt.
pub struct Message {
    /// Reject stale messages with ReloadSlot::accepts before dispatch.
    pub generation: neocogi_ui_document::Generation,
    /// Complete typed widget event.
    pub event: QueuedEvent,
}
/// Application-owned behavior; every emitting widget requires an implementation.
pub trait Handlers {
}
impl Event {
    /// Dispatch only after checking the enclosing message generation.
    pub fn dispatch(self, handlers: &mut impl Handlers) -> Result<(), neocogi_com::NcStatus> {
        let _ = &handlers;
        match self {
        }
    }
}
/// Checked widget fields expose their own getters and setters.
/// Resolve a new Form after replacement; old handles observe widget closure.
pub struct Form {
    /// Authored widget "label"; the concrete COM interface is checked when binding.
    pub widget_label_0: neocogi_ui::Widget<neocogi_ui::TextBlockAbi>,
}
/// Resolves concrete fields after construction has installed every receiver.
pub fn bind(
    component: &neocogi_ui_document::Component,
) -> Result<Form, neocogi_com::NcStatus> {
    let form = Form {
        widget_label_0: component.widget::<neocogi_ui::TextBlockAbi>("label")?,
    };
    Ok(form)
}
/// Captured facts retain weak origins; resolve against current fields before dispatch.
pub enum QueuedEvent {
}
impl QueuedEvent {
    /// Resolve only after generation validation; expired or foreign origins return None.
    /// Inspect the captured payload directly when its origin lifetime is irrelevant.
    pub fn resolve(self, form: &Form) -> Result<Option<Event>, neocogi_com::NcStatus> {
        let _ = &form;
        match self {
        }
    }
}
/// Supplies shared queue projections before Loader or ReloadSlot creates widgets.
pub fn events(generation: neocogi_ui_document::Generation, send: std::sync::mpsc::Sender<Message>) -> Result<neocogi_ui_document::EventBindings, neocogi_com::NcStatus> {
    let mut bindings = neocogi_ui_document::EventBindings::new();
    let _ = (generation, send, &mut bindings);
    Ok(bindings)
}
