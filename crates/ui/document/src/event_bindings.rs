//! Application-owned receivers supplied before any authored widget is constructed.
use crate::{CustomWidgetRegistry, Document, WidgetSpec};
use neocogi_com::{ComPtr, NcStatus, NC_E_INVALID_ARG};
use neocogi_ui::events::IEventReceiverAbi;
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
        let queue = queue.try_add_ref()?;
        self.receivers.insert(key, queue);
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
    pub message: neocogi_ui::events::EventMessage,
}
impl DocumentEvent {
    /// Returns the event only for the current attempt and the named live widget.
    /// Stale attempts return None before lookup or QueryInterface. Current lookup
    /// errors remain visible; foreign and removed origins are ordinary absence.
    pub fn event_for(&self, slot: &crate::ReloadSlot, key: &str) -> Result<Option<&neocogi_ui::events::EventMessage>, NcStatus> {
        if !slot.accepts(self.generation) {
            return Ok(None);
        }
        let neocogi_ui::events::EventOrigin::Widget(origin) = &self.message.origin else {
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
#[neocogi_com::com_object(interfaces(neocogi_ui::events::IEventReceiver))]
struct DocumentReceiver {
    /// Token of the one construction attempt this receiver serves.
    generation: crate::Generation,
    /// Disconnection returns the original event to its producer.
    sender: std::sync::mpsc::Sender<DocumentEvent>,
}
impl neocogi_ui::events::IEventReceiver for DocumentReceiver {
    /// Captures identity first; a rejected send restores the same COM owner.
    fn post(
        &mut self,
        origin: neocogi_com::NcValueRef<'_, neocogi_ui::events::EventOrigin>,
        event: neocogi_com::ComBox<neocogi_ui::events::IEventAbi>,
    ) -> Result<(), (NcStatus, neocogi_com::ComBox<neocogi_ui::events::IEventAbi>)> {
        let message = neocogi_ui::events::EventMessage::capture(origin, event)?;
        self.sender
            .send(DocumentEvent { generation: self.generation, message })
            .map_err(|error| (neocogi_com::NC_E_CLOSED, error.0.message.event))
    }
}

/// Creates the common receiver for a reload attempt, without generating Rust code.
/// Use EventBindings::all or insert_queue to select observed widgets before loading.
/// Drain the inbox after traversal and use DocumentEvent::event_for before querying the event.
pub fn document_receiver(generation: crate::Generation, sender: std::sync::mpsc::Sender<DocumentEvent>) -> Result<ComPtr<IEventReceiverAbi>, NcStatus> {
    DocumentReceiver::from_value(DocumentReceiver { generation, sender }).query::<IEventReceiverAbi>()
}

#[cfg(test)]
mod tests {
    //! Receiver capture executes through the real document consumer and generated ABI.
    use super::*;
    use neocogi_com::{com_object, ComBox, ComRef, NcValueRef};
    use neocogi_ui::events::{EventOrigin, IEventAbi, IEventReceiver};

    #[com_object(::neocogi_ui::events::IEventReceiver, ownership = exclusive)]
    /// A valid borrow cannot grant another shared owner of this receiver.
    struct ExclusiveReceiver;
    impl IEventReceiver for ExclusiveReceiver {
        fn post(&mut self, _: NcValueRef<'_, EventOrigin>, event: ComBox<IEventAbi>) -> Result<(), (NcStatus, ComBox<IEventAbi>)> {
            Err((neocogi_com::NC_E_CLOSED, event))
        }
    }

    use crate::receiver_fixture::*;
    #[com_object(IRetainer)]
    /// The adapter owns the actual public document bindings implementation.
    struct Retainer {
        /// Existing entries must survive a rejected insertion.
        bindings: EventBindings,
    }
    impl IRetainer for Retainer {
        fn install(&mut self, queue: ComRef<'_, neocogi_com::IUnknown>) -> Result<(), NcStatus> {
            let queue = queue.try_add_ref()?.query::<IEventReceiverAbi>()?;
            self.bindings.insert_queue("candidate", queue.as_com_ref())
        }
    }

    /// Rejection preserves keys and sole-owner liveness; shared capture succeeds afterward.
    #[test]
    fn document_binding_adapter_rejects_exclusive_capability_through_com() {
        let exclusive = ExclusiveReceiver::into_box(ExclusiveReceiver).query::<IEventReceiverAbi>().unwrap();
        let observed = exclusive.downgrade();
        let (original, _inbox) = neocogi_ui::events::channel();
        let mut bindings = EventBindings::new();
        bindings.insert_queue("original", original.as_com_ref()).unwrap();
        assert_eq!(bindings.insert_queue("direct", exclusive.as_com_ref()), Err(NC_E_INVALID_ARG));
        assert_eq!(bindings.receivers.len(), 1);
        let local = Retainer::into_local(Retainer { bindings });
        let mut abi = local.query::<IRetainerAbi>().unwrap();
        assert_eq!(abi.install(exclusive.query_ref::<neocogi_com::IUnknown>().unwrap()), Err(NC_E_INVALID_ARG));
        assert!(observed.is_alive());
        {
            let state = local.try_borrow().unwrap();
            assert_eq!(state.bindings.receivers.len(), 1);
            assert!(state.bindings.receivers["original"].as_com_ref().same_object(original.as_com_ref()));
        }
        abi.install(original.query::<neocogi_com::IUnknown>().unwrap().as_com_ref()).unwrap();
        assert_eq!(local.try_borrow().unwrap().bindings.receivers.len(), 2);
        drop(exclusive);
        assert!(!observed.is_alive());
    }

    /// Whole-document convenience obeys the same generic-borrow capture contract.
    #[test]
    fn all_bindings_reject_exclusive_and_retain_shared_receivers() {
        let document = Document::from_json(include_bytes!("../tests/fixtures/shared.ui.json")).unwrap();
        let custom = CustomWidgetRegistry::new();
        custom.validate_document(&document).unwrap();
        let exclusive = ExclusiveReceiver::into_box(ExclusiveReceiver).query::<IEventReceiverAbi>().unwrap();
        assert!(matches!(EventBindings::all(&document, &custom, exclusive.as_com_ref()), Err(NC_E_INVALID_ARG)));
        assert!(exclusive.downgrade().is_alive());
        let (shared, _inbox) = neocogi_ui::events::channel();
        let weak = shared.downgrade().unwrap();
        let bindings = EventBindings::all(&document, &custom, shared.as_com_ref()).unwrap();
        assert!(bindings.receivers.contains_key("first"));
        drop(shared);
        assert!(weak.upgrade().unwrap().is_some());
        drop(bindings);
        assert!(weak.upgrade().unwrap().is_none());
    }
}
