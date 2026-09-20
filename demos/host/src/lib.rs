//! Concrete demo host over the static facade, with dynamic native and static browser deployments.
//!
//! Application state and handlers remain local. UI implementation, DOM/SDL events and GPU objects
//! are reached only through their checked COM interfaces. Native packages are executable-relative.
#![deny(missing_docs)]
use neocogi_ui::*;
use neocogi_com::{com_object, ComPtr, NcCopyFromBufferOffset, NcStatus};
use std::fmt;

#[cfg(target_arch = "wasm32")]
mod browser;

/// Setup and frame errors preserve either an OS failure, a COM status or a concrete diagnostic.
#[derive(Debug)]
pub enum HostError {
    /// Native package discovery and file access failed.
    Io(std::io::Error),
    /// A checked interface rejected the requested operation.
    Status(NcStatus),
    /// Package validation or native renderer detail is available as owned text.
    Message(String),
}
impl From<std::io::Error> for HostError {
    /// Keeps the original OS error available through Error::source.
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}
impl From<NcStatus> for HostError {
    /// Preserves the exact COM status when no richer native diagnostic is required.
    fn from(error: NcStatus) -> Self {
        Self::Status(error)
    }
}
impl fmt::Display for HostError {
    /// Formats the concrete cause without hiding a second boxed error hierarchy.
    fn fmt(&self, output: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => error.fmt(output),
            Self::Status(error) => error.fmt(output),
            Self::Message(message) => output.write_str(message),
        }
    }
}
impl std::error::Error for HostError {
    /// Native IO errors retain their normal inspection chain.
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            Self::Status(error) => Some(error),
            Self::Message(_) => None,
        }
    }
}

/// Reports a demo's concrete display error and converts its result into a process exit status.
///
/// The native min-size preset deliberately removes generated `Debug` formatting. Rust's blanket
/// `Termination` implementation for `Result` prints errors with `Debug`, which would consequently
/// reduce any failure to a bare `Error:` line. Demo entry points use this explicit display boundary
/// so size optimization cannot discard the operational diagnostic.
/// Browser binaries have no OS stderr: their startup errors go to the JavaScript console. The page
/// separately verifies that successful initialization published a canvas before hiding its status.
pub fn report_exit(result: Result<(), HostError>) -> std::process::ExitCode {
    match result {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(error) => {
            // Display preserves OS messages, COM status codes, and the richer contextual messages
            // collected by the host without depending on generated enum-debug implementations.
            #[cfg(not(target_arch = "wasm32"))]
            eprintln!("Error: {error}");
            #[cfg(target_arch = "wasm32")]
            web_sys::console::error_1(&format!("Demo startup failed: {error}").into());
            std::process::ExitCode::FAILURE
        }
    }
}

/// Ordinary application state consumes one typed message enum and participates in frame updates.
pub trait AppState: 'static {
    /// Application-defined messages never cross the plugin ABI.
    type Message: 'static;
    /// Handles one message after the runtime releases its tree borrows.
    fn update(&mut self, _message: Self::Message) -> Result<(), NcStatus> {
        Ok(())
    }

    /// Observes one ordered native event before UI dispatch, outside the platform call.
    /// Concrete editors use pointer boundaries to group live edits into one undo operation.
    fn input(&mut self, _event: UiInput, _text: &str) -> Result<(), NcStatus> {
        Ok(())
    }

    /// Handles native close intent. False keeps the window alive for an application prompt.
    fn close_requested(&mut self) -> Result<bool, NcStatus> {
        Ok(true)
    }

    /// Runs after platform polling and before this frame's single UI update. Return false to stop.
    /// Input handlers run afterward; work they defer here is applied on the next frame. The host
    /// serializes frame edits and message consumption through ordinary mutable borrows.
    fn frame(&mut self, _host: &Host, _size: Size, _seconds: f64) -> Result<bool, NcStatus> {
        Ok(true)
    }

    /// Runs after input dispatch returns and before painting. Embedded renderers may
    /// safely install resources, render previews, and synchronize paint-only content here.
    fn after_update(&mut self, _host: &Host, _size: Size) -> Result<(), NcStatus> {
        Ok(())
    }

    /// Selects the one native IME session after all UI updates. Embedded UI hosts
    /// may translate their preview's committed rectangle into window coordinates.
    fn text_input_area(&self, host: &Host) -> Result<TextInputAreaAbi, NcStatus> {
        host.ui.text_input_area()
    }
}

