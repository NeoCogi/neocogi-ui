//! Concrete demo host over the static facade, with dynamic native and static browser deployments.
//!
//! Application state and handlers remain local. UI implementation, DOM/SDL events and GPU objects
//! are reached only through their checked COM interfaces. Native packages are executable-relative.
#![deny(missing_docs)]
use microui_redux::*;
use nc_com::{ComPtr, NcCopyFromBufferOffset, NcStatus};
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

/// Application-owned state participates through one explicit method, never a stored closure.
pub trait AppState: 'static {
    /// Runs after input handlers and before the final update/render. Return false to stop the loop.
    /// Keep UI changes here or in typed event handlers; this call must not dispatch its own session
    /// while that session's state is mutably borrowed. The host releases that borrow before update.
    fn frame(&mut self, _host: &Host, _size: Size, _seconds: f64) -> Result<bool, NcStatus> {
        Ok(true)
    }
}

/// One demo UI and platform window; these fields are concrete capabilities, with no native handles.
pub struct Host {
    /// Retained runtime, widget factory and window manager share one UI COM allocation.
    pub ui: Ui,
    /// Selected backend owns all images and custom render items used by this host.
    pub renderer: ComPtr<IRendererAbi>,
    /// OS/canvas polling and native composition share the renderer's COM identity.
    pub window: ComPtr<IPlatformWindowAbi>,
    /// Input projection is queried once, then borrowed by every platform poll.
    input: ComPtr<IInputSinkAbi>,
}
impl Host {
    /// Loads/registers trusted assemblies once and creates one complete demo host from immutable assets.
    pub fn new(title: &str, size: Size, assets: ComPtr<IAssetBundleAbi>) -> Result<Self, HostError> {
        startup()?;
        let runtime = nc_com::Runtime::global();
        runtime.require_interface_type::<IUiProviderAbi>(UI_PROVIDER_CLASS)?;
        runtime.require_interface_type::<IPlatformAbi>(PLATFORM_CLASS)?;
        // SAFETY: Both validated classes declare no constructor arguments.
        let provider: ComPtr<IUiProviderAbi> = unsafe { runtime.construct_class_as(UI_PROVIDER_CLASS, std::ptr::null_mut())? };
        let platform: ComPtr<IPlatformAbi> = unsafe { runtime.construct_class_as(PLATFORM_CLASS, std::ptr::null_mut())? };
        let window = platform.create_window(NcCopyFromBufferOffset::new(0, title.as_bytes()), size, assets.as_com_ref())?;
        let renderer = window.query()?;
        let ui = Ui::from_interface(provider.create_runtime(assets.as_com_ref())?)?;
        let input = ui.input_sink()?;
        Ok(Self { ui, renderer, window, input })
    }

