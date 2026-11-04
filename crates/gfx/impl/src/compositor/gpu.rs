//! GFX-owned UI layout, shaders and lowering into the common graphics command vocabulary.
use super::{Batch, FrameState, RenderVertex};
use neocogi_gfx_support::{MAX_UI_BATCHES, MAX_UI_VERTICES};
use neocogi_com::{
    com_object, ComBox, ComPtr, ComRef, NcCopyFromBufferOffset, NcOutputBuffer, NcOutputWriter, NcStatus, NC_E_INVALID_ARG, NC_E_OUT_OF_MEMORY,
    NC_E_OUT_OF_RANGE,
};
use neocogi_render_core::{
    abi::*,
    source::{Bytes, PipelineDesc},
};
use std::{collections::HashMap, rc::Rc};

/// UI's explicit GPU layout belongs here, independent of generated Rust record layout.
const STRIDE: usize = 20;
/// Every upload ends on a complete triangle and stays within the common 64 MiB buffer limit.
const CHUNK_VERTICES: usize = (64 * 1024 * 1024 / STRIDE) / 3 * 3;
/// A single pipeline shades solid white-tile, atlas and ordinary-image UI geometry alike.
const SHADER: &str = r#"
struct Frame { size:vec4<f32> }
@group(1) @binding(0) var<uniform> frame:Frame;
@group(2) @binding(0) var image:texture_2d<f32>;
@group(2) @binding(1) var image_sampler:sampler;
struct V { @builtin(position) position:vec4<f32>, @location(0) uv:vec2<f32>, @location(1) color:vec4<f32> }
@vertex fn vs(@location(0) p:vec2<f32>, @location(1) uv:vec2<f32>, @location(2) color:vec4<f32>) -> V {
    return V(vec4(p.x*2.0/frame.size.x-1.0,1.0-p.y*2.0/frame.size.y,0.0,1.0),uv,color);
}
@fragment fn fs(v:V) -> @location(0) vec4<f32> { return textureSample(image,image_sampler,v.uv)*v.color; }
"#;

/// One private compositor session owns its factory, pipeline, atlas and native-lifetime authority.
pub(super) struct Graphics {
    /// No host getter is exposed by GFX or widget services.
    host: ComPtr<IRenderHostAbi>,
    /// Closing this exclusive controller revokes later admission without cancelling submitted data.
    _domain: ComBox<IRenderClientDomainAbi>,
    /// Private color/image resources carry only the permissions explicitly selected for this compositor.
    resources: ComPtr<IRenderResourcesAbi>,
    /// Separate CPU frame accounting for this compositor's own UI work.
    broker: ComPtr<IRenderViewBrokerAbi>,
    /// Explicit GFX vertex/uniform/resource layouts and alpha blend policy.
    pipeline: ComBox<IGpuPipelineAbi>,
    /// Nearest/clamped sampling matches the UI asset and immutable-image contract.
    sampler: ComBox<IGpuSamplerAbi>,
    /// Immutable atlas version is published with the matching resource metadata.
    atlas: ComBox<IGpuTextureAbi>,
}

