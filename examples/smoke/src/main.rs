//! Small native host using the static facade/contracts and runtime binding.
//! Implementations may be loaded from an assembly or linked into one monolith.
use neocogi_ui_contracts::*;
use neocogi_com::{ComPtr, ComRef, NcCopyFromBufferOffset, NcStatus};
use std::{error::Error, time::Duration};
#[cfg(feature = "extension")]
mod extension;
mod gpu;

/// Host-owned behavior persists across every frame and owns its concrete editing capability.
struct ClickModel {
    /// Number of activations delivered so far.
    count: u32,
    /// Ordinary view capability; no stack callback is lent through COM.
    button: ButtonWidget,
}
impl events::MessageProcessor for ClickModel {
    type Message = neocogi_ui_contracts::events::EventMessage;
    /// Uses exact source identity and commits presentation before later input.
    fn handle(&mut self, message: Self::Message) -> Result<(), NcStatus> {
        let Some(origin) = message.origin.widget::<ButtonAbi>()? else {
            return Ok(());
        };
        if !origin.is_alive() {
            return Ok(());
        };
        if !origin.same_object(self.button.as_interface()) {
            return Ok(());
        }
        let Some(_event) = message.query::<IActivatedEventAbi>()? else {
            return Ok(());
        };
        self.count += 1;
        println!("button activated: {}", self.count);
        self.button.set_label(&format!("Clicked {} times", self.count))
    }
}

/// Explicit startup choices; native path arguments retain their OS encoding.
struct Options {
    /// Runs deterministic acceptance and exits after teardown.
    smoke: bool,
    /// Exercises the optional builder by repacking the supplied base catalog.
    rebuild_assets: bool,
    /// Optional JSON theme path, relative to the staged package unless absolute.
    theme: Option<std::path::PathBuf>,
}
impl Options {
    /// Rejects unknown flags and missing path values before loading any plugin.
    fn read() -> Result<Self, Box<dyn Error>> {
        let mut options = Self {
            smoke: false,
            rebuild_assets: false,
            theme: None,
        };
        let mut args = std::env::args_os().skip(1);
        while let Some(arg) = args.next() {
            if arg == "--smoke" {
                options.smoke = true;
            } else if arg == "--rebuild-assets" {
                options.rebuild_assets = true;
            } else if arg == "--theme" {
                options.theme = Some(args.next().ok_or("--theme requires a native path")?.into());
            } else {
                return Err(format!("unknown argument: {}", arg.to_string_lossy()).into());
            }
        }
        Ok(options)
    }
}

/// Initializes the Cargo composition and runs interactively or exercises its smoke gate.
#[neocogi_com::com_main]
fn main(runtime: neocogi_com::Runtime) -> std::process::ExitCode {
    neocogi_demo_host::report_exit(run(runtime))
}

