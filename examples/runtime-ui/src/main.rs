//! Loads a UI document without linking its concrete widgets into application code.
use neocogi_com::NcStatus;
use neocogi_com::events::bind_output;
use neocogi_demo_host::{AppState, Host, HostError};
use neocogi_ui::*;
use neocogi_ui_document::{Document, Generation, ReloadSlot, Resources, EventDeliveryReceiver, event_delivery_diagnostics};
use std::sync::mpsc::{self, Sender};

/// Application messages carry the component generation that emitted them.
enum Message {
    /// Re-read the selected file, or the built-in document.
    Reload,
    /// The component's public Save action, independent of its widget layout.
    Save(Generation),
}
/// Application state owns its current component and uses checked concrete widget handles.
struct State {
    /// Stable publication slot owns the current loaded component.
    slot: ReloadSlot,
    /// Source path is a host concern, never interpreted by the COM loader.
    path: Option<std::path::PathBuf>,
    /// Connects replacement components to the same application inbox.
    sender: Sender<Message>,
    /// Counts application-handled button activations.
    saves: u64,
    /// Bounded delivery failures reported by the loaded component after each frame.
    event_delivery_diagnostics: EventDeliveryReceiver,
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
        let sender = self.sender.clone();
        let outcome = self.slot.reload(&document, Resources::new(), move |component, generation| {
            // Check the exact title widget interface before publishing a replacement.
            component.widget::<TextBlockAbi>("title")?;
            bind_output(component.outputs()?.as_com_ref(), "save_requested", sender, move |_: Activation| {
                Message::Save(generation)
            })
        })?;
        if outcome.teardown.is_err() {
            return Err(format!("UI reloaded; old content cleanup: {}", outcome.teardown));
        }
        Ok(())
    }
}
impl AppState for State {
    type Message = Message;
    /// Runtime calls application handlers only at its normal safe mutation boundary.
    fn update(&mut self, message: Message) -> Result<(), NcStatus> {
        match message {
            Message::Reload => {
                if let Err(error) = self.reload() {
                    eprintln!("Reload: {error}");
                }
            }
            Message::Save(generation) if self.slot.accepts(generation) => {
                self.saves += 1;
                if let Some(component) = self.slot.component() {
                    component.widget::<TextBlockAbi>("title")?.set_text(&format!("Save requested {} times", self.saves))?;
                }
            }
            Message::Save(_) => {} // An old queued event cannot affect the replacement view.
        }
        Ok(())
    }
    /// Reports nonfatal application delivery failures after the frame.
    fn after_update(&mut self, _host: &Host, _size: Size) -> Result<(), NcStatus> {
        let batch = self.event_delivery_diagnostics.drain();
        for diagnostic in batch.reports {
            eprintln!("Event delivery: {diagnostic:?}");
        }
        if batch.omitted != 0 {
            eprintln!("Event delivery: {} intermediate failures omitted", batch.omitted);
        }
        Ok(())
    }
}
/// Uses the same host and loader in shared, monolithic, and browser assemblies.
fn run() -> Result<(), HostError> {
    // Asset loading activates a registered GFX provider before Host::new runs.
    neocogi_demo_host::initialize()?;
    let host = Host::new("Runtime UI", Size { width: 800, height: 600 }, neocogi_demo_assets::load_bundle()?)?;
    let mut root = host.ui.widgets().linear(FlowDirection::TopToBottom, CrossSize::Stretch)?;
    let window = host.ui.window("Loaded component", Rect { x: 20, y: 20, width: 740, height: 540 }, &root)?;
    let (event_sender, event_delivery_diagnostics) = event_delivery_diagnostics(64);
    let slot = ReloadSlot::new(host.ui.clone(), window.clone(), event_sender)?;
    root.insert(0, slot.widget(), LinearEdge::new(LayoutTrack::Flex { weight: 1.0 }))?.finish()?;
    let (sender, receiver) = mpsc::channel();
    let menus = host.ui.menus()?;
    let bar = menus.create("Runtime UI")?;
    neocogi_ui::events::bind(bar.submenu("File")?.item("Reload")?.activated()?, sender.clone(), |_| Message::Reload)?;
    menus.attach_bar(window.as_com_ref(), &bar)?;
    #[cfg(not(target_arch = "wasm32"))]
    let path = std::env::args_os().nth(1).map(std::path::PathBuf::from);
    #[cfg(target_arch = "wasm32")]
    let path = None;
    let mut state = State {
        slot,
        path,
        sender,
        saves: 0,
        event_delivery_diagnostics,
    };
    state.reload().map_err(HostError::Message)?;
    host.run(state, receiver)
}
/// Preserves operational diagnostics under the min-size profile.
fn main() -> std::process::ExitCode {
    neocogi_demo_host::report_exit(run())
}
