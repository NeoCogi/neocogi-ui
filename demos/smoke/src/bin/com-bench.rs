//! Separately linked CPU benchmark of real COM trees; never part of an application binary.
//! The recorder copies the ordinary bulk stream but deliberately performs no GPU work.
use neocogi_ui::*;
use neocogi_com::{ComPtr, ComRef, NcCopyToBufferOffset, NcStatus, com_object};
use std::{
    cell::RefCell,
    error::Error,
    hint::black_box,
    time::{Duration, Instant},
};

/// Normal drawable bounds keep all 32 × 32 small buttons visible at once.
const SIZE: Size = Size { width: 1000, height: 1000 };

/// Debugger markers bracket only measured work; counters remain external to production code.
#[unsafe(no_mangle)]
#[inline(never)]
pub extern "C" fn neocogi_bench_begin(operation: u32) {
    black_box(operation);
}
/// Ends the counter interval before result formatting or memory inspection allocates.
#[unsafe(no_mangle)]
#[inline(never)]
pub extern "C" fn neocogi_bench_end() {
    black_box(());
}

/// Reusable receiver storage represents an ordinary renderer's bounded bulk-copy cost.
#[derive(Default)]
struct Buffers {
    /// Copied geometry remains observable after the stream scope expires.
    vertices: Vec<RenderVertex>,
    /// Copied ordered batch records, using the canonical ABI representation.
    batches: Vec<RenderBatchAbi>,
}
impl Buffers {
    /// Copies both complete tables; warmed capacity is retained between frames.
    fn copy(&mut self, info: RenderInfo, stream: ComRef<'_, IRenderStreamAbi>) -> Result<(), NcStatus> {
        self.vertices.resize(info.vertex_count as usize, RenderVertex::default());
        self.batches.resize(info.batch_count as usize, RenderBatch::Atlas { first: 0, count: 0 }.into());
        stream.vertices(info.invocation, NcCopyToBufferOffset::new(0, &mut self.vertices))?;
        stream.batches(info.invocation, NcCopyToBufferOffset::new(0, &mut self.batches))?;
        black_box((&self.vertices, &self.batches));
        Ok(())
    }
}
#[com_object(interfaces(IRenderer))]
/// Concrete synchronous recorder measures UI geometry generation and transport without driver noise.
struct Recorder {
    /// Taking this storage rejects recursive calls and avoids a RefCell borrow across COM.
    buffers: RefCell<Option<Buffers>>,
}
impl IRenderer for Recorder {
    /// CPU transport requires no device upload; the UI has already validated its atlas image.
    fn prepare_atlas(&self, _atlas: ComRef<'_, IRgbaImageAbi>) -> Result<(), NcStatus> {
        Ok(())
    }
    /// Always restores capacity after an outward call, including failed stream reads.
    fn render(&self, info: RenderInfo, _atlas: ComRef<'_, IRgbaImageAbi>, stream: ComRef<'_, IRenderStreamAbi>) -> Result<(), NcStatus> {
        let mut buffers = self.buffers.borrow_mut().take().ok_or(UI_E_BUSY)?;
        let result = buffers.copy(info, stream);
        *self.buffers.borrow_mut() = Some(buffers);
        result
    }
}

#[derive(Clone, Copy)]
/// Explicit workloads distinguish ordinary update, layout-causing edits, resize and paint/transport.
enum Operation {
    /// Traverses the retained scene without input or model changes.
    Update,
    /// Changes every label and includes the resulting measurement and placement.
    Edit,
    /// Changes both drawable and window allocation before updating the scene.
    Resize,
    /// Records geometry and copies both bulk tables into retained receiver buffers.
    Render,
}
impl Operation {
    /// Stable IDs are also the debugger's counter keys.
    fn id(self) -> u32 {
        match self {
            Self::Update => 1,
            Self::Edit => 2,
            Self::Resize => 3,
            Self::Render => 4,
        }
    }
    /// Names describe the public boundary being timed rather than claiming an internal layout timer.
    fn name(self) -> &'static str {
        match self {
            Self::Update => "unchanged_update",
            Self::Edit => "setters_and_layout_update",
            Self::Resize => "resize_layout_update",
            Self::Render => "paint_and_bulk_copy",
        }
    }
}