/// Keeps ordinary startup errors intact until the shared Display reporting boundary.
fn run(runtime: neocogi_com::Runtime) -> Result<(), Box<dyn Error>> {
    let options = Options::read()?;
    neocogi_demo_host::initialize(runtime)?;
    let platform = runtime.acquire_plugin_root::<PlatformAbi>(neocogi_com::guid_from_qualified_name(renderer_plugin_name()))?;
    let assets = neocogi_demo_assets::load_bundle(runtime)?;
    let native = platform.create_window(NcCopyFromBufferOffset::new(0, b"NeoCogi UI"), Size { width: 640, height: 400 })?;
    let renderer: ComPtr<IRendererAbi> = native.query()?;
    let gfx = neocogi_ui_contracts::services::gfx(runtime)?;
    let runtime_factory = neocogi_ui_contracts::services::ui(runtime)?;
    let mut ui_runtime = runtime_factory.create_runtime(assets.as_com_ref(), gfx.as_com_ref())?;
    let render_host = native.graphics_host()?;
    let surface = render_host
        .query::<IRenderSurfaceFactoryAbi>()?
        .create_surface(RenderSurfaceDesc::Framebuffer)?;
    ui_runtime.set_render_host(render_host.as_com_ref(), surface.as_com_ref())?;
    let node_provider = neocogi_ui_contracts::services::nodes(runtime)?;
    let node_factory = node_provider.bind(ui_runtime.as_com_ref())?;
    let mut ui = Ui::from_interfaces(ui_runtime, node_factory)?;
    if options.rebuild_assets {
        // The optional builder owns packing/font/PNG implementation; the host sees only typed roots.
        let gfx = neocogi_ui_contracts::services::gfx(runtime)?;
        let loader = gfx
            .asset_loader()
            .map_err(|status| format!("--rebuild-assets requires a files package: {status}"))?;
        let builder = loader.derive_builder(
            Size { width: 1024, height: 512 },
            assets.as_com_ref(),
            AssetDeriveOptions {
                replace_semantic_fonts: false,
                replace_semantic_icons: false,
            },
        )?;
        let rebuilt = builder.finish()?;
        ui.install_assets(rebuilt.as_com_ref())?;
    }
    if let Some(path) = &options.theme {
        // Relative paths belong to the staged package, so startup remains independent of cwd.
        let path = if path.is_absolute() {
            path.clone()
        } else {
            std::env::current_exe()?.parent().ok_or("executable has no parent")?.join(path)
        };
        let gfx = neocogi_ui_contracts::services::gfx(runtime)?;
        let assets_loader = gfx.asset_loader().map_err(|status| format!("--theme requires a GFX files package: {status}"))?;
        let publisher = gfx.asset_publisher().map_err(|status| format!("--theme requires GFX publication: {status}"))?;
        let skin = neocogi_ui_contracts::services::skin(runtime)?;
        let loader = skin
            .theme_loader(assets_loader.as_com_ref(), publisher.as_com_ref())
            .map_err(|status| format!("--theme requires a Skin files package: {status}"))?;
        let theme = loader.load_theme(NativePath::new(&path).as_interface(), assets.as_com_ref()).map_err(|error| {
            let detail = loader
                .query::<ISkinDiagnosticsAbi>()
                .and_then(|diagnostics| diagnostics.diagnostic())
                .and_then(|text| neocogi_com::INcStringComPtrExt::to_string(&text))
                .unwrap_or_default();
            format!("theme load failed: {error}: {detail}")
        })?;
        ui.install_assets(theme.as_com_ref())?;
    }
    let host = ui.widgets().host()?;
    if options.smoke {
        assert_eq!(ui.widgets().host()?.as_raw().object, host.as_raw().object);
    }
    let (queue, session) = neocogi_ui_contracts::events::channel();
    let button = ui.widgets().button_with_queue("Hello across COM", Some(queue.as_com_ref()))?;
    let button_handle = button.handle();
    let window = ui
        .window("Dynamic UI", Rect { x: 40, y: 40, width: 500, height: 200 }, button)
        .map_err(|(status, _candidate)| status)?;
    let mut button = button_handle;
    let app = events::Dispatcher::into_local(events::Dispatcher::new(ClickModel { count: 0, button: button.clone() }, session));
    let owner = app.query::<IApplicationDispatcherAbi>()?;
    let size = native.size()?;
    ui.update_with_events(size, owner.as_com_ref())?;
    ui.render(size, renderer.as_com_ref())?;
    if options.smoke {
        // Keyboard routing must cross the real UI plugin, enqueue, then reach the host sink.
        key(&ui, KeyCode::Tab)?;
        ui.update_with_events(size, owner.as_com_ref())?;
        key(&ui, KeyCode::Enter)?;
        ui.update_with_events(size, owner.as_com_ref())?;
        assert_eq!(app.try_borrow()?.processor.count, 1);
        button.set_label("A longer label changes preferred measurement across the plugin boundary")?;
        assert_eq!(ui.render(size, renderer.as_com_ref()).map_err(|error| error.status), Err(UI_E_UPDATE_REQUIRED));
        ui.update_with_events(size, owner.as_com_ref())?;
        ui.render(size, renderer.as_com_ref())?;
        window.set_rect(Rect { x: 20, y: 20, width: 560, height: 280 })?;
        assert_eq!(ui.render(size, renderer.as_com_ref()).map_err(|error| error.status), Err(UI_E_UPDATE_REQUIRED));
        ui.update_with_events(size, owner.as_com_ref())?;
        ui.render(size, renderer.as_com_ref())?;
        // Restoring the compiled bundle exercises a second real upload and shared UI revision.
        ui.install_assets(assets.as_com_ref())?;
        ui.update_with_events(size, owner.as_com_ref())?;
        ui.render(size, renderer.as_com_ref())?;
        window.close()?;
        assert_eq!(button.set_label("closed"), Err(neocogi_com::NC_E_DROPPED));
        smoke_containers(&ui, size, renderer.as_com_ref())?;
        smoke_components(&ui, size, renderer.as_com_ref())?;
        gpu::smoke(&ui, size, renderer.as_com_ref())?;
        #[cfg(feature = "extension")]
        extension::smoke(runtime, &ui, size, renderer.as_com_ref())?;
        ui.close()?;
        println!(
            "COM smoke passed: dynamic activation, native GPU frame and generic graphics, texture ownership, inherited widgets, composed containers, menus, native-path dialog, input, invalidation, resize, closure"
        );
        return Ok(());
    }
    let input = ui.input_sink()?;
    'frames: loop {
        loop {
            match native.poll(input.as_com_ref())? {
                PollStatus::Event => continue,
                PollStatus::Idle => break,
                PollStatus::Close => break 'frames,
                _ => return Err("unknown platform polling result".into()),
            }
        }
        let size = native.size()?;
        if size.width > 0 && size.height > 0 {
            ui.update_with_events(size, owner.as_com_ref())?;
            native.text_input(ui.text_input_area()?)?;
            ui.render(size, renderer.as_com_ref())?;
        }
        std::thread::sleep(Duration::from_millis(8));
    }
    ui.close()?;
    Ok(())
}

