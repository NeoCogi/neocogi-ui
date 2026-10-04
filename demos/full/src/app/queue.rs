//! The demo's one queue object posts snapshots without application callbacks.
use super::{Action, NumericTarget};
use neocogi_com::WeakComPtr;
use neocogi_ui::*;

#[neocogi_com::com_object(interfaces(neocogi_com::events::IEventReceiver))]
/// One receiver allocation serves every observed control in the demo.
/// It owns only posting access, never a reference to application state or targets.
struct Queue {
    /// Disconnection rejects delivery and returns the original event owner.
    sender: std::sync::mpsc::Sender<neocogi_com::events::EventMessage>,
}

/// Ordinary application-owned construction and command identities.
/// Only &mut Inputs registers controls; no second COM borrow gate protects this data.
pub(crate) struct Inputs {
    /// Common receiver projection shared directly with every observed factory.
    pub(super) queue: neocogi_com::ComPtr<neocogi_com::events::IEventReceiverAbi>,
    /// Weak identities keep action lookup independent of labels and address reuse.
    pub(super) buttons: Vec<(neocogi_com::WeakComBox<ButtonAbi>, Action)>,
    /// Menu entries retain only their explicit application command.
    pub(super) menus: Vec<(WeakComPtr<IMenuItemAbi>, Action)>,
    /// Color and metric targets are data rather than closure specializations.
    pub(super) sliders: Vec<(neocogi_com::WeakComBox<SliderAbi>, NumericTarget)>,
}
impl Inputs {
    /// Constructs the sole queue before creating any eventful widgets.
    pub(super) fn new(sender: std::sync::mpsc::Sender<neocogi_com::events::EventMessage>) -> Self {
        Self {
            queue: Queue::from_value(Queue { sender }).query().expect("Queue implements IEventReceiver"),
            buttons: Vec::new(),
            menus: Vec::new(),
            sliders: Vec::new(),
        }
    }
}

impl neocogi_com::events::IEventReceiver for Queue {
    /// Transfer the exclusive event to the inbox; a failed send restores its owner.
    fn post(
        &mut self,
        origin: neocogi_com::NcValueRef<'_, neocogi_com::events::EventOrigin>,
        event: neocogi_com::ComBox<neocogi_com::events::IEventAbi>,
    ) -> Result<(), (neocogi_com::NcStatus, neocogi_com::ComBox<neocogi_com::events::IEventAbi>)> {
        let message = neocogi_com::events::EventMessage::capture(origin, event)?;
        self.sender.send(message).map_err(|error| (neocogi_com::NC_E_CLOSED, error.0.event))
    }
}
