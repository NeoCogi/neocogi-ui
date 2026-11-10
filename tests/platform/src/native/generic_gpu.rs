//! Generic GPU acceptance through published COM contracts and the production native executor.
use super::*;
use neocogi_com::{ComBox, WeakComBox};
use neocogi_platform::gpu::source::{Bytes, PipelineDesc};

/// Production GFX owns the UI shader/layout; UI, images and a client share one native executor.
#[test]
#[ignore = "requires SDL GL; run with SDL_VIDEODRIVER=offscreen"]
fn gfx_generic_blocks_preserve_window_order_and_immutable_surface_output() {
    use neocogi_gfx_support::abi::{GfxPluginFactoryAbi, SolidVertex};
    let _session = native_session();
    let runtime = crate::host_runtime();
    // SAFETY: this statically linked descriptor and its implementation live for this process.
    unsafe {
        runtime.register_plugin_descriptor(nc_gfx::plugin_descriptor()).unwrap();
    }
    let gfx = runtime
        .acquire_plugin_root::<GfxPluginFactoryAbi>(neocogi_com::guid_from_qualified_name("neocogi.gfx.runtime"))
        .unwrap();
    let assets = neocogi_demo_assets::load_bundle(runtime).unwrap();
    let native = testing::window("GFX generic executor", SIZE).unwrap();
    let weak = native.probe.clone();
    let window = native.window;
    let renderer: ComPtr<IRendererAbi> = window.query().unwrap();
    let host = window.query::<IPlatformWindowAbi>().unwrap().graphics_host().unwrap();
    let mut compositor = gfx.create_compositor(assets.as_com_ref()).unwrap();
    compositor
        .bind_render_host(host.as_com_ref(), framebuffer(host.as_com_ref()).as_com_ref())
        .unwrap();
    let images: ComPtr<ITextureFactoryAbi> = window.query().unwrap();
    let image = images
        .create_texture(
            Size { width: 2, height: 2 },
            neocogi_platform_contracts::render::source::initial_bytes(
                (NcCopyFromBufferOffset::new(0, &[255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 255, 255, 0, 255]))
                    .as_slice()
                    .to_vec(),
            )
            .borrow()
            .unwrap(),
        )
        .unwrap();
    let clients: ComPtr<IRenderClientFactoryAbi> = window.query().unwrap();
    let domain = clients
        .create_client(
            RenderClientLimits::default(),
            clients
                .query::<IRenderSurfaceFactoryAbi>()
                .unwrap()
                .create_surface(RenderSurfaceDesc::Framebuffer)
                .unwrap()
                .as_com_ref(),
            RenderClientPermissions::default(),
        )
        .unwrap();
    let resources = domain.resources().unwrap();
    let broker = host.recording_broker().unwrap();
    let root = Rect { x: 16, y: 16, width: 32, height: 32 };
    let bounds = Rect { x: 0, y: 0, width: 64, height: 64 };
    let fill = |compositor: &mut neocogi_com::ComPtr<neocogi_gfx_support::abi::GfxCompositorAbi>, rect: Rect, rgba| {
        let v = rectangle(rect, rgba)
            .into_iter()
            .map(|v| SolidVertex { x: v.x, y: v.y, rgba: v.rgba })
            .collect::<Vec<_>>();
        compositor.solid_triangles(bounds, NcCopyFromBufferOffset::new(0, &v)).unwrap();
    };
    let unit = TextureRegion { u0: 0., v0: 0., u1: 1., v1: 1. };
    compositor.begin(SIZE, 0xff000000).unwrap();
    fill(&mut compositor, bounds, 0xff00ff00);
    compositor
        .texture(bounds, Rect { x: 0, y: 0, width: 16, height: 16 }, unit, 0xffffffff, image.as_com_ref())
        .unwrap();
    compositor
        .render_view(graphics(
            &resources,
            &broker,
            RenderViewRoot {
                client: Rect { x: 0, y: 0, width: 32, height: 32 },
                target: root,
            },
        ))
        .unwrap();
    fill(&mut compositor, Rect { x: 32, y: 32, width: 16, height: 16 }, 0xffff00ff);
    compositor.submit(renderer.as_com_ref()).unwrap();
    let pixels = weak.pixels().unwrap();
    let pixel = |x: usize, y: usize| &pixels[((63 - y) * 64 + x) * 4..((63 - y) * 64 + x) * 4 + 4];
    assert_eq!(pixel(2, 2), [255, 0, 0, 255]);
    assert_eq!(pixel(12, 2), [0, 255, 0, 255]);
    assert_eq!(pixel(2, 12), [0, 0, 255, 255]);
    assert_eq!(pixel(12, 12), [255, 255, 0, 255]);
    assert_eq!(pixel(32, 24), [0, 0, 255, 255]);
    assert_eq!(pixel(40, 40), [255, 0, 255, 255]);
    assert_eq!(pixel(60, 60), [0, 255, 0, 255]);

    let surfaces: ComPtr<IRenderSurfaceFactoryAbi> = window.query().unwrap();
    let surface = surfaces.create_surface(RenderSurfaceDesc::Image(SIZE)).unwrap();
    let preview: ComPtr<IRendererAbi> = surface.query().unwrap();
    compositor.bind_render_host(host.as_com_ref(), surface.as_com_ref()).unwrap();
    compositor.begin(SIZE, 0xff000000).unwrap();
    compositor.texture(bounds, bounds, unit, 0xffffffff, image.as_com_ref()).unwrap();
    compositor.submit(preview.as_com_ref()).unwrap();
    let output = surface.completed_image().unwrap();
    // An image-bound compositor cannot acquire/present the window framebuffer. Rejection ends
    // the GFX frame while preserving the last independently completed immutable image.
    compositor.begin(SIZE, 0xff112233).unwrap();
    compositor.texture(bounds, bounds, unit, 0xffffffff, image.as_com_ref()).unwrap();
    let error = compositor.submit(renderer.as_com_ref()).unwrap_err();
    assert_eq!(error.status, RENDER_E_OWNER);
    assert_eq!(identity(&surface.completed_image().unwrap()).unwrap(), identity(&output).unwrap());
    compositor.begin(SIZE, 0xff000000).unwrap();
    compositor
        .render_view(graphics(&resources, &broker, RenderViewRoot { client: root, target: root }))
        .unwrap();
    assert_eq!(compositor.submit(preview.as_com_ref()).map_err(|error| error.status), Err(RENDER_E_OWNER));
    assert_eq!(identity(&surface.completed_image().unwrap()).unwrap(), identity(&output).unwrap());
    drop((preview, surface, surfaces));
    compositor
        .bind_render_host(host.as_com_ref(), framebuffer(host.as_com_ref()).as_com_ref())
        .unwrap();
    compositor.begin(SIZE, 0xff000000).unwrap();
    compositor.texture(bounds, bounds, unit, 0xffffffff, output.as_com_ref()).unwrap();
    compositor.submit(renderer.as_com_ref()).unwrap();
    let pixels = weak.pixels().unwrap();
    let pixel = |x: usize, y: usize| &pixels[((63 - y) * 64 + x) * 4..((63 - y) * 64 + x) * 4 + 4];
    assert_eq!(pixel(8, 8), [255, 0, 0, 255]);
    assert_eq!(pixel(56, 8), [0, 255, 0, 255]);
    assert_eq!(pixel(8, 56), [0, 0, 255, 255]);
    assert_eq!(pixel(56, 56), [255, 255, 0, 255]);
    drop((compositor, image, output, resources, domain, clients, broker, images, host, renderer, window));
    assert!(!weak.is_alive(), "GFX/native image caches cannot retain their own assembly");
}

