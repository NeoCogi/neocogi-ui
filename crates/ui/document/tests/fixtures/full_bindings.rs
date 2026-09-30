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
/// A captured event belongs to exactly one component reload attempt.
pub struct Message {
    /// Reject stale messages with ReloadSlot::accepts before dispatch.
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
/// Captured facts retain weak origins; resolve against current fields before dispatch.
pub enum QueuedEvent {
    /// Complete stream for instances of neocogi_ui::ButtonAbi.
    Source0 {
        /// Does not retain a removed widget.
        origin: neocogi_com::WeakComBox<neocogi_ui::ButtonAbi>,
        /// Owned snapshot acquired before post returned.
        event: neocogi_ui::button::Event,
    },
    /// Complete stream for instances of neocogi_ui::CheckboxAbi.
    Source1 {
        /// Does not retain a removed widget.
        origin: neocogi_com::WeakComBox<neocogi_ui::CheckboxAbi>,
        /// Owned snapshot acquired before post returned.
        event: neocogi_ui::checkbox::Event,
    },
    /// Complete stream for instances of neocogi_ui::TextboxAbi.
    Source2 {
        /// Does not retain a removed widget.
        origin: neocogi_com::WeakComBox<neocogi_ui::TextboxAbi>,
        /// Owned snapshot acquired before post returned.
        event: neocogi_ui::textbox::Event,
    },
    /// Complete stream for instances of neocogi_ui::SliderAbi.
    Source3 {
        /// Does not retain a removed widget.
        origin: neocogi_com::WeakComBox<neocogi_ui::SliderAbi>,
        /// Owned snapshot acquired before post returned.
        event: neocogi_ui::slider::Event,
    },
    /// Complete stream for instances of neocogi_ui::ScrollbarAbi.
    Source4 {
        /// Does not retain a removed widget.
        origin: neocogi_com::WeakComBox<neocogi_ui::ScrollbarAbi>,
        /// Owned snapshot acquired before post returned.
        event: neocogi_ui::scrollbar::Event,
    },
    /// Complete stream for instances of neocogi_ui::SplitViewAbi.
    Source5 {
        /// Does not retain a removed widget.
        origin: neocogi_com::WeakComBox<neocogi_ui::SplitViewAbi>,
        /// Owned snapshot acquired before post returned.
        event: neocogi_ui::split_view::Event,
    },
    /// Complete stream for instances of neocogi_ui::ComboAbi.
    Source6 {
        /// Does not retain a removed widget.
        origin: neocogi_com::WeakComBox<neocogi_ui::ComboAbi>,
        /// Owned snapshot acquired before post returned.
        event: neocogi_ui::combo::Event,
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
                    origin if origin.same_object(form.widget_button_1.as_interface()) => Some(Event::Widget1(event)),
                    _ => None,
                })
            }
            Self::Source1 { origin, event } => {
                if !origin.is_alive() { return Ok(None); }
                Ok(match origin {
                    origin if origin.same_object(form.widget_check_2.as_interface()) => Some(Event::Widget2(event)),
                    _ => None,
                })
            }
            Self::Source2 { origin, event } => {
                if !origin.is_alive() { return Ok(None); }
                Ok(match origin {
                    origin if origin.same_object(form.widget_text_3.as_interface()) => Some(Event::Widget3(event)),
                    _ => None,
                })
            }
            Self::Source3 { origin, event } => {
                if !origin.is_alive() { return Ok(None); }
                Ok(match origin {
                    origin if origin.same_object(form.widget_slider_4.as_interface()) => Some(Event::Widget4(event)),
                    _ => None,
                })
            }
            Self::Source4 { origin, event } => {
                if !origin.is_alive() { return Ok(None); }
                Ok(match origin {
                    origin if origin.same_object(form.widget_scroll_5.as_interface()) => Some(Event::Widget5(event)),
                    _ => None,
                })
            }
            Self::Source5 { origin, event } => {
                if !origin.is_alive() { return Ok(None); }
                Ok(match origin {
                    origin if origin.same_object(form.widget_split_6.as_interface()) => Some(Event::Widget6(event)),
                    _ => None,
                })
            }
            Self::Source6 { origin, event } => {
                if !origin.is_alive() { return Ok(None); }
                Ok(match origin {
                    origin if origin.same_object(form.widget_combo_7.as_interface()) => Some(Event::Widget7(event)),
                    _ => None,
                })
            }
        }
    }
}
#[neocogi_com::com_object(interfaces(neocogi_ui::nodes::IEventQueueInstance0517d9243d615b7d80421e7a13f2acac, neocogi_ui::nodes::IEventQueueInstance1bb30dd91a74581d939df2bf2c3ed308, neocogi_ui::nodes::IEventQueueInstancec95e3d4dc4375ebf8d2dd2827a5a36d8, neocogi_ui::nodes::IEventQueueInstance7f7563cbeb7b5354b412a35d95c4f0c3, neocogi_ui::nodes::IEventQueueInstanced998bacc3a98553fa30d0c4b9a180789, neocogi_ui::nodes::IEventQueueInstanceaf0d7b53ec305e8ea1ab44ef4721cdad, neocogi_ui::nodes::IEventQueueInstancec51448c5bc925b9db65195711c0433be))]
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
impl neocogi_ui::nodes::IEventQueueInstance1bb30dd91a74581d939df2bf2c3ed308 for FormQueue {
    /// Acquires the snapshot and weak origin, then enqueues without running handlers.
    fn post(&mut self, origin: neocogi_com::ComRef<'_, neocogi_ui::CheckboxAbi>, event: neocogi_com::NcValueRef<'_, neocogi_ui::checkbox::Event>) -> Result<(), neocogi_com::NcStatus> {
        let event = event.to_owned()?;
        neocogi_ui::checkbox::validate(&event)?;
        let event = QueuedEvent::Source1 { origin: origin.downgrade_box()?, event };
        let _ = self.send.send(Message { generation: self.generation, event });
        Ok(())
    }
}
impl neocogi_ui::nodes::IEventQueueInstancec95e3d4dc4375ebf8d2dd2827a5a36d8 for FormQueue {
    /// Acquires the snapshot and weak origin, then enqueues without running handlers.
    fn post(&mut self, origin: neocogi_com::ComRef<'_, neocogi_ui::TextboxAbi>, event: neocogi_com::NcValueRef<'_, neocogi_ui::textbox::Event>) -> Result<(), neocogi_com::NcStatus> {
        let event = event.to_owned()?;
        neocogi_ui::textbox::validate(&event)?;
        let event = QueuedEvent::Source2 { origin: origin.downgrade_box()?, event };
        let _ = self.send.send(Message { generation: self.generation, event });
        Ok(())
    }
}
impl neocogi_ui::nodes::IEventQueueInstance7f7563cbeb7b5354b412a35d95c4f0c3 for FormQueue {
    /// Acquires the snapshot and weak origin, then enqueues without running handlers.
    fn post(&mut self, origin: neocogi_com::ComRef<'_, neocogi_ui::SliderAbi>, event: neocogi_com::NcValueRef<'_, neocogi_ui::slider::Event>) -> Result<(), neocogi_com::NcStatus> {
        let event = event.to_owned()?;
        neocogi_ui::slider::validate(&event)?;
        let event = QueuedEvent::Source3 { origin: origin.downgrade_box()?, event };
        let _ = self.send.send(Message { generation: self.generation, event });
        Ok(())
    }
}
impl neocogi_ui::nodes::IEventQueueInstanced998bacc3a98553fa30d0c4b9a180789 for FormQueue {
    /// Acquires the snapshot and weak origin, then enqueues without running handlers.
    fn post(&mut self, origin: neocogi_com::ComRef<'_, neocogi_ui::ScrollbarAbi>, event: neocogi_com::NcValueRef<'_, neocogi_ui::scrollbar::Event>) -> Result<(), neocogi_com::NcStatus> {
        let event = event.to_owned()?;
        neocogi_ui::scrollbar::validate(&event)?;
        let event = QueuedEvent::Source4 { origin: origin.downgrade_box()?, event };
        let _ = self.send.send(Message { generation: self.generation, event });
        Ok(())
    }
}
impl neocogi_ui::nodes::IEventQueueInstanceaf0d7b53ec305e8ea1ab44ef4721cdad for FormQueue {
    /// Acquires the snapshot and weak origin, then enqueues without running handlers.
    fn post(&mut self, origin: neocogi_com::ComRef<'_, neocogi_ui::SplitViewAbi>, event: neocogi_com::NcValueRef<'_, neocogi_ui::split_view::Event>) -> Result<(), neocogi_com::NcStatus> {
        let event = event.to_owned()?;
        neocogi_ui::split_view::validate(&event)?;
        let event = QueuedEvent::Source5 { origin: origin.downgrade_box()?, event };
        let _ = self.send.send(Message { generation: self.generation, event });
        Ok(())
    }
}
impl neocogi_ui::nodes::IEventQueueInstancec51448c5bc925b9db65195711c0433be for FormQueue {
    /// Acquires the snapshot and weak origin, then enqueues without running handlers.
    fn post(&mut self, origin: neocogi_com::ComRef<'_, neocogi_ui::ComboAbi>, event: neocogi_com::NcValueRef<'_, neocogi_ui::combo::Event>) -> Result<(), neocogi_com::NcStatus> {
        let event = event.to_owned()?;
        neocogi_ui::combo::validate(&event)?;
        let event = QueuedEvent::Source6 { origin: origin.downgrade_box()?, event };
        let _ = self.send.send(Message { generation: self.generation, event });
        Ok(())
    }
}
/// Supplies shared queue projections before Loader or ReloadSlot creates widgets.
pub fn events(generation: neocogi_ui_document::Generation, send: std::sync::mpsc::Sender<Message>) -> Result<neocogi_ui_document::EventBindings, neocogi_com::NcStatus> {
    let mut bindings = neocogi_ui_document::EventBindings::new();
    let queue = FormQueue::from_value(FormQueue { generation, send });
    let queue_0 = queue.query::<neocogi_com::events::QueueOf<neocogi_ui::ButtonAbi>>()?;
    bindings.insert_queue::<neocogi_ui::ButtonAbi>("button", queue_0.as_com_ref())?;
    let queue_1 = queue.query::<neocogi_com::events::QueueOf<neocogi_ui::CheckboxAbi>>()?;
    bindings.insert_queue::<neocogi_ui::CheckboxAbi>("check", queue_1.as_com_ref())?;
    let queue_2 = queue.query::<neocogi_com::events::QueueOf<neocogi_ui::TextboxAbi>>()?;
    bindings.insert_queue::<neocogi_ui::TextboxAbi>("text", queue_2.as_com_ref())?;
    let queue_3 = queue.query::<neocogi_com::events::QueueOf<neocogi_ui::SliderAbi>>()?;
    bindings.insert_queue::<neocogi_ui::SliderAbi>("slider", queue_3.as_com_ref())?;
    let queue_4 = queue.query::<neocogi_com::events::QueueOf<neocogi_ui::ScrollbarAbi>>()?;
    bindings.insert_queue::<neocogi_ui::ScrollbarAbi>("scroll", queue_4.as_com_ref())?;
    let queue_5 = queue.query::<neocogi_com::events::QueueOf<neocogi_ui::SplitViewAbi>>()?;
    bindings.insert_queue::<neocogi_ui::SplitViewAbi>("split", queue_5.as_com_ref())?;
    let queue_6 = queue.query::<neocogi_com::events::QueueOf<neocogi_ui::ComboAbi>>()?;
    bindings.insert_queue::<neocogi_ui::ComboAbi>("combo", queue_6.as_com_ref())?;
    Ok(bindings)
}
