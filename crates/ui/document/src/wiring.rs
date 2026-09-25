//! The form is each child's sole event consumer and handles captured events after traversal.
use crate::{*, contracts::*};
use neocogi_com::{ComPtr, NcStatus, NC_E_INVALID_ARG};
use neocogi_ui::{nodes, ChoiceSelection, IChoiceMenuEvent, IChoiceMenuObserverAbi, Widget, IWidgetAbi};
use neocogi_ui::events::{Controller, EventModel};
use std::{collections::BTreeMap, sync::mpsc::Receiver};

/// Captured fields from the finite set of supported event variants.
/// The sum remains typed across form delivery; no arbitrary runtime value is stored.
pub(crate) enum Payload<'a> {
    /// Complete text at the time of a change or submission.
    Text(&'a str),
    /// Captured checkbox state.
    Boolean(bool),
    /// Captured finite numeric value.
    Number(f32),
    /// Captured offset or divider position.
    Integer(i32),
    /// Captured optional combo identity.
    Selection(&'a ChoiceSelection),
}

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
    /// queuing public forwarding or visual actions.
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

    /// Selects one authored visual-event variant and its captured forwarded field.
    fn route_payload(&self) -> (&'static str, Payload<'_>) {
        match self {
            Self::Button(nodes::ButtonEvent::Activated(value))
            | Self::ListItem(nodes::ListItemEvent::Activated(value))
            | Self::ListBox(nodes::ListBoxEvent::Activated(value)) => ("activated", Payload::Text(&value.label)),
            Self::Checkbox(nodes::CheckboxEvent::Changed(value)) => ("changed", Payload::Boolean(value.checked)),
            Self::Slider(nodes::SliderEvent::Changed(value)) | Self::Number(nodes::NumberEvent::Changed(value)) => ("changed", Payload::Number(value.value)),
            Self::Textbox(nodes::TextboxEvent::Changed(value)) | Self::TextArea(nodes::TextAreaEvent::Changed(value)) => {
                ("changed", Payload::Text(&value.text))
            }
            Self::Textbox(nodes::TextboxEvent::Submitted(value)) | Self::TextArea(nodes::TextAreaEvent::Submitted(value)) => {
                ("submitted", Payload::Text(&value.text))
            }
            Self::Scrollbar(nodes::ScrollbarEvent::Changed(value)) => ("changed", Payload::Integer(value.offset)),
            Self::Split(nodes::SplitViewEvent::Changed(value)) => ("changed", Payload::Integer(value.position)),
            Self::Combo(IChoiceMenuEvent::Changed(value)) => ("changed", Payload::Selection(value)),
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

/// Optional form-level public emitter. Its observer is never passed to a child widget.
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

/// A resolved command holds its exact receiver and optional constant override.
enum Operation {
    /// Prepared SetText delivery without name lookup or interface discovery.
    Text {
        /// Exact destination setter resolved while loading the form.
        receiver: InputHandle<SetText>,
        /// Authored literal, or the event text when absent.
        constant: Option<String>,
    },
    /// Prepared SetChecked delivery without name lookup or interface discovery.
    Checked {
        /// Exact destination setter resolved while loading the form.
        receiver: InputHandle<SetChecked>,
        /// Authored literal, or the event boolean when absent.
        constant: Option<bool>,
    },
    /// Prepared SetValue delivery without name lookup or interface discovery.
    Value {
        /// Exact destination setter resolved while loading the form.
        receiver: InputHandle<SetValue>,
        /// Authored literal, or the event number when absent.
        constant: Option<f32>,
    },
    /// Prepared SetEnabled delivery without name lookup or interface discovery.
    Enabled {
        /// Exact destination setter resolved while loading the form.
        receiver: InputHandle<SetEnabled>,
        /// Authored literal, or the event boolean when absent.
        constant: Option<bool>,
    },
    /// Prepared SetExpanded delivery without name lookup or interface discovery.
    Expanded {
        /// Exact destination setter resolved while loading the form.
        receiver: InputHandle<SetExpanded>,
        /// Authored literal, or the event boolean when absent.
        constant: Option<bool>,
    },
    /// Prepared SetSelection delivery without name lookup or interface discovery.
    Selection {
        /// Exact destination setter resolved while loading the form.
        receiver: InputHandle<SetSelection>,
        /// Authored ordinal, or the event selection when absent.
        constant: Option<i32>,
    },
    /// Prepared SetOffset delivery without name lookup or interface discovery.
    Offset {
        /// Exact destination setter resolved while loading the form.
        receiver: InputHandle<SetOffset>,
        /// Authored literal, or the event offset when absent.
        constant: Option<i32>,
    },
    /// Prepared SetPosition delivery without name lookup or interface discovery.
    Position {
        /// Exact destination setter resolved while loading the form.
        receiver: InputHandle<SetPosition>,
        /// Authored literal, or the event position when absent.
        constant: Option<i32>,
    },
}
impl Operation {
    /// Executes one visual action after traversal releases all widget borrows.
    fn apply(&mut self, payload: &Payload<'_>) -> Result<(), NcStatus> {
        macro_rules! deliver {
            ($receiver:expr, $constant:expr, $variant:ident, $command:ident, $field:ident) => {{
                let value = match ($constant.as_ref(), payload) {
                    (Some(value), _) => *value,
                    (None, Payload::$variant(value)) => *value,
                    _ => return Err(NC_E_INVALID_ARG),
                };
                $receiver.send($command { $field: value })
            }};
        }
        match self {
            Self::Text { receiver, constant } => {
                // The event owns its snapshot throughout dispatch. Only a SetText
                // command needs its own String after that event is consumed.
                let text = match (constant.as_ref(), payload) {
                    (Some(value), _) => value.clone(),
                    (None, Payload::Text(value)) => (*value).to_owned(),
                    _ => return Err(NC_E_INVALID_ARG),
                };
                receiver.send(SetText { text })
            }
            Self::Checked { receiver, constant } => deliver!(receiver, constant, Boolean, SetChecked, checked),
            Self::Value { receiver, constant } => deliver!(receiver, constant, Number, SetValue, value),
            Self::Enabled { receiver, constant } => deliver!(receiver, constant, Boolean, SetEnabled, enabled),
            Self::Expanded { receiver, constant } => deliver!(receiver, constant, Boolean, SetExpanded, expanded),
            Self::Selection { receiver, constant } => {
                let selected = match (*constant, payload) {
                    (Some(value), _) => value,
                    (None, Payload::Selection(ChoiceSelection::None)) => -1,
                    (None, Payload::Selection(ChoiceSelection::Selected(value))) => i32::try_from(*value).map_err(|_| NC_E_INVALID_ARG)?,
                    _ => return Err(NC_E_INVALID_ARG),
                };
                receiver.send(SetSelection { selected })
            }
            Self::Offset { receiver, constant } => deliver!(receiver, constant, Integer, SetOffset, offset),
            Self::Position { receiver, constant } => deliver!(receiver, constant, Integer, SetPosition, position),
        }
    }
}

/// One form-owned action selected by child key and event enum variant.
struct Route {
    /// Stable target key used in typed delivery diagnostics.
    target: String,
    /// Concrete destination command used in typed delivery diagnostics.
    input: InputKind,
    /// Prepared typed target command.
    operation: Operation,
}

/// Mounted form behavior owns public forwarding and private visual actions together.
pub(crate) struct Wiring {
    /// Authored actions grouped by source and variant; each group retains document order.
    routes: BTreeMap<String, BTreeMap<String, Vec<Route>>>,
    /// Public output relays keyed by the child whose event they forward.
    public: BTreeMap<String, PublicEmitter>,
    /// Bounded nonblocking owner channel for failures of independent effects.
    diagnostics: RouteDiagnosticSender,
}
impl EventModel for Wiring {
    type Message = Message;
    /// Attempts all independent effects and reports rejection without aborting the frame.
    /// Other widget mutations and public receivers cannot participate in one rollback,
    /// so a rejected effect never turns an already consumed event into a frame error.
    fn handle(&mut self, message: Message) -> Result<(), NcStatus> {
        let (variant, payload) = message.event.route_payload();
        if let Some(public) = self.public.get_mut(&message.source) {
            if let Err(status) = public.post(&message.event) {
                let _ = self.diagnostics.report(RouteDiagnostic {
                    source: message.source.clone(),
                    event: variant.into(),
                    effect: RouteEffect::Public,
                    status,
                });
            }
        }
        if let Some(routes) = self.routes.get_mut(&message.source).and_then(|events| events.get_mut(variant)) {
            for route in routes {
                if let Err(status) = route.operation.apply(&payload) {
                    // The bounded report sink retains the newest overflow without blocking dispatch.
                    let _ = self.diagnostics.report(RouteDiagnostic {
                        source: message.source.clone(),
                        event: variant.into(),
                        effect: RouteEffect::Visual {
                            target: route.target.clone(),
                            input: route.input.clone(),
                        },
                        status,
                    });
                }
            }
        }
        Ok(())
    }
    /// Release public and private command capabilities when the owning form retires.
    fn retire(&mut self) -> Result<(), NcStatus> {
        self.routes.clear();
        self.public.clear();
        Ok(())
    }
}

/// Resolves every visual target once; child observers have already been supplied by Construction.
pub(crate) fn prepare(
    document: &Document,
    nodes: &BTreeMap<String, Widget<IWidgetAbi>>,
    public: BTreeMap<String, PublicEmitter>,
    messages: Receiver<Message>,
    diagnostics: RouteDiagnosticSender,
) -> Result<Controller<Wiring>, NcStatus> {
    let mut routes: BTreeMap<String, BTreeMap<String, Vec<Route>>> = BTreeMap::new();
    for connection in &document.connections {
        let target = document.root.find(&connection.target).ok_or(NC_E_INVALID_ARG)?;
        let view = nodes.get(&connection.target).ok_or(NC_E_INVALID_ARG)?.as_interface().add_ref();
        let kind = connection.action.input();
        let receiver = crate::bindings::receiver(target, view, &kind)?;
        macro_rules! operation {
            ($variant:ident) => {{
                let constant = match &connection.action {
                    Action::$variant(value) => Some(value.clone()),
                    Action::Forward(_) => None,
                    _ => return Err(NC_E_INVALID_ARG),
                };
                Operation::$variant {
                    receiver: InputHandle { receiver: receiver.query()? },
                    constant,
                }
            }};
        }
        let operation = match kind.clone() {
            InputKind::Text => operation!(Text),
            InputKind::Checked => operation!(Checked),
            InputKind::Value => operation!(Value),
            InputKind::Enabled => operation!(Enabled),
            InputKind::Expanded => operation!(Expanded),
            InputKind::Selection => operation!(Selection),
            InputKind::Offset => operation!(Offset),
            InputKind::Position => operation!(Position),
            InputKind::Choices => return Err(NC_E_INVALID_ARG),
        };
        routes
            .entry(connection.source.clone())
            .or_default()
            .entry(connection.event.clone())
            .or_default()
            .push(Route {
                target: connection.target.clone(),
                input: kind,
                operation,
            });
    }
    Ok(Controller::new(Wiring { routes, public, diagnostics }, messages))
}

#[cfg(test)]
mod event_validation_tests {
    use super::*;

    /// Routing observes the original snapshot; only a forwarded SetText owns a copy.
    #[test]
    fn text_route_payload_borrows_the_captured_event() {
        let event = WidgetEvent::Textbox(nodes::TextboxEvent::Changed(neocogi_ui::TextChanged {
            text: "large event snapshot".repeat(1024),
            cursor: 0,
        }));
        let original = match &event {
            WidgetEvent::Textbox(nodes::TextboxEvent::Changed(value)) => value.text.as_ptr(),
            _ => unreachable!(),
        };
        let (variant, payload) = event.route_payload();
        assert_eq!(variant, "changed");
        match payload {
            Payload::Text(text) => assert_eq!(text.as_ptr(), original),
            _ => panic!("textbox change lost its text payload"),
        }
    }

    /// A malformed custom-provider event has the same result at both consumer boundaries.
    #[test]
    fn form_events_use_public_widget_domain_checks() {
        let text = nodes::TextboxEvent::Changed(neocogi_ui::TextChanged { text: "é".into(), cursor: 1 });
        assert_eq!(WidgetEvent::Textbox(text.clone()).validate(), neocogi_ui::textbox::validate(&text));

        let number = nodes::SliderEvent::Changed(nodes::NumericChanged { value: f32::NAN });
        assert_eq!(WidgetEvent::Slider(number.clone()).validate(), neocogi_ui::slider::validate(&number));

        let scroll = nodes::ScrollbarEvent::Changed(nodes::ScrollbarChanged { offset: -1 });
        assert_eq!(WidgetEvent::Scrollbar(scroll.clone()).validate(), neocogi_ui::scrollbar::validate(&scroll));

        let split = nodes::SplitViewEvent::Changed(nodes::SplitChanged { position: -1 });
        assert_eq!(WidgetEvent::Split(split.clone()).validate(), neocogi_ui::split_view::validate(&split));
        assert!(WidgetEvent::Textbox(text).validate().is_err());
        assert!(WidgetEvent::Slider(number).validate().is_err());
        assert!(WidgetEvent::Scrollbar(scroll).validate().is_err());
        assert!(WidgetEvent::Split(split).validate().is_err());
    }
}
