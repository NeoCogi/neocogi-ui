//! Prepared private connections queue captured facts and apply concrete commands after traversal.
use crate::{*, contracts::*};
use neocogi_com::{ComPtr, NcStatus, NC_E_INVALID_ARG};
use neocogi_com::events::{bind_output, INamedOutputsAbi};
use neocogi_ui::{
    events::{Controller, EventModel, *},
    Widget, IWidgetAbi, ChoiceSelection,
};
use std::{collections::BTreeMap, sync::mpsc};

/// Captured fields from the finite set of supported output contracts.
/// This private sum remains typed and never holds arbitrary runtime values.
pub(crate) enum Payload {
    /// An activation carries no value; only constant actions accept it.
    Activation,
    /// Captured complete text.
    Text(String),
    /// Captured checkbox state.
    Boolean(bool),
    /// Captured finite numeric value.
    Number(f32),
    /// Captured pixel offset or position.
    Integer(i32),
    /// Captured optional combo identity, validated against ordinal commands at dispatch.
    Selection(ChoiceSelection),
}
/// One queued fact belongs to the controller whose construction allocated its route index.
pub(crate) struct Message {
    /// Setup-only index; it is never read from document bytes.
    route: usize,
    /// Owned snapshot captured before traversal continues.
    payload: Payload,
}
/// A resolved command holds its exact receiver and optional constant override.
enum Operation {
    /// Prepared SetText delivery without name lookup or interface discovery.
    Text {
        /// Concrete receiver resolved once during candidate preparation.
        receiver: InputHandle<SetText>,
        /// None forwards the compatible event field; Some replaces it explicitly.
        constant: Option<String>,
    },
    /// Prepared SetChecked delivery without name lookup or interface discovery.
    Checked {
        /// Concrete receiver resolved once during candidate preparation.
        receiver: InputHandle<SetChecked>,
        /// None forwards the compatible event field; Some replaces it explicitly.
        constant: Option<bool>,
    },
    /// Prepared SetValue delivery without name lookup or interface discovery.
    Value {
        /// Concrete receiver resolved once during candidate preparation.
        receiver: InputHandle<SetValue>,
        /// None forwards the compatible event field; Some replaces it explicitly.
        constant: Option<f32>,
    },
    /// Prepared SetEnabled delivery without name lookup or interface discovery.
    Enabled {
        /// Concrete receiver resolved once during candidate preparation.
        receiver: InputHandle<SetEnabled>,
        /// None forwards the compatible event field; Some replaces it explicitly.
        constant: Option<bool>,
    },
    /// Prepared SetExpanded delivery without name lookup or interface discovery.
    Expanded {
        /// Concrete receiver resolved once during candidate preparation.
        receiver: InputHandle<SetExpanded>,
        /// None forwards the compatible event field; Some replaces it explicitly.
        constant: Option<bool>,
    },
    /// Prepared SetSelection delivery without name lookup or interface discovery.
    Selection {
        /// Concrete receiver resolved once during candidate preparation.
        receiver: InputHandle<SetSelection>,
        /// None forwards the compatible event field; Some replaces it explicitly.
        constant: Option<i32>,
    },
    /// Prepared SetOffset delivery without name lookup or interface discovery.
    Offset {
        /// Concrete receiver resolved once during candidate preparation.
        receiver: InputHandle<SetOffset>,
        /// None forwards the compatible event field; Some replaces it explicitly.
        constant: Option<i32>,
    },
    /// Prepared SetPosition delivery without name lookup or interface discovery.
    Position {
        /// Concrete receiver resolved once during candidate preparation.
        receiver: InputHandle<SetPosition>,
        /// None forwards the compatible event field; Some replaces it explicitly.
        constant: Option<i32>,
    },
}
/// Mounted component behavior owns only concrete prepared routes.
pub(crate) struct Wiring {
    /// Route order matches the private channel's setup-only indices.
    routes: Vec<Operation>,
}
impl EventModel for Wiring {
    type Message = Message;
    /// Runs only after traversal releases widget borrows; setters remain silent.
    fn handle(&mut self, message: Message) -> Result<(), NcStatus> {
        let operation = self.routes.get_mut(message.route).ok_or(NC_E_INVALID_ARG)?;
        // Each arm statically selects its command and rejects an impossible payload mismatch.
        macro_rules! deliver {
            ($receiver:expr, $constant:expr, $variant:ident, $command:ident, $field:ident) => {{
                let value = match ($constant.clone(), message.payload) {
                    (Some(value), _) => value,
                    (None, Payload::$variant(value)) => value,
                    _ => return Err(NC_E_INVALID_ARG),
                };
                $receiver.send($command { $field: value })
            }};
        }
        match operation {
            Operation::Text { receiver, constant } => deliver!(receiver, constant, Text, SetText, text),
            Operation::Checked { receiver, constant } => deliver!(receiver, constant, Boolean, SetChecked, checked),
            Operation::Value { receiver, constant } => deliver!(receiver, constant, Number, SetValue, value),
            Operation::Enabled { receiver, constant } => deliver!(receiver, constant, Boolean, SetEnabled, enabled),
            Operation::Expanded { receiver, constant } => deliver!(receiver, constant, Boolean, SetExpanded, expanded),
            Operation::Selection { receiver, constant } => {
                let selected = match (*constant, message.payload) {
                    (Some(value), _) => value,
                    (None, Payload::Selection(ChoiceSelection::None)) => -1,
                    (None, Payload::Selection(ChoiceSelection::Selected(value))) => i32::try_from(value).map_err(|_| NC_E_INVALID_ARG)?,
                    _ => return Err(NC_E_INVALID_ARG),
                };
                receiver.send(SetSelection { selected })
            }
            Operation::Offset { receiver, constant } => deliver!(receiver, constant, Integer, SetOffset, offset),
            Operation::Position { receiver, constant } => deliver!(receiver, constant, Integer, SetPosition, position),
        }
    }
    /// Release command capabilities when the owning subtree retires, even if COM handles remain.
    fn retire(&mut self) -> Result<(), NcStatus> {
        self.routes.clear();
        Ok(())
    }
}

