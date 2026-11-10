use neocogi_com::{ComRef, ComPtr, NcCopyFromBufferOffset};
#[cfg(feature = "glow")]
use neocogi_com::{com_object, ComBox, IUnknown, ComInterface, NcStatus, NC_E_INVALID_ARG};
use neocogi_platform_contracts::render::abi::*;
#[cfg(feature = "glow")]
use neocogi_platform_contracts::render::platform::*;
#[cfg(feature = "glow")]
use neocogi_gfx_support::abi::RenderVertex;
#[cfg(feature = "glow")]
use std::{cell::RefCell, rc::Rc};
use neocogi_platform::platform::testing;
#[cfg(feature = "glow")]
use neocogi_platform::platform::testing::NativeBackend;
#[cfg(feature = "glow")]
use neocogi_platform::gpu::source::{Bytes, PipelineDesc};
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

/// Creates an explicit view of the production window framebuffer for this fixture.
fn framebuffer(host: ComRef<'_, IRenderHostAbi>) -> ComPtr<IRenderSurfaceAbi> {
    host.try_add_ref()
        .unwrap()
        .query::<IRenderSurfaceFactoryAbi>()
        .unwrap()
        .create_surface(RenderSurfaceDesc::Framebuffer)
        .unwrap()
}
