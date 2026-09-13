//! Retained-state owner shared by production and tests, paired with a GFX compositor.
//!
//! Painting records semantic commands; submission crosses the independently owned GFX boundary.
//! The runtime checks foreign widget failures between these phases before submitting any frame.

use crate::{Dimensioni, Recti, ResourceCatalog, SkinBundle, window_manager::WindowManager};
use neocogi_com::{ComPtr, ComRef, NcStatus};
use neocogi_gfx_contracts::GfxCompositorAbi;
use neocogi_renderer_contracts::{IRendererAbi, Size};

/// Production UI state with no renderer executor or render-stream implementation.
pub(crate) struct RuntimeContext {
    /// Immutable application identities against which derived revisions are authenticated.
    resource_catalog: ResourceCatalog,
    /// GFX-owned coarse-operation executor retained for this runtime's complete lifetime.
    compositor: ComPtr<GfxCompositorAbi>,
    /// Authoritative retained windows, nodes, input, layout, and semantic display list.
    pub(crate) window_manager: WindowManager,
    /// Conversion scratch reused while sending solid geometry across the typed boundary.
    solid_scratch: Vec<neocogi_gfx_contracts::SolidVertex>,
}

impl RuntimeContext {
    /// Creates retained UI state from the exact resource revision installed in the compositor.
    pub(crate) fn new(compositor: ComPtr<GfxCompositorAbi>, bundle: SkinBundle) -> Self {
        Self {
            resource_catalog: ResourceCatalog::new(bundle.atlas().clone()),
            compositor,
            window_manager: WindowManager::new(bundle),
            solid_scratch: Vec::new(),
        }
    }

    /// Returns the focused editor's committed drawable rectangle, when one exists.
    pub(crate) fn text_input_area(&self) -> Option<Recti> {
        self.window_manager.text_input_area()
    }

    /// Validates and publishes UI-local resource identities after GFX accepted the same revision.
    pub(crate) fn set_skin_bundle(&mut self, bundle: &SkinBundle) -> Result<(), crate::SkinInstallError> {
        self.resource_catalog.validate_bundle(bundle)?;
        self.window_manager.set_skin_bundle(bundle.clone());
        Ok(())
    }

    /// Records the last committed UI state without acquiring a compositor or renderer frame.
    pub(crate) fn paint(&mut self, size: Size) -> Result<(), NcStatus> {
        let dimensions = Dimensioni::new(size.width, size.height);
        if !self.window_manager.can_render(dimensions) {
            return Err(neocogi_ui_core_contracts::UI_E_UPDATE_REQUIRED);
        }
        self.window_manager.paint(dimensions);
        Ok(())
    }

    /// Releases commands and their resource references after a rejected paint phase.
    pub(crate) fn discard_paint(&mut self) {
        self.window_manager.display_list_mut().clear();
    }

    /// Submits a successfully checked paint phase through the real GFX compositor.
    pub(crate) fn submit(&mut self, size: Size, clear: u32, renderer: ComRef<'_, IRendererAbi>) -> Result<(), NcStatus> {
        // Clone the cheap immutable atlas handle before mutably borrowing the display list.
        let atlas = self.window_manager.skin_bundle().atlas().clone();
        crate::render::compositor::submit(
            self.window_manager.display_list_mut(),
            &atlas,
            &mut self.compositor,
            renderer,
            size,
            clear,
            &mut self.solid_scratch,
        )
    }

    /// Installs the copied resource revision in the GFX owner before UI publishes its projection.
    pub(crate) fn install_compositor_assets(&mut self, source: ComRef<'_, neocogi_gfx_contracts::IAssetBundleAbi>) -> Result<(), NcStatus> {
        self.compositor.install_assets(source)
    }
}
