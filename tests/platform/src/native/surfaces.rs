//! Production GFX blocks exercise fresh immutable outputs, resizing and device lifetime.
use super::*;

/// Build an independent compositor through the same statically linked GFX service as applications.
fn compositor(host: ComRef<'_, IRenderHostAbi>, surface: ComRef<'_, IRenderSurfaceAbi>) -> ComPtr<neocogi_gfx_support::abi::GfxCompositorAbi> {
    use neocogi_gfx_support::abi::GfxPluginFactoryAbi;
    let runtime = crate::host_runtime();
    // SAFETY: the statically linked descriptor and implementation live for this process.
    unsafe {
        runtime.register_plugin_descriptor(nc_gfx::plugin_descriptor()).unwrap();
    }
    let gfx = runtime
        .acquire_plugin_root::<GfxPluginFactoryAbi>(neocogi_com::guid_from_qualified_name("neocogi.gfx.runtime"))
        .unwrap();
    let assets = neocogi_demo_assets::load_bundle(runtime).unwrap();
    let mut compositor = gfx.create_compositor(assets.as_com_ref()).unwrap();
    compositor.bind_render_host(host, surface).unwrap();
    compositor
}

/// Surface failure/resize/teardown must preserve completed images and the unrelated window target.
#[test]
#[ignore = "requires SDL GL; run with SDL_VIDEODRIVER=offscreen"]
fn independent_surface_snapshots_keep_orientation_and_device_lifetime() {
    let _session = native_session();
    let native = PlatformObject::new(
        nc_shader_compiler::CompilerObject::from_value(nc_shader_compiler::CompilerObject)
            .query()
            .unwrap(),
    )
    .window("immutable generic outputs", SIZE)
    .unwrap();
    let weak = native.probe.clone();
    let window = WindowObject::from_value(native);
    let host = window.query::<IPlatformWindowAbi>().unwrap().graphics_host().unwrap();
    let renderer: ComPtr<IRendererAbi> = window.query().unwrap();
    let images: ComPtr<ITextureFactoryAbi> = window.query().unwrap();
    let factory: ComPtr<IRenderSurfaceFactoryAbi> = window.query().unwrap();
    let extent = Size { width: 32, height: 32 };
    let surface = factory.create_surface(RenderSurfaceDesc { kind: RenderSurfaceKind::Image, size: extent }).unwrap();
    let preview: ComPtr<IRendererAbi> = surface.query().unwrap();
    let driver = renderer.driver_identity().unwrap();
    assert_eq!(identity(&driver).unwrap(), identity(&surface.driver_identity().unwrap()).unwrap());
    assert_eq!(identity(&driver).unwrap(), identity(&preview.driver_identity().unwrap()).unwrap());
    assert_eq!(identity(&driver).unwrap(), identity(&host.driver_identity().unwrap()).unwrap());
    let source = images
        .create_texture(
            Size { width: 2, height: 2 },
            neocogi_render_core::source::initial_bytes(
                (NcCopyFromBufferOffset::new(0, &[255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 255, 255, 0, 255]))
                    .as_slice()
                    .to_vec(),
            )
            .borrow()
            .unwrap(),
        )
        .unwrap();
    let mut compositor = compositor(host.as_com_ref(), surface.as_com_ref());
    let bounds = Rect { x: 0, y: 0, width: 32, height: 32 };
    compositor.begin(extent, 0xff000000).unwrap();
    compositor
        .texture(bounds, bounds, TextureRegion::FULL, 0xffffffff, source.as_com_ref())
        .unwrap();
    compositor.submit(preview.as_com_ref()).unwrap();
    let old = surface.completed_image().unwrap();
    compositor.begin(extent, 0xffff00ff).unwrap();
    compositor.submit(preview.as_com_ref()).unwrap();
    let new = surface.completed_image().unwrap();
    assert_ne!(identity(&old).unwrap(), identity(&new).unwrap());
    assert_eq!(old.size().unwrap(), extent);
    // The portable envelope admits this extent; the native device rejects it before publication.
    let oversized = Size { width: 131072, height: 1 };
    surface.resize(oversized).unwrap();
    compositor.begin(oversized, 0xff000000).unwrap();
    assert_eq!(compositor.submit(preview.as_com_ref()).map_err(|error| error.status), Err(RENDER_E_BACKEND));
    assert_eq!(identity(&surface.completed_image().unwrap()).unwrap(), identity(&new).unwrap());
    assert_eq!(surface.resize(Size { width: -1, height: 32 }), Err(NC_E_INVALID_ARG));
    surface.resize(Size { width: 0, height: 0 }).unwrap();
    compositor.begin(extent, 0xff000000).unwrap();
    assert_eq!(compositor.submit(preview.as_com_ref()).map_err(|error| error.status), Err(NC_E_INVALID_ARG));
    surface.resize(Size { width: 16, height: 16 }).unwrap();
    compositor.begin(Size { width: 16, height: 16 }, 0xff123456).unwrap();
    compositor.submit(preview.as_com_ref()).unwrap();
    assert_eq!(old.size().unwrap(), extent);
    assert_eq!(new.size().unwrap(), extent);
    drop((preview, surface, factory));
    compositor.bind_render_host(host.as_com_ref(), framebuffer(host.as_com_ref()).as_com_ref()).unwrap();
    let full = Rect { x: 0, y: 0, width: 64, height: 64 };
    compositor.begin(SIZE, 0xff333333).unwrap();
    compositor.texture(full, bounds, TextureRegion::FULL, 0xffffffff, old.as_com_ref()).unwrap();
    compositor
        .texture(full, Rect { x: 32, y: 0, ..bounds }, TextureRegion::FULL, 0xffffffff, new.as_com_ref())
        .unwrap();
    compositor.submit(renderer.as_com_ref()).unwrap();
    let pixels = weak.pixels().unwrap();
    let pixel = |x: usize, y: usize| &pixels[((63 - y) * 64 + x) * 4..((63 - y) * 64 + x) * 4 + 4];
    assert_eq!(pixel(4, 4), [255, 0, 0, 255]);
    assert_eq!(pixel(24, 4), [0, 255, 0, 255]);
    assert_eq!(pixel(4, 24), [0, 0, 255, 255]);
    assert_eq!(pixel(24, 24), [255, 255, 0, 255]);
    assert_eq!(pixel(40, 4), [255, 0, 255, 255]);
    assert_eq!(pixel(40, 40), [51, 51, 51, 255]);
    let physical = weak.clone();
    drop((compositor, source, renderer, images, host, window));
    assert!(!weak.is_alive(), "completed images cannot retain their driver facade");
    assert!(physical.native_alive(), "completed images retain only actual native lifetime");
    drop((old, new));
    assert!(!physical.native_alive(), "final image release tears down the physical owner");
}

