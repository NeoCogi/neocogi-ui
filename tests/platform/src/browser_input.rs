use neocogi_com::NcCopyFromBufferOffset;
use web_sys::{EventTarget, HtmlTextAreaElement};
use neocogi_render_core::{abi::*, platform::*};
use neocogi_gfx_support::abi::GfxPluginFactoryAbi;
use neocogi_ui::{
    UI_E_UPDATE_REQUIRED,
    abi::{core::UiPluginFactoryAbi, nodes::NodesPluginFactoryAbi},
};
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::HtmlCanvasElement;
#[wasm_bindgen(inline_js = r#"
export function lose_test_context(context) {
    const extension = context.getExtension("WEBGL_lose_context");
    if (!extension) throw new Error("WebGL loss extension unavailable");
    extension.loseContext();
}
export function record_panic(text) {
    document.getElementById('result').dataset.panic = text;
}
export function exercise(target, name) {
    if (name === 'compose-start') {
        target.dispatchEvent(new CompositionEvent('compositionstart'));
        target.value = '日本';
        target.dispatchEvent(new InputEvent('input', {isComposing:true}));
        target.dispatchEvent(new KeyboardEvent('keydown', {key:'Enter', isComposing:true}));
    } else if (name === 'compose-end') {
        target.dispatchEvent(new CompositionEvent('compositionend', {data:'日本'}));
        target.dispatchEvent(new InputEvent('input', {data:'日本'}));
    } else if (name === 'paste') {
        target.value = 'é\n pasted';
        target.dispatchEvent(new InputEvent('input', {inputType:'insertFromPaste'}));
    } else if (name === 'tab') {
        target.dispatchEvent(new KeyboardEvent('keydown', {key:'Tab'}));
        target.dispatchEvent(new KeyboardEvent('keyup', {key:'Tab'}));
    } else if (name === 'key') {
        target.dispatchEvent(new KeyboardEvent('keydown', {key:'F'}));
    } else if (name === 'wheel') {
        target.dispatchEvent(new WheelEvent('wheel', {deltaY:0.5, deltaMode:0}));
        target.dispatchEvent(new WheelEvent('wheel', {deltaY:0.5, deltaMode:0}));
    } else if (name === 'down') {
        target.setPointerCapture = () => {};
        target.dispatchEvent(new PointerEvent('pointerdown', {pointerId:7, isPrimary:true, buttons:1, clientX:10, clientY:10}));
    } else if (name === 'chord') {
        target.dispatchEvent(new PointerEvent('pointermove', {pointerId:7, isPrimary:true, buttons:3}));
    } else if (name === 'up') {
        target.dispatchEvent(new PointerEvent('pointerup', {pointerId:7, isPrimary:true, buttons:0}));
        target.dispatchEvent(new PointerEvent('lostpointercapture', {pointerId:7, isPrimary:true}));
    } else if (name === 'cancel') {
        target.dispatchEvent(new PointerEvent('pointercancel', {pointerId:7, isPrimary:true}));
    }
}
"#)]
extern "C" {
    /// Keeps the Rust panic diagnostic in the dumped DOM even when wasm traps without unwinding.
    fn record_panic(text: &str);
    /// Dispatches one deterministic DOM sequence against the actual installed Rust listeners.
    fn lose_test_context(context: &web_sys::WebGl2RenderingContext);
    fn exercise(target: &EventTarget, name: &str);
}

