//! Headless integration assembly. This module is excluded from deployable GFX builds.
//!
//! The fixture shares one logical device on each test thread so independently constructed
//! runtimes and assertion renderers can exchange real sealed blocks. Domains and their quotas
//! remain independent. Ordinary images must be explicitly registered with trusted fixture sizes;
//! this is not a native image authenticator and must never accept untrusted application images.
use crate::abi::RenderVertex;
use neocogi_com::{com_object, ComBox, ComPtr, ComRef, NcStatus, NC_E_INVALID_ARG};
use neocogi_render_core::{abi::*};
use neocogi_render_gpu::{
    command::{Command, DrawRange},
    testing::{Backend, Core, Image},
};
use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
};
use std::rc::Rc;

/// Concrete virtual family is selected explicitly by headless tests.
type GpuDevice = neocogi_render_gpu::GpuDevice<Backend>;

/// Decoded UI observations used only by headless integration assertions.
#[derive(Clone, Debug)]
pub enum UiBatch {
    /// Complete triangles using the private UI atlas.
    Atlas {
        /// First decoded vertex.
        first: u32,
        /// Triangle-aligned decoded vertex count.
        count: u32,
    },
    /// Four corners recovered from one immutable-image draw.
    Texture {
        /// First decoded corner.
        first: u32,
    },
    /// Opaque client contribution retains its painter position.
    View,
}

thread_local! {
    /// Thread affinity matches COM objects and the production graphics recorder.
    static CORE: Rc<Core> = Core::new();
    /// Physical virtual core is shared only on this fixture thread.
    static DEVICE: GpuDevice = GpuDevice::new(CORE.with(Clone::clone),nc_shader_compiler::CompilerObject::from_value(nc_shader_compiler::CompilerObject).query().unwrap());
    /// Keys are explicitly replaced at each fixture-image construction; no COM owner is retained.
    static IMAGES: RefCell<HashMap<usize, Rc<Image>>> = RefCell::new(HashMap::new());
}

#[com_object(interfaces(IRenderHost))]
/// CPU-only assembly has no native window or device getter.
struct Host {
    /// Canonical recording and private UI-resource provenance.
    device: GpuDevice,
    /// Caller-owned failure injection has no reference to the runtime or widgets.
    failure: Rc<Cell<Option<NcStatus>>>,
}
impl IRenderDriverBound for Host {
    fn driver_identity(&self) -> Result<ComPtr<IRenderDriverIdentityAbi>, NcStatus> {
        Ok(self.device.driver_identity())
    }
}
impl IRenderHost for Host {
    /// CPU-only assembly has no native work or retirement queue.
    fn maintain(&self) -> Result<(), NcStatus> {
        Ok(())
    }
    /// Import requires prior explicit fixture registration and never calls the image's getters.
    fn import_ui_texture(&self, resources: ComRef<'_, IRenderResourcesAbi>, image: ComRef<'_, ITextureAbi>) -> Result<ComBox<IGpuTextureAbi>, NcStatus> {
        let id = IMAGES
            .with(|images| images.borrow().get(&(image.as_raw().object as usize)).cloned())
            .ok_or(NC_E_INVALID_ARG)?;
        self.device.import_ui_texture(resources, id)
    }
    /// Run real logical provenance checks; this assembly owns no native texture upload.
    fn prepare_ui_texture(&self, resources: ComRef<'_, IRenderResourcesAbi>, texture: ComRef<'_, IGpuTextureAbi>) -> Result<(), NcStatus> {
        self.device.ui_texture_snapshot(resources, texture)?;
        match self.failure.get() {
            Some(status) => Err(status),
            None => Ok(()),
        }
    }
    /// UI provenance is granted only through the trusted fixture assembly.
    fn create_ui_client(&self, limits: RenderClientLimits) -> Result<ComBox<IRenderClientDomainAbi>, NcStatus> {
        self.device.create_ui_client(limits)
    }
    /// Every consumer gets a separate frame coordinator on the same logical device.
    fn recording_broker(&self) -> Result<ComPtr<IRenderViewBrokerAbi>, NcStatus> {
        Ok(self.device.broker())
    }
}

