//! The form is each child's sole event consumer and handles captured events after traversal.
use crate::*;
use neocogi_com::{ComPtr, NcStatus, NC_E_INVALID_ARG};
use neocogi_ui::{nodes, IChoiceMenuEvent, IChoiceMenuObserverAbi};
use neocogi_ui::events::{Controller, EventModel};
use std::{collections::BTreeMap, sync::mpsc::Receiver};

/// One child's entire typed event stream, captured by the form's construction-time observer.
pub(crate) enum WidgetEvent {
    /// Button activation.
    Button(nodes::ButtonEvent),
    /// Checkbox state transition.
    Checkbox(nodes::CheckboxEvent),
    /// Single-line text edit or submission.
    Textbox(nodes::TextboxEvent),
    /// Multiline text edit or submission.
    TextArea(nodes::TextAreaEvent),
    /// Slider value transition.
    Slider(nodes::SliderEvent),
    /// Numeric-entry value transition.
    Number(nodes::NumberEvent),
    /// List-row activation.
    ListItem(nodes::ListItemEvent),
    /// List-box activation.
    ListBox(nodes::ListBoxEvent),
    /// Scrollbar offset transition.
    Scrollbar(nodes::ScrollbarEvent),
    /// Split-view divider transition.
    Split(nodes::SplitViewEvent),
    /// Combo selection transition.
    Combo(IChoiceMenuEvent),
}
impl WidgetEvent {
    /// Applies the same typed domain checks as the ordinary public widget observers.
    /// Custom providers can post ABI-valid values, so a form must check them before
    /// queuing application forwarding.
    pub(crate) fn validate(&self) -> Result<(), NcStatus> {
        match self {
            Self::Button(value) => neocogi_ui::button::validate(value),
            Self::Checkbox(value) => neocogi_ui::checkbox::validate(value),
            Self::Textbox(value) => neocogi_ui::textbox::validate(value),
            Self::TextArea(value) => neocogi_ui::text_area::validate(value),
            Self::Slider(value) => neocogi_ui::slider::validate(value),
            Self::Number(value) => neocogi_ui::number::validate(value),
            Self::ListItem(value) => neocogi_ui::list_item::validate(value),
            Self::ListBox(value) => neocogi_ui::list_box::validate(value),
            Self::Scrollbar(value) => neocogi_ui::scrollbar::validate(value),
            Self::Split(value) => neocogi_ui::split_view::validate(value),
            Self::Combo(value) => neocogi_ui::combo::validate(value),
        }
    }

    /// Names the captured enum case for nonfatal delivery diagnostics.
    fn variant(&self) -> &'static str {
        match self {
            Self::Button(nodes::ButtonEvent::Activated(_))
            | Self::ListItem(nodes::ListItemEvent::Activated(_))
            | Self::ListBox(nodes::ListBoxEvent::Activated(_)) => "activated",
            Self::Textbox(nodes::TextboxEvent::Submitted(_))
            | Self::TextArea(nodes::TextAreaEvent::Submitted(_)) => "submitted",
            Self::Checkbox(nodes::CheckboxEvent::Changed(_))
            | Self::Textbox(nodes::TextboxEvent::Changed(_))
            | Self::TextArea(nodes::TextAreaEvent::Changed(_))
            | Self::Slider(nodes::SliderEvent::Changed(_))
            | Self::Number(nodes::NumberEvent::Changed(_))
            | Self::Scrollbar(nodes::ScrollbarEvent::Changed(_))
            | Self::Split(nodes::SplitViewEvent::Changed(_))
            | Self::Combo(IChoiceMenuEvent::Changed(_)) => "changed",
        }
    }

}

/// One queued child event belongs to the form that supplied its sole observer.
pub(crate) struct Message {
    /// Stable authored child key, captured by the construction-time mapper.
    pub source: String,
    /// Complete typed event, including the text editor's variant.
    pub event: WidgetEvent,
}

