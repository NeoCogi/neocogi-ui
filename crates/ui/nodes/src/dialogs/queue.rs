//! One typed COM queue for a file dialog's private child controls.

#[neocogi_com::com_object(interfaces(neocogi_com::events::IEventReceiver))]
/// The component's queue retains no dialog, window or child widget strongly.
/// It owns only posting access; targets belong to the exclusively borrowed dialog model.
pub(super) struct DialogQueue {
    /// Receiver destruction disconnects posting without invalidating retained children.
    sender: std::sync::mpsc::Sender<neocogi_com::events::EventMessage>,
}
impl DialogQueue {
    /// Supplies one posting endpoint before any eventful child is constructed.
    pub(super) fn new(sender: std::sync::mpsc::Sender<neocogi_com::events::EventMessage>) -> Self {
        Self { sender }
    }
}

impl neocogi_com::events::IEventReceiver for DialogQueue {
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
