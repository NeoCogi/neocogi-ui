// Generated from a validated UI document. Implement Handlers in application-owned code.
/// Complete captured widget events, one variant per event-producing widget.
pub enum Event {
    /// Complete event stream from authored widget "first".
    Widget1(neocogi_ui::button::Event),
    /// Complete event stream from authored widget "second".
    Widget2(neocogi_ui::button::Event),
    /// Complete event stream from authored widget "text".
    Widget3(neocogi_ui::textbox::Event),
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
    /// Handles the event from widget "first" after traversal releases widget borrows.
    fn on_first_1(&mut self, event: neocogi_ui::button::Event) -> Result<(), neocogi_com::NcStatus>;
    /// Handles the event from widget "second" after traversal releases widget borrows.
    fn on_second_2(&mut self, event: neocogi_ui::button::Event) -> Result<(), neocogi_com::NcStatus>;
    /// Handles the event from widget "text" after traversal releases widget borrows.
    fn on_text_3(&mut self, event: neocogi_ui::textbox::Event) -> Result<(), neocogi_com::NcStatus>;
}
impl Event {
    /// Dispatch only after checking the enclosing message generation.
    pub fn dispatch(self, handlers: &mut impl Handlers) -> Result<(), neocogi_com::NcStatus> {
        let _ = &handlers;
        match self {
            Self::Widget1(event) => handlers.on_first_1(event),
            Self::Widget2(event) => handlers.on_second_2(event),
            Self::Widget3(event) => handlers.on_text_3(event),
        }
    }
}
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
/// A captured exclusive event and weak source, still readable after source removal.
pub struct QueuedEvent(pub neocogi_com::events::EventMessage);
impl QueuedEvent {
    /// Call only after generation validation. Removed or foreign sources return None.
    /// Native decoding keeps the COM event owned by this message throughout the call.
    pub fn resolve(&self, form: &Form) -> Result<Option<Event>, neocogi_com::NcStatus> {
        let _ = form;
        let neocogi_com::events::EventOrigin::Widget(origin) = &self.0.origin else { return Ok(None); };
        if !origin.is_alive() { return Ok(None); }
        if origin.same_object(form.widget_first_1.as_interface()) {
            return Ok(self.0.payload::<neocogi_ui::button::Event>()?.map(Event::Widget1));
        }
        if origin.same_object(form.widget_second_2.as_interface()) {
            return Ok(self.0.payload::<neocogi_ui::button::Event>()?.map(Event::Widget2));
        }
        if origin.same_object(form.widget_text_3.as_interface()) {
            return Ok(self.0.payload::<neocogi_ui::textbox::Event>()?.map(Event::Widget3));
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
    bindings.insert_queue("first", queue.as_com_ref())?;
    bindings.insert_queue("second", queue.as_com_ref())?;
    bindings.insert_queue("text", queue.as_com_ref())?;
    Ok(bindings)
}
