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
                    interface.event_queue.as_ref().ok_or(NC_E_INVALID_ARG)?;
                }
                widget => {
                    widget.event_stream().ok_or(NC_E_INVALID_ARG)?;
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