#[com_object(interfaces(IRenderStream))]
/// Foreign callbacks close an already captured UI domain and attempt resource mutation.
struct ClosingUiStream {
    /// Canonical UI contributions remain owned throughout preparation.
    blocks: Vec<ComBox<IRenderViewBlockAbi>>,
    /// Dropping the controller is terminal even though the immutable block is retained.
    close: Rc<RefCell<Option<ComBox<IRenderClientDomainAbi>>>>,
    /// A different still-open factory verifies the device-wide preparation gate.
    editing: ComPtr<IRenderResourcesAbi>,
}
impl IRenderStream for ClosingUiStream {
    fn view(&self, _: u64, index: u32) -> Result<WeakComBox<IRenderViewBlockAbi>, NcStatus> {
        assert!(matches!(
            self.editing.create_dynamic_buffer(GpuBufferDesc::Vertex { byte_len: 4 }),
            Err(RENDER_E_BUSY)
        ));
        if index == 1 {
            drop(self.close.borrow_mut().take());
        }
        self.blocks.get(index as usize).map(ComBox::downgrade).ok_or(NC_E_INVALID_ARG)
    }
}

/// Late stream callbacks revoke surface admission before target mutation.
#[test]
#[ignore = "requires SDL GL; run with SDL_VIDEODRIVER=offscreen"]
fn surfaces_recheck_ui_domains_and_exclude_resource_reentry_after_foreign_callbacks() {
    let _session = native_session();
    let native = testing::window("surface admission", SIZE).unwrap();
    let window = native.window;
    let host = window.query::<IPlatformWindowAbi>().unwrap().graphics_host().unwrap();
    let surface = window
        .query::<IRenderSurfaceFactoryAbi>()
        .unwrap()
        .create_surface(RenderSurfaceDesc::Image(SIZE))
        .unwrap();
    let renderer: ComPtr<IRendererAbi> = surface.query().unwrap();
    let empty: ComPtr<IRenderStreamAbi> = super::view_geometry::ViewStream::from_value(super::view_geometry::ViewStream { blocks: vec![] })
        .query()
        .unwrap();
    let info = RenderInfo {
        size: SIZE,
        clear: 0xff112233,
        invocation: 1,
        block_count: 0,
    };
    renderer.render(info, empty.as_com_ref()).unwrap();
    let previous = surface.completed_image().unwrap();
    {
        let first = host
            .query::<IRenderClientFactoryAbi>()
            .unwrap()
            .create_client(
                RenderClientLimits::default(),
                surface.as_com_ref(),
                neocogi_gfx_support::compositor_permissions(),
            )
            .unwrap();
        let second = host
            .query::<IRenderClientFactoryAbi>()
            .unwrap()
            .create_client(
                RenderClientLimits::default(),
                surface.as_com_ref(),
                neocogi_gfx_support::compositor_permissions(),
            )
            .unwrap();
        let first_resources = first.resources().unwrap();
        let second_resources = second.resources().unwrap();
        let broker = host.recording_broker().unwrap();
        let mut frame = broker.begin_frame().unwrap();
        let root = Rect { x: 0, y: 0, width: 64, height: 64 };
        let mut blocks = Vec::new();
        for resources in [&first_resources, &second_resources] {
            let mut session = frame.begin_view(RenderViewRoot { client: root, target: root }, resources.as_com_ref()).unwrap();
            session.view().unwrap().finish().unwrap();
            blocks.push(session.take_block().unwrap());
        }
        frame.finish().unwrap();
        let close = Rc::new(RefCell::new(Some(first)));
        let stream: ComPtr<IRenderStreamAbi> = ClosingUiStream::from_value(ClosingUiStream {
            blocks,
            close: close.clone(),
            editing: second_resources,
        })
        .query()
        .unwrap();
        assert_eq!(
            renderer
                .render(RenderInfo { block_count: 2, ..info }, stream.as_com_ref())
                .map_err(|error| error.status),
            Err(neocogi_com::NC_E_DROPPED)
        );
        assert!(close.borrow().is_none());
        assert_eq!(identity(&surface.completed_image().unwrap()).unwrap(), identity(&previous).unwrap());
    }
}