/// Returns the qualified identity selected for this smoke executable's renderer assembly.
fn renderer_plugin_name() -> &'static str {
    if cfg!(feature = "vulkan") {
        "neocogi.platform.vulkan"
    } else if cfg!(feature = "wgpu") {
        "neocogi.platform.wgpu"
    } else {
        "neocogi.platform.glow"
    }
}

/// Injects a complete logical press/release pair without pretending it came from an OS event.
fn key(ui: &Ui, key: KeyCode) -> Result<(), NcStatus> {
    for pressed in [true, false] {
        ui.input(
            UiInput::Key(KeyStroke {
                key,
                action: if pressed {
                    neocogi_ui_contracts::KeyAction::Pressed { repeat: false }
                } else {
                    neocogi_ui_contracts::KeyAction::Released
                },
                modifiers: 0,
            }),
            "",
        )?;
    }
    Ok(())
}

/// Exercises every migrated container through independently compiled factory and layout interfaces.
fn smoke_containers(ui: &Ui, size: Size, renderer: ComRef<'_, IRendererAbi>) -> Result<(), RenderError> {
    let section = ui.widgets().disclosure("Composed editor", DisclosureStyle::Tree, true)?;
    let text = ui
        .widgets()
        .text_area_unobserved(&"Editable native content\n".repeat(60), TextWrapping::Word, true)?;
    let text_handle = text.handle();
    section
        .body()?
        .insert(0, text, LinearEdge::new(LayoutTrack::Fixed { extent: 100 }))
        .map_err(|(status, _candidate)| status)?;
    let mut text = text_handle;
    let section_handle = section.handle();
    let mut grid = ui.widgets().grid(
        &[LayoutTrack::Flex { weight: 1.0 }],
        &[],
        neocogi_ui_contracts::GridLimits { max_columns: 1, max_rows: 2 },
    )?;
    grid.insert(0, section, GridExtent { columns: 1, rows: 1 })
        .map_err(|(status, _candidate)| status)?;
    let mut section = section_handle;
    let tail = ui.widgets().text_block(&"Outer scrolling content\n".repeat(60), TextWrapping::None)?;
    grid.insert(1, tail, GridExtent { columns: 1, rows: 1 })
        .map_err(|(status, _candidate)| status)?;
    let scroll = ui.widgets().scroll_area(grid, true).map_err(|(status, _candidate)| status)?;
    let scroll_handle = scroll.handle();
    let window = ui
        .window("Dynamic containers", Rect { x: 20, y: 20, width: 560, height: 320 }, scroll)
        .map_err(|(status, _candidate)| status)?;
    let mut scroll = scroll_handle;
    ui.update(size)?;
    ui.render(size, renderer)?;
    text.set_cursor(u64::MAX)?;
    ui.update(size)?;
    assert!(text.scrolling()?.info()?.requested.y > 0);
    scroll.scroll_to_end()?;
    ui.update(size)?;
    assert!(scroll.info()?.requested.y > 0);
    ui.render(size, renderer)?;
    section.set_expanded(false)?;
    ui.update(size)?;
    ui.render(size, renderer)?;
    window.close()?;
    assert_eq!(text.text(), Err(neocogi_com::NC_E_DROPPED));
    assert_eq!(section.expanded(), Err(neocogi_com::NC_E_DROPPED));
    Ok(())
}

