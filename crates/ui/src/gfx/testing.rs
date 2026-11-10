//! Headless integration assembly. This module is excluded from deployable GFX builds.
//!
//! The fixture shares one logical device on each test thread so independently constructed
//! runtimes and assertion renderers can exchange real sealed blocks. Domains and their quotas
//! remain independent. Ordinary images must be explicitly registered with trusted fixture sizes;
//! this is not a native image authenticator and must never accept untrusted application images.
use crate::gfx::abi::RenderVertex;
use neocogi_com::{com_object, ComBox, ComPtr, ComRef, NcStatus, NC_E_INVALID_ARG};
use neocogi_platform_contracts::render::{abi::*};
use neocogi_platform::gpu::{
    command::{Command, DrawRange},
    testing::{Backend, Core, Image},
};
use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
};
use std::rc::Rc;

/// Concrete virtual family is selected explicitly by headless tests.
type GpuDevice = neocogi_platform::gpu::GpuDevice<Backend>;

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
    static DEVICE: GpuDevice = GpuDevice::new(CORE.with(Clone::clone)).unwrap();
    /// Target metadata is shared by all fixture groups without granting UI-specific authority.
    static TARGET: Rc<neocogi_platform::gpu::target::SurfaceTarget<Backend>> = DEVICE.with(|device| device.surface_target(RenderSurfaceKind::Framebuffer).unwrap());
    /// Concrete identity authenticates factory requests without calling supplied surface getters.
    static SURFACE: ComPtr<IRenderSurfaceAbi> = Surface::from_value(Surface).query().unwrap();
    /// Keys are explicitly replaced at each fixture-image construction; no COM owner is retained.
    static IMAGES: RefCell<HashMap<usize, Rc<Image>>> = RefCell::new(HashMap::new());
}

#[com_object(interfaces(IRenderHost, IRenderClientFactory, IRenderSurfaceFactory))]
/// CPU-only assembly has no native window or device getter.
struct Host {
    /// Canonical logical device authenticates resource and block ownership.
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
    fn maintain_result(&self) -> Result<ComPtr<IRenderOutcomeAbi>, NcStatus> {
        Ok(operation_result(Ok(())))
    }
    /// Import requires explicit fixture registration and never calls image getters.
    fn import_texture_result(
        &self,
        resources: ComRef<'_, IRenderResourcesAbi>,
        image: ComRef<'_, ITextureAbi>,
    ) -> Result<ComBox<ITextureImportResultAbi>, NcStatus> {
        let result = (|| {
            let id = IMAGES
                .with(|images| images.borrow().get(&(image.as_raw().object as usize)).cloned())
                .ok_or(NC_E_INVALID_ARG)?;
            self.device.import_texture(resources, id)
        })();
        Ok(texture_import_result(result.map_err(RenderError::from)))
    }
    /// Validate logical ownership before injecting this preparation's failure.
    fn prepare_texture_result(
        &self,
        resources: ComRef<'_, IRenderResourcesAbi>,
        texture: ComRef<'_, IGpuBufferAbi>,
    ) -> Result<ComPtr<IRenderOutcomeAbi>, NcStatus> {
        let result = self
            .device
            .imported_texture_snapshot(resources, texture)
            .and_then(|_| self.failure.get().map_or(Ok(()), Err));
        Ok(operation_result(result.map_err(RenderError::from)))
    }
    /// Each consumer gets a separate frame coordinator on the same logical device.
    fn recording_broker(&self) -> Result<ComPtr<IRenderViewBrokerAbi>, NcStatus> {
        Ok(self.device.broker())
    }
}
impl IRenderClientFactory for Host {
    /// Only the fixture's exact framebuffer capability can select its shared target.
    fn create_client(
        &self,
        limits: RenderClientLimits,
        surface: ComRef<'_, IRenderSurfaceAbi>,
        permissions: RenderClientPermissions,
    ) -> Result<ComBox<IRenderClientDomainAbi>, NcStatus> {
        if surface.as_raw().object != framebuffer().as_raw().object {
            return Err(RENDER_E_OWNER);
        }
        TARGET.with(|target| self.device.create_client(limits, target.clone(), permissions))
    }
}
impl IRenderSurfaceFactory for Host {
    /// Headless fixtures support only the shared framebuffer; native suites qualify image targets.
    fn create_surface(&self, desc: RenderSurfaceDesc) -> Result<ComPtr<IRenderSurfaceAbi>, NcStatus> {
        if !matches!(desc, RenderSurfaceDesc::Framebuffer) {
            return Err(NC_E_INVALID_ARG);
        }
        Ok(framebuffer())
    }
}