/// Submit UI/client/UI through the actual GFX producer, with no renderer UI command vocabulary.
fn submit_interleaved(host: ComRef<'_, IRenderHostAbi>, renderer: ComRef<'_, IRendererAbi>, block: ComBox<IRenderViewBlockAbi>, vertices: &[RenderVertex]) {
    use neocogi_gfx_support::abi::{GfxPluginFactoryAbi, SolidVertex};
    let runtime = crate::host_runtime();
    // SAFETY: the linked descriptor and all implementation data live for the process.
    unsafe {
        runtime.register_plugin_descriptor(nc_gfx::plugin_descriptor()).unwrap();
    }
    let gfx = runtime
        .acquire_plugin_root::<GfxPluginFactoryAbi>(neocogi_com::guid_from_qualified_name("neocogi.gfx.runtime"))
        .unwrap();
    let assets = neocogi_demo_assets::load_bundle(runtime).unwrap();
    let mut compositor = gfx.create_compositor(assets.as_com_ref()).unwrap();
    compositor.bind_render_host(host, framebuffer(host).as_com_ref()).unwrap();
    compositor.begin(SIZE, 0xff000000).unwrap();
    let bounds = Rect { x: 0, y: 0, width: 64, height: 64 };
    let draw = |compositor: &mut ComPtr<neocogi_gfx_support::abi::GfxCompositorAbi>, vertices: &[RenderVertex]| {
        let vertices = vertices.iter().map(|v| SolidVertex { x: v.x, y: v.y, rgba: v.rgba }).collect::<Vec<_>>();
        compositor.solid_triangles(bounds, NcCopyFromBufferOffset::new(0, &vertices)).unwrap();
    };
    draw(&mut compositor, &vertices[..6]);
    compositor.render_view(block).unwrap();
    draw(&mut compositor, &vertices[6..]);
    compositor.submit(renderer).unwrap();
}

/// Encodes an ordinary white-atlas rectangle using the actual renderer wire values.
fn rectangle(rect: Rect, color: u32) -> Vec<RenderVertex> {
    let q = quad(rect).map(|vertex| RenderVertex { u: 0., v: 0., rgba: color, ..vertex });
    vec![q[0], q[1], q[2], q[0], q[2], q[3]]
}

