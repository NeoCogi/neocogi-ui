//! Custom event capabilities use the same root and ownership as built-in events.
use crate::*;
use neocogi_com::{ComBox, NcStatus};
use neocogi_com::events::{EventPayload, IEvent, IEventAbi, downcast_event};
use neocogi_ui_core_contracts::Activation;

#[neocogi_com::com_object(interfaces(crate::ICounterChangedEvent), ownership = exclusive)]
/// Exclusive snapshot from the independently authored counter widget.
struct ChangedCounter {
    /// User-committed count; this value never owns or reads the source widget.
    value: u32,
}
impl IEvent for ChangedCounter {}
impl ICounterChangedEvent for ChangedCounter {
    /// Return the captured count through the concrete event's C ABI getter.
    fn value(&self) -> Result<u32, NcStatus> {
        Ok(self.value)
    }
}
impl EventPayload for CounterEvent {
    /// The custom native enum delegates activation rules to the built-in semantic value.
    fn validate(&self) -> Result<(), NcStatus> {
        match self {
            Self::Activated(value) => value.validate(),
            Self::Changed(_) => Ok(()),
        }
    }
    /// A native producer emits one of two explicit IDL event interfaces.
    fn into_event(self) -> Result<ComBox<IEventAbi>, NcStatus> {
        match self {
            Self::Activated(value) => value.into_event(),
            Self::Changed(value) => ChangedCounter::into_box(ChangedCounter { value: value.value })
                .query()
                .map_err(|(status, _unpublished)| status),
        }
    }
    /// Decode only when a Rust handler requests its native enum convenience.
    fn from_event(event: &ComBox<IEventAbi>) -> Result<Option<Self>, NcStatus> {
        if let Some(value) = Activation::from_event(event)? {
            Ok(Some(Self::Activated(value)))
        } else {
            downcast_event::<ICounterChangedEventAbi>(event)?
                .map(|event| Ok(Self::Changed(CounterChanged { value: event.value()? })))
                .transpose()
        }
    }
}
