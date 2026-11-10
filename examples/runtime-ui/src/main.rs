//! Loads a UI document without linking its concrete widgets into application code.
use neocogi_com::{ComPtr, NcStatus};
use neocogi_ui::document::EventBindings;
use neocogi_demo_host::{AppState, Host, HostError};
use neocogi_ui_contracts::*;
use neocogi_ui::document::{Document, ReloadIds, ReloadSlot, Resources};

/// Application state owns its current component and uses checked concrete widget handles.
struct State {
    /// Stable publication slot owns the current loaded component.
    slot: ReloadSlot,
    /// Source path is a host concern, never interpreted by the COM loader.
    path: Option<std::path::PathBuf>,
    /// Supplies the same concrete receiver to every replacement component.
    queue: ComPtr<neocogi_ui_contracts::events::IEventReceiverAbi>,
    /// Exact menu entry accepted as a reload command.
    reload_item: MenuItem,
    /// Counts application-handled button activations.
    saves: u64,
}
/// The application requires concrete Save/title fields independently of generic event eligibility.
fn required_fields(component: &neocogi_ui::document::Component) -> Result<(), NcStatus> {
    component.widget::<ButtonAbi>("save")?;
    component.widget::<TextBlockAbi>("title")?;
    Ok(())
}

impl State {
    /// Loads authored JSON or deployed binary, then binds before publication.
    fn reload(&mut self) -> Result<neocogi_ui::document::Reloaded, String> {
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
        let queue = self
            .queue
            .query::<neocogi_ui_contracts::events::IEventReceiverAbi>()
            .map_err(|error| error.to_string())?;
        self.slot.reload(
            &document,
            Resources::new(),
            move |_| {
                let mut events = EventBindings::new();
                events.insert_queue("save", queue.as_com_ref())?;
                Ok(events)
            },
            |component, _| required_fields(component),
        )
    }
}
impl AppState for State {
    type Message = neocogi_ui_contracts::events::EventMessage;
    /// Runtime calls application handlers only at its normal safe mutation boundary.
    fn update(&mut self, event: Self::Message) -> Result<(), NcStatus> {
        if let Some(origin) = event.origin.service::<IWindowAbi>()? {
            if event.query::<IWindowCloseRequestedEventAbi>()?.is_some() {
                if let Some(window) = origin.upgrade()? {
                    window.set_visibility(Visibility::Hidden)?;
                }
            }
        } else if let Some(origin) = event.origin.service::<IMenuItemAbi>()? {
            if let Some(origin) = origin.upgrade()? {
                if origin.as_com_ref().same_object(self.reload_item.as_interface()) && event.query::<IActivatedEventAbi>()?.is_some() {
                    if let Err(error) = self.reload() {
                        eprintln!("Reload: {error}");
                    }
                }
            }
        } else if let Some(origin) = event.origin.widget::<ButtonAbi>()? {
            if origin.is_alive() {
                if let Some(component) = self.slot.component() {
                    if origin.same_object(component.widget::<ButtonAbi>("save")?.as_interface()) && event.query::<IActivatedEventAbi>()?.is_some() {
                        self.saves += 1;
                        component
                            .widget::<TextBlockAbi>("title")?
                            .set_text(&format!("Save requested {} times", self.saves))?;
                    }
                }
            }
        }
        Ok(())
    }
}
/// Uses the same host and loader in shared, monolithic, and browser assemblies.
fn run(runtime: neocogi_com::Runtime) -> Result<(), HostError> {
    // Asset loading activates a registered GFX provider before Host::new runs.
    neocogi_demo_host::initialize(runtime)?;
    let host = Host::new(
        runtime,
        "Runtime UI",
        Size { width: 800, height: 600 },
        neocogi_demo_assets::load_bundle(runtime)?,
    )?;
    // One allocator belongs to this application's dispatch domain. Reuse it for
    // any additional/recreated slots that post into the same channel.
    let mut reload_ids = ReloadIds::new();
    let (queue, receiver) = neocogi_ui_contracts::events::channel();
    let lifecycle = queue.query::<neocogi_ui_contracts::events::IEventReceiverAbi>()?;
    let root = host.ui.widgets().linear(FlowDirection::TopToBottom, CrossSize::Stretch)?;
    let root_handle = root.handle();
    let window = host
        .ui
        .window_with_queue(
            "Loaded component",
            Rect { x: 20, y: 20, width: 740, height: 540 },
            root,
            Some(lifecycle.as_com_ref()),
        )
        .map_err(|(status, _candidate)| status)?;
    let mut root = root_handle;

    let mut slot = ReloadSlot::new(&mut reload_ids, host.ui.clone(), window.clone(), GridLimits { max_columns: 64, max_rows: 256 })?;
    root.insert(0, slot.take_widget()?, LinearEdge::new(LayoutTrack::Flex { weight: 1.0 }))
        .map_err(|(status, _candidate)| status)?;
    let menus = host.ui.menus()?;
    let bar = menus.create("Runtime UI")?;
    let reload_item = bar
        .submenu("File")?
        .item_with_queue("Reload", Some(queue.query::<neocogi_ui_contracts::events::IEventReceiverAbi>()?.as_com_ref()))?;
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
#[neocogi_com::com_main]
fn main(runtime: neocogi_com::Runtime) -> std::process::ExitCode {
    neocogi_demo_host::report_exit(run(runtime))
}

#[cfg(test)]
mod tests {
    use super::*;
    use neocogi_ui::document::WidgetSpec;

    /// Uses actual production UI/GFX factories with tiny canonical test assets, without a native window.
    fn application(path: std::path::PathBuf) -> (State, neocogi_com::Runtime, Ui) {
        // SAFETY: the test executable owns its process-lived immutable host service graph.
        let registry = unsafe { neocogi_com::Runtime::__from_host_api(neocogi_com_host::initialize_process()) }.unwrap();
        let gfx = neocogi_ui::support::test_support::gfx_factory(registry);
        let bundle = neocogi_ui::support::test_support::assets::export_bundle(&neocogi_ui::support::SkinBundle::from_atlas(
            neocogi_ui::support::test_support::test_atlas(),
        ))
        .unwrap();
        let runtime = neocogi_ui::runtime::create_runtime_from_assets(bundle.as_com_ref(), gfx.as_com_ref()).unwrap();
        let ui = neocogi_ui::nodes::bind_builtin_nodes(runtime).unwrap();
        let root = ui.widgets().linear(FlowDirection::TopToBottom, CrossSize::Stretch).unwrap();
        let window = ui.window("reload regression", Rect { x: 0, y: 0, width: 320, height: 200 }, root).unwrap();
        let mut ids = ReloadIds::new();
        let slot = ReloadSlot::new(&mut ids, ui.clone(), window.clone(), GridLimits { max_columns: 64, max_rows: 256 }).unwrap();
        let menus = ui.menus().unwrap();
        let bar = menus.create("").unwrap();
        let file = bar.submenu("File").unwrap();
        let reload_item = file.item("Reload").unwrap();
        menus.attach_bar(window.as_com_ref(), &bar).unwrap();
        let (queue, _inbox) = neocogi_ui_contracts::events::channel();
        (
            State {
                slot,
                path: Some(path),
                queue,
                reload_item,
                saves: 0,
            },
            registry,
            ui,
        )
    }

    /// A valid common event with a concrete weak origin still reaches the application's real handler.
    fn activation(button: &Widget<ButtonAbi>) -> neocogi_ui_contracts::events::EventMessage {
        neocogi_ui_contracts::events::EventMessage {
            origin: neocogi_ui_contracts::events::EventOrigin::Widget(button.as_interface().clone().into_unknown()),
            event: neocogi_ui_contracts::events::activated().unwrap(),
        }
    }

    /// Rejected candidates preserve publication and Save; accepted replacements reject stale/foreign origins.
    #[test]
    fn production_reload_and_handler_preserve_required_fields_and_origins() {
        let temporary = tempfile::tempdir().unwrap();
        let path = temporary.path().join("form.json");
        let valid = Document::new("accepted");
        std::fs::write(&path, valid.to_json().unwrap()).unwrap();
        let (mut state, registry, ui) = application(path.clone());
        let first = state.reload().unwrap().generation;
        let old_save = state.slot.component().unwrap().widget::<ButtonAbi>("save").unwrap();
        let old_root = state.slot.component().unwrap().root().unwrap();
        let old_title = state.slot.component().unwrap().widget::<TextBlockAbi>("title").unwrap();
        let old_recipe = state.slot.component().unwrap().save(&registry).unwrap();
        for failure in 0..4 {
            let mut candidate = valid.clone();
            match failure {
                0 => {
                    candidate.root.find_mut("save").unwrap().widget = WidgetSpec::Checkbox {
                        text: "wrong kind".into(),
                        checked: false,
                    }
                }
                1 => {
                    candidate.root.children.retain(|child| child.node.key != "save");
                    candidate.outputs.clear();
                }
                2 => candidate.root.find_mut("title").unwrap().widget = WidgetSpec::Button { text: "wrong title".into() },
                3 => candidate.root.children.retain(|child| child.node.key != "title"),
                _ => unreachable!(),
            }
            std::fs::write(&path, candidate.to_json().unwrap()).unwrap();
            assert!(state.reload().is_err(), "case {failure}");
            assert!(state.slot.accepts(first));
            assert_eq!(state.slot.component().unwrap().save(&registry).unwrap(), old_recipe);
            assert!(
                old_title
                    .as_interface()
                    .same_object(state.slot.component().unwrap().widget::<TextBlockAbi>("title").unwrap().as_interface())
            );
            assert!(
                old_root
                    .as_interface()
                    .same_object(state.slot.component().unwrap().root().unwrap().as_interface())
            );
            assert!(
                old_save
                    .as_interface()
                    .same_object(state.slot.component().unwrap().widget::<ButtonAbi>("save").unwrap().as_interface())
            );
            state.update(activation(&old_save)).unwrap();
            assert_eq!(state.saves, failure + 1);
        }
        let stale = activation(&old_save);
        let mut replacement = valid;
        replacement.name = "layout-only replacement".into();
        replacement.root.children.reverse();
        std::fs::write(&path, replacement.to_json().unwrap()).unwrap();
        let second = state.reload().unwrap().generation;
        assert_ne!(first, second);
        assert!(!state.slot.accepts(first));
        state.update(stale).unwrap();
        assert_eq!(state.saves, 4);
        let current = state.slot.component().unwrap().widget::<ButtonAbi>("save").unwrap();
        state.update(activation(&current)).unwrap();
        assert_eq!(state.saves, 5);
        // A separately live Button is not a member of the published slot.
        let foreign_owner = ui.widgets().button_unobserved("foreign").unwrap();
        state.update(activation(&foreign_owner.handle())).unwrap();
        assert_eq!(state.saves, 5);
        assert_eq!(
            state.slot.component().unwrap().widget::<TextBlockAbi>("title").unwrap().text().unwrap(),
            "Save requested 5 times"
        );
        state.slot.component().unwrap().save(&registry).unwrap();
    }
}
