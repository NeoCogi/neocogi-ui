//! Name resolution is setup-only; concrete receivers own subsequent typed calls.
use crate::{model::*, contracts::*};
use neocogi_com::{ComPtr, IUnknown, ComInterface, NcMessage, NcStatus, NcCopyFromBufferOffset, NC_E_INVALID_ARG};
use neocogi_ui::*;
use std::collections::BTreeMap;

/// Builds concrete command receivers; retained receivers observe closure after reload.
pub(crate) fn inputs(document: &Document, nodes: &BTreeMap<String, Widget<IWidgetAbi>>) -> Result<BTreeMap<String, ComPtr<IUnknown>>, NcStatus> {
    let mut table = BTreeMap::new();
    for input in &document.inputs {
        let authored = document.root.find(&input.node).ok_or(NC_E_INVALID_ARG)?;
        let source = nodes.get(&input.node).ok_or(NC_E_INVALID_ARG)?.as_interface().add_ref();
        table.insert(input.name.clone(), receiver(authored, source, &input.kind)?);
    }
    Ok(table)
}

/// Resolves one concrete command capability for a public input or a private connection.
pub(crate) fn receiver(authored: &Node, mut source: ComPtr<IWidgetAbi>, input: &InputKind) -> Result<ComPtr<IUnknown>, NcStatus> {
    let kind = &authored.widget;
    let authored_options = authored.style.options.map(WidgetOptions::from_raw).unwrap_or_else(|| kind.default_options());
    let receiver = match input {
        InputKind::Text => text_receiver(source, kind)?,
        InputKind::Checked => {
            let mut source = source.query::<CheckboxAbi>()?;
            SetChecked::__receiver(Box::new(move |value| source.set_checked(value.checked)))?.query()?
        }
        InputKind::Value => match kind {
            WidgetSpec::Slider { .. } => {
                let mut source = source.query::<SliderAbi>()?;
                SetValue::__receiver(Box::new(move |value| source.set_value(value.value)))?.query()?
            }
            WidgetSpec::Number { .. } => {
                let mut source = source.query::<NumberAbi>()?;
                SetValue::__receiver(Box::new(move |value| source.set_value(value.value)))?.query()?
            }
            _ => return Err(NC_E_INVALID_ARG),
        },
        InputKind::Enabled => SetEnabled::__receiver(Box::new(move |value| {
            let old = authored_options;
            let options = if value.enabled {
                WidgetOptions::from_raw(old.raw() & !WidgetOptions::NO_INTERACT.raw())
            } else {
                old | WidgetOptions::NO_INTERACT
            };
            source.set_options(options)
        }))?
        .query()?,
        InputKind::Expanded => {
            let mut source = source.query::<DisclosureAbi>()?;
            SetExpanded::__receiver(Box::new(move |value| source.set_expanded(value.expanded)))?.query()?
        }
        InputKind::Selection => {
            let mut source = source.query::<ComboAbi>()?;
            SetSelection::__receiver(Box::new(move |value| {
                // Preserve the distinction between clearing selection and malformed ordinals.
                let selection = match value.selected {
                    -1 => ChoiceSelection::None,
                    ordinal if ordinal >= 0 => ChoiceSelection::Selected(ordinal as u64),
                    _ => return Err(NC_E_INVALID_ARG),
                };
                source.set_selection(selection.into())
            }))?
            .query()?
        }
        InputKind::Offset => {
            let mut source = source.query::<ScrollbarAbi>()?;
            SetOffset::__receiver(Box::new(move |value| source.set_offset(value.offset)))?.query()?
        }
        InputKind::Position => {
            let mut source = source.query::<SplitViewAbi>()?;
            SetPosition::__receiver(Box::new(move |value| {
                // Read current settings for each command: callers may edit limits separately.
                let mut settings = source.settings()?;
                settings.position = value.position;
                source.set_settings(settings)
            }))?
            .query()?
        }
        InputKind::Choices => {
            let mut source = Widget::from_interface(source.query::<ComboAbi>()?);
            SetChoices::__receiver(Box::new(move |value| {
                let mut previous = 0;
                let mut labels = Vec::new();
                for end in value.ends {
                    let end = end as usize;
                    labels.push(value.text.get(previous..end).ok_or(NC_E_INVALID_ARG)?);
                    previous = end;
                }
                if previous != value.text.len() || value.selected < -1 || value.selected >= labels.len() as i32 {
                    return Err(NC_E_INVALID_ARG);
                }
                source.replace_items(
                    &labels,
                    crate::build::selection(if value.selected < 0 { None } else { Some(value.selected as u32) }),
                )
            }))?
            .query()?
        }
    };
    Ok(receiver)
}

