//! Explicit, non-asserting measurements of real UI recording and native execution.
use super::*;
use std::time::Instant;
use nc_render::testing::Sample;

#[com_object(interfaces(IRenderer))]
/// Captures authentic blocks after real UI paint without substituting a fake GPU host.
struct Capture {
    /// Same logical device as the production window authenticates all stream observers.
    logical: neocogi_render_gpu::GpuDevice<NativeBackend>,
    /// Only a synchronous COM receiver needs interior mutability; no outward borrow is retained.
    blocks: Rc<RefCell<Vec<Rc<neocogi_render_gpu::record::Block<NativeBackend>>>>>,
}
impl neocogi_render_core::abi::IRenderDriverBound for Capture {
    fn driver_identity(&self) -> Result<neocogi_com::ComPtr<neocogi_render_core::abi::IRenderDriverIdentityAbi>, neocogi_com::NcStatus> {
        Err(neocogi_com::NC_E_NOINTERFACE)
    }
}
impl IRenderer for Capture {
    fn render(&self, info: RenderInfo, stream: ComRef<'_, IRenderStreamAbi>) -> Result<(), NcStatus> {
        let mut blocks = Vec::new();
        for index in 0..info.block_count {
            let block = stream.view(info.invocation, index)?;
            blocks.push(self.logical.block_snapshot(block.as_com_ref())?);
        }
        *self.blocks.borrow_mut() = blocks;
        Ok(())
    }
}

/// This is a measurement entrypoint, not a timing regression threshold. Cold and
/// warm frames execute the same immutable scene; glFinish includes native completion.
#[test]
#[ignore = "explicit native cost measurement; SDL_VIDEODRIVER=offscreen and --nocapture"]
fn measure_ui_and_mixed_client_costs() {
    use neocogi_ui::{Ui, UiPluginFactoryAbi, nodes::NodesPluginFactoryAbi};
    let _session = native_session();
    let runtime = crate::host_runtime();
    // SAFETY: All statically linked provider code and descriptors are process-lived.
    unsafe {
        runtime.register_plugin_descriptor(nc_gfx::plugin_descriptor()).unwrap();
        runtime.register_plugin_descriptor(nc_ui::plugin_descriptor()).unwrap();
        runtime.register_plugin_descriptor(nc_ui_nodes::plugin_descriptor()).unwrap();
    }
    for mixed in [false, true] {
        let setup = Instant::now();
        let native = PlatformObject::new(
            nc_shader_compiler::CompilerObject::from_value(nc_shader_compiler::CompilerObject)
                .query()
                .unwrap(),
        )
        .window("native costs", SIZE)
        .unwrap();
        let device = native.probe.clone();
        let window = WindowObject::from_value(native);
        let platform: ComPtr<IPlatformWindowAbi> = window.query().unwrap();
        let host = platform.graphics_host().unwrap();
        let assets = neocogi_demo_assets::load_bundle(runtime).unwrap();
        let gfx = runtime
            .acquire_plugin_root::<neocogi_gfx_support::abi::GfxPluginFactoryAbi>(neocogi_com::guid_from_qualified_name("neocogi.gfx.runtime"))
            .unwrap();
        let factory = runtime
            .acquire_plugin_root::<UiPluginFactoryAbi>(neocogi_com::guid_from_qualified_name("neocogi.ui.runtime"))
            .unwrap();
        let mut core = factory.create_runtime(assets.as_com_ref(), gfx.as_com_ref()).unwrap();
        core.set_render_host(host.as_com_ref()).unwrap();
        let nodes = runtime
            .acquire_plugin_root::<NodesPluginFactoryAbi>(neocogi_com::guid_from_qualified_name("neocogi.ui.nodes.runtime"))
            .unwrap();
        let widgets = nodes.bind(core.as_com_ref()).unwrap();
        let ui = Ui::from_interfaces(core, widgets).unwrap();
        let root = ui.widgets().button_unobserved("Measured UI").unwrap();
        let _ui_window = ui.window("costs", Rect { x: 0, y: 0, width: 64, height: 64 }, root).unwrap();
        ui.update(SIZE).unwrap();
        let captured = Rc::new(RefCell::new(Vec::new()));
        let capture = Capture::from_value(Capture {
            logical: device.logical().unwrap(),
            blocks: captured.clone(),
        });
        let receiver: ComPtr<IRendererAbi> = capture.query().unwrap();
        let record = Instant::now();
        ui.render(SIZE, receiver.as_com_ref()).unwrap();
        let record_ns = record.elapsed().as_nanos();
        let mut blocks = captured.borrow().clone();
        let clients: ComPtr<IRenderClientFactoryAbi> = window.query().unwrap();
        let domain = clients.create_client(Default::default()).unwrap();
        let resources = domain.resources().unwrap();
        let client = mixed.then(|| {
            graphics(
                &resources,
                &host.recording_broker().unwrap(),
                RenderViewRoot {
                    client: Rect { x: 0, y: 0, width: 32, height: 32 },
                    target: Rect { x: 16, y: 16, width: 32, height: 32 },
                },
            )
        });
        if let Some(client) = &client {
            blocks.push(device.logical().unwrap().block_snapshot(client.as_com_ref()).unwrap());
        }
        let setup_ns = setup.elapsed().as_nanos();
        let mut commands = 0;
        let mut draws = 0;
        let mut buffers = std::collections::HashSet::new();
        let mut textures = std::collections::HashSet::new();
        let mut pipelines = std::collections::HashSet::new();
        for block in &blocks {
            commands += block.commands().len();
            for command in block.commands() {
                if let neocogi_render_gpu::command::Command::Draw(draw) = command {
                    draws += 1;
                    pipelines.insert(Rc::as_ptr(&draw.pipeline) as usize);
                    for vertex in &draw.vertex_buffers {
                        buffers.insert(Rc::as_ptr(&vertex.buffer) as usize);
                    }
                    if let Some(index) = &draw.index {
                        buffers.insert(Rc::as_ptr(&index.buffer) as usize);
                    }
                    for image in &draw.textures {
                        textures.insert(Rc::as_ptr(&image.texture) as usize);
                    }
                }
            }
        }
        device
            .measure(
                &blocks,
                Sample {
                    setup_ns,
                    record_ns,
                    mixed,
                    commands,
                    draws,
                    buffers: buffers.len(),
                    textures: textures.len(),
                    pipelines: pipelines.len(),
                },
            )
            .unwrap();
    }
}
