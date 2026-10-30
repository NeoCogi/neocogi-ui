//! Headless integration assembly. This module is excluded from deployable GFX builds.
//!
//! The fixture shares one logical device on each test thread so independently constructed
//! runtimes and assertion renderers can exchange real sealed blocks. Domains and their quotas
//! remain independent. Ordinary images must be explicitly registered with trusted fixture sizes;
//! this is not a native image authenticator and must never accept untrusted application images.
use neocogi_com::{com_object, ComBox, ComPtr, ComRef, NcStatus, NC_E_INVALID_ARG};
use neocogi_render_core::{abi::*, TextureId};
use neocogi_render_gpu::{
    command::{Command, DrawRange},
    GpuDevice,
};
use std::{cell::RefCell, collections::HashMap};
use std::rc::Rc;

thread_local! {
    /// Thread affinity matches COM objects and the production graphics recorder.
    static DEVICE: GpuDevice = GpuDevice::new();
    /// Keys are explicitly replaced at each fixture-image construction; no COM owner is retained.
    static IMAGES: RefCell<HashMap<usize, TextureId>> = RefCell::new(HashMap::new());
}

#[com_object(interfaces(IRenderHost))]
/// CPU-only assembly has no native window or device getter.
struct Host {
    /// Canonical recording and private UI-resource provenance.
    device: GpuDevice,
}
impl IRenderHost for Host {
    /// Import requires prior explicit fixture registration and never calls the image's getters.
    fn import_ui_texture(&self, resources: ComRef<'_, IRenderResourcesAbi>, image: ComRef<'_, ITextureAbi>) -> Result<ComBox<IGpuTextureAbi>, NcStatus> {
        let id = IMAGES
            .with(|images| images.borrow().get(&(image.as_raw().object as usize)).copied())
            .ok_or(NC_E_INVALID_ARG)?;
        self.device.import_ui_texture(resources, id, image.try_add_ref()?)
    }
    /// Run real logical provenance checks; this assembly owns no native texture upload.
    fn prepare_ui_texture(&self, resources: ComRef<'_, IRenderResourcesAbi>, texture: ComRef<'_, IGpuTextureAbi>) -> Result<(), NcStatus> {
        self.device.ui_texture_snapshot(resources, texture)?;
        Ok(())
    }
    /// UI provenance is granted only through the trusted fixture assembly.
    fn create_ui_client(&self, limits: RenderClientLimits) -> Result<ComBox<IRenderClientDomainAbi>, NcStatus> {
        self.device.create_ui_client(limits, None)
    }
    /// Every consumer gets a separate frame coordinator on the same logical device.
    fn recording_broker(&self) -> Result<ComPtr<IRenderViewBrokerAbi>, NcStatus> {
        Ok(self.device.broker())
    }
}

/// Creates a trusted CPU host on the current test thread, with no native upload behavior.
pub fn host() -> ComPtr<IRenderHostAbi> {
    DEVICE.with(|device| Host::from_value(Host { device: device.clone() }).query().expect("Host declares IRenderHost"))
}

/// Registers dimensions known by the fixture owner, preserving hostile getter/reentrancy tests.
pub fn register_image(image: ComRef<'_, ITextureAbi>, size: Size) {
    let id = TextureId::for_render_target(size.width, size.height).expect("positive fixture image size");
    IMAGES.with(|images| images.borrow_mut().insert(image.as_raw().object as usize, id));
}

/// Assertion-only reverse lookup of a registered fixture image; this does not retain its owner.
pub fn image_identity(id: TextureId) -> Option<usize> {
    IMAGES.with(|images| images.borrow().iter().find_map(|(key, value)| (*value == id).then_some(*key)))
}

/// Authenticates a real sealed contribution on this thread's fixture assembly.
pub fn snapshot(block: ComRef<'_, IRenderViewBlockAbi>) -> Result<Rc<neocogi_render_gpu::record::Block>, NcStatus> {
    DEVICE.with(|device| device.block_snapshot(block))
}

