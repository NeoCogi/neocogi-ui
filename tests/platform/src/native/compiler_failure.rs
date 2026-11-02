use super::*;
#[com_object(interfaces(neocogi_shader_core::abi::ShaderCompiler))]
struct RejectedCompiler;
impl neocogi_shader_core::abi::ShaderCompiler for RejectedCompiler {
    fn compile(
        &self,
        request: neocogi_shader_core::abi::ShaderCompileRequest,
        _: NcCopyFromBufferOffset<'_, u8>,
        entry: NcCopyFromBufferOffset<'_, u8>,
    ) -> Result<ComPtr<neocogi_shader_core::abi::IShaderCompilationAbi>, NcStatus> {
        let root: ComPtr<neocogi_shader_core::abi::ShaderCompilerAbi> =
            nc_shader_compiler::CompilerObject::from_value(nc_shader_compiler::CompilerObject).query()?;
        root.compile(request, NcCopyFromBufferOffset::new(0, b"rejected_ui_shader_diagnostic"), entry)
    }
}
/// Exercise the actual UI->GFX->resource->compiler first-bind chain; capture detail
/// before dropping the native window, then prove the owned text remains usable.
#[test]
#[ignore = "requires SDL GL; run with SDL_VIDEODRIVER=offscreen"]
fn first_ui_bind_shader_rejection_preserves_device_diagnostic() {
    use neocogi_gfx_support::abi::GfxPluginFactoryAbi;
    use neocogi_ui::abi::core::UiPluginFactoryAbi;
    let _session = native_session();
    let runtime = crate::host_runtime();
    // SAFETY: linked immutable descriptors and code live for the process.
    unsafe {
        runtime.register_plugin_descriptor(nc_gfx::plugin_descriptor()).unwrap();
        runtime.register_plugin_descriptor(nc_ui::plugin_descriptor()).unwrap();
    }
    let gfx = runtime
        .acquire_plugin_root::<GfxPluginFactoryAbi>(neocogi_com::guid_from_qualified_name("neocogi.gfx.runtime"))
        .unwrap();
    let factory = runtime
        .acquire_plugin_root::<UiPluginFactoryAbi>(neocogi_com::guid_from_qualified_name("neocogi.ui.runtime"))
        .unwrap();
    let assets = neocogi_demo_assets::load_bundle(runtime).unwrap();
    let mut ui = factory.create_runtime(assets.as_com_ref(), gfx.as_com_ref()).unwrap();
    let platform = PlatformObject::new(RejectedCompiler::from_value(RejectedCompiler).query().unwrap());
    let native = platform.window("first bind rejection", Size { width: 8, height: 8 }).unwrap();
    let weak = native.probe.clone();
    let window: ComPtr<IPlatformWindowAbi> = WindowObject::from_value(native).query().unwrap();
    let renderer: ComPtr<IRendererAbi> = window.query().unwrap();
    let host = window.graphics_host().unwrap();
    assert!(ui.set_render_host(host.as_com_ref()).is_err());
    let detail = renderer
        .query::<IRenderDiagnosticsAbi>()
        .unwrap()
        .diagnostic()
        .unwrap()
        .to_string_bounded(4096)
        .unwrap();
    assert!(detail.contains("rejected_ui_shader_diagnostic"), "{detail}");
    drop((ui, host, renderer, window, platform));
    assert!(!weak.is_alive());
    assert!(detail.contains("rejected_ui_shader_diagnostic"));
}
