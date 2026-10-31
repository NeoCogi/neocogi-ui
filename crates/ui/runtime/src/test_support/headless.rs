//! Fixtures assemble real UI and GFX objects; only the final GPU consumer is replaced.

use neocogi_gfx_support::testing::UiBatch;
use neocogi_com::{com_object, ComPtr, ComRef, NcStatus};
use neocogi_render_core::abi::*;
use neocogi_ui::{IAssetBundleAbi, Ui, RenderVertex};
use std::{cell::RefCell, rc::Rc};

/// Owned stream observations remain readable after the compositor's invocation expires.
#[derive(Clone, Default)]
pub(crate) struct Frame {
    /// Number of complete renderer calls since fixture creation.
    pub(crate) calls: usize,
    /// Final screen-space vertices copied during the latest call.
    pub(crate) vertices: Vec<RenderVertex>,
    /// Actual batch order; no fixture tessellation or clipping is performed.
    pub(crate) batches: Vec<UiBatch>,
}

#[com_object(interfaces(IRenderer))]
/// A headless device accepts complete streams from the production GFX compositor.
struct CaptureRenderer {
    /// IRenderer has shared receivers, so callback observations require one interior-mutable slot.
    frame: Rc<RefCell<Frame>>,
}
impl neocogi_render_core::abi::IRenderDriverBound for CaptureRenderer {
    fn driver_identity(&self) -> Result<neocogi_com::ComPtr<neocogi_render_core::abi::IRenderDriverIdentityAbi>, neocogi_com::NcStatus> {
        Err(neocogi_com::NC_E_NOINTERFACE)
    }
}
impl IRenderer for CaptureRenderer {
    /// Reads complete arrays while the token is live, then publishes copied observations.
    fn render(&self, info: RenderInfo, stream: ComRef<'_, IRenderStreamAbi>) -> Result<(), NcStatus> {
        let (vertices, batches) = neocogi_gfx_support::testing::read(info, stream)?;
        // No observation borrow survives a foreign stream call. This also permits deliberately
        // reentrant renderer fixtures to be added without changing the production path.
        let mut frame = self.frame.borrow_mut();
        frame.calls += 1;
        frame.vertices = vertices;
        frame.batches = batches;
        Ok(())
    }
}

/// Test-owned public clients contain no retained tree, routing policy, or second frame coordinator.
pub(crate) struct Headless {
    /// All widget construction, input and surface operations use the public UI facade.
    pub(crate) ui: Ui,
    /// Actual renderer capability is also used when tests install another resource revision.
    pub(crate) renderer: ComPtr<IRendererAbi>,
    /// Copied observations are separate from the runtime and GPU resource lifetime.
    frame: Rc<RefCell<Frame>>,
}
impl Headless {
    /// Creates an independent runtime/compositor pair through the production entrypoint.
    pub(crate) fn new(source: ComRef<'_, IAssetBundleAbi>) -> Self {
        let runtime = super::runtime_from_assets(source).expect("fixture resources create a production runtime");
        let frame = Rc::new(RefCell::new(Frame::default()));
        let renderer = CaptureRenderer::from_value(CaptureRenderer { frame: frame.clone() }).query().unwrap();
        Self {
            ui: nc_ui_nodes::bind_builtin_nodes(runtime).unwrap(),
            renderer,
            frame,
        }
    }

    /// Constructs deterministic small resources before any behavior or allocation measurement.
    pub(crate) fn standard() -> Self {
        let source = crate::test_support::assets::export_bundle(&crate::SkinBundle::from_atlas(super::test_atlas())).unwrap();
        Self::new(source.as_com_ref())
    }

    /// Commits queued input and renders through the ordinary UI-to-GFX-to-renderer path.
    pub(crate) fn frame(&mut self, size: Size) -> Result<(), NcStatus> {
        self.ui.update(size)?;
        self.ui.render(size, self.renderer.as_com_ref())
    }

    /// Returns an owned assertion snapshot without exposing the compositor's expired buffers.
    pub(crate) fn snapshot(&self) -> Frame {
        self.frame.borrow().clone()
    }
}
