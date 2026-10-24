//! Receiver ownership regressions moved with the UI protocol.
//! Receiver capture distinguishes a valid borrow from shared lifetime authority.
use super::*;
use neocogi_com::{ComBox, NcStatus, NcValueRef, NC_E_CLOSED, NC_E_INVALID_ARG};
use std::{cell::Cell, rc::Rc};

#[neocogi_com::com_object(interfaces(neocogi_ui::abi::core::IEventReceiver), ownership = exclusive)]
/// A valid exclusive receiver which must never gain a second owner.
struct ExclusiveReceiver {
    /// Independent observation of synchronous owner destruction.
    drops: Rc<Cell<u32>>,
}
impl Drop for ExclusiveReceiver {
    fn drop(&mut self) {
        self.drops.set(self.drops.get() + 1);
    }
}
impl IEventReceiver for ExclusiveReceiver {
    fn post(&mut self, _origin: NcValueRef<'_, EventOrigin>, event: ComBox<IEventAbi>) -> Result<(), (NcStatus, ComBox<IEventAbi>)> {
        Err((NC_E_CLOSED, event))
    }
}

/// Shared capture retains exactly the destination; absence acquires nothing.
#[test]
fn binding_retains_shared_receiver_until_consumed_queue_is_dropped() {
    assert!(matches!(EventBinding::from_queue(None).unwrap(), EventBinding::Unobserved));
    let (queue, inbox) = channel();
    let weak = queue.downgrade().unwrap();
    let binding = EventBinding::from_queue(Some(queue.as_com_ref())).unwrap();
    drop(queue);
    assert!(weak.upgrade().unwrap().is_some());
    let retained = binding.into_queue().unwrap();
    drop(retained);
    assert!(weak.upgrade().unwrap().is_none());
    assert!(matches!(inbox.try_recv(), Err(std::sync::mpsc::TryRecvError::Disconnected)));
}

/// Rejection neither destroys nor duplicates the live exclusive receiver.
#[test]
fn binding_rejects_exclusive_receiver_without_changing_owner_lifetime() {
    let drops = Rc::new(Cell::new(0));
    let owner = ExclusiveReceiver::into_box(ExclusiveReceiver { drops: drops.clone() })
        .query::<IEventReceiverAbi>()
        .unwrap();
    let weak = owner.downgrade();
    assert!(matches!(EventBinding::from_queue(Some(owner.as_com_ref())), Err(NC_E_INVALID_ARG)));
    assert!(matches!(EventBinding::from_queue(Some(weak.as_com_ref())), Err(NC_E_INVALID_ARG)));
    assert!(weak.is_alive());
    assert_eq!(drops.get(), 0);
    drop(owner);
    assert_eq!(drops.get(), 1);
    assert!(!weak.is_alive());
}
