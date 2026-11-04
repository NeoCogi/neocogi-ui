//! Owned failures cross the UI and GFX plugin boundaries without shared diagnostic state.
use super::*;
use neocogi_com::ComRef;

impl UiRuntimeClient {
    /// Binds this UI to an explicit drawing surface, preserving the previous binding on failure.
    pub fn set_render_host(&mut self, host: ComRef<'_, IRenderHostAbi>, surface: ComRef<'_, IRenderSurfaceAbi>) -> Result<(), RenderError> {
        let report = self.set_render_host_result(host, surface)?;
        check_outcome(&report, "bind UI renderer")
    }

    /// Prepares and publishes a compatible resource revision, retaining an upload failure's text.
    pub fn install_assets(&mut self, assets: ComRef<'_, IAssetBundleAbi>) -> Result<(), RenderError> {
        let report = self.install_assets_result(assets)?;
        check_outcome(&report, "install UI assets")
    }

    /// Paints synchronized geometry and preserves the original submission status and explanation.
    pub fn render(&self, size: Size, renderer: ComRef<'_, IRendererAbi>) -> Result<(), RenderError> {
        let report = self.render_result(size, renderer)?;
        check_outcome(&report, "render UI")
    }
}