/// Executes platform DOM checks and the real UI integration from this application-owned test artifact.
#[wasm_bindgen]
pub fn run_browser_input_tests() {
    nc_platform::testing::run_platform_input_tests();
    static_com_editing_and_webgl();
}
/// Registers all provider descriptors in one linked runtime, then exercises real DOM input,
/// inherited editing, committed IME placement and WebGL submission through their COM interfaces.
fn static_com_editing_and_webgl() {
    use neocogi_com::ComPtr;
    let runtime = crate::host_runtime();
    // SAFETY: All descriptions and referenced code/data live in this statically linked wasm.
    unsafe {
        runtime.register_plugin_descriptor(nc_ui::plugin_descriptor()).unwrap();
        runtime.register_plugin_descriptor(nc_ui_nodes::plugin_descriptor()).unwrap();
        runtime.register_plugin_descriptor(nc_gfx::plugin_descriptor()).unwrap();
        runtime.register_plugin_descriptor(nc_shader_compiler::plugin_descriptor()).unwrap();
        runtime.register_plugin_descriptor(nc_platform::plugin_descriptor()).unwrap();
    }
    let platform = runtime
        .acquire_plugin_root::<PlatformAbi>(neocogi_com::guid_from_qualified_name("neocogi.platform.webgl"))
        .unwrap();
    let assets = neocogi_demo_assets::load_bundle(crate::host_runtime()).unwrap();
    let native = platform
        .create_window(NcCopyFromBufferOffset::new(0, b"Browser COM regression"), Size { width: 640, height: 400 })
        .unwrap();
    let document = web_sys::window().unwrap().document().unwrap();
    let canvas = document.query_selector("canvas").unwrap().unwrap().dyn_into::<HtmlCanvasElement>().unwrap();
    canvas.style().set_css_text("width:640px;height:400px");
    let renderer: ComPtr<IRendererAbi> = native.query().unwrap();
    let factory = runtime
        .acquire_plugin_root::<UiPluginFactoryAbi>(neocogi_com::guid_from_qualified_name("neocogi.ui.runtime"))
        .unwrap();
    let gfx = runtime
        .acquire_plugin_root::<GfxPluginFactoryAbi>(neocogi_com::guid_from_qualified_name("neocogi.gfx.runtime"))
        .unwrap();
    let mut core = factory.create_runtime(assets.as_com_ref(), gfx.as_com_ref()).unwrap();
    let host = native.graphics_host().unwrap();
    let surface = host.query::<IRenderSurfaceFactoryAbi>().unwrap().create_surface(RenderSurfaceDesc {
        kind: RenderSurfaceKind::Framebuffer, size: Size::default(),
    }).unwrap();
    core.set_render_host(host.as_com_ref(), surface.as_com_ref()).unwrap();
    let nodes = runtime
        .acquire_plugin_root::<NodesPluginFactoryAbi>(neocogi_com::guid_from_qualified_name("neocogi.ui.nodes.runtime"))
        .unwrap();
    let widgets = nodes.bind(core.as_com_ref()).unwrap();
    let ui = neocogi_ui::Ui::from_interfaces(core, widgets).unwrap();
    let editor_owner = ui.widgets().textbox_unobserved("").unwrap();
    // The window takes exclusive ownership; later assertions use only a typed weak handle.
    let editor = editor_owner.handle();
    let window = ui
        .window("Editable COM window", Rect { x: 10, y: 10, width: 400, height: 180 }, editor_owner)
        .unwrap();
    let input = ui.input_sink().unwrap();
    exercise(canvas.unchecked_ref(), "tab");
    while native.poll(input.as_com_ref()).unwrap() == PollStatus::Event {}
    let size = native.size().unwrap();
    ui.update(size).unwrap();
    native.text_input(ui.text_input_area().unwrap()).unwrap();
    let text = document.query_selector("textarea").unwrap().unwrap().dyn_into::<HtmlTextAreaElement>().unwrap();
    assert_eq!(document.active_element(), Some(text.clone().into()));
    exercise(text.unchecked_ref(), "compose-start");
    assert_eq!(native.poll(input.as_com_ref()).unwrap(), PollStatus::Idle);
    exercise(text.unchecked_ref(), "compose-end");
    while native.poll(input.as_com_ref()).unwrap() == PollStatus::Event {}
    assert_eq!(ui.text_input_area().unwrap(), TextInputArea::None);
    ui.update(size).unwrap();
    assert_eq!(editor.text().unwrap(), "日本");
    ui.render(size, renderer.as_com_ref()).unwrap();
    // Changing CSS size requires the same explicit update-before-render used on SDL drawables.
    canvas.style().set_property("width", "500px").unwrap();
    let resized = native.size().unwrap();
    assert_ne!(resized, size);
    assert_eq!(ui.render(resized, renderer.as_com_ref()).map_err(|error| error.status), Err(UI_E_UPDATE_REQUIRED));
    ui.update(resized).unwrap();
    ui.render(resized, renderer.as_com_ref()).unwrap();
    window.close().unwrap();
    assert!(!editor.as_interface().is_alive(), "closing the window releases its exclusively owned editor");
    ui.close().unwrap();
    drop(renderer);
    drop(native);
    assert!(canvas.parent_node().is_none());
    assert!(text.parent_node().is_none());
}
