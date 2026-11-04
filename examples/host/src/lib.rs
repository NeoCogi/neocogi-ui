//! Concrete demo host over the static facade, with dynamic native and static browser deployments.
//!
//! Application state and handlers remain local. UI implementation, DOM/SDL events and GPU objects
//! are reached only through their checked COM interfaces. Native packages are executable-relative.
#![deny(missing_docs)]
use neocogi_ui::*;
use neocogi_ui::events::ApplyOutcome;
use neocogi_com::{com_object, ComPtr, NcCopyFromBufferOffset, NcStatus};
use std::fmt;

/// Owns browser animation callbacks and releases their platform session on stop or failure.
#[cfg(target_arch = "wasm32")]
mod browser;

/// Setup and frame errors preserve either an OS failure, a COM status or a concrete diagnostic.
#[derive(Debug)]
pub enum HostError {
    /// Native package discovery and file access failed.
    Io(std::io::Error),
    /// A checked interface rejected the requested operation.
    Status(NcStatus),
    /// One frame operation failed with its original checked-interface status.
    Operation {
        /// Static host phase identifies the rejecting boundary without retaining application state.
        operation: &'static str,
        /// Exact cause remains available through Error::source.
        status: NcStatus,
        /// Bounded text owned by this operation, independent of subsequent calls or device lifetime.
        detail: Option<String>,
    },
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
            Self::Operation { operation, status, detail } => {
                write!(output, "{operation} failed: {status}")?;
                if let Some(detail) = detail {
                    write!(output, ": {detail}")?;
                }
                Ok(())
            }
            Self::Message(message) => output.write_str(message),
        }
    }
}
impl std::error::Error for HostError {
    /// IO and checked-interface errors retain their original cause, including contextual statuses.
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            Self::Status(error) => Some(error),
            Self::Operation { status, .. } => Some(status),
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
pub fn report_exit<E: fmt::Display>(result: Result<(), E>) -> std::process::ExitCode {
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
    /// Polling may run ahead of routing; use input_boundary for per-command ownership.
    fn input(&mut self, _event: UiInput, _text: &str) -> Result<(), NcStatus> {
        Ok(())
    }

    /// Observes one consumed UI input scope and its settled descendant messages, outside traversal.
    /// Raw input polling can run ahead; use these ordered markers to associate authoring commands.
    /// Record bookkeeping only here. Begin/End remain paired after a failed input attempt.
    fn input_boundary(&mut self, _boundary: InputBoundary) -> Result<(), NcStatus> {
        Ok(())
    }

    /// Handles native close intent. False keeps the window alive for an application prompt.
    fn close_requested(&mut self) -> Result<bool, NcStatus> {
        Ok(true)
    }

    /// Runs after platform polling and before this frame's single UI update. Return false to stop.
    /// UI input handlers run afterward. Apply deferred publication through apply_pending_changes
    /// within the same update. The host serializes frame edits and message consumption through
    /// ordinary mutable borrows.
    fn frame(&mut self, _host: &Host, _size: Size, _seconds: f64) -> Result<bool, NcStatus> {
        Ok(true)
    }

    /// Applies coalesced pending changes when the application inbox is empty or disconnected.
    /// May run several times per update, including before the first queued input. An idle call
    /// returns NothingPending; consuming work returns ChangesApplied so child work is revisited.
    /// The host/runtime retains no traversal borrow here. Geometry edits still need their ordinary
    /// notifications. Errors preserve committed edits; native rendering belongs in after_update.
    fn apply_pending_changes(&mut self, _host: &Host, _size: Size) -> Result<ApplyOutcome, NcStatus> {
        Ok(ApplyOutcome::NothingPending)
    }

    /// Runs after input dispatch returns and before painting. Embedded renderers may
    /// safely install resources, render previews, and synchronize paint-only content here.
    fn after_update(&mut self, _host: &Host, _size: Size) -> Result<(), NcStatus> {
        Ok(())
    }

    /// Selects the one native IME session after all UI updates. Embedded UI hosts
    /// may translate their preview's committed rectangle into window coordinates.
    fn text_input_area(&self, host: &Host) -> Result<TextInputArea, NcStatus> {
        host.ui.text_input_area()
    }
}

/// One demo UI and platform window; these fields are concrete capabilities, with no native handles.
pub struct Host {
    /// Supplied access to the executable-owned COM services; copying it owns no registry.
    pub runtime: neocogi_com::Runtime,
    /// Bound core runtime and nodes factory share canonical host identity, not one COM allocation.
    pub ui: Ui,
    /// Selected backend owns all images and custom render items used by this host.
    pub renderer: ComPtr<IRendererAbi>,
    /// Explicit framebuffer view shared by UI and application-created resource groups.
    pub surface: ComPtr<IRenderSurfaceAbi>,
    /// OS/canvas polling and native composition share the renderer's COM identity.
    pub window: ComPtr<IPlatformWindowAbi>,
    /// Pending native observations owned independently of any mutable application borrow.
    observations: std::sync::mpsc::Receiver<InputObservation>,
    /// Platform calls only this typed channel bridge; application callbacks run after polling.
    input: ComPtr<IInputSinkAbi>,
}
impl Host {
    /// Context for status-only UI/input operations carries no unrelated renderer text.
    fn frame_error(&self, phase: &'static str, status: NcStatus) -> HostError {
        HostError::Operation { operation: phase, status, detail: None }
    }