/// One process owns one shape, preventing earlier scenes from skewing its retained-memory sample.
struct Workload {
    /// Client retains the actual dynamically constructed UI runtime.
    ui: Ui,
    /// App-side editing capabilities; each points to the tree's same concrete object.
    buttons: Vec<ButtonWidget>,
    /// App-owned concrete COM recorder has no edge back to the UI runtime.
    recorder: ComPtr<IRendererAbi>,
    /// Keeps the window capability inspectable until explicit teardown.
    window: ComPtr<IWindowAbi>,
    /// Alternates geometry-changing labels without allocating a formatted test string.
    long_label: bool,
    /// Last committed drawable dimensions; render must use the same size.
    size: Size,
}
impl Workload {
    /// Constructs real mount-once edges and a fully visible scene through the public facade.
    fn new(ui: Ui, shape: &str) -> Result<Self, NcStatus> {
        let factory = ui.widgets();
        let mut buttons = Vec::new();
        let root = if shape == "wide" {
            let tracks = [LayoutTrack::Fixed { extent: 24 }; 32];
            let mut grid = factory.grid(&tracks, &tracks)?;
            for index in 0..1024 {
                let mut button = factory.button("MMMM")?;
                button.set_options(WidgetOptions::NONE)?;
                grid.insert(index, &button, GridExtent { columns: 1, rows: 1 })?.finish()?;
                buttons.push(button);
            }
            grid.node_interface()?
        } else {
            let mut button = factory.button("MMMM")?;
            button.set_options(WidgetOptions::NONE)?;
            let mut root = button.node_interface()?;
            buttons.push(button);
            if shape == "deep" {
                for _ in 0..64 {
                    let mut parent = factory.linear(FlowDirection::TopToBottom, CrossSize::Stretch)?;
                    let child = Widget::from_interface(root);
                    parent.insert(0, &child, LinearEdge::new(LayoutTrack::Flex { weight: 1.0 }))?.finish()?;
                    root = parent.node_interface()?;
                }
            }
            root
        };
        let root = Widget::from_interface(root);
        let window = ui.window(
            "COM benchmark",
            Rect {
                x: 0,
                y: 0,
                width: SIZE.width,
                height: SIZE.height,
            },
            &root,
        )?;
        let recorder = Recorder::from_value(Recorder {
            buffers: RefCell::new(Some(Buffers::default())),
        })
        .query()?;
        Ok(Self {
            ui,
            buttons,
            recorder,
            window,
            long_label: true,
            size: SIZE,
        })
    }
    /// Performs the exact caller-visible workload; all no-change and invalidation logic remains in the plugin.
    fn step(&mut self, operation: Operation) -> Result<(), NcStatus> {
        match operation {
            Operation::Update => self.ui.update(self.size),
            Operation::Edit => {
                self.long_label = !self.long_label;
                for button in &mut self.buttons {
                    button.set_label(if self.long_label { "MMMM" } else { "i" })?;
                }
                self.ui.update(self.size)
            }
            Operation::Resize => {
                self.size.width = if self.size.width == SIZE.width { SIZE.width + 1 } else { SIZE.width };
                self.window.set_rect(Rect {
                    x: 0,
                    y: 0,
                    width: self.size.width,
                    height: self.size.height,
                })?;
                self.ui.update(self.size)
            }
            Operation::Render => self.ui.render(self.size, self.recorder.as_com_ref()),
        }
    }
    /// Warm both label sizes, both drawable sizes and the largest display-list/receiver capacities.
    fn warm(&mut self) -> Result<(), NcStatus> {
        self.ui.update(self.size)?;
        for _ in 0..4 {
            self.step(Operation::Edit)?;
            self.step(Operation::Resize)?;
            self.step(Operation::Render)?;
        }
        Ok(())
    }
    /// Times repeated work with formatting, setup, teardown and debugger setup outside the interval.
    fn sample(&mut self, operation: Operation, iterations: u32) -> Result<Duration, NcStatus> {
        neocogi_bench_begin(operation.id());
        let start = Instant::now();
        for _ in 0..iterations {
            self.step(operation)?;
        }
        let elapsed = start.elapsed();
        neocogi_bench_end();
        Ok(elapsed)
    }
}

