//! Rust clients retain each graphics operation's status and owned failure text.
use super::*;
use neocogi_com::ComRef;
use neocogi_render_core::abi::{IRenderHostAbi, IRenderSurfaceAbi, IRendererAbi, RenderError, check_outcome};

impl GfxCompositorClient {
    /// Prepares a complete candidate for the selected surface before replacing the current binding.
    pub fn bind_render_host(&mut self, host: ComRef<'_, IRenderHostAbi>, surface: ComRef<'_, IRenderSurfaceAbi>) -> Result<(), RenderError> {
        let report = self.bind_render_host_result(host, surface)?;
        check_outcome(&report, "bind compositor")
    }

    /// Publishes a compatible revision only after its native atlas has prepared successfully.
    pub fn install_assets(&mut self, assets: ComRef<'_, IAssetBundleAbi>) -> Result<(), RenderError> {
        let report = self.install_assets_result(assets)?;
        check_outcome(&report, "install compositor assets")
    }

    /// Submits once, preserving this submission's failure after every native object is released.
    pub fn submit(&mut self, renderer: ComRef<'_, IRendererAbi>) -> Result<(), RenderError> {
        let report = self.submit_result(renderer)?;
        check_outcome(&report, "submit compositor")
    }
}