/// One demo UI and platform window; these fields are concrete capabilities, with no native handles.
pub struct Host {
    /// Bound core runtime and nodes factory share canonical host identity, not one COM allocation.
    pub ui: Ui,
    /// Selected backend owns all images and custom render items used by this host.
    pub renderer: ComPtr<IRendererAbi>,
    /// OS/canvas polling and native composition share the renderer's COM identity.
    pub window: ComPtr<IPlatformWindowAbi>,
    /// Pending native observations owned independently of any mutable application borrow.
    observations: std::sync::mpsc::Receiver<InputObservation>,
    /// Platform calls only this typed channel bridge; application callbacks run after polling.
    input: ComPtr<IInputSinkAbi>,
}
impl Host {
    /// Loads/registers trusted assemblies once and creates one complete demo host from immutable assets.
    pub fn new(title: &str, size: Size, assets: ComPtr<IAssetBundleAbi>) -> Result<Self, HostError> {
        initialize()?;
        let runtime = neocogi_com::Runtime::global();
        let platform = runtime.activate_plugin_factory::<PlatformAbi>(neocogi_com::guid_from_qualified_name(renderer_plugin_name()))?;
        let atlas = assets.image()?;
        let window = platform.create_window(NcCopyFromBufferOffset::new(0, title.as_bytes()), size, atlas.as_com_ref())?;
        let renderer = window.query()?;
        let gfx = runtime.activate_plugin_factory::<GfxPluginFactoryAbi>(neocogi_com::guid_from_qualified_name("neocogi.gfx.runtime"))?;
        let runtime_factory = runtime.activate_plugin_factory::<UiPluginFactoryAbi>(neocogi_com::guid_from_qualified_name("neocogi.ui.runtime"))?;
        let ui_runtime = runtime_factory.create_runtime(assets.as_com_ref(), gfx.as_com_ref())?;
        let node_provider =
            runtime.activate_plugin_factory::<nodes::NodesPluginFactoryAbi>(neocogi_com::guid_from_qualified_name("neocogi.ui.nodes.runtime"))?;
        let factory = node_provider.bind(ui_runtime.as_com_ref())?;
        let ui = Ui::from_interfaces(ui_runtime, factory)?;
        let (send, observations) = std::sync::mpsc::channel();
        let input = PollInput::from_value(PollInput { send }).query()?;
        Ok(Self {
            ui,
            renderer,
            window,
            input,
            observations,
        })
    }

    /// Owns state and its channel receiver until native close, an application stop request or a frame error.
    /// Browser startup schedules animation and returns; the loop releases its state on stop/error.
    pub fn run<State: AppState>(self, state: State, events: std::sync::mpsc::Receiver<State::Message>) -> Result<(), HostError> {
        let driver = Driver {
            host: self,
            state,
            events,
            remaining: frame_limit()?,
        };
        #[cfg(target_arch = "wasm32")]
        {
            browser::run(driver)
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            let mut driver = driver;
            // Native presentation already applies the backend's pacing policy. An extra sleep
            // caps throughput even when a mailbox swapchain can accept another frame immediately.
            while driver.frame(monotonic_seconds())? {}
            Ok(())
        }
    }

    /// Decodes a complete compressed PNG through the optional provider and uploads to this renderer.
    /// The caller receives the actual immutable renderer image; decoded pixels live only during upload.
    #[cfg(feature = "png-assets")]
    pub fn png_texture(&self, png: &[u8]) -> Result<ComPtr<ITextureAbi>, HostError> {
        let runtime = neocogi_com::Runtime::global();
        let gfx = runtime.activate_plugin_factory::<GfxPluginFactoryAbi>(neocogi_com::guid_from_qualified_name("neocogi.gfx.runtime"))?;
        let decoder = gfx.png_decoder()?;
        let target = self.renderer.query::<ITextureFactoryAbi>()?;
        decoder.upload(NcCopyFromBufferOffset::new(0, png), target.as_com_ref()).map_err(|status| {
            let detail = decoder
                .query::<IGfxDiagnosticsAbi>()
                .and_then(|diagnostics| diagnostics.diagnostic())
                .and_then(|text| neocogi_com::INcStringComPtrExt::to_string(&text))
                .unwrap_or_default();
            HostError::Message(format!("PNG upload failed: {status}: {detail}"))
        })
    }
}
impl Drop for Host {
    /// Logical closure reaches retained widget capabilities before the host's COM references retire.
    fn drop(&mut self) {
        let _ = self.ui.close();
    }
}

