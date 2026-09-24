//! The form is each child's sole event consumer and handles captured events after traversal.
use crate::{*, contracts::*};
use neocogi_com::{ComPtr, NcStatus, NC_E_INVALID_ARG};
use neocogi_ui::{nodes, ChoiceSelection, IChoiceMenuEvent, IChoiceMenuObserverAbi, Widget, IWidgetAbi};
use neocogi_ui::events::{Controller, EventModel};
use std::{collections::BTreeMap, sync::mpsc::Receiver};

/// Captured fields from the finite set of supported event variants.
/// The sum remains typed across form delivery; no arbitrary runtime value is stored.
#[derive(Clone)]
pub(crate) enum Payload {
    /// An activation carries no forwarded value.
    Activation,
    /// Complete text at the time of a change or submission.
    Text(String),
    /// Captured checkbox state.
    Boolean(bool),
    /// Captured finite numeric value.
    Number(f32),
    /// Captured offset or divider position.
    Integer(i32),
    /// Captured optional combo identity.
    Selection(ChoiceSelection),
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
    /// Selects one authored visual-event variant and its captured forwarded field.
    fn route_payload(&self) -> (&'static str, Payload) {
        match self {
            Self::Button(nodes::ButtonEvent::Activated(_))
            | Self::ListItem(nodes::ListItemEvent::Activated(_))
            | Self::ListBox(nodes::ListBoxEvent::Activated(_)) => ("activated", Payload::Activation),
            Self::Checkbox(nodes::CheckboxEvent::Changed(value)) => ("changed", Payload::Boolean(value.checked)),
            Self::Slider(nodes::SliderEvent::Changed(value)) | Self::Number(nodes::NumberEvent::Changed(value)) => {
                ("changed", Payload::Number(value.value))
            }
            Self::Textbox(nodes::TextboxEvent::Changed(value)) | Self::TextArea(nodes::TextAreaEvent::Changed(value)) => {
                ("changed", Payload::Text(value.text.clone()))
            }
            Self::Textbox(nodes::TextboxEvent::Submitted(value)) | Self::TextArea(nodes::TextAreaEvent::Submitted(value)) => {
                ("submitted", Payload::Text(value.text.clone()))
            }
            Self::Scrollbar(nodes::ScrollbarEvent::Changed(value)) => ("changed", Payload::Integer(value.offset)),
            Self::Split(nodes::SplitViewEvent::Changed(value)) => ("changed", Payload::Integer(value.position)),
            Self::Combo(IChoiceMenuEvent::Changed(value)) => ("changed", Payload::Selection(value.clone())),
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
    fn apply(&mut self, payload: Payload) -> Result<(), NcStatus> {
        macro_rules! deliver {
            ($receiver:expr, $constant:expr, $variant:ident, $command:ident, $field:ident) => {{
                let value = match ($constant.clone(), payload) {
                    (Some(value), _) => value,
                    (None, Payload::$variant(value)) => value,
                    _ => return Err(NC_E_INVALID_ARG),
                };
                $receiver.send($command { $field: value })
            }};
        }
        match self {
            Self::Text { receiver, constant } => deliver!(receiver, constant, Text, SetText, text),
            Self::Checked { receiver, constant } => deliver!(receiver, constant, Boolean, SetChecked, checked),
            Self::Value { receiver, constant } => deliver!(receiver, constant, Number, SetValue, value),
            Self::Enabled { receiver, constant } => deliver!(receiver, constant, Boolean, SetEnabled, enabled),
            Self::Expanded { receiver, constant } => deliver!(receiver, constant, Boolean, SetExpanded, expanded),
            Self::Selection { receiver, constant } => {
                let selected = match (*constant, payload) {
                    (Some(value), _) => value,
                    (None, Payload::Selection(ChoiceSelection::None)) => -1,
                    (None, Payload::Selection(ChoiceSelection::Selected(value))) => i32::try_from(value).map_err(|_| NC_E_INVALID_ARG)?,
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
    /// Stable source widget key.
    source: String,
    /// Logical event variant, rather than a second widget observer port.
    variant: String,
    /// Prepared typed target command.
    operation: Operation,
}

/// Mounted form behavior owns public forwarding and private visual actions together.
pub(crate) struct Wiring {
    /// Ordered authored actions, selected after one child event reaches this form.
    routes: Vec<Route>,
    /// Public output relays keyed by the child whose event they forward.
    public: BTreeMap<String, PublicEmitter>,
}
impl EventModel for Wiring {
    type Message = Message;
    /// Handles each child event exactly once, then attempts its independent form effects.
    fn handle(&mut self, message: Message) -> Result<(), NcStatus> {
        let mut first_error = None;
        if let Some(public) = self.public.get_mut(&message.source) {
            if let Err(error) = public.post(&message.event) {
                first_error.get_or_insert(error);
            }
        }
        let (variant, payload) = message.event.route_payload();
        for route in self.routes.iter_mut().filter(|route| route.source == message.source && route.variant == variant) {
            if let Err(error) = route.operation.apply(payload.clone()) {
                first_error.get_or_insert(error);
            }
        }
        first_error.map_or(Ok(()), Err)
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
) -> Result<Controller<Wiring>, NcStatus> {
    let mut routes = Vec::with_capacity(document.connections.len());
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
                Operation::$variant { receiver: InputHandle { receiver: receiver.query()? }, constant }
            }};
        }
        let operation = match kind {
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
        routes.push(Route { source: connection.source.clone(), variant: connection.event.clone(), operation });
    }
    Ok(Controller::new(Wiring { routes, public }, messages))
}
