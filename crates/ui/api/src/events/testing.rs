//! Owned test applications exercise production controller gating and runtime scheduling.

use neocogi_com::{ComPtr, ComRef, LocalCom, LocalMut, NcStatus};
use crate::{
    IEventOwnerAbi, Size, Ui,
    events::{Controller, EventModel},
};
use std::{
    ops::{Deref, DerefMut},
    sync::mpsc::Receiver,
};

/// A fixture supplies plain owned state and a captureless handler, never a borrowed callback.
pub struct TestApp<State: 'static, Message: 'static> {
    /// Exact typed allocation used for direct assertions and ordinary model edits.
    local: LocalCom<Controller<Model<State, Message>>>,
    /// Runtime projection resolved once and reused for every tested frame.
    owner: ComPtr<IEventOwnerAbi>,
}

/// Concrete fixture behavior; its state and protocol remain private to the test binary.
struct Model<State, Message> {
    /// Authoritative mutable assertions, controls and expected outcomes.
    state: State,
    /// Captureless behavior receives exclusive state after the COM gate succeeds.
    handle: fn(&mut State, Message) -> Result<(), NcStatus>,
}
impl<State: 'static, Message: 'static> EventModel for Model<State, Message> {
    type Message = Message;
    /// Delegates one dequeued value without introducing another borrow boundary.
    fn handle(&mut self, message: Message) -> Result<(), NcStatus> {
        (self.handle)(&mut self.state, message)
    }
}
impl<State: 'static, Message: 'static> TestApp<State, Message> {
    /// Moves the receiver and complete test model into one persistent COM allocation.
    pub fn new(state: State, messages: Receiver<Message>, handle: fn(&mut State, Message) -> Result<(), NcStatus>) -> Self {
        let local = Controller::into_local(Controller::root(Model { state, handle }, messages));
        let owner = local.query().expect("Controller declares IEventOwner");
        Self { local, owner }
    }
    /// Borrows the cached projection without querying or incrementing its reference count.
    pub fn as_interface(&self) -> ComRef<'_, IEventOwnerAbi> {
        self.owner.as_com_ref()
    }
    /// Uses the production eventful update with the same owner throughout the fixture.
    pub fn update(&self, ui: &Ui, size: Size) -> Result<(), NcStatus> {
        ui.update_with_events(size, self.as_interface())
    }
    /// Grants a short assertion/edit guard; callers release it before event dispatch.
    pub fn state(&self) -> impl DerefMut<Target = State> + '_ {
        StateGuard(self.local.try_borrow_mut().expect("test model is idle outside dispatch"))
    }
}

/// Maps a typed local guard to fixture state without an erased pointer or separate cell.
struct StateGuard<'a, State: 'static, Message: 'static>(LocalMut<'a, Controller<Model<State, Message>>>);
impl<State, Message> Deref for StateGuard<'_, State, Message> {
    type Target = State;
    /// The underlying COM lease keeps this exact allocation alive and exclusively borrowed.
    fn deref(&self) -> &State {
        &self.0.model.state
    }
}
impl<State, Message> DerefMut for StateGuard<'_, State, Message> {
    /// Reborrowing the lease cannot overlap another local or foreign mutation.
    fn deref_mut(&mut self) -> &mut State {
        &mut self.0.model.state
    }
}
