//! Small native host using the static facade/contracts and runtime binding.
//! Both UI and native GPU implementation are loaded from the packaged executable directory.
use neocogi_ui::*;
use neocogi_com::{ComPtr, ComRef, NcCopyFromBufferOffset, NcStatus};
use std::{error::Error, time::Duration};
#[cfg(feature = "extension")]
mod extension;
mod gpu;

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

/// Concrete application handler owns only the button capability whose label it updates.
struct Submitted {
    /// A producer never owns this handler, so retaining its editing capability cannot create a cycle.
    button: ButtonWidget,
}
impl EventHandler<u32, events::Activation> for Submitted {
    /// Edits the actual button during the safe event boundary; the same input transaction commits its new layout.
    fn handle(&mut self, count: &mut u32, event: &events::Activation) -> Result<(), NcStatus> {
        *count += 1;
        println!("button submitted: {}", event.label);
        self.button.set_label(&format!("Clicked {} times", *count))
    }
}

/// Loads the staged deployment and either runs interactively or exercises its smoke gate.
fn main() -> Result<(), Box<dyn Error>> {
    let options = Options::read()?;
    let executable = std::env::current_exe()?;
    let directory = executable.parent().ok_or("executable has no parent")?;
    let runtime = neocogi_com::Runtime::global();
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
    // Optional class availability is a recoverable startup check, before constructing an OS window.
    if options.rebuild_assets {
        runtime
            .require_interface_type::<AssetLoaderAbi>(AssetLoaderAbi::CLASS_ID)
            .map_err(|status| format!("--rebuild-assets requires a files package: {status}"))?;
    }
    if options.theme.is_some() {
        runtime
            .require_interface_type::<ThemeLoaderAbi>(ThemeLoaderAbi::CLASS_ID)
            .map_err(|status| format!("--theme requires a files package: {status}"))?;
    }
    let platform: ComPtr<PlatformAbi> = runtime.construct(())?;
    let assets = neocogi_demo_assets::load_bundle()?;
    let native = platform.create_window(
        NcCopyFromBufferOffset::new(0, b"NeoCogi Platform"),
        Size { width: 640, height: 400 },
        assets.as_com_ref(),
    )?;
    let renderer: ComPtr<IRendererAbi> = native.query()?;
    let arguments = UiRuntimeAbi::arguments(assets.as_com_ref())?;
    let mut ui = Ui::from_interface(runtime.construct::<UiRuntimeAbi>(arguments)?)?;
    if options.rebuild_assets {
        // The optional builder owns packing/font/PNG implementation; the host sees only typed roots.
        // SAFETY: The packaged asset loader declares a no-argument constructor.
        let loader: ComPtr<AssetLoaderAbi> = runtime.construct(())?;
        let builder = loader.derive_builder(Size { width: 1024, height: 512 }, assets.as_com_ref())?;
        let rebuilt = builder.finish()?;
        ui.install_assets(rebuilt.as_com_ref(), renderer.as_com_ref())?;
    }
    if let Some(path) = &options.theme {
        // Relative paths belong to the staged package, so startup remains independent of cwd.
        let path = if path.is_absolute() { path.clone() } else { directory.join(path) };
        // SAFETY: The optional theme class declares a no-argument constructor.
        let loader: ComPtr<ThemeLoaderAbi> = runtime.construct(())?;
        let theme = loader.load_theme(NativePath::new(&path).as_interface(), assets.as_com_ref()).map_err(|error| {
            let detail = loader
                .query::<IAssetDiagnosticsAbi>()
                .and_then(|diagnostics| diagnostics.diagnostic())
                .and_then(|text| neocogi_com::INcStringComPtrExt::to_string(&text))
                .unwrap_or_default();
            format!("theme load failed: {error}: {detail}")
        })?;
        ui.install_assets(theme.as_com_ref(), renderer.as_com_ref())?;
    }
    // The host can construct the registered Button class directly. UiFactory remains an
    // optional dynamically linked shorthand for callers that prefer its convenience methods.
    let host = ui.widgets().host()?;
    if options.smoke {
        // UiFactory is itself a registered dynamic class, not a runtime interface alias.
        let factory_arguments = UiFactoryAbi::arguments(host.as_com_ref())?;
        let factory = runtime.construct::<UiFactoryAbi>(factory_arguments)?;
        assert_eq!(factory.host()?.as_ptr(), host.as_ptr());
    }
    let button_arguments = ButtonAbi::arguments(host.as_com_ref(), NcCopyFromBufferOffset::new(0, b"Hello across COM"), WidgetOptions::FRAME)?;
    let mut button = ButtonWidget::from_interface(runtime.construct::<ButtonAbi>(button_arguments)?);
    let window = ui.window("Dynamic UI", Rect { x: 40, y: 40, width: 500, height: 200 }, &button)?;
    let session = EventSession::new(0u32);
    let subscription = session.subscribe_activation(&button, Submitted { button: button.clone() })?;
    let size = native.size()?;
    ui.update_with_events(size, &session)?;
    ui.render(size, renderer.as_com_ref())?;
    if options.smoke {
        // Keyboard routing must cross the real UI plugin, enqueue, then reach the host sink.
        key(&ui, KeyCode::TAB)?;
        ui.update_with_events(size, &session)?;
        key(&ui, KeyCode::ENTER)?;
        ui.update_with_events(size, &session)?;
        assert_eq!(*session.state()?, 1);
        button.set_label("A longer label changes preferred measurement across the plugin boundary")?;
        assert_eq!(ui.render(size, renderer.as_com_ref()), Err(UI_E_UPDATE_REQUIRED));
        ui.update_with_events(size, &session)?;
        ui.render(size, renderer.as_com_ref())?;
        window.set_rect(Rect { x: 20, y: 20, width: 560, height: 280 })?;
        assert_eq!(ui.render(size, renderer.as_com_ref()), Err(UI_E_UPDATE_REQUIRED));
        ui.update_with_events(size, &session)?;
        ui.render(size, renderer.as_com_ref())?;
        // Restoring the compiled bundle exercises a second real upload and shared UI revision.
        ui.install_assets(assets.as_com_ref(), renderer.as_com_ref())?;
        ui.update_with_events(size, &session)?;
        ui.render(size, renderer.as_com_ref())?;
        window.close()?;
        assert_eq!(button.set_label("closed"), Err(UI_E_CLOSED));
        subscription.cancel()?;
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
                PollStatus::EVENT => continue,
                PollStatus::IDLE => break,
                PollStatus::CLOSE => break 'frames,
                _ => return Err("unknown platform polling result".into()),
            }
        }
        let size = native.size()?;
        if size.width > 0 && size.height > 0 {
            ui.update_with_events(size, &session)?;
            native.text_input(ui.text_input_area()?)?;
            ui.render(size, renderer.as_com_ref())?;
        }
        std::thread::sleep(Duration::from_millis(8));
    }
    subscription.cancel()?;
    ui.close()?;
    Ok(())
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
    let mut section = ui.widgets().disclosure("Composed editor", DisclosureStyle::TREE, true)?;
    let mut text = ui.widgets().text_area(&"Editable native content\n".repeat(60), TextWrapping::WORD, true)?;
    section
        .body()?
        .insert(0, &text, LinearEdge::new(LayoutTrack::Fixed { extent: 100 }))?
        .finish()?;
    let mut grid = ui.widgets().grid(&[LayoutTrack::Flex { weight: 1.0 }], &[])?;
    grid.insert(0, &section, GridExtent { columns: 1, rows: 1 })?.finish()?;
    let tail = ui.widgets().text_block(&"Outer scrolling content\n".repeat(60), TextWrapping::NONE)?;
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
/// Concrete native smoke consumer verifies the compact row's captured label.
struct MenuSubmission;
impl EventHandler<u32, events::Activation> for MenuSubmission {
    /// Actual native keyboard routing produces this snapshot across the UI library boundary.
    fn handle(&mut self, count: &mut u32, event: &events::Activation) -> Result<(), NcStatus> {
        assert_eq!(event.label, "Menu command");
        *count += 1;
        Ok(())
    }
}
/// Concrete native completion handler carries the exact expected path as explicit owned data.
struct FileSelection(std::path::PathBuf);
impl EventHandler<u32, events::FileDialogCompleted> for FileSelection {
    /// The static adapter reconstructs native identity before invoking this local handler.
    fn handle(&mut self, count: &mut u32, event: &events::FileDialogCompleted) -> Result<(), NcStatus> {
        assert_eq!(event, &events::FileDialogCompleted::Accepted(self.0.clone()));
        *count += 1;
        Ok(())
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
/// Exercises COM combo painting, compact menu input and the full native-path picker across separate libraries.
fn smoke_components(ui: &Ui, size: Size, renderer: ComRef<'_, IRendererAbi>) -> Result<(), Box<dyn Error>> {
    let directory = SmokeDirectory::new()?;
    let mut combo = ui.widgets().combo()?;
    combo.set_selection(1, "Selected across COM")?;
    let window = ui.window("Dynamic components", Rect { x: 10, y: 10, width: 610, height: 300 }, &combo)?;
    let menus = ui.menus()?;
    let bar = menus.create("")?;
    let item = bar.submenu("File")?.item("Menu command")?;
    menus.attach_bar(window.as_com_ref(), &bar)?;
    let events = EventSession::new(0u32);
    events.subscribe_menu_item(&item, MenuSubmission)?;
    ui.update_with_events(size, &events)?;
    ui.render(size, renderer)?;
    for (key, value) in [(KeyCode::FUNCTION, 10), (KeyCode::ARROW_DOWN, 0), (KeyCode::ENTER, 0)] {
        smoke_key(ui, key, value)?;
    }
    ui.update_with_events(size, &events)?;
    assert_eq!(*events.state()?, 1);
    let picker = ui.file_dialog(window.as_com_ref())?;
    events.subscribe_file_dialog(&picker, FileSelection(directory.0.join(" native file ")))?;
    let mut request = FileDialogRequest::new(&directory.0);
    request.rect = Rect { x: 10, y: 10, width: 620, height: 370 };
    picker.open(&request)?;
    ui.update_with_events(size, &events)?;
    ui.render(size, renderer)?;
    for _ in 0..6 {
        smoke_key(ui, KeyCode::TAB, 0)?;
    }
    ui.input(UiInput::Text, " native file ")?;
    for _ in 0..2 {
        smoke_key(ui, KeyCode::TAB, 0)?;
    }
    smoke_key(ui, KeyCode::SPACE, 0)?;
    ui.update_with_events(size, &events)?;
    assert_eq!(*events.state()?, 2);
    assert_eq!(picker.active(), Ok(false));
    ui.render(size, renderer)?;
    picker.close()?;
    window.close()?;
    ui.update_with_events(size, &events)?;
    assert_eq!(combo.label(), Err(UI_E_CLOSED));
    assert_eq!(item.label(), Err(UI_E_CLOSED));
    Ok(())
}
