//! Application-owned model and typed inbox, independent of widget attachment.

use std::sync::mpsc::{Receiver, TryRecvError};
use neocogi_com::{NcStatus, com_object};
use neocogi_ui_core_contracts::{IApplicationDispatcher, IApplicationDispatcherAbi, IApplicationDispatcherVtblFor};

/// Application behavior with a concrete, binary-local message protocol.
/// Composites instead own their inbox in the widget and implement IWidget dispatch.
pub trait EventModel: 'static {
    /// Exact private message type; no Send bound or type erasure is required.
    type Message: 'static;
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
    fn dispatch_next(&mut self) -> Result<bool, NcStatus> {
        let message = match self.messages.try_recv() {
            Ok(message) => message,
            Err(TryRecvError::Empty | TryRecvError::Disconnected) => return Ok(false),
        };
        self.model.handle(message)?;
        Ok(true)
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
        assert_eq!(events.dispatch_next(), Ok(true));
        assert_eq!(events.dispatch_next(), Ok(false));
        assert_eq!(&local.try_borrow().unwrap().model.0, &[1, 2]);
        drop(local);
        send.send(Rc::new(3)).unwrap();
        drop(events);
        assert!(send.send(Rc::new(4)).is_err());
    }
}