/// Selects each text destination at bind time; delivery never matches widget kinds.
fn text_receiver(source: ComPtr<IWidgetAbi>, kind: &WidgetSpec) -> Result<ComPtr<IUnknown>, NcStatus> {
    macro_rules! text {
        ($ty:ty, $setter:ident) => {{
            let mut source = source.query::<$ty>()?;
            SetText::__receiver(Box::new(move |value| source.$setter(NcCopyFromBufferOffset::new(0, value.text.as_bytes()))))?.query()
        }};
    }
    match kind {
        WidgetSpec::Button { .. } => text!(ButtonAbi, set_label),
        WidgetSpec::Checkbox { .. } => text!(CheckboxAbi, set_label),
        WidgetSpec::Textbox { .. } => text!(TextboxAbi, set_text),
        WidgetSpec::TextArea { .. } => text!(TextAreaAbi, set_text),
        WidgetSpec::TextBlock { .. } => text!(TextBlockAbi, set_text),
        WidgetSpec::ColorSwatch { .. } => text!(ColorSwatchAbi, set_text),
        WidgetSpec::ListItem { .. } => text!(ListItemAbi, set_text),
        WidgetSpec::ListBox { .. } => text!(ListBoxAbi, set_text),
        WidgetSpec::Disclosure { .. } => {
            let mut source = source.query::<DisclosureAbi>()?.header()?;
            SetText::__receiver(Box::new(move |value| source.set_text(NcCopyFromBufferOffset::new(0, value.text.as_bytes()))))?.query()
        }
        _ => Err(NC_E_INVALID_ARG),
    }
}

/// A command's concrete receiver contract, selected statically by application code.
pub trait Command: Sized {
    /// Exact generated COM projection accepted by this command.
    type Receiver: ComInterface;
    /// Posts through that projection without serialization, name lookup, or payload erasure.
    fn post(&self, receiver: &mut ComPtr<Self::Receiver>) -> Result<(), NcStatus>;
}
/// Generates only repetitive statically typed receiver calls, never a generic wire envelope.
macro_rules! command {
    ($message:ty, $receiver:ty) => {
        impl Command for $message {
            type Receiver = $receiver;
            fn post(&self, receiver: &mut ComPtr<Self::Receiver>) -> Result<(), NcStatus> {
                receiver.post(NcMessage::new(self))
            }
        }
    };
}
command!(SetText, __SetTextReceiverAbi);
command!(SetChecked, __SetCheckedReceiverAbi);
command!(SetValue, __SetValueReceiverAbi);
command!(SetEnabled, __SetEnabledReceiverAbi);
command!(SetExpanded, __SetExpandedReceiverAbi);
command!(SetChoices, __SetChoicesReceiverAbi);
command!(SetSelection, __SetSelectionReceiverAbi);
command!(SetOffset, __SetOffsetReceiverAbi);
command!(SetPosition, __SetPositionReceiverAbi);

/// A resolved typed application input; reconnect after replacing its component.
pub struct InputHandle<C: Command> {
    /// Concrete receiver resolved once from the opaque component.
    pub(crate) receiver: ComPtr<C::Receiver>,
}
impl<C: Command> InputHandle<C> {
    /// Applies one command at an ordinary application update boundary.
    pub fn send(&mut self, command: C) -> Result<(), NcStatus> {
        command.post(&mut self.receiver)
    }
}