/// Linux resident/proportional pages include the executable, shared runtime/UI, allocator and asset storage.
fn memory() -> Result<(), Box<dyn Error>> {
    let text = std::fs::read_to_string("/proc/self/smaps_rollup")?;
    for line in text.lines().filter(|line| line.starts_with("Rss:") || line.starts_with("Pss:")) {
        let mut parts = line.split_whitespace();
        let name = parts.next().ok_or("missing memory field")?.trim_end_matches(':');
        let kib: u64 = parts.next().ok_or("missing memory value")?.parse()?;
        println!("{{\"memory\":\"{name}\",\"kib\":{kib}}}");
    }
    Ok(())
}

/// Loads one trusted UI plugin and emits machine-readable timing samples for one selected shape.
fn main() -> Result<(), Box<dyn Error>> {
    let mut arguments = std::env::args_os().skip(1);
    let library = arguments.next().ok_or("usage: com-bench <UI-library> <single|deep|wide> [--counts]")?;
    let shape = arguments.next().ok_or("missing shape")?.into_string().map_err(|_| "invalid shape")?;
    if !matches!(shape.as_str(), "single" | "deep" | "wide") {
        return Err("invalid shape".into());
    }
    let counts = match arguments.next() {
        None => false,
        Some(value) if value == "--counts" => true,
        _ => return Err("unknown argument".into()),
    };
    if arguments.next().is_some() {
        return Err("extra argument".into());
    }
    let start = Instant::now();
    let runtime = neocogi_com::Runtime::global();
    // SAFETY: This development benchmark accepts a trusted workspace-built matching UI plugin.
    unsafe {
        runtime.load_plugin_library(std::path::PathBuf::from(library));
    }
    runtime.register_factory_plugin_description(neocogi_gfx::factory_plugin_description())?;
    runtime.register_factory_plugin_description(neocogi_ui_nodes::factory_plugin_description())?;
    let loaded = start.elapsed();
    let start = Instant::now();
    let assets = neocogi_demo_assets::load_bundle()?;
    let decoded = start.elapsed();
    let start = Instant::now();
    let runtime_factory = runtime.activate_plugin_factory::<UiPluginFactoryAbi>(neocogi_com::guid_from_qualified_name("neocogi.ui.runtime"))?;
    let gfx = runtime.activate_plugin_factory::<GfxPluginFactoryAbi>(neocogi_com::guid_from_qualified_name("neocogi.gfx.runtime"))?;
    let ui_runtime = runtime_factory.create_runtime(assets.as_com_ref(), gfx.as_com_ref())?;
    let node_provider = runtime.activate_plugin_factory::<nodes::NodesPluginFactoryAbi>(neocogi_com::guid_from_qualified_name("neocogi.ui.nodes.runtime"))?;
    let node_factory = node_provider.bind(ui_runtime.as_com_ref())?;
    let ui = Ui::from_interfaces(ui_runtime, node_factory)?;
    let created = start.elapsed();
    let start = Instant::now();
    let mut workload = Workload::new(ui, &shape)?;
    let tree = start.elapsed();
    workload.warm()?;
    println!(
        "{{\"shape\":\"{shape}\",\"load_ns\":{},\"assets_ns\":{},\"ui_ns\":{},\"tree_ns\":{}}}",
        loaded.as_nanos(),
        decoded.as_nanos(),
        created.as_nanos(),
        tree.as_nanos()
    );
    memory()?;
    let iterations = if counts { 1 } else { 64 };
    for operation in [Operation::Update, Operation::Edit, Operation::Resize, Operation::Render] {
        for sample in 0..if counts { 1 } else { 7 } {
            let elapsed = workload.sample(operation, iterations)?;
            println!(
                "{{\"operation\":\"{}\",\"sample\":{sample},\"iterations\":{iterations},\"ns_per_iteration\":{}}}",
                operation.name(),
                elapsed.as_nanos() / u128::from(iterations)
            );
        }
    }
    workload.window.close()?;
    workload.ui.close()?;
    Ok(())
}