/// Real immutable-image import samples captured native pixels without retaining a public COM image.
#[test]
#[ignore = "requires SDL GL; run with SDL_VIDEODRIVER=offscreen"]
fn permitted_image_import_retains_native_pixels_without_a_com_image_cycle() {
    let _session = native_session();
    let native = testing::window("private UI image import", SIZE).unwrap();
    let device = native.probe.clone();
    let window = native.window;
    let renderer: ComPtr<IRendererAbi> = window.query().unwrap();
    let graphics = window.query::<IPlatformWindowAbi>().unwrap().graphics_host().unwrap();
    let images: ComPtr<ITextureFactoryAbi> = window.query().unwrap();
    let pixels = [255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 255, 255, 0, 255];
    let image = images
        .create_texture(
            Size { width: 2, height: 2 },
            neocogi_platform_contracts::render::source::initial_bytes((NcCopyFromBufferOffset::new(0, &pixels)).as_slice().to_vec())
                .borrow()
                .unwrap(),
        )
        .unwrap();
    let image_key = identity(&image).unwrap();
    let clients: ComPtr<IRenderClientFactoryAbi> = window.query().unwrap();
    let client = clients
        .create_client(
            RenderClientLimits::default(),
            clients
                .query::<IRenderSurfaceFactoryAbi>()
                .unwrap()
                .create_surface(RenderSurfaceDesc::Framebuffer)
                .unwrap()
                .as_com_ref(),
            RenderClientPermissions::default(),
        )
        .unwrap();
    assert!(matches!(
        graphics
            .import_texture(client.resources().unwrap().as_com_ref(), image.as_com_ref())
            .map_err(|error| error.status),
        Err(RENDER_E_PERMISSION)
    ));
    let ui = clients
        .create_client(
            RenderClientLimits::default(),
            framebuffer(graphics.as_com_ref()).as_com_ref(),
            neocogi_gfx_support::compositor_permissions(),
        )
        .unwrap();
    let resources = ui.resources().unwrap();
    let imported = graphics.import_texture(resources.as_com_ref(), image.as_com_ref()).unwrap();
    assert!(imported.query_ref::<ITextureAbi>().is_err());
    graphics.prepare_texture(resources.as_com_ref(), imported.as_com_ref()).unwrap();
    let code = Bytes::source(
        br#"
        @group(2) @binding(0) var image:texture_2d<f32>;
        @group(2) @binding(1) var image_sampler:sampler;
        struct V { @builtin(position) p:vec4<f32>, @location(0) uv:vec2<f32> }
        @vertex fn vs(@builtin(vertex_index) i:u32) -> V {
            let p=array<vec2<f32>,3>(vec2(-1.,-1.),vec2(3.,-1.),vec2(-1.,3.));
            return V(vec4(p[i],0.,1.),p[i]*vec2(0.5,-0.5)+vec2(0.5));
        }
        @fragment fn fs(v:V) -> @location(0) vec4<f32> { return textureSample(image,image_sampler,v.uv); }
    "#
        .to_vec(),
    );
    let vs = resources
        .create_shader(GpuShaderStage::Vertex, code.as_com_ref(), NcCopyFromBufferOffset::new(0, b"vs"))
        .unwrap();
    let fs = resources
        .create_shader(GpuShaderStage::Fragment, code.as_com_ref(), NcCopyFromBufferOffset::new(0, b"fs"))
        .unwrap();
    let desc = PipelineDesc {
        bindings: vec![GpuShaderBinding {
            stage: GpuShaderStage::Fragment,
            slot: 0,
            kind: GpuBindingKind::SampledTexture,
        }],
        ..Default::default()
    }
    .source()
    .unwrap();
    let pipeline = resources.create_pipeline(desc.as_com_ref(), vs.as_com_ref(), fs.as_com_ref()).unwrap();
    let sampler = resources.create_sampler(GpuSamplerDesc::default()).unwrap();
    let mut frame = graphics.recording_broker().unwrap().begin_frame().unwrap();
    let root = Rect {
        x: 0,
        y: 0,
        width: SIZE.width,
        height: SIZE.height,
    };
    let mut session = frame.begin_view(RenderViewRoot { client: root, target: root }, resources.as_com_ref()).unwrap();
    let mut view = session.view().unwrap();
    view.bind_pipeline(pipeline.as_com_ref()).unwrap();
    view.bind_texture(GpuShaderStage::Fragment, 0, imported.as_com_ref(), sampler.as_com_ref())
        .unwrap();
    view.draw(3, 1, 0).unwrap();
    view.finish().unwrap();
    let block = session.take_block().unwrap();
    frame.finish().unwrap();
    drop((view, session, frame, image, imported, pipeline, sampler, vs, fs));
    let source: ComPtr<IRenderStreamAbi> = super::view_geometry::ViewStream::from_value(super::view_geometry::ViewStream { blocks: vec![block] })
        .query()
        .unwrap();
    renderer
        .render(
            RenderInfo {
                size: SIZE,
                clear: 0xff000000,
                invocation: 1,
                block_count: 1,
            },
            source.as_com_ref(),
        )
        .unwrap();
    let pixels = device.pixels().unwrap();
    // GL readback is bottom-up; imported immutable images retain their top-left source convention.
    let pixel = |x: usize, y: usize| &pixels[((63 - y) * 64 + x) * 4..((63 - y) * 64 + x) * 4 + 4];
    assert_eq!(pixel(8, 8), [255, 0, 0, 255]);
    assert_eq!(pixel(56, 8), [0, 255, 0, 255]);
    assert_eq!(pixel(8, 56), [0, 0, 255, 255]);
    assert_eq!(pixel(56, 56), [255, 255, 0, 255]);
    drop(source);
    assert!(!device.contains_image(image_key), "native captures must not keep public COM images alive");
    drop((resources, ui, client, clients, images, graphics, renderer, window));
    assert!(!device.is_alive(), "native images must not retain their driver facade");
}