/// Resolves command projections and installs output-to-channel mappings before publication.
pub(crate) fn prepare(
    document: &Document,
    nodes: &BTreeMap<String, Widget<IWidgetAbi>>,
    outputs: &BTreeMap<String, ComPtr<INamedOutputsAbi>>,
) -> Result<Controller<Wiring>, NcStatus> {
    let (sender, messages) = mpsc::channel();
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
                Operation::$variant {
                    receiver: InputHandle { receiver: receiver.query()? },
                    constant,
                }
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
        let route = routes.len();
        let source = document.root.find(&connection.source).ok_or(NC_E_INVALID_ARG)?;
        let output = outputs.get(&connection.source).ok_or(NC_E_INVALID_ARG)?;
        // These mappings only capture facts. They never call a destination while a source is borrowed.
        macro_rules! bind {
            ($event:ty, $convert:expr) => {
                bind_output(output.as_com_ref(), &connection.port, sender.clone(), move |value: $event| Message {
                    route,
                    payload: ($convert)(value),
                })?
            };
        }
        match source.widget.output_type(&connection.port).ok_or(NC_E_INVALID_ARG)? {
            OutputType::Activation => bind!(Activation, |_: Activation| Payload::Activation),
            OutputType::Checked => bind!(CheckboxChanged, |value: CheckboxChanged| Payload::Boolean(value.checked)),
            OutputType::Number => bind!(NumericChanged, |value: NumericChanged| Payload::Number(value.value)),
            OutputType::TextChanged => bind!(TextChanged, |value: TextChanged| Payload::Text(value.text)),
            OutputType::TextSubmitted => bind!(TextSubmitted, |value: TextSubmitted| Payload::Text(value.text)),
            OutputType::Offset => bind!(ScrollbarChanged, |value: ScrollbarChanged| Payload::Integer(value.offset)),
            OutputType::Position => bind!(SplitChanged, |value: SplitChanged| Payload::Integer(value.position)),
            OutputType::Selection => bind!(ChoiceSelection, Payload::Selection),
        }
        routes.push(operation);
    }
    Ok(Controller::new(Wiring { routes }, messages))
}