    /// Loads/registers trusted assemblies once and creates one complete demo host from immutable assets.
    pub fn new(runtime: neocogi_com::Runtime, title: &str, size: Size, assets: ComPtr<IAssetBundleAbi>) -> Result<Self, HostError> {
        initialize(runtime)?;
        let platform = runtime.acquire_plugin_root::<PlatformAbi>(neocogi_com::guid_from_qualified_name(renderer_plugin_name()))?;
        let window = platform.create_window(NcCopyFromBufferOffset::new(0, title.as_bytes()), size)?;
        let renderer = window.query()?;
        let gfx = runtime.acquire_plugin_root::<GfxPluginFactoryAbi>(neocogi_com::guid_from_qualified_name("neocogi.gfx.runtime"))?;
        let runtime_factory = runtime.acquire_plugin_root::<UiPluginFactoryAbi>(neocogi_com::guid_from_qualified_name("neocogi.ui.runtime"))?;
        let mut ui_runtime = runtime_factory.create_runtime(assets.as_com_ref(), gfx.as_com_ref())?;
        ui_runtime.set_clipboard(window.clipboard()?.as_com_ref())?;
        let render_host = window.graphics_host()?;
        let surface = render_host.query::<IRenderSurfaceFactoryAbi>()?.create_surface(RenderSurfaceDesc {
            kind: RenderSurfaceKind::Framebuffer,
            size: Size::default(),
        })?;
        ui_runtime.set_render_host(render_host.as_com_ref(), surface.as_com_ref())
            .map_err(|error| render_error("UI renderer setup", error))?;
        let node_provider = runtime.acquire_plugin_root::<nodes::NodesPluginFactoryAbi>(neocogi_com::guid_from_qualified_name("neocogi.ui.nodes.runtime"))?;
        let factory = node_provider.bind(ui_runtime.as_com_ref())?;
        let ui = Ui::from_interfaces(ui_runtime, factory)?;
        let (send, observations) = std::sync::mpsc::channel();
        let input = PollInput::from_value(PollInput { send }).query()?;
        Ok(Self {
            runtime,
            ui,
            renderer,
            surface,
            window,
            input,
            observations,
        })
    }

    /// Owns state and its channel receiver until native close, an application stop request or a frame error.
    /// Browser startup schedules animation and returns; the loop releases its state on stop/error.
    pub fn run<State: AppState>(self, state: State, events: std::sync::mpsc::Receiver<State::Message>) -> Result<(), HostError> {
        // Both local frame hooks and foreign dispatch use this allocation's
        // existing COM borrow gate. Resolve its owner projection only once.
        let host = std::rc::Rc::new(self);
        let state = neocogi_ui::events::Dispatcher::into_local(neocogi_ui::events::Dispatcher::new(
            Application {
                state,
                host: host.clone(),
                size: Size::default(),
            },
            events,
        ));
        let owner = state.query()?;
        let driver = Driver {
            host,
            state,
            owner,
            remaining: frame_limit()?,
            suspended: false,
        };
        #[cfg(target_arch = "wasm32")]
        {
            browser::run(driver)
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            let mut driver = driver;
            loop {
                match driver.frame(monotonic_seconds())? {
                    // Successful presentation applies the backend's own pacing policy.
                    FrameOutcome::Presented => {}
                    // No presentation takes place while suspended. Bound idle polling while
                    // retaining a short wakeup for resize, input, and close requests.
                    FrameOutcome::Suspended => std::thread::sleep(std::time::Duration::from_millis(16)),
                    FrameOutcome::Stopped => break,
                }
            }
            Ok(())
        }
    }