#[com_object(interfaces(IRenderSurface))]
/// One authenticated framebuffer identity per fixture thread, without a native attachment.
struct Surface;
impl IRenderDriverBound for Surface {
    /// Shares the real virtual device identity used by fixture resources.
    fn driver_identity(&self) -> Result<ComPtr<IRenderDriverIdentityAbi>, NcStatus> {
        Ok(DEVICE.with(|device| device.driver_identity()))
    }
}
impl IRenderSurface for Surface {
    /// Framebuffer recording is submitted by the fixture's capture renderer.
    fn kind(&self) -> Result<RenderSurfaceKind, NcStatus> {
        Ok(RenderSurfaceKind::Framebuffer)
    }
    /// The headless fixture has no drawable; tests supply explicit positive frame sizes.
    fn size(&self) -> Result<Size, NcStatus> {
        Ok(Size::default())
    }
    /// Framebuffer size is controlled by its enclosing host.
    fn resize(&self, _: Size) -> Result<(), NcStatus> {
        Err(NC_E_INVALID_ARG)
    }
    /// A framebuffer does not publish independent immutable images.
    fn completed_image(&self) -> Result<ComPtr<ITextureAbi>, NcStatus> {
        Err(neocogi_com::NC_E_NOT_FOUND)
    }
}

/// Retains the concrete framebuffer identity accepted by this thread's host factories.
pub fn framebuffer() -> ComPtr<IRenderSurfaceAbi> {
    SURFACE.with(Clone::clone)
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
pub fn snapshot(block: ComRef<'_, IRenderViewBlockAbi>) -> Result<Rc<neocogi_platform::gpu::record::Block<Backend>>, NcStatus> {
    DEVICE.with(|device| device.block_snapshot(block))
}

/// Inspect an actual private UI texture version while a fixture models native preparation.
pub fn texture(
    resources: ComRef<'_, IRenderResourcesAbi>,
    texture: ComRef<'_, IGpuBufferAbi>,
) -> Result<Rc<neocogi_platform::gpu::resource::TextureVersion<Backend>>, NcStatus> {
    DEVICE.with(|device| device.imported_texture_snapshot(resources, texture))
}

/// Creates a real client controller; tests keep it alive while submitting its blocks.
pub fn client() -> ComBox<IRenderClientDomainAbi> {
    DEVICE.with(|device| {
        device
            .create_client(RenderClientLimits::default(), TARGET.with(Clone::clone), RenderClientPermissions::default())
            .expect("valid client fixture limits")
    })
}

/// Observes charged pending bytes and live client shells on the fixture's actual recording device.
pub fn recording_usage(resources: ComRef<'_, IRenderResourcesAbi>) -> Result<(u64, u32), NcStatus> {
    DEVICE.with(|device| device.recording_usage(resources))
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
pub fn blocks(info: RenderInfo, stream: ComRef<'_, IRenderStreamAbi>) -> Result<Vec<Rc<neocogi_platform::gpu::record::Block<Backend>>>, NcStatus> {
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
        // Observation recognizes GFX's concrete vertex layout, never a resource-group role.
        if !block
            .commands()
            .iter()
            .any(|command| matches!(command, Command::Draw(draw) if draw.pipeline.vertex_buffers().iter().any(|layout| layout.stride == 20)))
        {
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
            let bytes = &binding.buffer.bytes().as_slice()[first_byte..first_byte + count as usize * 20];
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