/// Asymmetry proves normalized position orientation; color clear and later UI prove confinement/order.
#[test]
#[ignore = "requires SDL GL; run with SDL_VIDEODRIVER=offscreen"]
fn generic_pipeline_renders_in_its_root_and_preserves_surrounding_ui() {
    let _session = native_session();
    let native = testing::window("generic graphics acceptance", SIZE).unwrap();
    let device = native.probe.clone();
    let unknown = native.window;
    let renderer: ComPtr<IRendererAbi> = unknown.query().unwrap();
    let clients: ComPtr<IRenderClientFactoryAbi> = unknown.query().unwrap();
    let domain = clients
        .create_client(
            RenderClientLimits::default(),
            clients
                .query::<IRenderSurfaceFactoryAbi>()
                .unwrap()
                .create_surface(RenderSurfaceDesc::Framebuffer)
                .unwrap()
                .as_com_ref(),
            RenderClientPermissions::default(),
        )
        .unwrap();
    let resources = domain.resources().unwrap();
    assert!(resources.query::<IRendererAbi>().is_err());
    assert!(resources.query::<IRenderClientFactoryAbi>().is_err());
    let code = Bytes::source(
        br#"
        @vertex fn vs(@builtin(vertex_index) i: u32) -> @builtin(position) vec4<f32> {
            let positions = array<vec2<f32>,3>(vec2(-0.8,0.8), vec2(0.0,0.8), vec2(-0.8,0.0));
            return vec4(positions[i], 0.5, 1.0);
        }
        @fragment fn fs(@builtin(position) p:vec4<f32>) -> @location(0) vec4<f32> {
            return select(vec4(1.0,1.0,0.0,1.0), vec4(1.0,0.0,0.0,1.0), p.y < 32.0);
        }
    "#
        .to_vec(),
    );
    let vs = resources
        .create_shader(GpuShaderStage::Vertex, code.as_com_ref(), NcCopyFromBufferOffset::new(0, b"vs"))
        .unwrap();
    let fs = resources
        .create_shader(GpuShaderStage::Fragment, code.as_com_ref(), NcCopyFromBufferOffset::new(0, b"fs"))
        .unwrap();
    let description = PipelineDesc::default().source().unwrap();
    let pipeline = resources.create_pipeline(description.as_com_ref(), vs.as_com_ref(), fs.as_com_ref()).unwrap();
    let broker = unknown
        .query::<IPlatformWindowAbi>()
        .unwrap()
        .graphics_host()
        .unwrap()
        .recording_broker()
        .unwrap();
    let mut frame = broker.begin_frame().unwrap();
    let root = Rect { x: 16, y: 16, width: 32, height: 32 };
    let mut session = frame
        .begin_view(
            RenderViewRoot {
                client: Rect { x: 0, y: 0, width: 32, height: 32 },
                target: root,
            },
            resources.as_com_ref(),
        )
        .unwrap();
    let mut view = session.view().unwrap();
    assert!(view.downgrade().query::<IRendererAbi>().is_err());
    view.clear_color(0xffff0000).unwrap(); // Blue replacement, inside the root only.
    view.bind_pipeline(pipeline.as_com_ref()).unwrap();
    view.draw(3, 1, 0).unwrap();
    view.finish().unwrap();
    let block = session.take_block().unwrap();
    assert_eq!(view.clear_color(0), Err(RENDER_E_EXPIRED));
    frame.finish().unwrap();
    // Recorded backing versions must survive all public shader/pipeline owners.
    drop((view, session, frame, pipeline, vs, fs));
    let vertices = rectangle(Rect { x: 0, y: 0, width: 64, height: 64 }, 0xff00ff00)
        .into_iter()
        .chain(rectangle(Rect { x: 28, y: 28, width: 8, height: 8 }, 0xffff00ff))
        .collect::<Vec<_>>();
    submit_interleaved(
        unknown.query::<IPlatformWindowAbi>().unwrap().graphics_host().unwrap().as_com_ref(),
        renderer.as_com_ref(),
        block,
        &vertices,
    );
    let pixels = device.pixels().unwrap();
    let pixel = |x: usize, y: usize| &pixels[((63 - y) * 64 + x) * 4..((63 - y) * 64 + x) * 4 + 4];
    assert_eq!(pixel(4, 4), [0, 255, 0, 255]);
    assert_eq!(pixel(21, 21), [255, 0, 0, 255]); // Positive normalized Y is visually upward.
    assert_eq!(pixel(21, 42), [0, 0, 255, 255]);
    assert_eq!(pixel(31, 31), [255, 0, 255, 255]); // Later UI remains above generic work.
    assert_eq!(pixel(50, 50), [0, 255, 0, 255]);

    drop((resources, domain, broker, clients, renderer, unknown));
    assert!(!device.is_alive(), "native allocations must not retain their host through a domain");
}