    /// Decodes a complete compressed PNG through the optional provider and uploads to this renderer.
    /// The caller receives the actual immutable renderer image; decoded pixels live only during upload.
    #[cfg(feature = "png-assets")]
    pub fn png_texture(&self, png: &[u8]) -> Result<ComPtr<ITextureAbi>, HostError> {
        self.png_texture_bounded(png, MAX_ASSET_PIXELS_BYTES)
    }

    /// Uploads within the caller's remaining aggregate image budget, checked before pixel allocation.
    #[cfg(feature = "png-assets")]
    pub fn png_texture_bounded(&self, png: &[u8], rgba_limit: usize) -> Result<ComPtr<ITextureAbi>, HostError> {
        let runtime = self.runtime;
        let gfx = runtime.acquire_plugin_root::<GfxPluginFactoryAbi>(neocogi_com::guid_from_qualified_name("neocogi.gfx.runtime"))?;
        let decoder = gfx.png_decoder()?;
        let target = self.renderer.query::<ITextureFactoryAbi>()?;
        decoder
            .upload(NcCopyFromBufferOffset::new(0, png), rgba_limit as u64, target.as_com_ref())
            .map_err(|status| {
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
    fn input(&self, event: UiInput, text: NcCopyFromBufferOffset<'_, u8>) -> Result<(), NcStatus> {
        if text.offset() != 0 {
            return Err(neocogi_com::NC_E_INVALID_ARG);
        }
        let text = std::str::from_utf8(text.as_slice()).map_err(|_| neocogi_com::NC_E_INVALID_ARG)?.to_owned();
        self.send.send(InputObservation { event, text }).map_err(|_| UI_E_CLOSED)
    }
}

/// Adapts the host's frame-capable application to synchronous message-processing behavior.
struct Application<State: AppState> {
    /// Ordinary authoritative application fields, owned by the persistent dispatcher.
    state: State,
    /// Shared immutable host services own no application dispatcher and cannot form a cycle.
    host: std::rc::Rc<Host>,
    /// Current positive drawable size installed before entering the UI update.
    size: Size,
}
impl<State: AppState> neocogi_ui::events::MessageProcessor for Application<State> {
    type Message = State::Message;
    /// Preserves actual UI routing order instead of deriving command ownership from the poll tail.
    fn input_boundary(&mut self, boundary: InputBoundary) -> Result<(), NcStatus> {
        self.state.input_boundary(boundary)
    }
    /// Uses the real application gate while the outer UI has released traversal ownership.
    fn apply_pending_changes(&mut self) -> Result<ApplyOutcome, NcStatus> {
        self.state.apply_pending_changes(&self.host, self.size)
    }
    /// Runtime delivery holds exclusive dispatcher access before receiving a message.
    fn handle(&mut self, message: Self::Message) -> Result<(), NcStatus> {
        self.state.update(message)
    }
}

/// Result of one host iteration, separating presentation pacing from idle suspension.
/// Platform adapters decide how to schedule the next iteration; the driver never sleeps.
enum FrameOutcome {
    /// A drawable frame completed and the application remains active.
    Presented,
    /// The window has no drawable area; input was polled but no frame budget was consumed.
    Suspended,
    /// Close, application state, or the rendered-frame limit ended the run.
    Stopped,
}

/// Concrete loop ownership is shared by the native iteration and browser scheduling adapter.
struct Driver<State: AppState> {
    /// Owns exactly one UI/renderer pair for this application.
    host: std::rc::Rc<Host>,
    /// Persistent owned state; short local guards end before entering UI dispatch.
    state: neocogi_com::LocalCom<neocogi_ui::events::Dispatcher<Application<State>>>,
    /// Checked once at construction and borrowed for every frame without querying.
    owner: ComPtr<neocogi_ui::IApplicationDispatcherAbi>,
    /// Optional finite run counts successfully rendered frames, including the final frame.
    remaining: Option<u32>,
    /// Whether the last observed drawable size was zero; cancellation is sent once per transition.
    suspended: bool,
}
impl<State: AppState> Driver<State> {
    /// Polls input, applies frame edits, then settles events and layout in one update before rendering.
    fn frame(&mut self, seconds: f64) -> Result<FrameOutcome, HostError> {
        let host = &self.host;
        loop {
            let status = host
                .window
                .poll(host.input.as_com_ref())
                .map_err(|status| host.frame_error("platform input polling", status))?;
            let size = host.window.size().map_err(|status| host.frame_error("window size query", status))?;
            if size.width <= 0 || size.height <= 0 {
                if !self.suspended {
                    self.suspended = true;
                    // End host gestures and reset UI held state once. Events
                    // received while minimized have no drawable target and are
                    // discarded instead of accumulating behind a skipped update.
                    self.state
                        .try_borrow_mut()?
                        .processor
                        .state
                        .input(UiInput::Cancel, "")
                        .map_err(|status| host.frame_error("application input cancellation", status))?;
                    host.ui
                        .input(UiInput::Cancel, "")
                        .map_err(|status| host.frame_error("UI input cancellation", status))?;
                }
                for _ in host.observations.try_iter() {}
            } else {
                self.suspended = false;
                for observation in host.observations.try_iter() {
                    self.state
                        .try_borrow_mut()?
                        .processor
                        .state
                        .input(observation.event, &observation.text)
                        .map_err(|status| host.frame_error("application input observation", status))?;
                    host.ui
                        .input(observation.event, &observation.text)
                        .map_err(|status| host.frame_error("UI input intake", status))?;
                }
            }
            match status {
                PollStatus::Event => continue,
                PollStatus::Idle => break,
                PollStatus::Close => {
                    if self
                        .state
                        .try_borrow_mut()?
                        .processor
                        .state
                        .close_requested()
                        .map_err(|status| host.frame_error("application close request", status))?
                    {
                        return Ok(FrameOutcome::Stopped);
                    }
                    break;
                }
                _ => return Err(HostError::Message("unknown platform polling result".to_owned())),
            }
        }
        let size = host.window.size().map_err(|status| host.frame_error("window size query", status))?;
        if size.width <= 0 || size.height <= 0 {
            return Ok(FrameOutcome::Suspended);
        }
        // Apply frame edits before consuming the input just polled. Handlers can prepare work for
        // the next frame callback; this frame's direct edits and nested events settle in one update.
        self.state.try_borrow_mut()?.processor.size = size;
        if !self
            .state
            .try_borrow_mut()?
            .processor
            .state
            .frame(host, size, seconds)
            .map_err(|status| host.frame_error("application frame preparation", status))?
        {
            return Ok(FrameOutcome::Stopped);
        }
        host.ui
            .update_with_events(size, self.owner.as_com_ref())
            .map_err(|status| host.frame_error("event and layout update", status))?;
        self.state
            .try_borrow_mut()?
            .processor
            .state
            .after_update(host, size)
            .map_err(|status| host.frame_error("application frame update", status))?;
        let text_area = self
            .state
            .try_borrow()?
            .processor
            .state
            .text_input_area(host)
            .map_err(|status| host.frame_error("text input placement", status))?;
        host.window
            .text_input(text_area)
            .map_err(|status| host.frame_error("native text input", status))?;
        host.ui
            .render(size, host.renderer.as_com_ref())
            .map_err(|error| render_error("frame rendering", error))?;
        if let Some(remaining) = self.remaining {
            self.remaining = Some(remaining - 1);
            return Ok(match remaining {
                1 => FrameOutcome::Stopped,
                _ => FrameOutcome::Presented,
            });
        }
        Ok(FrameOutcome::Presented)
    }
}

#[cfg(all(not(target_arch = "wasm32"), feature = "dynamic", feature = "monolith"))]
compile_error!("select one native linkage: dynamic or monolith; disable defaults when selecting monolith");

#[cfg(all(not(target_arch = "wasm32"), not(any(feature = "dynamic", feature = "monolith"))))]
/// Reports missing linkage selection without entering the runtime.
fn register(_runtime: neocogi_com::Runtime) -> Result<(), HostError> {
    Err(HostError::Message("select dynamic or monolith for native execution".to_owned()))
}

/// Keeps this operation's original status and owned text without retaining the failed device.
fn render_error(operation: &'static str, error: RenderError) -> HostError {
    HostError::Operation {
        operation,
        status: error.status,
        detail: if error.message.is_empty() { None } else { Some(error.message) },
    }
}

impl From<RenderError> for HostError {
    /// Native setup failures own their message before reaching application formatting.
    fn from(error: RenderError) -> Self { render_error("platform operation", error) }
}

/// Registers the process plugin catalog before loading assets or constructing windows.
/// Repeated calls reuse the first result; Host::new also calls this for callers supplying their own bundle.
pub fn initialize(runtime: neocogi_com::Runtime) -> Result<(), HostError> {
    static STARTUP: std::sync::OnceLock<Result<(), String>> = std::sync::OnceLock::new();
    STARTUP
        .get_or_init(|| register(runtime).map_err(|error| error.to_string()))
        .as_ref()
        .map(|_| ())
        .map_err(|error| HostError::Message(error.clone()))
}

/// Loads the exact native artifacts Cargo built for this composition.
/// Registration order is explicit because Nodes and Skin declare hard provider requirements.
#[cfg(all(not(target_arch = "wasm32"), feature = "dynamic", not(feature = "monolith")))]
fn register(runtime: neocogi_com::Runtime) -> Result<(), HostError> {
    neocogi_demo_plugins::register(runtime)?;
    Ok(())
}

/// Registers the implementation descriptors selected by static Cargo features.
#[cfg(any(target_arch = "wasm32", feature = "monolith"))]
fn register(runtime: neocogi_com::Runtime) -> Result<(), HostError> {
    let descriptors = [
        nc_ui::plugin_descriptor(),
        nc_ui_nodes::plugin_descriptor(),
        nc_shader_compiler::plugin_descriptor(),
        nc_platform::plugin_descriptor(),
        #[cfg(feature = "monolith-extension")]
        nc_ui_extension::plugin_descriptor(),
        #[cfg(any(target_arch = "wasm32", feature = "monolith-gfx"))]
        nc_gfx::plugin_descriptor(),
        #[cfg(feature = "skin")]
        nc_skin::plugin_descriptor(),
    ];
    for descriptor in descriptors {
        // SAFETY: descriptors and provider code remain linked for process lifetime.
        unsafe {
            runtime.register_plugin_descriptor(descriptor)?;
        }
    }
    Ok(())
}

/// Returns the qualified identity selected for the separately built renderer assembly.
fn renderer_plugin_name() -> &'static str {
    if cfg!(feature = "vulkan") {
        "neocogi.platform.vulkan"
    } else if cfg!(feature = "wgpu") {
        "neocogi.platform.wgpu"
    } else if cfg!(feature = "webgl") {
        "neocogi.platform.webgl"
    } else {
        // Glow is also the workspace-check fallback when this library is built
        // directly without an application selecting its renderer feature.
        "neocogi.platform.glow"
    }
}

/// Opt-in finite demos exercise ordinary input/update/render and teardown without a separate mock loop.
/// Native runs use NEOCOGI_DEMO_FRAMES; browser runs use the `frames` query parameter. Omission runs
/// until close or AppState's stop request; present undecodable, malformed or zero limits fail
/// explicitly at startup. Parsing remains before driver-loop entry without reordering setup.
fn frame_limit() -> Result<Option<u32>, HostError> {
    let invalid = || HostError::Message("demo frame limit must be a positive u32".to_owned());
    #[cfg(not(target_arch = "wasm32"))]
    let value = match std::env::var("NEOCOGI_DEMO_FRAMES") {
        Ok(value) => Some(value),
        Err(std::env::VarError::NotPresent) => None,
        Err(std::env::VarError::NotUnicode(_)) => return Err(invalid()),
    };
    #[cfg(target_arch = "wasm32")]
    let value = web_sys::window()
        .and_then(|window| window.location().search().ok())
        .and_then(|search| web_sys::UrlSearchParams::new_with_str(&search).ok())
        .and_then(|query| query.get("frames"));
    value
        .map(|value| value.parse::<u32>().ok().filter(|count| *count > 0).ok_or_else(invalid))
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

#[cfg(test)]
mod tests;