/// Owned native input fact copied before the borrowed platform callback returns.
struct InputObservation {
    /// Complete logical input in its original ordering.
    event: UiInput,
    /// Complete UTF-8 commit, empty for nontext observations.
    text: String,
}

#[com_object(interfaces(IInputSink))]
/// A concrete sender bridges foreign polling without Cell/RefCell or borrowed application state.
struct PollInput {
    /// The host exclusively owns the receiver and drains after each platform call.
    send: std::sync::mpsc::Sender<InputObservation>,
}
impl IInputSink for PollInput {
    /// Validates UTF-8 and offset before retaining an owned copy of the native input.
    fn input(&self, event: UiInputAbi, text: NcCopyFromBufferOffset<'_, u8>) -> Result<(), NcStatus> {
        if text.offset() != 0 {
            return Err(neocogi_com::NC_E_INVALID_ARG);
        }
        let event = UiInput::try_from(event).map_err(|_| neocogi_com::NC_E_INVALID_ARG)?;
        let text = std::str::from_utf8(text.as_slice()).map_err(|_| neocogi_com::NC_E_INVALID_ARG)?.to_owned();
        self.send.send(InputObservation { event, text }).map_err(|_| UI_E_CLOSED)
    }
}

/// Concrete loop ownership is shared by the native iteration and browser scheduling adapter.
struct Driver<State: AppState> {
    /// Owns exactly one UI/renderer pair for this application.
    host: Host,
    /// Native state is an ordinary owned value, exclusively borrowed during consumption.
    state: State,
    /// The sole standard receiver retains pending messages independently of application state.
    events: std::sync::mpsc::Receiver<State::Message>,
    /// Optional finite run counts successfully rendered frames, including the final frame.
    remaining: Option<u32>,
}
impl<State: AppState> Driver<State> {
    /// Polls input, applies frame edits, then settles events and layout in one update before rendering.
    fn frame(&mut self, seconds: f64) -> Result<bool, HostError> {
        let host = &self.host;
        loop {
            let status = host.window.poll(host.input.as_com_ref())?;
            for observation in host.observations.try_iter() {
                self.state.input(observation.event, &observation.text)?;
                host.ui.input(observation.event, &observation.text)?;
            }
            match status {
                PollStatus::Event => continue,
                PollStatus::Idle => break,
                PollStatus::Close => {
                    if self.state.close_requested()? {
                        return Ok(false);
                    }
                    break;
                }
                _ => return Err(HostError::Message("unknown platform polling result".to_owned())),
            }
        }
        let size = host.window.size()?;
        if size.width <= 0 || size.height <= 0 {
            return Ok(true);
        }
        // Apply frame edits before consuming the input just polled. Handlers can prepare work for
        // the next frame callback; this frame's direct edits and nested events settle in one update.
        if !self.state.frame(host, size, seconds)? {
            return Ok(false);
        }
        host.ui.update_with_events(size, &self.events, |message| self.state.update(message))?;
        self.state.after_update(host, size)?;
        host.window.text_input(self.state.text_input_area(host)?)?;
        host.ui.render(size, host.renderer.as_com_ref()).map_err(|status| {
            let detail = host
                .renderer
                .query::<IRenderDiagnosticsAbi>()
                .and_then(|diagnostics| diagnostics.diagnostic())
                .and_then(|text| neocogi_com::INcStringComPtrExt::to_string(&text))
                .unwrap_or_default();
            HostError::Message(format!("frame failed: {status}: {detail}"))
        })?;
        if let Some(remaining) = self.remaining {
            self.remaining = Some(remaining - 1);
            return Ok(remaining > 1);
        }
        Ok(true)
    }
}

/// Registers the process plugin catalog before loading assets or constructing windows.
/// Repeated calls reuse the first result; Host::new also calls this for callers supplying their own bundle.
pub fn initialize() -> Result<(), HostError> {
    static STARTUP: std::sync::OnceLock<Result<(), String>> = std::sync::OnceLock::new();
    STARTUP
        .get_or_init(|| register().map_err(|error| error.to_string()))
        .as_ref()
        .map(|_| ())
        .map_err(|error| HostError::Message(error.clone()))
}

