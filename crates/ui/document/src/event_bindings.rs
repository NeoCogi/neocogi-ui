//! Application-owned receivers supplied before any authored widget is constructed.
use crate::{CustomWidgetRegistry, Document, WidgetSpec};
use neocogi_com::{ComPtr, NcStatus, NC_E_INVALID_ARG};
use neocogi_com::events::IEventReceiverAbi;
use std::collections::BTreeMap;

/// One construction attempt's receivers, indexed by authored widget key.
///
/// Every entry retains the same common COM receiver contract.
/// Preflight checks that each selected widget declares observable events. Missing entries mean permanently unobserved widgets.
/// Receivers are live application state and are never serialized.
#[derive(Default)]
pub struct EventBindings {
    /// Unique keys prevent a second consumer from silently replacing the first.
    receivers: BTreeMap<String, ComPtr<IEventReceiverAbi>>,
}
impl EventBindings {
    /// Starts an attempt with all widgets unobserved.
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers a shared application queue without allocating a per-widget mapper.
    /// No widget-specific queue projection or Rust type parameter is needed.
    pub fn insert_queue(&mut self, key: impl Into<String>, queue: neocogi_com::ComRef<'_, IEventReceiverAbi>) -> Result<(), NcStatus> {
        let key = key.into();
        if key.is_empty() || self.receivers.contains_key(&key) {
            return Err(NC_E_INVALID_ARG);
        }
        self.receivers.insert(key, queue.add_ref());
        Ok(())
    }

    /// Validates all entries before construction can allocate widgets or popup surfaces.
    pub(crate) fn validate(&self, document: &Document, custom: &CustomWidgetRegistry) -> Result<(), NcStatus> {
        for key in self.receivers.keys() {
            let node = document.root.find(key).ok_or(NC_E_INVALID_ARG)?;
            match &node.widget {
                WidgetSpec::Custom { kind, .. } => {
                    let interface = custom.interface(kind).map_err(|_| NC_E_INVALID_ARG)?;
                    if interface.events.is_empty() {
                        return Err(NC_E_INVALID_ARG);
                    }
                }
                widget => {
                    if widget.event_interfaces().is_empty() {
                        return Err(NC_E_INVALID_ARG);
                    }
                }
            }
        }
        Ok(())
    }

    /// Moves the unique receiver into its constructor; the table retains no second owner.
    pub(crate) fn take(&mut self, key: &str) -> Option<ComPtr<IEventReceiverAbi>> {
        self.receivers.remove(key)
    }
}

impl EventBindings {
    /// Binds every emitting widget in one document to the same receiver.
    /// Custom event declarations come from the validated provider catalog.
    pub fn all(document: &Document, custom: &CustomWidgetRegistry, queue: neocogi_com::ComRef<'_, IEventReceiverAbi>) -> Result<Self, NcStatus> {
        custom.validate_document(document).map_err(|_| NC_E_INVALID_ARG)?;
        let mut bindings = Self::new();
        let mut pending = vec![&document.root];
        while let Some(node) = pending.pop() {
            let emits = match &node.widget {
                WidgetSpec::Custom { kind, .. } => !custom.interface(kind).map_err(|_| NC_E_INVALID_ARG)?.events.is_empty(),
                widget => !widget.event_interfaces().is_empty(),
            };
            if emits {
                bindings.insert_queue(&node.key, queue)?;
            }
            pending.extend(node.children.iter().map(|child| &child.node));
        }
        Ok(bindings)
    }
}

/// One construction attempt's fact; the sole COM event owner stays in this envelope.
/// Obtain its generation from ReloadSlot::reload in the receiving ReloadIds domain.
pub struct DocumentEvent {
    /// Complete slot/attempt identity, checked before inspecting an event.
    pub generation: crate::Generation,
    /// Weak origin and exclusive immutable event owner.
    pub message: neocogi_com::events::EventMessage,
}
impl DocumentEvent {
    /// Returns the event only for the current attempt and the named live widget.
    /// Stale attempts return None before lookup or QueryInterface. Current lookup
    /// errors remain visible; foreign and removed origins are ordinary absence.
    pub fn event_for(&self, slot: &crate::ReloadSlot, key: &str) -> Result<Option<&neocogi_com::events::EventMessage>, NcStatus> {
        if !slot.accepts(self.generation) {
            return Ok(None);
        }
        let neocogi_com::events::EventOrigin::Widget(origin) = &self.message.origin else {
            return Ok(None);
        };
        if !origin.is_alive() {
            return Ok(None);
        }
        let component = slot.component().ok_or(NC_E_INVALID_ARG)?;
        let widget = component.widget::<neocogi_ui::IWidgetAbi>(key)?;
        Ok(origin.same_object(widget.as_interface()).then_some(&self.message))
    }
}

/// Adds the construction generation while preserving the event's sole ownership.
#[neocogi_com::com_object(interfaces(neocogi_com::events::IEventReceiver))]
struct DocumentReceiver {
    /// Token of the one construction attempt this receiver serves.
    generation: crate::Generation,
    /// Disconnection returns the original event to its producer.
    sender: std::sync::mpsc::Sender<DocumentEvent>,
}
impl neocogi_com::events::IEventReceiver for DocumentReceiver {
    /// Captures identity first; a rejected send restores the same COM owner.
    fn post(
        &mut self,
        origin: neocogi_com::NcValueRef<'_, neocogi_com::events::EventOrigin>,
        event: neocogi_com::ComBox<neocogi_com::events::IEventAbi>,
    ) -> Result<(), (NcStatus, neocogi_com::ComBox<neocogi_com::events::IEventAbi>)> {
        let message = neocogi_com::events::EventMessage::capture(origin, event)?;
        self.sender
            .send(DocumentEvent { generation: self.generation, message })
            .map_err(|error| (neocogi_com::NC_E_CLOSED, error.0.message.event))
    }
}

/// Creates the common receiver for a reload attempt, without generating Rust code.
/// Use EventBindings::all or insert_queue to select observed widgets before loading.
/// Drain the inbox after traversal and use DocumentEvent::event_for before downcast.
pub fn document_receiver(generation: crate::Generation, sender: std::sync::mpsc::Sender<DocumentEvent>) -> Result<ComPtr<IEventReceiverAbi>, NcStatus> {
    DocumentReceiver::from_value(DocumentReceiver { generation, sender }).query::<IEventReceiverAbi>()
}
