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
/// A captured event belongs to exactly one component reload attempt.
pub struct Message {
    /// Reject stale messages with ReloadSlot::accepts before dispatch.
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
/// Captured facts retain weak origins; resolve against current fields before dispatch.
pub enum QueuedEvent {
    /// Complete stream for instances of neocogi_ui::ButtonAbi.
    Source0 {
        /// Does not retain a removed widget.
        origin: neocogi_com::WeakComBox<neocogi_ui::ButtonAbi>,
        /// Owned snapshot acquired before post returned.
        event: neocogi_ui::button::Event,
    },
    /// Complete stream for instances of neocogi_ui::TextboxAbi.
    Source1 {
        /// Does not retain a removed widget.
        origin: neocogi_com::WeakComBox<neocogi_ui::TextboxAbi>,
        /// Owned snapshot acquired before post returned.
        event: neocogi_ui::textbox::Event,
    },
}
impl QueuedEvent {
    /// Resolve only after generation validation; expired or foreign origins return None.
    /// Inspect the captured payload directly when its origin lifetime is irrelevant.
    pub fn resolve(self, form: &Form) -> Result<Option<Event>, neocogi_com::NcStatus> {
        let _ = &form;
        match self {
            Self::Source0 { origin, event } => {
                if !origin.is_alive() { return Ok(None); }
                Ok(match origin {
                    origin if origin.same_object(form.widget_first_1.as_interface()) => Some(Event::Widget1(event)),
                    origin if origin.same_object(form.widget_second_2.as_interface()) => Some(Event::Widget2(event)),
                    _ => None,
                })
            }
            Self::Source1 { origin, event } => {
                if !origin.is_alive() { return Ok(None); }
                Ok(match origin {
                    origin if origin.same_object(form.widget_text_3.as_interface()) => Some(Event::Widget3(event)),
                    _ => None,
                })
            }
        }
    }
}
#[neocogi_com::com_object(interfaces(neocogi_ui::nodes::IEventQueueInstance0517d9243d615b7d80421e7a13f2acac, neocogi_ui::nodes::IEventQueueInstancec95e3d4dc4375ebf8d2dd2827a5a36d8))]
/// One application queue allocation serves every widget in this construction attempt.
struct FormQueue {
    /// Fixed reload identity, independent of widget lifetime.
    generation: neocogi_ui_document::Generation,
    /// Retains the channel, never the form, model or widgets.
    send: std::sync::mpsc::Sender<Message>,
}
impl neocogi_ui::nodes::IEventQueueInstance0517d9243d615b7d80421e7a13f2acac for FormQueue {
    /// Acquires the snapshot and weak origin, then enqueues without running handlers.
    fn post(&mut self, origin: neocogi_com::ComRef<'_, neocogi_ui::ButtonAbi>, event: neocogi_com::NcValueRef<'_, neocogi_ui::button::Event>) -> Result<(), neocogi_com::NcStatus> {
        let event = event.to_owned()?;
        neocogi_ui::button::validate(&event)?;
        let event = QueuedEvent::Source0 { origin: origin.downgrade_box()?, event };
        let _ = self.send.send(Message { generation: self.generation, event });
        Ok(())
    }
}
impl neocogi_ui::nodes::IEventQueueInstancec95e3d4dc4375ebf8d2dd2827a5a36d8 for FormQueue {
    /// Acquires the snapshot and weak origin, then enqueues without running handlers.
    fn post(&mut self, origin: neocogi_com::ComRef<'_, neocogi_ui::TextboxAbi>, event: neocogi_com::NcValueRef<'_, neocogi_ui::textbox::Event>) -> Result<(), neocogi_com::NcStatus> {
        let event = event.to_owned()?;
        neocogi_ui::textbox::validate(&event)?;
        let event = QueuedEvent::Source1 { origin: origin.downgrade_box()?, event };
        let _ = self.send.send(Message { generation: self.generation, event });
        Ok(())
    }
}
/// Supplies shared queue projections before Loader or ReloadSlot creates widgets.
pub fn events(generation: neocogi_ui_document::Generation, send: std::sync::mpsc::Sender<Message>) -> Result<neocogi_ui_document::EventBindings, neocogi_com::NcStatus> {
    let mut bindings = neocogi_ui_document::EventBindings::new();
    let queue = FormQueue::from_value(FormQueue { generation, send });
    let queue_0 = queue.query::<neocogi_com::events::QueueOf<neocogi_ui::ButtonAbi>>()?;
    bindings.insert_queue::<neocogi_ui::ButtonAbi>("first", queue_0.as_com_ref())?;
    bindings.insert_queue::<neocogi_ui::ButtonAbi>("second", queue_0.as_com_ref())?;
    let queue_1 = queue.query::<neocogi_com::events::QueueOf<neocogi_ui::TextboxAbi>>()?;
    bindings.insert_queue::<neocogi_ui::TextboxAbi>("text", queue_1.as_com_ref())?;
    Ok(bindings)
}