/// Indexed encoded streams, packed attributes, uniforms, texture rows and update versions reach GL.
#[test]
#[ignore = "requires SDL GL; run with SDL_VIDEODRIVER=offscreen"]
fn generic_indexed_texture_and_uniform_snapshots_reach_the_native_pipeline() {
    let _session = native_session();
    let native = testing::window("generic bindings acceptance", SIZE).unwrap();
    let device = native.probe.clone();
    let unknown = native.window;
    let renderer: ComPtr<IRendererAbi> = unknown.query().unwrap();
    let clients: ComPtr<IRenderClientFactoryAbi> = unknown.query().unwrap();
    let domain = clients
        .create_client(
            RenderClientLimits::default(),
            clients
                .query::<IRenderSurfaceFactoryAbi>()
                .unwrap()
                .create_surface(RenderSurfaceDesc::Framebuffer)
                .unwrap()
                .as_com_ref(),
            RenderClientPermissions::default(),
        )
        .unwrap();
    let resources = domain.resources().unwrap();
    let code = Bytes::source(
        br#"
        struct V { @builtin(position) position:vec4<f32>, @location(0) uv:vec2<f32>, @location(1) color:vec4<f32> }
        struct Tint { value:vec4<f32> }
        @group(3) @binding(0) var<uniform> tint:Tint;
        @group(2) @binding(0) var image:texture_2d<f32>;
        @group(2) @binding(1) var image_sampler:sampler;
        @vertex fn vs(@location(0) position:vec2<f32>, @location(1) uv:vec2<f32>, @location(2) color:vec4<f32>) -> V {
            return V(vec4(position,0.5,1.0),uv,color);
        }
        @fragment fn fs(v:V) -> @location(0) vec4<f32> { return textureSample(image,image_sampler,v.uv)*tint.value*v.color; }
    "#
        .to_vec(),
    );
    let vs = resources
        .create_shader(GpuShaderStage::Vertex, code.as_com_ref(), NcCopyFromBufferOffset::new(0, b"vs"))
        .unwrap();
    let fs = resources
        .create_shader(GpuShaderStage::Fragment, code.as_com_ref(), NcCopyFromBufferOffset::new(0, b"fs"))
        .unwrap();
    let description = PipelineDesc {
        vertex_buffers: vec![GpuVertexBufferLayout {
            slot: 0,
            stride: 20,
            step: GpuStepMode::Vertex,
        }],
        attributes: vec![
            GpuVertexAttribute {
                location: 0,
                slot: 0,
                format: GpuVertexFormat::Float32x2,
                offset: 0,
            },
            GpuVertexAttribute {
                location: 1,
                slot: 0,
                format: GpuVertexFormat::Float32x2,
                offset: 8,
            },
            GpuVertexAttribute {
                location: 2,
                slot: 0,
                format: GpuVertexFormat::Unorm8x4,
                offset: 16,
            },
        ],
        bindings: vec![
            GpuShaderBinding {
                stage: GpuShaderStage::Fragment,
                slot: 0,
                kind: GpuBindingKind::Uniform(16),
            },
            GpuShaderBinding {
                stage: GpuShaderStage::Fragment,
                slot: 0,
                kind: GpuBindingKind::SampledTexture,
            },
        ],
        ..PipelineDesc::default()
    }
    .source()
    .unwrap();
    let pipeline = resources.create_pipeline(description.as_com_ref(), vs.as_com_ref(), fs.as_com_ref()).unwrap();
    let mut encoded = Vec::new();
    for values in [[-1f32, 1., 0., 0.], [1., 1., 1., 0.], [1., -1., 1., 1.], [-1., -1., 0., 1.]] {
        encoded.extend(values.into_iter().flat_map(f32::to_le_bytes));
        encoded.extend([255; 4]);
    }
    let vertex_buffer = resources
        .create_static_buffer(GpuBufferDesc::Vertex { byte_len: encoded.len() as u64 }, Bytes::source(encoded).as_com_ref())
        .unwrap();
    let index_buffer = resources
        .create_static_buffer(
            GpuBufferDesc::Index { byte_len: 12 },
            Bytes::source([0u16, 1, 2, 0, 2, 3].into_iter().flat_map(u16::to_le_bytes).collect()).as_com_ref(),
        )
        .unwrap();
    let mut texture = resources
        .create_streamed_buffer(GpuBufferDesc::Texture {
            size: Size { width: 2, height: 2 },
            format: GpuTextureFormat::Rgba8Unorm,
            mip_count: 1,
        })
        .unwrap();
    texture
        .upload(NcCopyFromBufferOffset::new(
            0,
            &[255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 255, 255, 0, 255],
        ))
        .unwrap();
    let mut uniform = resources.create_streamed_buffer(GpuBufferDesc::Uniform { byte_len: 16 }).unwrap();
    let sampler = resources.create_sampler(GpuSamplerDesc::default()).unwrap();
    let broker = unknown
        .query::<IPlatformWindowAbi>()
        .unwrap()
        .graphics_host()
        .unwrap()
        .recording_broker()
        .unwrap();
    let mut frame = broker.begin_frame().unwrap();
    let mut session = frame
        .begin_view(
            RenderViewRoot {
                client: Rect { x: 0, y: 0, width: 48, height: 48 },
                target: Rect { x: 8, y: 8, width: 48, height: 48 },
            },
            resources.as_com_ref(),
        )
        .unwrap();
    let mut view = session.view().unwrap();
    view.bind_pipeline(pipeline.as_com_ref()).unwrap();
    view.bind_vertex_buffer(0, vertex_buffer.as_com_ref(), 0).unwrap();
    view.bind_index_buffer(index_buffer.as_com_ref(), GpuIndexFormat::U16, 0).unwrap();
    view.bind_texture(GpuShaderStage::Fragment, 0, texture.as_com_ref(), sampler.as_com_ref())
        .unwrap();
    let white: Vec<_> = [1f32; 4].into_iter().flat_map(f32::to_le_bytes).collect();
    uniform.upload(NcCopyFromBufferOffset::new(0, &white)).unwrap();
    view.bind_uniform_buffer(GpuShaderStage::Fragment, 0, uniform.as_com_ref(), 0, 16).unwrap();
    view.draw_indexed(6, 1, 0).unwrap();
    // Later draws capture another fixed slot of the same persistent resources; A remains exact.
    texture.upload(NcCopyFromBufferOffset::new(0, &[255; 16])).unwrap();
    view.push_view(Rect { x: 24, y: 0, width: 24, height: 48 }).unwrap();
    let magenta: Vec<_> = [1f32, 0., 1., 1.].into_iter().flat_map(f32::to_le_bytes).collect();
    uniform.upload(NcCopyFromBufferOffset::new(0, &magenta)).unwrap();
    view.draw_indexed(6, 1, 0).unwrap();
    view.pop().unwrap();
    view.finish().unwrap();
    let block = session.take_block().unwrap();
    frame.finish().unwrap();
    drop((pipeline, vs, fs, vertex_buffer, index_buffer, texture, uniform, sampler));
    let vertices = rectangle(Rect { x: 0, y: 0, width: 64, height: 64 }, 0xff00ff00)
        .into_iter()
        .chain(rectangle(Rect { x: 28, y: 28, width: 8, height: 8 }, 0xffffffff))
        .collect::<Vec<_>>();
    submit_interleaved(
        unknown.query::<IPlatformWindowAbi>().unwrap().graphics_host().unwrap().as_com_ref(),
        renderer.as_com_ref(),
        block,
        &vertices,
    );
    let pixels = device.pixels().unwrap();
    let pixel = |x: usize, y: usize| &pixels[((63 - y) * 64 + x) * 4..((63 - y) * 64 + x) * 4 + 4];
    assert_eq!(pixel(12, 12), [255, 0, 0, 255]);
    assert_eq!(pixel(12, 50), [0, 0, 255, 255]);
    assert_eq!(pixel(48, 12), [255, 0, 255, 255]);
    assert_eq!(pixel(48, 50), [255, 0, 255, 255]);
    assert_eq!(pixel(31, 31), [255, 255, 255, 255]);
    assert_eq!(pixel(2, 2), [0, 255, 0, 255]);
}