    /// Owns the session until native close, an application stop request or a frame error.
    /// Browser startup schedules animation and returns; the loop releases its state on stop/error.
    pub fn run<State: AppState>(self, events: EventSession<State>) -> Result<(), HostError> {
        let driver = Driver {
            host: self,
            events,
            remaining: std::cell::Cell::new(frame_limit()?),
        };
        #[cfg(target_arch = "wasm32")]
        {
            browser::run(driver)
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
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
        let runtime = nc_com::Runtime::global();
        runtime.require_interface_type::<IPngDecoderAbi>(PNG_DECODER_CLASS)?;
        // SAFETY: The checked decoder class has the documented argument-free constructor.
        let decoder: ComPtr<IPngDecoderAbi> = unsafe { runtime.construct_class_as(PNG_DECODER_CLASS, std::ptr::null_mut())? };
        let target = self.renderer.query::<ITextureFactoryAbi>()?;
        decoder.upload(NcCopyFromBufferOffset::new(0, png), target.as_com_ref()).map_err(|status| {
            let detail = decoder
                .query::<IAssetDiagnosticsAbi>()
                .and_then(|diagnostics| diagnostics.diagnostic())
                .and_then(|text| nc_com::INcStringComPtrExt::to_string(&text))
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

/// Concrete loop ownership is shared by the native iteration and browser scheduling adapter.
struct Driver<State: AppState> {
    /// Owns exactly one UI/platform pair for this application.
    host: Host,
    /// Session owns typed handlers and app state, and no producer retains either.
    events: EventSession<State>,
    /// Optional finite run counts successfully rendered frames, including the final frame.
    remaining: std::cell::Cell<Option<u32>>,
}
impl<State: AppState> Driver<State> {
    /// Applies every queued host transition before delivering app work and committing the final frame.
    fn frame(&self, seconds: f64) -> Result<bool, HostError> {
        let host = &self.host;
        loop {
            match host.window.poll(host.input.as_com_ref())? {
                PollStatus::EVENT => continue,
                PollStatus::IDLE => break,
                PollStatus::CLOSE => return Ok(false),
                _ => return Err(HostError::Message("unknown platform polling result".to_owned())),
            }
        }
        let size = host.window.size()?;
        if size.width <= 0 || size.height <= 0 {
            return Ok(true);
        }
        host.ui.update_with_events(size, &self.events)?;
        // End the state borrow before callbacks can run in the next synchronization phase.
        if !self.events.state_mut()?.frame(host, size, seconds)? {
            return Ok(false);
        }
        host.ui.update_with_events(size, &self.events)?;
        host.window.text_input(host.ui.text_input_area()?)?;
        host.ui.render(size, host.renderer.as_com_ref()).map_err(|status| {
            let detail = host
                .renderer
                .query::<IRenderDiagnosticsAbi>()
                .and_then(|diagnostics| diagnostics.diagnostic())
                .and_then(|text| nc_com::INcStringComPtrExt::to_string(&text))
                .unwrap_or_default();
            HostError::Message(format!("frame failed: {status}: {detail}"))
        })?;
        if let Some(remaining) = self.remaining.get() {
            self.remaining.set(Some(remaining - 1));
            return Ok(remaining > 1);
        }
        Ok(true)
    }
}

/// Initializes the one process/module catalog once, retaining startup failures for deterministic retries.
fn startup() -> Result<(), HostError> {
    static STARTUP: std::sync::OnceLock<Result<(), String>> = std::sync::OnceLock::new();
    STARTUP
        .get_or_init(|| register().map_err(|error| error.to_string()))
        .as_ref()
        .map(|_| ())
        .map_err(|error| HostError::Message(error.clone()))
}

/// Loads the dependency-ordered manifest adjacent to the executable, independent of current directory.
#[cfg(not(target_arch = "wasm32"))]
fn register() -> Result<(), HostError> {
    let executable = std::env::current_exe()?;
    let directory = executable
        .parent()
        .ok_or_else(|| HostError::Message("executable has no parent directory".to_owned()))?;
    let manifest = std::fs::read_to_string(directory.join("plugins.txt"))?;
    let runtime = nc_com::Runtime::global();
    for name in manifest.lines() {
        let path = std::path::Path::new(name);
        if path.components().count() != 1 || !matches!(path.components().next(), Some(std::path::Component::Normal(_))) {
            return Err(HostError::Message("plugin manifest entries must be file names".to_owned()));
        }
        // SAFETY: com-build stages these trusted workspace assemblies and their matching runtime.
        // Required live activation follows nc-com's documented abort policy on invalid binaries.
        unsafe {
            runtime.load_plugin_library(directory.join(path));
        }
    }
    Ok(())
}

/// Browser implementation descriptors live in this wasm module and use its single static runtime.
#[cfg(target_arch = "wasm32")]
fn register() -> Result<(), HostError> {
    let runtime = nc_com::Runtime::global();
    for descriptor in [
        microui_plugin_ui::plugin_descriptor(),
        microui_plugin_platform::plugin_descriptor(),
        #[cfg(feature = "png-assets")]
        microui_plugin_assets::plugin_descriptor(),
    ] {
        // SAFETY: The complete graph and implementation code remain static for this module's life.
        let status = unsafe { runtime.register_plugin_descriptor(descriptor) };
        if status.is_err() {
            return Err(status.into());
        }
    }
    Ok(())
}

/// Opt-in finite demos exercise ordinary input/update/render and teardown without a separate mock loop.
/// Native runs use MICROUI_DEMO_FRAMES; browser runs use the `frames` query parameter. Omission runs
/// until close or AppState's stop request; malformed or zero limits fail explicitly at startup.
fn frame_limit() -> Result<Option<u32>, HostError> {
    #[cfg(not(target_arch = "wasm32"))]
    let value = std::env::var("MICROUI_DEMO_FRAMES").ok();
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
