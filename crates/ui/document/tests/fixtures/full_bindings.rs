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
    pub event: Event,
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
/// Prepares application receivers before Loader or ReloadSlot creates widgets.
pub fn events(
    generation: neocogi_ui_document::Generation,
    send: std::sync::mpsc::Sender<Message>,
) -> Result<neocogi_ui_document::EventBindings, neocogi_com::NcStatus> {
    let mut bindings = neocogi_ui_document::EventBindings::new();
    bindings.insert::<neocogi_ui::ButtonAbi, _>("button", send.clone(), move |event: neocogi_ui::button::Event| Message { generation, event: Event::Widget1(event) })?;
    bindings.insert::<neocogi_ui::CheckboxAbi, _>("check", send.clone(), move |event: neocogi_ui::checkbox::Event| Message { generation, event: Event::Widget2(event) })?;
    bindings.insert::<neocogi_ui::TextboxAbi, _>("text", send.clone(), move |event: neocogi_ui::textbox::Event| Message { generation, event: Event::Widget3(event) })?;
    bindings.insert::<neocogi_ui::SliderAbi, _>("slider", send.clone(), move |event: neocogi_ui::slider::Event| Message { generation, event: Event::Widget4(event) })?;
    bindings.insert::<neocogi_ui::ScrollbarAbi, _>("scroll", send.clone(), move |event: neocogi_ui::scrollbar::Event| Message { generation, event: Event::Widget5(event) })?;
    bindings.insert::<neocogi_ui::SplitViewAbi, _>("split", send.clone(), move |event: neocogi_ui::split_view::Event| Message { generation, event: Event::Widget6(event) })?;
    bindings.insert::<neocogi_ui::ComboAbi, _>("combo", send.clone(), move |event: neocogi_ui::combo::Event| Message { generation, event: Event::Widget7(event) })?;
    Ok(bindings)
}
