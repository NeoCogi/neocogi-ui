//! Bounded, nonblocking reports for independent form effects.
use crate::InputKind;
use neocogi_com::NcStatus;
use std::sync::{
    Arc, Mutex,
    mpsc::{self, Receiver, SyncSender, TrySendError},
};

/// The failed effect is kept distinct from its source event and destination status.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RouteEffect {
    /// The optional form-level application relay rejected a captured event.
    Public,
    /// One authored visual command was rejected by its current destination.
    Visual {
        /// Stable destination node key from the authored route.
        target: String,
        /// Concrete command contract selected when the form was constructed.
        input: InputKind,
    },
}

/// A typed, nonfatal delivery failure from one independently applied form effect.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RouteDiagnostic {
    /// Stable source node key that produced the captured event.
    pub source: String,
    /// Logical event variant selected by the form's sole child consumer.
    pub event: String,
    /// Public forwarding or the particular visual target that rejected delivery.
    pub effect: RouteEffect,
    /// Exact COM status returned by that destination.
    pub status: NcStatus,
}

/// Reports retained after a bounded channel is drained.
pub struct RouteDiagnosticBatch {
    /// Queued failures in order, followed by the newest overflowed failure if any.
    pub reports: Vec<RouteDiagnostic>,
    /// Intermediate overflowed failures omitted from the report vector.
    pub omitted: usize,
}

/// When a channel fills, one slot retains the newest failure and its total count.
struct Overflow {
    /// Number of failures redirected to this slot since the previous drain.
    count: usize,
    /// Most recent failure, replacing earlier overflowed failures.
    latest: RouteDiagnostic,
}

/// Form-owned nonblocking writer; clones share one overflow record.
#[derive(Clone)]
pub struct RouteDiagnosticSender {
    /// Ordinary bounded, ordered diagnostic queue.
    sender: SyncSender<RouteDiagnostic>,
    /// Synchronizes overflow replacement with receiver draining.
    overflow: Arc<Mutex<Option<Overflow>>>,
}

impl RouteDiagnosticSender {
    /// Sends a failure without blocking UI dispatch or losing the newest failure.
    pub(crate) fn report(&self, diagnostic: RouteDiagnostic) {
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
pub struct RouteDiagnosticReceiver {
    /// Ordinary bounded report queue.
    receiver: Receiver<RouteDiagnostic>,
    /// Shared overflow slot, synchronized with every writer.
    overflow: Arc<Mutex<Option<Overflow>>>,
}

impl RouteDiagnosticReceiver {
    /// Drains one atomic snapshot, preserving the newest failed effect.
    pub fn drain(&self) -> RouteDiagnosticBatch {
        let mut overflow = self.overflow.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        let mut reports = self.receiver.try_iter().collect::<Vec<_>>();
        let omitted = match overflow.take() {
            Some(record) => {
                reports.push(record.latest);
                record.count - 1
            }
            None => 0,
        };
        RouteDiagnosticBatch { reports, omitted }
    }
}

/// Constructs bounded route reports with an overflow slot independent of channel capacity.
pub fn route_diagnostics(capacity: usize) -> (RouteDiagnosticSender, RouteDiagnosticReceiver) {
    let (sender, receiver) = mpsc::sync_channel(capacity);
    let overflow = Arc::new(Mutex::new(None));
    (
        RouteDiagnosticSender { sender, overflow: overflow.clone() },
        RouteDiagnosticReceiver { receiver, overflow },
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use neocogi_com::NC_E_INVALID_ARG;

    /// A full channel still exposes the final failure and an exact omitted count.
    #[test]
    fn overflow_retains_latest_failure_and_recovers_after_drain() {
        let (sender, receiver) = route_diagnostics(2);
        for index in 0..5 {
            sender.report(RouteDiagnostic {
                source: format!("source-{index}"),
                event: "changed".into(),
                effect: RouteEffect::Public,
                status: NC_E_INVALID_ARG,
            });
        }
        let batch = receiver.drain();
        assert_eq!(batch.reports.len(), 3);
        assert_eq!(batch.reports.last().unwrap().source, "source-4");
        assert_eq!(batch.omitted, 2);
        sender.report(RouteDiagnostic {
            source: "next".into(),
            event: "changed".into(),
            effect: RouteEffect::Public,
            status: NC_E_INVALID_ARG,
        });
        let batch = receiver.drain();
        assert_eq!(batch.reports.len(), 1);
        assert_eq!(batch.reports[0].source, "next");
        assert_eq!(batch.omitted, 0);
    }
}
