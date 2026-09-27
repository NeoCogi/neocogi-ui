//! One owned model and typed inbox, shared by application and composite controllers.

use std::sync::mpsc::{Receiver, TryRecvError};
use neocogi_com::{ComRef, NcStatus, com_object};
use neocogi_ui_core_contracts::{IEventOwner, IEventOwnerAbi, IEventOwnerVtblFor, IWidgetAbi, UI_E_CLOSED, UI_E_OWNER};

/// Local behavior behind a persistent COM event owner.
///
/// Message enums and model layout stay in the implementing binary. Concrete
/// widget output contracts carry snapshots across COM into this model's inbox.
pub trait EventModel: 'static {
    /// The owner's exact private message protocol; no Send or type erasure is required.
    type Message: 'static;
    /// Handles one owned message with exclusive model access and no active tree borrow.
    fn handle(&mut self, message: Self::Message) -> Result<(), NcStatus>;
    /// Settles terminal outcomes after delivery ends; defaults to ordinary field destruction.
    fn retire(&mut self) -> Result<(), NcStatus> {
        Ok(())
    }
}

/// Irreversible controller lifetime, independent of retained COM reference counts.
enum Phase {
    /// Construction has not committed the controller to its one owning scope.
    Unmounted,
    /// The application host or mounted view authorizes message delivery.
    Mounted,
    /// Delivery ended permanently, including when final cleanup reported an error.
    Retired,
}

#[com_object(interfaces(IEventOwner))]
/// Owns one model directly and consumes its private channel through ordinary mutable access.
///
/// Applications may allocate this object with `into_local`; a composite may
/// embed it as an ordinary field and delegate IEventOwner alongside its public
/// editing interfaces. Both forms use exactly this inbox and lifecycle logic.
pub struct Controller<Model: EventModel> {
    /// Authoritative application/component state, never shared through another allocation.
    pub model: Model,
    /// Sole receiving endpoint; retirement drops queued private work and disconnects senders.
    messages: Option<Receiver<Model::Message>>,
    /// One-way attachment and retirement policy, changed only through mutable access.
    phase: Phase,
}

impl<Model: EventModel> Controller<Model> {
    /// Prepares owned state before publishing any COM projection or mount relationship.
    pub fn new(model: Model, messages: Receiver<Model::Message>) -> Self {
        Self {
            model,
            messages: Some(messages),
            phase: Phase::Unmounted,
        }
    }

    /// Creates the host-owned root, whose lifetime is independent of any one window.
    /// A root is already claimed and therefore cannot also mount as a composite.
    pub fn root(model: Model, messages: Receiver<Model::Message>) -> Self {
        Self {
            model,
            messages: Some(messages),
            phase: Phase::Mounted,
        }
    }
}

impl<Model: EventModel> IEventOwner for Controller<Model> {
    /// Claims the only attachment without draining messages captured during construction.
    fn mount(&mut self, mut view: ComRef<'_, IWidgetAbi>) -> Result<(), NcStatus> {
        match self.phase {
            Phase::Unmounted => {
                // This is the last fallible step. A rejected view leaves the
                // controller unclaimed, and success commits both lifetimes.
                view.mount()?;
                self.phase = Phase::Mounted;
                Ok(())
            }
            Phase::Mounted => Err(UI_E_OWNER),
            Phase::Retired => Err(UI_E_CLOSED),
        }
    }

    /// Receives only after the object's COM or local borrow gate grants exclusive access.
    fn dispatch_next(&mut self) -> Result<bool, NcStatus> {
        match self.phase {
            Phase::Unmounted => return Err(UI_E_OWNER),
            Phase::Retired => return Err(UI_E_CLOSED),
            Phase::Mounted => {}
        }
        let message = match self.messages.as_ref().expect("mounted controller owns its inbox").try_recv() {
            Ok(message) => message,
            Err(TryRecvError::Empty | TryRecvError::Disconnected) => return Ok(false),
        };
        // The receive borrow ended before model behavior may mutate children or
        // post upward. An error consumes this message but preserves successors.
        self.model.handle(message)?;
        Ok(true)
    }