/// Loads the dependency-ordered dynamic manifest adjacent to the executable.
#[cfg(all(not(target_arch = "wasm32"), not(feature = "monolith")))]
fn register() -> Result<(), HostError> {
    let executable = std::env::current_exe()?;
    let directory = executable
        .parent()
        .ok_or_else(|| HostError::Message("executable has no parent directory".to_owned()))?;
    let manifest = std::fs::read_to_string(directory.join("plugins.txt"))?;
    let runtime = neocogi_com::Runtime::global();
    for name in manifest.lines() {
        let path = std::path::Path::new(name);
        if path.components().count() != 1 || !matches!(path.components().next(), Some(std::path::Component::Normal(_))) {
            return Err(HostError::Message("plugin manifest entries must be file names".to_owned()));
        }
        // SAFETY: cargo-neocogi stages these trusted workspace assemblies and their matching runtime.
        // Required live activation follows neocogi-com's documented abort policy on invalid binaries.
        unsafe {
            runtime.load_plugin_library(directory.join(path));
        }
    }
    Ok(())
}

/// Registers descriptors linked into a browser module or native monolith.
#[cfg(any(target_arch = "wasm32", feature = "monolith"))]
fn register() -> Result<(), HostError> {
    let runtime = neocogi_com::Runtime::global();
    // SAFETY: The complete generated descriptions, linked contracts, and
    // implementation code remain static for this module's process lifetime.
    unsafe {
        runtime.register_factory_plugin_description(neocogi_ui_runtime::factory_plugin_description())?;
        runtime.register_factory_plugin_description(neocogi_ui_nodes::factory_plugin_description())?;
        runtime.register_factory_plugin_description(neocogi_render_plugin::factory_plugin_description())?;
        #[cfg(any(target_arch = "wasm32", feature = "monolith-gfx"))]
        runtime.register_factory_plugin_description(neocogi_gfx::factory_plugin_description())?;
        #[cfg(feature = "skin")]
        runtime.register_factory_plugin_description(neocogi_skin::factory_plugin_description())?;
    }
    Ok(())
}

/// Returns the qualified identity selected for the separately built renderer assembly.
fn renderer_plugin_name() -> &'static str {
    if cfg!(feature = "vulkan") {
        "neocogi.render.vulkan"
    } else if cfg!(feature = "wgpu") {
        "neocogi.render.wgpu"
    } else if cfg!(feature = "webgl") {
        "neocogi.render.webgl"
    } else {
        // Glow is also the workspace-check fallback when this library is built
        // directly without an application selecting its renderer feature.
        "neocogi.render.glow"
    }
}

/// Opt-in finite demos exercise ordinary input/update/render and teardown without a separate mock loop.
/// Native runs use NEOCOGI_DEMO_FRAMES; browser runs use the `frames` query parameter. Omission runs
/// until close or AppState's stop request; malformed or zero limits fail explicitly at startup.
fn frame_limit() -> Result<Option<u32>, HostError> {
    #[cfg(not(target_arch = "wasm32"))]
    let value = std::env::var("NEOCOGI_DEMO_FRAMES").ok();
    #[cfg(target_arch = "wasm32")]
    let value = web_sys::window()
        .and_then(|window| window.location().search().ok())
        .and_then(|search| web_sys::UrlSearchParams::new_with_str(&search).ok())
        .and_then(|query| query.get("frames"));
    value
        .map(|value| {
            value
                .parse::<u32>()
                .ok()
                .filter(|count| *count > 0)
                .ok_or_else(|| HostError::Message("demo frame limit must be a positive u32".to_owned()))
        })
        .transpose()
}

/// Browser monotonic clock uses seconds, matching application animation and native time units.
#[cfg(target_arch = "wasm32")]
pub fn monotonic_seconds() -> f64 {
    web_sys::window()
        .and_then(|window| window.performance())
        .map(|clock| clock.now() / 1000.0)
        .unwrap_or(0.0)
}
/// Native time starts at the first observation and never depends on wall-clock adjustment.
#[cfg(not(target_arch = "wasm32"))]
pub fn monotonic_seconds() -> f64 {
    static START: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();
    START.get_or_init(std::time::Instant::now).elapsed().as_secs_f64()
}
