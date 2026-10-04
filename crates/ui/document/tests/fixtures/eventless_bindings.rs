// Generated from a validated UI document. Implement Handlers in application-owned code.
/// Complete captured widget events, one variant per event-producing widget.
pub enum Event {
}
/// A captured event belongs to one slot/attempt in the application ReloadIds domain.
pub struct Message {
    /// Preserve the complete slot/attempt pair; check ReloadSlot::accepts before dispatch.
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
/// A captured exclusive event and weak source, still readable after source removal.
pub struct QueuedEvent(pub neocogi_com::events::EventMessage);
impl QueuedEvent {
    /// Call only after generation validation. Removed or foreign sources return None.
    /// Native decoding keeps the COM event owned by this message throughout the call.
    pub fn resolve(&self, form: &Form) -> Result<Option<Event>, neocogi_com::NcStatus> {
        let _ = form;
        let neocogi_com::events::EventOrigin::Widget(origin) = &self.0.origin else { return Ok(None); };
        if !origin.is_alive() { return Ok(None); }
        Ok(None)
    }
}
/// Supplies the common receiver before Loader or ReloadSlot creates widgets.
/// Use the token issued by a slot in the receiving application's ReloadIds domain.
pub fn events(generation: neocogi_ui_document::Generation, send: std::sync::mpsc::Sender<Message>) -> Result<neocogi_ui_document::EventBindings, neocogi_com::NcStatus> {
    let mut bindings = neocogi_ui_document::EventBindings::new();
    let _ = (generation, send, &mut bindings);
    Ok(bindings)
}
