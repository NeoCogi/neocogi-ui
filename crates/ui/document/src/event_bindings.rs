//! Application-owned receivers supplied before any authored widget is constructed.
use crate::{CustomWidgetRegistry, Document, WidgetSpec};
use neocogi_com::{ComPtr, IUnknown, NcGuid, NcStatus, NC_E_INVALID_ARG};
use neocogi_com::events::EventSource;
use std::collections::BTreeMap;

/// One construction attempt's receivers, indexed by authored widget key.
///
/// Each entry retains an ordinary COM object implementing one exact `IEventQueue<Object, Event>`.
/// Its payload never becomes untyped: metadata preflight and the concrete factory both
/// check the receiver's COM identity. Missing entries mean permanently unobserved widgets.
/// Receivers are live application state and are never serialized.
#[derive(Default)]
pub struct EventBindings {
    /// Unique keys prevent a second consumer from silently replacing the first.
    receivers: BTreeMap<String, ComPtr<IUnknown>>,
}
impl EventBindings {
    /// Starts an attempt with all widgets unobserved.
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers a shared application queue without allocating a per-widget mapper.
    /// The concrete queue may implement several product queue interfaces. Exact
    /// constructor metadata is checked during preflight before any widget is made.
    pub fn insert_queue<C: EventSource>(&mut self, key: impl Into<String>, queue: neocogi_com::ComRef<'_, C::Queue>) -> Result<(), NcStatus> {
        let key = key.into();
        if key.is_empty() || self.receivers.contains_key(&key) {
            return Err(NC_E_INVALID_ARG);
        }
        self.receivers.insert(key, queue.add_ref().query()?);
        Ok(())
    }

    /// Validates all entries before construction can allocate widgets or popup surfaces.
    pub(crate) fn validate(&self, document: &Document, custom: &CustomWidgetRegistry) -> Result<(), NcStatus> {
        for (key, receiver) in &self.receivers {
            let node = document.root.find(key).ok_or(NC_E_INVALID_ARG)?;
            match &node.widget {
                WidgetSpec::Custom { kind, .. } => {
                    let interface = custom.interface(kind).map_err(|_| NC_E_INVALID_ARG)?;
                    let queue = interface.event_queue.as_ref().ok_or(NC_E_INVALID_ARG)?;
                    receiver.supports_interface_bytes(NcGuid::from_uuid_bytes(queue.iid), &queue.signature)?;
                }
                widget => {
                    let queue = widget.event_stream().ok_or(NC_E_INVALID_ARG)?;
                    // SAFETY: Built-in descriptors are generated static storage, not foreign input.
                    let signature = unsafe { queue.abi.abi_signature.as_bytes() }?.try_into().map_err(|_| NC_E_INVALID_ARG)?;
                    receiver.supports_interface_bytes(queue.abi.interface_id, signature)?;
                }
            }
        }
        Ok(())
    }

    /// Moves the unique receiver into its constructor; the table retains no second owner.
    pub(crate) fn take(&mut self, key: &str) -> Option<ComPtr<IUnknown>> {
        self.receivers.remove(key)
    }
}