#[com_object(interfaces(IRenderHost, IRenderClientFactory))]
/// Native fixture captures its actual UI factory and gives it a bounded logical byte quota.
struct QuotaHost {
    /// All native preparation/maintenance uses the production host, not a test renderer.
    native: ComPtr<IRenderHostAbi>,
    /// The real private factory is inspected only by this acceptance test.
    factory: Rc<RefCell<Option<ComPtr<IRenderResourcesAbi>>>>,
}
impl IRenderDriverBound for QuotaHost {
    fn driver_identity(&self) -> Result<ComPtr<IRenderDriverIdentityAbi>, NcStatus> {
        self.native.driver_identity()
    }
}
impl IRenderHost for QuotaHost {
    fn maintain_result(&self) -> Result<ComPtr<neocogi_render_core::abi::IRenderOutcomeAbi>, NcStatus> {
        self.native.maintain_result()
    }
    fn import_texture_result(&self, resources: ComRef<'_, neocogi_render_core::abi::IRenderResourcesAbi>, image: ComRef<'_, neocogi_render_core::abi::ITextureAbi>) -> Result<neocogi_com::ComBox<neocogi_render_core::abi::ITextureImportResultAbi>, NcStatus> {
        self.native.import_texture_result(resources, image)
    }
    fn prepare_texture_result(&self, resources: ComRef<'_, neocogi_render_core::abi::IRenderResourcesAbi>, texture: ComRef<'_, neocogi_render_core::abi::IGpuBufferAbi>) -> Result<ComPtr<neocogi_render_core::abi::IRenderOutcomeAbi>, NcStatus> {
        self.native.prepare_texture_result(resources, texture)
    }
    fn recording_broker(&self) -> Result<ComPtr<IRenderViewBrokerAbi>, NcStatus> {
        self.native.recording_broker()
    }

}

