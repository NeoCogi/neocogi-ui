//! UI-owned event delivery, immutable failure snapshots and weak source identities.
use crate::{EventBinding, EventOrigin, IEventAbi, IEventReceiverAbi, IEvent, IErrorEvent};
mod message;
pub use message::EventMessage;
mod stream;
pub use stream::EventPort;
mod error;
pub use error::error;

impl EventBinding {
    /// Selects immutable delivery policy before publishing a widget or service.
    ///
    /// An observed binding retains a shared receiver reference. A borrowed
    /// exclusive receiver cannot grant that ownership and returns
    /// `NC_E_INVALID_ARG`, leaving its sole owner alive and unchanged.
    /// `None` produces an unobserved binding without acquiring any reference.
    pub fn from_queue(queue: Option<neocogi_com::ComRef<'_, IEventReceiverAbi>>) -> Result<Self, neocogi_com::NcStatus> {
        Ok(match queue {
            Some(queue) => Self::Observed(queue.try_add_ref()?),
            None => Self::Unobserved,
        })
    }
    /// Moves the sole constructor receiver reference into producer-owned state.
    pub fn into_queue(self) -> Option<neocogi_com::ComPtr<IEventReceiverAbi>> {
        match self {
            Self::Observed(queue) => Some(queue),
            Self::Unobserved => None,
        }
    }
}
