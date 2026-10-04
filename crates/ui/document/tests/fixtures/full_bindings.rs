// Generated from a validated UI document. Implement Handlers in application-owned code.
/// Complete captured widget events, one variant per event-producing widget.
pub enum Event {
    /// Complete event stream from authored widget "button".
    Widget1(neocogi_ui::button::Event),
    /// Complete event stream from authored widget "check".
    Widget2(neocogi_ui::checkbox::Event),
    /// Complete event stream from authored widget "text".
    Widget3(neocogi_ui::textbox::Event),
    /// Complete event stream from authored widget "slider".
    Widget4(neocogi_ui::slider::Event),
    /// Complete event stream from authored widget "scroll".
    Widget5(neocogi_ui::scrollbar::Event),
    /// Complete event stream from authored widget "split".
    Widget6(neocogi_ui::split_view::Event),
    /// Complete event stream from authored widget "combo".
    Widget7(neocogi_ui::combo::Event),
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
    /// Handles the event from widget "button" after traversal releases widget borrows.
    fn on_button_1(&mut self, event: neocogi_ui::button::Event) -> Result<(), neocogi_com::NcStatus>;
    /// Handles the event from widget "check" after traversal releases widget borrows.
    fn on_check_2(&mut self, event: neocogi_ui::checkbox::Event) -> Result<(), neocogi_com::NcStatus>;
    /// Handles the event from widget "text" after traversal releases widget borrows.
    fn on_text_3(&mut self, event: neocogi_ui::textbox::Event) -> Result<(), neocogi_com::NcStatus>;
    /// Handles the event from widget "slider" after traversal releases widget borrows.
    fn on_slider_4(&mut self, event: neocogi_ui::slider::Event) -> Result<(), neocogi_com::NcStatus>;
    /// Handles the event from widget "scroll" after traversal releases widget borrows.
    fn on_scroll_5(&mut self, event: neocogi_ui::scrollbar::Event) -> Result<(), neocogi_com::NcStatus>;
    /// Handles the event from widget "split" after traversal releases widget borrows.
    fn on_split_6(&mut self, event: neocogi_ui::split_view::Event) -> Result<(), neocogi_com::NcStatus>;
    /// Handles the event from widget "combo" after traversal releases widget borrows.
    fn on_combo_7(&mut self, event: neocogi_ui::combo::Event) -> Result<(), neocogi_com::NcStatus>;
}
impl Event {
    /// Dispatch only after checking the enclosing message generation.
    pub fn dispatch(self, handlers: &mut impl Handlers) -> Result<(), neocogi_com::NcStatus> {
        let _ = &handlers;
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
/// Checked widget fields expose their own getters and setters.
/// Resolve a new Form after replacement; old handles observe widget closure.
pub struct Form {
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
/// Resolves concrete fields after construction has installed every receiver.
pub fn bind(
    component: &neocogi_ui_document::Component,
) -> Result<Form, neocogi_com::NcStatus> {
    let form = Form {
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
        if origin.same_object(form.widget_button_1.as_interface()) {
            return Ok(self.0.payload::<neocogi_ui::button::Event>()?.map(Event::Widget1));
        }
        if origin.same_object(form.widget_check_2.as_interface()) {
            return Ok(self.0.payload::<neocogi_ui::checkbox::Event>()?.map(Event::Widget2));
        }
        if origin.same_object(form.widget_text_3.as_interface()) {
            return Ok(self.0.payload::<neocogi_ui::textbox::Event>()?.map(Event::Widget3));
        }
        if origin.same_object(form.widget_slider_4.as_interface()) {
            return Ok(self.0.payload::<neocogi_ui::slider::Event>()?.map(Event::Widget4));
        }
        if origin.same_object(form.widget_scroll_5.as_interface()) {
            return Ok(self.0.payload::<neocogi_ui::scrollbar::Event>()?.map(Event::Widget5));
        }
        if origin.same_object(form.widget_split_6.as_interface()) {
            return Ok(self.0.payload::<neocogi_ui::split_view::Event>()?.map(Event::Widget6));
        }
        if origin.same_object(form.widget_combo_7.as_interface()) {
            return Ok(self.0.payload::<neocogi_ui::combo::Event>()?.map(Event::Widget7));
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
    bindings.insert_queue("button", queue.as_com_ref())?;
    bindings.insert_queue("check", queue.as_com_ref())?;
    bindings.insert_queue("text", queue.as_com_ref())?;
    bindings.insert_queue("slider", queue.as_com_ref())?;
    bindings.insert_queue("scroll", queue.as_com_ref())?;
    bindings.insert_queue("split", queue.as_com_ref())?;
    bindings.insert_queue("combo", queue.as_com_ref())?;
    Ok(bindings)
}