/// A queued native deletion must free quota before GFX attempts its next geometry allocation.
#[test]
#[ignore = "requires native GL; run with SDL_VIDEODRIVER=offscreen"]
fn gfx_maintenance_precedes_geometry_admission_at_exact_quota_pressure() {
    let _session = native_session();
    let native = PlatformObject::new(
        nc_shader_compiler::CompilerObject::from_value(nc_shader_compiler::CompilerObject)
            .query()
            .unwrap(),
    )
    .window("GFX pre-admission retirement", SIZE)
    .unwrap();
    let device = native.probe.clone();
    let window = WindowObject::from_value(native);
    let renderer: ComPtr<IRendererAbi> = window.query().unwrap();
    let captured = Rc::new(RefCell::new(None));
    let host: ComPtr<IRenderHostAbi> = QuotaHost::from_value(QuotaHost {
        native: window.query::<IPlatformWindowAbi>().unwrap().graphics_host().unwrap(),
        factory: captured.clone(),
    })
    .query()
    .unwrap();
    let mut compositor = compositor(host.as_com_ref(), framebuffer(window.graphics_host().unwrap().as_com_ref()).as_com_ref());
    let resources = captured.borrow().as_ref().unwrap().clone();
    let texture = resources
        .create_dynamic_buffer(
            GpuBufferDesc::Texture {
                size: Size { width: 1, height: 1 },
                format: GpuTextureFormat::Rgba8Unorm,
                mip_count: 1,
            },
        )
        .unwrap();
    host.prepare_texture(resources.as_com_ref(), texture.as_com_ref()).unwrap();
    device.finish().unwrap();
    host.maintain().unwrap();
    drop(texture);
    let before = device.logical().unwrap().live_bytes(resources.as_com_ref()).unwrap();
    // Six UI vertices round to 128 bytes in each of three persistent slots. Leave 380 while the native
    // texture's 4-byte retirement packet is queued; no CPU zero payload is allocated for padding.
    let padding = resources
        .create_dynamic_buffer(
            GpuBufferDesc::Vertex {
                byte_len: 8 * 1024 * 1024 - before - 380,
            },
        )
        .unwrap();
    assert_eq!(8 * 1024 * 1024 - device.logical().unwrap().live_bytes(resources.as_com_ref()).unwrap(), 380);
    let vertex = |x, y| RenderVertex { x, y, u: 0., v: 0., rgba: 0xffffffff };
    let vertices = [
        vertex(0., 0.),
        vertex(64., 0.),
        vertex(64., 64.),
        vertex(64., 64.),
        vertex(0., 64.),
        vertex(0., 0.),
    ];
    compositor.begin(SIZE, 0xff000000).unwrap();
    compositor
        .atlas_triangles(Rect { x: 0, y: 0, width: 64, height: 64 }, NcCopyFromBufferOffset::new(0, &vertices))
        .unwrap();
    // Moving maintenance after geometry would reject this call before renderer.render is reached.
    compositor.submit(renderer.as_com_ref()).unwrap();
    drop(padding);
}

/// Delegates group creation while retaining the fixture's upload/quota observations.
impl neocogi_render_core::abi::IRenderClientFactory for QuotaHost {
    fn create_client(&self, mut limits: neocogi_render_core::abi::RenderClientLimits, surface: ComRef<'_, neocogi_render_core::abi::IRenderSurfaceAbi>, permissions: neocogi_render_core::abi::RenderClientPermissions) -> Result<neocogi_com::ComBox<neocogi_render_core::abi::IRenderClientDomainAbi>, NcStatus> {
        limits.live_bytes = 8 * 1024 * 1024;
        let domain = self.native.query::<neocogi_render_core::abi::IRenderClientFactoryAbi>()?.create_client(limits, surface, permissions)?;
        *self.factory.borrow_mut() = Some(domain.resources()?);
        Ok(domain)

    }
}
