//! Small native host using the static facade/contracts and runtime binding.
//! Implementations may be loaded from an assembly or linked into one monolith.
use neocogi_ui::*;
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
impl events::EventModel for ClickModel {
    type Message = String;
    /// Uses the captured label and commits the next presentation before later input.
    fn handle(&mut self, label: String) -> Result<(), NcStatus> {
        self.count += 1;
        println!("button submitted: {label}");
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

/// Loads the dependency-ordered dynamic plugin manifest from the assembly.
#[cfg(not(feature = "monolith"))]
fn register_plugins(runtime: &neocogi_com::Runtime, directory: &std::path::Path) -> Result<(), Box<dyn Error>> {
    let manifest = std::fs::read_to_string(directory.join("plugins.txt"))?;
    for name in manifest.lines() {
        // The package manifest contains file names only, keeping all loads in the staged directory.
        let path = std::path::Path::new(name);
        if path.components().count() != 1 || !matches!(path.components().next(), Some(std::path::Component::Normal(_))) {
            return Err("plugin manifest entry must be a file name".into());
        }
        // SAFETY: The build driver stages these trusted, matching workspace plugin assemblies.
        let module = unsafe { runtime.load_plugin_library(directory.join(path)) };
        println!("loaded {}", module.qualified_name());
    }
    Ok(())
}

/// Registers implementation descriptors retained by the monolithic executable.
#[cfg(feature = "monolith")]
fn register_plugins(runtime: &neocogi_com::Runtime, _directory: &std::path::Path) -> Result<(), Box<dyn Error>> {
    use neocogi_demo_host::static_plugins::{self, StaticPlugin};
    let available = [
        StaticPlugin { package: "neocogi-ui-runtime", description: nc_ui::factory_plugin_description() },
        StaticPlugin { package: "neocogi-ui-nodes", description: nc_ui_nodes::factory_plugin_description() },
        StaticPlugin { package: "neocogi-render-plugin", description: nc_render::factory_plugin_description() },
        StaticPlugin { package: "neocogi-gfx", description: nc_gfx::factory_plugin_description() },
        StaticPlugin { package: "neocogi-skin", description: nc_skin::factory_plugin_description() },
        #[cfg(feature = "extension")]
        StaticPlugin { package: "neocogi-example-ui-extension", description: nc_ui_extension::factory_plugin_description() },
    ];
    let selected = static_plugins::select(&available, option_env!("NEOCOGI_ASSEMBLY_PLUGINS"))?;
    for plugin in selected {
        // SAFETY: Every generated descriptor and implementation remains linked for process lifetime.
        unsafe { runtime.register_factory_plugin_description(plugin.description)?; }
    }
    Ok(())
}

/// Loads the staged deployment and either runs interactively or exercises its smoke gate.
fn main() -> Result<(), Box<dyn Error>> {
    let options = Options::read()?;
    let executable = std::env::current_exe()?;
    let directory = executable.parent().ok_or("executable has no parent")?;
    let runtime = neocogi_com::Runtime::global();
    register_plugins(runtime, directory)?;
    let platform = runtime.activate_plugin_factory::<PlatformAbi>(neocogi_com::guid_from_qualified_name(renderer_plugin_name()))?;
    let assets = neocogi_demo_assets::load_bundle()?;
    let atlas = assets.image()?;
    let native = platform.create_window(
        NcCopyFromBufferOffset::new(0, b"NeoCogi UI"),
        Size { width: 640, height: 400 },
        atlas.as_com_ref(),
    )?;
    let renderer: ComPtr<IRendererAbi> = native.query()?;
    let gfx = runtime.activate_plugin_factory::<GfxPluginFactoryAbi>(neocogi_com::guid_from_qualified_name("neocogi.gfx.runtime"))?;
    let runtime_factory = runtime.activate_plugin_factory::<UiPluginFactoryAbi>(neocogi_com::guid_from_qualified_name("neocogi.ui.runtime"))?;
    let ui_runtime = runtime_factory.create_runtime(assets.as_com_ref(), gfx.as_com_ref())?;
    let node_provider = runtime.activate_plugin_factory::<nodes::NodesPluginFactoryAbi>(neocogi_com::guid_from_qualified_name("neocogi.ui.nodes.runtime"))?;
    let node_factory = node_provider.bind(ui_runtime.as_com_ref())?;
    let mut ui = Ui::from_interfaces(ui_runtime, node_factory)?;
    if options.rebuild_assets {
        // The optional builder owns packing/font/PNG implementation; the host sees only typed roots.
        let gfx = runtime.activate_plugin_factory::<GfxPluginFactoryAbi>(neocogi_com::guid_from_qualified_name("neocogi.gfx.runtime"))?;
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
        ui.install_assets(rebuilt.as_com_ref(), renderer.as_com_ref())?;
    }
    if let Some(path) = &options.theme {
        // Relative paths belong to the staged package, so startup remains independent of cwd.
        let path = if path.is_absolute() { path.clone() } else { directory.join(path) };
        let gfx = runtime.activate_plugin_factory::<GfxPluginFactoryAbi>(neocogi_com::guid_from_qualified_name("neocogi.gfx.runtime"))?;
        let assets_loader = gfx.asset_loader().map_err(|status| format!("--theme requires a GFX files package: {status}"))?;
        let publisher = gfx.asset_publisher().map_err(|status| format!("--theme requires GFX publication: {status}"))?;
        let skin = runtime.activate_plugin_factory::<SkinPluginFactoryAbi>(neocogi_com::guid_from_qualified_name("neocogi.skin.runtime"))?;
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
        ui.install_assets(theme.as_com_ref(), renderer.as_com_ref())?;
    }
    let host = ui.widgets().host()?;
    if options.smoke {
        assert_eq!(ui.widgets().host()?.as_raw().object, host.as_raw().object);
    }
    let (session_tx, session) = std::sync::mpsc::channel();
    let mut button = ui
        .widgets()
        .button("Hello across COM", session_tx.clone(), |neocogi_ui::button::Event::Activated(event)| {
            event.label
        })?;
    let window = ui.window("Dynamic UI", Rect { x: 40, y: 40, width: 500, height: 200 }, &button)?;
    let app = events::Controller::into_local(events::Controller::root(ClickModel { count: 0, button: button.clone() }, session));
    let owner = app.query::<IEventOwnerAbi>()?;
    let size = native.size()?;
    ui.update_with_events(size, owner.as_com_ref())?;
    ui.render(size, renderer.as_com_ref())?;
    if options.smoke {
        // Keyboard routing must cross the real UI plugin, enqueue, then reach the host sink.
        key(&ui, KeyCode::Tab)?;
        ui.update_with_events(size, owner.as_com_ref())?;
        key(&ui, KeyCode::Enter)?;
        ui.update_with_events(size, owner.as_com_ref())?;
        assert_eq!(app.try_borrow()?.model.count, 1);
        button.set_label("A longer label changes preferred measurement across the plugin boundary")?;
        assert_eq!(ui.render(size, renderer.as_com_ref()), Err(UI_E_UPDATE_REQUIRED));
        ui.update_with_events(size, owner.as_com_ref())?;
        ui.render(size, renderer.as_com_ref())?;
        window.set_rect(Rect { x: 20, y: 20, width: 560, height: 280 })?;
        assert_eq!(ui.render(size, renderer.as_com_ref()), Err(UI_E_UPDATE_REQUIRED));
        ui.update_with_events(size, owner.as_com_ref())?;
        ui.render(size, renderer.as_com_ref())?;
        // Restoring the compiled bundle exercises a second real upload and shared UI revision.
        ui.install_assets(assets.as_com_ref(), renderer.as_com_ref())?;
        ui.update_with_events(size, owner.as_com_ref())?;
        ui.render(size, renderer.as_com_ref())?;
        window.close()?;
        assert_eq!(button.set_label("closed"), Err(UI_E_CLOSED));
        smoke_containers(&ui, size, renderer.as_com_ref())?;
        smoke_components(&ui, size, renderer.as_com_ref())?;
        gpu::smoke(&ui, size, renderer.as_com_ref())?;
        #[cfg(feature = "extension")]
        extension::smoke(&ui, size, renderer.as_com_ref())?;
        ui.close()?;
        println!(
            "COM smoke passed: dynamic activation, native GPU frame and 3D mesh, texture ownership, inherited widgets, composed containers, menus, native-path dialog, input, invalidation, resize, closure"
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
        "neocogi.render.vulkan"
    } else if cfg!(feature = "wgpu") {
        "neocogi.render.wgpu"
    } else {
        "neocogi.render.glow"
    }
}

/// Injects a complete logical press/release pair without pretending it came from an OS event.
fn key(ui: &Ui, key: KeyCode) -> Result<(), NcStatus> {
    for pressed in [true, false] {
        ui.input(
            UiInput::Key(KeyStroke {
                key,
                value: 0,
                pressed,
                repeat: false,
                modifiers: 0,
            }),
            "",
        )?;
    }
    Ok(())
}

/// Exercises every migrated container through independently compiled factory and layout interfaces.
fn smoke_containers(ui: &Ui, size: Size, renderer: ComRef<'_, IRendererAbi>) -> Result<(), NcStatus> {
    let mut section = ui.widgets().disclosure("Composed editor", DisclosureStyle::Tree, true)?;
    let mut text = ui
        .widgets()
        .text_area_unobserved(&"Editable native content\n".repeat(60), TextWrapping::Word, true)?;
    section
        .body()?
        .insert(0, &text, LinearEdge::new(LayoutTrack::Fixed { extent: 100 }))?
        .finish()?;
    let mut grid = ui.widgets().grid(&[LayoutTrack::Flex { weight: 1.0 }], &[])?;
    grid.insert(0, &section, GridExtent { columns: 1, rows: 1 })?.finish()?;
    let tail = ui.widgets().text_block(&"Outer scrolling content\n".repeat(60), TextWrapping::None)?;
    grid.insert(1, &tail, GridExtent { columns: 1, rows: 1 })?.finish()?;
    let mut scroll = ui.widgets().scroll_area(&grid, true)?;
    let window = ui.window("Dynamic containers", Rect { x: 20, y: 20, width: 560, height: 320 }, &scroll)?;
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
    assert_eq!(text.text(), Err(UI_E_CLOSED));
    assert_eq!(section.expanded(), Err(UI_E_CLOSED));
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
fn smoke_key(ui: &Ui, key: KeyCode, value: u32) -> Result<(), NcStatus> {
    ui.input(
        UiInput::Key(KeyStroke {
            key,
            value,
            pressed: true,
            repeat: false,
            modifiers: 0,
        }),
        "",
    )
}
/// Smoke application facts from independent menu and completion protocols.
enum ComponentMessage {
    /// Captured menu activation label.
    Menu(events::Activation),
    /// Exact native file completion.
    File(FileDialogCompletion),
}
/// Owned assertion model combines independent public component protocols.
struct ComponentModel {
    /// Completed semantic messages across menu and file-dialog sources.
    count: u32,
    /// Expected exact native identity, retained independently of display text.
    expected_path: std::path::PathBuf,
}
impl events::EventModel for ComponentModel {
    type Message = ComponentMessage;
    /// Checks the captured fact and mutates ordinary host state after traversal.
    fn handle(&mut self, message: ComponentMessage) -> Result<(), NcStatus> {
        match message {
            ComponentMessage::Menu(event) => assert_eq!(event.label, "Menu command"),
            ComponentMessage::File(event) => assert_eq!(events::completed_file_path(&event)?.as_deref(), Some(self.expected_path.as_path())),
        }
        self.count += 1;
        Ok(())
    }
}

/// Exercises COM combo painting, compact menu input and the full native-path picker across separate libraries.
fn smoke_components(ui: &Ui, size: Size, renderer: ComRef<'_, IRendererAbi>) -> Result<(), Box<dyn Error>> {
    let directory = SmokeDirectory::new()?;
    let mut root = ui.widgets().linear(FlowDirection::TopToBottom, CrossSize::Stretch)?;
    let window = ui.window("Dynamic components", Rect { x: 10, y: 10, width: 610, height: 300 }, &root)?;
    let combo = ui.combo_box_unobserved(window.as_com_ref(), &["First", "Selected across COM"], ChoiceSelection::Selected(1))?;
    root.insert(0, &combo, LinearEdge::new(LayoutTrack::Content))?.finish()?;
    let menus = ui.menus()?;
    let bar = menus.create("")?;
    let item = bar.submenu("File")?.item("Menu command")?;
    menus.attach_bar(window.as_com_ref(), &bar)?;
    let (events_tx, events) = std::sync::mpsc::channel();
    neocogi_ui::events::bind(item.activated()?, events_tx.clone(), ComponentMessage::Menu)?;
    let app = events::Controller::into_local(events::Controller::root(
        ComponentModel {
            count: 0,
            expected_path: directory.0.join(" native file "),
        },
        events,
    ));
    let owner = app.query::<IEventOwnerAbi>()?;
    ui.update_with_events(size, owner.as_com_ref())?;
    ui.render(size, renderer)?;
    for (key, value) in [(KeyCode::Function, 10), (KeyCode::ArrowDown, 0), (KeyCode::Enter, 0)] {
        smoke_key(ui, key, value)?;
    }
    ui.update_with_events(size, owner.as_com_ref())?;
    assert_eq!(app.try_borrow()?.model.count, 1);
    let mut picker = ui.file_dialog(window.as_com_ref(), events_tx.clone(), |neocogi_ui::file_dialog::Event::Completed(event)| {
        ComponentMessage::File(event)
    })?;
    let mut request = FileDialogRequest::new(&directory.0);
    request.rect = Rect { x: 10, y: 10, width: 620, height: 370 };
    picker.open(&request)?;
    ui.update_with_events(size, owner.as_com_ref())?;
    ui.render(size, renderer)?;
    for _ in 0..6 {
        smoke_key(ui, KeyCode::Tab, 0)?;
    }
    ui.input(UiInput::Text, " native file ")?;
    for _ in 0..2 {
        smoke_key(ui, KeyCode::Tab, 0)?;
    }
    smoke_key(ui, KeyCode::Space, 0)?;
    ui.update_with_events(size, owner.as_com_ref())?;
    assert_eq!(app.try_borrow()?.model.count, 2);
    assert_eq!(picker.active(), Ok(false));
    ui.render(size, renderer)?;
    picker.close()?;
    window.close()?;
    ui.update_with_events(size, owner.as_com_ref())?;
    assert_eq!(combo.selection(), Err(UI_E_CLOSED));
    assert_eq!(item.label(), Err(UI_E_CLOSED));
    Ok(())
}