/// Draws in one view share depth; nested clears and later views affect their authorized regions.
#[test]
#[ignore = "requires SDL GL; run with SDL_VIDEODRIVER=offscreen"]
fn generic_depth_survives_draws_and_reset_with_independent_ordered_views() {
    let _session = native_session();
    let native = testing::window("generic depth and clears", SIZE).unwrap();
    let device = native.probe.clone();
    let window = native.window;
    let renderer: ComPtr<IRendererAbi> = window.query().unwrap();
    let controller = window
        .query::<IRenderClientFactoryAbi>()
        .unwrap()
        .create_client(
            RenderClientLimits::default(),
            framebuffer(window.graphics_host().unwrap().as_com_ref()).as_com_ref(),
            RenderClientPermissions::default(),
        )
        .unwrap();
    let resources = controller.resources().unwrap();
    let code = Bytes::source(
        br#"
        struct Values { color:vec4<f32>, params:vec4<f32> }
        @group(1) @binding(0) var<uniform> values:Values;
        struct V { @builtin(position) p:vec4<f32>, @location(0) color:vec4<f32> }
        @vertex fn vs(@builtin(vertex_index) i:u32) -> V {
            let p = array<vec2<f32>,3>(vec2(-1.0,-1.0),vec2(3.0,-1.0),vec2(-1.0,3.0));
            return V(vec4(p[i],values.params.x,1.0),values.color);
        }
        @fragment fn fs(v:V) -> @location(0) vec4<f32> { return v.color; }
    "#
        .to_vec(),
    );
    let vs = resources
        .create_shader(GpuShaderStage::Vertex, code.as_com_ref(), NcCopyFromBufferOffset::new(0, b"vs"))
        .unwrap();
    let fs = resources
        .create_shader(GpuShaderStage::Fragment, code.as_com_ref(), NcCopyFromBufferOffset::new(0, b"fs"))
        .unwrap();
    let pipeline = |color_write| {
        let desc = PipelineDesc {
            state: GpuPipelineInfo {
                color_write,
                depth_write: true,
                depth_compare: GpuCompare::LessEqual,
                ..GpuPipelineInfo::default()
            },
            bindings: vec![GpuShaderBinding {
                stage: GpuShaderStage::Vertex,
                slot: 0,
                kind: GpuBindingKind::Uniform(32),
            }],
            ..PipelineDesc::default()
        }
        .source()
        .unwrap();
        resources.create_pipeline(desc.as_com_ref(), vs.as_com_ref(), fs.as_com_ref()).unwrap()
    };
    let visible = pipeline(15);
    let masked = pipeline(0);
    let draw = |view: &mut ComBox<IRenderViewAbi>, color: [f32; 4], depth: f32| {
        let bytes: Vec<_> = color.into_iter().chain([depth, 0., 0., 0.]).flat_map(f32::to_le_bytes).collect();
        let buffer = resources
            .create_static_buffer(GpuBufferDesc::Uniform { byte_len: bytes.len() as u64 }, Bytes::source(bytes).as_com_ref())
            .unwrap();
        view.bind_uniform_buffer(GpuShaderStage::Vertex, 0, buffer.as_com_ref(), 0, 32).unwrap();
        view.draw(3, 1, 0).unwrap();
    };
    let broker = window
        .query::<IPlatformWindowAbi>()
        .unwrap()
        .graphics_host()
        .unwrap()
        .recording_broker()
        .unwrap();
    let mut frame = broker.begin_frame().unwrap();
    let begin = |frame: &mut ComBox<IRenderRecordingFrameAbi>, target: Rect| {
        let mut session = frame
            .begin_view(
                RenderViewRoot {
                    client: Rect {
                        x: 0,
                        y: 0,
                        width: target.width,
                        height: target.height,
                    },
                    target,
                },
                resources.as_com_ref(),
            )
            .unwrap();
        let view = session.view().unwrap();
        (session, view)
    };
    let (mut first, mut a) = begin(&mut frame, Rect { x: 4, y: 4, width: 56, height: 56 });
    a.bind_pipeline(visible.as_com_ref()).unwrap();
    draw(&mut a, [1., 0., 0., 1.], 0.25);
    a.push_clip(Rect { x: 28, y: 0, width: 28, height: 56 }).unwrap();
    a.clear_depth(1.0).unwrap();
    draw(&mut a, [0., 0., 1., 1.], 0.75);
    a.pop().unwrap();
    a.reset().unwrap();
    draw(&mut a, [1., 1., 0., 1.], 0.9); // Fails existing red/blue depth; reset must not clear it.
    a.finish().unwrap();
    let first = first.take_block().unwrap();
    let (mut second, mut b) = begin(&mut frame, Rect { x: 20, y: 20, width: 24, height: 24 });
    b.bind_pipeline(visible.as_com_ref()).unwrap();
    draw(&mut b, [0., 1., 0., 1.], 0.75); // Fresh bounded depth overrides earlier nearer red.
    b.finish().unwrap();
    let second = second.take_block().unwrap();
    let (mut third, mut c) = begin(&mut frame, Rect { x: 1, y: 1, width: 2, height: 2 });
    c.bind_pipeline(masked.as_com_ref()).unwrap();
    draw(&mut c, [1., 0., 0., 1.], 0.5);
    c.clear_color(0xffffffff).unwrap(); // Clear ignores the preceding pipeline's zero write mask.
    c.finish().unwrap();
    let third = third.take_block().unwrap();
    frame.finish().unwrap();
    let source = super::view_geometry::ViewStream::from_value(super::view_geometry::ViewStream { blocks: vec![first, second, third] })
        .query::<IRenderStreamAbi>()
        .unwrap();
    renderer
        .render(
            RenderInfo {
                size: SIZE,
                clear: 0xff13110f,
                invocation: 3,
                block_count: 3,
            },
            source.as_com_ref(),
        )
        .unwrap();
    let pixels = device.pixels().unwrap();
    let pixel = |x: usize, y: usize| &pixels[((63 - y) * 64 + x) * 4..((63 - y) * 64 + x) * 4 + 4];
    assert_eq!(pixel(6, 6), [255, 0, 0, 255]);
    assert_eq!(pixel(54, 6), [0, 0, 255, 255]);
    assert_eq!(pixel(24, 24), [0, 255, 0, 255]);
    assert_eq!(pixel(40, 40), [0, 255, 0, 255]);
    assert_eq!(pixel(1, 1), [255, 255, 255, 255]);
    assert_eq!(pixel(0, 0), [15, 17, 19, 255]);
}
