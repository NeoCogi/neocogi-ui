use neocogi_com::{com_object, ComPtr, NcCopyFromBufferOffset, NcStatus, INcStringComPtrExt};
#[cfg(feature = "glow")]
use neocogi_com::{ComRef, ComBox, IUnknown, ComInterface, NC_E_INVALID_ARG};
use neocogi_render_core::{abi::*, platform::*};
#[cfg(feature = "glow")]
use neocogi_gfx_support::abi::RenderVertex;
#[cfg(feature = "glow")]
use std::{cell::RefCell, rc::Rc};
use nc_render::testing::NativeWindow;
#[cfg(feature = "glow")]
use nc_render::testing::NativeBackend;
#[cfg(feature = "glow")]
use neocogi_render_gpu::source::{Bytes, PipelineDesc};
mod compiler_failure;
#[cfg(feature = "glow")]
mod costs;
#[cfg(feature = "glow")]
mod generic_gpu;
#[cfg(feature = "glow")]
mod surfaces;
#[cfg(feature = "glow")]
mod view_geometry;
/// Serializes SDL ownership for this acceptance executable.
static NATIVE_SESSION: std::sync::Mutex<()> = std::sync::Mutex::new(());
/// Keeps event and native video ownership exclusive until all fixture values drop.
fn native_session() -> std::sync::MutexGuard<'static, ()> {
    NATIVE_SESSION.lock().expect("earlier native fixture")
}
#[cfg(feature = "glow")]
const SIZE: Size = Size { width: 64, height: 64 };
/// Owned compiler selection reused by windows within one test.
struct PlatformObject {
    /// Strong compiler capability remains available during native construction.
    compiler: ComPtr<neocogi_shader_core::abi::ShaderCompilerAbi>,
}
impl PlatformObject {
    /// Selects the test compiler without creating a native device yet.
    fn new(compiler: ComPtr<neocogi_shader_core::abi::ShaderCompilerAbi>) -> Self {
        Self { compiler }
    }
    /// Uses the public opt-in platform fixture, including its weak observations.
    fn window(&self, title: &str, size: Size) -> Result<NativeWindow, NcStatus> {
        nc_render::testing::window(self.compiler.clone(), title, size)
    }
}
/// Transfers the factory-issued production window into its public COM capability.
struct WindowObject;
impl WindowObject {
    /// Transfers the strong window capability without retaining its probe.
    fn from_value(native: NativeWindow) -> ComPtr<IPlatformWindowAbi> {
        native.window
    }
}
#[cfg(feature = "glow")]
fn identity<I: ComInterface>(value: &ComPtr<I>) -> Result<usize, NcStatus> {
    Ok(value.query::<IUnknown>()?.as_raw().object as usize)
}
/// Captures a generic procedural triangle; all client shader semantics live in this fixture.
#[cfg(feature = "glow")]
fn graphics(resources: &ComPtr<IRenderResourcesAbi>, broker: &ComPtr<IRenderViewBrokerAbi>, root: RenderViewRoot) -> ComBox<IRenderViewBlockAbi> {
    let code = Bytes::source(
        br#"
        @vertex fn vs(@builtin(vertex_index) i:u32) -> @builtin(position) vec4<f32> {
            let p = array<vec2<f32>,3>(vec2(-0.9,-0.9),vec2(0.9,-0.9),vec2(0.0,0.9));
            return vec4(p[i],0.5,1.0);
        }
        @fragment fn fs() -> @location(0) vec4<f32> { return vec4(0.0,0.0,1.0,1.0); }
    "#
        .to_vec(),
    );
    let vs = resources
        .create_shader(GpuShaderStage::Vertex, code.as_com_ref(), NcCopyFromBufferOffset::new(0, b"vs"))
        .unwrap();
    let fs = resources
        .create_shader(GpuShaderStage::Fragment, code.as_com_ref(), NcCopyFromBufferOffset::new(0, b"fs"))
        .unwrap();
    let descriptor = PipelineDesc::default().source().unwrap();
    let pipeline = resources.create_pipeline(descriptor.as_com_ref(), vs.as_com_ref(), fs.as_com_ref()).unwrap();
    let mut frame = broker.begin_frame().unwrap();
    let mut session = frame.begin_view(root, resources.as_com_ref()).unwrap();
    let mut view = session.view().unwrap();
    view.bind_pipeline(pipeline.as_com_ref()).unwrap();
    view.draw(3, 1, 0).unwrap();
    view.finish().unwrap();
    let block = session.take_block().unwrap();
    frame.finish().unwrap();
    block
}
/// Creates a clockwise full-image quad in the final UI vertex format.
#[cfg(feature = "glow")]
fn quad(rect: Rect) -> [RenderVertex; 4] {
    let (x, y, right, bottom) = (rect.x as f32, rect.y as f32, (rect.x + rect.width) as f32, (rect.y + rect.height) as f32);
    [
        RenderVertex { x, y, u: 0., v: 0., rgba: 0xffffffff },
        RenderVertex {
            x: right,
            y,
            u: 1.,
            v: 0.,
            rgba: 0xffffffff,
        },
        RenderVertex {
            x: right,
            y: bottom,
            u: 1.,
            v: 1.,
            rgba: 0xffffffff,
        },
        RenderVertex {
            x,
            y: bottom,
            u: 0.,
            v: 1.,
            rgba: 0xffffffff,
        },
    ]
}