    /// Commits terminal state before user cleanup, preventing duplicate outcomes after errors.
    fn retire(&mut self) -> Result<(), NcStatus> {
        match std::mem::replace(&mut self.phase, Phase::Retired) {
            Phase::Retired => Ok(()),
            Phase::Unmounted | Phase::Mounted => {
                self.messages.take();
                self.model.retire()
            }
        }
    }
}

impl<Model: EventModel> Drop for Controller<Model> {
    /// Final release also disconnects an unpublished or otherwise explicitly unretired owner.
    fn drop(&mut self) {
        // Fallible runtime retirement reports errors before releasing its final
        // reference. A destructor can only guarantee idempotent local cleanup.
        let _ = self.retire();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{rc::Rc, sync::mpsc};

    /// A thread-local model makes attempted delivery and terminal cleanup observable.
    struct Model {
        /// Successfully dequeued messages, including a deliberately failing handler.
        attempted: Vec<u32>,
        /// Number of terminal cleanup attempts, even when cleanup reports failure.
        retirements: usize,
    }

    impl EventModel for Model {
        type Message = Rc<u32>;
        /// Message one fails after a visible mutation, proving there is no implicit rollback.
        fn handle(&mut self, value: Self::Message) -> Result<(), NcStatus> {
            self.attempted.push(*value);
            match *value {
                1 => Err(neocogi_com::NC_E_INVALID_ARG),
                _ => Ok(()),
            }
        }
        /// A cleanup error must never cause a repeated terminal notification attempt.
        fn retire(&mut self) -> Result<(), NcStatus> {
            self.retirements += 1;
            Err(neocogi_com::NC_E_INVALID_ARG)
        }
    }

    /// Busy access never dequeues; a handler error preserves its unattempted successor.
    #[test]
    fn persistent_owner_preserves_messages_across_busy_and_handler_errors() {
        let (send, messages) = mpsc::channel();
        let owner = Controller::into_local(Controller::root(Model { attempted: Vec::new(), retirements: 0 }, messages));
        let mut events = owner.query::<IEventOwnerAbi>().unwrap();
        send.send(Rc::new(1)).unwrap();
        send.send(Rc::new(2)).unwrap();
        {
            let held = owner.try_borrow_mut().unwrap();
            assert_eq!(events.dispatch_next(), Err(neocogi_com::NC_E_BUSY));
            assert!(held.model.attempted.is_empty());
        }
        assert_eq!(events.dispatch_next(), Err(neocogi_com::NC_E_INVALID_ARG));
        assert_eq!(&owner.try_borrow().unwrap().model.attempted, &[1]);
        assert_eq!(events.dispatch_next(), Ok(true));
        assert_eq!(events.dispatch_next(), Ok(false));
        assert_eq!(&owner.try_borrow().unwrap().model.attempted, &[1, 2]);
    }

    /// Retirement disconnects the inbox and remains terminal after a cleanup failure.
    #[test]
    fn retirement_is_irreversible_and_attempted_once() {
        let (send, messages) = mpsc::channel();
        let owner = Controller::into_local(Controller::root(Model { attempted: Vec::new(), retirements: 0 }, messages));
        let mut events = owner.query::<IEventOwnerAbi>().unwrap();
        send.send(Rc::new(2)).unwrap();
        assert_eq!(events.retire(), Err(neocogi_com::NC_E_INVALID_ARG));
        assert!(send.send(Rc::new(3)).is_err());
        assert_eq!(events.dispatch_next(), Err(UI_E_CLOSED));
        assert_eq!(events.retire(), Ok(()));
        let model = owner.try_borrow().unwrap();
        assert!(model.model.attempted.is_empty());
        assert_eq!(model.model.retirements, 1);
    }
}