/// Native smoke owns a newly created empty directory and removes only that directory when finished.
struct SmokeDirectory(std::path::PathBuf);
impl SmokeDirectory {
    /// Creates a unique native path without relying on the executable's working directory.
    fn new() -> Result<Self, std::io::Error> {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock follows Unix epoch")
            .as_nanos();
        let path = std::env::temp_dir().join(format!("neocogi-com-smoke-{}-{stamp}", std::process::id()));
        std::fs::create_dir(&path)?;
        Ok(Self(path))
    }
}
impl Drop for SmokeDirectory {
    /// The test creates no file contents; failure cannot recursively remove unrelated filesystem data.
    fn drop(&mut self) {
        let _ = std::fs::remove_dir(&self.0);
    }
}
/// Queues one concrete key used by the independently compiled menu and standard-dialog implementations.
fn smoke_key(ui: &Ui, key: KeyCode) -> Result<(), NcStatus> {
    ui.input(
        UiInput::Key(KeyStroke {
            key,
            action: neocogi_ui_contracts::KeyAction::Pressed { repeat: false },
            modifiers: 0,
        }),
        "",
    )
}
/// Owned assertion model combines independent public component protocols.
struct ComponentModel {
    /// Completed semantic messages across menu and file-dialog sources.
    count: u32,
    /// Expected exact native identity, retained independently of display text.
    expected_path: std::path::PathBuf,
}
impl events::MessageProcessor for ComponentModel {
    type Message = neocogi_ui_contracts::events::EventMessage;
    /// Checks the captured fact and mutates ordinary host state after traversal.
    fn handle(&mut self, event: Self::Message) -> Result<(), NcStatus> {
        if let Some(origin) = event.origin.service::<IMenuItemAbi>()? {
            if origin.upgrade()?.is_none() {
                return Ok(());
            }
            let Some(_activation) = event.query::<IActivatedEventAbi>()? else {
                return Ok(());
            };
        } else if let Some(origin) = event.origin.widget::<IFileDialogAbi>()? {
            if !origin.is_alive() {
                return Ok(());
            }
            let Some(event) = event.query::<IFileAcceptedEventAbi>()? else {
                return Ok(());
            };
            assert_eq!(neocogi_ui_contracts::path::decode(event.path()?.as_com_ref())?, self.expected_path);
        } else {
            return Ok(());
        }
        self.count += 1;
        Ok(())
    }
}

/// Exercises COM combo painting, compact menu input and the full native-path picker across separate libraries.
fn smoke_components(ui: &Ui, size: Size, renderer: ComRef<'_, IRendererAbi>) -> Result<(), Box<dyn Error>> {
    let directory = SmokeDirectory::new()?;
    let root = ui.widgets().linear(FlowDirection::TopToBottom, CrossSize::Stretch)?;
    let root_handle = root.handle();
    let window = ui
        .window("Dynamic components", Rect { x: 10, y: 10, width: 610, height: 300 }, root)
        .map_err(|(status, _candidate)| status)?;
    let mut root = root_handle;
    let combo = ui.combo_box_unobserved(window.as_com_ref(), &["First", "Selected across COM"], ChoiceSelection::Selected(1))?;
    let combo_handle = combo.handle();
    root.insert(0, combo, LinearEdge::new(LayoutTrack::Content))
        .map_err(|(status, _candidate)| status)?;
    let combo = combo_handle;
    let menus = ui.menus()?;
    let bar = menus.create("")?;
    let (queue, events) = neocogi_ui_contracts::events::channel();
    let item = bar.submenu("File")?.item_with_queue(
        "Menu command",
        Some(queue.query::<neocogi_ui_contracts::events::IEventReceiverAbi>()?.as_com_ref()),
    )?;
    menus.attach_bar(window.as_com_ref(), &bar)?;
    let app = events::Dispatcher::into_local(events::Dispatcher::new(
        ComponentModel {
            count: 0,
            expected_path: directory.0.join(" native file "),
        },
        events,
    ));
    let owner = app.query::<IApplicationDispatcherAbi>()?;
    ui.update_with_events(size, owner.as_com_ref())?;
    ui.render(size, renderer)?;
    for key in [KeyCode::Function(10), KeyCode::ArrowDown, KeyCode::Enter] {
        smoke_key(ui, key)?;
    }
    ui.update_with_events(size, owner.as_com_ref())?;
    assert_eq!(app.try_borrow()?.processor.count, 1);
    let mut picker = ui.file_dialog_with_queue(
        window.as_com_ref(),
        Some(queue.query::<neocogi_ui_contracts::events::IEventReceiverAbi>()?.as_com_ref()),
    )?;
    let mut request = FileDialogRequest::new(&directory.0);
    request.rect = Rect { x: 10, y: 10, width: 620, height: 370 };
    picker.open(&request)?;
    ui.update_with_events(size, owner.as_com_ref())?;
    ui.render(size, renderer)?;
    for _ in 0..6 {
        smoke_key(ui, KeyCode::Tab)?;
    }
    ui.input(UiInput::Text, " native file ")?;
    for _ in 0..2 {
        smoke_key(ui, KeyCode::Tab)?;
    }
    smoke_key(ui, KeyCode::Space)?;
    ui.update_with_events(size, owner.as_com_ref())?;
    assert_eq!(app.try_borrow()?.processor.count, 2);
    assert_eq!(picker.active(), Ok(false));
    ui.render(size, renderer)?;
    picker.close()?;
    window.close()?;
    ui.update_with_events(size, owner.as_com_ref())?;
    assert_eq!(combo.selection(), Err(neocogi_com::NC_E_DROPPED));
    assert_eq!(item.label(), Err(UI_E_CLOSED));
    Ok(())
}
