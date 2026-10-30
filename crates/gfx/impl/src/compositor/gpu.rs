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
    /// Never delegated to widgets: only this factory has trusted compositor provenance.
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
    /// Builds and uploads a complete candidate before replacing any installed graphics session.
    pub(super) fn new(host: ComRef<'_, IRenderHostAbi>, image: ComRef<'_, IRgbaImageAbi>) -> Result<Self, NcStatus> {
        let host = host.try_add_ref()?;
        let domain = host.create_ui_client(RenderClientLimits {
            resources: 65_536,
            live_bytes: 1024 * 1024 * 1024,
            pending_bytes: 64 * 1024 * 1024,
            frame_draws: MAX_UI_BATCHES + 1,
            frame_indices: u64::from(MAX_UI_VERTICES) * 2 + 3,
            live_views: MAX_UI_BATCHES + 1,
            stack_depth: 1,
        })?;
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
    pub(super) fn candidate_atlas(&self, image: ComRef<'_, IRgbaImageAbi>) -> Result<ComBox<IGpuTextureAbi>, NcStatus> {
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
    ) -> Result<ComBox<IGpuTextureAbi>, NcStatus> {
        let size = image.size()?;
        let source: ComPtr<IGpuDataSourceAbi> = ImageBytes::from_value(ImageBytes { image: image.try_add_ref()?, size }).query()?;
        let atlas = resources.create_texture(
            GpuTextureDesc {
                size,
                format: GpuTextureFormat::Rgba8Unorm,
                mip_count: 1,
            },
            source.as_com_ref(),
        )?;
        host.prepare_ui_texture(resources, atlas.as_com_ref())?;
        Ok(atlas)
    }

    /// Pack geometry once, then preserve client positions with UI blocks using the same resources.
    pub(super) fn lower(&self, frame: FrameState, vertices: &[RenderVertex], batches: &[Batch]) -> Result<Vec<Rc<ComBox<IRenderViewBlockAbi>>>, NcStatus> {
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
                        entry.insert(self.host.import_ui_texture(self.resources.as_com_ref(), texture.as_com_ref())?);
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
        let total = batches.iter().try_fold(0usize, |total, batch| {
            total
                .checked_add(match batch {
                    Batch::View(_) => 0,
                    Batch::Atlas { count, .. } => *count as usize,
                    Batch::Texture { .. } => 6,
                })
                .ok_or(NC_E_OUT_OF_RANGE)
        })?;
        let mut output = Vec::new();
        output.try_reserve_exact(total.div_ceil(CHUNK_VERTICES)).map_err(|_| NC_E_OUT_OF_MEMORY)?;
        let mut bytes = Vec::new();
        bytes.try_reserve_exact(total.min(CHUNK_VERTICES) * STRIDE).map_err(|_| NC_E_OUT_OF_MEMORY)?;
        for batch in batches {
            let mut append = |vertex: RenderVertex| -> Result<(), NcStatus> {
                for value in [vertex.x, vertex.y, vertex.u, vertex.v] {
                    bytes.extend_from_slice(&value.to_le_bytes());
                }
                bytes.extend_from_slice(&vertex.rgba.to_le_bytes());
                if bytes.len() == CHUNK_VERTICES * STRIDE {
                    output.push(self.buffer(std::mem::take(&mut bytes))?);
                    let remaining = total.saturating_sub(output.len() * CHUNK_VERTICES);
                    bytes
                        .try_reserve_exact(remaining.min(CHUNK_VERTICES) * STRIDE)
                        .map_err(|_| NC_E_OUT_OF_MEMORY)?;
                }
                Ok(())
            };
            match batch {
                Batch::View(_) => {}
                Batch::Atlas { first, count } => {
                    let range = *first as usize..(*first as usize).checked_add(*count as usize).ok_or(NC_E_OUT_OF_RANGE)?;
                    for vertex in vertices.get(range).ok_or(NC_E_INVALID_ARG)? {
                        append(*vertex)?;
                    }
                }
                Batch::Texture { first, .. } => {
                    let quad = vertices.get(*first as usize..*first as usize + 4).ok_or(NC_E_INVALID_ARG)?;
                    for index in [0, 1, 2, 0, 2, 3] {
                        append(quad[index])?;
                    }
                }
            }
        }
        if !bytes.is_empty() {
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
        self.resources.create_buffer(desc, Bytes::source(bytes).as_com_ref())
    }
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
