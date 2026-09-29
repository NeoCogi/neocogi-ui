//! Owned test applications exercise production controller gating and runtime scheduling.

use neocogi_com::{ComPtr, ComRef, LocalCom, LocalMut, NcStatus};
use crate::{
    IApplicationDispatcherAbi, Size, Ui,
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
    owner: ComPtr<IApplicationDispatcherAbi>,
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
        let local = Controller::into_local(Controller::new(Model { state, handle }, messages));
        let owner = local.query().expect("Controller declares IApplicationDispatcher");
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

/// Defines a concrete recording COM object inside a test fixture's lexical scope.
///
/// This helper exists only with `test-support`. Its output type and captured fields
/// are explicit: the body is compiled directly into a concrete `post` implementation,
/// with no callback field, `FnMut` bound, generic adapter or production constructor.
/// The optional fields let failure/lifetime fixtures own their instrumentation rather
/// than closing over a mutable external model. Normal applications implement their
/// own shared queue interfaces; this macro only reduces repeated test declarations.
#[macro_export]
macro_rules! test_event_queue {
    (button, $message:ty, $sender:expr, |$this:ident, $origin:pat_param, $event:pat_param| $body:expr $(, { $($field:ident: $ty:ty = $value:expr),* $(,)? })? $(,)?) => {
        $crate::test_event_queue!(@concrete $crate::nodes::ButtonEventQueue, $crate::nodes::ButtonEventQueueAbi, $crate::nodes::ButtonAbi,
            button, $message, $sender, |$this, $origin, $event| $body $(, { $($field: $ty = $value),* })?)
    };
    (checkbox, $message:ty, $sender:expr, |$this:ident, $origin:pat_param, $event:pat_param| $body:expr $(, { $($field:ident: $ty:ty = $value:expr),* $(,)? })? $(,)?) => {
        $crate::test_event_queue!(@concrete $crate::nodes::CheckboxEventQueue, $crate::nodes::CheckboxEventQueueAbi, $crate::nodes::CheckboxAbi,
            checkbox, $message, $sender, |$this, $origin, $event| $body $(, { $($field: $ty = $value),* })?)
    };
    (textbox, $message:ty, $sender:expr, |$this:ident, $origin:pat_param, $event:pat_param| $body:expr $(, { $($field:ident: $ty:ty = $value:expr),* $(,)? })? $(,)?) => {
        $crate::test_event_queue!(@concrete $crate::nodes::TextboxEventQueue, $crate::nodes::TextboxEventQueueAbi, $crate::nodes::TextboxAbi,
            textbox, $message, $sender, |$this, $origin, $event| $body $(, { $($field: $ty = $value),* })?)
    };
    (text_area, $message:ty, $sender:expr, |$this:ident, $origin:pat_param, $event:pat_param| $body:expr $(, { $($field:ident: $ty:ty = $value:expr),* $(,)? })? $(,)?) => {
        $crate::test_event_queue!(@concrete $crate::nodes::TextAreaEventQueue, $crate::nodes::TextAreaEventQueueAbi, $crate::nodes::TextAreaAbi,
            text_area, $message, $sender, |$this, $origin, $event| $body $(, { $($field: $ty = $value),* })?)
    };
    (slider, $message:ty, $sender:expr, |$this:ident, $origin:pat_param, $event:pat_param| $body:expr $(, { $($field:ident: $ty:ty = $value:expr),* $(,)? })? $(,)?) => {
        $crate::test_event_queue!(@concrete $crate::nodes::SliderEventQueue, $crate::nodes::SliderEventQueueAbi, $crate::nodes::SliderAbi,
            slider, $message, $sender, |$this, $origin, $event| $body $(, { $($field: $ty = $value),* })?)
    };
    (number, $message:ty, $sender:expr, |$this:ident, $origin:pat_param, $event:pat_param| $body:expr $(, { $($field:ident: $ty:ty = $value:expr),* $(,)? })? $(,)?) => {
        $crate::test_event_queue!(@concrete $crate::nodes::NumberEventQueue, $crate::nodes::NumberEventQueueAbi, $crate::nodes::NumberAbi,
            number, $message, $sender, |$this, $origin, $event| $body $(, { $($field: $ty = $value),* })?)
    };
    (list_item, $message:ty, $sender:expr, |$this:ident, $origin:pat_param, $event:pat_param| $body:expr $(, { $($field:ident: $ty:ty = $value:expr),* $(,)? })? $(,)?) => {
        $crate::test_event_queue!(@concrete $crate::nodes::ListItemEventQueue, $crate::nodes::ListItemEventQueueAbi, $crate::nodes::ListItemAbi,
            list_item, $message, $sender, |$this, $origin, $event| $body $(, { $($field: $ty = $value),* })?)
    };
    (list_box, $message:ty, $sender:expr, |$this:ident, $origin:pat_param, $event:pat_param| $body:expr $(, { $($field:ident: $ty:ty = $value:expr),* $(,)? })? $(,)?) => {
        $crate::test_event_queue!(@concrete $crate::nodes::ListBoxEventQueue, $crate::nodes::ListBoxEventQueueAbi, $crate::nodes::ListBoxAbi,
            list_box, $message, $sender, |$this, $origin, $event| $body $(, { $($field: $ty = $value),* })?)
    };
    (scrollbar, $message:ty, $sender:expr, |$this:ident, $origin:pat_param, $event:pat_param| $body:expr $(, { $($field:ident: $ty:ty = $value:expr),* $(,)? })? $(,)?) => {
        $crate::test_event_queue!(@concrete $crate::nodes::ScrollbarEventQueue, $crate::nodes::ScrollbarEventQueueAbi, $crate::nodes::ScrollbarAbi,
            scrollbar, $message, $sender, |$this, $origin, $event| $body $(, { $($field: $ty = $value),* })?)
    };
    (split_view, $message:ty, $sender:expr, |$this:ident, $origin:pat_param, $event:pat_param| $body:expr $(, { $($field:ident: $ty:ty = $value:expr),* $(,)? })? $(,)?) => {
        $crate::test_event_queue!(@concrete $crate::nodes::SplitViewEventQueue, $crate::nodes::SplitViewEventQueueAbi, $crate::nodes::SplitViewAbi,
            split_view, $message, $sender, |$this, $origin, $event| $body $(, { $($field: $ty = $value),* })?)
    };
    (combo, $message:ty, $sender:expr, |$this:ident, $origin:pat_param, $event:pat_param| $body:expr $(, { $($field:ident: $ty:ty = $value:expr),* $(,)? })? $(,)?) => {
        $crate::test_event_queue!(@concrete $crate::nodes::ComboEventQueue, $crate::nodes::ComboEventQueueAbi, $crate::nodes::ComboAbi,
            combo, $message, $sender, |$this, $origin, $event| $body $(, { $($field: $ty = $value),* })?)
    };
    (file_dialog, $message:ty, $sender:expr, |$this:ident, $origin:pat_param, $event:pat_param| $body:expr $(, { $($field:ident: $ty:ty = $value:expr),* $(,)? })? $(,)?) => {
        $crate::test_event_queue!(@concrete $crate::IFileDialogEventQueue, $crate::IFileDialogEventQueueAbi, $crate::IFileDialogAbi,
            file_dialog, $message, $sender, |$this, $origin, $event| $body $(, { $($field: $ty = $value),* })?)
    };
    (window, $message:ty, $sender:expr, |$this:ident, $origin:pat_param, $event:pat_param| $body:expr $(, { $($field:ident: $ty:ty = $value:expr),* $(,)? })? $(,)?) => {
        $crate::test_event_queue!(@concrete $crate::IWindowEventQueue, $crate::IWindowEventQueueAbi, $crate::IWindowAbi,
            window, $message, $sender, |$this, $origin, $event| $body $(, { $($field: $ty = $value),* })?)
    };
    (popup, $message:ty, $sender:expr, |$this:ident, $origin:pat_param, $event:pat_param| $body:expr $(, { $($field:ident: $ty:ty = $value:expr),* $(,)? })? $(,)?) => {
        $crate::test_event_queue!(@concrete $crate::IPopupEventQueue, $crate::IPopupEventQueueAbi, $crate::IPopupAbi,
            popup, $message, $sender, |$this, $origin, $event| $body $(, { $($field: $ty = $value),* })?)
    };
    (activation, $message:ty, $sender:expr, |$this:ident, $origin:pat_param, $event:pat_param| $body:expr $(, { $($field:ident: $ty:ty = $value:expr),* $(,)? })? $(,)?) => {
        $crate::test_event_queue!(@concrete $crate::IMenuItemEventQueue, $crate::IMenuItemEventQueueAbi, $crate::IMenuItemAbi,
            activation, $message, $sender, |$this, $origin, $event| $body $(, { $($field: $ty = $value),* })?)
    };
    (@concrete $interface:path, $abi:ty, $object:ty, $kind:ident,
        $message:ty, $sender:expr, |$this:ident, $origin:pat_param, $event:pat_param| $body:expr
        $(, { $($field:ident: $ty:ty = $value:expr),* $(,)? })? $(,)?) => {{
        #[neocogi_com::com_object($interface)]
        /// Fixture-local receiver with concrete, fully owned observation storage.
        struct RecordingQueue {
            /// Posting capability does not own the tested widget or application model.
            send: std::sync::mpsc::Sender<$message>,
            $($(/// Explicit fixture instrumentation; lifetime is owned by this queue.
                $field: $ty,)*)?
        }
        impl $interface for RecordingQueue {
            /// Exercise real generated decoding and domain validation before recording.
            fn post(&mut self, $origin: neocogi_com::ComRef<'_, $object>,
                value: neocogi_com::NcValueRef<'_, $crate::events::$kind::Event>) -> Result<(), neocogi_com::NcStatus> {
                let value = value.to_owned()?;
                $crate::events::$kind::validate(&value)?;
                let $event = value;
                let $this = self;
                let message: $message = $body;
                let _ = $this.send.send(message);
                Ok(())
            }
        }
        RecordingQueue::from_value(RecordingQueue { send: $sender, $($($field: $value,)*)? }).query::<$abi>()
    }};
}
