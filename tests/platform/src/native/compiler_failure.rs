//! The renderer owns its compiler; malformed client shaders fail through resource creation.
use super::*;
/// Failed compilation returns owned diagnostics that survive the native window.
#[test]
#[ignore = "requires SDL GL; run with SDL_VIDEODRIVER=offscreen"]
fn shader_rejection_returns_owned_operation_error() {
    let _session = native_session();
    crate::host_runtime();
    let native = testing::window("shader rejection", Size { width: 8, height: 8 }).unwrap();
    let weak = native.probe.clone();
    let host = native.window.graphics_host().unwrap();
    let surface = framebuffer(host.as_com_ref());
    let factory = host.query::<IRenderClientFactoryAbi>().unwrap();
    let domain = factory
        .create_client(RenderClientLimits::default(), surface.as_com_ref(), RenderClientPermissions::default())
        .unwrap();
    let resources = domain.resources().unwrap();
    let code = neocogi_platform_contracts::render::source::Bytes::source(b"rejected_ui_shader_diagnostic".to_vec());
    let error = resources
        .create_shader(GpuShaderStage::Vertex, code.as_com_ref(), NcCopyFromBufferOffset::new(0, b"vs"))
        .unwrap_err();
    let detail = error.message;
    assert!(detail.contains("rejected_ui_shader_diagnostic"), "{detail}");
    drop((resources, domain, factory, surface, host, native));
    assert!(!weak.is_alive());
    assert!(detail.contains("rejected_ui_shader_diagnostic"));
}
