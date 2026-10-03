//! Application-owned model and typed inbox, independent of widget attachment.

use std::sync::mpsc::{Receiver, TryRecvError};
use neocogi_com::{NcStatus, com_object};
use neocogi_ui_core_contracts::{ApplicationDispatch, IApplicationDispatcher, IApplicationDispatcherAbi, IApplicationDispatcherVtblFor};

/// Result of applying application-owned pending changes at an empty-inbox boundary.
/// This local model result is mapped to the generated application dispatcher outcome.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ApplyOutcome {
    /// No pending changes were applied. An idle call must not manufacture another unit of work.
    NothingPending,
    /// Pending changes were applied, possibly posting child work without changing geometry.
    /// The runtime revisits private dispatch and layout before routing later input.
    ChangesApplied,
}

/// Application behavior with a concrete, binary-local message protocol.
/// Composites instead own their inbox in the widget and implement optional IWidgetDispatcher.
pub trait EventModel: 'static {
    /// Exact private message type; no Send bound or type erasure is required.
    type Message: 'static;
    /// Applies coalesced model changes when no message is available, including after disconnection.
    /// Runs under the same mutable gate as handle, with no retained traversal borrow. It may run
    /// before the first input and after each input, several times per update; it is not a frame hook.
    /// Report ChangesApplied only after consuming pending work. Geometry/topology edits still
    /// need their ordinary notifications. Native rendering belongs after the completed UI update.
    /// Errors stop the update without undoing changes already committed; the model owns retry state.
    fn apply_pending_changes(&mut self) -> Result<ApplyOutcome, NcStatus> { Ok(ApplyOutcome::NothingPending) }
    /// Handles one message with exclusive model access and no retained traversal borrow.
    fn handle(&mut self, message: Self::Message) -> Result<(), NcStatus>;
}

#[com_object(interfaces(IApplicationDispatcher))]
/// One application model and receiver behind the existing COM borrow gate.
/// The runtime only borrows its dispatcher for an update. No mount, retirement
/// state machine, or reference back from a widget controls its lifetime.
pub struct Controller<Model: EventModel> {
    /// Authoritative application state, directly available through typed LocalCom access.
    pub model: Model,
    /// Sole receiving endpoint; final destruction disconnects and discards queued work.
    messages: Receiver<Model::Message>,
}
impl<Model: EventModel> Controller<Model> {
    /// Transfers both model and inbox to the application-owned allocation.
    pub fn new(model: Model, messages: Receiver<Model::Message>) -> Self {
        Self { model, messages }
    }
}
impl<Model: EventModel> IApplicationDispatcher for Controller<Model> {
    /// Busy COM entry cannot dequeue; handler errors preserve unattempted successors.
    fn dispatch_next(&mut self) -> Result<ApplicationDispatch, NcStatus> {
        let message = match self.messages.try_recv() {
            Ok(message) => message,
            Err(TryRecvError::Empty | TryRecvError::Disconnected) => {
                return self.model.apply_pending_changes().map(|outcome| match outcome {
                    ApplyOutcome::NothingPending => ApplicationDispatch::Idle,
                    ApplyOutcome::ChangesApplied => ApplicationDispatch::PendingChangesApplied,
                });
            }
        };
        self.model.handle(message)?;
        Ok(ApplicationDispatch::MessageHandled)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{rc::Rc, sync::mpsc};

    /// Records consumption of non-Send values through actual COM dispatch calls.
    struct Model(Vec<u32>);
    impl EventModel for Model {
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
        let local = Controller::into_local(Controller::new(Model(Vec::new()), messages));
        let mut events = local.query::<IApplicationDispatcherAbi>().unwrap();
        send.send(Rc::new(1)).unwrap();
        send.send(Rc::new(2)).unwrap();
        {
            let held = local.try_borrow_mut().unwrap();
            assert_eq!(events.dispatch_next(), Err(neocogi_com::NC_E_BUSY));
            assert!(held.model.0.is_empty());
        }
        assert_eq!(events.dispatch_next(), Err(neocogi_com::NC_E_INVALID_ARG));
        assert_eq!(events.dispatch_next(), Ok(ApplicationDispatch::MessageHandled));
        assert_eq!(events.dispatch_next(), Ok(ApplicationDispatch::Idle));
        assert_eq!(&local.try_borrow().unwrap().model.0, &[1, 2]);
        drop(local);
        send.send(Rc::new(3)).unwrap();
        drop(events);
        assert!(send.send(Rc::new(4)).is_err());
    }

    /// One consumed request either applies successfully or reports an error after consumption.
    enum Pending {
        /// No work remains for another visit.
        None,
        /// A model-owned request; the test controls its completion status.
        Apply(Result<(), NcStatus>),
    }

    /// Records the distinction between message handling and empty-inbox application work.
    struct PendingModel {
        /// Order of admitted callbacks, observable through typed local access.
        trace: Vec<&'static str>,
        /// Sole authority for whether the hook has a request to consume.
        pending: Pending,
    }
    impl EventModel for PendingModel {
        type Message = ();
        /// The message schedules work without applying it inside the handler.
        fn handle(&mut self, (): ()) -> Result<(), NcStatus> {
            self.trace.push("handled");
            self.pending = Pending::Apply(Ok(()));
            Ok(())
        }
        /// Consumes the request once; this model deliberately does not retry a failed request.
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
            let local = Controller::into_local(Controller::new(PendingModel { trace: Vec::new(), pending: Pending::None }, messages));
            let mut dispatcher = local.query::<IApplicationDispatcherAbi>().unwrap();
            send.send(()).unwrap();
            if disconnect { drop(send); }
            assert_eq!(dispatcher.dispatch_next(), Ok(ApplicationDispatch::MessageHandled));
            assert_eq!(local.try_borrow().unwrap().model.trace, ["handled"]);
            {
                let held = local.try_borrow_mut().unwrap();
                assert_eq!(dispatcher.dispatch_next(), Err(neocogi_com::NC_E_BUSY));
                assert_eq!(held.model.trace, ["handled"]);
            }
            assert_eq!(dispatcher.dispatch_next(), Ok(ApplicationDispatch::PendingChangesApplied));
            assert_eq!(dispatcher.dispatch_next(), Ok(ApplicationDispatch::Idle));
            assert_eq!(local.try_borrow().unwrap().model.trace, ["handled", "applied", "idle"]);
        }
    }

    /// Failure preserves committed model edits; replay is determined by the model's request state.
    #[test]
    fn pending_change_error_propagates_without_implicit_replay() {
        let (_send, messages) = mpsc::channel();
        let local = Controller::into_local(Controller::new(PendingModel { trace: Vec::new(), pending: Pending::Apply(Err(neocogi_com::NC_E_INVALID_ARG)) }, messages));
        let mut dispatcher = local.query::<IApplicationDispatcherAbi>().unwrap();
        assert_eq!(dispatcher.dispatch_next(), Err(neocogi_com::NC_E_INVALID_ARG));
        assert_eq!(dispatcher.dispatch_next(), Ok(ApplicationDispatch::Idle));
        assert_eq!(local.try_borrow().unwrap().model.trace, ["applied", "idle"]);
    }
}