impl Graphics {
    /// Builds and uploads a complete candidate before replacing the installed session.
    /// The host authenticates the selected surface; the resource group retains its target
    /// after the caller releases the public surface capability.
    pub(super) fn new(host: ComRef<'_, IRenderHostAbi>, surface: ComRef<'_, IRenderSurfaceAbi>, image: ComRef<'_, IRgbaImageAbi>) -> Result<Self, RenderError> {
        let host = host.try_add_ref()?;
        let clients = host.query::<IRenderClientFactoryAbi>()?;
        let domain = clients.create_client(
            RenderClientLimits {
                resources: 65_536,
                live_bytes: 1024 * 1024 * 1024,
                pending_bytes: 64 * 1024 * 1024,
                frame_draws: MAX_UI_BATCHES + 1,
                frame_indices: u64::from(MAX_UI_VERTICES) * 2 + 3,
                live_views: MAX_UI_BATCHES + 1,
                stack_depth: 1,
            },
            surface,
            neocogi_gfx_support::compositor_permissions(),
        )?;
        let resources = domain.resources()?;
        let broker = host.recording_broker()?;
        let code = Bytes::source(SHADER.as_bytes().to_vec());
        let vertex = resources.create_shader(GpuShaderStage::Vertex, code.as_com_ref(), NcCopyFromBufferOffset::new(0, b"vs"))?;
        let fragment = resources.create_shader(GpuShaderStage::Fragment, code.as_com_ref(), NcCopyFromBufferOffset::new(0, b"fs"))?;
        let desc = PipelineDesc {
            state: GpuPipelineInfo {
                blend: GpuBlendState {
                    enabled: true,
                    src_color: GpuBlendFactor::SrcAlpha,
                    dst_color: GpuBlendFactor::OneMinusSrcAlpha,
                    color_op: GpuBlendOp::Add,
                    src_alpha: GpuBlendFactor::One,
                    dst_alpha: GpuBlendFactor::OneMinusSrcAlpha,
                    alpha_op: GpuBlendOp::Add,
                },
                ..Default::default()
            },
            vertex_buffers: vec![GpuVertexBufferLayout {
                slot: 0,
                stride: STRIDE as u32,
                step: GpuStepMode::Vertex,
            }],
            attributes: vec![
                GpuVertexAttribute {
                    location: 0,
                    slot: 0,
                    offset: 0,
                    format: GpuVertexFormat::Float32x2,
                },
                GpuVertexAttribute {
                    location: 1,
                    slot: 0,
                    offset: 8,
                    format: GpuVertexFormat::Float32x2,
                },
                GpuVertexAttribute {
                    location: 2,
                    slot: 0,
                    offset: 16,
                    format: GpuVertexFormat::Unorm8x4,
                },
            ],
            bindings: vec![
                GpuShaderBinding {
                    stage: GpuShaderStage::Vertex,
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
        }
        .source()?;
        let pipeline = resources.create_pipeline(desc.as_com_ref(), vertex.as_com_ref(), fragment.as_com_ref())?;
        let sampler = resources.create_sampler(GpuSamplerDesc::default())?;
        let atlas = Self::upload_atlas(host.as_com_ref(), resources.as_com_ref(), image)?;
        Ok(Self {
            host,
            _domain: domain,
            resources,
            broker,
            pipeline,
            sampler,
            atlas,
        })
    }

    /// Complete native preparation precedes atomic metadata/atlas publication in the parent.
    pub(super) fn candidate_atlas(&self, image: ComRef<'_, IRgbaImageAbi>) -> Result<ComBox<IGpuTextureAbi>, RenderError> {
        Self::upload_atlas(self.host.as_com_ref(), self.resources.as_com_ref(), image)
    }

    /// No failure remains when the parent publishes a prepared atlas with its resource revision.
    pub(super) fn replace_atlas(&mut self, atlas: ComBox<IGpuTextureAbi>) {
        self.atlas = atlas;
    }

    /// A bounded source forwards immutable CPU image reads without copying the entire atlas twice.
    fn upload_atlas(
        host: ComRef<'_, IRenderHostAbi>,
        resources: ComRef<'_, IRenderResourcesAbi>,
        image: ComRef<'_, IRgbaImageAbi>,
    ) -> Result<ComBox<IGpuTextureAbi>, RenderError> {
        let size = image.size()?;
        let source: ComPtr<IGpuDataSourceAbi> = ImageBytes::from_value(ImageBytes { image: image.try_add_ref()?, size }).query()?;
        let atlas = resources.create_texture(
            GpuTextureDesc {
                size,
                format: GpuTextureFormat::Rgba8Unorm,
                mip_count: 1,
            },
            neocogi_render_core::source::supplied(source.as_com_ref())?.borrow()?,
        )?;
        host.prepare_texture(resources, atlas.as_com_ref())?;
        Ok(atlas)
    }

    /// Pack geometry once, then preserve client positions with UI blocks using the same resources.
    pub(super) fn lower(&self, frame: FrameState, vertices: &[RenderVertex], batches: &[Batch]) -> Result<Vec<Rc<ComBox<IRenderViewBlockAbi>>>, RenderError> {
        // A previous frame can release its final block only after renderer.render returns.
        // Drain before allocating geometry so quota pressure cannot prevent reaching maintenance.
        self.host.maintain()?;
        let buffers = self.geometry(vertices, batches)?;
        let mut recording = self.broker.begin_frame()?;
        let mut active: Option<Recording> = None;
        let mut output = Vec::new();
        output.try_reserve(batches.len()).map_err(|_| NC_E_OUT_OF_MEMORY)?;
        let mut images = HashMap::<usize, ComBox<IGpuTextureAbi>>::new();
        let mut cursor = 0usize;
        for batch in batches {
            let (count, image) = match batch {
                Batch::View(block) => {
                    seal(&mut active, &mut output)?;
                    output.push(block.clone());
                    continue;
                }
                Batch::Atlas { count, .. } => (*count as usize, self.atlas.as_com_ref()),
                Batch::Texture { texture, .. } => {
                    let key = texture.as_raw().object as usize;
                    if let std::collections::hash_map::Entry::Vacant(entry) = images.entry(key) {
                        entry.insert(self.host.import_texture(self.resources.as_com_ref(), texture.as_com_ref())?);
                    }
                    (6, images.get(&key).expect("import installed above").as_com_ref())
                }
            };
            if active.is_none() {
                let root = Rect {
                    x: 0,
                    y: 0,
                    width: frame.size.width,
                    height: frame.size.height,
                };
                let mut session = recording.begin_view(RenderViewRoot { client: root, target: root }, self.resources.as_com_ref())?;
                let mut view = session.view()?;
                view.bind_pipeline(self.pipeline.as_com_ref())?;
                let values: Vec<_> = [frame.size.width as f32, frame.size.height as f32, 0., 0.]
                    .into_iter()
                    .flat_map(f32::to_le_bytes)
                    .collect();
                view.push_uniforms(GpuShaderStage::Vertex, 0, NcCopyFromBufferOffset::new(0, &values))?;
                active = Some(Recording { session, view });
            }
            let view = &mut active.as_mut().expect("UI session installed above").view;
            view.bind_texture(GpuShaderStage::Fragment, 0, image, self.sampler.as_com_ref())?;
            let mut remaining = count;
            while remaining > 0 {
                let chunk = cursor / CHUNK_VERTICES;
                let first = cursor % CHUNK_VERTICES;
                let count = remaining.min(CHUNK_VERTICES - first);
                view.bind_vertex_buffer(0, buffers.get(chunk).ok_or(NC_E_INVALID_ARG)?.as_com_ref(), 0)?;
                view.draw(count as u32, 1, first as u32)?;
                cursor += count;
                remaining -= count;
            }
        }
        seal(&mut active, &mut output)?;
        recording.finish()?;
        Ok(output)
    }

    /// Triangle-aligned shared buffers avoid one native allocation per UI operation or panel.
    fn geometry(&self, vertices: &[RenderVertex], batches: &[Batch]) -> Result<Vec<ComBox<IGpuBufferAbi>>, NcStatus> {
        let chunks = pack_geometry(vertices, batches, CHUNK_VERTICES)?;
        let mut output = Vec::new();
        output.try_reserve_exact(chunks.len()).map_err(|_| NC_E_OUT_OF_MEMORY)?;
        for bytes in chunks {
            output.push(self.buffer(bytes)?);
        }
        Ok(output)
    }

    /// Byte ownership is explicit; no Rust/IDL struct layout becomes GPU layout.
    fn buffer(&self, bytes: Vec<u8>) -> Result<ComBox<IGpuBufferAbi>, NcStatus> {
        let desc = GpuBufferDesc {
            role: GpuBufferRole::Vertex,
            byte_len: bytes.len() as u64,
        };
        self.resources
            .create_buffer(desc, neocogi_render_core::source::supplied(Bytes::source(bytes).as_com_ref())?.borrow()?)
    }
}

/// Packs complete triangle-aligned chunks before publishing any resource. Source batches append
/// slices into pre-sized rows, removing a function call and vector growth bookkeeping per vertex.
/// Each byte remains explicitly encoded; flattening transfers the same owned row allocation.
fn pack_geometry(vertices: &[RenderVertex], batches: &[Batch], chunk_vertices: usize) -> Result<Vec<Vec<u8>>, NcStatus> {
    if chunk_vertices == 0 || !chunk_vertices.is_multiple_of(3) {
        return Err(NC_E_INVALID_ARG);
    }
    let total = batches.iter().try_fold(0usize, |total, batch| {
        total
            .checked_add(match batch {
                Batch::View(_) => 0,
                Batch::Atlas { count, .. } => *count as usize,
                Batch::Texture { .. } => 6,
            })
            .ok_or(NC_E_OUT_OF_RANGE)
    })?;
    let mut chunks = Vec::new();
    chunks.try_reserve_exact(total.div_ceil(chunk_vertices)).map_err(|_| NC_E_OUT_OF_MEMORY)?;
    let mut bytes: Vec<[u8; STRIDE]> = Vec::new();
    let size = total.min(chunk_vertices);
    bytes.try_reserve_exact(size).map_err(|_| NC_E_OUT_OF_MEMORY)?;
    bytes.resize(size, [0; STRIDE]);
    let mut cursor = 0;
    let mut remaining = total;
    for batch in batches {
        let mut append = |mut source: &[RenderVertex]| -> Result<(), NcStatus> {
            while !source.is_empty() {
                let count = source.len().min(bytes.len() - cursor);
                let (prefix, tail) = source.split_at(count);
                for (destination, vertex) in bytes[cursor..cursor + count].iter_mut().zip(prefix) {
                    *destination = encode_vertex(*vertex);
                }
                cursor += count;
                source = tail;
                if cursor == bytes.len() {
                    remaining -= bytes.len();
                    chunks.push(std::mem::take(&mut bytes).into_flattened());
                    let size = remaining.min(chunk_vertices);
                    bytes.try_reserve_exact(size).map_err(|_| NC_E_OUT_OF_MEMORY)?;
                    bytes.resize(size, [0; STRIDE]);
                    cursor = 0;
                }
            }
            Ok(())
        };
        match batch {
            Batch::View(_) => {}
            Batch::Atlas { first, count } => {
                let range = *first as usize..(*first as usize).checked_add(*count as usize).ok_or(NC_E_OUT_OF_RANGE)?;
                append(vertices.get(range).ok_or(NC_E_INVALID_ARG)?)?;
            }
            Batch::Texture { first, .. } => {
                let first = *first as usize;
                let end = first.checked_add(4).ok_or(NC_E_OUT_OF_RANGE)?;
                let quad = vertices.get(first..end).ok_or(NC_E_INVALID_ARG)?;
                append(&[quad[0], quad[1], quad[2], quad[0], quad[2], quad[3]])?;
            }
        }
    }
    debug_assert_eq!(remaining, 0);
    debug_assert_eq!(cursor, 0);
    Ok(chunks)
}

/// Encodes the portable vertex layout once, independently of native endianness and padding.
fn encode_vertex(vertex: RenderVertex) -> [u8; STRIDE] {
    let mut packed = [0u8; STRIDE];
    // Fixed offsets let min-size combine these stores instead of emitting a five-step loop.
    packed[0..4].copy_from_slice(&vertex.x.to_le_bytes());
    packed[4..8].copy_from_slice(&vertex.y.to_le_bytes());
    packed[8..12].copy_from_slice(&vertex.u.to_le_bytes());
    packed[12..16].copy_from_slice(&vertex.v.to_le_bytes());
    packed[16..20].copy_from_slice(&vertex.rgba.to_le_bytes());
    packed
}

/// One contiguous UI contribution; client blocks force sealing at their original painter position.
struct Recording {
    /// Host authority expires an unfinished view on any lowering error.
    session: ComBox<IRenderViewSessionAbi>,
    /// Bounded root covers this trusted UI frame; depth authority remains disabled by its domain.
    view: ComBox<IRenderViewAbi>,
}

/// Sealing transfers one exclusive block; the active session is never reused across a client insertion.
fn seal(active: &mut Option<Recording>, output: &mut Vec<Rc<ComBox<IRenderViewBlockAbi>>>) -> Result<(), NcStatus> {
    if let Some(mut active) = active.take() {
        active.view.finish()?;
        output.try_reserve(1).map_err(|_| NC_E_OUT_OF_MEMORY)?;
        output.push(Rc::new(active.session.take_block()?));
    }
    Ok(())
}

#[com_object(interfaces(IGpuDataSource))]
/// Bounded immutable image adapter owns no native renderer or widget state.
struct ImageBytes {
    /// Native preparation and UI metadata publication refer to this exact image revision.
    image: ComPtr<IRgbaImageAbi>,
    /// Header is rechecked before and after the factory's bounded bulk traversal.
    size: Size,
}
impl IGpuDataSource for ImageBytes {
    /// A changing producer header rejects instead of silently changing upload interpretation.
    fn byte_count(&self) -> Result<u64, NcStatus> {
        if self.image.size()? != self.size {
            return Err(NC_E_INVALID_ARG);
        }
        Ok(rgba_bytes(self.size)? as u64)
    }
    /// The resource factory bounds each requested interval to its fixed upload chunk size.
    fn read(&self, mut output: NcOutputWriter<'_, u8>) -> Result<(), NcStatus> {
        let len = self.byte_count()?;
        if output.offset().checked_add(output.capacity() as u64).filter(|end| *end <= len).is_none() {
            return Err(NC_E_OUT_OF_RANGE);
        }
        let mut bytes = NcOutputBuffer::new();
        self.image.pixels(bytes.output(output.offset(), output.capacity()))?;
        if bytes.len() != output.capacity() {
            return Err(NC_E_INVALID_ARG);
        }
        output.extend_from_slice(bytes.as_slice())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[com_object(interfaces(ITexture))]
    /// Retained capability is irrelevant to pure packing; no foreign texture method is needed.
    struct UnusedTexture;
    impl IRenderDriverBound for UnusedTexture {
        fn driver_identity(&self) -> Result<ComPtr<IRenderDriverIdentityAbi>, NcStatus> {
            Err(neocogi_com::NC_E_NOINTERFACE)
        }
    }
    impl ITexture for UnusedTexture {
        fn size(&self) -> Result<Size, NcStatus> {
            Err(NC_E_INVALID_ARG)
        }
    }

    /// Quad expansion and adjacent atlas ranges retain winding even at the smallest chunk size.
    #[test]
    fn bulk_geometry_expands_texture_quads_before_following_atlas_vertices() {
        let texture = Rc::new(UnusedTexture::from_value(UnusedTexture).query::<ITextureAbi>().unwrap());
        let vertices = (0..7).map(|index| RenderVertex { x: index as f32, ..Default::default() }).collect::<Vec<_>>();
        let batches = [Batch::Texture { first: 0, texture }, Batch::Atlas { first: 4, count: 3 }];
        let chunks = pack_geometry(&vertices, &batches, 3).unwrap();
        assert_eq!(chunks.len(), 3);
        let positions = chunks
            .iter()
            .flat_map(|chunk| chunk.as_chunks::<STRIDE>().0.iter())
            .map(|vertex| f32::from_le_bytes(vertex[..4].try_into().unwrap()))
            .collect::<Vec<_>>();
        assert_eq!(positions, [0., 1., 2., 0., 2., 3., 4., 5., 6.]);
    }

    /// Bulk writes keep batch order and complete triangles across full and partial chunks.
    #[test]
    fn bulk_geometry_preserves_triangle_order_across_chunk_boundaries() {
        let vertices = (0..21).map(|index| RenderVertex { x: index as f32, ..Default::default() }).collect::<Vec<_>>();
        let batches = [
            Batch::Atlas { first: 3, count: 9 },
            Batch::Atlas { first: 0, count: 0 },
            Batch::Atlas { first: 15, count: 6 },
        ];
        let chunks = pack_geometry(&vertices, &batches, 6).unwrap();
        assert_eq!(chunks.iter().map(Vec::len).collect::<Vec<_>>(), [6 * STRIDE, 6 * STRIDE, 3 * STRIDE]);
        let actual = chunks
            .iter()
            .flat_map(|chunk| chunk.as_chunks::<STRIDE>().0.iter())
            .map(|vertex| f32::from_le_bytes(vertex[..4].try_into().unwrap()))
            .collect::<Vec<_>>();
        assert_eq!(actual, [3., 4., 5., 6., 7., 8., 9., 10., 11., 15., 16., 17., 18., 19., 20.]);
        assert!(pack_geometry(&vertices, &[], 6).unwrap().is_empty());
        assert_eq!(
            pack_geometry(&vertices, &[Batch::Atlas { first: 20, count: 3 }], 6).unwrap_err(),
            NC_E_INVALID_ARG
        );
    }

    /// GPU byte order includes float sign bits and RGBA order independently of struct layout.
    #[test]
    fn packed_vertex_matches_portable_shader_layout() {
        let vertex = RenderVertex {
            x: 1.,
            y: -2.5,
            u: 0.5,
            v: -0.,
            rgba: 0x7856_3412,
        };
        assert_eq!(
            encode_vertex(vertex),
            [
                0x00, 0x00, 0x80, 0x3f, 0x00, 0x00, 0x20, 0xc0, 0x00, 0x00, 0x00, 0x3f, 0x00, 0x00, 0x00, 0x80, 0x12, 0x34, 0x56, 0x78,
            ]
        );
    }
}
