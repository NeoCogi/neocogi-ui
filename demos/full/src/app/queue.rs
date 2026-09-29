//! The demo's one queue object posts snapshots without application callbacks.
use super::{Action, Message, NumericTarget};
use neocogi_com::{com_object, ComRef, NcStatus, NcValueRef, WeakComPtr};
use neocogi_ui::*;
use neocogi_ui::nodes::{ButtonEventQueue, SliderEventQueue, TextboxEventQueue, ComboEventQueue};

#[com_object(interfaces(
    neocogi_ui::nodes::ButtonEventQueue,
    neocogi_ui::nodes::SliderEventQueue,
    neocogi_ui::nodes::TextboxEventQueue,
    neocogi_ui::nodes::ComboEventQueue,
    IMenuItemEventQueue,
    IFileDialogEventQueue,
    IWindowEventQueue
))]
/// One receiver allocation serves every observed control in the demo.
/// It owns only posting access, never a reference to application state or targets.
struct Queue {
    /// Disconnection discards future events without undoing committed UI edits.
    sender: std::sync::mpsc::Sender<Message>,
}

/// Ordinary application-owned construction and command identities.
/// Only &mut Inputs registers controls; no second COM borrow gate protects this data.
pub(crate) struct Inputs {
    /// Shared concrete receiver; each factory requests its exact interface projection.
    pub(super) queue: neocogi_com::ComPtr<neocogi_com::IUnknown>,
    /// Weak identities keep action lookup independent of labels and address reuse.
    pub(super) buttons: Vec<(WeakComPtr<ButtonAbi>, Action)>,
    /// Menu entries retain only their explicit application command.
    pub(super) menus: Vec<(WeakComPtr<IMenuItemAbi>, Action)>,
    /// Color and metric targets are data rather than closure specializations.
    pub(super) sliders: Vec<(WeakComPtr<SliderAbi>, NumericTarget)>,
}
impl Inputs {
    /// Constructs the sole queue before creating any eventful widgets.
    pub(super) fn new(sender: std::sync::mpsc::Sender<Message>) -> Self {
        Self {
            queue: Queue::from_value(Queue { sender }),
            buttons: Vec::new(),
            menus: Vec::new(),
            sliders: Vec::new(),
        }
    }
}
impl ButtonEventQueue for Queue {
    /// Acquires producer-owned fields before returning across COM; handlers run later.
    fn post(&mut self, origin: ComRef<'_, ButtonAbi>, event: NcValueRef<'_, neocogi_ui::events::button::Event>) -> Result<(), NcStatus> {
        let event = event.to_owned()?;
        neocogi_ui::events::button::validate(&event)?;
        let _ = self.sender.send(Message::Button(origin.downgrade()?, event));
        Ok(())
    }
}
impl SliderEventQueue for Queue {
    /// Acquires producer-owned fields before returning across COM; handlers run later.
    fn post(&mut self, origin: ComRef<'_, SliderAbi>, event: NcValueRef<'_, neocogi_ui::events::slider::Event>) -> Result<(), NcStatus> {
        let event = event.to_owned()?;
        neocogi_ui::events::slider::validate(&event)?;
        let _ = self.sender.send(Message::Slider(origin.downgrade()?, event));
        Ok(())
    }
}
impl IMenuItemEventQueue for Queue {
    /// Acquires producer-owned fields before returning across COM; handlers run later.
    fn post(&mut self, origin: ComRef<'_, IMenuItemAbi>, event: NcValueRef<'_, neocogi_ui::events::activation::Event>) -> Result<(), NcStatus> {
        let event = event.to_owned()?;
        neocogi_ui::events::activation::validate(&event)?;
        let _ = self.sender.send(Message::Menu(origin.downgrade()?, event));
        Ok(())
    }
}
impl TextboxEventQueue for Queue {
    /// Acquires producer-owned fields before returning across COM; handlers run later.
    fn post(&mut self, origin: ComRef<'_, TextboxAbi>, event: NcValueRef<'_, neocogi_ui::events::textbox::Event>) -> Result<(), NcStatus> {
        let event = event.to_owned()?;
        neocogi_ui::events::textbox::validate(&event)?;
        let _ = self.sender.send(Message::Textbox(origin.downgrade()?, event));
        Ok(())
    }
}
impl ComboEventQueue for Queue {
    /// Acquires producer-owned fields before returning across COM; handlers run later.
    fn post(&mut self, origin: ComRef<'_, ComboAbi>, event: NcValueRef<'_, neocogi_ui::events::combo::Event>) -> Result<(), NcStatus> {
        let event = event.to_owned()?;
        neocogi_ui::events::combo::validate(&event)?;
        let _ = self.sender.send(Message::Combo(origin.downgrade()?, event));
        Ok(())
    }
}
impl IFileDialogEventQueue for Queue {
    /// Acquires producer-owned fields before returning across COM; handlers run later.
    fn post(&mut self, origin: ComRef<'_, IFileDialogAbi>, event: NcValueRef<'_, neocogi_ui::events::file_dialog::Event>) -> Result<(), NcStatus> {
        let event = event.to_owned()?;
        neocogi_ui::events::file_dialog::validate(&event)?;
        let _ = self.sender.send(Message::FileDialog(origin.downgrade()?, event));
        Ok(())
    }
}
impl IWindowEventQueue for Queue {
    /// Acquires producer-owned fields before returning across COM; handlers run later.
    fn post(&mut self, origin: ComRef<'_, IWindowAbi>, event: NcValueRef<'_, neocogi_ui::events::window::Event>) -> Result<(), NcStatus> {
        let event = event.to_owned()?;
        neocogi_ui::events::window::validate(&event)?;
        let _ = self.sender.send(Message::WindowEvent(origin.downgrade()?, event));
        Ok(())
    }
}
