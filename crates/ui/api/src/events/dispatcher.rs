//! Owned message processor and typed inbox, independent of widget attachment.

use std::sync::mpsc::{Receiver, TryRecvError};
use neocogi_com::{NcStatus, com_object};
use neocogi_ui_core_contracts::{ApplicationDispatch, InputBoundary, IApplicationDispatcher, IApplicationDispatcherAbi, IApplicationDispatcherVtblFor};

/// Result of applying application-owned pending changes at an empty-inbox boundary.
/// This local processor result is mapped to the generated application dispatcher outcome.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ApplyOutcome {
    /// No pending changes were applied. An idle call must not manufacture another unit of work.
    NothingPending,
    /// Pending changes were applied, possibly posting child work without changing geometry.
    /// The runtime revisits private dispatch and layout before routing later input.
    ChangesApplied,
}

/// Message-processing behavior with ordinary owned state and a concrete, binary-local protocol.
/// Dispatcher owns the processor and inbox, and the UI runtime schedules synchronous delivery.
/// Behavioral composites own their inbox in the widget and implement optional IWidgetDispatcher.
pub trait MessageProcessor: 'static {
    /// Exact private message type; no Send bound or type erasure is required.
    type Message: 'static;
    /// Applies coalesced state changes when no message is available, including after disconnection.
    /// Runs under the same mutable gate as handle, with no retained traversal borrow. It may run
    /// before the first input and after each input, several times per update; it is not a frame hook.
    /// Report ChangesApplied only after consuming pending work. Geometry/topology edits still
    /// need their ordinary notifications. Native rendering belongs after the completed UI update.
    /// Errors stop the update without undoing changes already committed; the processor owns retry state.
    fn apply_pending_changes(&mut self) -> Result<ApplyOutcome, NcStatus> {
        Ok(ApplyOutcome::NothingPending)
    }
    /// Records the exact consumed input scope before routing and after its messages settle.
    /// Runs outside traversal under the processor gate. Keep UI edits in handle/pending changes.
    /// End is attempted on errors too; queued snapshots can retain their original input identity.
    fn input_boundary(&mut self, _boundary: InputBoundary) -> Result<(), NcStatus> {
        Ok(())
    }
    /// Handles one message with exclusive processor access and no retained traversal borrow.
    fn handle(&mut self, message: Self::Message) -> Result<(), NcStatus>;
}

