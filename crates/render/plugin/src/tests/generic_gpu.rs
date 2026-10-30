//! Generic GPU acceptance through published COM contracts and the production native executor.
use super::*;
use neocogi_com::{ComBox, WeakComBox};
use neocogi_render_gpu::source::{Bytes, PipelineDesc};

#[com_object(interfaces(IRgbaImage))]
/// Deterministic UI atlas; this GPU test needs an opaque white texel, without font metadata.
struct White;
impl IRgbaImage for White {
    /// Exactly one complete initialized texel.
    fn size(&self) -> Result<Size, NcStatus> {
        Ok(Size { width: 1, height: 1 })
    }
    /// Supplies the exact requested prefix through the real image ABI.
    fn pixels(&self, mut output: NcOutputWriter<'_, u8>) -> Result<(), NcStatus> {
        let first = output.offset() as usize;
        let end = first + output.capacity();
        output.extend_from_slice([255; 4].get(first..end).ok_or(NC_E_INVALID_ARG)?)
    }
}

#[com_object(interfaces(IRenderStream))]
/// Owns one sealed contribution and ordinary UI geometry at later painter positions.
struct Source {
    /// Exclusive ownership lasts through the synchronous renderer call.
    block: ComBox<IRenderViewBlockAbi>,
    /// Green background and a later magenta overlay are ordinary UI triangles.
    vertices: Vec<RenderVertex>,
}
impl IRenderStream for Source {
    /// Observes only the declared generic position.
    fn view(&self, _: u64, index: u32) -> Result<WeakComBox<IRenderViewBlockAbi>, NcStatus> {
        if index != 1 {
            return Err(NC_E_INVALID_ARG);
        }
        Ok(self.block.downgrade())
    }
    /// Copies complete ordinary geometry around the generic contribution.
    fn vertices(&self, _: u64, mut output: NcOutputWriter<'_, RenderVertex>) -> Result<u32, NcStatus> {
        output.extend_from_slice(&self.vertices)?;
        Ok(self.vertices.len() as u32)
    }
    /// Preserves atlas, generic, atlas order without a semantic renderer command.
    fn batches(&self, _: u64, mut output: NcOutputWriter<'_, RenderBatch>) -> Result<u32, NcStatus> {
        output.extend_from_slice(&[
            RenderBatch::Atlas { first: 0, count: 6 },
            RenderBatch::View,
            RenderBatch::Atlas { first: 6, count: 6 },
        ])?;
        Ok(3)
    }
    /// No ordinary image capabilities are requested by this fixture.
    fn texture(&self, _: u64, _: u32) -> Result<ComPtr<ITextureAbi>, NcStatus> {
        Err(NC_E_INVALID_ARG)
    }
    /// Generic work never falls back to the semantic scene path.
    fn item(&self, _: u64, _: u32) -> Result<ComPtr<IRenderItemAbi>, NcStatus> {
        panic!("generic block requested as semantic scene")
    }
}

/// Encodes an ordinary white-atlas rectangle using the actual renderer wire values.
fn rectangle(rect: Rect, color: u32) -> Vec<RenderVertex> {
    let q = quad(rect).map(|vertex| RenderVertex { u: 0., v: 0., rgba: color, ..vertex });
    vec![q[0], q[1], q[2], q[0], q[2], q[3]]
}

/// Asymmetry proves normalized position orientation; color clear and later UI prove confinement/order.
#[test]
#[ignore = "requires SDL GL; run with SDL_VIDEODRIVER=offscreen"]
fn generic_pipeline_renders_in_its_root_and_preserves_surrounding_ui() {
    let _session = native_session();
    let platform = PlatformObject::default();
    let atlas: ComPtr<IRgbaImageAbi> = White::from_value(White).query().unwrap();
    let native = platform.window("generic graphics acceptance", SIZE, atlas.as_com_ref()).unwrap();
    let device = Rc::downgrade(&native.device);
    let unknown = WindowObject::from_value(native);
    let renderer: ComPtr<IRendererAbi> = unknown.query().unwrap();
    let clients: ComPtr<IRenderClientFactoryAbi> = unknown.query().unwrap();
    let domain = clients.create_client(RenderClientLimits::default()).unwrap();
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
    let broker = unknown.query::<IRenderHostAbi>().unwrap().recording_broker().unwrap();
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
        .collect();
    let source = Source::from_value(Source { block, vertices }).query::<IRenderStreamAbi>().unwrap();
    renderer
        .render(
            RenderInfo {
                size: SIZE,
                clear: 0xff000000,
                invocation: 1,
                vertex_count: 12,
                batch_count: 3,
            },
            atlas.as_com_ref(),
            source.as_com_ref(),
        )
        .unwrap();
    let owner = device.upgrade().unwrap();
    let access = owner.access().unwrap();
    let pixels = access.state.as_ref().unwrap().gpu.last_pixels();
    let pixel = |x: usize, y: usize| &pixels[((63 - y) * 64 + x) * 4..((63 - y) * 64 + x) * 4 + 4];
    assert_eq!(pixel(4, 4), [0, 255, 0, 255]);
    assert_eq!(pixel(21, 21), [255, 0, 0, 255]); // Positive normalized Y is visually upward.
    assert_eq!(pixel(21, 42), [0, 0, 255, 255]);
    assert_eq!(pixel(31, 31), [255, 0, 255, 255]); // Later UI remains above generic work.
    assert_eq!(pixel(50, 50), [0, 255, 0, 255]);
    drop(access);
    drop((owner, source, resources, domain, broker, clients, renderer, unknown));
    assert!(device.upgrade().is_none(), "native caches must not retain their host through a domain");
}