/// Form-level application emitter. Its observer is never passed to a child widget.
/// Application binding changes only this forwarding capability after construction.
pub(crate) enum PublicEmitter {
    /// Button output forwarded by the form.
    Button(Option<ComPtr<nodes::ButtonObserverAbi>>),
    /// Checkbox output forwarded by the form.
    Checkbox(Option<ComPtr<nodes::CheckboxObserverAbi>>),
    /// Whole single-line editor event enum forwarded by the form.
    Textbox(Option<ComPtr<nodes::TextboxObserverAbi>>),
    /// Whole multiline editor event enum forwarded by the form.
    TextArea(Option<ComPtr<nodes::TextAreaObserverAbi>>),
    /// Slider output forwarded by the form.
    Slider(Option<ComPtr<nodes::SliderObserverAbi>>),
    /// Numeric-entry output forwarded by the form.
    Number(Option<ComPtr<nodes::NumberObserverAbi>>),
    /// List-row output forwarded by the form.
    ListItem(Option<ComPtr<nodes::ListItemObserverAbi>>),
    /// List-box output forwarded by the form.
    ListBox(Option<ComPtr<nodes::ListBoxObserverAbi>>),
    /// Scrollbar output forwarded by the form.
    Scrollbar(Option<ComPtr<nodes::ScrollbarObserverAbi>>),
    /// Split-view output forwarded by the form.
    Split(Option<ComPtr<nodes::SplitViewObserverAbi>>),
    /// Combo output forwarded by the form.
    Combo(Option<ComPtr<IChoiceMenuObserverAbi>>),
}
impl PublicEmitter {
    /// Posts one captured event through the exact matching form-level typed observer.
    fn post(&mut self, event: &WidgetEvent) -> Result<(), NcStatus> {
        match (self, event) {
            (Self::Button(receiver), WidgetEvent::Button(value)) => value.post_to(receiver),
            (Self::Checkbox(receiver), WidgetEvent::Checkbox(value)) => value.post_to(receiver),
            (Self::Textbox(receiver), WidgetEvent::Textbox(value)) => value.post_to_observer(receiver),
            (Self::TextArea(receiver), WidgetEvent::TextArea(value)) => value.post_to_observer(receiver),
            (Self::Slider(receiver), WidgetEvent::Slider(value)) => value.post_to(receiver),
            (Self::Number(receiver), WidgetEvent::Number(value)) => value.post_to(receiver),
            (Self::ListItem(receiver), WidgetEvent::ListItem(value)) => value.post_to(receiver),
            (Self::ListBox(receiver), WidgetEvent::ListBox(value)) => value.post_to(receiver),
            (Self::Scrollbar(receiver), WidgetEvent::Scrollbar(value)) => value.post_to(receiver),
            (Self::Split(receiver), WidgetEvent::Split(value)) => value.post_to(receiver),
            (Self::Combo(receiver), WidgetEvent::Combo(value)) => value.post_to(receiver),
            _ => Err(NC_E_INVALID_ARG),
        }
    }
}

/// The mounted component owns capture-time relays, while application code owns handlers.
pub(crate) struct Wiring {
    /// One form-owned relay for each emitting child, keyed by its authored identity.
    public: BTreeMap<String, PublicEmitter>,
    /// Nonblocking delivery diagnostics for rejected application receivers.
    diagnostics: EventDeliverySender,
}
impl EventModel for Wiring {
    type Message = Message;
    /// Forwards a complete typed event after traversal releases widget borrows.
    fn handle(&mut self, message: Message) -> Result<(), NcStatus> {
        if let Some(public) = self.public.get_mut(&message.source) {
            if let Err(status) = public.post(&message.event) {
                self.diagnostics.report(EventDeliveryDiagnostic {
                    source: message.source,
                    event: message.event.variant().into(),
                    status,
                });
            }
        }
        Ok(())
    }
    /// Drops relay destinations when the loaded subtree retires.
    fn retire(&mut self) -> Result<(), NcStatus> {
        self.public.clear();
        Ok(())
    }
}

/// Creates one event queue with no executable widget-to-widget route graph.
pub(crate) fn prepare(
    public: BTreeMap<String, PublicEmitter>,
    messages: Receiver<Message>,
    diagnostics: EventDeliverySender,
) -> Controller<Wiring> {
    Controller::new(Wiring { public, diagnostics }, messages)
}

#[cfg(test)]
mod event_validation_tests {
    use super::*;

    /// Independently provided events receive the same domain checks as direct observers.
    #[test]
    fn form_events_use_public_widget_domain_checks() {
        let text = nodes::TextboxEvent::Changed(neocogi_ui::TextChanged { text: "é".into(), cursor: 1 });
        assert_eq!(WidgetEvent::Textbox(text.clone()).validate(), neocogi_ui::textbox::validate(&text));
        let number = nodes::SliderEvent::Changed(nodes::NumericChanged { value: f32::NAN });
        assert_eq!(WidgetEvent::Slider(number.clone()).validate(), neocogi_ui::slider::validate(&number));
        assert!(WidgetEvent::Textbox(text).validate().is_err());
        assert!(WidgetEvent::Slider(number).validate().is_err());
    }
}