/// Creates a real client controller; tests keep it alive while submitting its blocks.
pub fn client() -> ComBox<IRenderClientDomainAbi> {
    DEVICE.with(|device| device.create_client(RenderClientLimits::default()).expect("valid client fixture limits"))
}

/// Seals one real client contribution. The returned controller must outlive submission.
pub fn client_block(root: RenderViewRoot) -> (ComBox<IRenderClientDomainAbi>, ComBox<IRenderViewBlockAbi>) {
    let domain = client();
    let resources = domain.resources().unwrap();
    let broker = host().recording_broker().unwrap();
    let mut frame = broker.begin_frame().unwrap();
    let mut session = frame.begin_view(root, resources.as_com_ref()).unwrap();
    session.view().unwrap().finish().unwrap();
    let block = session.take_block().unwrap();
    frame.finish().unwrap();
    (domain, block)
}

/// Authenticates actual generic blocks for CPU benchmarks and assertions, without decoding UI bytes.
pub fn blocks(info: RenderInfo, stream: ComRef<'_, IRenderStreamAbi>) -> Result<Vec<Rc<neocogi_render_gpu::record::Block>>, NcStatus> {
    (0..info.batch_count)
        .map(|index| {
            let observer = stream.view(info.invocation, index)?;
            DEVICE.with(|device| device.block_snapshot(observer.as_com_ref()))
        })
        .collect()
}

/// Reads actual GFX-produced GPU bytes. Returned UI observations do not retain resource owners.
///
/// Atlas draws remain triangles; sampled immutable-image draws are observed as four-corner quads.
/// Client blocks remain opaque. This assertion adapter neither clips nor tessellates geometry.
pub fn read(info: RenderInfo, stream: ComRef<'_, IRenderStreamAbi>) -> Result<(Vec<RenderVertex>, Vec<RenderBatch>), NcStatus> {
    assert_eq!(info.vertex_count, 0, "native streams carry generic blocks only");
    let mut vertices = Vec::new();
    let mut batches = Vec::new();
    for block in blocks(info, stream)? {
        if !block.is_ui() {
            batches.push(RenderBatch::View);
            continue;
        }
        for command in &block.commands {
            let Command::Draw(draw) = command else {
                panic!("UI does not emit attachment clears")
            };
            let DrawRange::Vertices { first, count, instances: 1 } = draw.range else {
                panic!("UI uses direct triangle draws")
            };
            let binding = &draw.vertex_buffers[0];
            assert_eq!(binding.slot, 0);
            let first_byte = binding.offset as usize + first as usize * 20;
            let bytes = &binding.buffer.bytes[first_byte..first_byte + count as usize * 20];
            let decoded = bytes
                .chunks_exact(20)
                .map(|v| RenderVertex {
                    x: f32::from_le_bytes(v[0..4].try_into().unwrap()),
                    y: f32::from_le_bytes(v[4..8].try_into().unwrap()),
                    u: f32::from_le_bytes(v[8..12].try_into().unwrap()),
                    v: f32::from_le_bytes(v[12..16].try_into().unwrap()),
                    rgba: u32::from_le_bytes(v[16..20].try_into().unwrap()),
                })
                .collect::<Vec<_>>();
            let first = vertices.len() as u32;
            if draw.textures[0].texture.imported_id().is_some() {
                assert_eq!(decoded.len(), 6);
                vertices.extend([decoded[0], decoded[1], decoded[2], decoded[5]]);
                batches.push(RenderBatch::Texture { first });
            } else {
                vertices.extend(decoded);
                match batches.last_mut() {
                    Some(RenderBatch::Atlas { count: previous, .. }) => *previous += count,
                    _ => batches.push(RenderBatch::Atlas { first, count }),
                }
            }
        }
    }
    Ok((vertices, batches))
}
