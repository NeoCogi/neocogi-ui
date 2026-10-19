//! Loads a UI document without linking its concrete widgets into application code.
use neocogi_com::{ComPtr, NcStatus};
use neocogi_ui_document::EventBindings;
use neocogi_demo_host::{AppState, Host, HostError};
use neocogi_ui::*;
use neocogi_ui_document::{Document, ReloadIds, ReloadSlot, Resources};

/// Application state owns its current component and uses checked concrete widget handles.
struct State {
    /// Stable publication slot owns the current loaded component.
    slot: ReloadSlot,
    /// Source path is a host concern, never interpreted by the COM loader.
    path: Option<std::path::PathBuf>,
    /// Supplies the same concrete receiver to every replacement component.
    queue: ComPtr<neocogi_com::events::IEventReceiverAbi>,
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
        let queue = self.queue.query::<neocogi_com::events::IEventReceiverAbi>().map_err(|error| error.to_string())?;
        self.slot.reload(
            &document,
            Resources::new(),
            move |_| {
                let mut events = EventBindings::new();
                events.insert_queue("save", queue.as_com_ref())?;
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
    type Message = neocogi_com::events::EventMessage;
    /// Runtime calls application handlers only at its normal safe mutation boundary.
    fn update(&mut self, event: Self::Message) -> Result<(), NcStatus> {
        if let Some(origin) = event.origin.service::<IWindowAbi>()? {
            if event.query::<IWindowCloseRequestedEventAbi>()?.is_some() {
                if let Some(window) = origin.upgrade()? { window.set_visibility(Visibility::Hidden)?; }
            }
        } else if let Some(origin) = event.origin.service::<IMenuItemAbi>()? {
            if let Some(origin) = origin.upgrade()? {
                if origin.as_com_ref().same_object(self.reload_item.as_interface()) && event.query::<IActivatedEventAbi>()?.is_some() {
                    if let Err(error) = self.reload() { eprintln!("Reload: {error}"); }
                }
            }
        } else if let Some(origin) = event.origin.widget::<ButtonAbi>()? {
            if origin.is_alive() {
                if let Some(component) = self.slot.component() {
                    if origin.same_object(component.widget::<ButtonAbi>("save")?.as_interface()) && event.query::<IActivatedEventAbi>()?.is_some() {
                        self.saves += 1;
                        component.widget::<TextBlockAbi>("title")?.set_text(&format!("Save requested {} times", self.saves))?;
                    }
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
    // One allocator belongs to this application's dispatch domain. Reuse it for
    // any additional/recreated slots that post into the same channel.
    let mut reload_ids = ReloadIds::new();
    let (queue, receiver) = neocogi_com::events::channel();
    let lifecycle = queue.query::<neocogi_com::events::IEventReceiverAbi>()?;
    let root = host.ui.widgets().linear(FlowDirection::TopToBottom, CrossSize::Stretch)?;
    let root_handle = root.handle();
    let window = host.ui.window_with_queue("Loaded component", Rect { x: 20, y: 20, width: 740, height: 540 }, root, Some(lifecycle.as_com_ref())).map_err(|(status, _candidate)| status)?;
    let mut root = root_handle;

    let mut slot = ReloadSlot::new(&mut reload_ids, host.ui.clone(), window.clone(), GridLimits { max_columns: 64, max_rows: 256 })?;
    root.insert(0, slot.take_widget()?, LinearEdge::new(LayoutTrack::Flex { weight: 1.0 })).map_err(|(status, _candidate)| status)?;
    let menus = host.ui.menus()?;
    let bar = menus.create("Runtime UI")?;
    let reload_item = bar
        .submenu("File")?
        .item_with_queue("Reload", Some(queue.query::<neocogi_com::events::IEventReceiverAbi>()?.as_com_ref()))?;
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
