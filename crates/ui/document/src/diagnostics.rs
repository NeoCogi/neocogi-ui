//! Bounded, nonblocking reports for nonfatal application event delivery.
use neocogi_com::NcStatus;
use std::sync::{
    Arc, Mutex,
    mpsc::{self, Receiver, SyncSender, TrySendError},
};

/// A typed, nonfatal delivery failure from one application event relay.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EventDeliveryDiagnostic {
    /// Stable source node key that produced the captured event.
    pub source: String,
    /// Logical variant captured by the form's sole child observer.
    pub event: String,
    /// Exact COM status returned by that destination.
    pub status: NcStatus,
}

/// Reports retained after a bounded channel is drained.
pub struct EventDeliveryBatch {
    /// Queued failures in order, followed by the newest overflowed failure if any.
    pub reports: Vec<EventDeliveryDiagnostic>,
    /// Intermediate overflowed failures omitted from the report vector.
    pub omitted: usize,
}

/// When a channel fills, one slot retains the newest failure and its total count.
struct Overflow {
    /// Number of failures redirected to this slot since the previous drain.
    count: usize,
    /// Most recent failure, replacing earlier overflowed failures.
    latest: EventDeliveryDiagnostic,
}

/// Form-owned nonblocking writer; clones share one overflow record.
#[derive(Clone)]
pub struct EventDeliverySender {
    /// Ordinary bounded, ordered diagnostic queue.
    sender: SyncSender<EventDeliveryDiagnostic>,
    /// Synchronizes overflow replacement with receiver draining.
    overflow: Arc<Mutex<Option<Overflow>>>,
}

impl EventDeliverySender {
    /// Sends a failure without blocking UI dispatch or losing the newest failure.
    pub(crate) fn report(&self, diagnostic: EventDeliveryDiagnostic) {
        let mut overflow = self.overflow.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        match self.sender.try_send(diagnostic) {
            Ok(()) | Err(TrySendError::Disconnected(_)) => {}
            Err(TrySendError::Full(latest)) => match overflow.as_mut() {
                Some(record) => {
                    record.count += 1;
                    record.latest = latest;
                }
                None => *overflow = Some(Overflow { count: 1, latest }),
            },
        }
    }
}

/// Application-owned reader that drains ordered reports and the newest overflow.
pub struct EventDeliveryReceiver {
    /// Ordinary bounded report queue.
    receiver: Receiver<EventDeliveryDiagnostic>,
    /// Shared overflow slot, synchronized with every writer.
    overflow: Arc<Mutex<Option<Overflow>>>,
}

impl EventDeliveryReceiver {
    /// Drains one atomic snapshot, preserving the newest failed effect.
    pub fn drain(&self) -> EventDeliveryBatch {
        let mut overflow = self.overflow.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        let mut reports = self.receiver.try_iter().collect::<Vec<_>>();
        let omitted = match overflow.take() {
            Some(record) => {
                reports.push(record.latest);
                record.count - 1
            }
            None => 0,
        };
        EventDeliveryBatch { reports, omitted }
    }
}

/// Constructs bounded event-delivery reports with an overflow slot independent of channel capacity.
pub fn event_delivery_diagnostics(capacity: usize) -> (EventDeliverySender, EventDeliveryReceiver) {
    let (sender, receiver) = mpsc::sync_channel(capacity);
    let overflow = Arc::new(Mutex::new(None));
    (
        EventDeliverySender { sender, overflow: overflow.clone() },
        EventDeliveryReceiver { receiver, overflow },
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use neocogi_com::NC_E_INVALID_ARG;

    /// A full channel still exposes the final failure and an exact omitted count.
    #[test]
    fn overflow_retains_latest_failure_and_recovers_after_drain() {
        let (sender, receiver) = event_delivery_diagnostics(2);
        for index in 0..5 {
            sender.report(EventDeliveryDiagnostic {
                source: format!("source-{index}"),
                event: "changed".into(),
                status: NC_E_INVALID_ARG,
            });
        }
        let batch = receiver.drain();
        assert_eq!(batch.reports.len(), 3);
        assert_eq!(batch.reports.last().unwrap().source, "source-4");
        assert_eq!(batch.omitted, 2);
        sender.report(EventDeliveryDiagnostic {
            source: "next".into(),
            event: "changed".into(),
            status: NC_E_INVALID_ARG,
        });
        let batch = receiver.drain();
        assert_eq!(batch.reports.len(), 1);
        assert_eq!(batch.reports[0].source, "next");
        assert_eq!(batch.omitted, 0);
    }
}
