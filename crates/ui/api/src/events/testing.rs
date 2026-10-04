//! Owned test applications exercise production dispatcher gating and runtime scheduling.

use neocogi_com::{ComPtr, ComRef, LocalCom, LocalMut, NcStatus};
use crate::{
    IApplicationDispatcherAbi, Size, Ui,
    events::{ApplyOutcome, Dispatcher, MessageProcessor},
};
use std::{
    ops::{Deref, DerefMut},
    sync::mpsc::Receiver,
};

/// A fixture supplies plain owned state and a captureless handler, never a borrowed callback.
pub struct TestApp<State: 'static, Message: 'static, Context: 'static = ()> {
    /// Exact typed allocation used for direct assertions and ordinary state edits.
    local: LocalCom<Dispatcher<Processor<State, Message, Context>>>,
    /// Runtime projection resolved once and reused for every tested frame.
    owner: ComPtr<IApplicationDispatcherAbi>,
}

/// Concrete fixture behavior; its state and protocol remain private to the test binary.
struct Processor<State, Message, Context> {
    /// Authoritative mutable assertions, controls and expected outcomes.
    state: State,
    /// Captureless behavior receives exclusive state after the COM gate succeeds.
    handle: fn(&mut State, Message) -> Result<(), NcStatus>,
    /// Immutable concrete services used by the empty-inbox hook.
    context: Context,
    /// Captureless hook exercises production deferred publication under dispatcher admission.
    apply_pending_changes: fn(&mut State, &Context) -> Result<ApplyOutcome, NcStatus>,
}
impl<State: 'static, Message: 'static, Context: 'static> MessageProcessor for Processor<State, Message, Context> {
    type Message = Message;
    /// Applies fixture-owned pending work under the production dispatcher gate.
    fn apply_pending_changes(&mut self) -> Result<ApplyOutcome, NcStatus> {
        (self.apply_pending_changes)(&mut self.state, &self.context)
    }
    /// Delegates one dequeued value without introducing another borrow boundary.
    fn handle(&mut self, message: Message) -> Result<(), NcStatus> {
        (self.handle)(&mut self.state, message)
    }
}
impl<State: 'static, Message: 'static> TestApp<State, Message> {
    /// Moves the receiver and test state into one production dispatcher.
    pub fn new(state: State, messages: Receiver<Message>, handle: fn(&mut State, Message) -> Result<(), NcStatus>) -> Self {
        Self::with_context(state, messages, handle, (), |_, _| Ok(ApplyOutcome::NothingPending))
    }
}
impl<State: 'static, Message: 'static, Context: 'static> TestApp<State, Message, Context> {
    /// Supplies explicit concrete services for the same empty-inbox hook used by real hosts.
    pub fn with_context(
        state: State,
        messages: Receiver<Message>,
        handle: fn(&mut State, Message) -> Result<(), NcStatus>,
        context: Context,
        apply_pending_changes: fn(&mut State, &Context) -> Result<ApplyOutcome, NcStatus>,
    ) -> Self {
        let local = Dispatcher::into_local(Dispatcher::new(
            Processor {
                state,
                handle,
                context,
                apply_pending_changes,
            },
            messages,
        ));
        let owner = local.query().expect("Dispatcher declares IApplicationDispatcher");
        Self { local, owner }
    }
    /// Borrows the cached projection without querying or incrementing its reference count.
    pub fn as_interface(&self) -> ComRef<'_, IApplicationDispatcherAbi> {
        self.owner.as_com_ref()
    }
    /// Uses the production eventful update with the same owner throughout the fixture.
    pub fn update(&self, ui: &Ui, size: Size) -> Result<(), NcStatus> {
        ui.update_with_events(size, self.as_interface())
    }
    /// Grants a short assertion/edit guard; callers release it before event dispatch.
    pub fn state(&self) -> impl DerefMut<Target = State> + '_ {
        StateGuard(self.local.try_borrow_mut().expect("test processor is idle outside dispatch"))
    }
}

/// Maps a typed local guard to fixture state without an erased pointer or separate cell.
struct StateGuard<'a, State: 'static, Message: 'static, Context: 'static>(LocalMut<'a, Dispatcher<Processor<State, Message, Context>>>);
impl<State, Message, Context> Deref for StateGuard<'_, State, Message, Context> {
    type Target = State;
    /// The underlying COM lease keeps this exact allocation alive and exclusively borrowed.
    fn deref(&self) -> &State {
        &self.0.processor.state
    }
}
impl<State, Message, Context> DerefMut for StateGuard<'_, State, Message, Context> {
    /// Reborrowing the lease cannot overlap another local or foreign mutation.
    fn deref_mut(&mut self) -> &mut State {
        &mut self.0.processor.state
    }
}

/// Defines a concrete recording COM object inside a test fixture's lexical scope.
///
/// This helper exists only with `test-support`. Its output type and captured fields
/// are explicit: the body is compiled directly into a concrete `post` implementation,
/// with no callback field, `FnMut` bound, generic adapter or production constructor.
/// The optional fields let failure/lifetime fixtures own their instrumentation rather
/// than closing over a mutable external model. Normal applications use the common channel; this macro only reduces repeated test declarations.
#[macro_export]
macro_rules! test_event_queue {
    ($object:ty,
        $message:ty, $sender:expr, |$this:ident, $origin:pat_param, $event:pat_param| $body:expr
        $(, { $($field:ident: $ty:ty = $value:expr),* $(,)? })? $(,)?) => {{
        #[neocogi_com::com_object(interfaces(neocogi_com::events::IEventReceiver))]
        /// Fixture-local receiver with concrete, fully owned observation storage.
        struct RecordingQueue {
            /// Posting capability does not own the tested widget or application model.
            send: std::sync::mpsc::Sender<$message>,
            $($(/// Explicit fixture instrumentation; lifetime is owned by this queue.
                $field: $ty,)*)?
        }
        impl neocogi_com::events::IEventReceiver for RecordingQueue {
            /// This terminal test recorder reads the snapshot while owning its COM event.
            /// Production inboxes retain EventMessage itself until dispatch.
            fn post(&mut self, origin: neocogi_com::NcValueRef<'_, neocogi_com::events::EventOrigin>,
                event_owner: neocogi_com::ComBox<neocogi_com::events::IEventAbi>) -> Result<(), (neocogi_com::NcStatus, neocogi_com::ComBox<neocogi_com::events::IEventAbi>)> {
                let result = (|| -> Result<(), neocogi_com::NcStatus> {
                    #[allow(unused_imports)]
                    use neocogi_com::INcStringComPtrExt as _;
                    let captured = origin.to_owned()?;
                    let widget = captured.widget::<$object>()?;
                    let service = captured.service::<$object>()?.map(|origin| origin.upgrade()).transpose()?.flatten();
                    let $origin = match (&widget, &service) {
                        (Some(origin), _) => origin.as_com_ref(),
                        (_, Some(origin)) => origin.as_com_ref(),
                        _ => return Err(neocogi_com::NC_E_INVALID_ARG),
                    };
                    let $event = &event_owner;
                    let $this = self;
                    let message: $message = $body;
                    let _ = $this.send.send(message);
                    Ok(())
                })();
                result.map_err(|status| (status, event_owner))
            }
        }
        RecordingQueue::from_value(RecordingQueue { send: $sender, $($($field: $value,)*)? }).query::<neocogi_com::events::IEventReceiverAbi>()
    }};
}