/// Indexed encoded streams, packed attributes, uniforms, texture rows and update versions reach GL.
#[test]
#[ignore = "requires SDL GL; run with SDL_VIDEODRIVER=offscreen"]
fn generic_indexed_texture_and_uniform_snapshots_reach_the_native_pipeline() {
    let _session = native_session();
    let platform = PlatformObject::default();
    let atlas: ComPtr<IRgbaImageAbi> = White::from_value(White).query().unwrap();
    let native = platform.window("generic bindings acceptance", SIZE, atlas.as_com_ref()).unwrap();
    let device = Rc::downgrade(&native.device);
    let unknown = WindowObject::from_value(native);
    let renderer: ComPtr<IRendererAbi> = unknown.query().unwrap();
    let clients: ComPtr<IRenderClientFactoryAbi> = unknown.query().unwrap();
    let domain = clients.create_client(RenderClientLimits::default()).unwrap();
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
                kind: GpuBindingKind::Uniform,
                byte_len: 16,
            },
            GpuShaderBinding {
                stage: GpuShaderStage::Fragment,
                slot: 0,
                kind: GpuBindingKind::SampledTexture,
                byte_len: 0,
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
        .create_buffer(
            GpuBufferDesc {
                role: GpuBufferRole::Vertex,
                byte_len: encoded.len() as u64,
            },
            Bytes::source(encoded).as_com_ref(),
        )
        .unwrap();
    let index_buffer = resources
        .create_buffer(
            GpuBufferDesc { role: GpuBufferRole::Index, byte_len: 12 },
            Bytes::source([0u16, 1, 2, 0, 2, 3].into_iter().flat_map(u16::to_le_bytes).collect()).as_com_ref(),
        )
        .unwrap();
    let texture = resources
        .create_texture(
            GpuTextureDesc {
                size: Size { width: 2, height: 2 },
                format: GpuTextureFormat::Rgba8Unorm,
                mip_count: 1,
            },
            Bytes::source(vec![255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 255, 255, 0, 255]).as_com_ref(),
        )
        .unwrap();
    let sampler = resources.create_sampler(GpuSamplerDesc::default()).unwrap();
    let broker = unknown.query::<IRenderHostAbi>().unwrap().recording_broker().unwrap();
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
    view.push_uniforms(GpuShaderStage::Fragment, 0, NcCopyFromBufferOffset::new(0, &white)).unwrap();
    view.draw_indexed(6, 1, 0).unwrap();
    // Later draws see a new texture and new copied uniforms; the earlier draw remains exact A.
    resources
        .update_texture(
            texture.as_com_ref(),
            Rect { x: 0, y: 0, width: 2, height: 2 },
            Bytes::source(vec![255; 16]).as_com_ref(),
        )
        .unwrap();
    view.push_view(Rect { x: 24, y: 0, width: 24, height: 48 }).unwrap();
    let magenta: Vec<_> = [1f32, 0., 1., 1.].into_iter().flat_map(f32::to_le_bytes).collect();
    view.push_uniforms(GpuShaderStage::Fragment, 0, NcCopyFromBufferOffset::new(0, &magenta))
        .unwrap();
    view.draw_indexed(6, 1, 0).unwrap();
    view.pop().unwrap();
    view.finish().unwrap();
    let block = session.take_block().unwrap();
    frame.finish().unwrap();
    drop((pipeline, vs, fs, vertex_buffer, index_buffer, texture, sampler));
    let vertices = rectangle(Rect { x: 0, y: 0, width: 64, height: 64 }, 0xff00ff00)
        .into_iter()
        .chain(rectangle(Rect { x: 28, y: 28, width: 8, height: 8 }, 0xffffffff))
        .collect();
    let source = Source::from_value(Source { block, vertices }).query::<IRenderStreamAbi>().unwrap();
    renderer
        .render(
            RenderInfo {
                size: SIZE,
                clear: 0xff000000,
                invocation: 2,
                vertex_count: 12,
                batch_count: 3,
            },
            atlas.as_com_ref(),
            source.as_com_ref(),
        )
        .unwrap();
    let pixels = {
        let device = device.upgrade().unwrap();
        let access = device.access().unwrap();
        access.state.as_ref().unwrap().gpu.last_pixels().to_vec()
    };
    let pixel = |x: usize, y: usize| &pixels[((63 - y) * 64 + x) * 4..((63 - y) * 64 + x) * 4 + 4];
    assert_eq!(pixel(12, 12), [255, 0, 0, 255]);
    assert_eq!(pixel(12, 50), [0, 0, 255, 255]);
    assert_eq!(pixel(48, 12), [255, 0, 255, 255]);
    assert_eq!(pixel(48, 50), [255, 0, 255, 255]);
    assert_eq!(pixel(31, 31), [255, 255, 255, 255]);
    assert_eq!(pixel(2, 2), [0, 255, 0, 255]);
}
