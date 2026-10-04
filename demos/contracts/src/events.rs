//! Immutable COM event construction. Every result is the sole owner of its captured snapshot.
use crate::*;
use neocogi_com::{ComBox, NcStatus};
use neocogi_com::events::{IEvent, IEventAbi};

#[neocogi_com::com_object(interfaces(crate::ICounterChangedEvent), ownership = exclusive)]
/// Immutable captured fact; this allocation retains no source widget or service.
struct CounterChanged {
    /// Captured value, independent of subsequent producer edits.
    value: u32,
}
impl IEvent for CounterChanged {}
impl ICounterChangedEvent for CounterChanged {
    /// Reads the immutable value snapshot through the public event ABI.
    fn value(&self) -> Result<u32, NcStatus> {
        Ok(self.value)
    }
}
/// Captures counter changed as one exclusive event, without retaining its source.
pub fn counter_changed(value: u32) -> Result<ComBox<IEventAbi>, NcStatus> {
    CounterChanged::into_box(CounterChanged { value })
        .query()
        .map_err(|(status, _unpublished)| status)
}