#[com_object(interfaces(IApplicationDispatcher))]
/// One owned message processor and receiver behind the existing COM borrow gate.
/// The runtime only borrows its dispatcher for an update. No mount, retirement
/// state machine, or reference back from a widget controls its lifetime.
pub struct Dispatcher<Processor: MessageProcessor> {
    /// Authoritative processing state and behavior, directly available through typed LocalCom access.
    pub processor: Processor,
    /// Sole receiving endpoint; final destruction disconnects and discards queued work.
    messages: Receiver<Processor::Message>,
}
impl<Processor: MessageProcessor> Dispatcher<Processor> {
    /// Transfers both processor and inbox to the application-owned allocation.
    pub fn new(processor: Processor, messages: Receiver<Processor::Message>) -> Self {
        Self { processor, messages }
    }
}
impl<Processor: MessageProcessor> IApplicationDispatcher for Dispatcher<Processor> {
    /// The same local/foreign gate protects bookkeeping and message handling.
    fn input_boundary(&mut self, boundary: InputBoundary) -> Result<(), NcStatus> {
        self.processor.input_boundary(boundary)
    }
    /// Busy COM entry cannot dequeue; handler errors preserve unattempted successors.
    fn dispatch_next(&mut self) -> Result<ApplicationDispatch, NcStatus> {
        let message = match self.messages.try_recv() {
            Ok(message) => message,
            Err(TryRecvError::Empty | TryRecvError::Disconnected) => {
                return self.processor.apply_pending_changes().map(|outcome| match outcome {
                    ApplyOutcome::NothingPending => ApplicationDispatch::Idle,
                    ApplyOutcome::ChangesApplied => ApplicationDispatch::PendingChangesApplied,
                });
            }
        };
        self.processor.handle(message)?;
        Ok(ApplicationDispatch::MessageHandled)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{rc::Rc, sync::mpsc};

    /// Records consumption of non-Send values through actual COM dispatch calls.
    struct Processor(Vec<u32>);
    impl MessageProcessor for Processor {
        type Message = Rc<u32>;
        /// Message one fails after mutation, without consuming its successor.
        fn handle(&mut self, value: Self::Message) -> Result<(), NcStatus> {
            self.0.push(*value);
            match *value {
                1 => Err(neocogi_com::NC_E_INVALID_ARG),
                _ => Ok(()),
            }
        }
    }

    /// Application lifetime needs no second state machine to preserve borrow/error semantics.
    #[test]
    fn application_dispatch_preserves_messages_and_disconnects_on_final_release() {
        let (send, messages) = mpsc::channel();
        let local = Dispatcher::into_local(Dispatcher::new(Processor(Vec::new()), messages));
        let mut events = local.query::<IApplicationDispatcherAbi>().unwrap();
        send.send(Rc::new(1)).unwrap();
        send.send(Rc::new(2)).unwrap();
        {
            let held = local.try_borrow_mut().unwrap();
            assert_eq!(events.dispatch_next(), Err(neocogi_com::NC_E_BUSY));
            assert_eq!(events.input_boundary(InputBoundary::End), Err(neocogi_com::NC_E_BUSY));
            assert!(held.processor.0.is_empty());
        }
        assert_eq!(events.input_boundary(InputBoundary::End), Ok(()));
        assert_eq!(events.dispatch_next(), Err(neocogi_com::NC_E_INVALID_ARG));
        assert_eq!(events.dispatch_next(), Ok(ApplicationDispatch::MessageHandled));
        assert_eq!(events.dispatch_next(), Ok(ApplicationDispatch::Idle));
        assert_eq!(&local.try_borrow().unwrap().processor.0, &[1, 2]);
        drop(local);
        send.send(Rc::new(3)).unwrap();
        drop(events);
        assert!(send.send(Rc::new(4)).is_err());
    }

    /// One consumed request either applies successfully or reports an error after consumption.
    enum Pending {
        /// No work remains for another visit.
        None,
        /// A processor-owned request; the test controls its completion status.
        Apply(Result<(), NcStatus>),
    }

    /// Records the distinction between message handling and empty-inbox application work.
    struct PendingProcessor {
        /// Order of admitted callbacks, observable through typed local access.
        trace: Vec<&'static str>,
        /// Sole authority for whether the hook has a request to consume.
        pending: Pending,
    }
    impl MessageProcessor for PendingProcessor {
        type Message = ();
        /// The message schedules work without applying it inside the handler.
        fn handle(&mut self, (): ()) -> Result<(), NcStatus> {
            self.trace.push("handled");
            self.pending = Pending::Apply(Ok(()));
            Ok(())
        }
        /// Consumes the request once; this processor deliberately does not retry a failed request.
        fn apply_pending_changes(&mut self) -> Result<ApplyOutcome, NcStatus> {
            match core::mem::replace(&mut self.pending, Pending::None) {
                Pending::None => {
                    self.trace.push("idle");
                    Ok(ApplyOutcome::NothingPending)
                }
                Pending::Apply(result) => {
                    self.trace.push("applied");
                    result.map(|()| ApplyOutcome::ChangesApplied)
                }
            }
        }
    }

    /// All three generated ABI outcomes survive admission, both with and without a live sender.
    #[test]
    fn pending_changes_follow_messages_and_share_the_same_mutable_gate() {
        for disconnect in [false, true] {
            let (send, messages) = mpsc::channel();
            let local = Dispatcher::into_local(Dispatcher::new(PendingProcessor { trace: Vec::new(), pending: Pending::None }, messages));
            let mut dispatcher = local.query::<IApplicationDispatcherAbi>().unwrap();
            send.send(()).unwrap();
            if disconnect { drop(send); }
            assert_eq!(dispatcher.dispatch_next(), Ok(ApplicationDispatch::MessageHandled));
            assert_eq!(local.try_borrow().unwrap().processor.trace, ["handled"]);
            {
                let held = local.try_borrow_mut().unwrap();
                assert_eq!(dispatcher.dispatch_next(), Err(neocogi_com::NC_E_BUSY));
                assert_eq!(held.processor.trace, ["handled"]);
            }
            assert_eq!(dispatcher.dispatch_next(), Ok(ApplicationDispatch::PendingChangesApplied));
            assert_eq!(dispatcher.dispatch_next(), Ok(ApplicationDispatch::Idle));
            assert_eq!(local.try_borrow().unwrap().processor.trace, ["handled", "applied", "idle"]);
        }
    }

    /// Failure preserves committed processor edits; replay is determined by the processor's request state.
    #[test]
    fn pending_change_error_propagates_without_implicit_replay() {
        let (_send, messages) = mpsc::channel();
        let local = Dispatcher::into_local(Dispatcher::new(PendingProcessor { trace: Vec::new(), pending: Pending::Apply(Err(neocogi_com::NC_E_INVALID_ARG)) }, messages));
        let mut dispatcher = local.query::<IApplicationDispatcherAbi>().unwrap();
        assert_eq!(dispatcher.dispatch_next(), Err(neocogi_com::NC_E_INVALID_ARG));
        assert_eq!(dispatcher.dispatch_next(), Ok(ApplicationDispatch::Idle));
        assert_eq!(local.try_borrow().unwrap().processor.trace, ["applied", "idle"]);
    }
}
