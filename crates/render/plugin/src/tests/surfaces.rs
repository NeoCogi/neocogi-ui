//! Production GFX blocks exercise fresh immutable outputs, resizing and device lifetime.
use super::*;

/// Build an independent compositor through the same statically linked GFX service as applications.
fn compositor(host: ComRef<'_, IRenderHostAbi>) -> ComPtr<neocogi_gfx_support::abi::GfxCompositorAbi> {
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
    compositor.bind_render_host(host).unwrap();
    compositor
}

/// Surface failure/resize/teardown must preserve completed images and the unrelated window target.
#[test]
#[ignore = "requires SDL GL; run with SDL_VIDEODRIVER=offscreen"]
fn independent_surface_snapshots_keep_orientation_and_device_lifetime() {
    let _session = native_session();
    let native = PlatformObject::new(nc_shader_compiler::CompilerObject::from_value(nc_shader_compiler::CompilerObject).query().unwrap()).window("immutable generic outputs", SIZE).unwrap();
    let weak = Rc::downgrade(&native.device);
    let window = WindowObject::from_value(native);
    let host = window.query::<IPlatformWindowAbi>().unwrap().graphics_host().unwrap();
    let renderer: ComPtr<IRendererAbi> = window.query().unwrap();
    let images: ComPtr<ITextureFactoryAbi> = window.query().unwrap();
    let factory: ComPtr<IRenderSurfaceFactoryAbi> = window.query().unwrap();
    let extent = Size { width: 32, height: 32 };
    let surface = factory.create_surface(extent).unwrap();
    let preview: ComPtr<IRendererAbi> = surface.query().unwrap();
    let source = images
        .create_texture(
            Size { width: 2, height: 2 },
            NcCopyFromBufferOffset::new(0, &[255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 255, 255, 0, 255]),
        )
        .unwrap();
    let mut compositor = compositor(host.as_com_ref());
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
    assert_eq!(compositor.submit(preview.as_com_ref()), Err(RENDER_E_BACKEND));
    assert_eq!(identity(&surface.completed_image().unwrap()).unwrap(), identity(&new).unwrap());
    assert_eq!(surface.resize(Size { width: -1, height: 32 }), Err(NC_E_INVALID_ARG));
    surface.resize(Size { width: 0, height: 0 }).unwrap();
    compositor.begin(extent, 0xff000000).unwrap();
    assert_eq!(compositor.submit(preview.as_com_ref()), Err(NC_E_INVALID_ARG));
    surface.resize(Size { width: 16, height: 16 }).unwrap();
    compositor.begin(Size { width: 16, height: 16 }, 0xff123456).unwrap();
    compositor.submit(preview.as_com_ref()).unwrap();
    assert_eq!(old.size().unwrap(), extent);
    assert_eq!(new.size().unwrap(), extent);
    drop((preview, surface, factory));
    let full = Rect { x: 0, y: 0, width: 64, height: 64 };
    compositor.begin(SIZE, 0xff333333).unwrap();
    compositor.texture(full, bounds, TextureRegion::FULL, 0xffffffff, old.as_com_ref()).unwrap();
    compositor
        .texture(full, Rect { x: 32, y: 0, ..bounds }, TextureRegion::FULL, 0xffffffff, new.as_com_ref())
        .unwrap();
    compositor.submit(renderer.as_com_ref()).unwrap();
    let pixels = weak.upgrade().unwrap().access().unwrap().state.as_ref().unwrap().gpu.last_pixels().to_vec();
    let pixel = |x: usize, y: usize| &pixels[((63 - y) * 64 + x) * 4..((63 - y) * 64 + x) * 4 + 4];
    assert_eq!(pixel(4, 4), [255, 0, 0, 255]);
    assert_eq!(pixel(24, 4), [0, 255, 0, 255]);
    assert_eq!(pixel(4, 24), [0, 0, 255, 255]);
    assert_eq!(pixel(24, 24), [255, 255, 0, 255]);
    assert_eq!(pixel(40, 4), [255, 0, 255, 255]);
    assert_eq!(pixel(40, 40), [51, 51, 51, 255]);
    drop((compositor, source, renderer, images, host, window));
    assert!(weak.upgrade().is_some(), "retained images own their native device");
    drop((old, new));
    assert!(weak.upgrade().is_none(), "native caches must not retain their own facade");
}