/// Creates a trusted CPU host on the current test thread, with no native upload behavior.
pub fn host() -> ComPtr<IRenderHostAbi> {
    host_with_preparation_failure(Rc::new(Cell::new(None)))
}

/// Models a native preparation failure without bypassing provenance or capturing runtime owners.
pub fn host_with_preparation_failure(failure: Rc<Cell<Option<NcStatus>>>) -> ComPtr<IRenderHostAbi> {
    DEVICE.with(|device| {
        Host::from_value(Host { device: device.clone(), failure })
            .query()
            .expect("Host declares IRenderHost")
    })
}

/// Registers dimensions known by the fixture owner, preserving hostile getter/reentrancy tests.
pub fn register_image(image: ComRef<'_, ITextureAbi>, size: Size) {
    let key = image.as_raw().object as usize;
    let native = CORE.with(|core| Image::ordinary(core.clone(), size, key));
    IMAGES.with(|images| images.borrow_mut().insert(key, native));
}

/// Assertion observation belongs to the captured virtual image, not a recycled COM pointer lookup.
pub fn image_identity(image: &Image) -> Option<usize> {
    image.origin()
}

/// Authenticates a real sealed contribution on this thread's fixture assembly.
pub fn snapshot(block: ComRef<'_, IRenderViewBlockAbi>) -> Result<Rc<neocogi_render_gpu::record::Block<Backend>>, NcStatus> {
    DEVICE.with(|device| device.block_snapshot(block))
}

/// Inspect an actual private UI texture version while a fixture models native preparation.
pub fn texture(
    resources: ComRef<'_, IRenderResourcesAbi>,
    texture: ComRef<'_, IGpuTextureAbi>,
) -> Result<Rc<neocogi_render_gpu::resource::TextureVersion<Backend>>, NcStatus> {
    DEVICE.with(|device| device.ui_texture_snapshot(resources, texture))
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
pub fn blocks(info: RenderInfo, stream: ComRef<'_, IRenderStreamAbi>) -> Result<Vec<Rc<neocogi_render_gpu::record::Block<Backend>>>, NcStatus> {
    (0..info.block_count)
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
pub fn read(info: RenderInfo, stream: ComRef<'_, IRenderStreamAbi>) -> Result<(Vec<RenderVertex>, Vec<UiBatch>), NcStatus> {
    let mut vertices = Vec::new();
    let mut batches = Vec::new();
    for block in blocks(info, stream)? {
        if !block.is_ui() {
            batches.push(UiBatch::View);
            continue;
        }
        for command in block.commands() {
            let Command::Draw(draw) = command else {
                panic!("UI does not emit attachment clears")
            };
            let DrawRange::Vertices { first, count, instances: 1 } = draw.range else {
                panic!("UI uses direct triangle draws")
            };
            let binding = &draw.vertex_buffers[0];
            assert_eq!(binding.slot, 0);
            let first_byte = binding.offset as usize + first as usize * 20;
            let bytes = &binding.buffer.bytes().as_slice().ok_or(NC_E_INVALID_ARG)?[first_byte..first_byte + count as usize * 20];
            let decoded = bytes
                .as_chunks::<20>()
                .0
                .iter()
                .map(|v| RenderVertex {
                    x: f32::from_le_bytes(v[0..4].try_into().unwrap()),
                    y: f32::from_le_bytes(v[4..8].try_into().unwrap()),
                    u: f32::from_le_bytes(v[8..12].try_into().unwrap()),
                    v: f32::from_le_bytes(v[12..16].try_into().unwrap()),
                    rgba: u32::from_le_bytes(v[16..20].try_into().unwrap()),
                })
                .collect::<Vec<_>>();
            let first = vertices.len() as u32;
            if draw.textures[0].texture.imported_image().is_some() {
                assert_eq!(decoded.len(), 6);
                vertices.extend([decoded[0], decoded[1], decoded[2], decoded[5]]);
                batches.push(UiBatch::Texture { first });
            } else {
                vertices.extend(decoded);
                match batches.last_mut() {
                    Some(UiBatch::Atlas { count: previous, .. }) => *previous += count,
                    _ => batches.push(UiBatch::Atlas { first, count }),
                }
            }
        }
    }
    Ok((vertices, batches))
}
