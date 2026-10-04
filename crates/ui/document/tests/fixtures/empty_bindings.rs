// Generated from a validated UI document. Implement Handlers in application-owned code.
/// Complete captured widget events, one variant per event-producing widget.
pub enum Event {
    /// Complete event stream from authored widget "root".
    Widget0(neocogi_ui::button::Event),
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
    /// Handles the event from widget "root" after traversal releases widget borrows.
    fn on_root_0(&mut self, event: neocogi_ui::button::Event) -> Result<(), neocogi_com::NcStatus>;
}
impl Event {
    /// Dispatch only after checking the enclosing message generation.
    pub fn dispatch(self, handlers: &mut impl Handlers) -> Result<(), neocogi_com::NcStatus> {
        let _ = &handlers;
        match self {
            Self::Widget0(event) => handlers.on_root_0(event),
        }
    }
}
/// Checked widget fields expose their own getters and setters.
/// Resolve a new Form after replacement; old handles observe widget closure.
pub struct Form {
    /// Authored widget "root"; the concrete COM interface is checked when binding.
    pub widget_root_0: neocogi_ui::Widget<neocogi_ui::ButtonAbi>,
}
/// Resolves concrete fields after construction has installed every receiver.
pub fn bind(
    component: &neocogi_ui_document::Component,
) -> Result<Form, neocogi_com::NcStatus> {
    let form = Form {
        widget_root_0: component.widget::<neocogi_ui::ButtonAbi>("root")?,
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
        if origin.same_object(form.widget_root_0.as_interface()) {
            return Ok(self.0.payload::<neocogi_ui::button::Event>()?.map(Event::Widget0));
        }
        Ok(None)
    }
}
#[neocogi_com::com_object(interfaces(neocogi_com::events::IEventReceiver))]
/// One receiver serves every widget in this construction attempt.
struct FormQueue {
    /// Slot/attempt identity is checked before interpreting an event.
    generation: neocogi_ui_document::Generation,
    /// Posting access owns no application state or widgets.
    send: std::sync::mpsc::Sender<Message>,
}
impl neocogi_com::events::IEventReceiver for FormQueue {
    /// Move the event into the inbox; rejection restores exactly the original owner.
    fn post(&mut self, origin: neocogi_com::NcValueRef<'_, neocogi_com::events::EventOrigin>, event: neocogi_com::ComBox<neocogi_com::events::IEventAbi>) -> Result<(), (neocogi_com::NcStatus, neocogi_com::ComBox<neocogi_com::events::IEventAbi>)> {
        let event = QueuedEvent(neocogi_com::events::EventMessage::capture(origin, event)?);
        self.send.send(Message { generation: self.generation, event }).map_err(|error| (neocogi_com::NC_E_CLOSED, error.0.event.0.event))
    }
}
/// Supplies the common receiver before Loader or ReloadSlot creates widgets.
/// Use the token issued by a slot in the receiving application's ReloadIds domain.
pub fn events(generation: neocogi_ui_document::Generation, send: std::sync::mpsc::Sender<Message>) -> Result<neocogi_ui_document::EventBindings, neocogi_com::NcStatus> {
    let mut bindings = neocogi_ui_document::EventBindings::new();
    let queue = FormQueue::from_value(FormQueue { generation, send }).query::<neocogi_com::events::IEventReceiverAbi>()?;
    bindings.insert_queue("root", queue.as_com_ref())?;
    Ok(bindings)
}
