//! One typed COM queue for a file dialog's private child controls.
use super::Message;
use neocogi_com::{com_object, ComRef, NcStatus, NcValueRef};
use neocogi_ui::IWindowAbi;

#[com_object(interfaces(
    neocogi_ui::nodes::ButtonEventQueue,
    neocogi_ui::nodes::ListItemEventQueue,
    neocogi_ui::nodes::TextboxEventQueue,
    neocogi_ui::IWindowEventQueue
))]
/// The component's queue retains no dialog, window or child widget strongly.
/// It owns only posting access; targets belong to the exclusively borrowed dialog model.
pub(super) struct DialogQueue {
    /// Receiver destruction disconnects posting without invalidating retained children.
    sender: std::sync::mpsc::Sender<Message>,
}
impl DialogQueue {
    /// Supplies one posting endpoint before any eventful child is constructed.
    pub(super) fn new(sender: std::sync::mpsc::Sender<Message>) -> Self {
        Self { sender }
    }
}
impl neocogi_ui::nodes::ButtonEventQueue for DialogQueue {
    /// Owns the snapshot before returning to its producer; application work is deferred.
    fn post(&mut self, origin: ComRef<'_, neocogi_ui::ButtonAbi>, event: NcValueRef<'_, neocogi_ui::events::button::Event>) -> Result<(), NcStatus> {
        let event = event.to_owned()?;
        neocogi_ui::events::button::validate(&event)?;
        let _ = self.sender.send(Message::Button(origin.downgrade()?, event));
        Ok(())
    }
}
impl neocogi_ui::nodes::ListItemEventQueue for DialogQueue {
    /// Owns the snapshot before returning to its producer; application work is deferred.
    fn post(&mut self, origin: ComRef<'_, neocogi_ui::ListItemAbi>, event: NcValueRef<'_, neocogi_ui::events::list_item::Event>) -> Result<(), NcStatus> {
        let event = event.to_owned()?;
        neocogi_ui::events::list_item::validate(&event)?;
        let _ = self.sender.send(Message::Row(origin.downgrade()?, event));
        Ok(())
    }
}
impl neocogi_ui::nodes::TextboxEventQueue for DialogQueue {
    /// Owns the snapshot before returning to its producer; application work is deferred.
    fn post(&mut self, origin: ComRef<'_, neocogi_ui::TextboxAbi>, event: NcValueRef<'_, neocogi_ui::events::textbox::Event>) -> Result<(), NcStatus> {
        let event = event.to_owned()?;
        neocogi_ui::events::textbox::validate(&event)?;
        let _ = self.sender.send(Message::Text(origin.downgrade()?, event));
        Ok(())
    }
}
impl neocogi_ui::IWindowEventQueue for DialogQueue {
    /// Owns the snapshot before returning to its producer; application work is deferred.
    fn post(&mut self, origin: ComRef<'_, IWindowAbi>, event: NcValueRef<'_, neocogi_ui::events::window::Event>) -> Result<(), NcStatus> {
        let event = event.to_owned()?;
        neocogi_ui::events::window::validate(&event)?;
        let _ = self.sender.send(Message::Window(origin.downgrade()?, event));
        Ok(())
    }
}
