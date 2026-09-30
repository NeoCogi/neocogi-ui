//! Loads a UI document without linking its concrete widgets into application code.
use neocogi_com::{ComPtr, ComRef, WeakComPtr, NcValueRef, NcStatus, IUnknown};
use neocogi_ui_document::EventBindings;
use neocogi_demo_host::{AppState, Host, HostError};
use neocogi_ui::*;
use neocogi_ui_document::{Document, ReloadSlot, Resources};
use std::sync::mpsc::{self, Sender};

/// Application messages retain nonowning identities, checked against the current component.
enum Message {
    /// Re-read the selected file, or the built-in document.
    Reload(WeakComPtr<IMenuItemAbi>),
    /// The component's public Save action, independent of its widget layout.
    Save(neocogi_com::WeakComBox<ButtonAbi>),
}
#[neocogi_com::com_object(interfaces(neocogi_ui::nodes::ButtonEventQueue, neocogi_ui::IMenuItemEventQueue))]
/// One application receiver is reused across every document replacement and the static menu.
struct AppQueue {
    /// Queued origins never keep a removed component or control alive.
    sender: Sender<Message>,
}
impl neocogi_ui::nodes::ButtonEventQueue for AppQueue {
    /// Acquire the activation before returning; current-form membership is checked at dispatch.
    fn post(&mut self, origin: ComRef<'_, ButtonAbi>, event: NcValueRef<'_, neocogi_ui::button::Event>) -> Result<(), NcStatus> {
        neocogi_ui::button::validate(&event.to_owned()?)?;
        let _ = self.sender.send(Message::Save(origin.downgrade_box()?));
        Ok(())
    }
}
impl neocogi_ui::IMenuItemEventQueue for AppQueue {
    /// Static menus use the same queue but retain their own exact origin type.
    fn post(&mut self, origin: ComRef<'_, IMenuItemAbi>, event: NcValueRef<'_, Activation>) -> Result<(), NcStatus> {
        neocogi_ui::events::activation::validate(&event.to_owned()?)?;
        let _ = self.sender.send(Message::Reload(origin.downgrade()?));
        Ok(())
    }
}
/// Application state owns its current component and uses checked concrete widget handles.
struct State {
    /// Stable publication slot owns the current loaded component.
    slot: ReloadSlot,
    /// Source path is a host concern, never interpreted by the COM loader.
    path: Option<std::path::PathBuf>,
    /// Supplies the same concrete receiver to every replacement component.
    queue: ComPtr<IUnknown>,
    /// Exact menu entry accepted as a reload command.
    reload_item: MenuItem,
    /// Counts application-handled button activations.
    saves: u64,
}
impl State {
    /// Loads authored JSON or deployed binary, then binds before publication.
    fn reload(&mut self) -> Result<(), String> {
        let document = match &self.path {
            Some(path) => {
                let bytes = std::fs::read(path).map_err(|error| error.to_string())?;
                if bytes.starts_with(b"NCB") {
                    Document::from_binary(&bytes)?
                } else {
                    Document::from_json(&bytes)?
                }
            }
            None => Document::new("Runtime-loaded UI"),
        };
        let queue = self.queue.query::<ButtonEventQueueAbi>().map_err(|error| error.to_string())?;
        self.slot.reload(
            &document,
            Resources::new(),
            move |_| {
                let mut events = EventBindings::new();
                events.insert_queue::<ButtonAbi>("save", queue.as_com_ref())?;
                Ok(events)
            },
            |component, _| {
                // Check the exact title interface before publishing the complete replacement.
                component.widget::<TextBlockAbi>("title").map(|_| ())
            },
        )?;
        Ok(())
    }
}
impl AppState for State {
    type Message = Message;
    /// Runtime calls application handlers only at its normal safe mutation boundary.
    fn update(&mut self, message: Message) -> Result<(), NcStatus> {
        match message {
            Message::Reload(origin) => {
                let Some(origin) = origin.upgrade()? else {
                    return Ok(());
                };
                if !origin.as_com_ref().same_object(self.reload_item.as_interface()) {
                    return Ok(());
                }
                if let Err(error) = self.reload() {
                    eprintln!("Reload: {error}");
                }
            }
            Message::Save(origin) => {
                if !origin.is_alive() {
                    return Ok(());
                };
                if let Some(component) = self.slot.component() {
                    if !origin.same_object(component.widget::<ButtonAbi>("save")?.as_interface()) {
                        return Ok(());
                    }
                    self.saves += 1;
                    component
                        .widget::<TextBlockAbi>("title")?
                        .set_text(&format!("Save requested {} times", self.saves))?;
                }
            }
        }
        Ok(())
    }
}
/// Uses the same host and loader in shared, monolithic, and browser assemblies.
fn run() -> Result<(), HostError> {
    // Asset loading activates a registered GFX provider before Host::new runs.
    neocogi_demo_host::initialize()?;
    let host = Host::new("Runtime UI", Size { width: 800, height: 600 }, neocogi_demo_assets::load_bundle()?)?;
    let root = host.ui.widgets().linear(FlowDirection::TopToBottom, CrossSize::Stretch)?;
    let root_handle = root.handle();
    let window = host.ui.window("Loaded component", Rect { x: 20, y: 20, width: 740, height: 540 }, root).map_err(|(status, _candidate)| status)?;
    let mut root = root_handle;

    let mut slot = ReloadSlot::new(host.ui.clone(), window.clone())?;
    root.insert(0, slot.take_widget()?, LinearEdge::new(LayoutTrack::Flex { weight: 1.0 })).map_err(|(status, _candidate)| status)?;
    let (sender, receiver) = mpsc::channel();
    let menus = host.ui.menus()?;
    let bar = menus.create("Runtime UI")?;
    let queue = AppQueue::from_value(AppQueue { sender });
    let reload_item = bar
        .submenu("File")?
        .item_with_queue("Reload", Some(queue.query::<IMenuItemEventQueueAbi>()?.as_com_ref()))?;
    menus.attach_bar(window.as_com_ref(), &bar)?;
    #[cfg(not(target_arch = "wasm32"))]
    let path = std::env::args_os().nth(1).map(std::path::PathBuf::from);
    #[cfg(target_arch = "wasm32")]
    let path = None;
    let mut state = State { slot, path, queue, reload_item, saves: 0 };
    state.reload().map_err(HostError::Message)?;
    host.run(state, receiver)
}
/// Preserves operational diagnostics under the min-size profile.
fn main() -> std::process::ExitCode {
    neocogi_demo_host::report_exit(run())
}
