//! Retained-state owner shared by production and tests, paired with a GFX compositor.
//!
//! Painting records semantic commands; submission crosses the independently owned GFX boundary.
//! The runtime checks foreign widget failures between these phases before submitting any frame.

use crate::{
    Dimensioni, Recti, ResourceCatalog, SkinBundle,
    choice_menu::{ChoiceMenuManager, IChoiceMenuStateAbi},
    window_manager::{PopupHandle, WindowManager},
};
use neocogi_com::{ComPtr, ComRef, NcStatus};
use neocogi_gfx_contracts::GfxCompositorAbi;
use neocogi_renderer_contracts::{IRendererAbi, Size};

/// Production UI state with no renderer executor or render-stream implementation.
/// Shutdown drops this owner directly after idle preflight. Field destruction releases
/// the compositor, complete surface forest, queued raw input and private menu controllers
/// without executing ordinary window-removal operations or posting lifecycle facts.
pub(crate) struct RuntimeContext {
    /// Immutable application identities against which derived revisions are authenticated.
    resource_catalog: ResourceCatalog,
    /// Private GFX executor created for this runtime; never exposed or shared with another UI.
    /// Only this context changes its resources and frame state through exclusive calls.
    compositor: ComPtr<GfxCompositorAbi>,
    /// Authoritative retained windows, nodes, input, layout, and semantic display list.
    pub(crate) window_manager: WindowManager,
    /// Composition-layer ownership for core choice-menu state and popup handles.
    choice_menus: ChoiceMenuManager,
}

impl RuntimeContext {
    /// Adopts a fresh compositor and the exact resource revision used to construct it.
    /// Callers must move an independent session here, never clone another runtime's executor.
    pub(crate) fn new(compositor: ComPtr<GfxCompositorAbi>, bundle: SkinBundle) -> Self {
        Self {
            resource_catalog: ResourceCatalog::new(bundle.atlas().clone()),
            compositor,
            window_manager: WindowManager::new(bundle),
            choice_menus: ChoiceMenuManager::default(),
        }
    }

    /// Registers one core choice-menu composition without teaching the root forest about headers.
    pub(crate) fn register_choice_menu(&mut self, state: ComPtr<IChoiceMenuStateAbi>, popup: PopupHandle, observed_revision: u64) {
        self.choice_menus.register(state, popup, observed_revision);
    }

    /// Reconciles typed choice-menu intent with generic popup operations through exclusive ownership.
    pub(crate) fn reconcile_choice_menus(&mut self) -> Result<bool, NcStatus> {
        self.choice_menus.reconcile(&mut self.window_manager)
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
        self.window_manager.paint(dimensions)
    }

    /// Releases commands and their resource references after a rejected paint phase.
    pub(crate) fn discard_paint(&mut self) {
        self.window_manager.display_list_mut().clear();
    }

    /// Submits a successfully checked paint phase through the real GFX compositor.
    pub(crate) fn submit(&mut self, size: Size, clear: u32, renderer: ComRef<'_, IRendererAbi>) -> Result<(), NcStatus> {
        // Clone the cheap immutable atlas handle before mutably borrowing the display list.
        let atlas = self.window_manager.skin_bundle().atlas().clone();
        crate::render::compositor::submit(self.window_manager.display_list_mut(), &atlas, &mut self.compositor, renderer, size, clear)
    }

    /// Installs the copied resource revision in the GFX owner before UI publishes its projection.
    pub(crate) fn install_compositor_assets(&mut self, source: ComRef<'_, neocogi_gfx_contracts::IAssetBundleAbi>) -> Result<(), NcStatus> {
        self.compositor.install_assets(source)
    }
}
